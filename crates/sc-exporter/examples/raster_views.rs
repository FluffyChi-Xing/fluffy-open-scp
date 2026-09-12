//! 冒烟工具：把某个 Raster 的全部预览视图各导出一张 PNG，用于目视比对
//! 原 SCP 的 Display Channel（四层量化 / 合成 / R / G / B / A）。
//!
//! 用法：cargo run -p sc-exporter --release --example raster_views -- <package> <instance_hex> <out_dir>
//!
//! 走的是与 `read_raster_preview` 完全相同的 `RasterImage::render_view` 路径。
use dbpf::Package;
use rw4::{DEFAULT_QUANTIZED_COLORS, RasterImage, RasterView};

const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        eprintln!("usage: raster_views <package> <instance_hex> <out_dir>");
        std::process::exit(2);
    }
    let package = Package::open(&args[0]).unwrap_or_else(|error| panic!("open: {error}"));
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16)
        .unwrap_or_else(|error| panic!("bad instance {}: {error}", args[1]));
    let out_dir = &args[2];
    std::fs::create_dir_all(out_dir).unwrap_or_else(|error| panic!("create {out_dir}: {error}"));

    let entry = package
        .entries()
        .iter()
        .find(|entry| entry.id.type_id == RASTER_IMAGE_TYPE && entry.id.instance == instance)
        .cloned()
        .unwrap_or_else(|| panic!("raster 0x{instance:08x} not found"));
    let data = package.read(&entry).unwrap();
    let raster = RasterImage::parse(&data).expect("parse raster");
    println!(
        "raster 0x{instance:08x}: {}x{} pixFmt={} mips={} bytes={}",
        raster.width,
        raster.height,
        raster.pixel_format,
        raster.mip_count,
        data.len()
    );

    for view in RasterView::ALL {
        match raster.render_view(view, &DEFAULT_QUANTIZED_COLORS) {
            Ok(rgba) => {
                let image = image::RgbaImage::from_raw(raster.width, raster.height, rgba)
                    .expect("rgba buffer size");
                let path = format!("{out_dir}/view_{}.png", view.name());
                image.save(&path).expect("save png");
                println!("  {} -> {path}", view.name());
            }
            Err(error) => println!("  {} failed: {error}", view.name()),
        }
    }
}
