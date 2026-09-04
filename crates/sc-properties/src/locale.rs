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
    if data.len() < 3 {
        return Err("resource shorter than 3-byte prefix".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&data[3..]).map_err(|e| e.to_string())?;
    let object = json
        .as_object()
        .ok_or("locale resource is not a JSON object")?;

    let mut table = HashMap::new();
    for (key, value) in object {
        if key == "//" {
            continue; // comment entry
        }
        let id = u32::from_str_radix(key.trim_start_matches("0x"), 16)
            .map_err(|e| format!("bad locale key {key:?}: {e}"))?;
        let translation = value
            .as_str()
            .ok_or_else(|| format!("non-string value for {key:?}"))?;
        table.insert(id, translation.to_string());
    }
    Ok(table)
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
