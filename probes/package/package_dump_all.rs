//! 探针：列出包内全部条目（TGI+尺寸），property 条目附完整内容转储。仅开发用。
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let mut args = std::env::args().skip(1);
    let _label = args.next().expect("usage: package_dump_all <label> <package...>");
    for path in args {
        println!("===== {path} =====");
        let Ok(package) = dbpf::Package::open(&path) else { println!("open fail"); continue };
        let mut count = 0;
        for entry in package.entries() {
            count += 1;
            println!("--- [{count}] type=0x{:08X} group=0x{:08X} instance=0x{:08X} stored={}B",
                entry.id.type_id, entry.id.group, entry.id.instance, entry.compressed_size);
            if entry.id.type_id == 0x00B1_B104 {
                if let Ok(data) = package.read(entry) {
                    if let Ok(file) = PropertyFile::parse(&data) {
                        for prop in &file.values {
                            println!("    0x{:08X} {} = {}", prop.hash,
                                kind_name(&prop.kind), render_values(&prop.kind));
                        }
                    }
                }
            }
        }
    }
}

fn kind_name(kind: &sc_properties::Kind) -> &'static str {
    match kind {
        sc_properties::Kind::Scalar(_) => "scalar",
        sc_properties::Kind::Array(_) => "array",
        sc_properties::Kind::Empty => "empty",
    }
}

fn render_values(kind: &sc_properties::Kind) -> String {
    let values: Vec<&sc_properties::Value> = match kind {
        Kind::Scalar(v) => vec![v],
        Kind::Array(values) => values.iter().collect(),
        Kind::Empty => vec![],
    };
    values.iter().map(|v| match v {
        Value::Bool(b) => format!("bool:{b}"),
        Value::Int32(n) => format!("{n}"),
        Value::UInt32(n) => format!("u:{n}"),
        Value::Float(f) => format!("f:{f}"),
        Value::Key(k) => format!("key(T{:08X},G{:08X},I{:08X})", k.type_id, k.group, k.instance),
        Value::Text(t) => format!("txt(T{:08X},I{:08X})", t.table_id, t.instance_id),
        Value::String8(s) => format!("s8:{s}"),
        Value::String16(s) => format!("s16:{s}"),
        Value::ColorRgb { r, g, b } => format!("rgb({r},{g},{b})"),
        Value::ColorRgba { r, g, b, a } => format!("rgba({r},{g},{b},{a})"),
        Value::Vector2(v) => format!("v2({:?})", v),
        Value::Vector3(v) => format!("v3({:?})", v),
        Value::Vector4(v) => format!("v4({:?})", v),
        Value::BoundingBox { min, max } => format!("bbox({:?}-{:?})", min, max),
        Value::Transform(t) => format!("xform({:?})", t.matrix),
    }).collect::<Vec<_>>().join(" | ")
}
