//! 探针：ED 金字塔网格跨区域一致性 + ED/F0 槽位关系 + 水面子资源属性。
//! 仅开发用。回答：
//!   1. ED 341 槽位与 F0 342 唯一 instance 的重叠度；
//!   2. 各大组独立求解的 ED 网格是否逐槽一致（一致 → 网格全局唯一，可解一次复用）；
//!   3. tessendorfWater / watertable* 子资源属性转储（找每区域水位参数）。
use std::collections::HashMap;
use std::collections::HashSet;

use dbpf::Package;
use sc_properties::region_map::{ed_grid_for_region, SHARED_TILE_GRID};
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open RT0");

    // ---- 1. ED 槽位 vs F0 槽位 ----
    let mut f0_insts: HashSet<u32> = HashSet::new();
    let mut ed_insts: HashSet<u32> = HashSet::new();
    for e in p.entries() {
        match e.id.type_id {
            0x03E4_21F0 => {
                f0_insts.insert(e.id.instance);
            }
            0x03E4_21ED => {
                ed_insts.insert(e.id.instance);
            }
            _ => {}
        }
    }
    println!(
        "F0 unique={} ED unique={} 交集={}（ED−F0={}，F0−ED={}）",
        f0_insts.len(),
        ed_insts.len(),
        f0_insts.intersection(&ed_insts).count(),
        ed_insts.difference(&f0_insts).count(),
        f0_insts.difference(&ed_insts).count()
    );
    let f0_mip0: HashSet<u32> = SHARED_TILE_GRID.iter().flatten().copied().collect();
    println!(
        "ED ∩ F0_mip0 = {}（ED 全集 {}）",
        ed_insts.intersection(&f0_mip0).count(),
        ed_insts.len()
    );

    // ---- 2. 各大组独立解 ED 网格，互相比对 ----
    let groups = [
        0xBEAF_0510u32, // 白水谷（基准）
        0x9F73_5B20,    // 绵延不毛之地
        0xB12D_E348,
        0xD01F_A985,
        0xBC35_7A2B,
        0xA0B6_0DDE,
    ];
    let mut grids: HashMap<u32, [[u32; 16]; 16]> = HashMap::new();
    for g in groups {
        let t0 = std::time::Instant::now();
        let grid = ed_grid_for_region(&p, g);
        let ms = t0.elapsed().as_millis();
        match grid {
            Some(grid) => {
                let filled: usize = grid.iter().flatten().filter(|v| **v != 0).count();
                println!("{g:08X}: ok {ms}ms mip0 槽 {filled}/256");
                grids.insert(g, grid);
            }
            None => println!("{g:08X}: 失败 {ms}ms"),
        }
    }
    // 与白水谷网格逐槽比对
    if let Some(base) = grids.get(&0xBEAF_0510) {
        for (g, grid) in &grids {
            if *g == 0xBEAF_0510 {
                continue;
            }
            let diff = grid
                .iter()
                .zip(base.iter())
                .map(|(a, b)| a.iter().zip(b.iter()).filter(|(x, y)| x != y).count())
                .sum::<usize>();
            println!("网格一致性 {g:08X} vs BEAF0510: 逐槽差异 {diff}/256");
        }
    }

    // ---- 3. 水面子资源属性转储（白水谷 vs D01FA985）----
    for g in [0xBEAF_0510u32, 0xD01F_A985] {
        println!("\n== group {g:08X} 水相关子资源 ==");
        for e in p.entries() {
            if e.id.type_id != 0x00B1_B104 || e.id.group != g {
                continue;
            }
            let Ok(data) = p.read(e) else { continue };
            let Ok(pf) = PropertyFile::parse(&data) else { continue };
            let Some(Some(name)) = pf.values.iter().find(|pr| pr.hash == 0x00B2_CCCA).map(|pr| match &pr.kind {
                Kind::Scalar(Value::String8(s)) => Some(s.clone()),
                _ => None,
            }) else { continue };
            if !(name.contains("water") || name.contains("Water") || name.contains("tessendorf")) {
                continue;
            }
            println!("  -- instance {:08X} name=\"{name}\"（{} 键）--", e.id.instance, pf.values.len());
            for pr in &pf.values {
                let brief = match &pr.kind {
                    Kind::Scalar(v) => format!("{v:?}"),
                    Kind::Array(vs) => {
                        let show: Vec<String> = vs.iter().take(6).map(|v| format!("{v:?}")).collect();
                        format!("[{};{}]", show.join(","), vs.len())
                    }
                    Kind::Empty => "(empty)".to_string(),
                };
                println!("    {:08X}: {}", pr.hash, brief);
            }
        }
    }
}
