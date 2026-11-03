//! P2 构建队列 + 事件总线(2026-11-04,用户路线图第 4 点)。
//!
//! 职责:场景编辑器提交打包任务后即可关闭编辑器,由后台单工作线程静默
//! 构建;每个阶段边界经 `build-queue:event`(tauri emit)推送实时进度,
//! 任务元数据+阶段事件持久化到 `<root>/.build/queue/<job>.json`,供
//! OpenSCP Actions 页查看运行列表与详情(对标 GitHub Actions)。
//!
//! 架构口径(用户指令 2026-11-04):分层解耦/函数式/无副作用——队列本身
//! 是唯一的副作用边界,构建执行委托给 asset_build_m1::build_from_schema;
//! 事件经 `progress` 通道从责任链流出(见 stages.rs 的 BuildContext)。

use crate::asset_build_m1::events::StageEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex, OnceLock,
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

pub const BUILD_QUEUE_EVENT: &str = "build-queue:event";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BuildJobStatus {
    Queued,
    Running,
    Finished,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildJob {
    pub id: String,
    pub project_root: String,
    pub schema: String,
    pub status: BuildJobStatus,
    pub submitted_at_ms: u64,
    pub finished_at_ms: Option<u64>,
    /// 迭代号(s 计数)由提交方解析写入;队列只记录展示名。
    pub label: Option<String>,
    pub error: Option<String>,
    pub events: Vec<StageEvent>,
    /// 构建摘要(成功时):包路径/资源数/sha256 等(M1BuildResult 子集)。
    pub result: Option<Json>,
}

impl BuildJob {
    fn new(root: &str, schema: String, label: Option<String>) -> Self {
        Self {
            id: format!(
                "b{:016x}-{:03}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0),
                JOB_SEQ.fetch_add(1, Ordering::Relaxed)
            ),
            project_root: root.to_string(),
            schema,
            status: BuildJobStatus::Queued,
            submitted_at_ms: now_ms(),
            finished_at_ms: None,
            label,
            error: None,
            events: Vec::new(),
            result: None,
        }
    }
}

static JOB_SEQ: AtomicU64 = AtomicU64::new(1);
static WORKER: OnceLock<mpsc::Sender<JobCommand>> = OnceLock::new();

enum JobCommand {
    Enqueue(String),
}

#[derive(Default)]
pub struct BuildQueueManager {
    jobs: Mutex<Vec<BuildJob>>,
    queue: Mutex<VecDeque<String>>,
    app: OnceLock<AppHandle>,
}

impl BuildQueueManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 启动时注入 app 句柄并拉起唯一工作线程。
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        let _ = self.app.set(app);
        let manager = Arc::clone(self);
        let (tx, rx) = mpsc::channel::<JobCommand>();
        let _ = WORKER.set(tx);
        std::thread::Builder::new()
            .name("openscp-build-queue".into())
            .spawn(move || {
                while let Ok(JobCommand::Enqueue(job_id)) = rx.recv() {
                    manager.run_job(&job_id);
                }
            })
            .expect("spawn build queue worker");
    }

    /// 提交构建任务(幂等入口:编辑器/Actions 页/命令行共用)。
    pub fn enqueue(
        &self,
        project_root: &str,
        schema: String,
        label: Option<String>,
    ) -> Result<String, String> {
        let job = BuildJob::new(project_root, schema, label);
        let id = job.id.clone();
        {
            let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
            jobs.push(job.clone());
            self.queue
                .lock()
                .expect("build queue poisoned")
                .push_back(id.clone());
        }
        self.persist(&job);
        self.emit_status(&job);
        if let Some(tx) = WORKER.get() {
            let _ = tx.send(JobCommand::Enqueue(id.clone()));
        }
        Ok(id)
    }

    /// 某项目的运行列表(内存态 + 已持久化历史)。
    pub fn list(&self, project_root: &str) -> Vec<BuildJob> {
        let mut seen = std::collections::BTreeMap::new();
        {
            let jobs = self.jobs.lock().expect("build queue jobs poisoned");
            for job in jobs.iter().filter(|j| j.project_root == project_root) {
                seen.insert(job.id.clone(), job.clone());
            }
        }
        for path in persisted_jobs(project_root) {
            if let Ok(job) = serde_json::from_str::<BuildJob>(
                &std::fs::read_to_string(&path).unwrap_or_default(),
            ) {
                seen.entry(job.id.clone()).or_insert(job);
            }
        }
        seen.into_values().collect()
    }

    /// 单任务详情(含全部阶段事件)。
    pub fn detail(&self, project_root: &str, job_id: &str) -> Option<BuildJob> {
        {
            let jobs = self.jobs.lock().expect("build queue jobs poisoned");
            if let Some(job) = jobs
                .iter()
                .find(|j| j.id == job_id && j.project_root == project_root)
            {
                return Some(job.clone());
            }
        }
        let path = persisted_job_path(project_root, job_id)?;
        serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
    }

    /// 工作线程主体:取任务 → Running → build_from_schema → 持久化/广播。
    fn run_job(&self, job_id: &str) {
        let job = {
            let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
            let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) else {
                return;
            };
            job.status = BuildJobStatus::Running;
            job.clone()
        };
        self.emit_status(&job);
        // 责任链在阻塞线程执行;阶段事件经通道由转发线程边跑边广播
        // (Actions 页实时行)。构建结束后排空并关闭通道。
        let (tx, rx) = mpsc::channel::<StageEvent>();
        let job_id_owned = job_id.to_string();
        let app_for_stream = self.app.get().cloned();
        let stream = std::thread::Builder::new()
            .name("openscp-build-events".into())
            .spawn(move || {
                while let Ok(event) = rx.recv() {
                    if let Some(app) = &app_for_stream {
                        let payload = serde_json::json!({ "jobId": job_id_owned, "event": event });
                        let _ = app.emit(BUILD_QUEUE_EVENT, payload);
                    }
                }
            });
        let schema = job.schema.clone();
        let root = PathBuf::from(&job.project_root);
        let outcome =
            crate::asset_build_m1::build_from_schema_with_progress(&root, &schema, Some(tx));
        if let Ok(worker) = stream {
            let _ = worker.join();
        }
        {
            let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.finished_at_ms = Some(now_ms());
                match outcome {
                    Ok(result) => {
                        job.status = BuildJobStatus::Finished;
                        job.result = Some(result_summary(&result));
                        job.events = result.events.clone();
                    }
                    Err(error) => {
                        job.status = BuildJobStatus::Failed;
                        job.error = Some(error);
                    }
                }
                let snapshot = job.clone();
                drop(jobs);
                self.persist(&snapshot);
                self.emit_status(&snapshot);
            }
        }
    }

    fn persist(&self, job: &BuildJob) {
        let dir = Path::new(&job.project_root).join(".build").join("queue");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let path = dir.join(format!("{}.json", job.id));
        if let Ok(bytes) = serde_json::to_vec_pretty(job) {
            let _ = crate::atomic_fs::write_atomic(&path, &bytes, false);
        }
    }

    fn emit_status(&self, job: &BuildJob) {
        if let Some(app) = self.app.get() {
            let payload = serde_json::json!({
                "jobId": job.id,
                "status": job.status,
                "label": job.label,
                "error": job.error,
                "finishedAtMs": job.finished_at_ms,
            });
            let _ = app.emit(BUILD_QUEUE_EVENT, payload);
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn persisted_jobs(project_root: &str) -> Vec<PathBuf> {
    let dir = Path::new(project_root).join(".build").join("queue");
    let mut out = Vec::new();
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                out.push(path);
            }
        }
    }
    out
}

fn persisted_job_path(project_root: &str, job_id: &str) -> Option<PathBuf> {
    let path = Path::new(project_root)
        .join(".build")
        .join("queue")
        .join(format!("{job_id}.json"));
    path.is_file().then_some(path)
}

/// M1BuildResult → Actions 页摘要(去重字段,避免整包序列化)。
fn result_summary(result: &crate::asset_build_m1::M1BuildResult) -> Json {
    serde_json::json!({
        "path": result.path,
        "sha256": result.sha256,
        "buildHash": result.build_hash,
        "resourceCount": result.resource_count,
        "deltaMode": result.delta_mode,
        "menuInstance": result.menu_instance,
        "scene": result.scene,
    })
}

// ---- tauri 命令层(前端 Actions 页/api 的唯一入口) ----

#[tauri::command]
pub async fn build_queue_enqueue(
    queue: tauri::State<'_, Arc<BuildQueueManager>>,
    root: String,
    schema: String,
    label: Option<String>,
) -> Result<String, crate::activity::CommandError> {
    queue
        .enqueue(&root, schema, label)
        .map_err(|e| crate::activity::CommandError::new("build_queue_enqueue_failed", e))
}

#[tauri::command]
pub async fn build_queue_list(
    queue: tauri::State<'_, Arc<BuildQueueManager>>,
    root: String,
) -> Result<Vec<BuildJob>, crate::activity::CommandError> {
    Ok(queue.list(&root))
}

#[tauri::command]
pub async fn build_queue_detail(
    queue: tauri::State<'_, Arc<BuildQueueManager>>,
    root: String,
    jobId: String,
) -> Result<Option<BuildJob>, crate::activity::CommandError> {
    Ok(queue.detail(&root, &jobId))
}
