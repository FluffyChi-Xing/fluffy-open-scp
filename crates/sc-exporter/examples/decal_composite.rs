//! decal 合成探针 —— 与 lot_composite 同模式的离线合成实验。
//!
//! 目的：§65.10/§65.15 剩余的唯一问题——**decal 与墙面的输出合并状态**
//! （blend）。变体对象已证不含 blend 状态（§65.15），走实验路线：把真实
//! 包里解码出的 decal（四色掩码 → RGBA），在合成墙面上按不同混合假设
//! 合成，出对比图与游戏截图对拍即可定谳。
//!
//! 假设矩阵：
//!   alpha    out = src·a + dst·(1−a)                 （标准透明混合）
//!   modulate out = src.rgb × dst.rgb                 （调制混合假说，涂鸦候选）
//!   additive out = dst + src.rgb·a                   （加性，霓虹候选）
//!   premul   out = src + dst·(1−a)                   （预乘 over）
//!
//! 墙面两块：平光浅灰 + 带噪深色（blend 假设在明暗底上的表现不同）。
//!
//! ```text
//! cargo run -p sc-exporter --release --example decal_composite -- \
//!   --out=tmp/dynamic/decal_composite \
//!   D:/ea-games/SimCity/SimCityData/SimCity_Game.package \
//!   D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package \
//!   [more lookup packages...]
//! ```
use dbpf::Package;
use sc_properties::{DecalDictionary, PROPERTY_RESOURCE_TYPE, is_decal_dictionary_group};

const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;
/// 每字典采样的条目数（均匀取样）。
const ENTRIES_PER_DICT: usize = 4;
/// 单元格内 decal 的放大倍数（最近邻）。
const SCALE: usize = 2;
/// 合成单元（每假设一格）边长。
const CELL: usize = 160;

#[derive(Clone, Copy, PartialEq)]
enum Blend {
    Alpha,
    Modulate,
    Additive,
    Premul,
}

const BLEND_NAMES: [(Blend, &str); 4] = [
    (Blend::Alpha, "alpha"),
    (Blend::Modulate, "modulate"),
    (Blend::Additive, "additive"),
    (Blend::Premul, "premul"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--out="))
        .map(str::to_owned)
        .unwrap_or_else(|| "tmp/dynamic/decal_composite".to_owned());
    let dicts: Vec<String> = args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--dict="))
        .map(str::to_owned)
        .collect();
    let entries: Vec<usize> = args
        .iter()
        .filter_map(|arg| arg.strip_prefix("--entry="))
        .filter_map(|v| v.parse::<usize>().ok())
        .collect();
    let paths: Vec<&String> = args.iter().filter(|arg| !arg.starts_with("--")).collect();
    if paths.is_empty() {
        eprintln!("usage: decal_composite [--out=<dir>] [--dict=<instance>…] <package> [lookup…]");
        std::process::exit(2);
    }
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("create {out_dir}: {e}"));

    let packages: Vec<Package> = paths
        .iter()
        .map(|path| Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}")))
        .collect();

    // 收集字典（主包 + 各 lookup 包都可能有）。
    let mut found: Vec<(usize, dbpf::IndexEntry)> = Vec::new();
    for (pkg_idx, package) in packages.iter().enumerate() {
        for entry in package.entries().iter() {
            if entry.id.type_id != PROPERTY_RESOURCE_TYPE
                || !is_decal_dictionary_group(entry.id.group)
            {
                continue;
            }
            if !dicts.is_empty()
                && !dicts.iter().any(|d| {
                    u32::from_str_radix(d.trim_start_matches("0x"), 16) == Ok(entry.id.instance)
                })
            {
                continue;
            }
            found.push((pkg_idx, entry.clone()));
        }
    }
    println!("字典总数: {}", found.len());
    let mut processed = 0usize;
    for (pkg_idx, entry) in &found {
        let package = &packages[*pkg_idx];
        let Ok(data) = package.read(entry) else { continue };
        let Ok(dictionary) = DecalDictionary::parse(&data) else { continue };
        // 可解码条目；--entry= 指定时精确取该条目号，否则均匀取样。
        let decodable: Vec<_> = dictionary
            .entries
            .iter()
            .filter(|decal| {
                decal.colors_rgba8().is_some()
                    && resolve_raster(&packages, decal.raster.map(|k| k.instance))
                        .is_some_and(|(image, _)| image.is_raw_rgba())
            })
            .filter(|decal| {
                entries.is_empty() || entries.contains(&decal.index)
            })
            .collect();
        if decodable.is_empty() {
            continue;
        }
        let picked: Vec<_> = if entries.is_empty() {
            let step = decodable.len().div_ceil(ENTRIES_PER_DICT).max(1);
            decodable.into_iter().step_by(step).take(ENTRIES_PER_DICT).collect()
        } else {
            decodable
        };
        println!(
            "字典 0x{:08x} (group {:08x}): {} 条可解，取 {} 条",
            entry.id.instance,
            entry.id.group,
            dictionary.entries.len(),
            picked.len()
        );
        let sheet = compose_dictionary(&packages, &dictionary, &picked);
        let name = format!(
            "{out_dir}/dict_{:08x}_g{:04x}_sheet.png",
            entry.id.instance,
            entry.id.group as u16
        );
        sheet.save(&name).unwrap_or_else(|e| panic!("save {name}: {e}"));
        println!("  -> {name}");
        processed += 1;
        if processed >= 6 {
            break;
        }
    }
    println!("done: {processed} 张对比图 -> {out_dir}");
}

/// 单字典对比图：行 = 条目，列 = [源图(棋盘)] + [4 假设 × 2 墙面]。
fn compose_dictionary(
    packages: &[Package],
    dictionary: &DecalDictionary,
    picked: &[&sc_properties::DecalEntry],
) -> image::RgbaImage {
    let src_cols = 1usize;
    let hyp_cols = BLEND_NAMES.len() * 2; // 4 假设 × 2 墙
    let cols = src_cols + hyp_cols;
    let rows = picked.len();
    let mut sheet = image::RgbaImage::new(
        (cols * (CELL + 4) + 4) as u32,
        (rows * (CELL + 4) + 4) as u32,
    );
    let light = wall_light();
    let dark = wall_dark();

    for (row, decal) in picked.iter().enumerate() {
        let colors = decal.colors_rgba8().expect("picked decodable");
        let (image, _) = resolve_raster(packages, decal.raster.map(|k| k.instance))
            .expect("picked decodable");
        let rgba = image
            .decode_lot_mask_rgba(&colors)
            .expect("raw rgba decode");
        let src = image::RgbaImage::from_raw(image.width, image.height, rgba)
            .expect("decal buf");
        // 自适应缩放：最长边 ≤ CELL−10，保证不溢出单元格。
        let max_side = (image.width as usize).max(image.height as usize);
        let fit = ((CELL - 10) as f32 / max_side as f32).floor().max(1.0) as usize;
        let scaled = image::imageops::resize(
            &src,
            (image.width as usize * fit) as u32,
            (image.height as usize * fit) as u32,
            image::imageops::FilterType::Nearest,
        );
        let oy = (row * (CELL + 4) + 4) as i64;
        // 列 0：源图（棋盘底，看 alpha 覆盖）。
        paste_checkerboard(&mut sheet, 4, oy, CELL);
        overlay_centered(&mut sheet, &scaled, 4, oy);
        // 列 1..：4 假设 × 2 墙（居中偏移下单次混合）。
        let (off_x, off_y) = centered_offset(&scaled);
        for (hi, (blend, _)) in BLEND_NAMES.iter().enumerate() {
            for (wi, wall) in [&light, &dark].iter().enumerate() {
                let cx = (src_cols + hi * 2 + wi) * (CELL + 4) + 4;
                let mut scratch = (*wall).clone();
                blend_onto(&mut scratch, &scaled, *blend, off_x, off_y);
                image::imageops::overlay(&mut sheet, &scratch, cx as i64, oy);
            }
        }
    }
    sheet
}

/// 混合假设（在 (ox,oy) 处单次合成；src a=0 处保持墙面）。
fn blend_onto(
    wall: &mut image::RgbaImage,
    src: &image::RgbaImage,
    blend: Blend,
    ox: i64,
    oy: i64,
) {
    for (sx, sy, s_px) in src.enumerate_pixels() {
        let a = s_px[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let dx = ox + sx as i64;
        let dy = oy + sy as i64;
        if dx < 0 || dy < 0 || dx >= wall.width() as i64 || dy >= wall.height() as i64 {
            continue;
        }
        let (dx, dy) = (dx as u32, dy as u32);
        let d_px = *wall.get_pixel(dx, dy);
        let (dr, dg, db) = (d_px[0] as f32, d_px[1] as f32, d_px[2] as f32);
        let (sr, sg, sb) = (s_px[0] as f32, s_px[1] as f32, s_px[2] as f32);
        let (nr, ng, nb) = match blend {
            Blend::Alpha => (
                sr * a + dr * (1.0 - a),
                sg * a + dg * (1.0 - a),
                sb * a + db * (1.0 - a),
            ),
            Blend::Modulate => (sr * dr / 255.0, sg * dg / 255.0, sb * db / 255.0),
            Blend::Additive => (dr + sr * a, dg + sg * a, db + sb * a),
            Blend::Premul => (sr + dr * (1.0 - a), sg + dg * (1.0 - a), sb + db * (1.0 - a)),
        };
        let out = wall.get_pixel_mut(dx, dy);
        out[0] = nr.round().clamp(0.0, 255.0) as u8;
        out[1] = ng.round().clamp(0.0, 255.0) as u8;
        out[2] = nb.round().clamp(0.0, 255.0) as u8;
    }
}

/// 居中放置的偏移（与 blend_onto 共用）。
fn centered_offset(src: &image::RgbaImage) -> (i64, i64) {
    (
        ((CELL as i64 - src.width() as i64) / 2).max(0),
        ((CELL as i64 - src.height() as i64) / 2).max(0),
    )
}

fn overlay_centered(dst: &mut image::RgbaImage, src: &image::RgbaImage, ox: usize, oy: i64) {
    let x = ox as i64 + ((CELL as i64 - src.width() as i64) / 2).max(0);
    let y = oy + ((CELL as i64 - src.height() as i64) / 2).max(0);
    image::imageops::overlay(dst, src, x, y);
}

fn paste_checkerboard(dst: &mut image::RgbaImage, ox: usize, oy: i64, size: usize) {
    for y in 0..size {
        for x in 0..size {
            let v = if (x / 16 + y / 16) % 2 == 0 { 90 } else { 130 };
            dst.put_pixel(
                (ox + x) as u32,
                (oy as usize + y) as u32,
                image::Rgba([v, v, v, 255]),
            );
        }
    }
}

/// 平光浅灰墙（轻微竖向渐变）。
fn wall_light() -> image::RgbaImage {
    let mut img = image::RgbaImage::new(CELL as u32, CELL as u32);
    for y in 0..CELL {
        for x in 0..CELL {
            let n = ((x as i64 * 7 + y as i64 * 13) % 5) as i64 - 2;
            let base = 190 - (y * 12 / CELL) as i64 + n;
            let v = base.clamp(0, 255) as u8;
            img.put_pixel(x as u32, y as u32, image::Rgba([v, v - 2, v - 6, 255]));
        }
    }
    img
}

/// 深色墙（噪声 + 暗色）。
fn wall_dark() -> image::RgbaImage {
    let mut img = image::RgbaImage::new(CELL as u32, CELL as u32);
    for y in 0..CELL {
        for x in 0..CELL {
            let n = ((x as i64 * 11 + y as i64 * 7) % 9) as i64 - 4;
            let v = (58 + n).clamp(0, 255) as u8;
            img.put_pixel(x as u32, y as u32, image::Rgba([v, v - 4, v - 2, 255]));
        }
    }
    img
}

fn resolve_raster(packages: &[Package], instance: Option<u32>) -> Option<(rw4::RasterImage, String)> {
    let instance = instance?;
    for package in packages {
        let Some(entry) = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == RASTER_IMAGE_TYPE && entry.id.instance == instance)
        else {
            continue;
        };
        let Ok(bytes) = package.read(entry) else { continue };
        let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
        let name = package
            .path()
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        return Some((raster, name));
    }
    None
}
