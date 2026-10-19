//! 一次性探针：跨包查找指定 instance 的全部资源类型（decal raster 定位用）。
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let targets: Vec<u32> = args
        .iter()
        .filter_map(|a| u32::from_str_radix(a.trim_start_matches("0x"), 16).ok())
        .collect();
    for path in [
        r"D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        r"D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        r"D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        r"D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
    ] {
        let Ok(package) = Package::open(path) else { continue };
        for entry in package.entries().iter() {
            if targets.contains(&entry.id.instance) {
                println!(
                    "{}: {:08x}-{:#08x}-{:08x} ({} B)",
                    path.rsplit('/').next().unwrap_or(path),
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    entry.decompressed_size
                );
            }
        }
    }
}
