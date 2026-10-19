//! ED 三通道 × 帧假设 × 水陆分类 检验。仅开发用。
//!
//! 用法：ed_align3 <package> <region-group> <ed-grid.txt>
//! 分类（按 F0 高度 + 水位 3336）：ocean(h<300) / low-land(300..3336，游戏应为水或洼地) / land(>3336)
//! 对 identity / rot180 两种 ED 帧假设，输出每类 b0/b1/b2 均值。
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let ed_grid_file = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();

    let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
    let mut ed: HashMap<u32, Vec<u32>> = HashMap::new();
    for e in p.entries() {
        if e.id.group != group {
            continue;
        }
        match e.id.type_id {
            0x03E4_21F0 => {
                let d = p.read(e).unwrap();
                if d.len() == 131092 {
                    f0.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
                }
            }
            0x03E4_21ED => {
                let d = p.read(e).unwrap();
                if d.len() == 65556 {
                    ed.insert(e.id.instance, d[20..].chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect());
                }
            }
            _ => {}
        }
    }

    let mut f0g = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string("tmp/region_preview/grid_shared.txt").unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() {
            f0g[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0);
        }
    }
    let mut edg = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&ed_grid_file).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() {
            edg[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0);
        }
    }

    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in f0g.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = f0.get(i) {
                for y in 0..256 {
                    hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256].copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    let wm = 2048usize;
    let mut edpx = vec![[0u32; 3]; wm * wm];
    for (ty, row) in edg.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(px) = ed.get(i) {
                for y in 0..128 {
                    for x in 0..128 {
                        let v = px[y * 128 + x];
                        edpx[(ty * 128 + y) * wm + tx * 128 + x] = [v & 0xFF, (v >> 8) & 0xFF, (v >> 16) & 0xFF];
                    }
                }
            }
        }
    }

    for (fx, fy, name) in [(false, false, "identity"), (true, true, "rot180")] {
        // class 0 = ocean(h<300), 1 = low(300..3336), 2 = land(>=3336)
        let mut acc = [[0f64; 3]; 3];
        let mut cnt = [[0f64; 3]; 3];
        for y in 0..wm {
            for x in 0..wm {
                let mx = if fx { wm - 1 - x } else { x };
                let my = if fy { wm - 1 - y } else { y };
                let h = hgt[(my * 2) * w + mx * 2] as u32;
                if h == 0 {
                    continue;
                }
                let cls = if h < 300 {
                    0usize
                } else if h < 3336 {
                    1
                } else {
                    2
                };
                let v = &edpx[y * wm + x];
                for ch in 0..3 {
                    acc[cls][ch] += v[ch] as f64;
                    cnt[cls][ch] += 1.0;
                }
            }
        }
        for (cls, cname) in [(0usize, "ocean<300     "), (1usize, "low 300..3336 "), (2usize, "land>=3336    ")] {
            println!(
                "frame {name} {cname}: b0={:6.1} b1={:6.1} b2={:6.1}  ({} cells)",
                acc[cls][0] / cnt[cls][0].max(1.0),
                acc[cls][1] / cnt[cls][1].max(1.0),
                acc[cls][2] / cnt[cls][2].max(1.0),
                cnt[cls][0] as usize
            );
        }
    }
}
