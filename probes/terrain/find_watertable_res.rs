//! 探针：在全部游戏包中按 instance 定位 watertableheightmap 引用的资源并解码。仅开发用。
fn main() {
    let targets = [0x973B_ADEBu32, 0x7BEB_1C3A, 0x9D44_E61B];
    let packages = [
        "D:/ea-games/SimCity/SimCityData/SimCity_Game.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_App.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain0.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_RegionTerrain1.package",
        "D:/ea-games/SimCity/SimCityData/SimCityDataEP1.package",
        "D:/ea-games/SimCity/SimCityData/SimCity_DLC0.package",
    ];
    for pkg in packages {
        let Ok(p) = dbpf::Package::open(pkg) else { continue };
        for e in p.entries() {
            if targets.contains(&e.id.instance) {
                let d = p.read(e).unwrap_or_default();
                println!(
                    "{}\n  TGI {:08X}:{:08X}:{:08X} 压缩 {} 解压 {} B",
                    pkg,
                    e.id.type_id,
                    e.id.group,
                    e.id.instance,
                    e.compressed_size,
                    d.len()
                );
                println!("  头 24 B: {:02x?}", &d[..d.len().min(24)]);
                if d.len() >= 24 {
                    // 各种解释的统计
                    let body = &d[20..];
                    let n16 = body.len() / 2;
                    if n16 > 0 {
                        let px: Vec<u16> =
                            body.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                        let mut mn = u16::MAX;
                        let mut mx = 0u16;
                        let mut sum = 0u64;
                        for &v in &px {
                            mn = mn.min(v);
                            mx = mx.max(v);
                            sum += u64::from(v);
                        }
                        println!(
                            "  u16(偏移20): min={mn} max={mx} 均值={:.0}",
                            sum as f64 / n16 as f64
                        );
                    }
                }
            }
        }
    }
}
