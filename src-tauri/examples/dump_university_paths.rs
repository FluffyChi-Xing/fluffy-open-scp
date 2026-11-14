//! Dump the vanilla university lot's full path configuration (entries, scope,
//! roadConnect, per-path point/segment ranges).
//! Usage: cargo run -q -p fluffy-open-scp --example dump_university_paths -- <package> <lot-instance-hex>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn keys(file: &PropertyFile, hash: u32) -> Vec<u32> {
    file.get(hash)
        .and_then(|p| match &p.kind {
            Kind::Array(vs) => Some(
                vs.iter()
                    .filter_map(|v| match v {
                        Value::Key(k) => Some(k.instance),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}
fn ints(file: &PropertyFile, hash: u32) -> Vec<i32> {
    file.get(hash)
        .and_then(|p| match &p.kind {
            Kind::Array(vs) => Some(
                vs.iter()
                    .filter_map(|v| match v {
                        Value::Int32(x) => Some(*x),
                        Value::UInt32(x) => Some(*x as i32),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let instance = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x41E1_C000,
        instance,
    };
    let Some(entry) = package.entry(id) else {
        println!("lot {instance:08X} not found");
        return;
    };
    let bytes = package.read(entry).expect("read");
    let file = PropertyFile::parse(&bytes).expect("parse");
    let entries = keys(&file, 0x0CC8_FE7A);
    let sp_ids = keys(&file, 0x0E1B_AC61);
    let sp_count = file
        .get(0x0E1B_AC62)
        .and_then(|p| match &p.kind {
            Kind::Array(vs) => Some(vs.len()),
            _ => None,
        })
        .unwrap_or(0);
    println!(
        "spawners: ids={} transforms_count={}",
        sp_ids.len(),
        sp_count
    );
    let ec = file.get(0x0E1B_AC62).and_then(|p| match &p.kind {
        Kind::Array(vs) => Some(
            vs.iter()
                .take(3)
                .map(|v| format!("{v:?}"))
                .collect::<Vec<_>>(),
        ),
        _ => None,
    });
    if let Some(ec) = ec {
        for (i, t) in ec.iter().enumerate() {
            println!("  sp[{i}] {t}");
        }
    }
    let scope = keys(&file, 0x0D9A_4E54);
    let road = keys(&file, 0x0D9A_A2BB);
    let ranges = ints(&file, 0x0CAA_6841);
    let concat = ints(&file, 0x0CAA_6832);
    let points = ints(&file, 0x0CAA_680D);
    println!(
        "lot {instance:08X}: paths={} pool_points={} concat={}",
        entries.len(),
        points.len(),
        concat.len()
    );
    let name = |i: u32| match i {
        0x418A_18B1 => "Pedestrians".to_string(),
        0xE0CD_3122 => "Pedestrians2".to_string(),
        0x0310_6C2E => "CarsAndCrap".to_string(),
        0x7C9C_4FFF => "SchoolBusses".to_string(),
        0xC37D_661D => "SchoolBusses2".to_string(),
        0x36BA_75FA => "ShuttleBuss".to_string(),
        0xE2AD_993F => "ServiceCars".to_string(),
        other => format!("{other:08X}"),
    };
    let scope_name = |i: u32| match i {
        0xF44F_784A => "Local".to_string(),
        other => format!("{other:08X}"),
    };
    let road_name = |i: u32| match i {
        0xC186_DA37 => "None".to_string(),
        0xF0F6_1CED => "First".to_string(),
        other => format!("{other:08X}"),
    };
    for (index, entry) in entries.iter().enumerate() {
        let (a, b) = (ranges.get(index * 2), ranges.get(index * 2 + 1));
        let seg = match (a, b) {
            (Some(a), Some(b)) => format!("range[{a}..{b}] {}pt", b - a + 1),
            _ => "?".into(),
        };
        println!(
            "  path[{index}] {} scope={} roadConnect={} {}",
            name(*entry),
            scope.get(index).map_or("?".to_string(), |v| scope_name(*v)),
            road.get(index).map_or("?".to_string(), |v| road_name(*v)),
            seg,
        );
    }
    println!("concat list: {concat:?}");
}
