//! 探针：b0 路网判读叠合图：水=蓝 b0>=250=白 b0<=2=红 其余=灰。仅开发用。
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
    let mut img = image::RgbImage::new(wm as u32, wm as u32);
    for y in 0..wm { for x in 0..wm {
        let h = hgt[(y*2)*w + x*2] as i32;
        let v = b0[y*wm+x];
        let c = if h < sea || h == 0 {
            [60, 90, 160]
        } else if v >= 250 {
            [255, 255, 255]
        } else if v <= 2 {
            [220, 40, 40]
        } else {
            [v, v, v]
        };
        img.put_pixel(x as u32, y as u32, image::Rgb(c));
    } }
    img.save("tmp/region_preview/ed_b0_overlay_BEAF0510.png").unwrap();
    println!("-> ed_b0_overlay_BEAF0510.png");
}
