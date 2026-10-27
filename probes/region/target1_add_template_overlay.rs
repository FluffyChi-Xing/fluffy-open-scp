//! Add the authoritative EP1 Cape Trinity template replacement to target1.
//! Usage: <old-overlay> <EP1.package> <out.package> <manifest.json>
use dbpf::{OverlayEntry, Package};
fn main() {
    let mut a = std::env::args().skip(1);
    let old = Package::open(a.next().unwrap()).unwrap();
    let ep1 = Package::open(a.next().unwrap()).unwrap();
    let out_path = a.next().unwrap();
    let manifest_path = a.next().unwrap();
    let src = ep1
        .entries()
        .iter()
        .find(|e| {
            e.id.type_id == 0x0A98_EAF0 && e.id.group == 0xDACB_5A49 && e.id.instance == 0x4529_F96F
        })
        .expect("Cape Trinity template");
    let raw = ep1.read(src).unwrap();
    let start = raw.iter().position(|b| *b == b'{').unwrap();
    let prefix = &raw[..start];
    let mut v: serde_json::Value = serde_json::from_slice(&raw[start..]).unwrap();
    let city_count = {
        let map = v.get_mut("map").and_then(|x| x.as_object_mut()).unwrap();
        let cities = map
            .get_mut("cities")
            .and_then(|x| x.as_array_mut())
            .unwrap();
        cities.retain(|c| c.get("uid").and_then(|x| x.as_str()) != Some("1026"));
        cities.len()
    };
    let box_count = {
        let boxes = v.get_mut("boxes").and_then(|x| x.as_array_mut()).unwrap();
        boxes.retain(|b| {
            !(b.get("x").and_then(|x| x.as_i64()) == Some(1600)
                && b.get("y").and_then(|x| x.as_i64()) == Some(-1856))
        });
        boxes.len()
    };
    let mut data = prefix.to_vec();
    serde_json::to_writer(&mut data, &v).unwrap();
    let mut entries = Vec::new();
    for e in old.entries() {
        entries.push(OverlayEntry::new(e.id, old.read(e).unwrap()));
    }
    entries.push(OverlayEntry::new(src.id, data.clone()));
    std::fs::write(
        &out_path,
        dbpf::write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    let m = serde_json::json!({"base":"cape-trinity-clearwater.package","addedTemplate":"0A98EAF0:DACB5A49:4529F96F","removedUid":"1026","cityCount":city_count,"boxCount":box_count,"entries":entries.len()});
    std::fs::write(manifest_path, serde_json::to_vec_pretty(&m).unwrap()).unwrap();
    println!(
        "wrote {} entries; template cities={} boxes={}",
        entries.len(),
        city_count,
        box_count
    );
}
