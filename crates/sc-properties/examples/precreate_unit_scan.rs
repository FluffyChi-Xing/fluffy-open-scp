//! Locate static PrecreateUnits tables used by a new EcoGame bootstrap.
fn main() {
    let root = std::env::args().nth(1).expect("SimCityData directory");
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|x| x == "package") {
                out.push(p);
            }
        }
    }
    let mut package_paths = Vec::new();
    collect(std::path::Path::new(&root), &mut package_paths);
    // The script table stores local keys (type/group zero). Resolve those
    // instances across every package in the installation, as the engine's
    // ResourceManager does, instead of only within the script bundle.
    let wanted = [3916467622u32, 1213099590u32];
    for path in &package_paths {
        let Ok(pkg) = dbpf::Package::open(path) else { continue };
        for e in pkg.entries().iter().filter(|e| wanted.contains(&e.id.instance) && e.id.type_id == 0x00B1_B104) {
            println!("cross-package {} T{:08X} G{:08X} I{:08X}", path.display(), e.id.type_id, e.id.group, e.id.instance);
        }
    }
    for path in package_paths {
        let name = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let Ok(pkg) = dbpf::Package::open(&path) else {
            continue;
        };
        for e in pkg.entries() {
            let Ok(bytes) = pkg.read(e) else { continue };
            let Ok(pf) = sc_properties::PropertyFile::parse(&bytes) else {
                continue;
            };
            let keys = pf
                .get(0x0ADF_E207)
                .and_then(|p| p.array())
                .map_or(0, |v| v.len());
            let transforms = pf
                .get(0x0ADF_E208)
                .and_then(|p| p.array())
                .map_or(0, |v| v.len());
            if keys > 0 || transforms > 0 {
                println!(
                    "{name} {:08X}:{:08X}:{:08X} unitKeys={keys} transforms={transforms}",
                    e.id.type_id, e.id.group, e.id.instance
                );
                if let Some(p) = pf.get(0x0ADF_E207) {
                    println!(
                        "  keys={:?}",
                        p.array().or_else(|| p.scalar().map(std::slice::from_ref))
                    );
                }
                if let Some(p) = pf.get(0x0ADF_E208) {
                    println!(
                        "  transforms={:?}",
                        p.array().or_else(|| p.scalar().map(std::slice::from_ref))
                    );
                }
                for key in [3916467622u32, 1213099590u32] {
                    for target in pkg.entries().iter().filter(|x| x.id.instance == key) {
                        println!(
                            "  unit key {:08X} resource T{:08X} G{:08X} I{:08X}",
                            key, target.id.type_id, target.id.group, target.id.instance
                        );
                        if target.id.type_id == 0x00B1_B104 {
                            if let Ok(b) = pkg.read(target) {
                                if let Ok(unit) = sc_properties::PropertyFile::parse(&b) {
                                    for h in [
                                        0x0CAA_680D,
                                        0x0CAA_6841,
                                        0x0CB0_0ED8,
                                        0x0CC8_FE7A,
                                        0x0D9A_4E54,
                                        0x0D9A_A2BB,
                                    ] {
                                        if let Some(p) = unit.get(h) {
                                            println!(
                                                "    {h:08X} {:?}",
                                                p.array().or_else(|| p
                                                    .scalar()
                                                    .map(std::slice::from_ref))
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
