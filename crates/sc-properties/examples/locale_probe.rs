//! 探针：确认 locale 字符串表（0x0A98EAF0）在各包的分布。
use dbpf::Package;

fn main() {
    let paths = [
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Graphics.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_App.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\Locale\en-us\Data.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\Locale\zh-tw\Data.package",
    ];
    for p in paths {
        let Ok(package) = Package::open(p) else {
            println!("open fail: {p}");
            continue;
        };
        let mut locale_tables = 0u32;
        let mut has_target = false;
        for e in package.entries() {
            if e.id.type_id == 0x0A98_EAF0 {
                locale_tables += 1;
                if e.id.instance == 0x1DF185C3 {
                    has_target = true;
                }
            }
        }
        let name = std::path::Path::new(p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(p);
        println!("{name}: 0x0A98EAF0 x{locale_tables}, has table 1DF185C3: {has_target}");
    }
}
