//! 只读探针：lot property 的 Decal 单元 → Decal Atlas 字典条目 → Raster 贴图
//! 的完整链路追踪。
//!
//! 用法：decal_chain <lot_package> <lot_instance> --out=<dir> [lookup...]

use dbpf::{IndexEntry, Package};
use sc_properties::decal::{DecalDictionary, DECAL_ATLAS_INSTANCE_TYPES};
use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = raw.iter().filter(|a| !a.starts_with("--")).collect();
    let out_dir = raw
        .iter()
        .find_map(|a| a.strip_prefix("--out=").map(str::to_owned))
        .unwrap_or_else(|| "tmp/decal_chain".to_owned());
    std::fs::create_dir_all(&out_dir).unwrap();

    let mut paths: Vec<String> = vec![positional[0].clone()];
    paths.extend(positional.iter().skip(2).map(|s| (*s).clone()));
    let lot_instance =
        u32::from_str_radix(positional[1].trim_start_matches("0x"), 16).unwrap();
    let packages: Vec<Package> = paths.iter().map(|p| open(p)).collect();

    // ---- 1. lot property 的 decal 单元 ----
    let (owner, entry) = find(&packages, 0x00B1_B104, lot_instance, None).unwrap();
    let data = packages[owner].read(&entry).unwrap();
    let file = PropertyFile::parse(&data).expect("parse");

    const ID_BASE: u32 = 0x0D10_9050;
    const TR_BASE: u32 = 0x0D10_9060;
    const DEPTH_BASE: u32 = 0x0D10_9070;
    const MAT_BASE: u32 = 0x0D10_9080;

    for category in 0..3usize {
        let id_hash = ID_BASE + category as u32;
        let Some(id_property) = file.get(id_hash) else { continue };
        let Some(Kind::Array(ids)) = Some(&id_property.kind) else { continue };
        println!(
            "== category {category}：{} 个 decal（hash 0x{ID_HASH:08X}）",
            ids.len(),
            ID_HASH = id_hash
        );
        let transforms = file
            .get(TR_BASE + category as u32)
            .and_then(|p| p.array().map(|v| v.to_vec()))
            .unwrap_or_default();
        let depths = file
            .get(DEPTH_BASE + category as u32)
            .and_then(|p| p.array().map(|v| v.to_vec()))
            .unwrap_or_default();
        let mats = file
            .get(MAT_BASE + category as u32)
            .and_then(|p| p.array().map(|v| v.to_vec()))
            .unwrap_or_default();
        for (index, value) in ids.iter().enumerate() {
            let id = match value {
                Value::Key(k) => format!("T {:08X} G {:08X} I {:08X}", k.type_id, k.group, k.instance),
                other => format!("{other:?}"),
            };
            let transform = transforms.get(index).map(|v| format!("{v:?}")).unwrap_or_default();
            let depth = depths.get(index).map(|v| format!("{v:?}")).unwrap_or_default();
            let material = mats.get(index).map(|v| format!("{v:?}")).unwrap_or_default();
            println!("  [{index}] ID = {id}");
            println!("        transform = {transform}");
            println!("        depth = {depth}  material_data = {material}");
        }
    }

    // ---- 2. 收集包内全部 Decal Atlas（GroupContainer 低 16 位 ∈ {B185,1651,1652}）----
    let mut atlases: Vec<(usize, IndexEntry, DecalDictionary)> = Vec::new();
    for (index, package) in packages.iter().enumerate() {
        for e in package.entries() {
            if e.id.type_id != 0x00B1_B104 {
                continue;
            }
            if !DECAL_ATLAS_INSTANCE_TYPES.contains(&(e.id.group as u16)) {
                continue;
            }
            let Ok(bytes) = package.read(e) else { continue };
            let Ok(dict) = DecalDictionary::parse(&bytes) else { continue };
            atlases.push((index, e.clone(), dict));
        }
    }
    println!(
        "\n== 共发现 {} 个 Decal Atlas 字典（group 低16位 {{B185,1651,1652}}）",
        atlases.len()
    );
    for (index, e, dict) in &atlases {
        println!(
            "  atlas pkg#{} G {:08X} I {:08X}: {} 条目, textureSize {:?}, atlasSize {:?}, material {:?}",
            index,
            e.id.group,
            e.id.instance,
            dict.entries.len(),
            dict.texture_size,
            dict.atlas_size,
            dict.material
                .as_ref()
                .map(|k| format!("{:08X}", k.instance))
                .unwrap_or_else(|| "-".into())
        );
    }

    // ---- 3. 用 lot 里的 decal ID 反查字典条目并解码 raster ----
    let mut lot_ids: Vec<sc_properties::Key> = Vec::new();
    for category in 0..3usize {
        if let Some(p) = file.get(ID_BASE + category as u32)
            && let Some(Kind::Array(values)) = Some(&p.kind)
        {
            for v in values {
                if let Value::Key(k) = v {
                    lot_ids.push(*k);
                }
            }
        }
    }
    println!("\n== lot 引用的 {} 个 decal ID 的归属：", lot_ids.len());
    for id in &lot_ids {
        let mut hit = None;
        for (index, e, dict) in &atlases {
            if let Some(entry) = dict.entries.iter().find(|entry| {
                entry
                    .id
                    .map(|k| k.instance == id.instance)
                    .unwrap_or(false)
            }) {
                hit = Some((index, e, entry));
                break;
            }
        }
        match hit {
            Some((index, e, entry)) => {
                println!(
                    "  I {:08X} → atlas pkg#{} I {:08X} 条目#{} aspect={:?} raster={:?}",
                    id.instance,
                    index,
                    e.id.instance,
                    entry.index,
                    entry.aspect_ratio,
                    entry
                        .raster
                        .as_ref()
                        .map(|k| format!("{:08X}", k.instance))
                        .unwrap_or_else(|| "-".into())
                );
                if let Some(raster_key) = &entry.raster {
                    decode_raster(&packages, raster_key.instance, &out_dir);
                }
            }
            None => println!("  I {:08X} → 未在任何 Decal Atlas 中找到", id.instance),
        }
    }
}

fn decode_raster(packages: &[Package], instance: u32, out_dir: &str) {
    for (index, package) in packages.iter().enumerate() {
        for e in package.entries() {
            if e.id.type_id == 0x2F4E_681C && e.id.instance == instance {
                let bytes = package.read(e).unwrap();
                let Ok(raster) = rw4::RasterImage::parse(&bytes) else {
                    println!("      raster {instance:08X}: 解码失败（pixFmt {}）", raster_px(&bytes));
                    return;
                };
                match raster.decode_top_mip_rgba() {
                    Ok(rgba) => {
                        let img = image::RgbaImage::from_raw(
                            raster.width,
                            raster.height,
                            rgba,
                        )
                        .unwrap();
                        let path = format!("{out_dir}/decal_{instance:08X}.png");
                        img.save(&path).unwrap();
                        println!(
                            "      raster {instance:08X}: {}x{} → {path}",
                            raster.width, raster.height
                        );
                    }
                    Err(error) => println!("      raster {instance:08X}: 像素解码失败 {error}"),
                }
                return;
            }
        }
    }
    println!("      raster {instance:08X}: 未找到资源");
}

fn raster_px(_bytes: &[u8]) -> u32 {
    0
}

fn open(path: &str) -> Package {
    Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"))
}

fn find(
    packages: &[Package],
    type_id: u32,
    instance: u32,
    group: Option<u32>,
) -> Option<(usize, IndexEntry)> {
    for (index, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id == type_id
                && entry.id.instance == instance
                && group.is_none_or(|g| entry.id.group == g)
            {
                return Some((index, entry.clone()));
            }
        }
    }
    None
}
