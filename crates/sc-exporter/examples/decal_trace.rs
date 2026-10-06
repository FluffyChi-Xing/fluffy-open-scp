//! 冒烟工具：单条 decal 的解码语义追踪——R/G/B/A 四通道独立成像 +
//! 调色板 + 二值/加权两种渲染并排，用于定谳掩码通道语义。
//! 用法：cargo run -p sc-exporter --release --example decal_trace -- <id_instance_hex> <pkg> [pkg...]
//! 输出：tmp/dynamic/trace_<id>/ 目录（通道图/调色板/渲染对比）。
use dbpf::Package;
use sc_properties::{DecalDictionary, PROPERTY_RESOURCE_TYPE, is_decal_dictionary_group};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(id) = args.first().map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).expect("hex")) else {
        panic!("用法: decal_trace <id_instance_hex> <pkg> [pkg...]");
    };
    let packages: Vec<(String, Package)> = args
        .iter()
        .skip(1)
        .map(|p| {
            let pkg = Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}"));
            let name = p.rsplit(['/', '\\']).next().unwrap().to_string();
            (name, pkg)
        })
        .collect();

    let out = std::path::Path::new("tmp/dynamic/trace");
    std::fs::create_dir_all(out.join("channels")).unwrap();

    // 找字典与条目
    for (name, pkg) in &packages {
        for e in pkg.entries() {
            if e.id.type_id != PROPERTY_RESOURCE_TYPE || !is_decal_dictionary_group(e.id.group) {
                continue;
            }
            let Ok(data) = pkg.read(e) else { continue };
            let Ok(dict) = DecalDictionary::parse(&data) else { continue };
            let Some(pos) = dict.entries.iter().position(|en| en.id.map(|k| k.instance) == Some(id)) else {
                continue;
            };
            let entry = &dict.entries[pos];
            let colors = entry.colors_rgba8();
            println!(
                "字典 {}:{:08x} 条目[{}] id={:08x} aspect={:?} 有色={}",
                name,
                e.id.group,
                pos,
                id,
                entry.aspect_ratio,
                colors.is_some()
            );
            if let Some(c) = colors {
                println!(
                    "  调色板: color1={:?} color2={:?} color3={:?} color4={:?}",
                    c[0], c[1], c[2], c[3]
                );
            }
            // raster
            let Some(raster_key) = entry.raster.clone() else {
                println!("  无 raster"); 
                continue;
            };
            let mut found = None;
            for (n2, p2) in &packages {
                if let Some(e2) = p2
                    .entries()
                    .iter()
                    .find(|e2| e2.id.type_id == 0x2F4E_681C && e2.id.instance == raster_key.instance)
                    .cloned()
                {
                    found = Some((n2.clone(), p2.clone(), e2));
                    break;
                }
            }
            let Some((pname, rpkg, re)) = found else {
                println!("  raster {:08x} 未找到（可能在基础包）", raster_key.instance);
                continue;
            };
            let rdata = rpkg.read(&re).expect("read raster");
            let raster = rw4::RasterImage::parse(&rdata).expect("parse raster");
            let raw = raster.decode_top_mip_rgba().expect("decode");
            let (w, h) = (raster.width as u32, raster.height as u32);
            println!("  raster {}x{} pixFmt {}", w, h, raster.pixel_format);

            use image::RgbaImage;
            // 四通道独立成像（白底、通道值作为黑度/白度）
            for (ci, cname) in ["A", "R", "G", "B"].iter().enumerate() {
                let mut img = RgbaImage::new(w, h);
                for (x, y, p) in img.enumerate_pixels_mut() {
                    let v = raw[(y * w + x) as usize * 4 + ci];
                    *p = image::Rgba([255 - v, 255 - v, 255 - v, 255]);
                }
                let f = out.join("channels").join(format!("trace_{id:08x}_{cname}.png"));
                image::imageops::flip_vertical(&img).save(&f).unwrap();
                println!("  通道 {cname} -> {}", f.display());
            }
            // 调色板色卡
            if let Some(c) = colors {
                let mut img = RgbaImage::new(64 * 4, 64);
                for (i, col) in c.iter().enumerate() {
                    for x in 0..64usize {
                        for y in 0..64usize {
                            let px_x = (i * 64 + x) as u32;
                            let px_y = y as u32;
                            img.put_pixel(px_x, px_y, image::Rgba(*col));
                        }
                    }
                }
                let f = out.join(format!("trace_{id:08x}_palette.png"));
                image::imageops::flip_vertical(&img).save(&f).unwrap();
                println!("  调色板色卡 -> {}", f.display());
            }
            return;
        }
    }
    println!("全包未找到 id {id:08x}");
}
