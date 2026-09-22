//! ED b1 与高度对齐检验：河床(低) vs 平原(中) vs 山地(高) 的 b1 均值，按 4 种帧变换。仅开发用。
use std::collections::HashMap;
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let ed_grid_file = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 131092 { f0.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()); }
        }
    }
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 65556 { ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()); }
        }
    }
    let mut f0g = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&args.next().unwrap()).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() { f0g[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
    }
    let mut edg = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&ed_grid_file).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() { edg[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0); }
    }
    let w = 4096usize; let wm = 2048usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in f0g.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = f0.get(i) {
                for y in 0..256 { hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256].copy_from_slice(&px[y * 256..(y + 1) * 256]); }
            }
        }
    }
    let mut edb1 = vec![0u8; wm * wm];
    for (ty, row) in edg.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = ed.get(i) {
                for y in 0..128 { for x in 0..128 { edb1[(ty * 128 + y) * wm + tx * 128 + x] = ((px[y * 128 + x] >> 8) & 0xFF) as u8; } }
            }
        }
    }
    // 四种 ED 帧变换下：三段高度的 b1 均值
    for (fx, fy, name) in [(false, false, "identity"), (false, true, "yflip"), (true, false, "xflip"), (true, true, "rot180")] {
        let mut s_r = 0f64; let mut n_r = 0f64; // riverbed h<2800
        let mut s_p = 0f64; let mut n_p = 0f64; // plains 4900-5600
        let mut s_m = 0f64; let mut n_m = 0f64; // mountains h>8000
        for y in 0..wm { for x in 0..wm {
            let mx = if fx { wm - 1 - x } else { x };
            let my = if fy { wm - 1 - y } else { y };
            let h = hgt[(my * 2) * w + mx * 2] as u32;
            let v = edb1[y * wm + x] as f64;
            if h == 0 { continue; }
            if h < 2800 { s_r += v; n_r += 1.0; }
            else if (4900..=5600).contains(&h) { s_p += v; n_p += 1.0; }
            else if h > 8000 { s_m += v; n_m += 1.0; }
        }}
        println!("frame {name}: b1 mean riverbed={:.1}({}cells) plains={:.1}({}) mountains={:.1}({})",
            s_r / n_r.max(1.0), n_r as usize, s_p / n_p.max(1.0), n_p as usize, s_m / n_m.max(1.0), n_m as usize);
    }
}
