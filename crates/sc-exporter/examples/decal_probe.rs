//! 探针：定位 package 中的 Decal Dictionary（贴花图鉴）并打印结构摘要。
//!
//! 用法：
//! ```text
//! cargo run -p sc-exporter --release --example decal_probe -- <package> [lookup_package...]
//! cargo run -p sc-exporter --release --example decal_probe -- --out=<dir> <package> [lookup_package...]
//! ```
//!
//! 输出 Property 资源的 group 低 16 位分布，便于在类型集合与实际不符时发现
//! decal atlas 家族；随后逐条目打印 Raster 解析状态与四色。带 `--out` 时把
//! 可解码条目按四色映射导出为 PNG，用于与原始 SCP 的相册预览目视比对。
use dbpf::Package;
use sc_properties::{DecalDictionary, PROPERTY_RESOURCE_TYPE, is_decal_dictionary_group};

const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;
/// 单次导出上限，避免一次性写出上千张图。
const DUMP_LIMIT: usize = 40;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--out="))
        .map(str::to_owned);
    // 只处理指定字典：`--only=<instance>` 或 `--only=<group_low16>:<instance>`。
    let only = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--only="))
        .map(|value| {
            let parse = |text: &str| {
                u32::from_str_radix(text.trim_start_matches("0x"), 16)
                    .unwrap_or_else(|error| panic!("bad --only value {value}: {error}"))
            };
            match value.split_once(':') {
                Some((group, instance)) => (Some(parse(group)), parse(instance)),
                None => (None, parse(value)),
            }
        });
    let paths: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if paths.is_empty() {
        eprintln!(
            "usage: decal_probe [--out=<dir>] <package> [lookup_package...]"
        );
        std::process::exit(2);
    }
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir).unwrap_or_else(|error| panic!("create {dir}: {error}"));
    }

    let packages: Vec<Package> = paths
        .iter()
        .map(|path| Package::open(path).unwrap_or_else(|error| panic!("open {path}: {error}")))
        .collect();
    let main_package = &packages[0];
    println!("package: {}", paths[0]);
    if let Some(dir) = &out_dir {
        println!("dump dir: {dir}");
    }

    let mut buckets: Vec<(u16, usize)> = Vec::new();
    for entry in main_package
        .entries()
        .iter()
        .filter(|entry| entry.id.type_id == PROPERTY_RESOURCE_TYPE)
    {
        let low = entry.id.group as u16;
        match buckets.iter_mut().find(|(value, _)| *value == low) {
            Some((_, count)) => *count += 1,
            None => buckets.push((low, 1)),
        }
    }
    buckets.sort_by(|left, right| right.1.cmp(&left.1));
    println!("property group low16 distribution (top 12):");
    for (low, count) in buckets.iter().take(12) {
        let marker = if is_decal_dictionary_group(u32::from(*low)) {
            "  <- decal atlas"
        } else {
            ""
        };
        println!("  {low:#06x} x{count}{marker}");
    }

    let candidates: Vec<_> = main_package
        .entries()
        .iter()
        .filter(|entry| {
            entry.id.type_id == PROPERTY_RESOURCE_TYPE
                && is_decal_dictionary_group(entry.id.group)
                && only.is_none_or(|(group, instance)| {
                    entry.id.instance == instance
                        && group.is_none_or(|value| (entry.id.group & 0xffff) == value)
                })
        })
        .cloned()
        .collect();
    println!("\ndecal dictionaries: {}", candidates.len());

    let mut dumped = 0usize;
    for entry in &candidates {
        let Ok(data) = main_package.read(entry) else {
            println!("  0x{:08x}: read failed", entry.id.instance);
            continue;
        };
        let dictionary = match DecalDictionary::parse(&data) {
            Ok(dictionary) => dictionary,
            Err(error) => {
                println!("  0x{:08x}: parse failed: {error}", entry.id.instance);
                continue;
            }
        };

        let arrays = dictionary
            .array_lengths
            .iter()
            .map(|(hash, length)| {
                format!(
                    "{hash:08x}:{}",
                    length.map_or_else(|| "-".to_owned(), |value| value.to_string())
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "\n0x{:08x}-0x{:08x}-0x{:08x}  entries={} uniform={} arrays=[{arrays}]",
            entry.id.type_id,
            entry.id.group,
            entry.id.instance,
            dictionary.entries.len(),
            dictionary.uniform_arrays
        );
        println!(
            "  material={} textureSize={:?} atlasSize={:?}",
            dictionary
                .material
                .map_or_else(|| "-".to_owned(), |key| format!("0x{:08x}", key.instance)),
            dictionary.texture_size,
            dictionary.atlas_size
        );

        let mut found = 0usize;
        let mut decodable = 0usize;
        let mut colors_missing = 0usize;
        for decal in &dictionary.entries {
            let raster = resolve_raster(&packages, decal.raster.map(|key| key.instance));
            let status = match &raster {
                Some((image, source)) => format!(
                    "{}x{} pixFmt={} via {source}",
                    image.width, image.height, image.pixel_format
                ),
                None => "raster not found".to_owned(),
            };
            let colors_rgba8 = decal.colors_rgba8();
            if raster.is_some() {
                found += 1;
            }
            if raster
                .as_ref()
                .is_some_and(|(image, _)| image.is_raw_rgba())
                && colors_rgba8.is_some()
            {
                decodable += 1;
            }
            if colors_rgba8.is_none() {
                colors_missing += 1;
            }

            let colors = colors_rgba8.map_or_else(
                || "-".to_owned(),
                |colors| {
                    colors
                        .iter()
                        .map(|color| format!("{:02x}{:02x}{:02x}", color[0], color[1], color[2]))
                        .collect::<Vec<_>>()
                        .join(",")
                },
            );
            println!(
                "  [{:>3}] id={} raster={} ratio={} {status} colors=[{colors}]",
                decal.index,
                decal
                    .id
                    .map_or_else(|| "-".to_owned(), |key| format!("0x{:08x}", key.instance)),
                decal
                    .raster
                    .map_or_else(|| "-".to_owned(), |key| format!("0x{:08x}", key.instance)),
                decal
                    .aspect_ratio
                    .map_or_else(|| "-".to_owned(), |value| format!("{value}")),
            );

            if let Some(dir) = &out_dir
                && dumped < DUMP_LIMIT
                && let Some(colors) = colors_rgba8
                && let Some((image, _)) = &raster
                && image.is_raw_rgba()
                && let Ok(rgba) = image.decode_lot_mask_rgba(&colors)
                && let Some(bitmap) = image::RgbaImage::from_raw(image.width, image.height, rgba)
            {
                let name = format!(
                    "decal_{:03}_{:08x}.png",
                    decal.index,
                    decal.id.map_or(0, |key| key.instance)
                );
                match bitmap.save(format!("{dir}/{name}")) {
                    Ok(()) => {
                        dumped += 1;
                        println!("        -> {name}");
                    }
                    Err(error) => println!("        -> {name} failed: {error}"),
                }
            }
        }
        println!(
            "  summary: raster_found={found}/{} decodable={decodable} colors_missing={colors_missing}",
            dictionary.entries.len()
        );
    }
    if out_dir.is_some() {
        println!("\ndumped {dumped} PNG(s)");
    }
}

/// 按 instance + Raster 类型在已打开包中查找并解析。
fn resolve_raster(
    packages: &[Package],
    instance: Option<u32>,
) -> Option<(rw4::RasterImage, String)> {
    let instance = instance?;
    for package in packages {
        let Some(entry) = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == RASTER_IMAGE_TYPE && entry.id.instance == instance)
        else {
            continue;
        };
        let Ok(bytes) = package.read(entry) else {
            continue;
        };
        let Ok(raster) = rw4::RasterImage::parse(&bytes) else {
            continue;
        };
        let name = package
            .path()
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        return Some((raster, name));
    }
    None
}
