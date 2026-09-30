//! 冒烟工具：复刻 package_service::resolve_decal_textures 的分派逻辑，
//! 逐条目打印 raw/四色解码结局（诊断「贴花退化为 gizmo」）。
//! 用法：cargo run -p sc-exporter --release --example decal_resolve_replay -- <lot_instance_hex> <lot_pkg> [lookup...]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lot = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let packages: Vec<Package> = args
        .iter()
        .skip(1)
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();

    // 收集全部 decal atlas 字典
    let mut atlases = Vec::new();
    for (pi, package) in packages.iter().enumerate() {
        for e in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if !sc_properties::decal::DECAL_ATLAS_INSTANCE_TYPES.contains(&(e.id.group as u16)) { continue; }
            let Ok(data) = package.read(e) else { continue };
            let Ok(dict) = sc_properties::DecalDictionary::parse(&data) else { continue };
            atlases.push((pi, e.id.instance, dict));
        }
    }
    println!("atlases: {}", atlases.len());

    let lot_pkg = &packages[0];
    let Some(entry) = lot_pkg.entries().iter().find(|e| e.id.type_id == PROPERTY_TYPE && e.id.instance == lot).cloned() else {
        println!("lot not found"); return;
    };
    let data = lot_pkg.read(&entry).unwrap_or_default();
    let Ok(file) = PropertyFile::parse_with_limits(&data, Default::default()) else { println!("parse fail"); return; };

    for category in 0..3u32 {
        let Some(idp) = file.get(0x0D10_9050 + category) else { continue };
        let Kind::Array(ids) = &idp.kind else { continue };
        for (i, v) in ids.iter().enumerate() {
            let Value::Key(key) = v else { continue };
            let mut hit: Option<(usize, u32, &sc_properties::DecalEntry)> = None;
            for (pi, ai, dict) in &atlases {
                if let Some(de) = dict.entries.iter().find(|e| e.id.as_ref().map(|k| k.instance) == Some(key.instance)) {
                    hit = Some((*pi, *ai, de)); break;
                }
            }
            let Some((pi, ai, de)) = hit else {
                println!("cat{i} [{i}] I {:08X} → 未命中字典", key.instance); continue;
            };
            let has_colors = de.colors_rgba8().is_some();
            let rk = de.raster.as_ref().map(|k| k.instance).unwrap_or(0);
            // raw 路径
            let mut raw_status = String::from("no-raster");
            let mut raw_ok = false;
            if let Some(rk) = de.raster.as_ref() {
                'outer: for (qi, package) in packages.iter().enumerate() {
                    for e in package.entries().iter().filter(|e| e.id.instance == rk.instance) {
                        if e.id.type_id == RASTER_TYPE {
                            let d = package.read(e).unwrap_or_default();
                            match rw4::RasterImage::parse(&d) {
                                Ok(r) => match r.decode_top_mip_rgba() {
                                    Ok(_) => { raw_status = format!("raw OK pixFmt{} {}x{} (pkg{qi})", r.pixel_format, r.width, r.height); raw_ok = true; }
                                    Err(e) => raw_status = format!("raw decode-err {e}"),
                                },
                                Err(e) => raw_status = format!("raw parse-err {e}"),
                            }
                            break 'outer;
                        } else if e.id.type_id == RW4_TYPE {
                            raw_status = format!("rw4-texture (pkg{qi})");
                            raw_ok = true;
                            break 'outer;
                        }
                    }
                }
                if raw_status == "no-raster" { raw_status = format!("raster {:08X} not found", rk.instance); }
            }
            let four = if has_colors { "4color possible" } else { "no colors" };
            println!(
                "cat{} [{}] I {:08X} -> atlas pkg#{} I {:08X} entry#{} aspect={} colors={} | raw: {} raw_ok={} | {}",
                category, i, key.instance, pi, ai, de.index,
                de.aspect_ratio.is_some(), has_colors, raw_status, raw_ok, four
            );
        }
    }
}
