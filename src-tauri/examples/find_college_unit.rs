//! Scan all EcoGame scripts packages for units mounting bundle 6C042366
//! (社區大學) or 3256C0A2 (大学); dump their bundle tables + job leaves.
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
fn main() {
    let dir = std::path::Path::new(
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityUserData/EcoGame",
    );
    let targets: [(u32, &str); 2] = [(0x6C04_2366, "college"), (0x3256_C0A2, "university")];
    for entry in std::fs::read_dir(dir).expect("dir") {
        let path = entry.expect("e").path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.contains("Scripts_") {
            continue;
        }
        let Ok(package) = Package::open(&path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse(&bytes) else {
                continue;
            };
            let mut bundles: Vec<u32> = Vec::new();
            for hash in [0x0A02_09A2u32, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5] {
                if let Some(Kind::Array(vs)) = file.get(hash).map(|p| &p.kind) {
                    for v in vs {
                        if let Value::Key(k) = v {
                            bundles.push(k.instance);
                        }
                    }
                }
            }
            for (bundle, label) in targets {
                if bundles.contains(&bundle) {
                    let w1 = file.get(0x0A79_17C5).and_then(|p| match &p.kind {
                        Kind::Scalar(Value::Float(x)) => Some(*x),
                        _ => None,
                    });
                    println!(
                        "{name} {:08X} {label} bundles={} w1={w1:?}",
                        entry.id.instance,
                        bundles
                            .iter()
                            .map(|b| format!("{b:08X}"))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                }
            }
        }
    }
}
