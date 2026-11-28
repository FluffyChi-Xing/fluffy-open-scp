//! Read-only extraction of explicit garbage category bindings for the small library.
use dbpf::Package;
use sc_properties::{PropertyFile, Value};
fn main() {
    if std::env::args().nth(1).as_deref() == Some("--names") {
        let registry = sc_registry::Registry::open("src-tauri/resources/database_main.s3db").unwrap();
        for (id, record) in registry.instances() {
            let name = record.name.to_lowercase();
            if ["garbage", "trash", "dumpster", "litter", "recycle"].iter().any(|word| name.contains(word)) {
                println!("0x{id:08X}, // {}", record.name);
            }
        }
        return;
    }
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let mut paths = vec![];
    for dir in [root.join("SimCityData"), root.join("SimCityUserData/EcoGame")] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            paths.extend(entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "package")));
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    for path in paths {
        let Ok(pkg) = Package::open(path) else { continue };
        for e in pkg.entries().iter().filter(|e| e.id.type_id == 0x00b1b104 && [0xc100, 0xc600].contains(&(e.id.group as u16))) {
            let Ok(bytes) = pkg.read(e) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            if let Some(p) = file.get(0x08a16042) {
                let vals = match &p.kind { sc_properties::Kind::Array(v) => v.as_slice(), sc_properties::Kind::Scalar(v) => std::slice::from_ref(v), _ => &[] };
                if vals.iter().any(|v| matches!(v, Value::Key(k) if k.instance == 0x7cb23a4e)) { ids.insert(e.id.instance); }
            }
        }
    }
    for id in ids { println!("0x{id:08X},"); }
}
