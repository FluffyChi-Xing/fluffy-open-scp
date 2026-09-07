//! 冒烟工具：导出指定模型材质的 slot1/3/5 贴图为 PNG（人工目检语义，阶段 4）。
//!
//! 用法：cargo run -p sc-exporter --release --example slot_dump -- <package> 0xMODEL <out_dir>

use rw4::{Rw4File, SectionType};

const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (path, rest) = args.split_first().expect("usage: slot_dump <package> 0xMODEL <out_dir>");
    let model = rest.first().expect("need model instance");
    let out_dir = rest.get(1).expect("need out_dir");
    let instance = u32::from_str_radix(model.trim_start_matches("0x"), 16).expect("instance");
    let package = dbpf::Package::open(path).expect("open package");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_MODEL_TYPE && e.id.instance == instance)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read model");
    let file = Rw4File::parse(&data).expect("parse rw4");

    std::fs::create_dir_all(out_dir).expect("create out dir");
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let mat = match file.decode_material(&data, section.number) {
            Ok(rw4::MaterialSection::Decoded(m)) => m,
            _ => continue,
        };
        for r in mat.texture_slots() {
            let slot = r.slot_byte();
            // slot0 = Material Info f32 贴图（regionXform 参数表）
            if slot == 0 && r.texture_instance != 0 {
                if let Some(tex_entry) = package
                    .entries()
                    .iter()
                    .find(|e| e.id.instance == r.texture_instance && e.id.type_id == RW4_MODEL_TYPE)
                    .cloned()
                {
                    let tex_data = package.read(&tex_entry).expect("read");
                    let tex_file = Rw4File::parse(&tex_data).expect("rw4");
                    let sec = tex_file
                        .sections_of_type(SectionType::TEXTURE)
                        .next()
                        .expect("sec")
                        .number;
                    let tex = tex_file.decode_texture(&tex_data, sec).expect("decode");
                    let cols = u32::from(tex.width) as usize;
                    println!(
                        "  --- slot0 f32: type=0x{:08X} {cols}x{} ---",
                        tex.texture_type,
                        tex.height
                    );
                    if let Ok(pixels) = tex.decode_palette_f32() {
                        for m in (30..cols).step_by(8) {
                            for row in 0..u32::from(tex.height) as usize {
                                let mut cells = String::new();
                                for c in m..(m + 3).min(cols) {
                                    let px =
                                        pixels.get(row * cols + c).copied().unwrap_or([0.0; 4]);
                                    cells.push_str(&format!(
                                        "({:.3},{:.3},{:.3},{:.3})",
                                        px[0], px[1], px[2], px[3]
                                    ));
                                }
                                println!("    m{m:<3} row{row} {cells}");
                            }
                        }
                    }
                }
                continue;
            }
            if !(1..=5).contains(&slot) {
                continue;
            }
            let Some(tex_entry) = package
                .entries()
                .iter()
                .find(|e| {
                    e.id.instance == r.texture_instance
                        && (e.id.type_id == RASTER_IMAGE_TYPE || e.id.type_id == RW4_MODEL_TYPE)
                })
                .cloned()
            else {
                println!("slot{slot} 0x{:08X}: not in package", r.texture_instance);
                continue;
            };
            let tex_data = package.read(&tex_entry).expect("read texture");
            let decoded = if tex_entry.id.type_id == RASTER_IMAGE_TYPE {
                let raster = rw4::RasterImage::parse(&tex_data).expect("raster");
                let size = (u32::from(raster.width), u32::from(raster.height));
                (raster.decode_top_mip_rgba().expect("decode"), size)
            } else {
                let tex_file = Rw4File::parse(&tex_data).expect("rw4 texture");
                let sec = tex_file
                    .sections_of_type(SectionType::TEXTURE)
                    .next()
                    .expect("texture section")
                    .number;
                let tex = tex_file.decode_texture(&tex_data, sec).expect("decode");
                let size = (u32::from(tex.width), u32::from(tex.height));
                (tex.decode_top_mip_rgba().expect("decode"), size)
            };
            let (rgba, (width, height)) = decoded;
            let img = image::RgbaImage::from_raw(width, height, rgba.clone()).expect("rgba size");
            let out_path = format!("{out_dir}/mat{}_slot{}.png", section.number, slot);
            img.save(&out_path).expect("save png");
            println!("slot{slot} -> {out_path} ({width}x{height})");
            // Material Info 贴图取证：slot0 按 f32 dump（每材质 4 行 float4 = regionXform×2 + 参数）
            if slot == 0 && tex_entry.id.type_id == RW4_MODEL_TYPE {
                let tex_file = Rw4File::parse(&tex_data).expect("rw4");
                let sec = tex_file
                    .sections_of_type(SectionType::TEXTURE)
                    .next()
                    .expect("sec")
                    .number;
                let tex = tex_file.decode_texture(&tex_data, sec).expect("decode");
                if let Ok(pixels) = tex.decode_palette_f32() {
                    let cols = u32::from(tex.width) as usize;
                    println!("  --- slot0 f32 dump: {cols} cols x {} rows (4 float4 per material) ---", tex.height);
                    for m in (30..cols).step_by(6) {
                        for row in 0..u32::from(tex.height) as usize {
                            let mut cells = String::new();
                            for c in m..(m + 4).min(cols) {
                                let px = pixels.get(row * cols + c).copied().unwrap_or([0.0; 4]);
                                cells.push_str(&format!("({:.3},{:.3},{:.3},{:.3})", px[0], px[1], px[2], px[3]));
                            }
                            println!("    m{m:<3} row{row} {cells}");
                        }
                    }
                }
            }
            // Material Info 贴图取证：slot4 逐 texel dump（字节 + /255 浮点）
            if slot == 4 {
                let w = width as usize;
                let h = height as usize;
                println!("  --- slot4 raw dump (columns with vertex G 37..111, all rows) ---");
                for col in (30..120).step_by(6) {
                    for row in 0..h {
                        let mut cells = String::new();
                        for c in col..(col + 6).min(w) {
                            let px = &rgba[(row * w + c) * 4..(row * w + c) * 4 + 4];
                            cells.push_str(&format!(
                                "({:3},{:3},{:3},{:3})",
                                px[0], px[1], px[2], px[3]
                            ));
                        }
                        println!("    col{col:<3} row{row:<2} {cells}");
                    }
                }
            }
        }
    }
}
