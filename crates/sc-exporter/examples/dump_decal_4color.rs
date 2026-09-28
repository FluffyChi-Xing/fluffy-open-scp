//! 冒烟工具：按运行时口径（decode_lot_mask_rgba）解码指定 decal 条目。
//! 用法：cargo run -p sc-exporter --release --example dump_decal_4color -- <id_instance_hex> <out_png> <pkg> [...]
use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let id = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let packages: Vec<Package> = args
        .iter()
        .skip(2)
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();
    for package in &packages {
        for e in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            let Ok(data) = package.read(e) else { continue };
            let Ok(dict) = sc_properties::DecalDictionary::parse(&data) else { continue };
            let Some(decal_entry) = dict.entries.iter().find(|e| e.id.as_ref().map(|k| k.instance) == Some(id)) else { continue };
            println!(
                "atlas G{:08X}I{:08X} entry #{} aspect={:?} colors={:?}",
                e.id.group, e.id.instance, decal_entry.index, decal_entry.aspect_ratio, decal_entry.colors_rgba8()
            );
            let colors = decal_entry.colors_rgba8().expect("no colors");
            let raster_key = decal_entry.raster.clone().expect("no raster");
            for package in &packages {
                let Some(rentry) = package.entries().iter().find(|e| e.id.type_id == RASTER_TYPE && e.id.instance == raster_key.instance).cloned() else { continue };
                let rdata = package.read(&rentry).expect("read raster");
                let raster = rw4::RasterImage::parse(&rdata).expect("parse raster");
                let rgba = raster.decode_lot_mask_rgba(&colors).expect("decode");
                let img = image::RgbaImage::from_raw(raster.width, raster.height, rgba).expect("img");
                img.save(&args[1]).expect("save");
                println!("→ {}", args[1]);
                return;
            }
            panic!("raster {:08X} not found", raster_key.instance);
        }
    }
    panic!("id {id:08X} not in any decal dictionary");
}
