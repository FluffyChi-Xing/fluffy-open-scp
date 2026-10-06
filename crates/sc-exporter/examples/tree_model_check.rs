//! 冒烟工具：树模型候选（find_tree_models 同形状过滤）的纹理指纹定性——
//! 材质纹理引用清单 + 内嵌 TEXTURE 绿占比/alpha 剪影统计，落盘缩略图目验。
//! 用法：cargo run -p sc-exporter --release --example tree_model_check -- <pkg> [...]
use dbpf::Package;
use std::collections::BTreeSet;

const RW4_MODEL: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::fs::create_dir_all("tmp/tree_models").expect("mkdir");
    for path in &args {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.type_id == RW4_MODEL) {
            let Ok(data) = package.read(e) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
            let Some(bbox) = file.sections_of_type(rw4::SectionType::BBOX).next() else {
                continue;
            };
            let Ok(payload) = file.payload(&data, bbox.number) else { continue };
            if payload.len() < 24 {
                continue;
            }
            let rd = |off: usize| -> f32 {
                f32::from_le_bytes(payload[off..off + 4].try_into().unwrap())
            };
            let (minx, miny, minz, maxx, maxy, maxz) = (rd(0), rd(4), rd(8), rd(12), rd(16), rd(20));
            if !minx.is_finite() || !maxz.is_finite() {
                continue;
            }
            let (w, d, h) = (maxx - minx, maxy - miny, maxz - minz);
            if !(1.5..25.0).contains(&h) || w <= 0.2 || d <= 0.2 || w > 20.0 || h / w.max(d) < 1.3 {
                continue;
            }

            let meshes = file.sections_of_type(rw4::SectionType::MESH).count();
            let mut tex_refs: BTreeSet<u32> = BTreeSet::new();
            for mat in file.sections_of_type(rw4::SectionType::MATERIAL) {
                if let Ok(rw4::MaterialSection::Decoded(m)) =
                    file.decode_material(&data, mat.number)
                {
                    for r in &m.texture_refs {
                        tex_refs.insert(r.texture_instance);
                    }
                }
            }
            let mut embedded = Vec::new();
            for (i, tex) in file.sections_of_type(rw4::SectionType::TEXTURE).enumerate() {
                let Ok(t) = file.decode_texture(&data, tex.number) else { continue };
                let Ok(px) = t.decode_top_mip_rgba() else { continue };
                let (tw, th) = (t.width as usize, t.height as usize);
                let total = (tw * th).max(1);
                let step = (total / 400).max(1);
                let mut green = 0usize;
                let mut sampled = 0usize;
                for j in (0..total).step_by(step) {
                    let (r, g, b) =
                        (px[j * 4] as f32, px[j * 4 + 1] as f32, px[j * 4 + 2] as f32);
                    sampled += 1;
                    if g > 40.0 && g > r * 1.25 && g > b * 1.25 {
                        green += 1;
                    }
                }
                let mut a_min = 255u8;
                let mut a_max = 0u8;
                for j in 0..total {
                    a_min = a_min.min(px[j * 4 + 3]);
                    a_max = a_max.max(px[j * 4 + 3]);
                }
                let ratio = green as f32 / sampled as f32;
                if ratio > 0.10 {
                    let out = format!("tmp/tree_models/tex_{:08X}_{}.png", e.id.instance, i);
                    let img = image::RgbaImage::from_fn(tw as u32, th as u32, |x, y| {
                        let at = (x as usize + y as usize * tw) * 4;
                        image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
                    });
                    let _ = img.save(&out);
                }
                embedded.push(format!(
                    "#{i} {}x{} 绿{:.0}% a[{}..{}]",
                    tw, th, ratio * 100.0, a_min, a_max
                ));
            }
            let refs: Vec<String> = tex_refs.iter().map(|r| format!("{r:08X}")).collect();
            println!(
                "{path}: I {:08X} {}B {}mesh bbox {:.1}x{:.1}x{:.1} texref[{}] embedded[{}]",
                e.id.instance,
                data.len(),
                meshes,
                w,
                d,
                h,
                refs.join(","),
                embedded.join(" ")
            );
        }
    }
}
