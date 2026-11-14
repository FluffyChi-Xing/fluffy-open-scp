//! Match EGB curve entry IDs against every static Unit path resource.
use std::collections::HashSet;

fn main() {
    let state: serde_json::Value = serde_json::from_slice(
        &std::fs::read(std::env::args().nth(1).expect("state json")).unwrap(),
    )
    .unwrap();
    let wanted: HashSet<u32> = state["curves"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["entryId"].as_u64())
        .map(|x| x as u32)
        .collect();
    let root = std::path::PathBuf::from(std::env::args().nth(2).expect("package root"));
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
    let mut paths = Vec::new();
    collect(&root, &mut paths);
    let mut hits = 0usize;
    for path in paths {
        let Ok(pkg) = dbpf::Package::open(&path) else {
            continue;
        };
        for e in pkg.entries().iter().filter(|e| e.id.type_id == 0x00B1_B104) {
            let Ok(b) = pkg.read(e) else { continue };
            let Ok(p) = sc_properties::PropertyFile::parse_with_limits(
                &b,
                sc_properties::ParseLimits::default(),
            ) else {
                continue;
            };
            let Some(a) = p.get(0x0CC8_FE7A).and_then(|x| x.array()) else {
                continue;
            };
            let ids: Vec<u32> = a
                .iter()
                .filter_map(|v| match v {
                    sc_properties::Value::Key(k) => Some(k.instance),
                    _ => None,
                })
                .collect();
            let matched: Vec<u32> = ids.iter().copied().filter(|x| wanted.contains(x)).collect();
            if !matched.is_empty() {
                hits += 1;
                println!(
                    "{} T{:08X} G{:08X} I{:08X} pathEntries={} matched={:?}",
                    path.display(),
                    e.id.type_id,
                    e.id.group,
                    e.id.instance,
                    ids.len(),
                    matched
                        .iter()
                        .map(|x| format!("{x:08X}"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
    eprintln!(
        "static resources matched={hits}, EGB distinct entry IDs={}",
        wanted.len()
    );
}
