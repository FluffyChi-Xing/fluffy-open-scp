//! ED b2/b1 通道取值直方图（top 值）。仅开发用。
use std::collections::HashMap;
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let grid_file = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 65556 {
                ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
            }
        }
    }
    let mut grid = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&grid_file).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() { grid[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
    }
    let mut hist: [HashMap<u8, usize>; 4] = Default::default();
    for row in &grid { for inst in row {
        if let Some(d) = ed.get(inst) {
            for v in d {
                for ch in 0..4 { *hist[ch].entry(((v >> (ch * 8)) & 0xFF) as u8).or_insert(0) += 1; }
            }
        }
    }}
    for ch in 0..4usize {
        let mut v: Vec<(u8, usize)> = hist[ch].iter().map(|(k, n)| (*k, *n)).collect();
        v.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        println!("ch{ch}: distinct={} top: {:?}", hist[ch].len(), &v[..v.len().min(10)]);
    }
}
