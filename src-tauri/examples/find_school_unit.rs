//! Scan a package's Unit records for education-category units and dump the
//! field table that drives school presentation (category/capacity/bundles).
//! Usage: cargo run --release -p fluffy-open-scp --example find_school_unit -- <package> [capacity-filter]
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY: u32 = 0x00B1_B104;
const UNIT_GROUP: u32 = 0x40E0_C000;
const HASH_PARENT: u32 = 0x00B2_CCCB;
const HASH_UNIT_CATEGORY: u32 = 0x0AFE_27B1;
const HASH_CAPACITY: u32 = 0x0AD7_FAC9;
const EDUCATION_CATEGORY: u32 = 0x95A9_D6FD;

fn render(value: &Value) -> String {
    match value {
        Value::Int32(v) => format!("i32:{v}"),
        Value::UInt32(v) => format!("u32:{v}"),
        Value::Float(v) => format!("f32:{v}"),
        Value::Bool(v) => format!("bool:{v}"),
        Value::Key(key) => format!("key({:08X},{:08X},{:08X})", key.type_id, key.group, key.instance),
        Value::Text(text) => format!("text({:08X},{})", text.table_id, text.instance_id),
        _ => format!("{value:?}"),
    }
}

fn field(file: &PropertyFile, hash: u32) -> String {
    match file.get(hash).map(|p| &p.kind) {
        None => "<absent>".into(),
        Some(Kind::Empty) => "empty".into(),
        Some(Kind::Scalar(value)) => render(value),
        Some(Kind::Array(values)) => values.iter().map(render).collect::<Vec<_>>().join(", "),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let capacity_min: f32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let unit_ids: Vec<_> = package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == PROPERTY && entry.id.group == UNIT_GROUP)
        .map(|entry| entry.id)
        .collect();
    println!("units: {}", unit_ids.len());
    let mut found = 0;
    for id in &unit_ids {
        let Ok(unit) = PropertyFile::parse(&package.read(&package.entry(*id).expect("e")).expect("r"))
        else {
            continue;
        };
        // Capacity-only mode (EDU_ANY=1): find units by capacity regardless
        // of category, so inherited-category schools also show up.
        let education = std::env::var("EDU_ANY").is_ok()
            || matches!(
                unit.get(HASH_UNIT_CATEGORY).map(|p| &p.kind),
                Some(Kind::Scalar(Value::Key(k))) if k.instance == EDUCATION_CATEGORY
            );
        if !education {
            continue;
        }
        let capacity = match unit.get(HASH_CAPACITY).map(|p| &p.kind) {
            Some(Kind::Scalar(Value::Float(v))) => *v,
            _ => 0.0,
        };
        if capacity < capacity_min {
            continue;
        }
        found += 1;
        println!(
            "=== EDU unit {:08X} capacity {capacity}",
            id.instance
        );
        for (label, hash) in [
            ("parent", HASH_PARENT),
            ("category", HASH_UNIT_CATEGORY),
            ("capacity", HASH_CAPACITY),
            ("bundleA2", 0x0A02_09A2),
            ("bundleA3", 0x0A02_09A3),
            ("bundleA4", 0x0A02_09A4),
            ("bundleA5", 0x0A02_09A5),
            ("excludeB2", 0x0A02_09B2),
            ("parkFlag", 0x0EC4_80DF),
            ("schedule?", 0x0B57_3A64),
            ("unitHours", 0x0DDE_5724),
        ] {
            let value = field(&unit, hash);
            if value != "<absent>" {
                println!("  {label:9} {:08X} [{value}]", hash);
            }
        }
        // full field list for the first hits
        if found <= 3 {
            let mut hashes: Vec<u32> = unit.values.iter().map(|p| p.hash).collect();
            hashes.sort_unstable();
            println!("  ALL: {:08X?}", hashes);
        }
    }
    println!("education units: {found}");
}
