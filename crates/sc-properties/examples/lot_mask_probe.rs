//! 冒烟探针：扫描包内带 LotMask 的 property,执行会话同款 LotMask 解码链路。仅开发用。
//! 用法：cargo run -p sc-properties --example lot_mask_probe -- <package> [max]

use dbpf::ResourceId;
use rw4::RasterImage;

fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_mask_probe <package> [max]");
    let max: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(8);
    let package = dbpf::Package::open(&path).expect("open package");
    let mut shown = 0usize;
    let mut with_mask = 0usize;
    let mut ok = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let Ok(data) = package.read(entry) else { continue };
        let Ok(file) = sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default()) else { continue };
        let document = sc_properties::LotEditorDocument::from_property_file(file);
        let Some(mask_key) = document.lot_mask else { continue };
        with_mask += 1;
        // 会话同款定位：先精确 TGI,再 instance+raster 类型（忽略 group）
        let found = package
            .entry(ResourceId { type_id: mask_key.type_id, group: mask_key.group, instance: mask_key.instance })
            .filter(|e| e.id.type_id == 0x2F4E_681C)
            .or_else(|| package.entries().iter().find(|e| e.id.type_id == 0x2F4E_681C && e.id.instance == mask_key.instance))
            .map(|e| e.id);
        let status = match found {
            None => "RASTER_MISSING".to_string(),
            Some(id) => match package.read(&package.entry(id).unwrap()) {
                Ok(bytes) => match RasterImage::parse(&bytes) {
                    Ok(raster) => {
                        let colors = [[0u8; 3]; 4];
                        match raster.decode_lot_mask_rgba(&colors) {
                            Ok(rgba) => { ok += 1; format!("OK {}x{} pixFmt={} {}px", raster.width, raster.height, raster.pixel_format, rgba.len() / 4) }
                            Err(e) => format!("DECODE_ERR {e}"),
                        }
                    }
                    Err(e) => format!("PARSE_ERR {e}"),
                },
                Err(e) => format!("READ_ERR {e}"),
            },
        };
        if shown < max {
            println!("lot 0x{:08X} mask(instance=0x{:08X} type=0x{:08X} group=0x{:08X}) lotSize={:?} => {}",
                entry.id.instance, mask_key.instance, mask_key.type_id, mask_key.group, document.lot_size, status);
            shown += 1;
        }
    }
    println!("--- properties with_mask={with_mask} decode_ok={ok}");
}
