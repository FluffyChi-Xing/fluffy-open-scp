//! 探针：ED 金字塔网格硬编码提取 + ED 相对帧朝向判定（草量 b1 × 平坦度相关性）。
//! 仅开发用。输出：
//!   1. 9F735B20 ED 条目逐条读取诊断（定位 0ms 失败根因）；
//!   2. BEAF0510 求解的 16×16 ED mip0 网格文本（供硬编码进 region_map.rs）；
//!   3. D01FA985 四种帧假设（identity/flip-x/flip-y/rot180）的 b1×平坦度 Pearson 相关；
//!   4. D01FA985 / B12DE348 按 render_region_png 管线的全幅 PNG（按最优帧）。
use std::collections::HashMap;

use image::{Rgb, RgbImage};
use sc_properties::region_map::{ed_grid_for_region, WATER_LEVEL, SHARED_TILE_GRID};

const HALF: f32 = 16384.0;
const CELL: f32 = 8.0;

fn main() {
    let path = "D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package";
    let p = dbpf::Package::open(path).expect("open");

    // ---- 1. 9F735B20 ED 读取诊断 ----
    for group in [0x9F73_5B20u32, 0xB12D_E348] {
        let mut fail = 0usize;
        let mut first_err = String::new();
        let mut n = 0usize;
        for e in p.entries() {
            if e.id.type_id == 0x03E4_21ED && e.id.group == group {
                n += 1;
                if let Err(err) = p.read(e) {
                    fail += 1;
                    if first_err.is_empty() {
                        first_err = format!("inst={:08X}: {err}", e.id.instance);
                    }
                }
            }
        }
        println!("{group:08X}: ED 条目 {n}，读取失败 {fail}  首个错误: {first_err}");
    }

    // ---- 2. BEAF0510 ED 网格文本（硬编码素材）----
    let grid = ed_grid_for_region(&p, 0xBEAF_0510).expect("BEAF0510 grid");
    println!("\npub const SHARED_ED_GRID: [[u32; 16]; 16] = [");
    for row in &grid {
        let cells: Vec<String> = row.iter().map(|v| format!("0x{v:08X}")).collect();
        println!("    [{}],", cells.join(", "));
    }
    println!("];");

    // ---- 3. 帧假设相关性（D01FA985 绿区，信号最强）----
    let group = 0xD01F_A985u32;
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
    let mut b1 = vec![0u8; wm * wm];
    for (ty, row) in grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            let Some(px) = ed.get(inst) else { continue };
            for y in 0..128 {
                for x in 0..128 {
                    b1[(ty * 128 + y) * wm + tx * 128 + x] = (px[y * 128 + x] >> 8) as u8;
                }
            }
        }
    }
    let mut results: Vec<(f64, bool, bool)> = Vec::new();
    for fx in [false, true] {
        for fy in [false, true] {
            let (mut sx, mut sy, mut sxx, mut syy, mut sxy, mut n) = (0f64, 0f64, 0f64, 0f64, 0f64, 0f64);
            for y in 1..wm - 1 {
                for x in 1..wm - 1 {
                    let mx = if fx { wm - 1 - x } else { x };
                    let my = if fy { wm - 1 - y } else { y };
                    let h = hgt[(my * 2) * w + mx * 2] as i32;
                    if h <= WATER_LEVEL + 100 || h == 0 {
                        continue;
                    }
                    // 平坦度：8m 邻域坡度（用全分辨率高度）
                    let hx = hgt[(my * 2) * w + (mx * 2 + 1).min(w - 1)] as i32;
                    let hy = hgt[((my * 2 + 1).min(w - 1)) * w + mx * 2] as i32;
                    let slope = ((hx - h).abs() + (hy - h).abs()) as f64;
                    let flat = (1.0 - slope / 200.0).clamp(0.0, 1.0);
                    let g = f64::from(b1[y * wm + x]);
                    sx += flat;
                    sy += g;
                    sxx += flat * flat;
                    syy += g * g;
                    sxy += flat * g;
                    n += 1.0;
                }
            }
            let cov = sxy / n - (sx / n) * (sy / n);
            let r = cov / ((sxx / n - (sx / n).powi(2)).sqrt() * (syy / n - (sy / n).powi(2)).sqrt());
            println!("帧 fx={fx} fy={fy}: corr(b1, flat) = {r:.4}  (n={n})");
            results.push((r, fx, fy));
        }
    }
    results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let (best_r, bfx, bfy) = results[0];
    println!("最优帧: fx={bfx} fy={bfy} (r={best_r:.4})\n");

    // ---- 4. 全幅渲染（按最优帧）----
    std::fs::create_dir_all("tmp/region_preview").unwrap();
    for group in [0xD01F_A985u32, 0xB12D_E348] {
        render_full(&p, group, &grid, bfx, bfy, group == 0xD01F_A985);
    }
}

/// 与 render_region_png 相同配色的全幅（无裁剪）渲染，附 ED 帧假设。
fn render_full(
    p: &dbpf::Package,
    group: u32,
    grid: &[[u32; 16]; 16],
    fx: bool,
    fy: bool,
    reuse_f0: bool,
) {
    let _ = reuse_f0;
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
    let mut ed0 = vec![0u8; w * w];
    let mut ed1 = vec![0u8; w * w];
    {
        let mut tmp0 = vec![0u8; 2048 * 2048];
        let mut tmp1 = vec![0u8; 2048 * 2048];
        for (ty, row) in grid.iter().enumerate() {
            for (tx, inst) in row.iter().enumerate() {
                let Some(px) = ed.get(inst) else { continue };
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
        for y in 0..w {
            for x in 0..w {
                let mx = if fx { 2047 - (x / 2) } else { x / 2 };
                let my = if fy { 2047 - (y / 2) } else { y / 2 };
                let k = my * 2048 + mx;
                ed0[y * w + x] = tmp0[k];
                ed1[y * w + x] = tmp1[k];
            }
        }
    }
    let sea: i32 = 4928; // -870m × 32 + 32768（shader 垂直映射 z=raw/32-1024）
    // 水面判定用 4×4 box 滤波高度（≈引擎 mip2 降采样；区域视图天然平滑）
    let sm_n = w / 4;
    let mut hsm = vec![0u32; sm_n * sm_n];
    for y in 0..sm_n {
        for x in 0..sm_n {
            let mut s = 0u32;
            for dy in 0..4 {
                for dx in 0..4 {
                    s += u32::from(hgt[(y * 4 + dy) * w + x * 4 + dx]);
                }
            }
            hsm[y * sm_n + x] = s / 16;
        }
    }
    let mut img = RgbImage::new(2048, 2048);
    for y in 0..2048usize {
        for x in 0..2048usize {
            let (fx4, fy4) = (x * 2, y * 2);
            let h = hgt[fy4 * w + fx4] as i32;
            let dx = hgt[fy4 * w + fx4 + 1] as i32 - h;
            let dy = hgt[(fy4 + 1) * w + fx4] as i32 - h;
            let (nx, ny, nz) = (-(dx as f32), -(dy as f32), 60.0f32);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let light = ((nx * 0.5 + ny * 0.5 + nz * 0.7) / nl).max(0.0);
            let hsm_h = hsm[(fy4 / 4) * sm_n + fx4 / 4] as i32;
            let rel = hsm_h - sea;
            let (mut r, mut g, mut b);
            if rel < 0 || h == 0 {
                let d = (-rel as f32 / 600.0).clamp(0.0, 1.0);
                r = (90.0 - 50.0 * d) * (0.6 + 0.4 * light);
                g = (140.0 - 60.0 * d) * (0.6 + 0.4 * light);
                b = (190.0 - 60.0 * d) * (0.6 + 0.4 * light);
            } else {
                let grass = ed1[(y * 2) * w + (x * 2)];
                let desert = false; // 逐区域判定在正式管线内，这里直接看草
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
            }
            let shade = 0.45 + 0.55 * light;
            r *= shade; g *= shade; b *= shade;
            img.put_pixel(x as u32, y as u32, Rgb([r as u8, g as u8, b as u8]));
        }
    }
    let out = format!("tmp/region_preview/full_{group:08X}.png");
    img.save(&out).unwrap();
    println!("-> {out}");
}
