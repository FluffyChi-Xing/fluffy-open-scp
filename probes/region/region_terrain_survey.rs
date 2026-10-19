//! 探针：RegionTerrain0 包 B12DE348（ED 金字塔求解失败）/ D01FA985（渲染异常）诊断。
//! 仅开发用。输出：
//!   1. 全包 F0/ED 按 group 的条目普查（识别 11 大区域组）；
//!   2. 两组的 F0 tile 尺寸直方图 + 相对 SHARED_TILE_GRID 的 mip0 槽位缺口；
//!   3. ED 槽位 id 是否跨区域共享（决定能否全局解一次网格）；
//!   4. 两组 ED 金字塔贪心匹配诊断（接受数/根数/误差分布）；
//!   5. D01FA985 拼合高度统计（水位 3336 覆盖率）+ region desc 属性转储。
use std::collections::HashMap;
use std::collections::HashSet;

use dbpf::Package;
use sc_properties::region_map::{ed_grid_for_region, SHARED_TILE_GRID};
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open RT0");
    let focus = [0xB12D_E348u32, 0xD01F_A985u32];

    // ---- 1. 全包普查（只看索引，不解压）----
    let mut f0_by_group: HashMap<u32, usize> = HashMap::new();
    let mut ed_by_group: HashMap<u32, usize> = HashMap::new();
    let mut ed_insts_by_group: HashMap<u32, HashSet<u32>> = HashMap::new();
    let mut f0_insts: HashSet<u32> = HashSet::new();
    for e in p.entries() {
        match e.id.type_id {
            0x03E4_21F0 => {
                *f0_by_group.entry(e.id.group).or_default() += 1;
                f0_insts.insert(e.id.instance);
            }
            0x03E4_21ED => {
                *ed_by_group.entry(e.id.group).or_default() += 1;
                ed_insts_by_group
                    .entry(e.id.group)
                    .or_default()
                    .insert(e.id.instance);
            }
            _ => {}
        }
    }
    let mut big_f0: Vec<(u32, usize)> = f0_by_group
        .iter()
        .filter(|(_, n)| **n >= 300)
        .map(|(g, n)| (*g, *n))
        .collect();
    big_f0.sort_by_key(|(g, _)| *g);
    println!("== 大区域组（F0 >= 300 条） ==");
    for (g, n) in &big_f0 {
        let edn = ed_by_group.get(g).unwrap_or(&0);
        let edinst = ed_insts_by_group.get(g).map(|s| s.len()).unwrap_or(0);
        println!("  {g:08X}: F0={n} ED={edn} ED_unique_inst={edinst}");
    }
    let f0_mip0: HashSet<u32> = SHARED_TILE_GRID.iter().flatten().copied().collect();
    println!(
        "F0 全包唯一 instance={}（mip0 槽 {}，超出 {}）",
        f0_insts.len(),
        f0_mip0.len(),
        f0_insts.iter().filter(|i| !f0_mip0.contains(i)).count()
    );
    // ED 槽位跨区域共享性：与第一个大组的实例集合比对
    let mut ed_ref: Option<HashSet<u32>> = None;
    let mut ed_shared = true;
    for (g, _) in &big_f0 {
        let s = ed_insts_by_group.get(g).cloned().unwrap_or_default();
        match &ed_ref {
            None => ed_ref = Some(s.clone()),
            Some(r) => {
                if &s != r {
                    ed_shared = false;
                    println!(
                        "  ED 槽位不共享: {g:08X} vs 参照组 差异 {} / {}",
                        s.symmetric_difference(r).count(),
                        s.len().max(r.len())
                    );
                }
            }
        }
    }
    println!("ED 槽位跨大组共享 = {ed_shared}");

    // ---- 2. 两组 F0 tile 深检 ----
    for &group in &focus {
        println!("\n== F0 group {group:08X} ==");
        let mut sizes: HashMap<usize, usize> = HashMap::new();
        let mut insts: HashSet<u32> = HashSet::new();
        let mut dup = 0usize;
        for e in p.entries() {
            if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
                let d = match p.read(e) {
                    Ok(d) => d,
                    Err(err) => {
                        println!("  read 失败 inst={:08X}: {err}", e.id.instance);
                        continue;
                    }
                };
                *sizes.entry(d.len()).or_default() += 1;
                if !insts.insert(e.id.instance) {
                    dup += 1;
                }
            }
        }
        let mut size_vec: Vec<(usize, usize)> = sizes.into_iter().collect();
        size_vec.sort();
        println!("  尺寸直方图: {size_vec:?} 重复instance={dup}");
        let missing: Vec<u32> = f0_mip0.iter().filter(|i| !insts.contains(i)).copied().collect();
        println!(
            "  mip0 槽位 {}/256，缺失 {} 个 {:?}",
            insts.intersection(&f0_mip0).count(),
            missing.len(),
            &missing[..missing.len().min(8)]
        );
    }

    // ---- 3. ED 金字塔诊断 ----
    for &group in &focus {
        println!("\n== ED group {group:08X} ==");
        let mut tiles: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut sizes: HashMap<usize, usize> = HashMap::new();
        for e in p.entries() {
            if e.id.type_id == 0x03E4_21ED && e.id.group == group {
                let d = p.read(e).unwrap_or_default();
                *sizes.entry(d.len()).or_default() += 1;
                if d.len() == 65556 {
                    tiles.insert(
                        e.id.instance,
                        d[20..]
                            .chunks_exact(4)
                            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                            .collect(),
                    );
                }
            }
        }
        let mut size_vec: Vec<(usize, usize)> = sizes.into_iter().collect();
        size_vec.sort();
        println!("  尺寸直方图: {size_vec:?}");
        let grid = ed_grid_for_region(&p, group);
        println!("  ed_grid_for_region = {}", grid.is_some());
        if grid.is_none() {
            // 复刻贪心，输出误差分布与树规模
            let insts: Vec<u32> = tiles.keys().copied().collect();
            let mut ds: HashMap<u32, Vec<(u32, u32)>> = HashMap::new();
            for (&i, px) in &tiles {
                let mut b = vec![(0u32, 0u32); 64 * 64];
                for y in 0..64 {
                    for x in 0..64 {
                        let k = (2 * y) * 128 + 2 * x;
                        let lane = |v: u32, s: u32| ((v >> (s * 8)) & 0xFF) as u32;
                        b[y * 64 + x] = (
                            (lane(px[k], 0) + lane(px[k + 1], 0) + lane(px[k + 128], 0) + lane(px[k + 129], 0)) / 4,
                            (lane(px[k], 2) + lane(px[k + 1], 2) + lane(px[k + 128], 2) + lane(px[k + 129], 2)) / 4,
                        );
                    }
                }
                ds.insert(i, b);
            }
            let mut errs: Vec<f64> = Vec::new();
            let mut accepted = 0usize;
            for &child in &insts {
                let cb = &ds[&child];
                let mut best = f64::INFINITY;
                for &parent in &insts {
                    if parent == child { continue; }
                    let pp = &tiles[&parent];
                    for q in 0..4usize {
                        let (qx, qy) = ((q % 2) * 64, (q / 2) * 64);
                        let mut e = 0f64;
                        for y in 0..64 {
                            let prow = (qy + y) * 128 + qx;
                            let crow = y * 64;
                            for x in 0..64 {
                                let pv = pp[prow + x];
                                let ce = cb[crow + x];
                                e += (f64::from((pv & 0xFF) as u8) - f64::from(ce.0 as u8)).abs()
                                    + (f64::from(((pv >> 16) & 0xFF) as u8) - f64::from(ce.1 as u8)).abs();
                            }
                        }
                        e /= 4096.0;
                        if e < best {
                            best = e;
                        }
                    }
                }
                errs.push(best);
                if best <= 60.0 {
                    accepted += 1;
                }
            }
            errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let n = errs.len();
            println!(
                "  每子块最优误差: min={:.2} p25={:.2} 中位={:.2} p75={:.2} p95={:.2} max={:.2}",
                errs.first().copied().unwrap_or(0.0),
                errs[n / 4],
                errs[n / 2],
                errs[n * 3 / 4],
                errs[n * 95 / 100],
                errs[n - 1]
            );
            println!("  最优误差<=60 的子块: {accepted}/{}", insts.len());
        }
    }

    // ---- 4. D01FA985 高度统计 + region desc ----
    let group = 0xD01F_A985u32;
    println!("\n== D01FA985 高度统计 ==");
    let mut tiles: HashMap<u32, Vec<u16>> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 && e.id.group == group {
            if let Ok(d) = p.read(e) {
                if d.len() == 131092 {
                    tiles.insert(
                        e.id.instance,
                        d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect(),
                    );
                }
            }
        }
    }
    let w = 4096usize;
    let mut hgt = vec![0u16; w * w];
    for (ty, row) in SHARED_TILE_GRID.iter().enumerate() {
        for (tx, inst) in row.iter().enumerate() {
            if let Some(px) = tiles.get(inst) {
                for y in 0..256 {
                    hgt[(ty * 256 + y) * w + tx * 256..(ty * 256 + y) * w + tx * 256 + 256]
                        .copy_from_slice(&px[y * 256..(y + 1) * 256]);
                }
            }
        }
    }
    let sea = 3336i64;
    let mut below = 0u64;
    let mut zero = 0u64;
    let mut total = 0u64;
    let mut mn = u16::MAX;
    let mut mx = 0u16;
    let mut hist = [0u64; 9];
    for y in (0..w).step_by(4) {
        for x in (0..w).step_by(4) {
            let h = hgt[y * w + x];
            total += 1;
            if h == 0 {
                zero += 1;
            }
            if (h as i64) < sea {
                below += 1;
            }
            mn = mn.min(h);
            mx = mx.max(h);
            let bucket = ((h as i64 - 2000).max(0) / 2000).min(8) as usize;
            hist[bucket] += 1;
        }
    }
    println!(
        "  min={mn} max={mx} <3336 占 {:.1}%  h==0 占 {:.1}%（0/千分位）",
        below as f64 * 100.0 / total as f64,
        zero as f64 * 1000.0 / total as f64
    );
    println!("  高度直方 [2000+2k*n ..]: {hist:?}");

    // region desc（34 键）转储
    println!("\n== D01FA985 / B12DE348 region desc（0x51E7A18D）==");
    for &group in &focus {
        for e in p.entries() {
            if e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == 0x51E7_A18D {
                let Ok(data) = p.read(e) else { continue };
                let Ok(pf) = PropertyFile::parse(&data) else { continue };
                println!("  -- {group:08X}（{} 键）--", pf.values.len());
                for prop in &pf.values {
                    let brief = match &prop.kind {
                        Kind::Scalar(v) => format!("{v:?}"),
                        Kind::Array(vs) => {
                            let show: Vec<String> = vs.iter().take(4).map(|v| format!("{v:?}")).collect();
                            format!("[{};{}]", show.join(","), vs.len())
                        }
                        Kind::Empty => "(empty)".to_string(),
                    };
                    println!("    {:08X}: {}", prop.hash, truncate(&brief, 110));
                }
            }
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}
