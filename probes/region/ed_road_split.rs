//! 探针：b0<=2 按 0/1/2 分色（公路 vs 铁路?）+ 地块表 ids/positions 转储。仅开发用。
use std::collections::HashMap;
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let group = 0xBEAF_0510u32;
    let grid = &sc_properties::region_map::SHARED_ED_GRID;
    let sgrid = &sc_properties::region_map::SHARED_TILE_GRID;
    let sea = sc_properties::region_map::WATER_LEVEL as i32;
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.group != group { continue; }
        match e.id.type_id {
            0x03E4_21ED => { if let Ok(d) = p.read(e) { if d.len() == 65556 {
                ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()); } } }
            0x03E4_21F0 => { if let Ok(d) = p.read(e) { if d.len() == 131092 {
                f0.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()); } } }
            _ => {}
        }
    }
    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in sgrid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = f0.get(inst) {
                for y in 0..256 { hgt[(ty*256+y)*w+tx*256..(ty*256+y)*w+tx*256+256].copy_from_slice(&px[y*256..(y+1)*256]); }
            }
        }
    }
    let wm = 2048usize;
    let mut b0 = vec![0u8; wm * wm];
    for (ty, row) in grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let Some(px) = ed.get(inst) else { continue };
            for y in 0..128 { for x in 0..128 {
                b0[(ty*128+y)*wm+tx*128+x] = (px[y*128+x] & 0xFF) as u8;
            } }
        }
    }
    let mut img = image::RgbImage::new(wm as u32, wm as u32);
    let mut cnt = [0usize; 3];
    for y in 0..wm { for x in 0..wm {
        let h = hgt[(y*2)*w + x*2] as i32;
        let v = b0[y*wm+x];
        let c = if h < sea || h == 0 { [60, 90, 160] }
            else if v == 0 { cnt[0] += 1; [230, 30, 30] }
            else if v == 1 { cnt[1] += 1; [250, 180, 20] }
            else if v == 2 { cnt[2] += 1; [60, 200, 60] }
            else { [v, v, v] };
        img.put_pixel(x as u32, y as u32, image::Rgb(c));
    } }
    img.save("tmp/region_preview/ed_b0_split_BEAF0510.png").unwrap();
    println!("b0=0 陆上 {} px, b0=1 {} px, b0=2 {} px", cnt[0], cnt[1], cnt[2]);

    // ---- 地块表转储（Q4）----
    for g in [0xBEAF_0510u32, 0xD01F_A985] {
        for e in p.entries() {
            if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C || e.id.group != g { continue; }
            let d = p.read(e).unwrap();
            let pf = sc_properties::PropertyFile::parse(&d).unwrap();
            let ids: Vec<u32> = pf.get(0x16B7_B1EF).and_then(|pr| match &pr.kind {
                sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect()),
                _ => None,
            }).unwrap_or_default();
            let pos: Vec<(f32, f32)> = pf.get(0xF01D_E4B1).and_then(|pr| match &pr.kind {
                sc_properties::Kind::Array(vs) => Some(vs.iter().filter_map(|v| match v { sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect()),
                _ => None,
            }).unwrap_or_default();
            println!("区域 {g:08X} 地块表: ids={} positions={}", ids.len(), pos.len());
            for (i, id) in ids.iter().enumerate() {
                let p2 = pos.get(i).map(|(x, y)| format!("({x:.0},{y:.0})")).unwrap_or_default();
                println!("   [{i}] {:08X} {p2}", id);
            }
        }
    }
}
