//! 全包扫描：解压后含 groundColors+controlMap 的资源（terrain PS 容器）。
use dbpf::Package;

fn main() -> dbpf::Result<()> {
    let roots = [
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_App.package",
    ];
    for root in roots {
        let name = root.rsplit(std::path::MAIN_SEPARATOR).next().unwrap();
        let pkg = Package::open(root)?;
        let mut n = 0;
        for e in pkg.entries() {
            if e.decompressed_size < 100_000 || e.decompressed_size > 30_000_000 { continue; }
            let body = pkg.read(e)?;
            if let Some(p) = body.windows(12).position(|w| w == b"groundColors") {
                let has_cm = body.windows(10).any(|w| w == b"controlMap");
                println!("{name} inst={:08x} type={:08x} grp={:08x} size={} groundColors@{p} controlMap={has_cm}",
                         e.id.instance, e.id.type_id, e.id.group, body.len());
                let out = format!(r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\terrain_ps_{}_{}.bin",
                                  name.trim_end_matches(".package"), e.id.instance);
                std::fs::write(&out, &body)?;
                println!("  -> {out}");
                n += 1;
                if n >= 6 { break; }
            }
        }
    }
    Ok(())
}
