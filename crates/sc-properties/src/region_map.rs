//! 区域地图渲染管线：341-tile 金字塔拼合 → 全局水位面着色 → PNG。
//! 供 OpenSCP 地图开发面板与探针共用。
//!
//! 机制结论（详见 docs/overview/glass-box/region-and-map.md §4/§5）：
//! - 每区域 341 张 256² u16 tile = 4096²×8m 区域高度图的 5 级 mip 金字塔，
//!   排布全游戏唯一（`SHARED_TILE_GRID`，RT0/RT1 槽位集合一致实证）；
//! - 水位面为全游戏统一常量 ≈3336 raw（三区域交叉实证）；
//! - ED（0x03E421ED，128² u32）为独立槽位的地面场金字塔：
//!   b0=植被密度+路网、b1=草量（草地着色）、b2=材质权重。
use std::collections::HashMap;

use image::{Rgb, RgbImage};

/// 全游戏共享的 mip0 tile 排布（由 `tile_arrange` 像素级金字塔匹配复原，
/// BEAF0510 完美树：单根 0x7ADCBE88，节点 [1,4,16,64,256]）。
pub const SHARED_TILE_GRID: [[u32; 16]; 16] = [
    [0x7ADCBE8C, 0x5D292B9B, 0xA7C1AE16, 0x1E279CE5, 0xA12CF3A0, 0x6854893F, 0xF92D745A, 0x5A72E609, 0xED4A03E4, 0x453A7813, 0x2103B675, 0x57D87926, 0x834E206B, 0x04BDD09C, 0x82FD4959, 0x9BE696EA],
    [0x4766E36B, 0xF9197D3C, 0x25152B55, 0x7BB03F26, 0x1C7CC92F, 0xC605CDF0, 0x410980D9, 0xABB3C34A, 0xE8112423, 0x4A192C94, 0xA3B03936, 0xFA4FD6E5, 0xB6C3FB8C, 0x52AF89FB, 0x3B213CDA, 0x315904A9],
    [0xAD4DAB7A, 0x435C67E9, 0xAB991ED8, 0xD2EF6037, 0x38CB831E, 0x4C37186D, 0xDA61A76C, 0x5E21B4BB, 0xADFF3472, 0x77AB6481, 0xA98C6847, 0xAF4678E8, 0x47307D79, 0xB98CBA8A, 0xD53C07CB, 0x9F0FEAFC],
    [0x0B494079, 0xADE8672A, 0xC3BEF927, 0xD31E3EE8, 0x51C249DD, 0x1AEB98AE, 0x93483F4B, 0xC82B5F5C, 0xE2133BD1, 0x466A5E62, 0x91668E78, 0xAF179A37, 0xE934E87A, 0x517B2449, 0x1C556FEC, 0x1C6DE15B],
    [0x710404B8, 0xC4AF6B77, 0xEB0FF21A, 0x6D637AA9, 0x73BBA1B4, 0xB7E1AB83, 0x89A1B256, 0x0B370845, 0x0C566190, 0x3AE0830F, 0x2BAD5D39, 0xB41946EA, 0xC78B8487, 0xB4BB7F88, 0x48E8F815, 0x3FA5C926],
    [0x8BA44787, 0xA9159928, 0x45DE5A99, 0xD7F10CEA, 0x3F70AFB3, 0xA3CC4344, 0x06F52F95, 0x551A8B06, 0xC4C4A55F, 0xBD164480, 0xE6FCEA3A, 0x498BB4A9, 0xACEB41B8, 0xE6734757, 0xCB957AD6, 0xE21D26E5],
    [0x59890776, 0x3C252625, 0xCEBE8E2C, 0x5878575B, 0x59ED4A82, 0x8275F331, 0x8AFD2718, 0xAC59AC17, 0x780E784E, 0x2A9A3E3D, 0x8117B52B, 0xB7429AFC, 0x1047C5B5, 0x8F342646, 0x00DDE7E7, 0x9713C8E8],
    [0xD46088B5, 0x6D6EB766, 0x6BDC750B, 0xDB1A60FC, 0x5BC213A1, 0x67AEA052, 0xBEEA1F67, 0xA6E0F548, 0x9287AC0D, 0x3E954BFE, 0xDE50A54C, 0x34A0915B, 0x95704476, 0x2E7CC485, 0xCCF0EF98, 0x96E4EA37],
    [0xA6040CC4, 0xA1677C33, 0x6E8AC84E, 0x9834631D, 0x471ECCA8, 0x896C7E07, 0x9FA6AAB2, 0xBEB168C1, 0xD8423E2C, 0xEADE327B, 0x9B107CAD, 0x58EB6D5E, 0xF600B203, 0xB1EA8414, 0xF9E8EF91, 0x9093FEC2],
    [0xBA197503, 0xD5B26E34, 0x8903FC0D, 0x7CC3335E, 0x62B89EF7, 0x6ECC3B38, 0xB7F52711, 0xB9AF73A2, 0x7560250B, 0x9CEC791C, 0x8098DBEE, 0x745C9D1D, 0xE1EB49C4, 0xAD0BCF93, 0xE19A7332, 0xABB57CE1],
    [0x02C34549, 0xB0401F16, 0x54AABA9B, 0x96187398, 0xBAAD9CED, 0xDD7B162A, 0x9D5F5BEF, 0xD28C5F4C, 0xBEBE1F21, 0x0409E9AE, 0x40CFF468, 0xEF8E39AB, 0xDB206CE6, 0x2521DD59, 0x62993C9C, 0x0D7354FF],
    [0x6AD4DB8A, 0x2D939C55, 0xF09B0C3C, 0xCA056BE7, 0x89621D2E, 0x72EF16E9, 0x1F96B060, 0x75536F2B, 0xBCE95602, 0x3555696D, 0x40A115B7, 0x4CC729CC, 0xA9D6DBA5, 0xDD45D0DA, 0xB08AF5FB, 0x5830AAB0],
    [0x03F7AAFB, 0xB4178FD8, 0x3ADDF6E9, 0x94BCFED6, 0xD6CB0DBF, 0x28AC2C3C, 0x8D19169D, 0xDB38A43A, 0xA26CBB33, 0xAF241EF0, 0xE961F4A6, 0x9A23E1B9, 0x16C74EA8, 0x77609BCB, 0x1768268A, 0xF154512D],
    [0xB605F19C, 0xCC3D6A27, 0xA569F62A, 0x12107C15, 0x347C5270, 0x8CBBDA9B, 0x71A7E6DE, 0x1FE91739, 0xD6B7AD34, 0x5172DA3F, 0x8BD95265, 0x55736EBA, 0x326120F7, 0xBE7A03EC, 0xAF569049, 0xD6DCB06E],
    [0xDFC4E585, 0xF38E631A, 0xBC30FA77, 0xE57CF3EC, 0xF0EC77B1, 0x9D016666, 0xBDD8D973, 0x85A24878, 0xB4BE555D, 0x50CCF152, 0x48CC167C, 0x1802ECC7, 0x1B9A1CAA, 0xEB0D8C15, 0x1296EB88, 0x2E456A43],
    [0x407C4746, 0x4E5CCB99, 0xA0972828, 0x9E638BCB, 0xD62524D2, 0x6BB7D525, 0xAC980574, 0x9DC82247, 0x9BC78E9E, 0x6B944431, 0xC62A0CDB, 0xFFDD12F8, 0xB10E1D69, 0x6DBA0ED6, 0x444EB357, 0x35F8B304],
];

/// 全游戏统一水位面（raw 高度，经验值，三区域交叉实证）。
pub const WATER_LEVEL: i32 = 3336;
const HALF: f32 = 16384.0;
const CELL: f32 = 8.0;

/// 区域摘要。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegionSummary {
    pub group: u32,
    /// 区域描述内的数字串（= 区域 UI id；组 id = FNV1(数字串)）。
    pub numeric_id: String,
    /// 城市地块数（不含伟大工程位）。
    pub plot_count: usize,
}

/// 渲染输出。
#[derive(Debug, Clone, serde::Serialize)]
pub struct RegionRender {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// 裁剪框的世界坐标（左上角）。
    pub origin_world: (f32, f32),
    /// 每米对应的像素数（恒 1/8）。
    pub meters_per_pixel: f32,
    pub water_plane: i32,
    pub desert: bool,
    pub plots: Vec<(f32, f32)>,
    /// 资源画刷：目标 map 名 + 各 stamp 世界坐标。
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
}

fn fnv1(s: &str) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in s.bytes() {
        h = h.wrapping_mul(0x0100_0193);
        h ^= b as u32;
    }
    h
}

/// 枚举包内全部区域（F0 tile 计数 = 341 的组），附 UI id 与城市地块数。
pub fn list_regions(package: &dbpf::Package) -> Vec<RegionSummary> {
    let mut f0cnt: HashMap<u32, usize> = HashMap::new();
    let mut numeric: HashMap<u32, String> = HashMap::new();
    let mut region_cities: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in package.entries() {
        match e.id.type_id {
            0x03E4_21F0 => {
                *f0cnt.entry(e.id.group).or_default() += 1;
            }
            0x00B1_B104 => {
                if let Some(pf) = sc_parse(package, e) {
                    // 区域描述：数字 UI id
                    if e.id.instance == 0x51E7_A18D {
                        if let Some(p) = pf.get(0x4EE9_7F5B) {
                            if let crate::Kind::Scalar(crate::Value::String8(s)) = &p.kind {
                                numeric.insert(e.id.group, s.clone());
                            }
                        }
                    }
                    // 城市组：36B property 显式引用区域 desc
                    if e.compressed_size <= 60 {
                        if let Some(crate::Property { kind: crate::Kind::Scalar(crate::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
                            if k.instance == 0x51E7_A18D {
                                region_cities.entry(k.group).or_default().push(e.id.group);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (g, n) in f0cnt {
        if n != 341 {
            continue;
        }
        let plot_count = region_cities.get(&g).map(|c| c.len()).unwrap_or(0);
        let nid = numeric.get(&g).cloned().unwrap_or_default();
        out.push(RegionSummary { group: g, numeric_id: nid, plot_count });
    }
    out.sort_by_key(|r| r.numeric_id.clone());
    out
}

fn sc_parse(package: &dbpf::Package, e: &dbpf::IndexEntry) -> Option<crate::PropertyFile> {
    let data = package.read(e).ok()?;
    crate::PropertyFile::parse(&data).ok()
}

/// 区域地块位置（通过城市 id 交集选择正确的地块表）。
pub fn plot_positions(package: &dbpf::Package, group: u32) -> Vec<(f32, f32)> {
    let mut city_ids: Vec<u32> = Vec::new();
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 {
            continue;
        }
        if let Some(pf) = sc_parse(package, e) {
            if let Some(crate::Property { kind: crate::Kind::Scalar(crate::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
                if k.instance == 0x51E7_A18D && k.group == group {
                    city_ids.push(e.id.group);
                }
            }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    let mut best: Option<(usize, Vec<(f32, f32)>)> = None;
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C {
            continue;
        }
        let Some(pt) = sc_parse(package, e) else { continue };
        let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pt.get(0x16B7_B1EF) else { continue };
        let ids: Vec<u32> = vs.iter().filter_map(|v| match v { crate::Value::UInt32(x) => Some(*x), _ => None }).collect();
        let ov = ids.iter().filter(|i| set.contains(i)).count();
        if best.as_ref().map(|(b, _)| ov > *b).unwrap_or(true) {
            if let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) {
                let pos: Vec<(f32, f32)> = vs
                    .iter()
                    .filter_map(|v| match v { crate::Value::Vector2(v) => Some((v[0], v[1])), _ => None })
                    .collect();
                best = Some((ov, pos));
            }
        }
    }
    best.map(|(_, p)| p).unwrap_or_default()
}

/// 资源画刷清单（目标 map 名 + stamp 世界坐标）。
pub fn resource_brushes(package: &dbpf::Package, group: u32) -> Vec<(String, Vec<(f32, f32)>)> {
    let mut out = Vec::new();
    for e in package.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != group {
            continue;
        }
        let Some(pf) = sc_parse(package, e) else { continue };
        let Some(p) = pf.get(0x00B2_CCCA) else { continue };
        let crate::Kind::Scalar(crate::Value::String8(name)) = &p.kind else { continue };
        if !(name.ends_with("brushes") || name.ends_with("Brushes")) || name == "brushes" {
            continue;
        }
        let mut stamps = Vec::new();
        if let Some(crate::Property { kind: crate::Kind::Array(vs), .. }) = pf.get(0x02A9_07B6) {
            for v in vs {
                if let crate::Value::Transform(t) = v {
                    if t.matrix.len() >= 11 {
                        stamps.push((t.matrix[9], t.matrix[10]));
                    }
                }
            }
        }
        out.push((name.clone(), stamps));
    }
    out
}

/// 渲染区域彩色俯视图（PNG）。`ed_grid` 为该区域 ED 金字塔网格
///（可用 [`ed_grid_for_region`] 求得并缓存；为 None 时按无植被渲染）。
pub fn render_region_png(
    package: &dbpf::Package,
    group: u32,
    ed_grid: Option<&[[u32; 16]; 16]>,
    water_override: Option<i32>,
) -> Result<RegionRender, Box<dyn std::error::Error>> {
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = package.read(e)?;
            if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }
        }
    }
    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..256 {
                    hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256]
                        .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    // ED 场（草量）
    let mut ed0 = vec![0u8; w * w];
    let mut ed1 = vec![0u8; w * w];
    if let Some(eg) = ed_grid {
        let mut tmp0 = vec![0u8; w * w];
        let mut tmp1 = vec![0u8; w * w];
        for (ty, row) in eg.iter().enumerate() {
            for (tx, inst) in row.iter().enumerate() {
                let Some(px) = ed_tile(package, group, *inst) else { continue };
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        let k = (ty * 128 + y) * 2048 + tx * 128 + x;
                        tmp0[k] = (v & 0xFF) as u8;
                        tmp1[k] = ((v >> 8) & 0xFF) as u8;
                    }
                }
            }
        }
        // 最近邻上采样 ×2
        for y in 0..w { for x in 0..w {
            let k = (y / 2) * 2048 + (x / 2);
            ed0[y * w + x] = tmp0[k];
            ed1[y * w + x] = tmp1[k];
        }}
    }
    let _ = &mut ed0;

    let plots = plot_positions(package, group);
    let brushes = resource_brushes(package, group);
    let sea = WATER_LEVEL.max(water_override.unwrap_or(WATER_LEVEL));

    // 荒漠判定：land 均值 b1
    let mut b1_sum = 0f64;
    let mut b1_n = 0f64;
    for y in (0..w).step_by(4) {
        for x in (0..w).step_by(4) {
            let h = hgt[y * w + x];
            if h > (sea + 100) as u16 && h != 0 {
                b1_sum += f64::from(ed1[y * w + x]);
                b1_n += 1.0;
            }
        }
    }
    let mean_b1 = if b1_n > 0.0 { b1_sum / b1_n } else { 0.0 };
    let desert = mean_b1 < 20.0;

    // 裁剪：地块包围盒 + 2560m
    let mut x0c = w; let mut x1c = 0usize; let mut y0c = w; let mut y1c = 0usize;
    if !plots.is_empty() {
        for (wx, wy) in &plots {
            let cx = ((wx + HALF) / CELL) as usize;
            let cy = ((wy + HALF) / CELL) as usize;
            x0c = x0c.min(cx.saturating_sub(320));
            x1c = x1c.max(cx + 320);
            y0c = y0c.min(cy.saturating_sub(320));
            y1c = y1c.max(cy + 320);
        }
        x0c = x0c.saturating_sub(320);
        x1c = (x1c + 320).min(w);
        y0c = y0c.saturating_sub(320);
        y1c = (y1c + 320).min(w);
    } else {
        x0c = 0; x1c = w; y0c = 0; y1c = w;
    }

    let h_at = |x: usize, y: usize| -> i32 { hgt[y.min(w - 1) * w + x.min(w - 1)] as i32 };
    let mut img = RgbImage::new((x1c - x0c) as u32, (y1c - y0c) as u32);
    for y in y0c..y1c {
        for x in x0c..x1c {
            let h = h_at(x, y);
            let dx = h_at(x + 1, y) - h;
            let dy = h_at(x, y + 1) - h;
            let (nx, ny, nz) = (-(dx as f32), -(dy as f32), 60.0f32);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let light = ((nx * 0.5 + ny * 0.5 + nz * 0.7) / nl).max(0.0);
            let rel = h - sea;
            let (mut r, mut g, mut b);
            if rel < 0 || h == 0 {
                let d = (-rel as f32 / 600.0).clamp(0.0, 1.0);
                r = (90.0 - 50.0 * d) * (0.6 + 0.4 * light);
                g = (140.0 - 60.0 * d) * (0.6 + 0.4 * light);
                b = (190.0 - 60.0 * d) * (0.6 + 0.4 * light);
            } else {
                let grass = ed1[y * w + x];
                let grass_k = if desert { 0.0 } else { (grass as f32 / 120.0).clamp(0.0, 1.0) };
                let slope = ((dx * dx + dy * dy) as f32).sqrt() / 60.0;
                let slope_k = (slope / 1.6).clamp(0.0, 1.0);
                let g_amt = (grass_k * (1.0 - slope_k * 0.85)).clamp(0.0, 1.0);
                if rel < 350 {
                    r = 190.0; g = 175.0; b = 130.0;
                } else {
                    let t = ((rel - 350) as f32 / 6000.0).clamp(0.0, 1.0);
                    let rock_r = 150.0 + 50.0 * t;
                    let rock_g = 135.0 + 25.0 * t;
                    let rock_b = 100.0 + 35.0 * t;
                    r = rock_r + (95.0 - rock_r) * g_amt;
                    g = rock_g + (160.0 - rock_g) * g_amt;
                    b = rock_b + (70.0 - rock_b) * g_amt;
                }
                if !desert && (100..=120).contains(&ed0[y * w + x]) {
                    let d = 1.0 - (ed0[y * w + x] as f32 - 100.0) / 20.0;
                    r *= 1.0 - 0.5 * d; g *= 1.0 - 0.1 * d; b *= 1.0 - 0.5 * d;
                }
                if ed0[y * w + x] == 15 { r = 130.0; g = 122.0; b = 112.0; }
            }
            let shade = 0.45 + 0.55 * light;
            r *= shade; g *= shade; b *= shade;
            img.put_pixel((x - x0c) as u32, (y - y0c) as u32, Rgb([r as u8, g as u8, b as u8]));
        }
    }

    // 地块框（黄）+ 资源环
    let wpx = |wx: f32, wy: f32| -> (isize, isize) {
        (
            ((wx + HALF) / CELL) as isize - x0c as isize,
            ((wy + HALF) / CELL) as isize - y0c as isize,
        )
    };
    let iw = (x1c - x0c) as isize;
    let ih = (y1c - y0c) as isize;
    let palette: [(&str, [u8; 3]); 8] = [
        ("coal", [40, 40, 40]), ("oil", [20, 20, 20]), ("ore", [200, 150, 0]),
        ("watertable", [0, 120, 255]), ("radiation", [0, 220, 60]),
        ("soil", [150, 100, 50]), ("forest", [0, 150, 0]), ("groundpollution", [160, 60, 200]),
    ];
    for (name, stamps) in &brushes {
        let c = palette.iter().find(|(n, _)| name.starts_with(n)).map(|(_, c)| *c).unwrap_or([255, 0, 255]);
        for (wx, wy) in stamps {
            let (cx, cy) = wpx(*wx, *wy);
            for a in 0..360 {
                let (dx, dy) = ((a as f64).to_radians().cos(), (a as f64).to_radians().sin());
                for t in [128isize, 130] {
                    let px = cx + (dx * t as f64) as isize;
                    let py = cy + (dy * t as f64) as isize;
                    if px >= 0 && py >= 0 && px < iw && py < ih {
                        img.put_pixel(px as u32, py as u32, Rgb(c));
                    }
                }
            }
        }
    }
    for (wx, wy) in &plots {
        let (cx, cy) = wpx(*wx, *wy);
        let (x0, y0) = (cx - 128, cy - 128);
        for t in 0..256isize {
            for &(xx, yy) in &[(x0 + t, y0), (x0 + t, y0 + 255), (x0, y0 + t), (x0 + 255, y0 + t)] {
                if xx >= 0 && yy >= 0 && xx < iw && yy < ih {
                    img.put_pixel(xx as u32, yy as u32, Rgb([255, 210, 0]));
                }
            }
        }
    }

    let mut png = Vec::new();
    image::DynamicImage::ImageRgb8(img).write_to(
        &mut std::io::Cursor::new(&mut png),
        image::ImageFormat::Png,
    )?;
    let origin_world = (x0c as f32 * CELL - HALF, y0c as f32 * CELL - HALF);
    Ok(RegionRender {
        png,
        width: (x1c - x0c) as u32,
        height: (y1c - y0c) as u32,
        origin_world,
        meters_per_pixel: 8.0,
        water_plane: sea,
        desert,
        plots,
        brushes,
    })
}

fn ed_tile(package: &dbpf::Package, group: u32, inst: u32) -> Option<Vec<u32>> {
    let e = package.entries().iter().find(|e| e.id.type_id == 0x03E4_21ED && e.id.group == group && e.id.instance == inst)?;
    let d = package.read(e).ok()?;
    if d.len() != 65556 {
        return None;
    }
    Some(d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect())
}

/// ED 金字塔网格求解（该区域 ED 槽位独立，需单独匹配；结果可缓存）。
pub fn ed_grid_for_region(package: &dbpf::Package, group: u32) -> Option<[[u32; 16]; 16]> {
    let mut tiles: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let d = package.read(e).ok()?;
            if d.len() == 65556 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
            }
        }
    }
    if tiles.len() != 341 {
        return None;
    }
    let insts: Vec<u32> = tiles.keys().copied().collect();
    let mut ds: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for (&i, px) in &tiles {
        let mut b = vec![(0u32, 0u32); 64 * 64];
        for y in 0..64 {
            for x in 0..64 {
                let k = (2 * y) * 128 + 2 * x;
                let lane = |v: u32, s: u32| ((v >> (s * 8)) & 0xFF) as u32;
                b[y * 64 + x] = (
                    (lane(px[k], 0) + lane(px[k + 1], 0) + lane(px[k + 128], 0) + lane(px[k + 129], 0)) / 4,
                    (lane(px[k], 2) + lane(px[k + 1], 2) + lane(px[k + 128], 2) + lane(px[k + 129], 2)) / 4,
                );
            }
        }
        ds.insert(i, b);
    }
    let mut assign: HashMap<u32, (u32, usize, f64)> = HashMap::new();
    for &child in &insts {
        let cb = &ds[&child];
        let mut best = (0u32, 0usize, f64::INFINITY);
        for &parent in &insts {
            if parent == child { continue; }
            let pp = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 64, (q / 2) * 64);
                let mut e = 0f64;
                for y in 0..64 {
                    let prow = (qy + y) * 128 + qx;
                    let crow = y * 64;
                    for x in 0..64 {
                        let pv = pp[prow + x];
                        let ce = cb[crow + x];
                        e += (f64::from((pv & 0xFF) as u8) - f64::from(ce.0 as u8)).abs()
                            + (f64::from(((pv >> 16) & 0xFF) as u8) - f64::from(ce.1 as u8)).abs();
                    }
                }
                e /= 4096.0;
                if e < best.2 { best = (parent, q, e); }
            }
        }
        assign.insert(child, best);
    }
    let mut ch: Vec<(u32, u32, usize, f64)> = assign.iter().map(|(c, (p, q, e))| (*c, *p, *q, *e)).collect();
    ch.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let mut tree: HashMap<u32, [Option<u32>; 4]> = HashMap::new();
    let mut used_c = std::collections::HashSet::new();
    let mut used_q = std::collections::HashSet::new();
    for (c, p, q, e) in &ch {
        if used_c.contains(c) || used_q.contains(&(*p, *q)) || *e > 60.0 { continue; }
        tree.entry(*p).or_default()[*q] = Some(*c);
        used_c.insert(*c);
        used_q.insert((*p, *q));
    }
    let cof: HashMap<u32, (u32, usize)> = used_c
        .iter()
        .map(|c| {
            let (p, q, _) = assign[c];
            (*c, (p, q))
        })
        .collect();
    let isp: std::collections::HashSet<u32> = tree.keys().copied().collect();
    let roots: Vec<u32> = isp.iter().copied().filter(|x| !cof.contains_key(x)).collect();
    let root = *roots.first()?;
    let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
    pos.insert(root, (0, 0, 0));
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let (x, y, l) = pos[&n];
        if let Some(sl) = tree.get(&n) {
            for (q, c) in sl.iter().enumerate() {
                if let Some(c) = c {
                    pos.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, l + 1));
                    stack.push(*c);
                }
            }
        }
    }
    let mut grid = [[0u32; 16]; 16];
    for (i, (x, y, l)) in &pos {
        if *l == 4 { grid[*y][*x] = *i; }
    }
    Some(grid)
}
