//! 探针：白水谷中央区 b0 低值放大渲染（区分公路/铁路线型）。仅开发用。
use sc_properties::region_map::{SHARED_ED_GRID, SHARED_TILE_GRID};

fn main() {
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let group = 0xBEAF_0510u32;
    let mut tiles: std::collections::HashMap<u32, Vec<u16>> = Default::default();
    for e in rt0.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            if let Ok(d) = rt0.read(e) { if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }}
        }
    }
    let mut hgt = vec![0u16; 4096 * 4096];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..256 {
                    hgt[(ty * 256 + y) * 4096 + tx * 256..(ty * 256 + y) * 4096 + tx * 256 + 256]
                        .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    let mut ed = vec![0u32; 2048 * 2048];
    for (ty, row) in SHARED_ED_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let Some(e) = rt0.entries().iter().find(|e| e.id.type_id == 0x03E4_21ED && e.id.group == group && e.id.instance == *inst) else { continue };
            let d = rt0.read(e).unwrap();
            for y in 0..128 { for x in 0..128 {
                let o = 20 + (y * 128 + x) * 4;
                ed[(ty * 128 + y) * 2048 + tx * 128 + x] = u32::from_le_bytes([d[o], d[o+1], d[o+2], d[o+3]]);
            }}
        }
    }
    // 中央可玩区 ≈ ±8192m → ED 格 512..1536；放大 3× 渲染
    const X0: usize = 512; const Y0: usize = 512; const N: usize = 1024;
    let mut img = image::RgbImage::new((N * 2) as u32, (N * 2) as u32);
    for y in 0..N { for x in 0..N {
        let v = ed[(Y0 + y) * 2048 + X0 + x];
        let b0 = (v & 0xFF) as u8;
        let h = hgt[((Y0 + y) * 2) * 4096 + (X0 + x) * 2] as i32;
        let water = h < 4928;
        // 陆地基色按 b1（草）绿标 + b2 暗度
        let b1 = ((v >> 8) & 0xFF) as u8;
        let b2 = ((v >> 16) & 0xFF) as u8;
        let mut c = if water {
            [40u8, 60, 120]
        } else {
            let grass = (b1.min(64) as f32 / 64.0 * 160.0) as u8;
            let dark = b2 as f32 / 255.0;
            [(40.0 + 100.0 * dark) as u8, (60 + grass as u16).min(200) as u8, 40]
        };
        // b0 低值分色：0=红 1..7=彩虹 11..16=浅蓝
        if b0 <= 7 && b0 != 0 { c = [0, (60 + b0 * 30) as u8, 255]; }
        if b0 == 0 { c = [255, 40, 40]; }
        for dy in 0..2 { for dx in 0..2 {
            img.put_pixel((x * 2 + dx) as u32, (y * 2 + dy) as u32, image::Rgb(c));
        }}
    }}
    image::DynamicImage::ImageRgb8(img).save("tmp/ed_road_zoom.png").unwrap();
    println!("-> tmp/ed_road_zoom.png");
}
