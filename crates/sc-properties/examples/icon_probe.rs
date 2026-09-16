//! 探针：警报/图层图标 PNG 实例在哪个包。
use dbpf::Package;

fn main() {
    let targets = [
        ("alert icon B5D377C5", 0xB5D3_77C5u32),
        ("layer icon D6F2F91B", 0xD6F2_F91B),
        ("layer icon 58523582", 0x5852_3582),
    ];
    let paths = [
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Graphics.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_App.package",
    ];
    for p in paths {
        let Ok(package) = Package::open(p) else { continue };
        let name = std::path::Path::new(p).file_name().and_then(|s| s.to_str()).unwrap_or(p);
        for (label, inst) in targets {
            let hits: Vec<_> = package
                .entries()
                .iter()
                .filter(|e| e.id.instance == inst)
                .map(|e| format!("{:08X}", e.id.type_id))
                .collect();
            if !hits.is_empty() {
                println!("{name}: {label} -> T={}", hits.join(","));
            }
        }
    }
}
