//! 跨区域投票定稿 tile 排布：341 槽位全区域共享 ⇒ 排布唯一，多区域投票消歧。仅开发用。
//!
//! 每个区域独立计算 child→(parent,quad) 的最优匹配，然后对每个 child 按
//! (parent,quad) 多数票定稿；再全局重构金字塔树并校验 [1,4,16,64,256]。
//! 输出排布表（JSON 行）到 stdout 重定向文件，供 region_preview --grid 复用。
//!
//! 用法：tile_grid_vote <package0> <package1...>（跳过全零 tile 占比 > 50% 的区域）
use std::collections::{HashMap, HashSet};

fn load_tiles(path: &str, group: u32) -> HashMap<u32, Vec<u16>> {
    let package = dbpf::Package::open(path).expect("open");
    let mut tiles = HashMap::new();
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
    tiles
}

fn zero_ratio(tiles: &HashMap<u32, Vec<u16>>) -> f64 {
    let n = tiles.values().filter(|px| px.iter().all(|v| *v == 0)).count();
    n as f64 / tiles.len().max(1) as f64
}

fn downsample(px: &[u16]) -> Vec<u32> {
    let mut b = vec![0u32; 128 * 128];
    for y in 0..128 {
        for x in 0..128 {
            let i = (2 * y) * 256 + 2 * x;
            b[y * 128 + x] = (u32::from(px[i]) + u32::from(px[i + 1]) + u32::from(px[i + 256]) + u32::from(px[i + 257])) / 4;
        }
    }
    b
}

fn region_groups(path: &str) -> Vec<u32> {
    let package = dbpf::Package::open(path).expect("open");
    let mut count: HashMap<u32, usize> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 {
            *count.entry(e.id.group).or_default() += 1;
        }
    }
    let mut gs: Vec<u32> = count.into_iter().filter(|(_, n)| *n == 341).map(|(g, _)| g).collect();
    gs.sort();
    gs
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // 收集全部区域（跨包）
    let mut regions: Vec<(String, u32)> = Vec::new();
    for path in &args {
        for g in region_groups(path) {
            regions.push((path.clone(), g));
        }
    }
    println!("regions: {}", regions.len());

    // 预加载 + 过滤全零区域
    let mut data: Vec<(u32, HashMap<u32, Vec<u16>>, HashMap<u32, Vec<u32>>)> = Vec::new();
    for (path, g) in &regions {
        let tiles = load_tiles(path, *g);
        if zero_ratio(&tiles) > 0.5 {
            println!("skip {g:08X}: >50% zero tiles");
            continue;
        }
        let ds: HashMap<u32, Vec<u32>> = tiles.iter().map(|(&i, px)| (i, downsample(px))).collect();
        data.push((*g, tiles, ds));
    }

    // 共享槽位集合（用第一个区域的键）
    let slots: Vec<u32> = {
        let mut s: Vec<u32> = data[0].1.keys().copied().collect();
        s.sort();
        s
    };

    // 投票
    let mut votes: HashMap<u32, HashMap<(u32, usize), usize>> = HashMap::new(); // child -> (parent,quad) -> 票数
    for (g, tiles, ds) in &data {
        for &child in &slots {
            let cb = &ds[&child];
            let mut best = (0u32, 0usize, f64::INFINITY);
            for &parent in &slots {
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
            let slot = votes.entry(child).or_default().entry((best.0, best.1)).or_insert(0);
            *slot += 1;
        }
        println!("voted region {g:08X}");
    }

    // 定稿：每 child 取最高票 (parent, quad)
    let mut final_assign: HashMap<u32, (u32, usize, usize)> = HashMap::new();
    for (child, vs) in &votes {
        let mut best: Vec<((u32, usize), usize)> = vs.iter().map(|(k, n)| (*k, *n)).collect();
        best.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        let ((p, q), n) = best[0];
        final_assign.insert(*child, (p, q, n));
    }

    // 全局重构树：quad 冲突按票数解决
    let mut pairs: Vec<(u32, u32, usize, usize)> = final_assign.iter().map(|(c, (p, q, n))| (*c, *p, *q, *n)).collect();
    pairs.sort_by_key(|(_, _, _, n)| std::cmp::Reverse(*n));
    let mut tree: HashMap<u32, [Option<u32>; 4]> = HashMap::new();
    let mut used_child: HashSet<u32> = HashSet::new();
    let mut used_quad: HashSet<(u32, usize)> = HashSet::new();
    for (child, parent, quad, n) in &pairs {
        if used_child.contains(child) || used_quad.contains(&(*parent, *quad)) { continue; }
        tree.entry(*parent).or_default()[*quad] = Some(*child);
        used_child.insert(*child);
        used_quad.insert((*parent, *quad));
    }
    let child_of: HashMap<u32, (u32, usize)> = used_child.iter().map(|c| { let (p, q, _) = final_assign[c]; (*c, (p, q)) }).collect();
    let is_parent: HashSet<u32> = tree.keys().copied().collect();
    let roots: Vec<u32> = is_parent.iter().copied().filter(|p| !child_of.contains_key(p)).collect();
    let mut level_counts = [0usize; 8];
    if let Some(root) = roots.first() {
        let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
        pos.insert(*root, (0, 0, 0));
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let (x, y, l) = pos[&n];
            level_counts[l.min(7)] += 1;
            if let Some(slots) = tree.get(&n) {
                for (q, c) in slots.iter().enumerate() {
                    if let Some(c) = c {
                        pos.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, l + 1));
                        stack.push(*c);
                    }
                }
            }
        }
    }
    println!("roots={} levels={:?} (expect [1,4,16,64,256,0..])", roots.len(), level_counts);
    let unplaced: Vec<u32> = slots.iter().copied().filter(|s| !child_of.contains_key(s) && !is_parent.contains(s)).collect();
    println!("unplaced tiles: {} {:?}", unplaced.len(), &unplaced[..unplaced.len().min(8)]);

    // 输出 16×16 网格（child_of 里 level4 的才能落格；由根遍历更稳）
    println!("== GRID (16 rows) ==");
    if let Some(root) = roots.first() {
        let mut pos: HashMap<u32, (usize, usize, usize)> = HashMap::new();
        pos.insert(*root, (0, 0, 0));
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let (x, y, l) = pos[&n];
            if let Some(slots) = tree.get(&n) {
                for (q, c) in slots.iter().enumerate() {
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
        for row in &grid {
            println!("{}", row.iter().map(|v| format!("{v:08X}")).collect::<Vec<_>>().join(","));
        }
    }
}
