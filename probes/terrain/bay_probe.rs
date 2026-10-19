//! 采样火山群岛"半圆海湾"区域的高度分布，确定水位下限。仅开发用。
use std::collections::HashMap;
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain1.package").unwrap();
    let group = 0x9E67_572A;
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            let d = p.read(e).unwrap();
            if d.len() == 131092 {
                tiles.insert(e.id.instance, d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect());
            }
        }
    }
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
    // 采样函数：区域内高度分位数
    let sample = |name: &str, x0: usize, y0: usize, sx: usize, sy: usize| {
        let mut vs: Vec<u32> = Vec::new();
        for y in (y0..y0 + sy).step_by(2) { for x in (x0..x0 + sx).step_by(2) {
            vs.push(u32::from(hgt[y * w + x]));
        }}
        vs.sort_unstable();
        let q = |f: f64| vs[(vs.len() as f64 * f).min((vs.len() - 1) as f64) as usize];
        println!("{name}: min={} 10%={} 50%={} 90%={} max={}", vs[0], q(0.1), q(0.5), q(0.9), vs[vs.len()-1]);
    };
    // 右岛（火山岛）西侧半圆海湾区域：右岛 x≈1900-2700, y≈1600-2300
    sample("volcano-west-bite", 1850, 1850, 300, 300);
    // 海峡（已确认应为水）
    sample("strait-center", 2150, 2050, 200, 200);
    // 岛内高原（应保持陆地）
    sample("right-island-plateau", 2300, 1700, 300, 300);
    // 外海
    sample("open-ocean", 300, 300, 400, 400);
    // 底右岛平地（游戏中为草地）
    sample("br-island-flat", 2500, 3000, 300, 300);
}
