//! slot1/2/3/palette 通道统计探针（5a 调试，只读）：
//! 症状：5a spec 链上线后质感无显著变化。假设：shaderMap.b≈0（源码取 .b）或
//! palette alpha 恒定。输出各槽位逐通道 min/max/均值 + 高低分桶 + clip 窗口内统计。
//!
//! 用法：cargo run -p rw4 --release --example shader_map_stats -- <package> 0xMODEL [跨包...]

const RW4_IMAGE: u32 = 0x2F4E_681B;
const RASTER_IMAGE: u32 = 0x2F4E_681C;

fn stats(rgba: &[u8], ch: usize) -> (u8, u8, f32) {
    let mut mn = 255u8;
    let mut mx = 0u8;
    let mut sum = 0u64;
    let mut n = 0u64;
    for px in rgba.as_chunks::<4>().0 {
        let v = px[ch];
        mn = mn.min(v);
        mx = mx.max(v);
        sum += u64::from(v);
        n += 1;
    }
    (mn, mx, if n > 0 { sum as f32 / n as f32 } else { 0.0 })
}

fn buckets(rgba: &[u8], ch: usize) -> [usize; 4] {
    let mut b = [0usize; 4];
    for px in rgba.as_chunks::<4>().0 {
        b[px[ch] as usize / 64] += 1;
    }
    b
}

fn window(rgba: &[u8], w: u32, h: u32, xform: [f32; 4]) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let x0 = (xform[2].clamp(0.0, 1.0) * (w - 1) as f32) as usize;
    let y0 = (xform[3].clamp(0.0, 1.0) * (h - 1) as f32) as usize;
    let x1 = (((xform[2] + xform[0]).clamp(0.0, 1.0) * (w - 1) as f32) as usize).min(w - 1);
    let y1 = (((xform[3] + xform[1]).clamp(0.0, 1.0) * (h - 1) as f32) as usize).min(h - 1);
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let i = (y * w + x) * 4;
            out.extend_from_slice(&rgba[i..i + 4]);
        }
    }
    out
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let dump = args
        .iter()
        .position(|a| a == "--dump")
        .and_then(|i| args.get(i + 1).cloned());
    args.retain(|a| a != "--dump");
    if dump.is_some() {
        args.remove(args.len() - 1); // 目录参数
    }
    let (path, rest) = args.split_first().expect("usage: <package> 0xMODEL [跨包...] [--dump <目录>]");
    let (model, extra) = rest.split_first().expect("missing 0xMODEL");
    let instance = u32::from_str_radix(model.trim_start_matches("0x"), 16).expect("instance");

    let package = dbpf::Package::open(path).expect("open package");
    let extras: Vec<_> = extra
        .iter()
        .map(|p| dbpf::Package::open(p).expect("open extra package"))
        .collect();
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_IMAGE && e.id.instance == instance)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read model");
    let file = rw4::Rw4File::parse(&data).expect("parse rw4");

    let find_rgba = |inst: u32| -> Option<(Vec<u8>, u32, u32)> {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && (e.id.type_id == RASTER_IMAGE || e.id.type_id == RW4_IMAGE))
                .cloned()
            else {
                continue;
            };
            let bytes = pkg.read(&e).ok()?;
            if e.id.type_id == RASTER_IMAGE {
                let raster = rw4::RasterImage::parse(&bytes).ok()?;
                return Some((raster.decode_top_mip_rgba().ok()?, raster.width, raster.height));
            }
            let tex_file = rw4::Rw4File::parse(&bytes).ok()?;
            let sec = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next()?.number;
            let tex = tex_file.decode_texture(&bytes, sec).ok()?;
            return Some((
                tex.decode_top_mip_rgba().ok()?,
                u32::from(tex.width),
                u32::from(tex.height),
            ));
        }
        None
    };

    let material_sections: Vec<u32> = file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .filter_map(|s| {
            matches!(
                file.decode_material(&data, s.number),
                Ok(rw4::MaterialSection::Decoded(_))
            )
            .then_some(s.number)
        })
        .collect();
    if material_sections.is_empty() {
        println!("no decoded material");
        return;
    }
    println!("decoded materials: {material_sections:?}");
    for material_section in material_sections {
        println!("\n======== material section #{material_section} ========");
    let Ok(rw4::MaterialSection::Decoded(mat)) = file.decode_material(&data, material_section)
    else {
        continue;
    };

    // regionXform = 参数表 row1
    let mut xform = [1.0f32, 1.0, 0.0, 0.0];
    if let Some(inst) = mat.slot_texture(0) {
        for pkg in std::iter::once(&package).chain(extras.iter()) {
            let Some(e) = pkg
                .entries()
                .iter()
                .find(|e| e.id.instance == inst && e.id.type_id == RW4_IMAGE)
                .cloned()
            else {
                continue;
            };
            if let Ok(bytes) = pkg.read(&e) {
                if let Ok(tex_file) = rw4::Rw4File::parse(&bytes) {
                    if let Some(sec) = tex_file.sections_of_type(rw4::SectionType::TEXTURE).next() {
                        if let Ok(tex) = tex_file.decode_texture(&bytes, sec.number) {
                            if let Ok(pixels) = tex.decode_palette_f32() {
                                println!("params {} 行：", pixels.len());
                                for (i, r) in pixels.iter().enumerate().take(6) {
                                    println!("  row{i} = ({:.4}, {:.4}, {:.4}, {:.4})", r[0], r[1], r[2], r[3]);
                                }
                                if let Some(r) = pixels.get(1) {
                                    xform = [r[0], r[1], r[2], r[3]];
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
    }
    println!("regionXform(row1) = ({:.4}, {:.4}, {:.4}, {:.4})", xform[0], xform[1], xform[2], xform[3]);

    let names = ["R", "G", "B", "A"];
    // 窗口裁剪逐通道灰度导出（目视语义分析）
    let dump_channel = |tag: &str, rgba: &[u8], w: u32, h: u32, xform: [f32; 4], ch: usize| {
        let dir = match &dump {
            Some(d) => d,
            None => return,
        };
        let win = window(rgba, w, h, xform);
        let (w, h) = (w as usize, h as usize);
        let (x0, y0) = (
            (xform[2].clamp(0.0, 1.0) * (w - 1) as f32) as usize,
            (xform[3].clamp(0.0, 1.0) * (h - 1) as f32) as usize,
        );
        let x1 = (((xform[2] + xform[0]).clamp(0.0, 1.0) * (w - 1) as f32) as usize).min(w - 1);
        let y1 = (((xform[3] + xform[1]).clamp(0.0, 1.0) * (h - 1) as f32) as usize).min(h - 1);
        let (cw, ch_h) = (x1 - x0 + 1, y1 - y0 + 1);
        let gray: Vec<u8> = win
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|px| [px[ch], px[ch], px[ch], 255])
            .collect();
        let path = format!("{dir}\\{tag}_{}.png", names[ch]);
        let saved = image::RgbaImage::from_raw(cw as u32, ch_h as u32, gray)
            .ok_or_else(|| "size mismatch".to_string())
            .and_then(|img| img.save(&path).map_err(|e| e.to_string()));
        match saved {
            Ok(()) => println!("  [dump] {path}"),
            Err(e) => println!("  [dump 失败] {path}: {e}"),
        }
    };
    for slot in [1u32, 2, 3] {
        let Some(inst) = mat.slot_texture(slot) else { continue };
        let Some((rgba, w, h)) = find_rgba(inst) else {
            println!("slot{slot} 0x{inst:08X}: 无法解码");
            continue;
        };
        println!("\nslot{slot} 0x{inst:08X} {w}x{h}（全图 | clip 窗口内）:");
        for ch in 0..4 {
            let (mn, mx, mean) = stats(&rgba, ch);
            let win = window(&rgba, w, h, xform);
            let (wmn, wmx, wmean) = stats(&win, ch);
            let b = buckets(&rgba, ch);
            println!(
                "  {}: min={mn} max={mx} mean={mean:.1} | win {wmn}-{wmx} mean={wmean:.1} | 桶0-64/64-128/128-192/192-256={:?}",
                names[ch], b
            );
        }
        // A>128 与 A<=128 两区内 B 通道均值（窗洞与实体的 spec 分布）
        let win = window(&rgba, w, h, xform);
        if (slot == 1 || slot == 3) && dump.is_some() {
            for ch in 0..4 {
                dump_channel(
                    &format!("m{material_section}_slot{slot}"), &rgba, w, h, xform, ch,
                );
            }
        }
        for (label, hi) in [("A>128 区", true), ("A<=128 区", false)] {
            let mut sub = Vec::new();
            for px in win.as_chunks::<4>().0 {
                if (px[3] > 128) == hi {
                    sub.extend_from_slice(px);
                }
            }
            if !sub.is_empty() {
                let (_, _, bm) = stats(&sub, 2);
                let (_, _, gm) = stats(&sub, 1);
                println!("  窗口内{label}: B均值={bm:.1} G均值={gm:.1} ({}px)", sub.len() / 4);
            }
        }
    }

    // palette：逐逻辑行（物理 2 行）均值
    if let Some(inst) = mat.slot_texture(4) {
        if let Some((rgba, w, h)) = find_rgba(inst) {
            println!("\npalette(slot4) 0x{inst:08X} {w}x{h} 逐逻辑行均值:");
            let (w, h) = (w as usize, h as usize);
            for row in 0..8usize {
                let y0 = row * 2;
                if y0 >= h {
                    break;
                }
                let mut sums = [0u64; 4];
                let mut n = 0u64;
                for y in y0..(y0 + 2).min(h) {
                    for x in 0..w {
                        let px = &rgba[(y * w + x) * 4..(y * w + x) * 4 + 4];
                        for c in 0..4 {
                            sums[c] += u64::from(px[c]);
                        }
                        n += 1;
                    }
                }
                let m = |c: usize| sums[c] as f32 / n as f32;
                println!(
                    "  row{row}: R={:.0} G={:.0} B={:.0} A={:.0}",
                    m(0), m(1), m(2), m(3)
                );
            }
        }
    }
    }
}
