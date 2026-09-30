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
/// RW4 纹理资源类型（破洞/interior 家族的 raster 载体，DXT 压缩）。
const RW4_TEXTURE_TYPE: u32 = 0x2F4E_681B;

/// 解码条目 raster：裸 Raster（pixFmt21，四色映射）或 RW4 纹理（DXT 直解，
/// 自带颜色与光衰减 alpha——破洞/interior 家族）。
fn decode_entry_raster(
    packages: &[Package],
    key: &sc_properties::Key,
    colors: Option<[[u8; 4]; 4]>,
) -> Option<(u32, u32, Vec<u8>)> {
    for package in packages {
        for type_id in [RASTER_IMAGE_TYPE, RW4_TEXTURE_TYPE] {
            let Some(entry) = package.entries().iter().find(|entry| {
                entry.id.type_id == type_id && entry.id.instance == key.instance
            }) else {
                continue;
            };
            let Ok(bytes) = package.read(entry) else { continue };
            if type_id == RASTER_IMAGE_TYPE {
                let Ok(raster) = rw4::RasterImage::parse(&bytes) else { continue };
                if !raster.is_raw_rgba() {
                    continue;
                }
                let rgba = match colors {
                    Some(colors) => raster.decode_lot_mask_rgba(&colors).ok()?,
                    None => raster.decode_top_mip_rgba().ok()?,
                };
                return Some((u32::from(raster.width), u32::from(raster.height), rgba));
            }
            let Ok(file) = rw4::Rw4File::parse(&bytes) else { continue };
            for section in file.sections_of_type(rw4::SectionType::TEXTURE) {
                let Ok(texture) = file.decode_texture(&bytes, section.number) else {
                    continue;
                };
                let Ok(rgba) = texture.decode_top_mip_rgba() else { continue };
                return Some((
                    u32::from(texture.width),
                    u32::from(texture.height),
                    rgba,
                ));
            }
        }
    }
    None
}

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
                let colors = decal.colors_rgba8();
                // 有 colors → 四色映射（招牌/涂鸦）；无 colors → RW4 自彩
                // 直解（破洞/interior 家族）。
                let raster_key = match decal.raster.as_ref() {
                    Some(key) => key,
                    None => continue,
                };
                let Some((width, height, rgba)) =
                    decode_entry_raster(&packages, raster_key, colors)
                else {
                    continue;
                };
                if width < 16 || height < 16 {
                    continue;
                }
                let bitmap =
                    image::RgbaImage::from_raw(width as u32, height as u32, rgba.clone())
                        .expect("decal buf");
                scanned += 1;
                // 缩略图上板（棋盘底显透明）——全量条目。
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
                tsv.push_str(&format!(
                    "{}\t{}\t{:08x}\t{:08x}\t{}\t{:08x}\t{}\t{}\n",
                    thumb_count,
                    pkg_idx,
                    entry.id.instance,
                    entry.id.group,
                    decal.index,
                    decal.id.map_or(0, |k| k.instance),
                    width,
                    height
                ));
                thumb_count += 1;
                if !matches_labs(&rgba, width as usize, height as usize) {
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
                    width,
                    height,
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
