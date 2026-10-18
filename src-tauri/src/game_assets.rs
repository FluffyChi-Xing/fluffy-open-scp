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
    pub new_instance: u32,
    pub ui_categories: Vec<u32>,
    pub title_string_id: Option<u32>,
    pub desc_string_id: Option<u32>,
    pub unlock_string_id: Option<u32>,
    pub position: Option<i32>,
    pub icon_tgi: Option<TgiRef>,
    pub marquee_tgi: Option<TgiRef>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyCopyResult {
    pub data_base64: String,
    pub key_count: usize,
}

fn set_scalar_key(file: &mut PropertyFile, hash: u32, key: Value) {
    set_scalar_value(file, hash, PropType::Key, key);
}

fn fixed_array_item_size(prop_type: PropType) -> Option<i32> {
    match prop_type {
        PropType::Bool => Some(1),
        PropType::Int32 | PropType::UInt32 | PropType::Float => Some(4),
        PropType::Key => Some(12),
        PropType::Text => Some(8),
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
                prop.encoding.array_item_size = Some(8);
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
            // Source packages occasionally contain non-canonical array strides
            // (for example Text with item_size=520). The values we emit use the
            // canonical fixed stride required by encode_canonical.
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
        let mut copied: Option<PropertyFile> = None;
        for (path, entry) in &candidates {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let Ok(package) = dbpf::Package::open(path) else { continue };
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            copied = Some(file);
            break;
        }
        let Some(mut file) = copied else {
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
            set_scalar_text(&mut file, 0x0A09_F5FA, 0x6C96_9DEE, id);
        }
        if let Some(id) = request.desc_string_id {
            set_scalar_text(&mut file, 0x0A09_F5FB, 0x50AA_0BEA, id);
        }
        if let Some(id) = request.unlock_string_id {
            set_scalar_text(&mut file, 0x0DE8_4DDC, 0x4B54_417A, id);
        } else {
            // A copied source can carry an inherited unlock pointer even when
            // the edited menu entry has no unlock text. Keeping that stale
            // pointer produces a visible `***` in game when its source locale
            // item is unavailable.
            remove_property(&mut file, 0x0DE8_4DDC);
            remove_property(&mut file, 0x0DE8_4DD3);
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
        // Existing game files contain legacy text arrays whose metadata says
        // item_size=520 even though each parsed Text value is 8 bytes. The
        // copied file must be normalized as a whole, including untouched
        // statistics/tooltip arrays, before canonical encoding.
        normalize_array_item_sizes(&mut file);
        let key_count = file.values.len();
        let bytes = file
            .encode_canonical()
            .map_err(|e| CommandError::new("game_property_encode_failed", e.to_string()))?;
        Ok(PropertyCopyResult {
            data_base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
            key_count,
        })
    })
    .await
    .map_err(|e| crate::activity::CommandError::internal(e.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{PropertyEncoding, Text};

    #[test]
    fn overriding_noncanonical_text_array_normalizes_item_size() {
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
        assert_eq!(property.encoding.array_item_size, Some(8));
        assert!(matches!(property.kind, Kind::Array(ref values) if values.len() == 1));
    }

    #[test]
    fn untouched_noncanonical_text_array_is_normalized_before_copy_encoding() {
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
                    array_item_size: Some(520),
                },
            }],
            claimed_count: 1,
        };

        normalize_array_item_sizes(&mut file);
        assert!(file.encode_canonical().is_ok());
        assert_eq!(file.values[0].encoding.array_item_size, Some(8));
    }
}
