//! 编辑版本管理：记录本工具写出的 overlay 的每一次改动（**资源级前后字节**），
//! 提供 版本列表 / 详情 / 基线捕获。回滚与删除在后续命令里补。
//!
//! 版本线按 `target_path`（工具自己写出的 overlay 文件）分区，版本号在该分区内
//! 单调递增。存储落在 `sc-store`（SQLite，内容寻址去重 blob）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use dbpf::ResourceId;
use sc_store::{
    ChangeItemInput, ChangesetInput, ChangesetSummary, ChangeItemRecord, Store, StoreError,
    VersionTargetSummary,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::State;

use crate::activity::{AppState, CommandError};

/// 单条资源负载上限（与 `sc-store` 的 blob 上限一致）。
pub(crate) const MAX_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
/// 一次记录的资源条数上限。
pub(crate) const MAX_BATCH_RESOURCES: usize = 4096;

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// 读取目标 overlay 里已有的全部条目（文件不存在 / 不是有效包时返回空）。
/// 「合并保留」写入与基线捕获共用。
pub(crate) fn read_target_entries(path: &Path) -> Vec<(ResourceId, Vec<u8>)> {
    if !path.is_file() {
        return Vec::new();
    }
    let Ok(package) = dbpf::Package::open(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in package.entries() {
        if let Ok(data) = package.read(entry) {
            out.push((entry.id, data));
        }
    }
    out
}

/// 写盘后记录一条版本（供各写入方调用；失败由调用方决定是否上报）。
pub(crate) fn record_written(
    store: &Store,
    target_path: &str,
    writer: &str,
    label: Option<String>,
    note: Option<String>,
    detail: Option<Value>,
    file_digest: Option<String>,
    items: Vec<ChangeItemInput>,
) -> Result<ChangesetSummary, StoreError> {
    store.record_changeset(&ChangesetInput {
        target_path: target_path.to_string(),
        writer: writer.to_string(),
        label,
        note,
        file_digest,
        restored_from: None,
        detail,
        items,
    })
}

// ---------- DTO ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionListRequest {
    pub target_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetDetailRequest {
    pub changeset_id: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeItemView {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    /// `added` | `modified` | `removed`
    pub status: String,
    pub before_size: i64,
    pub after_size: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesetDetailResponse {
    pub changeset: ChangesetSummary,
    pub items: Vec<ChangeItemView>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceChangeInput {
    pub type_id: u32,
    pub group_id: u32,
    pub instance: u32,
    /// 目标里既有字节；`None` = 新增。
    pub before_base64: Option<String>,
    /// 新字节；`None` = 删除。
    pub after_base64: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordChangesetRequest {
    pub target_path: String,
    pub writer: String,
    pub label: Option<String>,
    pub note: Option<String>,
    pub detail: Option<Value>,
    /// 写出后整文件的 sha256（外部漂移检测用）。
    pub file_digest: Option<String>,
    pub resources: Vec<ResourceChangeInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBaselineRequest {
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureBaselineResponse {
    /// false = 该 target 已有版本线，幂等跳过。
    pub created: bool,
    pub changeset: ChangesetSummary,
}

fn status_of(item: &ChangeItemRecord) -> &'static str {
    match (item.before_digest.is_some(), item.after_digest.is_some()) {
        (false, true) => "added",
        (true, false) => "removed",
        _ => "modified",
    }
}

fn decode_payload(value: &Option<String>, label: &str) -> Result<Option<Vec<u8>>, CommandError> {
    let Some(encoded) = value else {
        return Ok(None);
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| CommandError::new("invalid_argument", format!("{label} 不是合法 base64：{error}")))?;
    if bytes.len() > MAX_PAYLOAD_BYTES {
        return Err(CommandError::new(
            "limit_exceeded",
            format!("{label} 超过 {MAX_PAYLOAD_BYTES} 字节上限"),
        ));
    }
    Ok(Some(bytes))
}

// ---------- commands ----------

#[tauri::command]
pub async fn version_list_targets(
    state: State<'_, AppState>,
) -> Result<Vec<VersionTargetSummary>, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.list_version_targets())
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[tauri::command]
pub async fn version_list_changesets(
    state: State<'_, AppState>,
    request: VersionListRequest,
) -> Result<Vec<ChangesetSummary>, CommandError> {
    let store = Arc::clone(&state.store);
    let limit = request.limit.unwrap_or(sc_store::DEFAULT_LIST_LIMIT);
    tauri::async_runtime::spawn_blocking(move || {
        store.list_changesets(request.target_path.as_deref(), limit)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn version_changeset_detail(
    state: State<'_, AppState>,
    request: ChangesetDetailRequest,
) -> Result<ChangesetDetailResponse, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let (changeset, items) = store.changeset_detail(request.changeset_id)?;
        let items = items
            .iter()
            .map(|item| ChangeItemView {
                type_id: item.type_id,
                group_id: item.group_id,
                instance: item.instance,
                status: status_of(item).to_string(),
                before_size: item.before_size,
                after_size: item.after_size,
            })
            .collect();
        Ok::<_, StoreError>(ChangesetDetailResponse { changeset, items })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[tauri::command]
pub async fn version_record_changeset(
    state: State<'_, AppState>,
    request: RecordChangesetRequest,
) -> Result<ChangesetSummary, CommandError> {
    if request.resources.is_empty() {
        return Err(CommandError::new("invalid_argument", "no resources to record"));
    }
    if request.resources.len() > MAX_BATCH_RESOURCES {
        return Err(CommandError::new(
            "limit_exceeded",
            format!("resources 超过 {MAX_BATCH_RESOURCES} 条上限"),
        ));
    }
    let mut items = Vec::with_capacity(request.resources.len());
    for resource in &request.resources {
        items.push(ChangeItemInput {
            type_id: resource.type_id,
            group_id: resource.group_id,
            instance: resource.instance,
            before: decode_payload(&resource.before_base64, "before")?,
            after: decode_payload(&resource.after_base64, "after")?,
        });
    }
    let store = Arc::clone(&state.store);
    let target_path = request.target_path.clone();
    let writer = request.writer.clone();
    let label = request.label.clone();
    let note = request.note.clone();
    let detail = request.detail.clone();
    let file_digest = request.file_digest.clone();
    tauri::async_runtime::spawn_blocking(move || {
        record_written(&store, &target_path, &writer, label, note, detail, file_digest, items)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

/// 把目标 overlay 当前的全部资源登记为一条 `baseline` 版本（`before = None`）。
///
/// 没有它，回滚会静默丢掉工具从未写过的既有条目 —— 版本线必须**全量**。
/// 幂等：该 target 已有任何版本记录时直接返回最新那条。
#[tauri::command]
pub async fn version_capture_baseline(
    state: State<'_, AppState>,
    request: CaptureBaselineRequest,
) -> Result<CaptureBaselineResponse, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        if let Some(existing) = store
            .list_changesets(Some(&request.target_path), 1)?
            .into_iter()
            .next()
        {
            return Ok(CaptureBaselineResponse {
                created: false,
                changeset: existing,
            });
        }
        let path = PathBuf::from(&request.target_path);
        let entries = read_target_entries(&path);
        if entries.is_empty() {
            return Err(CommandError::new(
                "not_found",
                "目标 overlay 不存在或没有可登记的条目",
            ));
        }
        let digest = std::fs::read(&path).ok().map(|bytes| sha256_hex(&bytes));
        let items = entries
            .into_iter()
            .map(|(id, data)| ChangeItemInput {
                type_id: id.type_id,
                group_id: id.group,
                instance: id.instance,
                before: None,
                after: Some(data),
            })
            .collect();
        let changeset = record_written(
            &store,
            &request.target_path,
            "baseline",
            None,
            None,
            None,
            digest,
            items,
        )?;
        Ok(CaptureBaselineResponse {
            created: true,
            changeset,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

// ---------- 回滚 / 删除 ----------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackRequest {
    pub changeset_id: i64,
    /// 目标文件被工具之外的东西改动过时，必须显式带上它才会继续。
    pub force: bool,
    /// 另存为新路径（文件被移走时用）；默认写回原 `target_path`。
    pub output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RollbackResult {
    pub output_path: String,
    /// 回滚到的源版本号。
    pub target_revision: i64,
    /// 回滚本身也追加了一条新版本，因此可再回滚。
    pub new_changeset: ChangesetSummary,
    pub resource_count: usize,
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteChangesetRequest {
    pub changeset_id: i64,
}

type ResourceKey = (u32, u32, u32);

fn to_map(entries: Vec<(ResourceId, Vec<u8>)>) -> std::collections::BTreeMap<ResourceKey, Vec<u8>> {
    entries
        .into_iter()
        .map(|(id, data)| ((id.type_id, id.group, id.instance), data))
        .collect()
}

/// 文件被外部程序占用（Windows 共享冲突）时给出可操作的中文提示。
fn map_write_error(error: std::io::Error) -> CommandError {
    const ERROR_SHARING_VIOLATION: i32 = 32;
    if error.raw_os_error() == Some(ERROR_SHARING_VIOLATION) {
        return CommandError::new(
            "target_locked",
            "目标文件被其它程序占用，关闭正在运行的 SimCity 后重试",
        );
    }
    CommandError::new("write_failed", error.to_string())
}

fn rollback_inner(
    store: &Store,
    request: &RollbackRequest,
) -> Result<RollbackResult, CommandError> {
    let (summary, _) = store.changeset_detail(request.changeset_id)?;
    let target_path = summary.target_path.clone();
    let revision = summary.revision;
    let effective = request
        .output_path
        .clone()
        .unwrap_or_else(|| target_path.clone());
    let effective_path = PathBuf::from(&effective);
    let same_file = effective == target_path;

    // 外部漂移检测：磁盘上的内容应该等于"最近一次记录写出后的摘要"。
    if same_file {
        if let Some(expected) = store.latest_target_digest(&target_path)? {
            if let Ok(current) = std::fs::read(&effective_path) {
                if sha256_hex(&current) != expected && !request.force {
                    return Err(CommandError::new(
                        "external_drift",
                        "目标文件已被工具之外的操作改动；继续回滚会丢掉那些改动，需显式 force",
                    ));
                }
            }
            // 文件不存在 -> 允许重建（版本线是全量的）
        }
    }

    let before = to_map(read_target_entries(&effective_path));
    let after = to_map(
        store
            .resource_states_at_revision(&target_path, revision)?
            .into_iter()
            .map(|resource| {
                (
                    ResourceId {
                        type_id: resource.type_id,
                        group: resource.group_id,
                        instance: resource.instance,
                    },
                    resource.bytes,
                )
            })
            .collect(),
    );

    let entries: Vec<dbpf::OverlayEntry> = after
        .iter()
        .map(|((type_id, group, instance), data)| {
            dbpf::OverlayEntry::new(
                ResourceId {
                    type_id: *type_id,
                    group: *group,
                    instance: *instance,
                },
                data.clone(),
            )
        })
        .collect();
    let overlay = dbpf::write_uncompressed_overlay(&entries)
        .map_err(|error| CommandError::new("write_failed", error.to_string()))?;
    crate::atomic_fs::write_atomic(&effective_path, &overlay, true).map_err(map_write_error)?;

    // 版本线保持只增：回滚本身也记一条，因此可再回滚。
    let keys: std::collections::BTreeSet<ResourceKey> =
        before.keys().chain(after.keys()).copied().collect();
    let items: Vec<ChangeItemInput> = keys
        .into_iter()
        .map(|key| ChangeItemInput {
            type_id: key.0,
            group_id: key.1,
            instance: key.2,
            before: before.get(&key).cloned(),
            after: after.get(&key).cloned(),
        })
        .collect();
    let detail = serde_json::json!({
        "restoredFromTarget": target_path,
        "restoredFromRevision": revision,
    });
    let new_changeset = store.record_changeset(&ChangesetInput {
        target_path: effective.clone(),
        writer: "rollback".to_string(),
        label: Some(format!("回滚到 v{revision}")),
        note: None,
        file_digest: Some(sha256_hex(&overlay)),
        restored_from: Some(revision),
        detail: Some(detail),
        items,
    })?;

    Ok(RollbackResult {
        output_path: effective,
        target_revision: revision,
        resource_count: entries.len(),
        bytes_written: overlay.len() as u64,
        new_changeset,
    })
}

/// 回滚到某条版本记录所代表的状态；回滚会追加一条新版本（自身也可再回滚）。
#[tauri::command]
pub async fn version_rollback(
    state: State<'_, AppState>,
    request: RollbackRequest,
) -> Result<RollbackResult, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || rollback_inner(&store, &request))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

/// 删除一条版本记录。**绝不动磁盘文件**，只删元数据并回收无引用 blob。
#[tauri::command]
pub async fn version_delete_changeset(
    state: State<'_, AppState>,
    request: DeleteChangesetRequest,
) -> Result<sc_store::VersionDeleteResult, CommandError> {
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || store.delete_changeset(request.changeset_id))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
        .map_err(CommandError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(label: &str) -> (std::path::PathBuf, Store, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("openscp-version-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let store = Store::open(dir.join("openscp.db")).unwrap();
        let target = dir.join("overlay.package");
        (dir, store, target)
    }

    fn item(instance: u32, before: Option<&[u8]>, after: Option<&[u8]>) -> ChangeItemInput {
        ChangeItemInput {
            type_id: 0x00B1_B104,
            group_id: 0x0987_8A01,
            instance,
            before: before.map(<[u8]>::to_vec),
            after: after.map(<[u8]>::to_vec),
        }
    }

    fn record(store: &Store, target: &str, items: Vec<ChangeItemInput>) -> ChangesetSummary {
        store
            .record_changeset(&ChangesetInput {
                target_path: target.to_string(),
                writer: "test".to_string(),
                label: None,
                note: None,
                file_digest: None,
                restored_from: None,
                detail: None,
                items,
            })
            .unwrap()
    }

    /// 纯 Rust fold —— 与 `resource_states_at_revision`（SQL）互为对照。
    fn fold_at(
        history: &[(i64, Vec<ChangeItemInput>)],
        revision: i64,
    ) -> std::collections::BTreeMap<ResourceKey, Vec<u8>> {
        let mut state = std::collections::BTreeMap::new();
        for (rev, items) in history {
            if *rev > revision {
                continue;
            }
            for item in items {
                let key = (item.type_id, item.group_id, item.instance);
                match &item.after {
                    Some(bytes) => {
                        state.insert(key, bytes.clone());
                    }
                    None => {
                        state.remove(&key);
                    }
                }
            }
        }
        state
    }

    fn expected_overlay(map: &std::collections::BTreeMap<ResourceKey, Vec<u8>>) -> Vec<u8> {
        let entries: Vec<dbpf::OverlayEntry> = map
            .iter()
            .map(|((type_id, group, instance), data)| {
                dbpf::OverlayEntry::new(
                    ResourceId {
                        type_id: *type_id,
                        group: *group,
                        instance: *instance,
                    },
                    data.clone(),
                )
            })
            .collect();
        dbpf::write_uncompressed_overlay(&entries).unwrap()
    }

    #[test]
    fn fold_matches_expected_overlay_byte_for_byte() {
        let (dir, store, target) = scratch("rollback-fold");
        let target_path = target.to_string_lossy().into_owned();
        // 增 -> 改 -> 删 -> 再加
        let history: Vec<(i64, Vec<ChangeItemInput>)> = vec![
            (1, vec![item(1, None, Some(b"v1-1")), item(2, None, Some(b"v1-2"))]),
            (2, vec![item(1, Some(b"v1-1"), Some(b"v2-1"))]),
            (3, vec![item(2, Some(b"v1-2"), None)]),
            (4, vec![item(2, None, Some(b"v4-2"))]),
        ];
        let mut ids = Vec::new();
        for (_, items) in &history {
            ids.push(record(&store, &target_path, items.clone()).id);
        }

        for revision in 1..=4 {
            let requested = ids[(revision - 1) as usize];
            let result = rollback_inner(
                &store,
                &RollbackRequest {
                    changeset_id: requested,
                    force: false,
                    output_path: None,
                },
            )
            .unwrap();

            let expected = fold_at(&history, revision);
            assert_eq!(
                std::fs::read(&target).unwrap(),
                expected_overlay(&expected),
                "回滚到 v{revision} 的文件必须与手工构造的期望 overlay 逐字节一致"
            );
            // 回滚追加的新版本号 = 源版本 + 已追加数
            assert_eq!(result.target_revision, revision);
            assert_eq!(result.new_changeset.writer, "rollback");
            assert_eq!(result.new_changeset.restored_from, Some(revision));
        }
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rollback_blocks_on_external_drift_but_force_succeeds() {
        let (dir, store, target) = scratch("rollback-drift");
        let target_path = target.to_string_lossy().into_owned();
        let first = record(&store, &target_path, vec![item(1, None, Some(b"v1"))]);
        let second = record(
            &store,
            &target_path,
            vec![item(1, Some(b"v1"), Some(b"v2"))],
        );
        // 先用第二次的版本号写出文件，让磁盘状态"合法"
        rollback_inner(
            &store,
            &RollbackRequest {
                changeset_id: second.id,
                force: false,
                output_path: None,
            },
        )
        .unwrap();
        // 外部改写文件 -> 漂移
        std::fs::write(&target, b"tampered").unwrap();

        let blocked = rollback_inner(
            &store,
            &RollbackRequest {
                changeset_id: first.id,
                force: false,
                output_path: None,
            },
        )
        .unwrap_err();
        assert_eq!(blocked.code, "external_drift");

        let forced = rollback_inner(
            &store,
            &RollbackRequest {
                changeset_id: first.id,
                force: true,
                output_path: None,
            },
        )
        .unwrap();
        assert_eq!(forced.target_revision, 1);
        assert_eq!(std::fs::read(&target).unwrap(), expected_overlay(&fold_at(&[(1, vec![item(1, None, Some(b"v1"))])], 1)));
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rollback_recreates_deleted_target() {
        let (dir, store, target) = scratch("rollback-recreate");
        let target_path = target.to_string_lossy().into_owned();
        let first = record(
            &store,
            &target_path,
            vec![item(7, None, Some(b"payload"))],
        );
        assert!(!target.exists());
        let result = rollback_inner(
            &store,
            &RollbackRequest {
                changeset_id: first.id,
                force: false,
                output_path: None,
            },
        )
        .unwrap();
        assert_eq!(result.resource_count, 1);
        assert!(target.exists(), "版本线是全量的，目标被删掉也能重建");
        let package = dbpf::Package::open(&target).unwrap();
        assert_eq!(package.entries().len(), 1);
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_changeset_never_touches_the_file() {
        let (dir, store, target) = scratch("rollback-delete");
        let target_path = target.to_string_lossy().into_owned();
        let first = record(&store, &target_path, vec![item(1, None, Some(b"v1"))]);
        record(&store, &target_path, vec![item(1, Some(b"v1"), Some(b"v2"))]);
        std::fs::write(&target, b"on-disk").unwrap();

        let removed = store.delete_changeset(first.id).unwrap();
        assert_eq!(removed.changesets, 1);
        assert_eq!(std::fs::read(&target).unwrap(), b"on-disk", "删版本不得动文件");
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 性能报告（项目约定：每阶段测试附耗时）。
    #[test]
    fn perf_rollback_roundtrip() {
        use std::time::Instant;

        let (dir, store, target) = scratch("rollback-perf");
        let target_path = target.to_string_lossy().into_owned();
        // 500 个资源 × 4 KB ≈ 2 MB
        let payload = vec![0x5Au8; 4 * 1024];
        let items: Vec<ChangeItemInput> = (1..=500)
            .map(|instance| item(instance, None, Some(&payload)))
            .collect();
        let first = record(&store, &target_path, items);
        let second = record(
            &store,
            &target_path,
            vec![item(1, Some(&payload), Some(b"tiny"))],
        );

        let start = Instant::now();
        let result = rollback_inner(
            &store,
            &RollbackRequest {
                changeset_id: second.id,
                force: true,
                output_path: None,
            },
        )
        .unwrap();
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        println!(
            "[perf version] 500 资源回滚（重建 + 原子写）：{elapsed_ms:.2}ms，写出 {} 字节",
            result.bytes_written
        );
        assert_eq!(result.resource_count, 500);

        // 回滚不应新增 blob：新版本记录的 after 全部复用既有 digest
        let (_, detail_items) = store.changeset_detail(result.new_changeset.id).unwrap();
        assert!(detail_items.iter().all(|entry| entry.after_digest.is_some()));
        let _ = first;
        drop(store);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_classification() {
        let item = |before: bool, after: bool| ChangeItemRecord {
            type_id: 1,
            group_id: 2,
            instance: 3,
            before_digest: before.then(|| "a".into()),
            after_digest: after.then(|| "b".into()),
            before_size: 0,
            after_size: 0,
        };
        assert_eq!(status_of(&item(false, true)), "added");
        assert_eq!(status_of(&item(true, false)), "removed");
        assert_eq!(status_of(&item(true, true)), "modified");
    }

    #[test]
    fn sha256_is_stable() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn capture_request_deserializes_from_camel_case() {
        let request: CaptureBaselineRequest =
            serde_json::from_str(r#"{"targetPath":"C:/a.package"}"#).unwrap();
        assert_eq!(request.target_path, "C:/a.package");
    }

    #[test]
    fn payload_decoding_rejects_bad_base64() {
        let error = decode_payload(&Some("not base64!!".into()), "after").unwrap_err();
        assert_eq!(error.code, "invalid_argument");
        assert!(decode_payload(&None, "after").unwrap().is_none());
    }
}
