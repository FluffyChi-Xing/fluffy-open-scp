//! 探针：扫描菜单条目（8A01/C900）上的路径单价键 ——
//! 0x0E76A7E2（mCost float，cEcoPathEntry::FillFromProps 反编译定案，
//! 道路造价 = mCost × 路径长度，GB::CostForPath）与 0x0E76A7E3（货币资源键）。
//! 仅开发用。
//!
//! 用法：cargo run -p probes --release --example path_cost_scan -- <package...>
use sc_properties::{Kind, PropertyFile, Value};

const TYPE_PROPERTY: u32 = 0x00B1_B104;
const HASH_PATH_COST: u32 = 0x0E76_A7E2;
const HASH_COST_RESOURCE: u32 = 0x0E76_A7E3;

fn float_of(file: &PropertyFile, hash: u32) -> Option<f32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Float(v)) => Some(*v),
        Kind::Array(values) => values.iter().find_map(|v| match v {
            Value::Float(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}

fn key_instance(file: &PropertyFile, hash: u32) -> Option<u32> {
    match &file.get(hash)?.kind {
        Kind::Scalar(Value::Key(k)) => Some(k.instance),
        Kind::Array(values) => values.iter().find_map(|v| match v {
            Value::Key(k) => Some(k.instance),
            _ => None,
        }),
        _ => None,
    }
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    for path in &paths {
        let Ok(package) = dbpf::Package::open(path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != TYPE_PROPERTY {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse(&data) else {
                continue;
            };
            let Some(cost) = float_of(&file, HASH_PATH_COST) else {
                continue;
            };
            println!(
                "group={:08X} instance={:08X} pathCost={} costResource={:?}",
                entry.id.group,
                entry.id.instance,
                cost,
                key_instance(&file, HASH_COST_RESOURCE),
            );
        }
    }
}
