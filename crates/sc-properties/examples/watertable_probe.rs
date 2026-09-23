//! 探针：定位并解码区域 waterTableEcoMapBrushes 引用的水位面高度图。
//! 每区域 0x02A907B5 Key(instance) 指向的资源 = watertable map（0xDC097E3=128 分辨率）。
//! 验证其类型/尺寸/数值分布，并与高度图拼出「真实水面」。仅开发用。
use std::collections::HashMap;

use dbpf::Package;
use sc_properties::region_map::SHARED_TILE_GRID;
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let p = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .expect("open");

    for group in [0xBEAF_0510u32, 0xD01F_A985, 0xB12D_E348] {
        // 1. 找该区域 waterTableEcoMapBrushes 的 Key 引用
        let mut key_inst = None;
        for e in p.entries() {
            if e.id.type_id != 0x00B1_B104 || e.id.group != group {
                continue;
            }
            let Ok(data) = p.read(e) else { continue };
            let Ok(pf) = PropertyFile::parse(&data) else { continue };
            let Some(Some(name)) = pf.values.iter().find(|pr| pr.hash == 0x00B2_CCCA).map(|pr| match &pr.kind {
                Kind::Scalar(Value::String8(s)) => Some(s.clone()),
                _ => None,
            }) else { continue };
            if name == "waterTableEcoMapBrushes" {
                if let Some(sc_properties::Property {
                    kind: Kind::Array(vs), ..
                }) = pf.get(0x02A9_07B5)
                {
                    if let Some(Value::Key(k)) = vs.first() {
                        key_inst = Some(k.instance);
                    }
                }
            }
        }
        let Some(ki) = key_inst else {
            println!("{group:08X}: 未找到 waterTableEcoMapBrushes Key");
            continue;
        };
        println!("\n== {group:08X}: watertable 资源 instance={ki:08X} ==");

        // 2. 全包按 instance 找到该资源，打印 TGI 与头部
        let mut found = false;
        for e in p.entries() {
            if e.id.instance != ki {
                continue;
            }
            let d = p.read(e).unwrap_or_default();
            println!(
                "  TGI {:08X}:{:08X}:{:08X}  解压后 {} B",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                d.len()
            );
            println!("  头 32 B: {:02X?}", &d[..d.len().min(32)]);
            if d.len() == 131092 {
                // F0 形状：20 B 头 + 256×256 u16（或 128² 其它布局）
                let px: Vec<u16> = d[20..]
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                let mut mn = u16::MAX;
                let mut mx = 0u16;
                let mut zeros = 0usize;
                let mut sum = 0u64;
                for &v in &px {
                    mn = mn.min(v);
                    mx = mx.max(v);
                    if v == 0 {
                        zeros += 1;
                    }
                    sum += u64::from(v);
                }
                println!(
                    "  u16 解释: min={mn} max={mx} 均值={:.0} 零值={}_axis/{}",
                    sum as f64 / px.len() as f64,
                    zeros,
                    px.len()
                );
                // 非零值分布直方（粗）
                let mut hist = [0u64; 10];
                for &v in &px {
                    if v == 0 {
                        continue;
                    }
                    let b = ((u64::from(v)) * 10 / 65536) as usize;
                    hist[b.min(9)] += 1;
                }
                println!("  非零直方(×6553.6): {hist:?}");
            }
            found = true;
        }
        if !found {
            println!("  全包无此 instance!");
        }
    }

    // 3. 对照：城市图（F0 第 342 instance）是什么，watertable 是否就是它
    let mut f0_insts: HashMap<u32, usize> = HashMap::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21F0 {
            *f0_insts.entry(e.id.instance).or_default() += 1;
        }
    }
    let mip0: std::collections::HashSet<u32> =
        SHARED_TILE_GRID.iter().flatten().copied().collect();
    let extra: Vec<u32> = f0_insts
        .keys()
        .filter(|i| !mip0.contains(i))
        .copied()
        .collect();
    println!("\nF0 非 mip0 槽 instance: {extra:?}（每槽条目数 {:?}）", extra.iter().map(|i| f0_insts[i]).collect::<Vec<_>>());
}
