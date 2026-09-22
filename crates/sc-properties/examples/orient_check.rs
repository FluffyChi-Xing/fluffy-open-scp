//! 定向终判：ED 道路线网 × F0 陆地 × 地块框的四种帧组合打分。仅开发用。
//!
//! 道路像素 = ED byte0 明显低于局部均值；对 ED 帧到 F0 帧的 4 种翻转，
//! 统计 road-on-land% 与 road-到-plot 距离。道路必须落在陆地并连接地块，
//! 满分帧 = 正确组合。
//!
//! 用法：orient_check <package> <region-group> <f0-grid.txt>
use std::collections::HashMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let grid_file = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();

    let read_bytes = |ty: u32| -> HashMap<u32, Vec<u8>> {
        let mut m = HashMap::new();
        for e in p.entries() {
            if e.id.type_id == ty && e.id.group == group {
                let data = p.read(e).unwrap();
                if data.len() >= 20 {
                    m.insert(e.id.instance, data[20..].to_vec());
                }
            }
        }
        m
    };
    let f0_raw = read_bytes(0x03E4_21F0);
    let ed_raw = read_bytes(0x03E4_21ED);
    let mut f0: HashMap<u32, Vec<u16>> = HashMap::new();
    for (i, d) in &f0_raw {
        if d.len() == 131072 {
            f0.insert(*i, d.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
        }
    }
    let ed: HashMap<u32, Vec<u8>> = ed_raw
        .iter()
        .filter(|(_, d)| d.len() == 65536)
        .map(|(i, d)| (*i, d.iter().enumerate().filter(|(k, _)| k % 4 == 0).map(|(_, v)| *v).collect()))
        .collect();
    println!("F0 tiles {}  ED tiles {}", f0.len(), ed.len());

    let mut f0grid = [[0u32; 16]; 16];
    for (y, line) in std::fs::read_to_string(&grid_file)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .take(16)
        .enumerate()
    {
        for (x, v) in line.split(',').take(16).enumerate() {
            f0grid[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0);
        }
    }
    let w = 4096usize;
    let mut mosaic = vec![0u16; w * w];
    for (ty, row) in f0grid.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = f0.get(inst) {
                for y in 0..256 {
                    let off = (ty * 256 + y) * w + tx * 256;
                    mosaic[off..off + 256].copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    let mut ring: Vec<u32> = Vec::new();
    for i in 0..w {
        for &j in &[0usize, 1, w - 2, w - 1] {
            ring.push(u32::from(mosaic[i * w + j]));
            ring.push(u32::from(mosaic[j * w + i]));
        }
    }
    ring.sort_unstable();
    let sea = ring[ring.len() / 2] as i32;
    println!("sea = {sea}");

    let mut plots: Vec<(f32, f32)> = Vec::new();
    if let Some(pe) = p.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == 0x51E7_A18D) {
        if let Ok(region) = sc_properties::PropertyFile::parse(&p.read(pe).unwrap()) {
            if let Some(ptk) = region.get(0xFB7A_85A0).and_then(|pp| match &pp.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Key(k)) => Some(k.instance),
                _ => None,
            }) {
                if let Some(pte) = p.entries().iter().find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == ptk) {
                    if let Ok(pt) = sc_properties::PropertyFile::parse(&p.read(pte).unwrap()) {
                        if let Some(sc_properties::Property { kind: sc_properties::Kind::Array(vs), .. }) = pt.get(0xF01D_E4B1) {
                            for v in vs {
                                if let sc_properties::Value::Vector2(v) = v {
                                    plots.push((v[0], v[1]));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    println!("plots = {}", plots.len());

    // ED 网格由 ed_render 导出（lane0+lane2 联合匹配，单根）
    let mut edgrid = [[0u32; 16]; 16];
    let ed_grid_path = grid_file.replace("grid_shared", "9F735B20_ed_grid");
    let ed_grid_path = if std::path::Path::new(&ed_grid_path).exists() {
        ed_grid_path
    } else {
        "tmp/region_preview/9F735B20_ed_grid.txt".to_string()
    };
    for (y, line) in std::fs::read_to_string(&ed_grid_path).unwrap().lines().filter(|l| !l.trim().is_empty()).take(16).enumerate() {
        for (x, v) in line.split(',').take(16).enumerate() {
            edgrid[y][x] = u32::from_str_radix(v.trim(), 16).unwrap_or(0);
        }
    }
    println!("ED grid from {}", ed_grid_path);
    let wm = 2048usize;
    let mut edmos = vec![0u8; wm * wm];
    for (ty, row) in edgrid.iter().enumerate() {
        for (tx, i) in row.iter().enumerate() {
            if let Some(d) = ed.get(i) {
                for y in 0..128 {
                    for x in 0..128 {
                        edmos[(ty * 128 + y) * wm + tx * 128 + x] = d[y * 128 + x];
                    }
                }
            }
        }
    }

    let mut road = vec![false; wm * wm];
    for y in 1..wm - 1 {
        for x in 1..wm - 1 {
            let v = i32::from(edmos[y * wm + x]);
            let avg = (i32::from(edmos[y * wm + x - 1])
                + i32::from(edmos[y * wm + x + 1])
                + i32::from(edmos[(y - 1) * wm + x])
                + i32::from(edmos[(y + 1) * wm + x]))
                / 4;
            if v < avg - 25 && v < 200 {
                road[y * wm + x] = true;
            }
        }
    }
    let road_total = road.iter().filter(|r| **r).count();
    println!("road px: {road_total}");

    for &(fx, fy, name) in &[(false, false, "identity"), (false, true, "yflip"), (true, false, "xflip"), (true, true, "rot180")] {
        let mut on_land = 0usize;
        let mut near_plot = 0usize;
        for y in 0..wm {
            for x in 0..wm {
                if !road[y * wm + x] {
                    continue;
                }
                let mx = (if fx { wm - 1 - x } else { x }) * 2;
                let my = (if fy { wm - 1 - y } else { y }) * 2;
                if mosaic[my * w + mx] as i32 > sea + 30 {
                    on_land += 1;
                }
            }
        }
        for (wx, wy) in &plots {
            let cx = ((wx + 16384.0) / 8.0) as isize;
            let cy = ((wy + 16384.0) / 8.0) as isize;
            let mut best: isize = isize::MAX;
            for y in (0..wm).step_by(2) {
                for x in (0..wm).step_by(2) {
                    if !road[y * wm + x] {
                        continue;
                    }
                    let mx = ((if fx { wm - 1 - x } else { x }) * 2) as isize;
                    let my = ((if fy { wm - 1 - y } else { y }) * 2) as isize;
                    let d = (mx - cx).pow(2) + (my - cy).pow(2);
                    if d < best {
                        best = d;
                    }
                }
            }
            if best < 400_isize.pow(2) {
                near_plot += 1;
            }
        }
        println!(
            "ED frame {name}: road-on-land = {on_land}/{road_total} ({}%)  plots within 800m of road = {near_plot}/{}",
            on_land * 100 / road_total.max(1),
            plots.len()
        );
    }
}
