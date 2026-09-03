use std::fmt;

use serde::Serialize;

/// Property element type, mirroring the C# `PropertyDefinitionAttribute`
/// table (`PackageReader/Properties/Types/*.cs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum PropType {
    Bool,
    Int32,
    UInt32,
    Float,
    String8,
    String16,
    Key,
    Text,
    Vector2,
    Vector3,
    ColorRgb,
    Vector4,
    ColorRgba,
    Transform,
    BoundingBox,
}

impl PropType {
    pub fn from_file_type(file_type: u16) -> Option<PropType> {
        let t = match file_type {
            1 => PropType::Bool,
            9 => PropType::Int32,
            10 => PropType::UInt32,
            13 => PropType::Float,
            18 => PropType::String8,
            19 => PropType::String16,
            32 => PropType::Key,
            34 => PropType::Text,
            48 => PropType::Vector2,
            49 => PropType::Vector3,
            50 => PropType::ColorRgb,
            51 => PropType::Vector4,
            52 => PropType::ColorRgba,
            56 => PropType::Transform,
            57 => PropType::BoundingBox,
            _ => return None,
        };
        Some(t)
    }

    /// The C# property name (`PropertyDefinitionAttribute.Name`), used in
    /// dumps and the frontend.
    pub fn name(&self) -> &'static str {
        match self {
            PropType::Bool => "bool",
            PropType::Int32 => "int32",
            PropType::UInt32 => "uint32",
            PropType::Float => "float",
            PropType::String8 => "string8",
            PropType::String16 => "string16",
            PropType::Key => "Key",
            PropType::Text => "text",
            PropType::Vector2 => "vector2",
            PropType::Vector3 => "vector3",
            PropType::ColorRgb => "colorRGB",
            PropType::Vector4 => "vector4",
            PropType::ColorRgba => "colorRGBA",
            PropType::Transform => "transform",
            PropType::BoundingBox => "bbox",
        }
    }
}

/// A TGI reference value (Key property).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Key {
    pub instance: u32,
    pub type_id: u32,
    pub group: u32,
}

/// A locale text reference (Text property): pointer into the locale string
/// tables, resolved to a localized string via the locale package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Text {
    pub table_id: u32,
    pub instance_id: u32,
}

/// Transform: flags-derived matrix count, optional leading unknown float.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Transform {
    pub flags: u16,
    /// Present only when flags == 15.
    pub unknown: Option<f32>,
    /// 12 (default), 3 (flags == 0x0C) or 4 (flags == 0x0D) floats.
    pub matrix: Vec<f32>,
}

/// A property value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Value {
    Bool(bool),
    Int32(i32),
    UInt32(u32),
    Float(f32),
    Key(Key),
    Text(Text),
    String8(String),
    String16(String),
    ColorRgb { r: f32, g: f32, b: f32 },
    ColorRgba { r: f32, g: f32, b: f32, a: f32 },
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    Vector4([f32; 4]),
    BoundingBox { min: [f32; 3], max: [f32; 3] },
    Transform(Transform),
}

impl fmt::Display for Value {
    /// Human-readable single-line rendering (invariant float formatting),
    /// matching the intent of the C# `DisplayValue`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Bool(v) => write!(f, "{v}"),
            Value::Int32(v) => write!(f, "{v}"),
            Value::UInt32(v) => write!(f, "{v}"),
            Value::Float(v) => write!(f, "{v}"),
            Value::Key(k) => write!(
                f,
                "T {:08X} - G {:08X} - I {:08X}",
                k.type_id, k.group, k.instance
            ),
            Value::Text(t) => write!(f, "#{:08X}:{:08X}", t.table_id, t.instance_id),
            Value::String8(s) | Value::String16(s) => write!(f, "{s}"),
            Value::ColorRgb { r, g, b } => write!(f, "({r}, {g}, {b})"),
            Value::ColorRgba { r, g, b, a } => write!(f, "({r}, {g}, {b}, {a})"),
            Value::Vector2([x, y]) => write!(f, "({x}, {y})"),
            Value::Vector3(v) => write!(f, "({}, {}, {})", v[0], v[1], v[2]),
            Value::Vector4(v) => write!(f, "({}, {}, {}, {})", v[0], v[1], v[2], v[3]),
            Value::BoundingBox { min, max } => write!(
                f,
                "({}, {}, {}) - ({}, {}, {})",
                min[0], min[1], min[2], max[0], max[1], max[2]
            ),
            Value::Transform(t) => write!(
                f,
                "Transform (Count = {}, {:?}, flags {:#06x}, {:?})",
                t.matrix.len(),
                t.matrix,
                t.flags,
                t.unknown
            ),
        }
    }
}

/// One entry in a property file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Property {
    pub hash: u32,
    /// Element type (the file's 16-bit type id resolved through the table).
    pub prop_type: PropType,
    pub kind: Kind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Kind {
    Scalar(Value),
    Array(Vec<Value>),
    /// Variant marker with no payload (flags & 0x30 != 0 && flags & 0x40 != 0).
    Empty,
}

impl Property {
    /// The value for scalar entries; `None` for arrays and empty markers.
    pub fn scalar(&self) -> Option<&Value> {
        match &self.kind {
            Kind::Scalar(v) => Some(v),
            _ => None,
        }
    }

    /// The values for array entries; `None` otherwise.
    pub fn array(&self) -> Option<&[Value]> {
        match &self.kind {
            Kind::Array(values) => Some(values),
            _ => None,
        }
    }

    /// All Key (TGI reference) values in this property, scalar or array —
    /// used for cross-resource reference walking (asset aggregation).
    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        let (single, array): (Option<&Value>, Option<&[Value]>) = match &self.kind {
            Kind::Scalar(v) => (Some(v), None),
            Kind::Array(values) => (None, Some(values)),
            Kind::Empty => (None, None),
        };
        single
            .into_iter()
            .chain(array.into_iter().flatten())
            .filter_map(|v| match v {
                Value::Key(k) => Some(k),
                _ => None,
            })
    }
}
