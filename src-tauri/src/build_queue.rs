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
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};

pub const BUILD_QUEUE_EVENT: &str = "build-queue:event";
const BUILD_TIMEOUT_MS: u64 = 45 * 60_000;
const TIMEOUT_ERROR: &str = "BUILD_QUEUE_TIMEOUT";
const INTERRUPTED_ERROR: &str = "BUILD_QUEUE_INTERRUPTED";

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
    #[serde(default)]
    pub started_at_ms: Option<u64>,
    pub finished_at_ms: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// 迭代号(s 计数)由提交方解析写入;队列只记录展示名。
    pub label: Option<String>,
    pub error: Option<String>,
    pub events: Vec<StageEvent>,
    /// 构建摘要(成功时):包路径/资源数/sha256 等(M1BuildResult 子集)。
    pub result: Option<Json>,
}

impl BuildJob {
    fn deadline(&self) -> u64 {
        self.submitted_at_ms.saturating_add(BUILD_TIMEOUT_MS)
    }

    fn is_pending(&self) -> bool {
        matches!(
            self.status,
            BuildJobStatus::Queued | BuildJobStatus::Running
        )
    }

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
            started_at_ms: None,
            finished_at_ms: None,
            duration_ms: None,
            label,
            error: None,
            events: Vec::new(),
            result: None,
        }
    }
}

static JOB_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct BuildQueueManager {
    jobs: Mutex<Vec<BuildJob>>,
    queue: Mutex<VecDeque<String>>,
    ready: Condvar,
    app: OnceLock<AppHandle>,
}

impl BuildQueueManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 启动时注入 app 句柄并拉起唯一工作线程。
    pub fn start(self: &Arc<Self>, app: AppHandle) {
        if self.app.set(app).is_err() {
            return;
        }
        let manager = Arc::clone(self);
        std::thread::Builder::new()
            .name("openscp-build-queue".into())
            .spawn(move || {
                loop {
                    let job_id = manager.next_job();
                    manager.run_job(&job_id);
                }
            })
            .expect("spawn build queue worker");
        let manager = Arc::clone(self);
        std::thread::Builder::new()
            .name("openscp-build-timeout".into())
            .spawn(move || {
                loop {
                    manager.expire_live_jobs(now_ms());
                    std::thread::sleep(Duration::from_secs(1));
                }
            })
            .expect("spawn build queue watchdog");
    }

    fn next_job(&self) -> String {
        let queue = self.queue.lock().expect("build queue poisoned");
        let mut queue = self
            .ready
            .wait_while(queue, |queue| queue.is_empty())
            .expect("build queue poisoned");
        queue.pop_front().expect("nonempty build queue")
    }

    /// 提交构建任务(幂等入口:编辑器/Actions 页/命令行共用)。
    pub fn enqueue(
        &self,
        project_root: &str,
        schema: String,
        label: Option<String>,
    ) -> Result<String, String> {
        if self.app.get().is_none() {
            return Err("build queue worker is not running".into());
        }
        self.recover_orphaned_jobs(project_root);
        let job = BuildJob::new(project_root.trim(), schema, label);
        let id = job.id.clone();
        let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
        self.persist(&job).map_err(|error| error.to_string())?;
        self.emit_status(&job);
        jobs.push(job);
        self.queue
            .lock()
            .expect("build queue poisoned")
            .push_back(id.clone());
        self.ready.notify_one();
        Ok(id)
    }

    /// 某项目的运行列表(内存态 + 已持久化历史)。
    pub fn list(&self, project_root: &str) -> Vec<BuildJob> {
        let root = project_root.trim();
        self.expire_live_jobs(now_ms());
        self.recover_orphaned_jobs(root);
        let mut seen = std::collections::BTreeMap::new();
        {
            let jobs = self.jobs.lock().expect("build queue jobs poisoned");
            for job in jobs
                .iter()
                .filter(|j| same_project_root(&j.project_root, root))
            {
                seen.insert(job.id.clone(), job.clone());
            }
        }
        for path in persisted_jobs(root) {
            if let Ok(job) = serde_json::from_str::<BuildJob>(
                &std::fs::read_to_string(&path).unwrap_or_default(),
            ) {
                match seen.get(&job.id) {
                    // Disk terminal state must win over a stale in-memory
                    // Queued/Running snapshot — otherwise Actions keeps a
                    // multi-hour "排队中" after we already failed the job on disk.
                    Some(existing) if status_rank(existing.status) >= status_rank(job.status) => {}
                    _ => {
                        seen.insert(job.id.clone(), job);
                    }
                }
            }
        }
        // Keep memory in sync when disk won with Failed/Finished.
        {
            let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
            for job in jobs
                .iter_mut()
                .filter(|j| same_project_root(&j.project_root, root))
            {
                if let Some(canonical) = seen.get(&job.id) {
                    if status_rank(canonical.status) > status_rank(job.status) {
                        *job = canonical.clone();
                    }
                }
            }
        }
        seen.into_values().collect()
    }

    fn expire_live_jobs(&self, now: u64) {
        let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
        for job in jobs
            .iter_mut()
            .filter(|job| job.is_pending() && job.deadline() <= now)
        {
            self.queue
                .lock()
                .expect("build queue poisoned")
                .retain(|id| id != &job.id);
            fail_job(job, job.deadline(), TIMEOUT_ERROR);
            self.publish(job);
        }
    }

    /// A job from a previous session has no worker to resume it.
    fn recover_orphaned_jobs(&self, project_root: &str) {
        let jobs = self.jobs.lock().expect("build queue jobs poisoned");
        let now = now_ms();
        for path in persisted_jobs(project_root.trim()) {
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(mut job) = serde_json::from_str::<BuildJob>(&raw) else {
                continue;
            };
            if !job.is_pending() || jobs.iter().any(|live| live.id == job.id) {
                continue;
            }
            let deadline = job.deadline();
            if deadline <= now {
                fail_job(&mut job, deadline, TIMEOUT_ERROR);
            } else {
                fail_job(&mut job, now, INTERRUPTED_ERROR);
            }
            // Use the directory being recovered, even if the project was moved.
            job.project_root = project_root.trim().to_string();
            self.publish(&job);
        }
    }

    /// 单任务详情(含全部阶段事件)。
    pub fn detail(&self, project_root: &str, job_id: &str) -> Option<BuildJob> {
        let root = project_root.trim();
        self.expire_live_jobs(now_ms());
        self.recover_orphaned_jobs(root);
        {
            let jobs = self.jobs.lock().expect("build queue jobs poisoned");
            if let Some(job) = jobs
                .iter()
                .find(|j| j.id == job_id && same_project_root(&j.project_root, root))
            {
                return Some(job.clone());
            }
        }
        let path = persisted_job_path(root, job_id)?;
        let disk = serde_json::from_str::<BuildJob>(&std::fs::read_to_string(path).ok()?).ok()?;
        Some(disk)
    }

    fn start_job(&self, job_id: &str, now: u64) -> Option<BuildJob> {
        let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
        let job = jobs
            .iter_mut()
            .find(|job| job.id == job_id && job.status == BuildJobStatus::Queued)?;
        let deadline = job.deadline();
        if deadline <= now {
            fail_job(job, deadline, TIMEOUT_ERROR);
            self.publish(job);
            return None;
        }
        job.status = BuildJobStatus::Running;
        job.started_at_ms = Some(now);
        self.publish(job);
        Some(job.clone())
    }

    fn run_job(&self, job_id: &str) {
        let Some(job) = self.start_job(job_id, now_ms()) else {
            return;
        };
        let (tx, rx) = mpsc::channel::<StageEvent>();
        let outcome = std::thread::scope(|scope| {
            scope.spawn(move || {
                while let Ok(event) = rx.recv() {
                    let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
                    let Some(job) = jobs
                        .iter_mut()
                        .find(|job| job.id == job_id && job.status == BuildJobStatus::Running)
                    else {
                        break;
                    };
                    if let Some(app) = self.app.get() {
                        let payload = serde_json::json!({
                            "jobId": job_id,
                            "label": job.label,
                            "totalStages": crate::asset_build_m1::stages::CHAIN.len(),
                            "event": event,
                        });
                        let _ = app.emit(BUILD_QUEUE_EVENT, payload);
                    }
                    job.events.push(event);
                }
            });
            let root = Path::new(&job.project_root);
            std::panic::catch_unwind(|| {
                crate::asset_build_m1::build_from_schema_with_progress(root, &job.schema, Some(tx))
            })
            .unwrap_or_else(|_| Err(INTERRUPTED_ERROR.into()))
        });
        self.finish_job(job_id, now_ms(), outcome);
    }

    fn finish_job(
        &self,
        job_id: &str,
        now: u64,
        outcome: Result<crate::asset_build_m1::M1BuildResult, String>,
    ) {
        let mut jobs = self.jobs.lock().expect("build queue jobs poisoned");
        let Some(job) = jobs
            .iter_mut()
            .find(|job| job.id == job_id && job.status == BuildJobStatus::Running)
        else {
            return;
        };
        let deadline = job.deadline();
        if deadline <= now {
            fail_job(job, deadline, TIMEOUT_ERROR);
        } else {
            match outcome {
                Ok(result) => {
                    job.status = BuildJobStatus::Finished;
                    job.finished_at_ms = Some(now);
                    job.duration_ms = job.started_at_ms.map(|started| now.saturating_sub(started));
                    job.result = Some(result_summary(&result));
                    job.events = result.events;
                }
                Err(error) => fail_job(job, now, &error),
            }
        }
        self.publish(job);
    }

    fn persist(&self, job: &BuildJob) -> std::io::Result<()> {
        let dir = Path::new(&job.project_root).join(".build").join("queue");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.json", job.id));
        let bytes = serde_json::to_vec_pretty(job)?;
        crate::atomic_fs::write_atomic(&path, &bytes, true)
    }

    fn publish(&self, job: &BuildJob) {
        if let Err(error) = self.persist(job) {
            eprintln!("[build queue] persist {}: {error}", job.id);
        }
        self.emit_status(job);
    }

    fn emit_status(&self, job: &BuildJob) {
        if let Some(app) = self.app.get() {
            let payload = serde_json::json!({
                "jobId": job.id,
                "status": job.status,
                "label": job.label,
                "error": job.error,
                "startedAtMs": job.started_at_ms,
                "finishedAtMs": job.finished_at_ms,
                "durationMs": job.duration_ms,
                "totalStages": crate::asset_build_m1::stages::CHAIN.len(),
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

fn fail_job(job: &mut BuildJob, finished: u64, error: &str) {
    job.status = BuildJobStatus::Failed;
    job.finished_at_ms = Some(finished);
    job.duration_ms =
        Some(finished.saturating_sub(job.started_at_ms.unwrap_or(job.submitted_at_ms)));
    job.error = Some(error.to_string());
}

fn normalize_root(root: &str) -> String {
    let trimmed = root.trim().trim_end_matches(['/', '\\']);
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        out.push(if cfg!(windows) && ch == '/' { '\\' } else { ch });
    }
    if cfg!(windows) {
        out.make_ascii_lowercase();
    }
    out
}

fn same_project_root(left: &str, right: &str) -> bool {
    normalize_root(left) == normalize_root(right)
}

fn status_rank(status: BuildJobStatus) -> u8 {
    match status {
        BuildJobStatus::Queued => 0,
        BuildJobStatus::Running => 1,
        BuildJobStatus::Finished | BuildJobStatus::Failed => 2,
    }
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
        "startedAtMs": result.started_at_ms,
        "finishedAtMs": result.finished_at_ms,
        "durationMs": result.duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestProject(PathBuf);

    impl TestProject {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "openscp-queue-{}-{}",
                std::process::id(),
                JOB_SEQ.fetch_add(1, Ordering::Relaxed)
            )))
        }

        fn root(&self) -> &str {
            self.0.to_str().unwrap()
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn deadline_expires_running_and_queued_jobs_but_preserves_fresh_work() {
        let project = TestProject::new();
        let manager = BuildQueueManager::new();
        let running = BuildJob::new(project.root(), String::new(), None);
        let mut queued = running.clone();
        queued.id.push_str("-queued");
        let mut fresh = running.clone();
        fresh.id.push_str("-fresh");
        fresh.submitted_at_ms = running.deadline() - 1;
        for job in [&running, &queued, &fresh] {
            manager.persist(job).unwrap();
            manager.jobs.lock().unwrap().push(job.clone());
        }
        manager
            .queue
            .lock()
            .unwrap()
            .extend([queued.id.clone(), fresh.id.clone()]);

        manager
            .start_job(&running.id, running.submitted_at_ms)
            .unwrap();
        manager.expire_live_jobs(running.deadline());
        assert_eq!(*manager.queue.lock().unwrap(), VecDeque::from([fresh.id]));
        assert!(manager.start_job(&queued.id, running.deadline()).is_none());
        manager.finish_job(
            &running.id,
            running.deadline() + 1,
            Err("late error".into()),
        );

        for id in [running.id, queued.id] {
            let path = persisted_job_path(project.root(), &id).unwrap();
            let job: BuildJob = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
            assert_eq!(job.status, BuildJobStatus::Failed);
            assert_eq!(job.error.as_deref(), Some(TIMEOUT_ERROR));
            assert_eq!(job.duration_ms, Some(BUILD_TIMEOUT_MS));
        }
    }

    #[test]
    fn reloading_old_queue_history_persists_the_timeout() {
        let project = TestProject::new();
        let mut old = BuildJob::new(project.root(), String::new(), None);
        old.submitted_at_ms = now_ms() - BUILD_TIMEOUT_MS - 60_000;
        BuildQueueManager::new().persist(&old).unwrap();

        let jobs = BuildQueueManager::new().list(project.root());
        assert_eq!(jobs[0].status, BuildJobStatus::Failed);
        assert_eq!(jobs[0].error.as_deref(), Some(TIMEOUT_ERROR));
        let path = persisted_job_path(project.root(), &old.id).unwrap();
        let disk: BuildJob = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(disk.finished_at_ms, Some(old.deadline()));
        assert_eq!(disk.status, BuildJobStatus::Failed);
    }
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

/// 一键构建：从项目内唯一的 `openscp.asset` schema 自动入队（不再粘贴）。
#[tauri::command]
pub async fn build_queue_enqueue_project(
    queue: tauri::State<'_, Arc<BuildQueueManager>>,
    root: String,
    label: Option<String>,
) -> Result<Json, crate::activity::CommandError> {
    let root_path = Path::new(&root);
    let candidate = crate::asset_build_m1::schema_discovery::discover_or_adapt(root_path)
        .map_err(|e| crate::activity::CommandError::new("asset_schema_missing", e))?;
    // 工作台逻辑图 → schema 语义覆盖（菜单/建筑信息/行为）。文档是权威输入：
    // 编译诊断在入队前 fail-closed，错误点名节点 id，不进入构建队列。
    let mut schema: Json = serde_json::from_str(&candidate.raw).map_err(|error| {
        crate::activity::CommandError::new(
            "asset_schema_invalid",
            format!("{}: {error}", candidate.path),
        )
    })?;
    let mut logic_summary = Json::Null;
    if let Some(compiled) = crate::asset_build_m1::workbench_logic::overlay_document_on_schema(
        root_path,
        &mut schema,
        &candidate.document,
    )
    .map_err(|errors| {
        crate::activity::CommandError::new(
            "asset_logic_invalid",
            format!("{}: {}", candidate.document, errors.join("; ")),
        )
    })? {
        logic_summary = serde_json::json!({
            "menu": compiled.menu.is_some(),
            "labor": compiled.labor.is_some(),
            "behaviors": compiled.behaviors.len(),
            "warnings": compiled.warnings,
        });
    }
    let composed = serde_json::to_string(&schema).map_err(|error| {
        crate::activity::CommandError::new("asset_schema_invalid", error.to_string())
    })?;
    let job = queue
        .enqueue(&root, composed, label)
        .map_err(|e| crate::activity::CommandError::new("build_queue_enqueue_failed", e))?;
    Ok(serde_json::json!({
        "jobId": job,
        "schema": candidate.path,
        "logic": logic_summary,
    }))
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
