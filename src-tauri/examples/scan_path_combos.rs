//! Count path-entry combinations across vanilla lots: which entry sets do
//! multi-path lots actually use? Usage: ... -- <package>
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    let mut combos: BTreeMap<String, (usize, Vec<u32>)> = BTreeMap::new();
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x40E1_C000 {
            continue;
        }
        let Ok(bytes) = package.read(entry) else {
            continue;
        };
        let Ok(file) = PropertyFile::parse(&bytes) else {
            continue;
        };
        let Some(sc_properties::Property {
            kind: Kind::Array(vs),
            ..
        }) = file.values.iter().find(|p| p.hash == 0x0CC8_FE7A)
        else {
            continue;
        };
        let mut ids: Vec<u32> = vs
            .iter()
            .filter_map(|v| match v {
                Value::Key(k) => Some(k.instance),
                _ => None,
            })
            .collect();
        if ids.is_empty() {
            continue;
        }
        ids.sort_unstable();
        ids.dedup();
        let key = ids
            .iter()
            .map(|i| format!("{i:08X}"))
            .collect::<Vec<_>>()
            .join("+");
        let slot = combos.entry(key).or_insert_with(|| (0, ids.clone()));
        slot.0 += 1;
    }
    let mut ranked: Vec<_> = combos.into_iter().collect();
    ranked.sort_by_key(|(_, (count, _))| std::cmp::Reverse(*count));
    println!("# vanilla lot path-entry combos (top 18):");
    for (key, (count, ids)) in ranked.iter().take(18) {
        let names: Vec<String> = ids
            .iter()
            .map(|i| match *i {
                0x418A_18B1 => "Pedestrians".into(),
                0xE0CD_3122 => "Pedestrians2".into(),
                0x0310_6C2E => "CarsAndCrap".into(),
                0x7C9C_4FFF => "SchoolBusses".into(),
                0xC37D_661D => "SchoolBusses2".into(),
                0x36BA_75FA => "ShuttleBuss".into(),
                0xE2AD_993F => "ServiceCars".into(),
                _ => format!("{i:08X}"),
            })
            .collect();
        println!("{count:4}x  {key}   ({})", names.join(", "));
    }
}
