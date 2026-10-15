//! Dump the Cape Trinity EP1 template resource TGI and map.cities entries.
fn main() {
    let p = dbpf::Package::open(std::env::args().nth(1).unwrap()).unwrap();
    for e in p
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x0A98_EAF0 && e.id.instance == 0x4529_F96F)
    {
        let d = p.read(e).unwrap();
        let s = String::from_utf8_lossy(&d);
        if s.contains("CapeTrinity") || s.contains("Cape Trinity") || s.contains("C2A9C48F") {
            println!(
                "TGI {:08X}:{:08X}:{:08X} bytes={}",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                d.len()
            );
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(
                s.trim_start_matches(|c: char| c != '{').as_bytes(),
            ) {
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                println!("{}", s);
            }
        }
    }
}
