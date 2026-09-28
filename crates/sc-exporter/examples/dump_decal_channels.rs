//! 冒烟工具：导出 decal raster 的四通道灰度图 + 指定扫描线剖面。
//! 用法：cargo run -p sc-exporter --release --example dump_decal_channels -- <package> <raster_instance_hex> <out_dir>
use dbpf::Package;

const RASTER_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("hex");
    let out_dir = &args[2];
    std::fs::create_dir_all(out_dir).expect("mkdir");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && e.id.type_id == RASTER_TYPE)
        .cloned()
        .expect("raster not found");
    let data = package.read(&entry).expect("read");
    let raster = rw4::RasterImage::parse(&data).expect("parse");
    println!("pixFmt={} {}x{}", raster.pixel_format, raster.width, raster.height);
    let rgba = raster.decode_top_mip_rgba().expect("decode");
    let (w, h) = (raster.width as usize, raster.height as usize);
    for ch in 0..4 {
        let mut img = image::GrayImage::new(w as u32, h as u32);
        for y in 0..h { for x in 0..w {
            let v = rgba[(y * w + x) * 4 + ch];
            img.put_pixel(x as u32, y as u32, image::Luma([v]));
        }}
        img.save(format!("{out_dir}/ch{ch}.png")).expect("save");
    }
    // 扫描线剖面：y = 字母主体行（32），全宽 R/G/B/A
    for y in [24usize, 32, 40] {
        print!("row {y}: x: ");
        for x in (0..w).step_by(4) {
            print!("{}/", rgba[(y * w + x) * 4]);
            print!("{}", rgba[(y * w + x) * 4 + 1]);
            print!("/");
            print!("{} ", rgba[(y * w + x) * 4 + 2]);
        }
        println!();
    }
}
