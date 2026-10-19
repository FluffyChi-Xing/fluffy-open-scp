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

const TYPE_PROPERTY: u32 = 0x00B1_B104;
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
            // 全类型入索引：布局资产走 PNG/JPG，菜单条目复制走 property
            if matches!(
                entry.id.type_id,
                TYPE_PNG | TYPE_JPG | TYPE_PROPERTY
            ) {
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

// ── 菜单条目全量 property 复制 ─────────────────────────────────────────
// 复制 = 源条目 property 深拷贝 + 受控变换（归属/身份/文案指针/图标指针）。
// 家族键（parent、分段引用、路径单价、统计行…）随源条目原样保留——
// 这是 SCP 教程式复制的语义，保证新条目在游戏内功能完整。

use sc_properties::{Kind, PropType, Property, PropertyFile, Value};

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TgiRef {
    pub r#type: u32,
    pub group: u32,
    pub instance: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyCopyRequest {
    pub source_instance: u32,
    /// 源分组过滤：同 instance 可能存在于多个包/分组（如 BC45E41B 同时在
    /// DLC0 41F0C900 与 EP1 61F0C900），提供后精确命中工作台所见的那个源。
    pub source_group: Option<u32>,
    pub new_instance: u32,
    /// 统计行模板重写（全量 Text 引用，写自有表）：前端编辑统计行 label 后
    /// 传入；缺省保留源统计行。
    #[serde(default)]
    pub stat_title_refs: Option<Vec<StatTitleRef>>,
    pub ui_categories: Vec<u32>,
    pub title_string_id: Option<u32>,
    pub desc_string_id: Option<u32>,
    pub unlock_string_id: Option<u32>,
    /// 源有解锁文案而副本被清空 → 删除解锁键；缺省 false = 保留源引用
    /// （未编辑文案的拷贝，源串在原版/自有表里必然可得）。
    #[serde(default)]
    pub remove_unlock_string: bool,
    pub position: Option<i32>,
    pub icon_tgi: Option<TgiRef>,
    pub marquee_tgi: Option<TgiRef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatTitleRef {
    pub table: u32,
    pub instance: u32,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyCopyResult {
    pub data_base64: String,
    pub key_count: usize,
    /// 实际读取的源 property 分组——覆盖包必须用同分组放置才能命中。
    pub source_group: u32,
}

fn set_scalar_key(file: &mut PropertyFile, hash: u32, key: Value) {
    set_scalar_value(file, hash, PropType::Key, key);
}

fn fixed_array_item_size(prop_type: PropType) -> Option<i32> {
    match prop_type {
        PropType::Bool => Some(1),
        PropType::Int32 | PropType::UInt32 | PropType::Float => Some(4),
        PropType::Key => Some(12),
        // Text 数组步长是游戏加载器可见元数据（见 sc_properties::TEXT_ARRAY_ITEM_SIZE），
        // 必须按原版惯例声明 520；写 8 会让游戏解析统计行等数组失败。
        PropType::Text => Some(sc_properties::TEXT_ARRAY_ITEM_SIZE),
        PropType::Vector2 => Some(8),
        PropType::Vector3 | PropType::ColorRgb => Some(12),
        PropType::Vector4 | PropType::ColorRgba => Some(16),
        PropType::BoundingBox => Some(24),
        PropType::Transform | PropType::String8 | PropType::String16 => None,
    }
}

fn normalize_array_item_sizes(file: &mut PropertyFile) {
    for prop in &mut file.values {
        if matches!(prop.kind, Kind::Array(_)) {
            if let Some(size) = fixed_array_item_size(prop.prop_type) {
                prop.encoding.array_item_size = Some(size);
            }
        }
    }
}

fn remove_property(file: &mut PropertyFile, hash: u32) {
    file.values.retain(|prop| prop.hash != hash);
}

fn set_scalar_text(file: &mut PropertyFile, hash: u32, table: u32, id: u32) {
    let value = Value::Text(sc_properties::Text {
        table_id: table,
        instance_id: id,
    });
    for prop in &mut file.values {
        if prop.hash == hash {
            if prop.encoding.flags & 0x30 != 0 {
                prop.kind = Kind::Array(vec![value]);
                prop.encoding.array_item_size = Some(sc_properties::TEXT_ARRAY_ITEM_SIZE);
            } else {
                prop.kind = Kind::Scalar(value);
                prop.encoding.array_item_size = None;
            }
            prop.prop_type = PropType::Text;
            return;
        }
    }
    file.values.push(Property {
        hash,
        prop_type: PropType::Text,
        kind: Kind::Scalar(value),
        encoding: Default::default(),
    });
}

fn set_scalar_value(file: &mut PropertyFile, hash: u32, prop_type: PropType, value: Value) {
    for prop in &mut file.values {
        if prop.hash == hash {
            // Some game resources encode singleton values as one-item arrays.
            // Preserve that shape, but normalize the item size for the new type.
            if prop.encoding.flags & 0x30 != 0 {
                prop.kind = Kind::Array(vec![value]);
                prop.encoding.array_item_size = fixed_array_item_size(prop_type);
            } else {
                prop.kind = Kind::Scalar(value);
                prop.encoding.array_item_size = None;
            }
            prop.prop_type = prop_type;
            return;
        }
    }
    file.values.push(Property {
        hash,
        prop_type,
        kind: Kind::Scalar(value),
        encoding: Default::default(),
    });
}

fn set_array_values(file: &mut PropertyFile, hash: u32, prop_type: PropType, values: Vec<Value>) {
    for prop in &mut file.values {
        if prop.hash == hash {
            prop.kind = Kind::Array(values);
            prop.prop_type = prop_type;
            // Text 数组的 520 步长是游戏可见元数据（必须保留/声明），其余类型
            // 统一规范到 encode_canonical 要求的固定步长。
            prop.encoding.array_item_size = fixed_array_item_size(prop_type);
            return;
        }
    }
    file.values.push(Property {
        hash,
        prop_type,
        kind: Kind::Array(values),
        encoding: Default::default(),
    });
}

#[tauri::command]
pub async fn game_property_copy(
    state: tauri::State<'_, crate::activity::AppState>,
    request: PropertyCopyRequest,
) -> Result<PropertyCopyResult, crate::activity::CommandError> {
    use crate::activity::CommandError;
    let game_data = state
        .store
        .app_settings()
        .map_err(CommandError::from)?
        .and_then(|s| s.game_data_path)
        .ok_or_else(|| {
            CommandError::new(
                "game_directory_not_configured",
                "Game data directory is not configured",
            )
        })?;
    let game_data = PathBuf::from(game_data);
    let source_instance = request.source_instance;
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = cached_index(&game_data);
        let Some(index) = guard.as_ref() else {
            return Err(CommandError::internal("game asset index unavailable"));
        };
        // 索引可能是在旧代码下构建的（缺少 property 类型）——找不到时重建一次
        let candidates = match index.entries.get(&source_instance) {
            Some(c) if c.iter().any(|(_, e)| e.id.type_id == TYPE_PROPERTY) => c.clone(),
            _ => {
                *guard = Some(build_index(&game_data));
                let index = guard.as_ref().unwrap();
                index.entries.get(&source_instance).ok_or_else(|| {
                    CommandError::new(
                        "game_property_missing",
                        format!("source property 0x{source_instance:08X} not found"),
                    )
                })?.clone()
            }
        };
        let source_group = request.source_group;
        let mut copied: Option<(PropertyFile, u32)> = None;
        for (path, entry) in &candidates {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            if let Some(group) = source_group {
                if entry.id.group != group {
                    continue;
                }
            }
            let Ok(package) = dbpf::Package::open(path) else { continue };
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            copied = Some((file, entry.id.group));
            break;
        }
        let Some((mut file, source_group)) = copied else {
            return Err(CommandError::new(
                "game_property_missing",
                format!("no readable property for 0x{source_instance:08X}"),
            ));
        };

        // 归属：uiCategories 数组整体替换（核心编辑）
        set_array_values(
            &mut file,
            0x0DB9_FC63,
            PropType::Key,
            request
                .ui_categories
                .iter()
                .map(|c| {
                    Value::Key(sc_properties::Key {
                        type_id: 0,
                        group: 0,
                        instance: *c,
                    })
                })
                .collect(),
        );
        // 文案指针：提供了新字符串 id 才覆盖，否则沿用源条目字符串
        if let Some(id) = request.title_string_id {
            set_scalar_text(&mut file, 0x0A09_F5FA, sc_properties::locale::OPENSCP_LOCALE_TABLE, id);
        }
        if let Some(id) = request.desc_string_id {
            set_scalar_text(&mut file, 0x0A09_F5FB, sc_properties::locale::OPENSCP_LOCALE_TABLE, id);
        }
        if let Some(id) = request.unlock_string_id {
            set_scalar_text(&mut file, 0x0DE8_4DDC, sc_properties::locale::OPENSCP_LOCALE_TABLE, id);
        } else if request.remove_unlock_string {
            // 显式清空解锁文案：删除解锁键，避免副本携带悬空指针。
            remove_property(&mut file, 0x0DE8_4DDC);
            remove_property(&mut file, 0x0DE8_4DD3);
        }
        // 两者皆否 = 未编辑文案的拷贝：保留源解锁引用（ref-first）。
        // 统计行模板重写（前端编辑统计行 label 后的全量引用，写自有表）。
        if let Some(refs) = &request.stat_title_refs {
            let values = refs
                .iter()
                .map(|r| {
                    Value::Text(sc_properties::Text {
                        table_id: r.table,
                        instance_id: r.instance,
                    })
                })
                .collect();
            set_array_values(&mut file, 0x0EB1_FC05, PropType::Text, values);
        }
        // 排序
        if let Some(pos) = request.position {
            set_array_values(
                &mut file,
                0x0DC1_E3E0,
                PropType::Int32,
                vec![Value::Int32(pos)],
            );
        }
        // 图标/大图指针：用户替换了图才覆盖；沿用原图时不发新资源
        if let Some(tgi) = &request.icon_tgi {
            let key = Value::Key(sc_properties::Key {
                type_id: tgi.r#type,
                group: tgi.group,
                instance: tgi.instance,
            });
            set_scalar_key(&mut file, 0x0977_AA8F, key.clone());
            set_scalar_key(&mut file, 0x0975_6950, key);
        }
        if let Some(tgi) = &request.marquee_tgi {
            set_scalar_key(
                &mut file,
                0x0DDE_EE56,
                Value::Key(sc_properties::Key {
                    type_id: tgi.r#type,
                    group: tgi.group,
                    instance: tgi.instance,
                }),
            );
        }
        // Text 数组的 520 步长是游戏加载器可见元数据（实机验证：写成 8 会让
        // 统计行等数组解析失败）；整文件统一到各类型约定步长后再编码，包括
        // 未改动的统计/提示数组。
        normalize_array_item_sizes(&mut file);
        let key_count = file.values.len();
        let bytes = file
            .encode_canonical()
            .map_err(|e| CommandError::new("game_property_encode_failed", e.to_string()))?;
        Ok(PropertyCopyResult {
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
            key_count,
            source_group,
        })
    })
    .await
    .map_err(|e| crate::activity::CommandError::internal(e.to_string()))?
}

// ===== property read/patch 原语：Sheet 深层语义编辑（经济/统计行）的数据通道 =====
use tauri::State;
use crate::activity::CommandError;
// 数据规范由 encode_canonical 保证（对原版属性逐字节往返，见 sc-properties 测试）：
// float/int 补丁走类型化标量写入，Text 数组补丁走 520 步长数组重写。

/// 候选包挑选启发式：EcoGame 脚本包带版本号（Scripts_<v>），取路径中最长
/// 数字串最大者 = 最新版本；无版本号的包得 0 分排在前。
fn candidate_score(path: &Path) -> u64 {
    path.to_string_lossy()
        .split(|c: char| !c.is_ascii_digit())
        .filter(|segment| segment.len() >= 6)
        .filter_map(|segment| segment.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
}

/// 按 instance（可选 group 过滤）挑出最佳 property 候选：版本号新者优先。
fn pick_property_candidate<'a>(
    candidates: &'a [(PathBuf, dbpf::IndexEntry)],
    group: Option<u32>,
) -> Option<&'a (PathBuf, dbpf::IndexEntry)> {
    candidates
        .iter()
        .filter(|(_, entry)| {
            entry.id.type_id == TYPE_PROPERTY && group.is_none_or(|g| entry.id.group == g)
        })
        .max_by_key(|(path, _)| candidate_score(path))
}

fn read_value_dto(v: &Value) -> PropertyReadValue {
    let mut dto = PropertyReadValue {
        kind: match v {
            Value::Bool(_) => "bool",
            Value::Int32(_) => "int32",
            Value::UInt32(_) => "uint32",
            Value::Float(_) => "float",
            Value::Text(_) => "text",
            Value::Key(_) => "key",
            _ => "other",
        }
        .to_string(),
        int: None,
        float: None,
        text: None,
        key: None,
    };
    match v {
        Value::Bool(b) => dto.int = Some(i64::from(*b)),
        Value::Int32(i) => dto.int = Some(i64::from(*i)),
        Value::UInt32(u) => dto.int = Some(i64::from(*u)),
        Value::Float(f) => dto.float = Some(f64::from(*f)),
        Value::Text(t) => dto.text = Some([t.table_id, t.instance_id]),
        Value::Key(k) => dto.key = Some([k.type_id, k.group, k.instance]),
        _ => {}
    }
    dto
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyReadValue {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub int: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub float: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<[u32; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<[u32; 3]>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyReadEntry {
    pub hash: u32,
    pub array: bool,
    pub values: Vec<PropertyReadValue>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyReadRequest {
    pub instance: u32,
    pub group: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyReadResult {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
    pub keys: Vec<PropertyReadEntry>,
}

/// 读取一条 property 的类型化视图（Sheet 深层编辑先读后改的数据源）。
#[tauri::command]
pub async fn game_property_read(
    state: State<'_, crate::activity::AppState>,
    request: PropertyReadRequest,
) -> Result<PropertyReadResult, CommandError> {
    let game_data = state
        .store
        .app_settings()
        .map_err(CommandError::from)?
        .and_then(|s| s.game_data_path)
        .ok_or_else(|| {
            CommandError::new(
                "game_directory_not_configured",
                "Game data directory is not configured",
            )
        })?;
    let game_data = PathBuf::from(game_data);
    let instance = request.instance;
    let group = request.group;
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = cached_index(&game_data);
        let Some(index) = guard.as_ref() else {
            return Err(CommandError::internal("game asset index unavailable"));
        };
        let Some(candidates) = index.entries.get(&instance) else {
            return Err(CommandError::new(
                "game_property_missing",
                format!("source property 0x{instance:08X} not found"),
            ));
        };
        let Some((path, entry)) = pick_property_candidate(candidates, group) else {
            return Err(CommandError::new(
                "game_property_missing",
                format!("no readable property for 0x{instance:08X}"),
            ));
        };
        let package = dbpf::Package::open(path)
            .map_err(|e| CommandError::new("game_property_missing", format!("open failed: {e}")))?;
        let data = package
            .read(entry)
            .map_err(|e| CommandError::new("game_property_missing", format!("read failed: {e}")))?;
        let file = PropertyFile::parse(&data)
            .map_err(|e| CommandError::new("game_property_parse_failed", e.to_string()))?;
        let keys = file
            .values
            .iter()
            .map(|prop| {
                let values: Vec<PropertyReadValue> = match &prop.kind {
                    Kind::Scalar(v) => vec![read_value_dto(v)],
                    Kind::Array(vs) => vs.iter().map(read_value_dto).collect(),
                    Kind::Empty => vec![],
                };
                PropertyReadEntry {
                    hash: prop.hash,
                    array: matches!(prop.kind, Kind::Array(_)),
                    values,
                }
            })
            .collect();
        Ok(PropertyReadResult {
            type_id: entry.id.type_id,
            group: entry.id.group,
            instance: entry.id.instance,
            keys,
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchFloat {
    pub hash: u32,
    pub value: f64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchInt {
    pub hash: u32,
    pub value: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchTextRef {
    pub table: u32,
    pub instance: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchTextRefs {
    pub hash: u32,
    pub refs: Vec<PropertyPatchTextRef>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchRequest {
    pub instance: u32,
    pub group: Option<u32>,
    #[serde(default)]
    pub set_floats: Vec<PropertyPatchFloat>,
    #[serde(default)]
    pub set_ints: Vec<PropertyPatchInt>,
    #[serde(default)]
    pub set_text_refs: Vec<PropertyPatchTextRefs>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchResult {
    pub data_base64: String,
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

/// 类型化键补丁：读源 property → 按键改值 → encode_canonical 规范编码，
/// 回显完整 TGI（覆盖包必须按原 TGI 放置才能命中同一条目）。
#[tauri::command]
pub async fn game_property_patch(
    state: State<'_, crate::activity::AppState>,
    request: PropertyPatchRequest,
) -> Result<PropertyPatchResult, CommandError> {
    use base64::Engine;
    let game_data = state
        .store
        .app_settings()
        .map_err(CommandError::from)?
        .and_then(|s| s.game_data_path)
        .ok_or_else(|| {
            CommandError::new(
                "game_directory_not_configured",
                "Game data directory is not configured",
            )
        })?;
    let game_data = PathBuf::from(game_data);
    let instance = request.instance;
    let group = request.group;
    let set_floats = request.set_floats.clone();
    let set_ints = request.set_ints.clone();
    let set_text_refs = request.set_text_refs.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = cached_index(&game_data);
        let Some(index) = guard.as_ref() else {
            return Err(CommandError::internal("game asset index unavailable"));
        };
        let Some(candidates) = index.entries.get(&instance) else {
            return Err(CommandError::new(
                "game_property_missing",
                format!("source property 0x{instance:08X} not found"),
            ));
        };
        let Some((path, entry)) = pick_property_candidate(candidates, group) else {
            return Err(CommandError::new(
                "game_property_missing",
                format!("no readable property for 0x{instance:08X}"),
            ));
        };
        let type_id = entry.id.type_id;
        let group = entry.id.group;
        let package = dbpf::Package::open(path)
            .map_err(|e| CommandError::new("game_property_missing", format!("open failed: {e}")))?;
        let data = package
            .read(entry)
            .map_err(|e| CommandError::new("game_property_missing", format!("read failed: {e}")))?;
        let mut file = PropertyFile::parse(&data)
            .map_err(|e| CommandError::new("game_property_parse_failed", e.to_string()))?;

        for patch in &set_floats {
            set_scalar_value(
                &mut file,
                patch.hash,
                PropType::Float,
                Value::Float(patch.value as f32),
            );
        }
        for patch in &set_ints {
            set_scalar_value(
                &mut file,
                patch.hash,
                PropType::Int32,
                Value::Int32(patch.value.clamp(i32::MIN as i64, i32::MAX as i64) as i32),
            );
        }
        for patch in &set_text_refs {
            let values = patch
                .refs
                .iter()
                .map(|r| {
                    Value::Text(sc_properties::Text {
                        table_id: r.table,
                        instance_id: r.instance,
                    })
                })
                .collect();
            set_array_values(&mut file, patch.hash, PropType::Text, values);
        }

        let encoded = file
            .encode_canonical()
            .map_err(|e| CommandError::new("game_property_encode_failed", e.to_string()))?;
        Ok(PropertyPatchResult {
            data_base64: base64::engine::general_purpose::STANDARD.encode(&encoded),
            type_id,
            group,
            instance,
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{PropertyEncoding, Text};

    #[test]
    fn overriding_text_array_keeps_game_stride() {
        let mut file = PropertyFile {
            values: vec![Property {
                hash: 0x0A09_F5FA,
                prop_type: PropType::Text,
                kind: Kind::Array(vec![Value::Text(Text {
                    table_id: 0,
                    instance_id: 0,
                })]),
                encoding: PropertyEncoding {
                    flags: 0x30,
                    array_item_size: Some(520),
                },
            }],
            claimed_count: 1,
        };

        set_scalar_text(&mut file, 0x0A09_F5FA, 0x6C96_9DEE, 0x1234_5678);
        let encoded = file.encode_canonical().expect("canonical text array");
        let reparsed = PropertyFile::parse(&encoded).expect("reparse canonical text array");
        let property = reparsed.get(0x0A09_F5FA).expect("title property");
        assert_eq!(
            property.encoding.array_item_size,
            Some(sc_properties::TEXT_ARRAY_ITEM_SIZE)
        );
        assert!(matches!(property.kind, Kind::Array(ref values) if values.len() == 1));
    }

    #[test]
    fn untouched_text_array_keeps_game_stride_before_copy_encoding() {
        let mut file = PropertyFile {
            values: vec![Property {
                hash: 0x0E94_855A,
                prop_type: PropType::Text,
                kind: Kind::Array(vec![Value::Text(Text {
                    table_id: 0x50AA_0BEA,
                    instance_id: 1,
                })]),
                encoding: PropertyEncoding {
                    flags: 0x30,
                    // 之前错误地规范化成 8 的 stride 也要自愈回游戏惯例值。
                    array_item_size: Some(8),
                },
            }],
            claimed_count: 1,
        };

        normalize_array_item_sizes(&mut file);
        assert!(file.encode_canonical().is_ok());
        assert_eq!(
            file.values[0].encoding.array_item_size,
            Some(sc_properties::TEXT_ARRAY_ITEM_SIZE)
        );
    }
}
