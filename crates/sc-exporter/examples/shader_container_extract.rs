//! 定位并全量提取 shader 源码容器 0x0469A3F7（ArgScript HLSL 片段库）。
use dbpf::Package;

const TARGET_TYPE: u32 = 0x0469A3F7;

fn main() -> dbpf::Result<()> {
    let roots = [
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Game.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_App.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Graphics.package",
        r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_DLC0.package",
    ];
    for root in roots {
        let name = root.rsplit(std::path::MAIN_SEPARATOR).next().unwrap();
        let pkg = match Package::open(root) {
            Ok(p) => p,
            Err(e) => { eprintln!("{name}: {e:?}"); continue; }
        };
        for e in pkg.entries() {
            if e.id.type_id == TARGET_TYPE {
                let body = pkg.read(e)?;
                println!("{name}: inst={:08x} grp={:08x} size={}", e.id.instance, e.id.group, body.len());
                let out = format!(
                    r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\shader_container_{}_g{:08x}_{}.bin",
                    name.trim_end_matches(".package"),
                    e.id.group,
                    e.id.instance
                );
                std::fs::write(&out, &body)?;
                println!("  -> {out}");
            }
        }
    }
    Ok(())
}
