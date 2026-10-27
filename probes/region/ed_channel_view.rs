//! 探针：ED b0/b1/b2 通道灰度渲染 + b0 值直方图（找路网/铁路编码）。仅开发用。
use std::collections::HashMap;
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let group = 0xBEAF_0510u32;
    let grid = &sc_properties::region_map::SHARED_ED_GRID;
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            if let Ok(d) = p.read(e) {
                if d.len() == 65556 {
                    ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
                }
            }
        }
    }
    let wm = 2048usize;
    let mut ch = [vec![0u8; wm * wm], vec![0u8; wm * wm], vec![0u8; wm * wm]];
    let mut hist: HashMap<u8, usize> = HashMap::new();
    for (ty, row) in grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let Some(px) = ed.get(inst) else { continue };
            for y in 0..128 {
                for x in 0..128 {
                    let v = px[y * 128 + x];
                    let k = (ty * 128 + y) * wm + tx * 128 + x;
                    ch[0][k] = (v & 0xFF) as u8;
                    ch[1][k] = ((v >> 8) & 0xFF) as u8;
                    ch[2][k] = ((v >> 16) & 0xFF) as u8;
                    *hist.entry((v & 0xFF) as u8).or_default() += 1;
                }
            }
        }
    }
    for (name, data) in ["b0_veg_road", "b1_grass", "b2_material"].iter().zip(ch.iter()) {
        let img = image::GrayImage::from_fn(wm as u32, wm as u32, |x, y| image::Luma([data[y as usize * wm + x as usize]]));
        img.save(format!("tmp/region_preview/ed_{name}_BEAF0510.png")).unwrap();
        println!("-> ed_{name}_BEAF0510.png");
    }
    let mut hv: Vec<(u8, usize)> = hist.into_iter().collect();
    hv.sort_by(|a, b| b.1.cmp(&a.1));
    println!("b0 top20 值: {:?}", &hv[..20.min(hv.len())]);
}
