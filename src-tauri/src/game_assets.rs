//! 游戏资产解析服务：把 UI 布局树里的「路径引用」解析成玩家安装包里的资源字节。
//!
//! 游戏 ResourceMan 的路径寻址规则（2026-10-18 字节等价对拍定案）：
//!   instance = FNV-1(小写(去扩展名的资源路径))，group = 0，
//!   type = 0x2F7D0004(PNG) / 0x3F8662EA(JPG)。
//! 例：Graphics/ToolIcons/RCI_Residential_default.png
//!   → instance 0xB62E6ED4 @ SimCity_Game.package（8273 B 字节等价命中）。
//!
//! 后端把玩家安装的全部 package 建成一张合并索引（与游戏 ResourceMan 的
//! 全量内存视图同构），前端查询只需给出布局里的原始路径字符串。

use base64::Engine;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const TYPE_PNG: u32 = 0x2F7D_0004;
const TYPE_JPG: u32 = 0x3F86_62EA;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAssetData {
    pub data_base64: String,
    pub mime: String,
}

/// instance → [(包路径, 索引项)]；同一 instance 可在多个包出现，
/// group 0（布局路径寻址的约定组）排在前。
struct AssetIndex {
    entries: HashMap<u32, Vec<(PathBuf, dbpf::IndexEntry)>>,
}

static INDEX: Mutex<Option<AssetIndex>> = Mutex::new(None);

/// FNV-1（先乘后异或），与游戏资源命名一致；输入先转小写。
fn fnv1_lower(stem: &str) -> u32 {
    let mut hash: u32 = 0x811C_9DC5;
    for byte in stem.to_lowercase().as_bytes() {
        hash = hash.wrapping_mul(0x0100_0193);
        hash ^= u32::from(*byte);
    }
    hash
}

fn stem_of(path: &str) -> &str {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match file.rfind('.') {
        Some(dot) => &file[..dot],
        None => file,
    }
}

/// 玩家安装的全部 package：SimCityData 根 + Locale 子目录 +
/// SimCityUserData 的 EcoGame 与 Packages（与游戏扫描范围一致）。
fn package_paths(game_data: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let root = game_data.parent().unwrap_or(game_data).to_path_buf();
    let mut push_dir = |dir: PathBuf| {
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let is_package = path
                .extension()
                .and_then(|v| v.to_str())
                .map(|v| v.eq_ignore_ascii_case("package"))
                .unwrap_or(false);
            if is_package {
                out.push(path);
            }
        }
    };
    push_dir(game_data.to_path_buf());
    if let Ok(locale) = std::fs::read_dir(game_data.join("Locale")) {
        for lang in locale.flatten() {
            push_dir(lang.path());
        }
    }
    push_dir(root.join("SimCityUserData").join("EcoGame"));
    push_dir(root.join("SimCityUserData").join("Packages"));
    out
}

fn build_index(game_data: &Path) -> AssetIndex {
    let mut entries: HashMap<u32, Vec<(PathBuf, dbpf::IndexEntry)>> = HashMap::new();
    for path in package_paths(game_data) {
        let Ok(package) = dbpf::Package::open(&path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id == TYPE_PNG || entry.id.type_id == TYPE_JPG {
                entries
                    .entry(entry.id.instance)
                    .or_default()
                    .push((path.clone(), entry.clone()));
            }
        }
    }
    for list in entries.values_mut() {
        list.sort_by_key(|(_, entry)| u16::from(entry.id.group != 0));
    }
    AssetIndex { entries }
}

fn cached_index(game_data: &Path) -> std::sync::MutexGuard<'static, Option<AssetIndex>> {
    let mut guard = INDEX.lock().expect("game asset index poisoned");
    if guard.is_none() {
        *guard = Some(build_index(game_data));
    }
    guard
}

/// 直接按 TGI 查询（菜单条目的图标/大图用完整 TGI 引用，不走路径哈希）。
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameTgi {
    pub r#type: u32,
    pub instance: u32,
    /// 可选组过滤；缺省 = 不限组（group 0 优先）。
    pub group: Option<u32>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAssetRequest {
    /// 布局树里的原始图片路径（路径寻址），如
    /// `Graphics/ToolIcons/RCI_Residential_default.png`。
    #[serde(default)]
    pub path: Option<String>,
    /// 菜单条目的图标/大图走完整 TGI（toolIconKey/marquee 键的引用目标）。
    #[serde(default)]
    pub tgi: Option<GameTgi>,
}

#[tauri::command]
pub async fn game_asset(
    state: tauri::State<'_, crate::activity::AppState>,
    request: GameAssetRequest,
) -> Result<GameAssetData, crate::activity::CommandError> {
    use crate::activity::CommandError;
    let game_data = state
        .store
        .app_settings()
        .map_err(CommandError::from)?
        .and_then(|s| s.game_data_path)
        .ok_or_else(|| {
            CommandError::new("game_directory_not_configured", "Game data directory is not configured")
        })?;
    let game_data = PathBuf::from(game_data);
    if !game_data.is_dir() {
        return Err(CommandError::new(
            "game_directory_unavailable",
            "Game data directory is unavailable",
        ));
    }
    let (instance, want_type, want_group) = match (&request.tgi, &request.path) {
        (Some(tgi), _) => (tgi.instance, Some(tgi.r#type), tgi.group),
        (None, Some(path)) => (fnv1_lower(stem_of(path)), None, Some(0)),
        _ => {
            return Err(CommandError::new(
                "game_asset_bad_request",
                "provide either path or tgi",
            ))
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = cached_index(&game_data);
        let Some(index) = guard.as_ref() else {
            return Err(CommandError::internal("game asset index unavailable"));
        };
        let missing = || {
            CommandError::new(
                "game_asset_missing",
                format!("no package resource for instance {instance:08X}"),
            )
        };
        let candidates = index.entries.get(&instance).ok_or_else(missing)?;
        let candidates: Vec<_> = candidates
            .iter()
            .filter(|(_, entry)| want_type.map_or(true, |t| entry.id.type_id == t))
            .filter(|(_, entry)| {
                want_group.map_or(true, |g| {
                    entry.id.group == g || (g != 0 && entry.id.group != 0)
                })
            })
            .collect();
        let read_candidate = |(path, entry): (&PathBuf, &dbpf::IndexEntry)| -> Option<GameAssetData> {
            let Ok(package) = dbpf::Package::open(path) else { return None };
            let Ok(data) = package.read(entry) else { return None };
            let mime = if entry.id.type_id == TYPE_PNG { "image/png" } else { "image/jpeg" };
            Some(GameAssetData {
                data_base64: base64::engine::general_purpose::STANDARD.encode(&data),
                mime: mime.into(),
            })
        };
        let from_ref = index.entries.get(&instance).ok_or_else(missing)?;
        let from_filtered = candidates.iter().map(|(p, e)| (p, e));
        let from_all = from_ref.iter().map(|(p, e)| (p, e));
        for candidate in from_filtered.chain(from_all) {
            if let Some(result) = read_candidate(candidate) {
                return Ok(result);
            }
        }
        Err(CommandError::new(
            "game_asset_missing",
            format!("unreadable resource for instance {instance:08X}"),
        ))
    })
    .await
    .map_err(|e| crate::activity::CommandError::internal(e.to_string()))?
}
