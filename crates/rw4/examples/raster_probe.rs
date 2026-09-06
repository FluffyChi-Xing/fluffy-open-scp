//! 冒烟探针：扫描包内 raster(0x2F4E681C) 资源，打印解析与解码结果。仅开发用。
//! 用法：cargo run -p rw4 --example raster_probe -- <package> [max]

fn main() {
    let path = std::env::args().nth(1).expect("usage: raster_probe <package> [max]");
    let max: usize = std::env::args().nth(2).and_then(|v| v.parse().ok()).unwrap_or(12);
    let package = dbpf::Package::open(&path).expect("open package");
    let mut shown = 0usize;
    let mut raw = 0usize;
    let mut compressed = 0usize;
    let mut bad = 0usize;
    for entry in package.entries() {
        if entry.id.type_id != 0x2F4E_681C { continue; }
        match package.read(entry) {
            Ok(data) => match rw4::RasterImage::parse(&data) {
                Ok(raster) => {
                    let status = if raster.is_raw_rgba() {
                        raw += 1;
                        match raster.decode_top_mip_rgba() {
                            Ok(rgba) => format!("OK {}px decoded", rgba.len() / 4),
                            Err(e) => format!("DECODE_ERR {e}"),
                        }
                    } else {
                        compressed += 1;
                        format!("COMPRESSED pixFmt={}", raster.pixel_format)
                    };
                    if shown < max {
                        println!("0x{:08X} type={} {}x{} mips={} pixSize={} pixFmt={} {}",
                            entry.id.instance, raster.raster_type, raster.width, raster.height,
                            raster.mip_count, raster.pixel_size, raster.pixel_format, status);
                        shown += 1;
                    }
                }
                Err(_) => bad += 1,
            },
            Err(_) => bad += 1,
        }
    }
    println!("--- rasters: raw21={raw} compressed={compressed} parse_fail={bad}");
}
