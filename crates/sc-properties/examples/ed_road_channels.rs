//! 探针：ED 场路网/桥梁/铁路通道语义深查——直方图 + 三通道灰度图 + 路网叠色图。仅开发用。
use sc_properties::region_map::{SHARED_ED_GRID, SHARED_TILE_GRID};

fn main() {
    let rt0 = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package").unwrap();
    let group = 0xBEAF_0510u32; // 白水谷
    let mut tiles: std::collections::HashMap<u32, Vec<u16>> = Default::default();
    for e in rt0.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            if let Ok(d) = rt0.read(e) {
                if d.len() == 131092 {
                    tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
                }
            }
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
            for y in 0..128 {
                for x in 0..128 {
                    let o = 20 + (y * 128 + x) * 4;
                    ed[(ty * 128 + y) * 2048 + tx * 128 + x] =
                        u32::from_le_bytes([d[o], d[o + 1], d[o + 2], d[o + 3]]);
                }
            }
        }
    }
    const SEA: i32 = 4928;
    for ch in 0..3u32 {
        let mut hist: [u32; 256] = [0; 256];
        for &v in ed.iter() {
            hist[((v >> (ch * 8)) & 0xFF) as usize] += 1;
        }
        let mut rows: Vec<(usize, u32)> = hist.iter().enumerate().filter(|p| *p.1 > 2000).map(|(v, n)| (v, *n)).collect();
        rows.sort_by(|a, b| b.1.cmp(&a.1));
        println!("b{ch} 直方图（>2000）: {:?}", &rows[..rows.len().min(12)]);
        let mut img = image::GrayImage::new(2048, 2048);
        for y in 0..2048 {
            for x in 0..2048 {
                let v = ed[y * 2048 + x];
                let b = ((v >> (ch * 8)) & 0xFF) as u8;
                img.put_pixel(x as u32, y as u32, image::Luma([b]));
            }
        }
        image::DynamicImage::ImageLuma8(img).save(format!("tmp/ed_ch{ch}.png")).unwrap();
    }
    for cand in [0u8, 15u8] {
        let mut n = 0u64;
        let mut below = 0u64;
        let mut h_sum = 0f64;
        for y in 0..2048 {
            for x in 0..2048 {
                if (ed[y * 2048 + x] & 0xFF) as u8 == cand {
                    let h = hgt[(y * 2) * 4096 + x * 2] as i32;
                    n += 1;
                    h_sum += f64::from(h);
                    if h < SEA { below += 1; }
                }
            }
        }
        if n > 0 {
            println!("b0=={cand}: {n} 格, 水下 {below} ({:.1}%), 平均 {:.0} m", below as f64 / n as f64 * 100.0, h_sum / n as f64 / 32.0 - 1024.0);
        }
    }
    let mut img = image::RgbImage::new(2048, 2048);
    for y in 0..2048 {
        for x in 0..2048 {
            let v = ed[y * 2048 + x];
            let b0 = (v & 0xFF) as u8;
            let h = hgt[(y * 2) * 4096 + x * 2] as i32;
            let water = h < SEA;
            let c = if b0 == 0 {
                [255, 60, 60]
            } else if b0 == 15 {
                if water { [255, 210, 0] } else { [255, 0, 255] }
            } else if water {
                [40, 60, 120]
            } else {
                let g = f32::from(((v >> 16) & 0xFF) as u8) / 255.0;
                [(60.0 + 120.0 * g) as u8, 55, 45]
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(c));
        }
    }
    image::DynamicImage::ImageRgb8(img).save("tmp/ed_road_channels.png").unwrap();
    println!("渲染 -> tmp/ed_road_channels.png + ed_ch0/1/2.png");
}
