//! 探针：全包定位 AutomatedRegionTemplates.json / AutomatedResources.json 资源
//! （搜 ASCII 特征串），转储命中资源全文。仅开发用。
fn main() {
    let packages = [
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
    ];
    let needles: &[&[u8]] = &[
        b"AutomatedRegionTemplates",
        b"Horizon_LoadingBackDrop",
        b"257368209",
    ];
    for pkg in packages {
        let Ok(p) = dbpf::Package::open(pkg) else { continue };
        for e in p.entries() {
            let Ok(d) = p.read(e) else { continue };
            if needles.iter().any(|n| {
                d.windows(n.len()).any(|w| w == *n)
            }) {
                println!(
                    "命中 {} TGI {:08X}:{:08X}:{:08X} 解压 {} B",
                    pkg,
                    e.id.type_id,
                    e.id.group,
                    e.id.instance,
                    d.len()
                );
                let out = format!(
                    "tmp/js/region_res_{:08X}_{:08X}.bin",
                    e.id.type_id,
                    e.id.instance
                );
                std::fs::write(&out, &d).ok();
            }
        }
    }
    println!("扫描完成");
}
