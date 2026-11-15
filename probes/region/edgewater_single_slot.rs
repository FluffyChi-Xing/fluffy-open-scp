//! Build a disposable EdgeWaterBay template override retaining city 1031 and site 0.
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};
use serde_json::Value as Json;
use std::{error::Error, fs::OpenOptions, io::Write};

const CITY: i32 = 1031;
const XY: (i64, i64) = (-2496, 1472);
const TABLE: ResourceId = id(0x00b1b104, 0x83831425, 0x2b9c480c);
const ENVIRONMENT: ResourceId = id(0x00b1b104, 0x81ced36f, 0x494f8678);
const TERRAIN_CITY_INDEX: u32 = 0x1b53d525;
const BINDING: ResourceId = id(0x00b1b104, 0x95f4aef0, 0x61713757);
const META: ResourceId = id(0x0a98eaf0, 0xccdff91d, 0x4529f96f);
const ROUTES: ResourceId = id(0x08068aee, 0xccdff91d, 0xf9b19bd3);

const fn id(type_id: u32, group: u32, instance: u32) -> ResourceId {
    ResourceId {
        type_id,
        group,
        instance,
    }
}

fn read(package: &Package, id: ResourceId) -> Vec<u8> {
    package
        .read(package.entry(id).expect("source TGI missing"))
        .unwrap()
}

fn keep_rows(props: &mut PropertyFile, hash: u32, count: usize, rows: &[usize]) {
    let p = props.values.iter_mut().find(|p| p.hash == hash).unwrap();
    let Kind::Array(values) = &mut p.kind else {
        panic!("expected parallel array")
    };
    assert_eq!(values.len(), count);
    *values = rows.iter().map(|&i| values[i].clone()).collect();
}

fn prop_entry(id: ResourceId, props: &PropertyFile) -> OverlayEntry {
    let bytes = props.encode_canonical().unwrap();
    let parsed = PropertyFile::parse(&bytes).unwrap();
    assert_eq!(parsed.values, props.values);
    OverlayEntry::new(id, bytes)
}

// v0 route table layout and site columns from GameManager 0x605840,
// dispatcher 0x605F90 and reverse mapping 0x605D50 in the retail executable.
fn keep_routes(bytes: &[u8]) -> Vec<u8> {
    let mut cursor = 0;
    let mut word = || {
        let value = u32::from_be_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        value
    };
    assert_eq!(word(), 0);
    let n = word();
    let inbound: Vec<[u32; 4]> = (0..n).map(|_| std::array::from_fn(|_| word())).collect();
    let m = word();
    let outbound: Vec<[u32; 5]> = (0..m).map(|_| std::array::from_fn(|_| word())).collect();
    assert_eq!(cursor, bytes.len());
    let keep_site = |site| site == 0 || site == CITY as u32;
    let inbound: Vec<_> = inbound.into_iter().filter(|r| keep_site(r[3])).collect();
    let outbound: Vec<_> = outbound
        .into_iter()
        .filter(|r| keep_site(r[0]) && inbound.iter().any(|i| (i[0], i[2]) == (r[2], r[3])))
        .collect();
    println!(
        "trade routes {n}/{m} -> {}/{}",
        inbound.len(),
        outbound.len()
    );
    let mut out = Vec::new();
    for w in [0, inbound.len() as u32]
        .into_iter()
        .chain(inbound.into_iter().flatten())
        .chain([outbound.len() as u32])
        .chain(outbound.into_iter().flatten())
    {
        out.extend(w.to_be_bytes());
    }
    out
}

fn main() -> Result<(), Box<dyn Error>> {
    let a: Vec<_> = std::env::args().skip(1).collect();
    let [terrain, ep, output] = a.as_slice() else {
        return Err("expected RegionTerrain0.package SimCityDataEP1.package out.package".into());
    };
    let terrain = Package::open(terrain)?;
    let ep = Package::open(ep)?;
    let mut table = PropertyFile::parse(&read(&terrain, TABLE))?;
    let mut binding = PropertyFile::parse(&read(&ep, BINDING))?;
    let slots = table.get(0x0543bc96).unwrap().array().unwrap();
    let slot = slots
        .iter()
        .position(|v| matches!(v, Value::String8(s) if s == "1031"))
        .unwrap();
    assert_eq!(slots.len(), 7);
    assert_eq!(
        table.get(0x16b7b1ef).unwrap().array().unwrap()[slot],
        Value::UInt32(0x81ced36f)
    );
    for hash in [0x0543bc96, 0x16b7b1ef, 0x9f2f9b65, 0xc0b13e7e, 0xf01de4b1] {
        keep_rows(&mut table, hash, 7, &[slot]);
    }
    // ExportEnvironmentPropFile stores the CityList row, not the site UID.
    // Retail TerrainGfx::Init passes it directly to TerrainExporterCity::Init.
    let mut environment = PropertyFile::parse(&read(&terrain, ENVIRONMENT))?;
    let index = environment
        .values
        .iter_mut()
        .find(|p| p.hash == TERRAIN_CITY_INDEX)
        .ok_or("terrain environment city index missing")?;
    assert_eq!(index.kind, Kind::Scalar(Value::Int32(slot.try_into()?)));
    let groups = table.get(0x16b7b1ef).unwrap().array().unwrap();
    let new_index = groups
        .iter()
        .position(|v| *v == Value::UInt32(ENVIRONMENT.group))
        .ok_or("retained environment not found in final CityList")?;
    index.kind = Kind::Scalar(Value::Int32(new_index.try_into()?));
    let sites = binding.get(0x46066281).unwrap().array().unwrap();
    assert_eq!(sites[0], Value::Int32(0));
    assert_eq!(sites.len(), 8);
    let site = sites.iter().position(|v| *v == Value::Int32(CITY)).unwrap();
    let keys = binding.get(0x46066280).unwrap().array().unwrap();
    for (&i, instance) in [(&0, 0x7439cb9c), (&site, 0x021bda2f)] {
        let Value::Key(key) = keys[i] else {
            panic!("bootstrap key missing")
        };
        assert_eq!(
            (key.type_id, key.group, key.instance),
            (0x08068aed, 0xccdff91d, instance)
        );
        assert!(ep.entry(id(key.type_id, key.group, key.instance)).is_some());
    }
    for hash in [0x46066280, 0x46066281] {
        keep_rows(&mut binding, hash, 8, &[0, site]);
    }
    let mut meta: Json = serde_json::from_slice(&read(&ep, META))?;
    assert_eq!(meta["name"], "EdgeWaterBay");
    let xy = |v: &Json| (v["x"].as_i64().unwrap(), v["y"].as_i64().unwrap());
    let cities = meta["map"]["cities"].as_array().unwrap();
    let boxes = meta["boxes"].as_array().unwrap();
    assert_eq!(cities.len(), 7);
    assert_eq!(boxes.len(), 7);
    assert!(cities.iter().zip(boxes).all(|(city, b)| xy(city) == xy(b)));
    assert_eq!(cities[slot]["uid"], "1031");
    assert_eq!(xy(&cities[slot]), XY);
    let (city, city_box) = (cities[slot].clone(), boxes[slot].clone());
    meta["map"]["cities"] = Json::Array(vec![city]);
    meta["boxes"] = Json::Array(vec![city_box]);
    let routes = keep_routes(&read(&ep, ROUTES));
    assert_eq!(routes, keep_routes(&routes));
    let entries = [
        prop_entry(TABLE, &table),
        prop_entry(BINDING, &binding),
        OverlayEntry::new(META, serde_json::to_vec(&meta)?),
        OverlayEntry::new(ROUTES, routes),
        prop_entry(ENVIRONMENT, &environment),
    ];
    let payload = dbpf::write_uncompressed_overlay(&entries)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&payload)?;
    drop(file);
    let check = Package::open(output)?;
    assert_eq!(check.entries().len(), entries.len());
    for entry in entries {
        assert_eq!(read(&check, entry.id), entry.data);
    }
    let written_table = PropertyFile::parse(&read(&check, TABLE))?;
    let written_environment = PropertyFile::parse(&read(&check, ENVIRONMENT))?;
    let Kind::Scalar(Value::Int32(row)) = written_environment
        .get(TERRAIN_CITY_INDEX)
        .ok_or("written terrain index missing")?
        .kind
    else {
        return Err("written terrain index has the wrong type".into());
    };
    let row = usize::try_from(row)?;
    let written_groups = written_table.get(0x16b7b1ef).unwrap().array().unwrap();
    assert_eq!(
        written_groups.get(row),
        Some(&Value::UInt32(ENVIRONMENT.group))
    );
    println!("one city/box/plot: 1031; bootstrap sites: 0, 1031");
    Ok(())
}
