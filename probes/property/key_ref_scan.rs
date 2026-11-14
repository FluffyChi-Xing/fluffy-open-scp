fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).unwrap());
    let needle = 0x30360939u32;
    fn c(d: &std::path::Path, o: &mut Vec<std::path::PathBuf>) {
        if let Ok(es) = std::fs::read_dir(d) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    c(&p, o)
                } else if p.extension().is_some_and(|x| x == "package") {
                    o.push(p)
                }
            }
        }
    }
    let mut ps = vec![];
    c(&root, &mut ps);
    for path in ps {
        let Ok(pkg) = dbpf::Package::open(&path) else {
            continue;
        };
        for e in pkg.entries() {
            let Ok(b) = pkg.read(e) else { continue };
            let Ok(p) = sc_properties::PropertyFile::parse(&b) else {
                continue;
            };
            for pr in &p.values {
                let vals = pr.array().or_else(|| pr.scalar().map(std::slice::from_ref));
                if vals.is_some_and(|vs| {
                    vs.iter()
                        .any(|v| matches!(v,sc_properties::Value::Key(k) if k.instance==needle))
                }) {
                    println!(
                        "{} T{:08X} G{:08X} I{:08X} prop {:08X}",
                        path.display(),
                        e.id.type_id,
                        e.id.group,
                        e.id.instance,
                        pr.hash
                    );
                }
            }
        }
    }
}
