//! Locale 字符串表与本地化资产名称（对应 C# `LocaleRegistry` + `BuildLocaleNameMap`）。
//!
//! Locale 包中类型 `0x0A98EAF0` 的资源是 UTF-8 JSON 字符串表：内容以 3 字节
//! BOM/前缀开始，形如 `{ "0x00000001": "译文", "//": "注释", ... }`，`//` 键
//! 为注释。表 id 取资源自身的 InstanceId。

use std::collections::HashMap;

use dbpf::ResourceId;

use crate::PropertyFile;
use crate::model::Value;

/// Locale JSON string-table resource type id.
pub const LOCALE_RESOURCE_TYPE: u32 = 0x0A98_EAF0;

/// OpenSCP 自有 locale 表 id（FNV-1("openscp/strings")，与原版表无冲突）。
/// 自定义词条一律写进这张新表、Text 引用指向它；重定义原版表 ID
/// （6C969DEE/50AA0BEA/4B54417A）只在 Locale 目录的包里生效，其他目录
/// 整表不生效（实机 2026-10-19 验证；先例 SimRollerCompletePack 同款做法）。
pub const OPENSCP_LOCALE_TABLE: u32 = 0xBF82_F7CE;

/// Name-candidate property hashes (C# `BuildLocaleNameMap`).
pub const NAME_PROPERTY_HASHES: [u32; 5] = [
    0x09FB_78CB,
    0x0A09_F5FA,
    0x09B7_11C3,
    0x0E28_B5BC,
    0x0E28_B5D5,
];

/// RW4 model type id: names propagate from catalog props to referenced models.
pub const MODEL_RESOURCE_TYPE: u32 = 0x2F4E_681B;

/// Parsed locale string tables: `table_id -> (string_id -> translation)`.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Locale {
    tables: HashMap<u32, HashMap<u32, String>>,
}

impl Locale {
    /// Build from `(instance_id, raw_resource_bytes)` pairs of all
    /// `0x0A98EAF0` resources in the locale package.
    pub fn from_resources<I: IntoIterator<Item = (u32, Vec<u8>)>>(
        resources: I,
    ) -> Result<Locale, String> {
        let mut tables = HashMap::new();
        for (instance, data) in resources {
            let table = parse_string_table(&data)
                .map_err(|e| format!("locale resource {instance:08X}: {e}"))?;
            tables.insert(instance, table);
        }
        Ok(Locale { tables })
    }

    /// `LocaleRegistry.GetLocalizedString` equivalent.
    pub fn get(&self, table: u32, id: u32) -> Option<&str> {
        self.tables.get(&table)?.get(&id).map(String::as_str)
    }

    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn string_count(&self) -> usize {
        self.tables.values().map(HashMap::len).sum()
    }
}

/// Parse one locale JSON string table (3-byte prefix + JSON object).
pub fn parse_string_table(data: &[u8]) -> Result<HashMap<u32, String>, String> {
    Ok(parse_locale_items(data)?
        .into_iter()
        .filter_map(|item| item.id.map(|id| (id, item.text)))
        .collect())
}

/// One editable line of a locale string table. `id == None` 表示注释行
/// （原键 `"//"`），序列化时原样保留，编辑界面只读展示。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocaleItem {
    pub key: String,
    pub id: Option<u32>,
    pub text: String,
}

/// UTF-8 BOM：SimCity locale JSON 资源的 3 字节前缀。
const LOCALE_PREFIX: [u8; 3] = [0xEF, 0xBB, 0xBF];

/// 按原文件顺序解析全部条目（含注释行），供编辑器做无损往返。
pub fn parse_locale_items(data: &[u8]) -> Result<Vec<LocaleItem>, String> {
    if data.len() < 3 {
        return Err("resource shorter than 3-byte prefix".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&data[3..]).map_err(|e| e.to_string())?;
    let object = json
        .as_object()
        .ok_or("locale resource is not a JSON object")?;
    let mut items = Vec::with_capacity(object.len());
    for (key, value) in object {
        let text = value
            .as_str()
            .ok_or_else(|| format!("non-string value for {key:?}"))?
            .to_string();
        let id = if key == "//" {
            None
        } else {
            Some(
                u32::from_str_radix(key.trim_start_matches("0x"), 16)
                    .map_err(|e| format!("bad locale key {key:?}: {e}"))?,
            )
        };
        items.push(LocaleItem {
            key: key.clone(),
            id,
            text,
        });
    }
    Ok(items)
}

/// 序列化回 locale JSON 资源。排版模仿原版宽松格式（BOM + 换行缩进 +
/// `"0x……" : "……"`，小写十六进制键）：游戏对单行紧凑 JSON 解析失败并整表
/// 丢弃（实机 2026-10-19 验证）。注释行保留原键，供编辑器无损往返。
pub fn serialize_locale_items(items: &[LocaleItem]) -> Result<Vec<u8>, String> {
    let mut seen = std::collections::HashSet::new();
    let mut lines = Vec::with_capacity(items.len());
    for item in items {
        let key_json = match item.id {
            Some(id) => {
                // 数据键必须唯一（注释行 `//` 可重复，原版表里本就多条并存）。
                if !seen.insert(id) {
                    return Err(format!("duplicate locale key 0x{id:08x} while serializing"));
                }
                serde_json::to_string(&format!("0x{id:08x}")).map_err(|e| e.to_string())?
            }
            None if item.key == "//" => {
                serde_json::to_string(&item.key).map_err(|e| e.to_string())?
            }
            None => return Err(format!("comment row lost its key: {:?}", item.key)),
        };
        lines.push(format!(
            "    {}: {}",
            key_json,
            serde_json::to_string(&item.text).map_err(|e| e.to_string())?
        ));
    }
    let body = if lines.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\r\n{}\r\n}}", lines.join(",\r\n"))
    };
    let mut out = LOCALE_PREFIX.to_vec();
    out.extend_from_slice(body.as_bytes());
    Ok(out)
}

/// Build instance → localized name map from prop entries (C#
/// `BuildLocaleNameMap`): a name property's `Array[0]` Text resolves through
/// the locale; the name then maps to the prop's own instance **and** every
/// referenced RW4 model instance.
pub fn collect_name_map<'a, I>(entries: I, locale: &Locale) -> HashMap<u32, String>
where
    I: IntoIterator<Item = (ResourceId, &'a PropertyFile)>,
{
    let mut names: HashMap<u32, String> = HashMap::new();

    for (id, file) in entries {
        for prop in &file.values {
            if !NAME_PROPERTY_HASHES.contains(&prop.hash) {
                continue;
            }
            let Some(text) = name_text(prop) else {
                continue;
            };
            let Some(name) = locale.get(text.table_id, text.instance_id) else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            names.insert(id.instance, name.to_string());
            for key in prop.keys() {
                if key.type_id == MODEL_RESOURCE_TYPE {
                    names.insert(key.instance, name.to_string());
                }
            }
        }
    }
    names
}

/// `ArrayProperty[0] TextProperty` extraction from a name property.
fn name_text(prop: &crate::model::Property) -> Option<crate::model::Text> {
    match &prop.kind {
        crate::model::Kind::Array(vals) => match vals.first() {
            Some(Value::Text(t)) => Some(*t),
            _ => None,
        },
        crate::model::Kind::Scalar(Value::Text(t)) => Some(*t),
        _ => None,
    }
}
