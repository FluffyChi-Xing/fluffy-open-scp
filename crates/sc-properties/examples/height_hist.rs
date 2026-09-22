//! 区域高度直方图（排除全零 tile），找水位面候选。仅开发用。
use std::collections::HashMap;
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }
        }
    }
    // 共享网格拼合
    let mut grid = [[0u32; 16]; 16];
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
    // 直方图（排除 0）：256 桶 × 256 高度
    let mut hist = [0u64; 256];
    let mut nonzero = 0u64;
    for v in &hgt {
        if *v == 0 { continue; }
        hist[*v as usize / 256] += 1;
        nonzero += 1;
    }
    println!("nonzero cells: {nonzero} / {}", hgt.len());
    for (i, n) in hist.iter().enumerate() {
        if *n > 0 { println!("{:4}-{:4}: {:9} {}", i * 256, i * 256 + 255, n, "*".repeat(((*n as f64).sqrt() as usize).min(60))); }
    }
}
