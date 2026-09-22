//! 区域地形俯视预览渲染：金字塔匹配 → 4096² 拼合 → hillshade PNG。仅开发用。
//!
//! 输出（tmp/region_preview/<group>.png）：
//!   - 灰度 hillshade（光来自左上）+ 海平面以下蓝染
//!   - 城市地块框（地块表世界坐标 → 256px 方框）
//!   - 16×16 tile 网格线（暗红，用于对照拼接痕迹）
//!
//! 用法：region_preview <package> <region-group> [outdir]
use std::collections::HashMap;
use std::path::Path;

use image::{Rgb, RgbImage};

const HALF: f32 = 16384.0;
const CELL: f32 = 8.0;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let outdir = args.next().unwrap_or_else(|| "tmp/region_preview".into());
    let grid_file = args.next();
    std::fs::create_dir_all(&outdir).expect("outdir");
    let package = dbpf::Package::open(&path).expect("open");

    // ---- 1. 读 tile ----
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let data = package.read(e).expect("read");
            if data.len() >= 20 {
                let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                if px.len() == 65536 {
                    tiles.insert(e.id.instance, px);
                }
            }
        }
    }
    if tiles.len() != 341 {
        println!("WARN group {group:08X}: {} tiles (expect 341)", tiles.len());
    }
    let insts: Vec<u32> = tiles.keys().copied().collect();

    // ---- 2. 排布：优先读共享网格文件（全游戏通用，tile_grid_vote/slot_compare 实证）----
    let mut grid = [[0u32; 16]; 16];
    let shared = grid_file.as_ref().and_then(|f| std::fs::read_to_string(f).ok());
    if let Some(text) = &shared {
        for (y, line) in text.lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
            for (x, v) in line.split(',').take(16).enumerate() {
                grid[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0);
            }
        }
        println!("grid from file");
    }

    // ---- 2b. 否则本区域金字塔匹配（与 tile_arrange 相同）----
    if shared.is_none() {
    let mut ds: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&inst, px) in &tiles {
        let mut b = vec![0u32; 128 * 128];
        for y in 0..128 {
            for x in 0..128 {
                let i = (2 * y) * 256 + 2 * x;
                b[y * 128 + x] = (u32::from(px[i]) + u32::from(px[i + 1]) + u32::from(px[i + 256]) + u32::from(px[i + 257])) / 4;
            }
        }
        ds.insert(inst, b);
    }
    let mut assign: HashMap<u32, (u32, usize, f64)> = HashMap::new();
    for &child in &insts {
        let cb = &ds[&child];
        let mut best = (0u32, 0usize, f64::INFINITY);
        for &parent in &insts {
            if parent == child { continue; }
            let px = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 128, (q / 2) * 128);
                let mut e = 0f64;
                for y in 0..128 {
                    let prow = (qy + y) * 256 + qx;
                    let crow = y * 128;
                    for x in 0..128 {
                        e += (f64::from(px[prow + x]) - f64::from(cb[crow + x])).abs();
                    }
                }
                e /= 16384.0;
                if e < best.2 { best = (parent, q, e); }
            }
        }
        assign.insert(child, best);
    }
    let mut children: Vec<(u32, u32, usize, f64)> = assign.iter().map(|(c, (p, q, e))| (*c, *p, *q, *e)).collect();
    children.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let mut tree: HashMap<u32, [Option<u32>; 4]> = HashMap::new();
    let mut used_child = std::collections::HashSet::new();
    let mut used_quad = std::collections::HashSet::new();
    for (child, parent, quad, err) in &children {
        if used_child.contains(child) || used_quad.contains(&(*parent, *quad)) || *err > 60.0 { continue; }
        tree.entry(*parent).or_default()[*quad] = Some(*child);
        used_child.insert(*child);
        used_quad.insert((*parent, *quad));
    }
    let child_of: HashMap<u32, (u32, usize)> = used_child.iter().map(|c| { let (p, q, _) = assign[c]; (*c, (p, q)) }).collect();
    let is_parent: std::collections::HashSet<u32> = tree.keys().copied().collect();
    let roots: Vec<u32> = is_parent.iter().copied().filter(|p| !child_of.contains_key(p)).collect();
    if let Some(root) = roots.first() {
        let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
        pos.insert(*root, (0, 0, 0));
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let (x, y, lvl) = pos[&n];
            if let Some(slots) = tree.get(&n) {
                for (q, c) in slots.iter().enumerate() {
                    if let Some(c) = c { pos.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, lvl + 1)); stack.push(*c); }
                }
            }
        }
        for (i, (x, y, l)) in &pos { if *l == 4 { grid[*y][*x] = *i; } }
    }
    } // end if shared.is_none()

    // ---- 3. 拼合 ----
    let w = 4096usize;
    let mut mosaic = vec![0u16; w * w];
    for (ty, row) in grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..256 {
                    mosaic[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256]
                        .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }

    // ---- 4. 地块表 ----
    let mut plots: Vec<(f32, f32)> = Vec::new();
    if let Some(pe) = package.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == 0x51E7_A18D) {
        if let Ok(region) = sc_properties::PropertyFile::parse(&package.read(pe).unwrap()) {
            if let Some(pt_key) = region.get(0xFB7A_85A0).and_then(|p| match &p.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Key(k)) => Some(k.instance), _ => None }) {
                if let Some(pte) = package.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == pt_key) {
                    if let Ok(pt) = sc_properties::PropertyFile::parse(&package.read(pte).unwrap()) {
                        if let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) {
                            for v in vs {
                                if let sc_properties::Value::Vector2(v) = v { plots.push((v[0], v[1])); }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("group {group:08X}: plots = {}", plots.len());

    // ---- 5. 渲染 ----
    // 高度归一化（2%-98% 分位）
    let mut sample: Vec<u32> = mosaic.iter().step_by(17).map(|v| u32::from(*v)).collect();
    sample.sort_unstable();
    let lo = sample[sample.len() / 50].max(1) as f32;
    let hi = (sample[sample.len() * 49 / 50] as f32).max(lo + 1.0);
    // 海平面：外圈 16px 环的中位数
    let mut ring: Vec<u32> = Vec::new();
    for i in 0..w {
        for &j in &[0usize, 1, 2, 3, w - 4, w - 3, w - 2, w - 1] {
            ring.push(u32::from(mosaic[i * w + j]));
            ring.push(u32::from(mosaic[j * w + i]));
        }
    }
    ring.sort_unstable();
    let sea = ring[ring.len() / 2] as f32;
    println!("sea level ≈ {sea:.0}  height range {lo}..{hi}");

    let mut img = RgbImage::new(w as u32, w as u32);
    let h_at = |x: usize, y: usize| -> f32 {
        let x = x.min(w - 1); let y = y.min(w - 1);
        f32::from(mosaic[y * w + x])
    };
    for y in 0..w {
        for x in 0..w {
            let h = h_at(x, y);
            // hillshade：法线由梯度，光从左上 (-0.5,-0.5,0.7)
            let dx = h_at(x + 1, y) - h;
            let dy = h_at(x, y + 1) - h;
            let nx = -dx; let ny = -dy; let nz = 60.0; // z 比例压平
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let light = ((nx * 0.5 + ny * 0.5 + nz * 0.7) / nl).max(0.0);
            let t = ((h - lo) / (hi - lo)).clamp(0.0, 1.0);
            let mut base = (30.0 + 200.0 * t * (0.35 + 0.65 * light)) as f32;
            let (r, g, b) = if h < sea - 20.0 {
                // 水下：蓝染，按深度变暗
                let d = ((sea - h) / 800.0).clamp(0.0, 1.0);
                base *= 1.0 - 0.4 * d;
                (base * 0.55, base * 0.75, base * 1.05)
            } else {
                (base, base, base)
            };
            img.put_pixel(x as u32, y as u32, Rgb([r.clamp(0.0, 255.0) as u8, g.clamp(0.0, 255.0) as u8, b.clamp(0.0, 255.0) as u8]));
        }
    }
    // tile 网格线（暗红）
    for k in 0..=16usize {
        let p = (k * 256).min(w - 1);
        for i in 0..w {
            let px = img.get_pixel_mut(p as u32, i as u32);
            *px = Rgb([120, 30, 30]);
            let px = img.get_pixel_mut(i as u32, p as u32);
            *px = Rgb([120, 30, 30]);
        }
    }
    // 地块框：四种世界→图坐标约定分色（A绿=原样 B紫=y翻 C青=x翻 D橙=xy翻）
    let variants: [(bool, bool, [u8; 3]); 4] = [
        (false, false, [0, 230, 0]),
        (false, true, [230, 0, 230]),
        (true, false, [0, 210, 230]),
        (true, true, [255, 150, 0]),
    ];
    for (fx, fy, color) in variants {
        let mut relief_sum = 0f64; let mut water_cnt = 0usize; let mut ok = 0usize;
        for (wx, wy) in &plots {
            let cx = (if fx { HALF - wx } else { wx + HALF }) / CELL;
            let cy = (if fy { HALF - wy } else { wy + HALF }) / CELL;
            // 客观判据：框内地形起伏与水深占比
            {
                let x0i = (cx as isize - 128).max(0) as usize; let y0i = (cy as isize - 128).max(0) as usize;
                let mut mn = u16::MAX; let mut mx = 0u16; let mut low = 0usize; let mut n = 0usize;
                for y in 0..256 { for x in 0..256 {
                    let v = mosaic[(y0i + y) * w + x0i + x];
                    mn = mn.min(v); mx = mx.max(v);
                    let d = f32::from(v) - sea;
                    if (-100.0..=600.0).contains(&d) { low += 1; }
                    n += 1;
                }}
                relief_sum += (mx - mn) as f64;
                water_cnt += low * 100 / n;
                if (mx - mn) < 4000 { ok += 1; }
                println!("    variant({fx},{fy}) plot ({wx:.0},{wy:.0}): center elev={:.0}, relief={}, low-plain%={}", f32::from(mosaic[(cy as usize) * w + cx as usize]) , mx - mn, low * 100 / n);
            }
            let (x0, y0) = (cx as isize - 128, cy as isize - 128);
            for t in 0..256isize {
                for &(xx, yy) in &[(x0 + t, y0), (x0 + t, y0 + 255), (x0, y0 + t), (x0 + 255, y0 + t)] {
                    if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < w {
                        *img.get_pixel_mut(xx as u32, yy as u32) = Rgb(color);
                    }
                }
            }
        }
        println!("variant fx={fx} fy={fy}: avg relief={:.0}, avg low-plain%={}, low-relief plots {}/{}", relief_sum / plots.len() as f64, water_cnt / plots.len().max(1), ok, plots.len());
    }
    let out = Path::new(&outdir).join(format!("{group:08X}.png"));
    // 缩到 2048 存储（够对照）
    let small = image::imageops::resize(&img, 2048, 2048, image::imageops::FilterType::Triangle);
    small.save(&out).expect("save png");
    println!("-> {}", out.display());
}
