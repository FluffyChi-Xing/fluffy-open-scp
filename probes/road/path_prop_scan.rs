//! Census static property resources carrying starter path arrays.
fn main() {
    let root = std::env::args().nth(1).expect("SimCityData directory");
    fn collect(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(es) = std::fs::read_dir(dir) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, out)
                } else if p.extension().is_some_and(|x| x == "package") {
                    out.push(p)
                }
            }
        }
    }
    let mut paths = Vec::new();
    collect(std::path::Path::new(&root), &mut paths);
    for path in paths {
        let name = path.file_name().unwrap().to_string_lossy();
        let Ok(pkg) = dbpf::Package::open(&path) else {
            continue;
        };
        for e in pkg.entries().iter().filter(|e| e.id.type_id == 0x00B1_B104) {
            let Ok(bytes) = pkg.read(e) else { continue };
            let Ok(pf) = sc_properties::PropertyFile::parse(&bytes) else {
                continue;
            };
            let points = pf
                .get(0x0CAA_680D)
                .and_then(|p| p.array())
                .map_or(0, |v| v.len());
            let pairs = pf
                .get(0x0CAA_6841)
                .and_then(|p| p.array())
                .map_or(0, |v| v.len());
            let entries = pf
                .get(0x0CC8_FE7A)
                .and_then(|p| p.array())
                .map_or(0, |v| v.len());
            if points > 0 || pairs > 0 || entries > 0 {
                println!(
                    "{name} {:08X}:{:08X}:{:08X} points={points} pairs={pairs} pathEntries={entries}",
                    e.id.type_id, e.id.group, e.id.instance
                );
            }
        }
    }
}
