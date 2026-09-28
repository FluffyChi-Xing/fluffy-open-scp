//! 冒烟工具：在多个包里查找同名 raster 资源的全部副本（尺寸/格式对比）。
//! 用法：cargo run -p sc-exporter --release --example find_raster_all -- <instance_hex> <package> [...]
use dbpf::Package;

const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries() {
            if e.id.instance != instance { continue; }
            if e.id.type_id != RASTER_TYPE && e.id.type_id != RW4_TYPE { continue; }
            let data = package.read(e).unwrap_or_default();
            let desc = if e.id.type_id == RASTER_TYPE {
                rw4::RasterImage::parse(&data)
                    .map(|r| format!("raster pixFmt{} {}x{}", r.pixel_format, r.width, r.height))
                    .unwrap_or_else(|e| format!("raster parse-fail {e}"))
            } else {
                format!("rw4 {}B", data.len())
            };
            println!("{path}: T {:08X} {} ({}B)", e.id.type_id, desc, data.len());
        }
    }
}
