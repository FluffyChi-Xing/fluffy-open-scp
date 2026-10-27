//! Add the Game registry JS replacement to target1.
use dbpf::{OverlayEntry, Package};

const REGISTRY_TYPE: u32 = 0x6777_1F5C;

fn remove_city(raw: &[u8]) -> Option<Vec<u8>> {
    let s = String::from_utf8(raw.to_vec()).ok()?;
    let start = s.find("CapeTrinity:{")?;
    let end = start + s[start..].find("},CaspianLake:{")?;
    let seg = &s[start..end];
    let cp = seg.find("cities:{1026:{")?;
    let body_start = cp + "cities:{".len();
    let next = seg[body_start..].find("},1027:{")?;
    let next_abs = body_start + next + 2;
    let mut replaced = String::with_capacity(s.len() - 128);
    replaced.push_str(&s[..start]);
    replaced.push_str(&seg[..body_start]);
    replaced.push_str(&seg[next_abs..]);
    replaced.push_str(&s[end..]);
    let region = &replaced[start..start + replaced[start..].find("},CaspianLake:{")?];
    if region.contains("cities:{1026:{") || !region.contains("cities:{1027:{") {
        return None;
    }
    Some(replaced.into_bytes())
}

fn main() {
    let mut a = std::env::args().skip(1);
    let old = Package::open(a.next().unwrap()).unwrap();
    let game = Package::open(a.next().unwrap()).unwrap();
    let out = a.next().unwrap();
    let mut entries = Vec::new();
    for x in old.entries() {
        if x.id.type_id == REGISTRY_TYPE {
            continue;
        }
        entries.push(OverlayEntry::new(x.id, old.read(x).unwrap()));
    }
    let mut changed = 0;
    for e in game
        .entries()
        .iter()
        .filter(|e| e.id.type_id == REGISTRY_TYPE)
    {
        let raw = game.read(e).unwrap();
        if let Some(replaced) = remove_city(&raw) {
            entries.push(OverlayEntry::new(e.id, replaced));
            changed += 1;
        }
    }
    assert!(changed > 0, "no CapeTrinity registry resource found");
    std::fs::write(out, dbpf::write_uncompressed_overlay(&entries).unwrap()).unwrap();
    println!(
        "wrote {} entries; removed CapeTrinity city 1026 from {} registry variants",
        entries.len(),
        changed
    );
}
