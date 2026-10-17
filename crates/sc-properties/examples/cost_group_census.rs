//! 探针：三种造价/维护哈希在哪些 class（group）里、以及样例条目。
//! 用法：cargo run -p sc-properties --release --example cost_group_census -- <package...>
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const HASH_COST: u32 = 0x0C8E_E92F;
const HASH_MAINT: u32 = 0x09AE_19D7;
const HASH_CONSTRUCTION: u32 = 0x0A8A_41DA;

fn i32_of(file: &PropertyFile, hash: u32) -> Option<i32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(v) => i32_val(v),
        Kind::Array(values) => values.iter().find_map(i32_val),
        _ => None,
    }
}

/// 造价键多为 float 存储（如 0x0C8EE92F），int/uint/float 都接受。
fn i32_val(v: &Value) -> Option<i32> {
    match v {
        Value::Int32(n) => Some(*n),
        Value::UInt32(n) => Some(*n as i32),
        Value::Float(f) => Some(*f as i32),
        _ => None,
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    // group -> (construction 数, maintenance 数, cost 数, 样例 (instance, c, m))
    let mut census: BTreeMap<u32, (usize, usize, usize, Vec<(u32, Option<i32>, Option<i32>)>)> = BTreeMap::new();
    let (mut total_props, mut parsed, mut hit) = (0usize, 0usize, 0usize);
    for path in &paths {
        let Ok(package) = dbpf::Package::open(path) else { continue };
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            total_props += 1;
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&data) else { continue };
            parsed += 1;
            let c = i32_of(&file, HASH_CONSTRUCTION);
            let m = i32_of(&file, HASH_MAINT);
            let p = i32_of(&file, HASH_COST);
            if c.is_none() && m.is_none() && p.is_none() {
                continue;
            }
            hit += 1;
            let slot = census.entry(entry.id.group).or_default();
            if c.is_some() {
                slot.0 += 1;
            }
            if m.is_some() {
                slot.1 += 1;
            }
            if p.is_some() {
                slot.2 += 1;
            }
            if slot.3.len() < 4 {
                slot.3.push((entry.id.instance, c.or(p), m));
            }
        }
    }
    println!(
        "property 条目 {}，可解析 {}，含造价/维护键 {}",
        total_props, parsed, hit
    );
    println!("group    constr maint cost  样例(instance, construction/cost, maintenance)");
    for (group, (nc, nm, np, samples)) in &census {
        let samples = samples
            .iter()
            .map(|(i, c, m)| format!("{:08X}(c={:?},m={:?})", i, c, m))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "{:08X} {:6} {:5} {:4}  {}",
            group, nc, nm, np, samples
        );
    }
}
