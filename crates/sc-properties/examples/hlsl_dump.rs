//! 探针：转储 shader 源码容器（typeId 0x0469A3F7，ArgScript + HLSL 文本）。
//! 找水面高度/地形顶点位移的权威定义。仅开发用。
//! 输出：tmp/hlsl/<instance>.hlsl + 命中关键词清单。
use std::path::Path;

fn main() {
    let packages = [
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
    ];
    std::fs::create_dir_all("tmp/hlsl").unwrap();
    let needles = [
        "WaterLevel",
        "waterLevel",
        "water_level",
        "WaterHeight",
        "waterHeight",
        "SeaLevel",
        "seaLevel",
        "gWater",
        "WaterDepth",
        "TerrainVS",
        "terrainVS",
        "HeightMap",
        "heightMap",
        "Tessendorf",
        "tessendorf",
    ];
    for pkg_path in packages {
        let Ok(p) = dbpf::Package::open(pkg_path) else {
            println!("跳过 {pkg_path}");
            continue;
        };
        let name = Path::new(pkg_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        for e in p.entries() {
            if e.id.type_id != 0x0469_A3F7 {
                continue;
            }
            let d = match p.read(e) {
                Ok(d) => d,
                Err(err) => {
                    println!("{name} inst={:08X} 读取失败 {err}", e.id.instance);
                    continue;
                }
            };
            // 抽取可打印 ASCII 文本段（HLSL/ArgScript 源码段）
            let mut text = String::new();
            let mut run = Vec::new();
            for &b in &d {
                if (0x20..0x7f).contains(&b) || b == b'\n' || b == b'\r' || b == b'\t' {
                    run.push(b);
                } else {
                    if run.len() > 64 {
                        text.push_str(&String::from_utf8_lossy(&run));
                        text.push('\n');
                    }
                    run.clear();
                }
            }
            if run.len() > 64 {
                text.push_str(&String::from_utf8_lossy(&run));
            }
            let hits: Vec<&str> = needles
                .iter()
                .filter(|n| text.contains(*n))
                .copied()
                .collect();
            let out = format!(
                "tmp/hlsl/{:08X}_{:08X}_{}.txt",
                e.id.group,
                e.id.instance,
                name.replace(".package", "")
            );
            std::fs::write(&out, &text).ok();
            println!(
                "{} inst={:08X}: {} B 容器，文本 {} KB，命中 {:?} -> {}",
                name,
                e.id.instance,
                d.len(),
                text.len() / 1024,
                hits,
                out
            );
        }
    }
}
