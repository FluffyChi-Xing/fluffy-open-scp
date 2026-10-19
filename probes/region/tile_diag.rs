//! 诊断：区域内每 tile 最优父的误差分布。仅开发用。
use std::collections::HashMap;
fn main() {
    let path = std::env::args().nth(1).expect("package");
    let group = u32::from_str_radix(std::env::args().nth(2).expect("group").trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let data = p.read(e).unwrap();
            if data.len() >= 20 {
                let px: Vec<u16> = data[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                if px.len() == 65536 { tiles.insert(e.id.instance, px); }
            }
        }
    }
    let insts: Vec<u32> = tiles.keys().copied().collect();
    let mut buckets = [0usize; 6]; // <1,<5,<20,<60,<200,>=200
    let mut flat = 0usize; // tile 自身方差极小（平坦）
    for &child in &insts {
        let px = &tiles[&child];
        let mean = px.iter().map(|v| u32::from(*v) as f64).sum::<f64>() / 65536.0;
        let var = px.iter().map(|v| { let d = f64::from(*v) - mean; d * d }).sum::<f64>() / 65536.0;
        if var < 25.0 { flat += 1; }
        let mut b = vec![0u32; 128 * 128];
        for y in 0..128 {
            for x in 0..128 {
                let i = (2 * y) * 256 + 2 * x;
                b[y * 128 + x] = (u32::from(px[i]) + u32::from(px[i + 1]) + u32::from(px[i + 256]) + u32::from(px[i + 257])) / 4;
            }
        }
        let mut best = f64::INFINITY;
        for &parent in &insts {
            if parent == child { continue; }
            let pp = &tiles[&parent];
            for q in 0..4usize {
                let (qx, qy) = ((q % 2) * 128, (q / 2) * 128);
                let mut e = 0f64;
                for y in 0..128 {
                    let prow = (qy + y) * 256 + qx;
                    let crow = y * 128;
                    for x in 0..128 { e += (f64::from(pp[prow + x]) - f64::from(b[crow + x])).abs(); }
                }
                e /= 16384.0;
                if e < best { best = e; }
            }
        }
        let bi = if best < 1.0 { 0 } else if best < 5.0 { 1 } else if best < 20.0 { 2 } else if best < 60.0 { 3 } else if best < 200.0 { 4 } else { 5 };
        buckets[bi] += 1;
    }
    println!("group {group:08X}: tiles={} flat={} err buckets <1:<5:<20:<60:<200:>=200 = {:?}", insts.len(), flat, buckets);
}
