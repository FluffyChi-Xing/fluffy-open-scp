//! 区域组 tile 数 + 高度统计（含零 tile 计数）。仅开发用。
use std::collections::HashMap;
fn main() {
    let path = std::env::args().nth(1).expect("package");
    let p = dbpf::Package::open(&path).unwrap();
    let mut agg: HashMap<u32, (usize, usize, u64, u16, u16)> = HashMap::new(); // group -> (tiles, zero_tiles, sum, min, max)
    for e in p.entries() {
        if e.id.type_id != 0x03E4_21F0 { continue; }
        let data = p.read(e).unwrap();
        if data.len() < 20 { continue; }
        let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        if px.len() != 65536 { continue; }
        let (mut z, mut s, mut mn, mut mx) = (0usize, 0u64, u16::MAX, 0u16);
        for v in &px { let u = u32::from(*v); s += u as u64; mn = mn.min(*v); mx = mx.max(*v); if *v == 0 { z += 1; } }
        let a = agg.entry(e.id.group).or_default();
        a.0 += 1; a.1 += if z == 65536 { 1 } else { 0 }; a.2 += s; a.3 = a.3.min(mn); a.4 = a.4.max(mx);
    }
    let mut v: Vec<_> = agg.into_iter().collect();
    v.sort_by_key(|(_, a)| std::cmp::Reverse(a.0));
    for (g, a) in v.iter().take(20) {
        println!("{g:08X}: tiles={} all-zero-tiles={} avg={} min={} max={}", a.0, a.1, a.2 / (a.0 as u64 * 65536), a.3, a.4);
    }
}
