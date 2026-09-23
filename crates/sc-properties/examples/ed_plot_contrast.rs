//! 探针：ED 相对帧判定——地块框内外的 b0/b1/b2 对比度。
//! 依据：区域作者在城市地块内预刷草地/清除植被（B12DE348 实证），
//! 正确帧应使框内/框外 ED 通道均值差异最大化。仅开发用。
use std::collections::HashMap;

use dbpf::Package;
use sc_properties::region_map::{ed_grid_for_region, SHARED_TILE_GRID};

fn plot_positions(p: &dbpf::Package, region: u32) -> Vec<(f32, f32)> {
    let mut city_ids: Vec<u32> = Vec::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        let Ok(data) = p.read(e) else { continue };
        let Ok(pf) = sc_properties::PropertyFile::parse(&data) else { continue };
        if let Some(sc_properties::Property {
            kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)),
            ..
        }) = pf.get(0xC194_9C4D)
        {
            if k.instance == 0x51E7_A18D && k.group == region {
                city_ids.push(e.id.group);
            }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    let mut best: Option<(usize, Vec<(f32, f32)>)> = None;
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C {
            continue;
        }
        let Ok(data) = p.read(e) else { continue };
        let Ok(pt) = sc_properties::PropertyFile::parse(&data) else { continue };
        let ids: Vec<u32> = match pt.get(0x16B7_B1EF) {
            Some(sc_properties::Property {
                kind: sc_properties::Kind::Array(vs),
                ..
            }) => vs.iter().filter_map(|v| match v {
                sc_properties::Value::UInt32(x) => Some(*x),
                _ => None,
            }).collect(),
            _ => continue,
        };
        let ov = ids.iter().filter(|i| set.contains(i)).count();
        if best.as_ref().map(|(b, _)| ov > *b).unwrap_or(true) {
            if let Some(sc_properties::Property {
                kind: sc_properties::Kind::Array(vs),
                ..
            }) = pt.get(0xF01D_E4B1)
            {
                let pos: Vec<(f32, f32)> = vs.iter().filter_map(|v| match v {
                    sc_properties::Value::Vector2(v) => Some((v[0], v[1])),
                    _ => None,
                }).collect();
                best = Some((ov, pos));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open");
    for group in [0xBEAF_0510u32, 0xD01F_A985, 0xB12D_E348] {
        let Some(grid) = ed_grid_for_region(&p, group) else {
            println!("{group:08X}: ED 网格失败，跳过");
            continue;
        };
        let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
        let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
        for e in p.entries() {
            if e.id.group != group {
                continue;
            }
            match e.id.type_id {
                0x03E4_21F0 => {
                    if let Ok(d) = p.read(e) {
                        if d.len() == 131092 {
                            f0.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
                        }
                    }
                }
                0x03E4_21ED => {
                    if let Ok(d) = p.read(e) {
                        if d.len() == 65556 {
                            ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
                        }
                    }
                }
                _ => {}
            }
        }
        let w = 4096usize;
        let mut hgt = vec![0u16; w * w];
        for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
            for (tx, inst) in row.iter().enumerate() {
                if let Some(px) = f0.get(inst) {
                    for y in 0..256 {
                        hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256]
                            .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                    }
                }
            }
        }
        let wm = 2048usize;
        let mut chans = vec![[0u8; 3]; wm * wm];
        for (ty, row) in grid.iter().enumerate() {
            for (tx, inst) in row.iter().enumerate() {
                let Some(px) = ed.get(inst) else { continue };
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        chans[(ty * 128 + y) * wm + tx * 128 + x] =
                            [(v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8];
                    }
                }
            }
        }
        let plots = plot_positions(&p, group);
        println!("\n== {group:08X} plots={} ==", plots.len());
        let half = 16384.0f32;
        let cell = 8.0f32;
        for fx in [false, true] {
            for fy in [false, true] {
                let mut inside = [0f64; 3];
                let mut outside = [0f64; 3];
                let mut nin = 0f64;
                let mut nout = 0f64;
                // 逐地块统计框内（±128 cell）与框外
                let mut inbox = vec![false; wm * wm];
                for (wx, wy) in &plots {
                    let cx = ((wx + half) / cell) as isize;
                    let cy = ((wy + half) / cell) as isize;
                    for y in (cy - 128).max(0)..=(cy + 127).min(4095) {
                        for x in (cx - 128).max(0)..=(cx + 127).min(4095) {
                            let yy = (y as usize) / 2;
                            let xx = (x as usize) / 2;
                            inbox[yy * wm + xx] = true;
                        }
                    }
                }
                for y in 0..wm {
                    for x in 0..wm {
                        let mx = if fx { wm - 1 - x } else { x };
                        let my = if fy { wm - 1 - y } else { y };
                        let h = hgt[(my * 2) * w + mx * 2];
                        if h == 0 {
                            continue; // void 无意义
                        }
                        let c = chans[y * wm + x];
                        if inbox[y * wm + x] {
                            for ch in 0..3 {
                                inside[ch] += f64::from(c[ch]);
                            }
                            nin += 1.0;
                        } else {
                            for ch in 0..3 {
                                outside[ch] += f64::from(c[ch]);
                            }
                            nout += 1.0;
                        }
                    }
                }
                let d0 = inside[0] / nin - outside[0] / nout;
                let d1 = inside[1] / nin - outside[1] / nout;
                let d2 = inside[2] / nin - outside[2] / nout;
                println!(
                    "帧 fx={fx} fy={fy}: Δb0={d0:+7.2} Δb1={d1:+7.2} Δb2={d2:+7.2}  |b1|={:.2} (框内 n={nin})",
                    inside[1] / nin
                );
            }
        }
    }
}
