//! 区域 ↔ 地块表配对探针：同 instance 的地块表每区域一份（group 不同），
//! 用"地块位置落陆率"给每个区域选对表。仅开发用。
//!
//! 用法：plot_pair <package> [grid-prefix]（grid 文件 = <prefix><GROUP>.png 同目录 <GROUP>_grid）
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();

    // 1. 所有区域组（F0 计数 = 341）
    let mut f0cnt: HashMap<u32, usize> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 {
            *f0cnt.entry(e.id.group).or_default() += 1;
        }
    }
    let regions: Vec<u32> = {
        let mut g: Vec<u32> = f0cnt.into_iter().filter(|(_, n)| *n == 341).map(|(k, _)| k).collect();
        g.sort();
        g
    };
    println!("regions: {:?}", regions.iter().map(|r| format!("{r:08X}")).collect::<Vec<_>>());

    // 2. 所有地块表（instance 2B9C480C）→ (group, positions)
    let mut tables: Vec<(u32, usize, Vec<(f32, f32)>)> = Vec::new();
    for e in p.entries() {
        if e.id.type_id == 0x00B1_B104 && e.id.instance == 0x2B9C_480C {
            if let Ok(pt) = sc_properties::PropertyFile::parse(&p.read(e).unwrap()) {
                let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) else { continue };
                let pos: Vec<(f32, f32)> = vs.iter().filter_map(|v| match v {
                    sc_properties::Value::Vector2(v) => Some((v[0], v[1])), _ => None }).collect();
                println!("table group {:08X}: {} plots, sizes {}B", e.id.group, pos.len(), e.compressed_size);
                tables.push((e.id.group, pos.len(), pos));
            }
        }
    }

    // 3. 每区域：拼马赛克（需要各区域金字塔——这里偷懒用海面=环中位数+高度直读）
    for &region in &regions {
        let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
        for e in p.entries() {
            if e.id.type_id == 0x03E4_21F0 && e.id.group == region {
                let d = p.read(e).unwrap();
                if d.len() == 131092 {
                    tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
                }
            }
        }
        if tiles.len() != 341 { println!("region {region:08X}: {} tiles, skip", tiles.len()); continue; }
        // 用共享网格（tile_arrange 已证全游戏一致）
        let mut grid = [[0u32; 16]; 16];
        // 网格文件约定：tmp/region_preview/grid_shared.txt 只有一份
        for (y, line) in std::fs::read_to_string("tmp/region_preview/grid_shared.txt").unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
            for (x, v) in line.split(',').take(16).enumerate() { grid[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
        }
        let w = 4096usize;
        let mut hgt = vec![0u16; w * w];
        for (ty, row) in grid.iter().enumerate() {
            for (tx, i) in row.iter().enumerate() {
                if let Some(px) = tiles.get(i) {
                    for y in 0..256 { hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256].copy_from_slice(&px[y * 256..(y + 1) * 256]); }
                }
            }
        }
        let mut ring: Vec<u32> = Vec::new();
        for i in 0..w { for &j in &[0usize, 1, w - 2, w - 1] { ring.push(u32::from(hgt[i * w + j])); ring.push(u32::from(hgt[j * w + i])); } }
        ring.sort_unstable();
        let sea = ring[ring.len() / 2] as i32;

        // 4. 每张表：落陆率（地块中心高度 > 海面+100 且 256px 窗内非全水）
        for (tgroup, n, pos) in &tables {
            let mut on_land = 0usize;
            for (wx, wy) in pos {
                let cx = ((wx + 16384.0) / 8.0) as usize;
                let cy = ((wy + 16384.0) / 8.0) as usize;
                if cx >= w || cy >= w { continue; }
                let mut mn = u16::MAX; let mut mx = 0u16;
                for dy in 0..256 { for dx in 0..256 {
                    let v = hgt[(cy + dy.min(w - 1 - cy)) * w + cx + dx.min(w - 1 - cx)];
                    mn = mn.min(v); mx = mx.max(v);
                }}
                if mx as i32 > sea + 200 { on_land += 1; }
            }
            if on_land > 0 {
                println!("region {region:08X} × table {tgroup:08X} ({n} plots): on-land {on_land}/{n}");
            }
        }
    }
}
