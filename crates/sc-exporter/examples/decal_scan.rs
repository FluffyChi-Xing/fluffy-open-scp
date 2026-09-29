//! decal 特征扫描：全包解码全部 decal 条目，按像素签名过滤目标贴花。
//! （2026-09-30 找 "INDUSTRIAL LABS" 墙面广告牌：深绿框 + 分子结构 +
//!   橄榄色 INDUSTRIAL 字 + 深绿 LABS，框内大量透明镂空。）
//!
//! ```text
//! cargo run -p sc-exporter --release --example decal_scan -- \
//!   --out=tmp/dynamic/decal_scan \
//!   D:/ea-games/SimCity/SimCityData/SimCity_Game.package \
//!   D:/ea-games/SimCity/SimCityData/SimCity_Graphics.package \
//!   "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/SimCityDataEP1.package"
//! ```
use dbpf::Package;
use sc_properties::{DecalDictionary, PROPERTY_RESOURCE_TYPE, is_decal_dictionary_group};

const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir = args
        .iter()
        .find_map(|arg| arg.strip_prefix("--out="))
        .map(str::to_owned)
        .unwrap_or_else(|| "tmp/dynamic/decal_scan".to_owned());
    let dump_all = args.iter().any(|arg| arg == "--dump-all");
    std::fs::create_dir_all(&out_dir).unwrap_or_else(|e| panic!("create {out_dir}: {e}"));
    let packages: Vec<Package> = args
        .iter()
        .filter(|arg| !arg.starts_with("--"))
        .map(|path| Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}")))
        .collect();

    let mut scanned = 0usize;
    let mut hits = 0usize;
    let mut skipped_compressed = 0usize;
    let mut thumb_sheet = image::RgbaImage::new(24 * 56 + 8, 4 * 1000); // 预留高，末尾裁
    let mut thumb_count = 0usize;
    const THUMB_COLS: usize = 24;
    let mut tsv = String::from("thumb\tpkg\tdict\tgroup\tentry\tid\tw\th\n");
    for (pkg_idx, package) in packages.iter().enumerate() {
        for entry in package.entries().iter() {
            if entry.id.type_id != PROPERTY_RESOURCE_TYPE
                || !is_decal_dictionary_group(entry.id.group)
            {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(dictionary) = DecalDictionary::parse(&data) else { continue };
            for decal in &dictionary.entries {
                let Some(colors) = decal.colors_rgba8() else { continue };
                let Some((image, _)) =
                    resolve_raster(&packages, decal.raster.map(|k| k.instance))
                else {
                    continue;
                };
                if !image.is_raw_rgba() {
                    skipped_compressed += 1;
                    continue;
                }
                if image.width < 16 || image.height < 16 {
                    continue;
                }
                let Ok(rgba) = image.decode_lot_mask_rgba(&colors) else { continue };
                let Some(bitmap) =
                    image::RgbaImage::from_raw(image.width, image.height, rgba.clone())
                else {
                    continue;
                };
                scanned += 1;
                // 缩略图上板（棋盘底显透明）。
                let tx = (thumb_count % THUMB_COLS) * 56 + 4;
                let ty = (thumb_count / THUMB_COLS) * 56 + 4;
                for y in 0..48usize {
                    for x in 0..48usize {
                        let v = if (x / 6 + y / 6) % 2 == 0 { 80 } else { 55 };
                        thumb_sheet.put_pixel(
                            (tx + x) as u32,
                            (ty + y) as u32,
                            image::Rgba([v, v, v, 255]),
                        );
                    }
                }
                let thumb = image::imageops::resize(
                    &bitmap,
                    48,
                    48,
                    image::imageops::FilterType::Nearest,
                );
                image::imageops::overlay(&mut thumb_sheet, &thumb, tx as i64, ty as i64);
                let label = format!(
                    "{}{}e{:03}",
                    ["G", "X", "E"][pkg_idx.min(2)],
                    if entry.id.group as u16 == 0xb185 { "*" } else { "" },
                    decal.index
                );
                let _ = label;
                tsv.push_str(&format!(
                    "{}\t{}\t{:08x}\t{:08x}\t{}\t{:08x}\t{}\t{}\n",
                    thumb_count,
                    pkg_idx,
                    entry.id.instance,
                    entry.id.group,
                    decal.index,
                    decal.id.map_or(0, |k| k.instance),
                    image.width,
                    image.height
                ));
                thumb_count += 1;
                if !matches_labs(&rgba, image.width as usize, image.height as usize) {
                    continue;
                }
                hits += 1;
                let name = format!(
                    "{out_dir}/hit_{}_{:08x}_g{:04x}_e{:03}.png",
                    package
                        .path()
                        .file_name()
                        .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                        .replace("SimCity_", "")
                        .replace(".package", ""),
                    decal.id.map_or(0, |k| k.instance),
                    entry.id.group as u16,
                    decal.index
                );
                bitmap.save(&name).unwrap_or_else(|e| panic!("save {name}: {e}"));
                println!(
                    "HIT 字典 0x{:08x} group {:08x} entry[{}] id=0x{:08x} {}x{} -> {name}",
                    entry.id.instance,
                    entry.id.group,
                    decal.index,
                    decal.id.map_or(0, |k| k.instance),
                    image.width,
                    image.height,
                );
            }
        }
    }
    // 裁掉多余高度并存盘。
    let rows = thumb_count.div_ceil(THUMB_COLS);
    let cropped =
        image::imageops::crop(&mut thumb_sheet, 0, 0, 24 * 56 + 8, rows as u32 * 56 + 8);
    let mut cropped = cropped.to_image();
    // 行标：每行左缘写行号点阵（简单色条区分行组）。
    for r in 0..rows {
        let tint = match r % 4 {
            0 => image::Rgba([120, 0, 0, 255]),
            1 => image::Rgba([0, 90, 0, 255]),
            2 => image::Rgba([0, 0, 120, 255]),
            _ => image::Rgba([90, 0, 90, 255]),
        };
        for y in 0..4 {
            for x in 0..(24 * 56 + 8) {
                cropped.put_pixel(x as u32, (r * 56 + y) as u32, tint);
            }
        }
    }
    cropped
        .save(format!("{out_dir}/all_entries_sheet.png"))
        .unwrap_or_else(|e| panic!("save sheet: {e}"));
    std::fs::write(format!("{out_dir}/thumbs.tsv"), &tsv)
        .unwrap_or_else(|e| panic!("write tsv: {e}"));
    println!(
        "scanned {scanned} decodable entries (compressed-skip {skipped_compressed}), {hits} hits; \
         全量缩略图 -> {out_dir}/all_entries_sheet.png ({rows} 行 × 24 列，行首色条标行组)"
    );
}

/// INDUSTRIAL LABS 签名：深绿框/字 + 橄榄字 + 大量透明镂空（海报镂空）。
fn matches_labs(rgba: &[u8], w: usize, h: usize) -> bool {
    let mut green = 0usize;
    let mut olive = 0usize;
    let mut transparent = 0usize;
    let mut total = 0usize;
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            let (r, g, b, a) = (rgba[at] as i32, rgba[at + 1] as i32, rgba[at + 2] as i32, rgba[at + 3]);
            total += 1;
            if a == 0 {
                transparent += 1;
                continue;
            }
            if g > 55 && g > r + 12 && g > b + 12 && r < 110 {
                green += 1;
            }
            if r >= 85 && r <= 175 && g >= 75 && g <= 165 && b <= 95 && r > b + 25 && (r - b) > 25
            {
                olive += 1;
            }
        }
    }
    let f = |n: usize| n as f32 / total as f32;
    f(transparent) > 0.12 && f(green) > 0.08 && f(olive) > 0.03 && f(green) < 0.6
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
        return Some((raster, String::new()));
    }
    None
}
