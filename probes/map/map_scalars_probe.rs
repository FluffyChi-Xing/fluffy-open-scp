//! 地图标量与灯光表普查：从真实安装包读取 `mFlatScale` / 高度缩放与灯光表。
//!
//! 依据（dev 样本反编译）：
//! - `SC::cTerrainGfx::FillFromProps` 从 config 属性 `0xB8F6B4CF` 调
//!   `SetFlatScale`，`0x95887CE7` → `SetHeightMapScale`，`0x91EECE40` →
//!   `SetHeightMapBias`。
//! - `SC::FillLightInfoMap` 以表属性 `0x0CAA8F10`（灯光类型）驱动 `cLightInfo`
//!   生成，列 `0x0CAA8F10..0x0CAA8F16` + `0x0D4C96B4` / `0x0D4CAD23` 等。
//!
//! 用法：`cargo run -p probes --release --example map_scalars_probe -- <SimCityData>`

const FLAT_SCALE: u32 = 0xB8F6_B4CF;
const HEIGHT_SCALE: u32 = 0x9588_7CE7;
const HEIGHT_BIAS: u32 = 0x91EE_CE40;
const LIGHT_TYPE: u32 = 0x0CAA_8F10;

fn main() {
    let dir = std::env::args().nth(1).unwrap_or_else(|| {
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData".into()
    });
    let mut packages: Vec<_> = std::fs::read_dir(&dir)
        .expect("SimCityData")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "package"))
        .collect();
    packages.sort();

    let mut seen = std::collections::BTreeMap::<u32, Vec<String>>::new();
    let mut light_hosts = 0usize;
    let mut light_tables = 0usize;
    for path in &packages {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let Ok(pkg) = dbpf::Package::open(path) else {
            continue;
        };
        for entry in pkg.entries() {
            if entry.id.type_id != 0x00B1_B104 {
                continue;
            }
            let Ok(bytes) = pkg.read(entry) else { continue };
            let Ok(props) = sc_properties::PropertyFile::parse(&bytes) else {
                continue;
            };
            if props.get(LIGHT_TYPE).is_some() {
                light_tables += 1;
                if light_tables <= 5 {
                    println!(
                        "light table: {name} {:08X}:{:08X}:{:08X} entries={}",
                        entry.id.type_id,
                        entry.id.group,
                        entry.id.instance,
                        props.values.len()
                    );
                }
            } else if props
                .values
                .iter()
                .any(|p| p.hash == LIGHT_TYPE || p.hash == 0x0D4C_96B4)
            {
                light_hosts += 1;
            }
            for (hash, label) in [
                (FLAT_SCALE, "flatScale"),
                (HEIGHT_SCALE, "heightScale"),
                (HEIGHT_BIAS, "heightBias"),
                // `cEcoGameScripts::Init`：cellBounds 数组 → mMapBounds；
                // 缺省时用 `1 << addOn`（addOn 默认 5）。
                (0x0D4A_E7FE, "cellBounds"),
                (0x086C_1E0F, "ecoAddOn"),
            ] {
                let Some(prop) = props.get(hash) else {
                    continue;
                };
                let value = match &prop.kind {
                    sc_properties::Kind::Scalar(sc_properties::Value::Float(v)) => format!("{v}"),
                    sc_properties::Kind::Scalar(v) => format!("{v}"),
                    sc_properties::Kind::Array(items) => {
                        let head: Vec<String> =
                            items.iter().take(4).map(|v| v.to_string()).collect();
                        format!("array[{}] [{}]", items.len(), head.join(", "))
                    }
                    other => format!("{other:?}"),
                };
                seen.entry(hash).or_default().push(format!(
                    "{label} {name} {:08X}:{:08X}:{:08X} = {value}",
                    entry.id.type_id, entry.id.group, entry.id.instance
                ));
            }
        }
    }
    for (hash, rows) in &seen {
        println!("\n== 0x{hash:08X} ({} 处) ==", rows.len());
        for row in rows.iter().take(8) {
            println!("  {row}");
        }
    }
    println!("\nlight tables={light_tables} light hosts={light_hosts}");
}
