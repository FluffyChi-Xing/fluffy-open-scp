//! 区域彩色合成渲染：F0 高度 + ED 场上色（水/森林/材质）+ 资源画刷叠图。仅开发用。
//!
//! 上色模型（对游戏机制的近似）：
//!   - 基底：高度分带（低=草地绿、中=黄绿、高=岩棕、顶=浅岩）
//!   - 水面：ED byte1 > 0 → 水蓝（深度按值加深）；高度 < 海面 → 海蓝
//!   - 森林：ED byte0 ∈ [100..120] 密度 → 深绿斑点
//!   - 道路：ED byte0 == 15 → 灰线
//!   - 光照：hillshade 乘算
//! 资源画刷（coal/oil/ore/watertable/radiation/soil/forest/g pollution）
//! 按世界坐标画环 + 输出每地块资源归属表。
//!
//! 用法：region_color <package> <group> <f0-grid> <ed-grid> <out.png>
use std::collections::HashMap;

#[derive(serde::Serialize)]
struct Brush {
    target: String,
    mapidx: i32,
    stamps: Vec<(f32, f32)>,
}


// ---- 正确选表：城市单条组显式引用区域；表按城市 id 交集选择 ----
fn find_plot_positions(p: &dbpf::Package, region: u32) -> Option<Vec<(f32, f32)>> {
    let mut city_ids: Vec<u32> = Vec::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.compressed_size > 60 { continue; }
        let Ok(pf) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(k)), .. }) = pf.get(0xC194_9C4D) {
            if k.instance == 0x51E7_A18D && k.group == region { city_ids.push(e.id.group); }
        }
    }
    let set: std::collections::HashSet<u32> = city_ids.iter().copied().collect();
    let mut best: Option<(usize, Vec<(f32, f32)>)> = None;
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x2B9C_480C { continue; }
        let Ok(pt) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        let ids: Vec<u32> = match pt.get(0x16B7_B1EF) {
            Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) => vs.iter().filter_map(|v| match v { sc_properties::Value::UInt32(x) => Some(*x), _ => None }).collect(),
            _ => continue,
        };
        let ov = ids.iter().filter(|i| set.contains(i)).count();
        if best.as_ref().map(|(b, _)| ov > *b).unwrap_or(true) {
            if let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) {
                let pos: Vec<(f32, f32)> = vs.iter().filter_map(|v| match v { sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect();
                best = Some((ov, pos));
            }
        }
    }
    best.map(|(_, pos)| pos)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let f0_grid_file = args.next().unwrap();
    let ed_grid_file = args.next().unwrap();
    let out = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();

    // ---- F0 高度马赛克 ----
    let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 131092 {
                f0.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }
        }
    }
    let mut f0g = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&f0_grid_file).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() { f0g[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
    }
    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in f0g.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = f0.get(i) {
                for y in 0..256 { hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256].copy_from_slice(&px[y * 256..(y + 1) * 256]); }
            }
        }
    }

    // ---- ED 场马赛克（2048，双线性上采样到 4096）----
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 65556 {
                ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
            }
        }
    }
    let mut edg = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&ed_grid_file).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() { edg[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
    }
    let wm = 2048usize;
    let mut ed0 = vec![0u8; wm * wm]; // forest/road
    let mut ed1 = vec![0u8; wm * wm]; // water
    let mut ed2 = vec![0u8; wm * wm]; // material
    for (ty, row) in edg.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = ed.get(i) {
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        let k = (ty * 128 + y) * wm + tx * 128 + x;
                        ed0[k] = (v & 0xFF) as u8;
                        ed1[k] = ((v >> 8) & 0xFF) as u8;
                        ed2[k] = ((v >> 16) & 0xFF) as u8;
                    }
                }
            }
        }
    }
    let ed_at = |x: usize, y: usize| -> (u8, u8, u8) {
        let e = ed1[(y / 2).min(wm - 1) * wm + (x / 2).min(wm - 1)];
        (ed0[(y / 2).min(wm - 1) * wm + (x / 2).min(wm - 1)], e, ed2[(y / 2).min(wm - 1) * wm + (x / 2).min(wm - 1)])
    };

    // ---- 水位面：低高度直方图的大跳变（河床被刻平到水位下方）----
    let mut hist = [0u64; 16];
    for v in &hgt {
        if *v >= 4096 { continue; }
        hist[*v as usize / 256] += 1;
    }
    let mut sea = 0i32;
    for i in 1..14usize {
        let prev_min = (0..i).map(|k| hist[k]).filter(|n| *n > 0).min().unwrap_or(u64::MAX);
        if hist[i] > 5000 && hist[i] > 4 * prev_min.max(1) {
            // 吸收连续大桶（河床平底可能跨多桶）
            let mut j = i;
            while j + 1 < 16 && hist[j + 1] > 5000 && hist[j + 1] * 4 > hist[j] * 3 { j += 1; }
            sea = (j * 256 + 264) as i32;
            break;
        }
    }
    println!("water plane L = {sea}");

    // ---- 地块（城市 id 交集选表）----
    let plots: Vec<(f32, f32)> = find_plot_positions(&p, group).unwrap_or_default();
    println!("plots = {}", plots.len());

    // ---- 画刷（本组内 00B2CCCA 名含 brushes/Brushes 的属性）----
    let mut brushes: Vec<Brush> = Vec::new();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.group != group { continue; }
        let Ok(pf) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) else { continue };
        let Some(name_prop) = pf.get(0x00B2_CCCA) else { continue };
        let sc_properties::Kind::Scalar(sc_properties::Value::String8(name)) = &name_prop.kind else { continue };
        if !(name.ends_with("brushes") || name.ends_with("Brushes")) { continue; }
        let mut stamps = Vec::new();
        if let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pf.get(0x02A9_07B6) {
            for v in vs {
                if let sc_properties::Value::Transform(t) = v {
                    stamps.push((t.matrix[9], t.matrix[10]));
                }
            }
        }
        let mapidx = pf.get(0x0DE4_3899).map(|pp| match &pp.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::UInt32(x)) => *x as i32, _ => -1 }).unwrap_or(-1);
        println!("brush {} (mapidx {mapidx}): {} stamps at {:?}", name, stamps.len(), stamps);
        brushes.push(Brush { target: name.clone(), mapidx, stamps });
    }

    // ---- 上色渲染 ----
    let h_at = |x: usize, y: usize| -> i32 { hgt[y.min(w - 1) * w + x.min(w - 1)] as i32 };
    let mut img = image::RgbImage::new(w as u32, w as u32);
    for y in 0..w {
        for x in 0..w {
            let h = h_at(x, y);
            let dx = h_at(x + 1, y) - h;
            let dy = h_at(x, y + 1) - h;
            let (nx, ny, nz) = (-dx as f32, -dy as f32, 60.0f32);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let light = ((nx * 0.5 + ny * 0.5 + nz * 0.7) / nl).max(0.0);
            let rel = h - sea;
            let (f, grass, _m) = ed_at(x, y);
            let (mut r, mut g, mut b);
            if rel < 0 || h == 0 {
                // 水体（河/湖/海）：按深度加深
                let d = (-rel as f32 / 600.0).clamp(0.0, 1.0);
                r = (90.0 - 50.0 * d) * (0.6 + 0.4 * light);
                g = (140.0 - 60.0 * d) * (0.6 + 0.4 * light);
                b = (190.0 - 60.0 * d) * (0.6 + 0.4 * light);
            } else {
                // 陆地：草量 = 湿度(b1) × 平缓度（陡坡露岩石，山上平地也是草）
                let grass_k = (grass as f32 / 120.0).clamp(0.0, 1.0);
                let slope = ((dx * dx + dy * dy) as f32).sqrt() / 60.0;
                let slope_k = (slope / 1.6).clamp(0.0, 1.0);
                let g_amt = (grass_k * (1.0 - slope_k * 0.85)).clamp(0.0, 1.0);
                if rel < 350 {
                    // 沙岸
                    r = 190.0; g = 175.0; b = 130.0;
                } else {
                    // 草绿 ↔ 岩棕 按草量混合
                    let t = ((rel - 350) as f32 / 6000.0).clamp(0.0, 1.0);
                    let rock_r = 150.0 + 50.0 * t;
                    let rock_g = 135.0 + 25.0 * t;
                    let rock_b = 100.0 + 35.0 * t;
                    r = rock_r + (95.0 - rock_r) * g_amt;
                    g = rock_g + (160.0 - rock_g) * g_amt;
                    b = rock_b + (70.0 - rock_b) * g_amt;
                }
                // 森林斑点
                if (100..=120).contains(&f) {
                    let d = 1.0 - (f as f32 - 100.0) / 20.0;
                    r *= 1.0 - 0.5 * d; g *= 1.0 - 0.1 * d; b *= 1.0 - 0.5 * d;
                }
                // 道路
                if f == 15 { r = 130.0; g = 122.0; b = 112.0; }
            }
            let shade = 0.45 + 0.55 * light;
            r *= shade; g *= shade; b *= shade;
            img.put_pixel(x as u32, y as u32, image::Rgb([r as u8, g as u8, b as u8]));
        }
    }

    // ---- 资源画刷环 + 地块框 ----
    let colors: [(&str, [u8; 3]); 8] = [
        ("coal", [40, 40, 40]), ("oil", [20, 20, 20]), ("ore", [200, 150, 0]),
        ("watertable", [0, 120, 255]), ("radiation", [0, 220, 60]),
        ("soil", [150, 100, 50]), ("forest", [0, 150, 0]), ("groundpollution", [160, 60, 200]),
    ];
    let ring_color = |target: &str| -> [u8; 3] {
        colors.iter().find(|(n, _)| target.starts_with(n)).map(|(_, c)| *c).unwrap_or([255, 0, 255])
    };
    let world_to_px = |wx: f32, wy: f32| -> (usize, usize) {
        (((wx + 16384.0) / 8.0) as usize, ((wy + 16384.0) / 8.0) as usize)
    };
    for br in &brushes {
        let c = ring_color(&br.target);
        for (wx, wy) in &br.stamps {
            let (cx, cy) = world_to_px(*wx, *wy);
            let rad = 128usize; // 1024m 半径示意
            for a in 0..360 {
                let (dx, dy) = ((a as f32).to_radians().cos(), (a as f32).to_radians().sin());
                for t in [rad, rad + 2] {
                    let px = (cx as f32 + dx * t as f32) as isize;
                    let py = (cy as f32 + dy * t as f32) as isize;
                    if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < w {
                        img.put_pixel(px as u32, py as u32, image::Rgb(c));
                    }
                }
            }
        }
    }
    for (wx, wy) in &plots {
        let (cx, cy) = world_to_px(*wx, *wy);
        let (x0, y0) = (cx as isize - 128, cy as isize - 128);
        for t in 0..256isize {
            for &(xx, yy) in &[(x0 + t, y0), (x0 + t, y0 + 255), (x0, y0 + t), (x0 + 255, y0 + t)] {
                if xx >= 0 && yy >= 0 && (xx as usize) < w && (yy as usize) < w {
                    img.put_pixel(xx as u32, yy as u32, image::Rgb([255, 210, 0]));
                }
            }
        }
        // 每地块资源归属（画刷环 1024m 内视为覆盖）
        let mut got: Vec<&str> = Vec::new();
        for br in &brushes {
            for (bx, by) in &br.stamps {
                let d = ((bx - wx).powi(2) + (by - wy).powi(2)).sqrt();
                if d < 1024.0 { got.push(br.target.trim_end_matches("heightmap")); break; }
            }
        }
        println!("plot ({wx:.0},{wy:.0}): resources = {:?}", got);
    }

    let small = image::imageops::resize(&img, 2048, 2048, image::imageops::FilterType::Triangle);
    small.save(&out).expect("save");
    println!("-> {out}");
}
