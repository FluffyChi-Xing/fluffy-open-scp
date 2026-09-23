//! 探针：b0 亮带/暗线 与 水陆掩码、坡度 的关系判定（公路 vs 河流 vs 铁路）。仅开发用。
use std::collections::HashMap;
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let group = 0xBEAF_0510u32;
    let (grid, sgrid) = (&sc_properties::region_map::SHARED_ED_GRID, &sc_properties::region_map::SHARED_TILE_GRID);
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
    // 分类统计
    let mut bright = (0usize, 0usize); // (水上, 陆上)
    let mut dark_land = 0usize;
    let mut dark = (0usize, 0usize);
    for y in 0..wm { for x in 0..wm {
        let h = hgt[(y*2)*w + x*2] as i32;
        let water = h < sea || h == 0;
        let v = b0[y*wm+x];
        if v >= 250 { if water { bright.0 += 1 } else { bright.1 += 1 } }
        if v <= 2 {
            if water { dark.0 += 1 } else { dark.1 += 1; if (100..=120).contains(&(h/32)) { dark_land += 1 } }
        }
    } }
    println!("b0>=250 亮带: 水上 {} 陆上 {}（陆上占 {:.0}%）", bright.0, bright.1, bright.1 as f64 * 100.0 / (bright.0+bright.1).max(1) as f64);
    println!("b0<=2  暗线: 水上 {} 陆上 {}", dark.0, dark.1);
    // 亮带的海拔分布
    let mut el = Vec::new();
    for y in (0..wm).step_by(3) { for x in (0..wm).step_by(3) {
        if b0[y*wm+x] >= 250 { el.push(hgt[(y*2)*w+x*2] as i32); }
    } }
    el.sort();
    if !el.is_empty() {
        println!("亮带海拔: min={} p10={} 中位={} p90={} max={}", el[0], el[el.len()/10], el[el.len()/2], el[el.len()*9/10], el[el.len()-1]);
    }
}
