//! 离线重放前端 refinedGround 的合成算法（阈值+优先级链+frac 平铺），
//! 用于裁决优先级方向等口径问题。输出 PNG 与游戏截图对拍。
//!
//! 用法：refined_replay <lot_package> <lot_instance_hex> --out=<dir>
//!        [--priority=abgr|rgba] [--tiles=N] [lookup_package...]

use dbpf::{IndexEntry, Package};
use sc_properties::{Key, Kind, PropertyFile, Value};

const LOT_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;

const H_LOT_MASK: u32 = 0x0CCB_7FD5;
const H_LOT_TEXTURES: u32 = 0x0CCB_7FD4;
const H_LOT_COLORS: [u32; 4] = [0x0D02_D586, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];

const ATLAS_COLS: usize = 4;

struct Args {
    lot_package: String,
    lot_instance: u32,
    out_dir: String,
    /// abgr = A>B>G>R（引擎 shader w→z→y→x 的现行解读）；rgba = LC1>LC2>LC3>LC4。
    priority_high_a: bool,
    tiles: f32,
    placeholders: bool,
    lookups: Vec<String>,
}

fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.out_dir).unwrap();

    let mut paths = vec![args.lot_package.clone()];
    paths.extend(args.lookups.iter().cloned());
    let packages: Vec<Package> = paths
        .iter()
        .map(|path| Package::open(path).unwrap_or_else(|error| panic!("open {path}: {error}")))
        .collect();

    // lot property
    let (owner, entry) = find_entry(&packages, LOT_TYPE, args.lot_instance, None)
        .unwrap_or_else(|| panic!("lot property 0x{:08X} not found", args.lot_instance));
    let data = packages[owner].read(&entry).unwrap();
    let file = PropertyFile::parse(&data).expect("parse lot property");
    let mask_key = key_at(&file, H_LOT_MASK).expect("no LotMask");
    let textures_key = key_at(&file, H_LOT_TEXTURES);
    let colors: Vec<[u8; 4]> = H_LOT_COLORS
        .iter()
        .map(|hash| {
            color_at(&file, *hash)
                .map(|c| [srgb(c[0]), srgb(c[1]), srgb(c[2]), c[3].clamp(0.0, 15.0) as u8])
                .unwrap_or([0, 0, 0, 0])
        })
        .collect();

    // LotMask raw（与后端一致：decode_top_mip + 行序翻转）
    let (mask_owner, mask_entry) = find_entry(&packages, RASTER_TYPE, mask_key.instance, None)
        .unwrap_or_else(|| panic!("LotMask raster 0x{:08X} not found", mask_key.instance));
    let mask_bytes = packages[mask_owner].read(&mask_entry).unwrap();
    let raster = rw4::RasterImage::parse(&mask_bytes).expect("parse LotMask");
    let (w, h) = (raster.width as usize, raster.height as usize);
    let mut raw = raster.decode_top_mip_rgba().expect("decode LotMask");
    for row in 0..h / 2 {
        let top = row * w * 4;
        let bottom = (h - 1 - row) * w * 4;
        for i in 0..w * 4 {
            raw.swap(top + i, bottom + i);
        }
    }

    // 共享图集；--placeholders=1 时模拟 surface 缺失，回退本地占位 tile。
    let use_placeholders = args.placeholders;
    let atlas: Vec<u8> = if use_placeholders {
        Vec::new()
    } else {
        textures_key
            .and_then(|key| find_entry(&packages, RW4_TYPE, key.instance, None))
            .map(|(tex_owner, tex_entry)| {
                let bytes = packages[tex_owner].read(&tex_entry).unwrap();
                let rw4_file = rw4::Rw4File::parse(&bytes).expect("parse Lot Textures");
                let section = rw4_file
                    .sections_of_type(rw4::SectionType::TEXTURE)
                    .next()
                    .expect("no texture section")
                    .number;
                let texture = rw4_file.decode_texture(&bytes, section).expect("decode");
                texture.decode_top_mip_rgba().expect("pixels")
            })
            .unwrap_or_default()
    };
    let atlas_w = if atlas.is_empty() { 0 } else { 1024 };
    let cell = if atlas.is_empty() { 64 } else { atlas_w / ATLAS_COLS };
    // 占位 tile：src/assets/ground/<n>.png（与前端 tile() 回退一致），预加载 16 格。
    let placeholder_dir = "src/assets/ground";
    let placeholders: Vec<Option<(usize, usize, Vec<u8>)>> = (0..16)
        .map(|index| {
            image::open(format!("{placeholder_dir}/{}.png", index))
                .ok()
                .map(|img| {
                    let img = img.to_rgba8();
                    (img.width() as usize, img.height() as usize, img.into_raw())
                })
        })
        .collect();

    let scale = 4usize;
    let out_w = w * scale;
    let out_h = h * scale;
    let mut out = vec![0u8; out_w * out_h * 4];
    let priority: [usize; 4] = if args.priority_high_a {
        [3, 2, 1, 0]
    } else {
        [0, 1, 2, 3]
    };

    for y in 0..out_h {
        for x in 0..out_w {
            let at = (y * out_w + x) * 4;
            // 画布 UV → raw 双线性权重（与前端 sampleWeights 同式）
            let u = x as f32 / out_w as f32;
            let v = y as f32 / out_h as f32;
            let weights = sample_weights(&raw, w, h, u, v);
            // 阈值 + 优先级链
            let mut channel: i32 = -1;
            for &c in &priority {
                if weights[c] > 0.5 {
                    channel = c as i32;
                    break;
                }
            }
            // tile 源：胜出通道 cell_{A}；未覆盖 → cell 8
            let tile_index = if channel >= 0 {
                colors[channel as usize][3] as usize % 16
            } else {
                8
            };
            let tint: [u8; 3] = if channel >= 0 {
                [
                    colors[channel as usize][0],
                    colors[channel as usize][1],
                    colors[channel as usize][2],
                ]
            } else {
                [255, 255, 255]
            };
            // frac(uv × tiles) 平铺
            let tiles = args.tiles;
            let fu = u * tiles;
            let fv = v * tiles;
            let (tw, th, src_pixels): (usize, usize, &[u8]) = if atlas.is_empty() {
                match placeholders[tile_index].as_ref() {
                    Some(t) => (t.0, t.1, &t.2),
                    None => {
                        out[at] = 58;
                        out[at + 1] = 62;
                        out[at + 2] = 54;
                        out[at + 3] = 255;
                        continue;
                    }
                }
            } else {
                (cell, cell, &atlas)
            };
            let px = (((fu - fu.floor()) * tw as f32) as usize).min(tw - 1);
            let py = (((fv - fv.floor()) * th as f32) as usize).min(th - 1);
            let src = (py * tw + px) * 4;
            for ch in 0..3 {
                out[at + ch] =
                    (u16::from(src_pixels[src + ch]) * u16::from(tint[ch]) / 255).min(255) as u8;
            }
            out[at + 3] = 255;
        }
    }

    let name = format!(
        "refined_{}_{}.png",
        if args.priority_high_a { "abgr" } else { "rgba" },
        args.lot_instance
    );
    let image = image::RgbaImage::from_raw(out_w as u32, out_h as u32, out).unwrap();
    image
        .save(format!("{}/{}", args.out_dir, name))
        .unwrap();
    println!("saved {}/{}", args.out_dir, name);

    // 区域裁决图（mask 原分辨率）：R=LC1 G=LC2 B=LC3 A=LC4，白=未覆盖。
    let mut regions = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let at = (y * w + x) * 4;
            let weights = sample_weights(&raw, w, h, x as f32 / w as f32, y as f32 / h as f32);
            let mut channel: i32 = -1;
            for &c in &priority {
                if weights[c] > 0.5 {
                    channel = c as i32;
                    break;
                }
            }
            let color: [u8; 3] = match channel {
                0 => [255, 0, 0],
                1 => [0, 255, 0],
                2 => [0, 0, 255],
                3 => [255, 255, 0],
                _ => [255, 255, 255],
            };
            regions[at..at + 3].copy_from_slice(&color);
            regions[at + 3] = 255;
        }
    }
    let region_image = image::RgbaImage::from_raw(w as u32, h as u32, regions).unwrap();
    let region_name = format!(
        "regions_{}_{}.png",
        if args.priority_high_a { "abgr" } else { "rgba" },
        args.lot_instance
    );
    region_image
        .save(format!("{}/{}", args.out_dir, region_name))
        .unwrap();
    println!("saved {}/{}", args.out_dir, region_name);
}

fn sample_weights(raw: &[u8], w: usize, h: usize, u: f32, v: f32) -> [f32; 4] {
    let fx = u * (w - 1) as f32;
    let fy = v * (h - 1) as f32;
    let x0 = fx as usize;
    let y0 = fy as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let read = |px: usize, py: usize| -> [f32; 4] {
        let at = (py * w + px) * 4;
        [
            raw[at] as f32 / 255.0,
            raw[at + 1] as f32 / 255.0,
            raw[at + 2] as f32 / 255.0,
            raw[at + 3] as f32 / 255.0,
        ]
    };
    let (c00, c10, c01, c11) = (read(x0, y0), read(x1, y0), read(x0, y1), read(x1, y1));
    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let top = c00[c] + (c10[c] - c00[c]) * tx;
        let bottom = c01[c] + (c11[c] - c01[c]) * tx;
        out[c] = top + (bottom - top) * ty;
    }
    out
}

fn srgb(linear: f32) -> u8 {
    let linear = linear.clamp(0.0, 1.0);
    let s = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

fn parse_args() -> Args {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = raw_args.iter().filter(|a| !a.starts_with("--")).collect();
    let flag = |name: &str| -> Option<String> {
        raw_args
            .iter()
            .find_map(|a| a.strip_prefix(&format!("--{name}=")).map(str::to_owned))
    };
    let parse_hex = |value: &str| {
        u32::from_str_radix(value.trim_start_matches("0x"), 16)
            .unwrap_or_else(|error| panic!("bad hex {value}: {error}"))
    };
    Args {
        lot_package: positional[0].clone(),
        lot_instance: parse_hex(&positional[1]),
        out_dir: flag("out").unwrap_or_else(|| "tmp/refined_replay".to_owned()),
        priority_high_a: flag("priority").as_deref() != Some("rgba"),
        tiles: flag("tiles")
            .map(|v| v.parse().unwrap_or_else(|e| panic!("bad --tiles: {e}")))
            .unwrap_or(5.0),
        placeholders: flag("placeholders").as_deref() == Some("1"),
        lookups: positional[2..].iter().map(|s| (*s).clone()).collect(),
    }
}

fn find_entry(
    packages: &[Package],
    type_id: u32,
    instance: u32,
    group: Option<u32>,
) -> Option<(usize, IndexEntry)> {
    for (index, package) in packages.iter().enumerate() {
        for entry in package.entries() {
            if entry.id.type_id == type_id
                && entry.id.instance == instance
                && group.is_none_or(|value| entry.id.group == value)
            {
                return Some((index, entry.clone()));
            }
        }
    }
    None
}

fn key_at(file: &PropertyFile, hash: u32) -> Option<Key> {
    match file.get(hash)?.kind {
        Kind::Scalar(Value::Key(key)) => Some(key),
        _ => None,
    }
}

fn color_at(file: &PropertyFile, hash: u32) -> Option<[f32; 4]> {
    match file.get(hash)?.kind {
        Kind::Scalar(Value::ColorRgba { r, g, b, a }) => Some([r, g, b, a]),
        _ => None,
    }
}
