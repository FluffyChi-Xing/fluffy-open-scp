//! 探针：EcoGame 脚本包（SimCityUserData/EcoGame/*.package）的 Unit 条目
//! （组 40E0C000/48E0C000）经济字段 + 名称字符串，供与菜单条目联查。仅开发用。
//!
//! 用法：cargo run -p probes --release --example eco_unit_cost_scan -- <package> [名称文本id]
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const UNIT_GROUPS: [u32; 2] = [0x40E0_C000, 0x48E0_C000];
const HASH_CONSTRUCTION: u32 = 0x0A8A_41DA;
const HASH_MAINT: u32 = 0x09AE_19D7;
const LOCALE_NAMES: u32 = 0x6C96_9DEE;

fn num_of(file: &PropertyFile, hash: u32) -> Option<i32> {
    let entry = file.get(hash)?;
    let values: Vec<&Value> = match &entry.kind {
        Kind::Scalar(v) => vec![v],
        Kind::Array(values) => values.iter().collect(),
        Kind::Empty => return None,
    };
    values.iter().find_map(|v| match v {
        Value::Int32(n) => Some(*n),
        Value::UInt32(n) => Some(*n as i32),
        Value::Float(f) => Some(*f as i32),
        _ => None,
    })
}

fn name_ids(file: &PropertyFile) -> Vec<u32> {
    let mut out = Vec::new();
    for entry in &file.values {
        let refs: Vec<&Value> = match &entry.kind {
            Kind::Scalar(v) => vec![v],
            Kind::Array(values) => values.iter().collect(),
            Kind::Empty => continue,
        };
        for v in refs {
            if let Value::Text(t) = v {
                if t.table_id == LOCALE_NAMES {
                    out.push(t.instance_id);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: eco_unit_cost_scan <package> [name-text-id]");
    let want: Option<u32> = args
        .next()
        .and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
    let package = dbpf::Package::open(&path).expect("open package");
    let mut rows: BTreeMap<u32, (Vec<u32>, Option<i32>, Option<i32>)> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != TYPE_PROPERTY || !UNIT_GROUPS.contains(&entry.id.group) {
            continue;
        }
        let Ok(data) = package.read(entry) else {
            continue;
        };
        let Ok(file) = PropertyFile::parse(&data) else {
            continue;
        };
        let c = num_of(&file, HASH_CONSTRUCTION);
        let m = num_of(&file, HASH_MAINT);
        if c.is_none() && m.is_none() {
            continue;
        }
        let ids = name_ids(&file);
        if let Some(want) = want {
            if !ids.contains(&want) {
                continue;
            }
        }
        rows.insert(entry.id.instance, (ids, c, m));
    }
    for (instance, (ids, c, m)) in &rows {
        let ids = ids
            .iter()
            .map(|i| format!("{i:08X}"))
            .collect::<Vec<_>>()
            .join(",");
        println!(
            "{:08X}  constr={:?} maint={:?}  names=[{}]",
            instance, c, m, ids
        );
    }
    println!("-- 共 {} 条", rows.len());
}
