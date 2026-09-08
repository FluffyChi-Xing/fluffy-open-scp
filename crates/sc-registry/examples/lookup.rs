//! instance/property 查名。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let db = args.next().unwrap();
    let registry = sc_registry::Registry::open(db).unwrap();
    for id in args {
        let v = u32::from_str_radix(id.trim_start_matches("0x"), 16).unwrap();
        for (name, map) in [("instance", registry.instances()), ("property", registry.properties())] {
            if let Some(r) = map.get(&v) {
                println!("{name} 0x{v:08X}: {}", r.name);
            }
        }
    }
}
