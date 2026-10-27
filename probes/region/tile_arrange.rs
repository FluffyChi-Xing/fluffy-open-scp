//! tile 金字塔完整还原：child→(parent,quadrant) 图 → 树 → 16×16 mip0 排布。仅开发用。
//!
//! 假设（已被 tile_pyramid 探针初步证实）：区域背景 = 4096×4096 格高度图，
//! 按 16×16 tile（256×256 u16）存 5 级 mip：256+64+16+4+1 = 341 张。
//! 父 tile 每象限 = 子 tile 的 2×2 box 降采样（u16 精度内近似成立）。
//!
//! 用法：tile_arrange <package> <region-group>
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package");
    let group = u32::from_str_radix(&args.next().expect("group").trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in package.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let data = package.read(e).expect("read");
            let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            if px.len() == 65536 {
                tiles.insert(e.id.instance, px);
            }
        }
    }
    println!("tiles: {}", tiles.len());
    let insts: Vec<u32> = tiles.keys().copied().collect();

    // 每 tile 的 2×2 box 降采样（128×128）
    let mut ds: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&inst, px) in &tiles {
        let mut b = vec![0u32; 128 * 128];
        for y in 0..128 {
            for x in 0..128 {
                let i = (2 * y) * 256 + 2 * x;
                b[y * 128 + x] = (u32::from(px[i])
                    + u32::from(px[i + 1])
                    + u32::from(px[i + 256])
                    + u32::from(px[i + 257]))
                    / 4;
            }
        }
        ds.insert(inst, b);
    }

    // child × parent × quadrant 误差，取最优
    let mut assign: HashMap<u32, (u32, usize, f64)> = HashMap::new();
    for &child in &insts {
        let cb = &ds[&child];
        let mut best = (0u32, 0usize, f64::INFINITY);
        for &parent in &insts {
            if parent == child {
                continue;
            }
            let px = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 128, (q / 2) * 128);
                let mut e = 0f64;
                for y in 0..128 {
                    let prow = (qy + y) * 256 + qx;
                    let crow = y * 128;
                    for x in 0..128 {
                        let d = f64::from(px[prow + x]) - f64::from(cb[crow + x]);
                        e += d.abs();
                    }
                }
                e /= 16384.0;
                if e < best.2 {
                    best = (parent, q, e);
                }
            }
        }
        assign.insert(child, best);
    }

    // 贪心构建 parent -> [quad] = child（按误差升序，误差阈值截断）
    let mut children: Vec<(u32, u32, usize, f64)> =
        assign.iter().map(|(c, (p, q, e))| (*c, *p, *q, *e)).collect();
    children.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());
    let mut tree: HashMap<u32, [Option<u32>; 4]> = HashMap::new();
    let mut used_child: std::collections::HashSet<u32> = Default::default();
    let mut used_quad: std::collections::HashSet<(u32, usize)> = Default::default();
    let mut rejected: Vec<(u32, u32, usize, f64)> = Vec::new();
    for (child, parent, quad, err) in &children {
        if used_child.contains(child) || used_quad.contains(&(*parent, *quad)) {
            continue;
        }
        if *err > 60.0 {
            rejected.push((*child, *parent, *quad, *err));
            continue;
        }
        tree.entry(*parent).or_default()[*quad] = Some(*child);
        used_child.insert(*child);
        used_quad.insert((*parent, *quad));
    }
    let complete: Vec<u32> = tree
        .iter()
        .filter(|(_, s)| s.iter().all(|o| o.is_some()))
        .map(|(p, _)| *p)
        .collect();
    println!("parents with 4 children: {} (expect 64)", complete.len());
    println!("rejected by err>60: {}", rejected.len());
    for (c, p, q, e) in rejected.iter().take(6) {
        println!("  rejected child 0x{c:08X} -> 0x{p:08X} q{q} err={e:.2}");
    }

    let child_of: HashMap<u32, (u32, usize)> = used_child
        .iter()
        .map(|c| {
            let (p, q, _) = assign[c];
            (*c, (p, q))
        })
        .collect();
    let is_parent: std::collections::HashSet<u32> = tree.keys().copied().collect();
    let mip0: Vec<u32> = insts
        .iter()
        .copied()
        .filter(|i| !is_parent.contains(i) && child_of.contains_key(i))
        .collect();
    println!("leaves (mip0): {} (expect 256)", mip0.len());
    let isolated: Vec<u32> = insts
        .iter()
        .copied()
        .filter(|i| !is_parent.contains(i) && !child_of.contains_key(i))
        .collect();
    println!("isolated (no parent no child): {}", isolated.len());
    for i in &isolated {
        println!("  isolated 0x{i:08X}");
    }

    // 根 = 是父但无父
    let roots: Vec<u32> = is_parent
        .iter()
        .copied()
        .filter(|p| !child_of.contains_key(p))
        .collect();
    println!(
        "roots: {}",
        roots.iter().map(|r| format!("0x{r:08X}")).collect::<Vec<_>>().join(" ")
    );

    // 自根向下赋 (x,y,level)
    let mut positions: HashMap<u32, (usize, usize, usize)> = HashMap::new();
    for root in &roots {
        positions.insert(*root, (0, 0, 0));
        let mut stack = vec![*root];
        while let Some(n) = stack.pop() {
            let (x, y, lvl) = positions[&n];
            if let Some(slots) = tree.get(&n) {
                for (q, c) in slots.iter().enumerate() {
                    if let Some(c) = c {
                        positions.insert(*c, (x * 2 + q % 2, y * 2 + q / 2, lvl + 1));
                        stack.push(*c);
                    }
                }
            }
        }
    }
    let mut counts = [0usize; 8];
    for (_, (_, _, l)) in &positions {
        counts[(*l).min(7)] += 1;
    }
    println!("levels node counts: {counts:?}");
    let mut grid = [[0f64; 16]; 16];
    let mut gridi = [[0u32; 16]; 16];
    let mut placed = 0usize;
    for (i, (x, y, l)) in &positions {
        if *l != 4 {
            continue;
        }
        let px = &tiles[i];
        let mut acc = 0u64;
        for v in px {
            acc += u64::from(*v);
        }
        grid[*y][*x] = acc as f64 / 65536.0;
        gridi[*y][*x] = *i;
        placed += 1;
    }
    println!("placed mip0 tiles: {placed} (expect 256)");
    println!("== 16x16 mip0 avg-height grid (avg/100) ==");
    for row in &grid {
        println!(
            "{}",
            row.iter().map(|v| format!("{:5.0}", v / 100.0)).collect::<Vec<_>>().join(" ")
        );
    }
    println!("== instance ids ==");
    for row in &gridi {
        println!(
            "{}",
            row.iter().map(|v| format!("{v:08X}")).collect::<Vec<_>>().join(",")
        );
    }
}
