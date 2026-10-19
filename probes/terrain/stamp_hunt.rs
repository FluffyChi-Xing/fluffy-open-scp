//! 探针：全类型、全包按 instance 搜 eco/terrain 画刷 stamp 资源。仅开发用。
fn main() {
    let stamps: &[(&str, u32)] = &[
        ("白水谷.oil", 0x0325_583A),
        ("白水谷.soil", 0xA4B5_6FF7),
        ("白水谷.water", 0x973B_ADEB),
        ("白水谷.forest", 0x2C3D_D165),
        ("白水谷.desir", 0xB968_C891),
        ("白水谷.ore", 0xFA25_4A4E),
        ("白水谷.coal", 0x4C5D_671F),
        ("地平.oil", 0xB8D6_0017),
        ("地平.soil", 0x5206_08E4),
        ("地平.water", 0x7BEB_1C3A),
        ("地平.forest", 0x6A8C_F97E),
        ("地平.ore", 0xA1D5_DC23),
        ("地平.coal", 0xF7D2_FB4C),
        ("地形刷共享", 0x2820_78BF),
        ("白水谷.地形刷", 0xB55C_E7DD),
    ];
    let packages = [
        "D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/SimCity_RegionTerrain0.package",
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/SimCity_RegionTerrain1.package",
    ];
    for (label, inst) in stamps {
        let mut found = false;
        for pkg in packages {
            let Ok(p) = dbpf::Package::open(pkg) else { continue };
            for e in p.entries() {
                if e.id.instance != *inst {
                    continue;
                }
                let d = p.read(e).map(|d| d.len()).unwrap_or(0);
                println!(
                    "{label:<14} {inst:08X} -> {} {:08X}:{:08X} 压缩{} 解压{d}B",
                    pkg.rsplit('/').next().unwrap(),
                    e.id.type_id,
                    e.id.group,
                    e.compressed_size
                );
                found = true;
            }
        }
        if !found {
            println!("{label:<14} {inst:08X} -> （全部包无此 instance）");
        }
    }
}
