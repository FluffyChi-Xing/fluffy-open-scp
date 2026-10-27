//! 全量 dump 指定区域的地块表（自动配对）。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let region = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    // 城市组 → 确认区域 + 收集城市 id
    let mut city_ids: Vec<u32> = Vec::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 { continue; }
        let Ok(pf) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
            if k.instance == 0x51E7_A18D && k.group == region { city_ids.push(e.id.group); }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    // 选表
    let mut best: Option<(usize, u32)> = None;
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C { continue; }
        let Ok(pt) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pt.get(0x16B7_B1EF) else { continue };
        let ov = vs.iter().filter(|v| match v { sc_properties::Value::UInt32(x) => set.contains(x), _ => false }).count();
        if best.as_ref().map(|(b, _)| ov > *b).unwrap_or(true) { best = Some((ov, e.id.group)); }
    }
    let tgroup = best.expect("no table").1;
    println!("table group = {tgroup:08X}");
    // 全量打印该表的所有 key
    for e in p.entries() {
        if e.id.type_id == 0x00B1_B104 && e.id.group == tgroup && e.id.instance == 0x2B9C_480C {
            let pt = sc_properties::PropertyFile::parse(&p.read(e).unwrap()).unwrap();
            print!("{pt}");
            return;
        }
    }
}
