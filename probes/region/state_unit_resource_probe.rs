//! Match unit entry IDs from a decompressed region state to static property resources.
fn main() {
    let state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(std::env::args().nth(1).unwrap()).unwrap()).unwrap();
    let wanted: std::collections::HashSet<u32> = state["units"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|u| u["id"].as_u64())
        .map(|x| x as u32)
        .collect();
    let root = std::path::PathBuf::from(std::env::args().nth(2).unwrap());
    fn collect(d: &std::path::Path, o: &mut Vec<std::path::PathBuf>) {
        if let Ok(es) = std::fs::read_dir(d) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, o)
                } else if p.extension().is_some_and(|x| x == "package") {
                    o.push(p)
                }
            }
        }
    }
    let mut ps = Vec::new();
    collect(&root, &mut ps);
    for p in ps {
        let Ok(pkg) = dbpf::Package::open(&p) else {
            continue;
        };
        for e in pkg
            .entries()
            .iter()
            .filter(|e| wanted.contains(&e.id.instance))
        {
            if e.id.type_id == 0x00B1_B104 {
                let Ok(bytes) = pkg.read(e) else { continue };
                let Ok(pf) = sc_properties::PropertyFile::parse_with_limits(
                    &bytes,
                    sc_properties::ParseLimits::default(),
                ) else {
                    continue;
                };
                let n = |h: u32| pf.get(h).and_then(|x| x.array()).map_or(0, |x| x.len());
                let path = n(0x0CAA_680D);
                let pairs = n(0x0CAA_6841);
                let entries = n(0x0CC8_FE7A);
                println!(
                    "{:08X} T{:08X} G{:08X} points={path} pairs={pairs} pathEntries={entries} props={}",
                    e.id.instance,
                    e.id.type_id,
                    e.id.group,
                    pf.values.len()
                );
            }
        }
    }
}
