//! `export-prop --json` 协议层 —— 与 C# `CliRunner.DumpProp` 逐字段对齐。
//!
//! C# 侧权威语义（`CliRunner.cs`，已逐条核对源码）：
//! - 根对象 `{ name, propertyCount, properties }`，`properties` 按 hash 升序；
//! - 每项 `{ name, hash, type, value }`：
//!   - `hash` 为 `"0x%08x"` 小写十六进制；
//!   - `type` 为 C# 类名去掉 `Property` 后缀（数组整体为 `Array`）；
//!   - `value` 为 `DisplayValue` 字符串（invariant culture）；
//!   - `name` 为描述符库名称（comments 优先，其次 name；未知为 `null`）。
//! - flags 区分出的「空变体」条目在 C# 读取时不进入 `Values`
//!   （`PropertyFile.Read` 的 else 分支为空），因此本层同样排除它们；
//! - `bool.ToString()` 输出 `True`/`False`；`Key`/`Text` 的 `DisplayValue`
//!   在本 fork 中返回空字符串；`uint32` 以 `0x%08x` 小写输出；
//! - 浮点走 .NET Framework `float.ToString()`（G7 语义），不能直接用
//!   Rust 最短往返格式，见 [`net48_float_to_string`]。

use dbpf::{IndexEntry, Package, ResourceId};
use sc_properties::{Kind, PropType, Property, PropertyFile, Value};
use sc_registry::Registry;
use serde::Serialize;

/// Property-list 资源类型（`CliRunner.PROP_TYPE_ID`）。
pub const PROP_TYPE_ID: u32 = 0x00B1_B104;

/// One dumped property: `{ name, hash, type, value }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DumpProperty {
    /// Descriptor name, `null` when unknown (C# `PropName`).
    pub name: Option<String>,
    /// `"0x%08x"` lowercase, matching C#.
    pub hash: String,
    /// C# class name minus the `Property` suffix (`Array` for arrays).
    #[serde(rename = "type")]
    pub type_name: &'static str,
    /// `DisplayValue` string, invariant culture.
    pub value: String,
}

/// Dump root: `{ name, propertyCount, properties }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DumpFile {
    pub name: String,
    #[serde(rename = "propertyCount")]
    pub property_count: u32,
    pub properties: Vec<DumpProperty>,
}

/// The C# fallback resource name: `"{typeid:x8}-{group:x8}-{instance:x8}"`.
pub fn fallback_name(id: ResourceId) -> String {
    format!("{:08x}-{:08x}-{:08x}", id.type_id, id.group, id.instance)
}

/// C# `PropTypeName`: class name minus `Property` suffix.
pub fn csharp_type_name(t: PropType) -> &'static str {
    match t {
        PropType::Bool => "Bool",
        PropType::Int32 => "Int32",
        PropType::UInt32 => "UInt32",
        PropType::Float => "Float",
        PropType::String8 => "String8",
        PropType::String16 => "String16",
        PropType::Key => "Key",
        PropType::Text => "Text",
        PropType::Vector2 => "Vector2",
        PropType::Vector3 => "Vector3",
        PropType::ColorRgb => "ColorRGB",
        PropType::Vector4 => "Vector4",
        PropType::ColorRgba => "ColorRGBA",
        PropType::Transform => "Transform",
        PropType::BoundingBox => "BoundingBox",
    }
}

/// C# `PropName(hash)`: descriptor comments first, then name, else `None`.
pub fn prop_name(hash: u32, registry: Option<&Registry>) -> Option<String> {
    let record = registry?.properties().get(&hash)?;
    if !record.comments.is_empty() {
        Some(record.comments.clone())
    } else if !record.name.is_empty() {
        Some(record.name.clone())
    } else {
        None
    }
}

/// .NET Framework `float.ToString()` (G7 semantics) —— 与 Rust 最短往返格式
/// 不同：超过 7 位有效数字的值按 7 位舍入，且按 G 规则切换定点/科学计数。
pub fn net48_float_to_string(v: f32) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if v == 0.0 {
        return "0".to_string();
    }

    let negative = v < 0.0;
    let magnitude = v.abs();

    // 7 significant digits in scientific form: "d.dddddde<exp>".
    let sci = format!("{magnitude:.6e}");
    let (mantissa, exponent) = sci.split_once('e').expect("scientific form");
    let exponent: i32 = exponent.parse().expect("exponent digits");
    let mut digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    while digits.len() > 1 && digits.ends_with('0') {
        digits.pop();
    }

    let body = if (-4..7).contains(&exponent) {
        // Fixed-point notation.
        if exponent >= 0 {
            let point = (exponent + 1) as usize;
            if digits.len() <= point {
                format!("{digits}{}", "0".repeat(point - digits.len()))
            } else {
                let fraction = digits[point..].trim_end_matches('0');
                if fraction.is_empty() {
                    digits[..point].to_string()
                } else {
                    format!("{}.{}", &digits[..point], fraction)
                }
            }
        } else {
            format!("0.{}{}", "0".repeat((-exponent - 1) as usize), digits)
        }
    } else {
        // Scientific notation: "D[.DDD]E±XX".
        let head = digits.remove(0);
        if digits.is_empty() {
            format!("{head}E{}{:02}", sign(exponent), exponent.abs())
        } else {
            format!("{head}.{}E{}{:02}", digits, sign(exponent), exponent.abs())
        }
    };

    if negative { format!("-{body}") } else { body }
}

fn sign(exponent: i32) -> char {
    if exponent < 0 { '-' } else { '+' }
}

fn float(value: f32) -> String {
    net48_float_to_string(value)
}

/// Single scalar `DisplayValue`（不含数组包装）。
pub fn display_value(v: &Value) -> String {
    match v {
        // C# `bool.ToString()` capitalizes.
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Int32(n) => n.to_string(),
        Value::UInt32(n) => format!("0x{n:08x}"),
        Value::Float(f) => float(*f),
        // This fork returns string.Empty for Key and Text displays.
        Value::Key(_) => String::new(),
        Value::Text(_) => String::new(),
        Value::String8(s) | Value::String16(s) => s.clone(),
        Value::ColorRgb { r, g, b } => format!("R {}-G {}-B {}", float(*r), float(*g), float(*b)),
        Value::ColorRgba { r, g, b, a } => {
            format!(
                "R {}-G {}-B {}-A {}",
                float(*r),
                float(*g),
                float(*b),
                float(*a)
            )
        }
        Value::Vector2([x, y]) => format!("Vector2 (X = {}, Y = {})", float(*x), float(*y)),
        Value::Vector3(v) => {
            format!(
                "Vector3 (X = {}, Y = {}, Z = {})",
                float(v[0]),
                float(v[1]),
                float(v[2])
            )
        }
        Value::Vector4(v) => format!(
            "Vector4 (X = {}, Y = {}, Z = {}, W = {})",
            float(v[0]),
            float(v[1]),
            float(v[2]),
            float(v[3])
        ),
        Value::BoundingBox { min, max } => format!(
            "Bounding Box (MinX = {}, MinY = {}, MinZ = {}, MaxX = {}, MaxY = {}, MaxZ = {})",
            float(min[0]),
            float(min[1]),
            float(min[2]),
            float(max[0]),
            float(max[1]),
            float(max[2])
        ),
        Value::Transform(t) => {
            let matrix = t
                .matrix
                .iter()
                .map(|m| float(*m))
                .collect::<Vec<_>>()
                .join(",");
            let unknown = t.unknown.map(float).unwrap_or_else(|| "0".to_string());
            format!(
                "Transform (Count = {}, {}, {}, {})",
                t.matrix.len(),
                matrix,
                t.flags,
                unknown
            )
        }
    }
}

/// Whole-entry `DisplayValue`：数组为带前导空格的逐项拼接（C# `ArrayProperty`）。
pub fn property_value(p: &Property) -> String {
    match &p.kind {
        Kind::Scalar(v) => display_value(v),
        Kind::Array(values) => values
            .iter()
            .map(|v| format!(" {}", display_value(v)))
            .collect(),
        // Empty variants never reach the dump (excluded before this call).
        Kind::Empty => String::new(),
    }
}

/// Dump one parsed property file exactly like `DumpProp(.., json: true)`.
pub fn dump_property_file(pf: &PropertyFile, name: &str, registry: Option<&Registry>) -> DumpFile {
    let mut entries: Vec<&Property> = pf
        .values
        .iter()
        // C# never stores empty variants, so they are absent from its dumps.
        .filter(|p| !matches!(p.kind, Kind::Empty))
        .collect();
    entries.sort_by_key(|p| p.hash);

    DumpFile {
        name: name.to_string(),
        property_count: entries.len() as u32,
        properties: entries
            .into_iter()
            .map(|p| DumpProperty {
                name: prop_name(p.hash, registry),
                hash: format!("0x{:08x}", p.hash),
                type_name: match &p.kind {
                    Kind::Array(_) => "Array",
                    _ => csharp_type_name(p.prop_type),
                },
                value: property_value(p),
            })
            .collect(),
    }
}

/// Serialize a dump like `JsonConvert.SerializeObject(root, Formatting.Indented)`.
/// Only structural equality matters for differential runs; whitespace may differ.
pub fn to_json(dump: &DumpFile) -> crate::Result<String> {
    Ok(serde_json::to_string_pretty(dump)?)
}

/// Read + parse + dump one `0x00B1B104` package resource.
pub fn dump_resource(
    package: &Package,
    entry: &IndexEntry,
    registry: Option<&Registry>,
) -> crate::Result<DumpFile> {
    debug_assert_eq!(entry.id.type_id, PROP_TYPE_ID, "not a property resource");
    let data = package.read(entry)?;
    let pf = PropertyFile::parse(&data)?;
    Ok(dump_property_file(&pf, &fallback_name(entry.id), registry))
}
