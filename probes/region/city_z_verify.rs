//! 探针：全链路一致性——模板城市 z vs 高度图采样 z。仅开发用。
//! 链路：EP1 模板 JSON（instance 4529F96F）城市 (x,y,z) → RT0 高度图 tile 采样 → z=raw/32-1024 对拍。
fn main() {
    let ep1 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package").unwrap();
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    // 拼 Confluence（白水谷 BEAF0510）4096² 高度场
    let mut tiles: std::collections::HashMap<u32, Vec<u16>> = Default::default();
    for e in rt0.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == 0xBEAF_0510 {
            if let Ok(d) = rt0.read(e) { if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }}
        }
    }
    let sample = |wx: f64, wy: f64| -> f64 {
        let cx = ((wx + 16384.0) / 8.0).round() as isize;
        let cy = ((wy + 16384.0) / 8.0).round() as isize;
        if cx < 0 || cy < 0 || cx >= 4096 || cy >= 4096 { return f64::NAN; }
        let inst = sc_properties::region_map::SHARED_TILE_GRID[(cy / 256) as usize][(cx / 256) as usize];
        let t = &tiles[&inst];
        let px = (cy % 256) as usize * 256 + (cx % 256) as usize;
        t[px] as f64 / 32.0 - 1024.0
    };
    for e in ep1.entries() {
        if e.id.type_id != 0x0A98_EAF0 || e.id.instance != 0x4529_F96F { continue; }
        let d = ep1.read(e).unwrap();
        let txt = std::str::from_utf8(&d).unwrap().trim_start_matches(|c| c != '{');
        // 只处理 Confluence：粗查名字
        if !txt.contains("\"Confluence\"") { continue; }
        // 城市数组提取（找准 map.cities）
        let ci = txt.find("\"cities\"").unwrap();
        let seg = &txt[ci..txt.find("\"greatWorks\"").unwrap()];
        let mut n = 0;
        let mut b = seg.as_bytes();
        let mut i = 0;
        while let Some(p) = find_obj(b, i) {
            let (obj, ni) = read_obj(b, p);
            i = ni;
            let get = |k: &str| -> String {
                let pat = format!("\"{k}\"");
                if let Some(q) = obj.find(&pat) {
                    let rest = &obj[q + pat.len()..];
                    let r = rest.trim_start_matches(|c: char| c == ' ' || c == ':' || c == '"');
                    let e = r.find(|c: char| c == '"' || c == ',' || c == '}' || c == '\r' || c == '\n').unwrap_or(r.len());
                    r[..e].to_string()
                } else { String::new() }
            };
            let (x, y, z) = (get("x").parse::<f64>().unwrap_or(f64::NAN), get("y").parse::<f64>().unwrap_or(f64::NAN), get("z").parse::<f64>().unwrap_or(f64::NAN));
            let uid = get("uid");
            if !x.is_nan() {
                let zs = sample(x, y);
                println!("城市 {uid:>5} ({x:>7.1},{y:>7.1}) 模板z={z:8.2} 高度图z={zs:8.2} 差={:+7.2}", z - zs);
                n += 1;
            }
            if n > 8 { break; }
        }
        return;
    }
    println!("未找到 Confluence 模板");
}
fn find_obj(b: &[u8], from: usize) -> Option<usize> { b[from..].iter().position(|&c| c == b'{').map(|p| p + from) }
fn read_obj(b: &[u8], s: usize) -> (&str, usize) {
    let mut d = 0;
    for i in s..b.len() {
        match b[i] { b'{' => d += 1, b'}' => { d -= 1; if d == 0 { return (std::str::from_utf8(&b[s..=i]).unwrap(), i + 1) } }, _ => {} }
    }
    (std::str::from_utf8(&b[s..]).unwrap(), b.len())
}
