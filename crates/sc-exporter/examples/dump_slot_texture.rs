//! 冒烟工具：解出指定 RW4 纹理包裹（2F4E681B）的顶层 mip 存 PNG。
//! 用法：cargo run -p sc-exporter --release --example dump_slot_texture -- <instance_hex> <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let out = &args[1];
    for path in &args[2..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| {
            e.id.instance == instance && matches!(e.id.type_id, 0x2F4E_681B | 0x2F4E_681C)
        }) {
            let Ok(data) = package.read(e) else { continue };
            let px_w_h = if e.id.type_id == 0x2F4E_681C {
                rw4::RasterImage::parse(&data).ok().and_then(|r| {
                    r.decode_top_mip_rgba()
                        .ok()
                        .map(|px| (px, r.width as usize, r.height as usize))
                })
            } else {
                rw4::Rw4File::parse(&data).ok().and_then(|file| {
                    let sec = file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
                    let tex = file.decode_texture(&data, sec).ok()?;
                    let px = tex.decode_top_mip_rgba().ok()?;
                    Some((px, tex.width as usize, tex.height as usize))
                })
            };
            if let Some((px, w, h)) = px_w_h {
                let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                    let at = (x as usize + y as usize * w) * 4;
                    image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                });
                let path = format!("{out}/tex_{:08X}_{}.png", instance, e.id.type_id);
                image::save(&path, &img).expect("save");
                println!("{path}: T {:08X} G {:08X} {}x{} → {}", path, e.id.type_id, e.id.group, w, h, path);
                return;
            }
        }
    }
    println!("{instance:08X}: 无可解纹理");
}
