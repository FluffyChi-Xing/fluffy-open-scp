//! 探针：读取真实 building lot，按 `generic_lot` 像素着色器公式做多通道合成并输出图片。
//!
//! 目的：在不改动渲染器的前提下，用真实数据对拍"通道 → 纹理"的两种解读，供目视判定
//! `LotColor.A`（shader 里的 `colorNormalsIdx`）到底索引 **法线/图案图集** 还是
//! **漫反射图集**。
//!
//! 用法：
//! ```text
//! cargo run -p sc-exporter --release --example lot_compose -- \
//!   <lot_package> <lot_instance_hex> --out=<dir> \
//!   [--group=<hex>] [--base-tile=N] [--border-width=F] [lookup_package...]
//! ```
//!
//! 依据的 shader 公式（`tmp/shaders/cpp_0x00000000_frac-worldToClip-atlas-facade.txt`）：
//! ```text
//! masks            = greaterThan(mask, 0.5 - borderWidth)      // 逐通道硬阈值
//! 优先级链          = w→z→y→x，保证 masks / borderMask 互不重叠
//! index            = masks[ch] ? borderMask[ch] ? borderNormals[ch] : colorNormals[ch]
//! lotColor.rgb     = dot(colorsR,masks) + dot(bordersR,borderMask)   // 三个分量各一次
//!                    + saturate(1-overlayMask) * 底图颜色
//! 底图             = lotTextureSampler 的 4×4 图集，每 lot 只取一格 baseTileUVMinMax.x
//! ```
use dbpf::{IndexEntry, Package};
use sc_properties::{Key, Kind, PropertyFile, Value};

const LOT_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;

const H_LOT_MASK: u32 = 0x0CCB_7FD5;
const H_LOT_TEXTURES: u32 = 0x0CCB_7FD4;
const H_LOT_SIZE: u32 = 0x0CCB_7FC8;
const H_LOT_COLORS: [u32; 4] = [0x0D02_D586, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];
const H_LOT_BORDER_COLORS: [u32; 4] = [0x0D7A_F042, 0x0D7A_F043, 0x0D7A_F044, 0x0D7A_F045];

/// 图集列数：shader 里 `*.25` 即 1/4 → 4×4 = 16 格。
const ATLAS_COLS: usize = 4;
const ATLAS_CELLS: usize = ATLAS_COLS * ATLAS_COLS;

/// 优先级链顺序（shader 的 w→z→y→x，w 最高）。
const PRIORITY: [usize; 4] = [3, 2, 1, 0];

struct Args {
    lot_package: String,
    lot_instance: u32,
    group: Option<u32>,
    out_dir: String,
    base_tile: usize,
    /// 图集格在地块上重复的次数（= 地块地面 mesh UV 的跨度 N）。
    tiles: f32,
    border_width: f32,
    lookups: Vec<String>,
}

fn main() {
    let args = parse_args();
    std::fs::create_dir_all(&args.out_dir)
        .unwrap_or_else(|error| panic!("create {}: {error}", args.out_dir));

    let mut paths = vec![args.lot_package.clone()];
    paths.extend(args.lookups.iter().cloned());
    let packages: Vec<Package> = paths
        .iter()
        .map(|path| Package::open(path).unwrap_or_else(|error| panic!("open {path}: {error}")))
        .collect();

    // ---- 1. lot property ----
    let (owner, entry) = find_entry(&packages, LOT_TYPE, args.lot_instance, args.group)
        .unwrap_or_else(|| panic!("lot property 0x{:08X} not found", args.lot_instance));
    println!(
        "lot property 0x{:08X}-0x{:08X}-0x{:08X}  pkg={}",
        entry.id.type_id, entry.id.group, entry.id.instance, paths[owner]
    );
    let data = packages[owner].read(&entry).unwrap();
    let file = PropertyFile::parse(&data).expect("parse lot property");

    let mask_key = key_at(&file, H_LOT_MASK);
    let textures_key = key_at(&file, H_LOT_TEXTURES);
    let colors: Vec<Option<[f32; 4]>> = H_LOT_COLORS.iter().map(|h| color_at(&file, *h)).collect();
    let border_colors: Vec<Option<[f32; 4]>> =
        H_LOT_BORDER_COLORS.iter().map(|h| color_at(&file, *h)).collect();

    println!("  LotMask        = {}", fmt_key(mask_key));
    println!("  Lot Textures   = {}", fmt_key(textures_key));
    println!("  LotSize        = {:?}", vector2_at(&file, H_LOT_SIZE));
    for (index, color) in colors.iter().enumerate() {
        println!("  LotColor{}       = {}", index + 1, fmt_color(*color));
    }
    for (index, color) in border_colors.iter().enumerate() {
        println!("  LotBorderColor{} = {}", index + 1, fmt_color(*color));
    }

    // ---- 2. LotMask ----
    let mask_key = mask_key.expect("lot has no LotMask");
    let (mask_owner, mask_entry) = find_entry(&packages, RASTER_TYPE, mask_key.instance, None)
        .unwrap_or_else(|| panic!("LotMask raster 0x{:08X} not found", mask_key.instance));
    let mask_bytes = packages[mask_owner].read(&mask_entry).unwrap();
    let raster = rw4::RasterImage::parse(&mask_bytes).expect("parse LotMask raster");
    let (mask_w, mask_h) = (raster.width as usize, raster.height as usize);
    let raw_mask = raster.decode_top_mip_rgba().expect("decode LotMask");
    println!(
        "\nLotMask raster {}x{} pixFmt={} via {}",
        mask_w, mask_h, raster.pixel_format, paths[mask_owner]
    );
    save_png(&args.out_dir, "mask_native.png", mask_w, mask_h, &raw_mask, 2);
    print_channel_stats("LotMask 原始 RGBA", &raw_mask);
    // 朝向对齐后的 mask（= 原始栅格旋转 180°），与游戏内方位一致。
    let mask = orient_mask(&raw_mask, mask_w, mask_h);
    save_png(&args.out_dir, "mask_oriented.png", mask_w, mask_h, &mask, 4);
    for (label, channel) in [("r", 0usize), ("g", 1), ("b", 2), ("a", 3)] {
        let gray = grayscale(&mask, channel);
        save_png(
            &args.out_dir,
            &format!("mask_channel_{label}.png"),
            mask_w,
            mask_h,
            &gray,
            2,
        );
    }

    // ---- 3. Lot Textures：解出全部 texture section（缺失时按 App 回退，不中断）----
    let mut atlases: Vec<Atlas> = Vec::new();
    match textures_key.map(|key| find_entry(&packages, RW4_TYPE, key.instance, None)) {
        Some(Some((tex_owner, tex_entry))) => {
            let tex_bytes = packages[tex_owner].read(&tex_entry).unwrap();
            let rw4_file = rw4::Rw4File::parse(&tex_bytes).expect("parse Lot Textures");
            println!("  RW4 sections (全部，用于定位法线图集):");
            for section in rw4_file.sections() {
                println!(
                    "    #{} type=0x{:08X} size={}",
                    section.number, section.type_code, section.size
                );
            }
            for section in rw4_file.sections_of_type(rw4::SectionType::TEXTURE) {
                match rw4_file.decode_texture(&tex_bytes, section.number) {
                    Ok(texture) => match texture.decode_top_mip_rgba() {
                        Ok(rgba) => {
                            let atlas = Atlas::new(u32::from(texture.width) as usize, rgba);
                            println!(
                                "  texture section {}: {}x{}  normal-likeness={:.3}",
                                section.number,
                                atlas.width,
                                atlas.height,
                                normal_likeness(&atlas.rgba)
                            );
                            atlases.push(atlas);
                        }
                        Err(error) => println!(
                            "  texture section {} pixel decode failed: {error}",
                            section.number
                        ),
                    },
                    Err(error) => {
                        println!("  texture section {} decode failed: {error}", section.number)
                    }
                }
            }
            println!(
                "Lot Textures: {} texture section(s) via {}",
                atlases.len(),
                paths[tex_owner]
            );
        }
        _ => println!("Lot Textures: 缺失（空壳 lot property；App 侧回退本地占位 tile）"),
    }
    for (index, atlas) in atlases.iter().enumerate() {
        save_png(
            &args.out_dir,
            &format!("atlas{index}_full.png"),
            atlas.width,
            atlas.height,
            &atlas.rgba,
            1,
        );
        for (cell_index, (cw, ch, pixels)) in atlas.cells.iter().enumerate() {
            save_png(
                &args.out_dir,
                &format!("atlas{index}_cell_{cell_index:02}.png"),
                *cw,
                *ch,
                pixels,
                3,
            );
        }
    }

    // 判定法线图集（平坦区接近 128,128,255）。
    let normal_idx = atlases
        .iter()
        .enumerate()
        .max_by(|a, b| {
            normal_likeness(&a.1.rgba)
                .total_cmp(&normal_likeness(&b.1.rgba))
        })
        .map(|(index, atlas)| {
            if normal_likeness(&atlas.rgba) > 0.35 {
                Some(index)
            } else {
                None
            }
        })
        .flatten();
    let diffuse_idx = match normal_idx {
        Some(slot) => (0..atlases.len()).find(|index| *index != slot),
        None => Some(0),
    };
    println!("  diffuse atlas = {diffuse_idx:?}   normal atlas = {normal_idx:?}");

    // ---- 4. 通道语义 ----
    // LotColor 缺失时按 App 的回退色（黑/红/绿/蓝），A=0 使材质索引落到图集第 0 格。
    const DEFAULT_PALETTE: [[f32; 4]; 4] = [
        [0.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];
    if colors.iter().all(Option::is_none) {
        println!("LotColor1-4 缺失 → 使用回退色 黑/红/绿/蓝（authored=false）");
    }
    let colors8: Vec<[u8; 4]> = (0..4)
        .map(|index| colors[index].map_or(to_rgba8(DEFAULT_PALETTE[index]), to_rgba8))
        .collect();
    let borders8: Vec<[u8; 4]> = (0..4)
        .map(|index| {
            border_colors[index].map_or(to_rgba8(DEFAULT_PALETTE[index]), to_rgba8)
        })
        .collect();
    // LotColor.A 是**整数索引本体**（实测 3/1/8/3），不是 0..1 的颜色分量，故不走 to_rgba8。
    let normal_index: Vec<usize> = colors
        .iter()
        .map(|color| {
            color.map_or(0, |color| {
                (color[3].max(0.0).round() as usize) % ATLAS_CELLS
            })
        })
        .collect();
    println!(
        "\nLotColor.A 原值 = {:?}",
        colors.iter().map(|c| c.map(|c| c[3])).collect::<Vec<_>>()
    );
    println!("colorNormalsIdx (= floor(LotColor.A)) = {normal_index:?}");
    println!("base tile index (--base-tile)  = {}", args.base_tile % ATLAS_CELLS);
    println!("borderWidth (--border-width)   = {}", args.border_width);

    // 原始 mask RGB（**不做任何调色板替换**）与纯覆盖率掩码，避免把探针的
    // 回退色误读成资产数据。
    {
        let mut rgb = mask.clone();
        for px in rgb.as_chunks_mut::<4>().0 {
            px[3] = 255;
        }
        save_png(&args.out_dir, "mask_rgb.png", mask_w, mask_h, &rgb, 4);
    }
    let masks_for_coverage = mask_one_hot(&mask, args.border_width);
    {
        let mut covered = vec![0u8; mask_w * mask_h * 4];
        for (index, (mask_px, _)) in masks_for_coverage.iter().enumerate() {
            let on = mask_px.iter().any(|value| *value > 0.0);
            let value = if on { 255u8 } else { 32 };
            let at = index * 4;
            covered[at] = value;
            covered[at + 1] = value;
            covered[at + 2] = value;
            covered[at + 3] = 255;
        }
        save_png(
            &args.out_dir,
            "v5_covered_mask.png",
            mask_w,
            mask_h,
            &covered,
            4,
        );
    }

    let masks = mask_one_hot(&mask, args.border_width);
    let coverage: Vec<f32> = masks.iter().map(|(m, _)| m.iter().sum::<f32>().min(1.0)).collect();

    let diffuse = diffuse_idx.and_then(|index| atlases.get(index));
    let normal = normal_idx.and_then(|index| atlases.get(index));
    let base_tile = args.base_tile % ATLAS_CELLS;

    // (a) 引擎反照率：通道颜色相加 + 未覆盖处保留底图（不含图案，因为图案走法线/高度）。
    //     两种调色板顺序对照：slot = 通道序号（正常）vs 反向（LC4..LC1）。
    let albedo = compose(
        mask_w,
        mask_h,
        diffuse,
        base_tile,
        |_, _, _| None,
        &colors8,
        &borders8,
        &masks,
        &coverage,
    );
    save_png(&args.out_dir, "compose_albedo.png", mask_w, mask_h, &albedo, 4);

    // (a2) 用 Lot Textures 图集做替换（= 当前渲染器语义 tile_{LotColor.A} × LotColor.RGB，
    //      但叠加已定案的朝向/配对/优先级链），用于与平色版本目视对照。
    println!("图集格重复次数 (--tiles) = {}", args.tiles);
    let tiled = compose_tiles(
        mask_w, mask_h, diffuse, base_tile, &normal_index, &colors8, &masks, &coverage,
        args.tiles,
    );
    save_png(&args.out_dir, "compose_tiles.png", mask_w, mask_h, &tiled, 4);

    let mut colors8_rev = colors8.clone();
    colors8_rev.reverse();
    let mut borders8_rev = borders8.clone();
    borders8_rev.reverse();
    let albedo_rev = compose(
        mask_w,
        mask_h,
        diffuse,
        base_tile,
        |_, _, _| None,
        &colors8_rev,
        &borders8_rev,
        &masks,
        &coverage,
    );
    save_png(
        &args.out_dir,
        "compose_albedo_reversed.png",
        mask_w,
        mask_h,
        &albedo_rev,
        4,
    );

    // (b) 假设 A 索引「法线/图案」图集：区域用该格图案（拉伸铺满）以便目视。
    let pattern_normal = compose(
        mask_w,
        mask_h,
        diffuse,
        base_tile,
        |u, v, ch| normal.and_then(|atlas| atlas.sample(normal_index[ch], u, v)),
        &colors8,
        &borders8,
        &masks,
        &coverage,
    );
    save_png(
        &args.out_dir,
        "compose_pattern_from_A.png",
        mask_w,
        mask_h,
        &pattern_normal,
        4,
    );

    // (c) 反假设：A 直接当「漫反射」图集格（当前渲染器的做法）。
    let pattern_diffuse = compose(
        mask_w,
        mask_h,
        diffuse,
        base_tile,
        |u, v, ch| diffuse.and_then(|atlas| atlas.sample(normal_index[ch], u, v)),
        &colors8,
        &borders8,
        &masks,
        &coverage,
    );
    save_png(
        &args.out_dir,
        "compose_A_as_diffuse.png",
        mask_w,
        mask_h,
        &pattern_diffuse,
        4,
    );

    // (d) v5 预览：复刻前端新逻辑——未覆盖区 alpha=0（此处用洋红标出以便观察），
    //     仅 mask 有内容的区域被替换。用于确认"只填充有颜色的区域"。
    let v5 = compose_v5_preview(mask_w, mask_h, &colors8, &masks);
    save_png(&args.out_dir, "compose_v5_preview.png", mask_w, mask_h, &v5, 4);

    println!("\n输出目录 {}：", args.out_dir);
    println!("  mask_*.png            四通道灰度 + 原图（看 mask 形态）");
    println!("  atlas*_full / _cell_* 每张图集的 16 格（看法线图集在这些 A 值上长什么样）");
    println!("  compose_albedo        引擎反照率（只有颜色 + 底图）");
    println!("  compose_pattern_from_A   A → 法线图集（假设 H1）");
    println!("  compose_A_as_diffuse     A → 漫反射图集（当前实现 H2）");
}

// ---- 图集 ----

struct Atlas {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
    cells: Vec<(usize, usize, Vec<u8>)>,
}

impl Atlas {
    fn new(width: usize, rgba: Vec<u8>) -> Self {
        let height = if width == 0 { 0 } else { rgba.len() / 4 / width };
        let cells = (0..ATLAS_CELLS)
            .map(|index| extract_cell(&rgba, width, height, index))
            .collect();
        Self {
            width,
            height,
            rgba,
            cells,
        }
    }

    /// 采样第 `index` 格并把该格**拉伸铺满** 0..1（对齐 shader 的 `lerp(min,max,uv)`）。
    fn sample(&self, index: usize, u: f32, v: f32) -> Option<[u8; 4]> {
        let (cw, ch, pixels) = self.cells.get(index % ATLAS_CELLS)?;
        if *cw == 0 || *ch == 0 {
            return None;
        }
        let x = ((u.clamp(0.0, 0.999) * *cw as f32) as usize).min(*cw - 1);
        let y = ((v.clamp(0.0, 0.999) * *ch as f32) as usize).min(*ch - 1);
        let offset = (y * *cw + x) * 4;
        Some([
            pixels[offset],
            pixels[offset + 1],
            pixels[offset + 2],
            pixels[offset + 3],
        ])
    }
}

fn extract_cell(rgba: &[u8], width: usize, height: usize, index: usize) -> (usize, usize, Vec<u8>) {
    let cw = width / ATLAS_COLS;
    let ch = height / ATLAS_COLS;
    if cw == 0 || ch == 0 {
        return (0, 0, Vec::new());
    }
    let ox = (index % ATLAS_COLS) * cw;
    let oy = (index / ATLAS_COLS) * ch;
    let mut out = vec![0u8; cw * ch * 4];
    for y in 0..ch {
        for x in 0..cw {
            let src = ((oy + y) * width + ox + x) * 4;
            let dst = (y * cw + x) * 4;
            out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    (cw, ch, out)
}

// ---- mask 语义 ----

/// `greaterThan(mask, 0.5 - borderWidth)`，并按 shader 的优先级链消除重叠：
/// 返回 (masks, borderMask)，两者各自至多一个通道为 1。
fn mask_one_hot(rgba: &[u8], border_width: f32) -> Vec<([f32; 4], [f32; 4])> {
    let mut out = Vec::with_capacity(rgba.len() / 4);
    for px in rgba.as_chunks::<4>().0 {
        let value = [
            px[0] as f32 / 255.0,
            px[1] as f32 / 255.0,
            px[2] as f32 / 255.0,
            px[3] as f32 / 255.0,
        ];
        let mut masks = [0.0f32; 4];
        for ch in 0..4 {
            if value[ch] > 0.5 - border_width {
                masks[ch] = 1.0;
            }
        }
        // 优先级链：高优先级通道先占用，其余让位（borderWidth=0 时等价 shader 结果）。
        let mut taken = false;
        for ch in PRIORITY {
            if masks[ch] > 0.0 {
                if taken {
                    masks[ch] = 0.0;
                } else {
                    taken = true;
                }
            }
        }
        // borderMask：落在 (0.5-borderWidth, 0.5+borderWidth] 带内的通道。
        let mut borders = [0.0f32; 4];
        if border_width > 0.0 {
            for ch in 0..4 {
                if masks[ch] > 0.0 && value[ch] <= 0.5 + border_width {
                    borders[ch] = 1.0;
                }
            }
        }
        out.push((masks, borders));
    }
    out
}

// ---- 合成 ----

#[allow(clippy::too_many_arguments)]
fn compose(
    width: usize,
    height: usize,
    base: Option<&Atlas>,
    base_tile: usize,
    pattern: impl Fn(f32, f32, usize) -> Option<[u8; 4]>,
    colors: &[[u8; 4]],
    borders: &[[u8; 4]],
    masks: &[([f32; 4], [f32; 4])],
    coverage: &[f32],
) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let (mask, border) = masks[index];
            // 图集/底图的 U 轴与 mask 栅格列序相反（2026-09-12 消防局对拍确认）：
            // mask 已由 orient_mask 处理列序，此处图集采样需再镜像 U，
            // 否则底图格内容会与游戏左右相反。
            let u = 1.0 - (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;

            // 通道颜色：dot(colorsR,masks) 逐分量展开；边框色同理叠加。
            let mut rgb = [0.0f32; 3];
            for ch in 0..3 {
                for slot in 0..4 {
                    rgb[ch] += colors[slot][ch] as f32 / 255.0 * mask[slot];
                    rgb[ch] += borders[slot][ch] as f32 / 255.0 * border[slot];
                }
            }

            // 图案仅用于目视对比（引擎里图案进法线/高度，不进反照率）。
            if let Some(ch) = PRIORITY.iter().copied().find(|ch| mask[*ch] > 0.0)
                && let Some(px) = pattern(u, v, ch)
            {
                let luma =
                    (px[0] as f32 + px[1] as f32 + px[2] as f32) / (3.0 * 255.0);
                for component in rgb.iter_mut() {
                    *component *= 0.35 + 0.65 * luma;
                }
            }

            // 未覆盖处保留底图：saturate(1-overlayMask) * 底图。
            let keep = (1.0 - coverage[index]).clamp(0.0, 1.0);
            let base_px = base
                .and_then(|atlas| atlas.sample(base_tile, u, v))
                .unwrap_or([255, 255, 255, 255]);

            let offset = index * 4;
            for (ch, component) in rgb.iter().enumerate() {
                let value = component + keep * (base_px[ch] as f32 / 255.0);
                out[offset + ch] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
            out[offset + 3] = 255;
        }
    }
    out
}

/// 用 Lot Textures 图集做表面替换：每个像素按优先级链取胜出通道，
/// 取 `图集[LotColor.A]` 该格（整格拉伸铺满地块）再乘该通道 LotColor.RGB；
/// 未覆盖区用底图格。等价于当前渲染器的材质选择，但朝向/配对/优先级已按实测修正。
#[allow(clippy::too_many_arguments)]
fn compose_tiles(
    width: usize,
    height: usize,
    diffuse: Option<&Atlas>,
    base_tile: usize,
    cell_of: &[usize],
    colors: &[[u8; 4]],
    masks: &[([f32; 4], [f32; 4])],
    coverage: &[f32],
    tiles: f32,
) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    let Some(atlas) = diffuse else {
        return out;
    };
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            let offset = index * 4;
            let (mask, _) = masks[index];
            // 图集 U 轴与 mask 列序相反（同 compose）。
            let u = 1.0 - (x as f32 + 0.5) / width as f32;
            let v = (y as f32 + 0.5) / height as f32;
            // 引擎：frac(baseUV)，即 UV 跨 0..N 时该格重复 N 次。
            let tu = (u * tiles).fract();
            let tv = (v * tiles).fract();
            let winner = PRIORITY.iter().copied().find(|ch| mask[*ch] > 0.0);
            let (tile, tint) = match winner {
                Some(ch) if coverage[index] > 0.0 => (
                    atlas.sample(cell_of[ch], tu, tv),
                    [colors[ch][0], colors[ch][1], colors[ch][2]],
                ),
                _ => {
                    let base = atlas.sample(base_tile, tu, tv);
                    (base, [255u8, 255, 255])
                }
            };
            let Some(tile) = tile else { continue };
            for ch in 0..3 {
                out[offset + ch] =
                    ((u32::from(tile[ch]) * u32::from(tint[ch])) / 255).min(255) as u8;
            }
            out[offset + 3] = 255;
        }
    }
    out
}

/// 复刻前端 v5 逻辑：只有 mask 有内容的像素才替换材质，其余不填。
/// 未覆盖区用洋红标出（PNG 无"透明"可看，用色块代替）。
fn compose_v5_preview(
    width: usize,
    height: usize,
    colors: &[[u8; 4]],
    masks: &[([f32; 4], [f32; 4])],
) -> Vec<u8> {
    let mut out = vec![0u8; width * height * 4];
    for (index, (mask, border)) in masks.iter().enumerate() {
        let offset = index * 4;
        let coverage = mask.iter().sum::<f32>().min(1.0);
        if coverage <= 0.0 {
            // 未替换区（前端为 alpha=0）→ 洋红标记
            out[offset] = 255;
            out[offset + 2] = 255;
            out[offset + 3] = 255;
            continue;
        }
        let mut rgb = [0.0f32; 3];
        for ch in 0..3 {
            for slot in 0..4 {
                rgb[ch] += colors[slot][ch] as f32 / 255.0 * mask[slot];
                rgb[ch] += colors[slot][ch] as f32 / 255.0 * border[slot];
            }
        }
        for ch in 0..3 {
            out[offset + ch] = (rgb[ch].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        out[offset + 3] = 255;
    }
    out
}

// ---- 属性取值 ----

fn scalar<'a>(file: &'a PropertyFile, hash: u32) -> Option<&'a Value> {
    match &file.get(hash)?.kind {
        Kind::Scalar(value) => Some(value),
        Kind::Array(values) => values.first(),
        Kind::Empty => None,
    }
}

fn key_at(file: &PropertyFile, hash: u32) -> Option<Key> {
    match scalar(file, hash)? {
        Value::Key(key) => Some(*key),
        _ => None,
    }
}

fn color_at(file: &PropertyFile, hash: u32) -> Option<[f32; 4]> {
    match scalar(file, hash)? {
        Value::ColorRgba { r, g, b, a } => Some([*r, *g, *b, *a]),
        Value::Vector4(values) => Some(*values),
        _ => None,
    }
}

fn vector2_at(file: &PropertyFile, hash: u32) -> Option<[f32; 2]> {
    match scalar(file, hash)? {
        Value::Vector2(values) => Some(*values),
        _ => None,
    }
}

// ---- 杂项 ----

fn parse_args() -> Args {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let positional: Vec<&String> = raw.iter().filter(|arg| !arg.starts_with("--")).collect();
    let flag = |name: &str| -> Option<String> {
        raw.iter()
            .find_map(|arg| arg.strip_prefix(&format!("--{name}=")).map(str::to_owned))
    };
    if positional.len() < 2 {
        eprintln!(
            "usage: lot_compose <lot_package> <lot_instance_hex> --out=<dir> \
             [--group=<hex>] [--base-tile=N] [--tiles=F] [--border-width=F] [lookup_package...]"
        );
        std::process::exit(2);
    }
    let parse_hex = |value: &str| {
        u32::from_str_radix(value.trim_start_matches("0x"), 16)
            .unwrap_or_else(|error| panic!("bad hex {value}: {error}"))
    };
    Args {
        lot_package: positional[0].clone(),
        lot_instance: parse_hex(positional[1]),
        group: flag("group").map(|value| parse_hex(&value)),
        out_dir: flag("out").unwrap_or_else(|| "tmp/lot_compose".to_owned()),
        tiles: flag("tiles")
            .map(|value| value.parse().unwrap_or_else(|e| panic!("bad --tiles: {e}")))
            .unwrap_or(1.0),
        base_tile: flag("base-tile")
            .map(|value| value.parse().unwrap_or_else(|error| panic!("bad --base-tile: {error}")))
            .unwrap_or(0),
        border_width: flag("border-width")
            .map(|value| {
                value
                    .parse()
                    .unwrap_or_else(|error| panic!("bad --border-width: {error}"))
            })
            .unwrap_or(0.0),
        lookups: positional[2..].iter().map(|value| (*value).clone()).collect(),
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

/// 逐通道统计，便于判断解码结果是否异常（例如全 255）。
fn print_channel_stats(label: &str, rgba: &[u8]) {
    let mut min = [255u8; 4];
    let mut max = [0u8; 4];
    let mut sum = [0u64; 4];
    let mut count = 0u64;
    for px in rgba.as_chunks::<4>().0 {
        for ch in 0..4 {
            min[ch] = min[ch].min(px[ch]);
            max[ch] = max[ch].max(px[ch]);
            sum[ch] += u64::from(px[ch]);
        }
        count += 1;
    }
    if count == 0 {
        println!("{label}: <empty>");
        return;
    }
    println!(
        "{label}: min={min:?} max={max:?} mean=[{:.1}, {:.1}, {:.1}, {:.1}] px={count}",
        sum[0] as f64 / count as f64,
        sum[1] as f64 / count as f64,
        sum[2] as f64 / count as f64,
        sum[3] as f64 / count as f64
    );
}

/// 法线图相似度：平坦法线区接近 (128,128,255)。
fn normal_likeness(rgba: &[u8]) -> f32 {
    let mut hits = 0usize;
    let mut total = 0usize;
    for px in rgba.as_chunks::<4>().0.iter().step_by(7) {
        total += 1;
        if px[2] > 200 && px[0].abs_diff(128) < 40 && px[1].abs_diff(128) < 40 {
            hits += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        hits as f32 / total as f32
    }
}

/// 朝向对齐（2026-09-12 与消防局游戏内截图逐细节比对确认）：
/// LotMask 栅格需 **行序翻转（V）+ 列序镜像（U）**——等价于把原始栅格旋转 180°。
/// 此前只做行翻转，导致合成图与游戏**左右镜像**；用户比对发现水平翻转后
/// 「完美契合」，故补上列镜像。
fn orient_mask(rgba: &[u8], width: usize, height: usize) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for y in 0..height {
        let src_row = (height - 1 - y) * width;
        let dst_row = y * width;
        for x in 0..width {
            let src = (src_row + (width - 1 - x)) * 4;
            let dst = (dst_row + x) * 4;
            out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    out
}

fn grayscale(rgba: &[u8], channel: usize) -> Vec<u8> {
    let mut out = vec![0u8; rgba.len()];
    for (index, px) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let value = px[channel];
        out[index * 4] = value;
        out[index * 4 + 1] = value;
        out[index * 4 + 2] = value;
        out[index * 4 + 3] = 255;
    }
    out
}

fn upscale(rgba: &[u8], width: usize, height: usize, factor: usize) -> (usize, usize, Vec<u8>) {
    if factor <= 1 {
        return (width, height, rgba.to_vec());
    }
    let (ow, oh) = (width * factor, height * factor);
    let mut out = vec![0u8; ow * oh * 4];
    for y in 0..oh {
        for x in 0..ow {
            let src = ((y / factor) * width + (x / factor)) * 4;
            let dst = (y * ow + x) * 4;
            out[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
        }
    }
    (ow, oh, out)
}

fn save_png(dir: &str, name: &str, width: usize, height: usize, rgba: &[u8], factor: usize) {
    if width == 0 || height == 0 || rgba.len() < width * height * 4 {
        return;
    }
    let (ow, oh, pixels) = upscale(rgba, width, height, factor);
    let Some(image) = image::RgbaImage::from_raw(ow as u32, oh as u32, pixels) else {
        return;
    };
    image
        .save(format!("{dir}/{name}"))
        .unwrap_or_else(|error| panic!("save {name}: {error}"));
}

fn to_rgba8(color: [f32; 4]) -> [u8; 4] {
    // LotColor 是**线性**颜色（引擎在 sRGB 输出端做伽马编码），故逐分量做 linear→sRGB。
    [
        linear_to_srgb_u8(color[0]),
        linear_to_srgb_u8(color[1]),
        linear_to_srgb_u8(color[2]),
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn linear_to_srgb_u8(linear: f32) -> u8 {
    let linear = if linear.is_finite() {
        linear.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let srgb = if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (srgb * 255.0).round().clamp(0.0, 255.0) as u8
}

fn fmt_key(key: Option<Key>) -> String {
    key.map_or_else(
        || "<missing>".to_owned(),
        |key| {
            format!(
                "T 0x{:08X} G 0x{:08X} I 0x{:08X}",
                key.type_id, key.group, key.instance
            )
        },
    )
}

fn fmt_color(color: Option<[f32; 4]>) -> String {
    color.map_or_else(
        || "<missing>".to_owned(),
        |color| {
            format!(
                "({:.4}, {:.4}, {:.4}) A={:.4}",
                color[0], color[1], color[2], color[3]
            )
        },
    )
}
