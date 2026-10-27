//! 探针：全包扫描文本资源（JS 脚本等）中的 waterLevel / seaLevel / water level 引用。
//! 目标：找引擎给 mSeaLevel/mWaterLevel uniform 赋值的脚本侧权威来源。仅开发用。
use std::path::Path;

fn main() {
    let packages = [
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
        "D:/ea-games/SimCity/SimCityUserData/EcoGame/Sandbox-Scripts_287520926.package",
        "D:/ea-games/SimCity/SimCityUserData/EcoGame/HeroesAndVillains-Scripts_277156632.package",
    ];
    let needles = [
        "waterlevel".to_string(),
        "sealevel".to_string(),
        "water_level".to_string(),
        "sea_level".to_string(),
    ];
    for pkg_path in packages {
        if !Path::new(pkg_path).exists() {
            println!("不存在 {pkg_path}");
            continue;
        }
        let Ok(p) = dbpf::Package::open(pkg_path) else {
            println!("打不开 {pkg_path}");
            continue;
        };
        let name = Path::new(pkg_path).file_name().unwrap().to_string_lossy().to_string();
        let mut hits = 0usize;
        for e in p.entries() {
            let Ok(d) = p.read(e) else { continue };
            if d.len() < 16 || d.len() > 20_000_000 {
                continue;
            }
            // 快速预筛：转小写搜（直接在字节上做 ASCII 小写化）
            let lower: Vec<u8> = d.iter().map(|b| b.to_ascii_lowercase()).collect();
            let Some(pos) = needles.iter().find_map(|n| {
                lower.windows(n.len()).position(|w| w == n.as_bytes())
            }) else {
                continue;
            };
            hits += 1;
            // 打印命中上下文
            let ctx = &d[pos.saturating_sub(120)..(pos + 160).min(d.len())];
            let ctx_text: String = ctx
                .iter()
                .map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { ' ' })
                .collect();
            println!(
                "{name} TGI {:08X}:{:08X}:{:08X} size={} 命中@{}: …{}…",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                d.len(),
                pos,
                ctx_text.replace('\n', " ")
            );
            if hits > 40 {
                println!("{name}: 命中过多，截断");
                break;
            }
        }
        println!("{name}: {hits} 条命中");
    }
}
