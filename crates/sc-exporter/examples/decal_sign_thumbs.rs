//! 只读工具：导出招牌字典（material 0x73684EFC）全部唯一条目的解码贴图。
//! 每个条目一张 PNG（文件名 = 条目 instance），用于目视定位特定招牌。
//!
//! 用法：
//!   cargo run -p sc-exporter --release --example decal_sign_thumbs -- \
//!       <out_dir> <package> [lookup...]

use dbpf::Package;
use sc_properties::{DecalDictionary, PROPERTY_RESOURCE_TYPE, is_decal_dictionary_group};

const SIGN_MATERIAL: u32 = 0x7368_4EFC;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args.first().expect("usage: decal_sign_thumbs <out> <package> [lookup...]").clone();
    let packages: Vec<Package> = args[1..]
        .iter()
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();
    std::fs::create_dir_all(&out_dir).expect("create out dir");

    let mut seen = std::collections::HashSet::new();
    let mut saved = 0usize;
    for package in &packages {
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_RESOURCE_TYPE
                || !is_decal_dictionary_group(entry.id.group)
            {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(dictionary) = DecalDictionary::parse(&data) else { continue };
            if dictionary.material.map(|k| k.instance) != Some(SIGN_MATERIAL) {
                continue;
            }
            for decal in &dictionary.entries {
                let Some(id) = decal.id.map(|k| k.instance) else { continue };
                if !seen.insert(id) {
                    continue;
                }
                let Some(colors) = decal.colors_rgba8() else { continue };
                let Some((image, _)) = resolve_raster(&packages, decal.raster.map(|k| k.instance))
                else {
                    continue;
                };
                if !image.is_raw_rgba() {
                    continue;
                }
                let Ok(rgba) = image.decode_lot_mask_rgba(&colors) else { continue };
                let Some(buf) = image::RgbaImage::from_raw(image.width, image.height, rgba)
                else {
                    continue;
                };
                let path = format!("{out_dir}/sign_{id:08X}.png");
                if buf.save(&path).is_ok() {
                    saved += 1;
                }
            }
        }
    }
    println!("导出 {saved} 张唯一招牌条目贴图 → {out_dir}");
}

fn resolve_raster(packages: &[Package], instance: Option<u32>) -> Option<(rw4::RasterImage, String)> {
    let target = instance?;
    for package in packages {
        for entry in package.entries() {
            if entry.id.instance != target {
                continue;
            }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
            return Some((raster, format!("{:08x}", entry.id.instance)));
        }
    }
    None
}
