//! 冒烟探针：统计 EP1 property 的 LotMask raster 像素格式分布（阶段 3.1 取证）。
//!
//! 用法：cargo run -p sc-properties --release --example lotmask_probe -- <package>

use sc_properties::{LotEditorDocument, PropertyFile};
use std::collections::BTreeMap;

const LOT_MASK_HASH: u32 = 0x0CCB_7FD5;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: lotmask_probe <package> [lookup...]");
    let package = dbpf::Package::open(path).expect("open package");
    let lookups: Vec<_> = args
        .iter()
        .skip(1)
        .map(|p| dbpf::Package::open(p).expect("open lookup package"))
        .collect();

    let mut pix_fmt_hist: BTreeMap<u32, usize> = BTreeMap::new();
    let mut source_hist: BTreeMap<usize, usize> = BTreeMap::new();
    let mut with_mask = 0usize;
    let mut raster_found = 0usize;
    let mut raster_missing = 0usize;
    let mut properties = 0usize;

    let mut sample_printed = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        properties += 1;
        let entry = entry.clone();
        let Ok(data) = package.read(&entry) else { continue };
        let Ok(properties_file) =
            PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default())
        else {
            continue;
        };
        let document = LotEditorDocument::from_property_file(properties_file);
        let Some(mask) = document.lot_mask else { continue };
        with_mask += 1;
        // Key 的精确 TGI 或 instance+RASTER_TYPE 扫描
        let exact = dbpf::ResourceId {
            type_id: mask.type_id,
            group: mask.group,
            instance: mask.instance,
        };
        let mut found = package
            .entry(exact)
            .filter(|e| e.id.type_id == RASTER_TYPE)
            .or_else(|| {
                package
                    .entries()
                    .iter()
                    .find(|e| e.id.type_id == RASTER_TYPE && e.id.instance == mask.instance)
            })
            .map(|e| (0usize, e));
        if found.is_none() {
            for (pkg_index, lookup) in lookups.iter().enumerate() {
                found = lookup
                    .entries()
                    .iter()
                    .find(|e| e.id.type_id == RASTER_TYPE && e.id.instance == mask.instance)
                    .map(|e| (pkg_index + 1, e));
                if found.is_some() {
                    break;
                }
            }
        }
        let Some((source, raster_entry)) = found else {
            raster_missing += 1;
            continue;
        };
        *source_hist.entry(source).or_insert(0) += 1;
        raster_found += 1;
        let Ok(raster_data) = package.read(&raster_entry) else { continue };
        if let Ok(raster) = rw4::RasterImage::parse(&raster_data) {
            *pix_fmt_hist.entry(u32::from(raster.pixel_format)).or_insert(0) += 1;
            if sample_printed < 5 {
                sample_printed += 1;
                println!(
                    "sample 0x{:08X}: raster {}x{} pixFmt={} raw={}",
                    mask.instance,
                    raster.width,
                    raster.height,
                    raster.pixel_format,
                    raster.is_raw_rgba()
                );
            }
        } else {
            *pix_fmt_hist.entry(u32::MAX).or_insert(0) += 1;
        }
    }

    println!("properties={properties} with_lot_mask={with_mask} raster_found={raster_found} raster_missing_in_package={raster_missing}");
    println!("mask raster source histogram (0=main, 1..=lookup index):");
    for (source, count) in &source_hist {
        println!("  package {source}: {count}");
    }
    println!("pixFmt histogram (u32::MAX = raster parse error):");
    for (fmt, count) in &pix_fmt_hist {
        println!("  pixFmt {fmt}: {count}");
    }
}
