//! 探针：找含 Prop 槽列（0x0C12EF40..）的 lot 属性文件，dump 全部 hash +
//! 值类型摘要，定位 slot → prop 原型 → RW4 模型的引用链。仅开发用。
use sc_properties::{Kind, PropertyFile, Value};

const PROP_SLOT_BASE: u32 = 0x0C12_EF40;
const PROP_TRANSFORM_BASE: u32 = 0x0C12_EF30;

fn main() {
    let path = std::env::args().nth(1).expect("usage: prop_chain_probe <package> [instance]");
    let only = std::env::args().nth(2).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let package = dbpf::Package::open(&path).expect("open package");
    let mut found = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 {
            continue;
        }
        if let Some(inst) = only {
            if entry.id.instance != inst {
                continue;
            }
        }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&data) else { continue };
        let has_props = (0..14).any(|b| {
            file.get(PROP_SLOT_BASE + b).is_some() || file.get(PROP_TRANSFORM_BASE + b).is_some()
        });
        if !has_props && only.is_none() {
            continue;
        }
        found += 1;
        if found > 3 {
            continue;
        }
        println!(
            "\n==== property 0x{:08X} group=0x{:08X}（{} 属性，含 prop 列）====",
            entry.id.instance,
            entry.id.group,
            file.values.len()
        );
        let mut props: Vec<_> = file.values.iter().collect();
        props.sort_by_key(|p| p.hash);
        for prop in props {
            let summary = match &prop.kind {
                Kind::Scalar(v) => scalar_summary(v),
                Kind::Array(vals) => {
                    let head: Vec<String> = vals.iter().take(4).map(scalar_summary).collect();
                    format!("array[{}] {}", vals.len(), head.join(", "))
                }
                Kind::Empty => "empty".to_string(),
            };
            println!("  0x{:08X} {}: {summary}", prop.hash, prop.prop_type.name());
        }
    }
    println!("\n共 {found} 个含 prop 列的属性文件");
}

fn scalar_summary(v: &Value) -> String {
    match v {
        Value::Bool(b) => format!("bool({b})"),
        Value::Int32(i) => format!("i32({i})"),
        Value::UInt32(u) => format!("u32(0x{u:08X})"),
        Value::Float(f) => format!("f({f:.3})"),
        Value::Text(t) => format!("text({},0x{:08X})", t.table_id, t.instance_id),
        Value::Key(k) => format!("key(t=0x{:08X},g=0x{:08X},i=0x{:08X})", k.type_id, k.group, k.instance),
        other => format!("{other:?}"),
    }
}
