//! 探针：扫包内 JSON（0x0A98EAF0）/文本资源，解压后按关键词过滤并转储命中。仅开发用。
fn main() {
    let kw = std::env::args().nth(1).unwrap().to_lowercase();
    let mut shown = 0;
    for path in std::env::args().skip(2) {
        let p = dbpf::Package::open(&path).unwrap();
        for e in p.entries() {
            if e.id.type_id != 0x0A98_EAF0 { continue; }
            let Ok(d) = p.read(e) else { continue };
            let low = String::from_utf8_lossy(&d).to_lowercase();
            if low.contains(&kw) {
                let hits = low.matches(&kw).count();
                println!("{} TGI {:08X}:{:08X}:{:08X} {}B 命中x{}", path, e.id.type_id, e.id.group, e.id.instance, d.len(), hits);
                if shown < 20 {
                    let out = format!("tmp/json/{:08X}_{:08X}.json", e.id.group, e.id.instance);
                    std::fs::create_dir_all("tmp/json").ok();
                    std::fs::write(&out, &d).ok();
                    shown += 1;
                }
            }
        }
    }
}
