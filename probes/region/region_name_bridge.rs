//! 探针：区域名称桥全链路。
//! ① 转储 11 个区域组的数字 UI id（0x4EE97F5B）；
//! ② 遍历 Locale/zh-tw 包，找 JSON 资源（0x0A98EAF0），转储含区域名的条目；
//! ③ 用 20 个 JS 模板 regionName hash 在 locale JSON 中查文本。
//! 仅开发用。
use std::collections::HashMap;

use dbpf::Package;

const TEMPLATE_HASHES: &[(u32, &str)] = &[
    (0x8e4a_59b0, "CapeTrinity"),
    (0xb667_b644, "CaspianLake"),
    (0xb14b_defd, "Caspiar"),
    (0x2a6b_0baa, "CliffsideVista"),
    (0x77cb_42ea, "Confluence/白水谷"),
    (0x6e0b_7d6c, "Desolation"),
    (0xda0a_b09c, "EdgeWaterBay"),
    (0xcab3_f796, "FlocksRiver"),
    (0xfd40_11aa, "Gallia"),
    (0x951c_174f, "Horizon"),
    (0xd29a_65a2, "LittleGorge"),
    (0x8b19_533d, "NewDenmont"),
    (0x17ff_e79d, "Oasis"),
    (0xc182_2f57, "Reflection"),
    (0x6e13_2f95, "Sawyer"),
    (0xd6a3_db4d, "SC_Hinsara"),
    (0x9d8e_fd24, "Tundra"),
    (0xbd21_955e, "Tutorial"),
    (0x617c_4d77, "TwinCities"),
];

fn main() {
    // ---- ① 数字 UI id ----
    let rt = Package::open("D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package")
        .unwrap();
    println!("-- 区域组 → 数字 UI id --");
    for e in rt.entries() {
        if e.id.type_id != 0x00B1_B104 || e.id.instance != 0x51E7_A18D {
            continue;
        }
        let Ok(data) = rt.read(e) else { continue };
        let Ok(pf) = sc_properties::PropertyFile::parse(&data) else { continue };
        let num = pf.get(0x4EE9_7F5B).and_then(|p| match &p.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::String8(s)) => Some(s.clone()),
            _ => None,
        });
        println!("  {:08X} → {:?}", e.id.group, num);
    }

    // ---- ②③ Locale JSON ----
    let loc = Package::open("D:/ea-games/SimCity/SimCityData/Locale/zh-tw/Data.package").unwrap();
    println!("\n-- Locale 包资源类型普查 --");
    let mut type_counts: HashMap<u32, usize> = HashMap::new();
    for e in loc.entries() {
        *type_counts.entry(e.id.type_id).or_default() += 1;
    }
    let mut tc: Vec<(u32, usize)> = type_counts.into_iter().collect();
    tc.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (t, n) in tc.iter().take(10) {
        println!("  {:08X}: {n}", t);
    }
    // 全条目解压搜模板 hash 的十六进制形式 + 区域名文本
    println!("\n-- Locale 条目内区域 hash/名称命中 --");
    let mut checked = 0usize;
    for e in loc.entries() {
        let Ok(d) = loc.read(e) else { continue };
        checked += 1;
        let text = String::from_utf8_lossy(&d);
        let mut hits: Vec<&str> = Vec::new();
        for (h, name) in TEMPLATE_HASHES {
            // hash 可能以十进制或十六进制出现
            if text.contains(&format!("{h}")) || text.contains(&format!("{h:08x}")) || text.contains(&format!("{h:08X}")) {
                hits.push(name);
            }
        }
        if !hits.is_empty() || text.contains("白水谷") || text.contains("地平线") {
            let snippet: String = text.chars().take(200).collect();
            println!(
                "  TGI {:08X}:{:08X}:{:08X} ({} B) 命中 {:?}: {}",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                d.len(),
                hits,
                snippet.replace('\n', " ")
            );
        }
    }
    println!("共扫描 {checked} 条");
}
