//! ED 地面场金字塔拼合渲染：查森林区周期条纹。仅开发用。
//! ED tile = 128×128 u32；同 341 槽位共享网格。u32 按字节拆 4 通道输出。
//! 用法：ed_render <package> <region-group> <grid.txt> <out-prefix>
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let grid_file = args.next().expect("grid file");
    let prefix = args.next().expect("out prefix");

    let package = dbpf::Package::open(&path).expect("open");
    let mut tiles: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let data = package.read(e).expect("read");
            if data.len() >= 20 {
                let px: Vec<u32> = data[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                if px.len() == 128 * 128 {
                    tiles.insert(e.id.instance, px);
                }
            }
        }
    }
    println!("ED tiles: {}", tiles.len());

    // ED 槽位与 F0 不同（交集=0），需自匹配金字塔：lane0+lane2 联合降采样误差
    let insts: Vec<u32> = tiles.keys().copied().collect();
    let mut ds: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
    for (&inst, px) in &tiles {
        let mut b = vec![(0u32, 0u32); 64 * 64];
        for y in 0..64 {
            for x in 0..64 {
                let i = (2 * y) * 128 + 2 * x;
                let lane = |v: u32, k: u32| (v >> (k * 8)) & 0xFF;
                let a = px[i]; let b2 = px[i + 1]; let c = px[i + 128]; let d = px[i + 129];
                b[y * 64 + x] = ((lane(a, 0) + lane(b2, 0) + lane(c, 0) + lane(d, 0)) / 4,
                                 (lane(a, 2) + lane(b2, 2) + lane(c, 2) + lane(d, 2)) / 4);
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
            let pp = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 64, (q / 2) * 64);
                let mut e = 0f64;
                for y in 0..64 {
                    let prow = (qy + y) * 128 + qx;
                    let crow = y * 64;
                    for x in 0..64 {
                        let pv = pp[prow + x];
                        let lane = |v: u32, k: u32| (v >> (k * 8)) & 0xFF;
                        let (pe, ce) = (ds[&parent][crow + x], cb[crow + x]);
                        let _ = (pv, lane, pe, ce);
                        e += (f64::from((pp[prow + x] & 0xFF) as u8) - f64::from(ce.0 as u8)).abs()
                           + (f64::from(((pp[prow + x] >> 16) & 0xFF) as u8) - f64::from(ce.1 as u8)).abs();
                    }
                }
                e /= 4096.0;
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
    println!("ED parents={} roots={} used_children={}", tree.len(), roots.len(), used_child.len());
    let mut grid = [[0u32; 16]; 16];
    if let Some(root) = roots.first() {
        let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
        pos.insert(*root, (0, 0, 0));
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let (x, y, l) = pos[&n];
            if let Some(slots) = tree.get(&n) {
                for (q, c) in slots.iter().enumerate() {
                    if let Some(c) = c { pos.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, l + 1)); stack.push(*c); }
                }
            }
        }
        for (i, (x, y, l)) in &pos { if *l == 4 { grid[*y][*x] = *i; } }
    }
    // 导出 ED 网格
    let gpath = format!("{prefix}_grid.txt");
    let mut txt = String::new();
    for row in &grid {
        txt.push_str(&row.iter().map(|v| format!("{v:08X}")).collect::<Vec<_>>().join(","));
        txt.push_str("
");
    }
    std::fs::write(&gpath, txt).unwrap();
    println!("-> {gpath}");

    let w = 2048usize; // 16×128
    let mut planes = vec![[0u32; 4]; w * w];
    for (ty, row) in grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        planes[(ty * 128 + y) * w + tx * 128 + x] = [v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF, (v >> 24) & 0xFF];
                    }
                }
            }
        }
    }

    // 每通道输出灰度 PNG（半分辨率 1024）
    for ch in 0..4 {
        let mut img = image::GrayImage::new(1024, 1024);
        for y in 0..1024usize {
            for x in 0..1024usize {
                let mut acc = 0u32;
                for dy in 0..2usize { for dx in 0..2usize {
                    acc += planes[(y * 2 + dy) * w + x * 2 + dx][ch];
                }}
                img.put_pixel(x as u32, y as u32, image::Luma([(acc / 4).min(255) as u8]));
            }
        }
        let out = format!("{prefix}_ch{ch}.png");
        img.save(&out).unwrap();
        println!("-> {out}");
    }
}
