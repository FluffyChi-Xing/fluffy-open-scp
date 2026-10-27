//! 探针：定位引擎地面 cube 细节纹理（实际为 type ED + instance=FNV1_lower(名)、group=0）。
//! 依次尝试 GraphicsCache/Cache/Graphics 等包。仅开发用。
fn main() {
    let paths = [
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityUserData/GraphicsCache.package",
        "D:/ea-games/SimCity/SimCityUserData/Cache.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
    ];
    let names = [
        "terrain_grassdetail",
        "terrain_dirtdetail",
        "terrain_cliffx",
        "terrain_snow",
        "terrain_beach",
        "terrain_pollution",
    ];
    for p in paths {
        let Ok(g) = dbpf::Package::open(p) else { continue };
        let mut found = 0;
        for n in names {
            let group = sc_properties::region_3d::fnv1_lower(n);
            for e in g.entries() {
                if e.id.instance == group || e.id.group == group {
                    let d = g.read(e).unwrap_or_default();
                    println!(
                        "[{}] {} ({group:08X}) inst={:08X} {}B 头: {:02X?}",
                        p.rsplit('/').next().unwrap(),
                        n,
                        e.id.instance,
                        d.len(),
                        &d[..d.len().min(20)]
                    );
                    let out = format!("tmp/gtex_{}.bin", n.trim_start_matches("terrain_"));
                    std::fs::create_dir_all("tmp").ok();
                    std::fs::write(out, &d).ok();
                    found += 1;
                }
            }
        }
        println!("[{}] 命中 {found}/6", p.rsplit('/').next().unwrap());
        if found >= 6 {
            return;
        }
    }
    println!("全部包未命中");
}
