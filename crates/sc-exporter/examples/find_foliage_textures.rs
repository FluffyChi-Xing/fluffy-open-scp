//! 冒烟工具：扫描包内绿色主导的纹理（树/植被图集特征）。
//! 用法：cargo run -p sc-exporter --release --example find_foliage_textures -- <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for path in &args {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter() {
            // RW4 纹理包裹与 raster 两种形态
            let is_candidate =
                matches!(e.id.type_id, 0x2F4E_681B | 0x2F4E_681C | 0x1C8F_E176);
            if !is_candidate || e.decompressed_size < 20_000 || e.decompressed_size > 400_000 {
                continue;
            }
            let Ok(data) = package.read(e) else { continue };
            // 解出顶层 RGBA 粗采样：绿主导 = g > r*1.3 且 g > b*1.3 的像素占比 > 25%
            let (rgba, w, h) = match e.id.type_id {
                0x2F4E_681C => match rw4::RasterImage::parse(&data) {
                    Ok(r) if r.is_raw_rgba() => match r.decode_top_mip_rgba() {
                        Ok(px) => (px, r.width as usize, r.height as usize),
                        _ => continue,
                    },
                    _ => continue,
                },
                _ => match rw4::Rw4File::parse(&data) {
                    Ok(file) => {
                        let Some(sec) = file.sections_of_type(rw4::SectionType::TEXTURE).next()
                        else {
                            continue;
                        };
                        let Ok(tex) = file.decode_texture(&data, sec.number) else { continue };
                        let Ok(px) = tex.decode_top_mip_rgba() else { continue };
                        (px, tex.width as usize, tex.height as usize)
                    }
                    _ => continue,
                },
            };
            let total = (w * h).max(1);
            let step = (total / 400).max(1);
            let mut green = 0usize;
            let mut sampled = 0usize;
            for i in (0..total).step_by(step) {
                let (r, g, b) = (rgba[i * 4] as f32, rgba[i * 4 + 1] as f32, rgba[i * 4 + 2] as f32);
                sampled += 1;
                if g > 40.0 && g > r * 1.25 && g > b * 1.25 {
                    green += 1;
                }
            }
            let ratio = green as f32 / sampled as f32;
            if ratio > 0.25 {
                println!(
                    "{path}: T {:08X} I {:08X} {}x{} 绿占比 {:.0}%",
                    e.id.type_id,
                    e.id.instance,
                    w,
                    h,
                    ratio * 100.0
                );
                // 竖长条优先落盘（树公告板轮廓特征：高>宽 且 ≥128 高）
                if h > w && h >= 128 {
                    let img = image::RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                        let at = (x as usize + y as usize * w) * 4;
                        image::Rgba([
                            rgba[at],
                            rgba[at + 1],
                            rgba[at + 2],
                            255,
                        ])
                    });
                    let out = format!("tmp/foliage_{:08X}.png", e.id.instance);
                    match img.save(&out) {
                        Ok(()) => println!("  → {out}"),
                        Err(e) => println!("  save err {e}"),
                    }
                }
            }
        }
    }
}
