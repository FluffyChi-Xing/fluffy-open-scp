//! LotMask raster 批量引擎式合成 —— 严格按 `generic_lot` 像素着色器
//! （docs/overview/lot-rendering.md §3）。
//!
//! 旧版误用 terrain PS 的 a⁴ 权重公式把这批 overlay 贴图当"地形控制图"逐像素涂色
//! + 双线性放大 → 双重模糊。实际它们由 `genericLot4ChanOverlay` 消费，语义是
//! 「材质分区选择器」：
//!
//! ```text
//! masks[c]      = pixel[c] > 0.5 - borderWidth[c]           // 硬阈值 one-hot
//! borderMask[c] = masks[c] && pixel[c] <= 0.5 + borderWidth[c]
//! 8 级瀑布      = A边框 > A主色 > B边框 > B主色 > G边框 > G主色 > R边框 > R主色
//! albedo.rgb    = 胜出通道平色（LotColor / LotBorderColor，linear→sRGB）
//!                 + saturate(1 - overlayMask) × 底图格
//! 底图          = Lot Textures 4×4 图集第 baseTile 格，整格拉伸铺满地块，
//!                 U 轴与 mask 列序相反（§5b）
//! ```
//!
//! mask 栅格先旋转 180°（行翻转 + 列镜像，§5b）。批量合成在**原生分辨率**上做
//! （每 texel 一像素，阈值后无任何权重混合），PNG 仅最近邻放大便于肉眼检查。
//! 每 lot property 输出一张 —— 颜色 / 边框 / 底图格是 per-lot 数据，
//! 同一 raster 被不同 lot 引用时颜色各不相同。
//!
//! ---- 单 lot 高清模式（游戏级尺度 + 纹理替换）----
//!
//! ```text
//! cargo run -p sc-exporter --release --example lot_composite -- --lot=<hex> [--ppm=32]
//! ```
//!
//! 按 LotSize（米）定长宽比、--ppm 像素密度放大输出；mask 以 GPU 口径双线性
//! 采样（阈值在插值之后，边缘随超采样自然平滑）。每像素 alpha = overlayMask
//! （引擎 `saturate(a+overlayMask)`，= 前端 v5"只替换有内容区"语义）。
//! 覆盖区两种纹理化对照：
//!   *_hires_pattern.png  引擎口径：平色 × 图案/法线图集 0x60E7805D 格（LotColor.A）
//!                        的坡度明暗（近似 lotCalcLighting 的图案光照）
//!   *_hires_tile.png     渲染器口径：漫反射图集格（LotColor.A 平行格）× LotColor 染色
//! 图案平铺密度 = LotSize / 0x0CCB7FD0（米/格，默认 8.0，uv1 世界锚定细节层）。
use dbpf::{IndexEntry, Package};
use image::RgbaImage;
use sc_properties::{Kind, Key, ParseLimits, PropertyFile, Value};
use std::collections::HashMap;

const RASTER_TYPE: u32 = 0x2F4E_681C;
const RW4_TYPE: u32 = 0x2F4E_681B;
const PROPERTY_TYPE: u32 = 0x00B1_B104;

const H_LOT_MASK: u32 = 0x0CCB_7FD5;
const H_LOT_TEXTURES: u32 = 0x0CCB_7FD4;
const H_LOT_SIZE: u32 = 0x0CCB_7FC8;
const H_LOT_COLORS: [u32; 4] = [0x0D02_D586, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];
const H_LOT_BORDER_COLORS: [u32; 4] = [0x0D7A_F042, 0x0D7A_F043, 0x0D7A_F044, 0x0D7A_F045];
const H_BORDER_WIDTHS: [u32; 4] = [0x0D7A_F046, 0x0D7A_F047, 0x0D7A_F048, 0x0D7A_F049];
const H_BASE_TILE: u32 = 0x0CCB_7FD6; // Int32 0..15（baseTileUVMinMax.x）
const H_BASE_TILE_MIN_A: u32 = 0x0CCB_7FD2; // 推导源 1
const H_BASE_TILE_MIN_B: u32 = 0x0CCB_7FD3; // 推导源 2
const H_DETAIL_METERS: u32 = 0x0CCB_7FD0; // 世界锚定细节层（米/格，默认 8.0）

/// 全局共享图案/法线图集（§6 [源码] 已解）：RW4 1024²×16，容器 0x2F4E681B。
const H_NORMAL_ATLAS: u32 = 0x60E7_805D;

const OUT_DIR: &str = r"D:\rust\packages\fluffy-open-scp\tmp\dynamic\lot_composite";
const DEFAULT_LIMIT: usize = 120;
const DEFAULT_PPM: f32 = 32.0;

const ATLAS_COLS: usize = 4;
const ATLAS_CELLS: usize = ATLAS_COLS * ATLAS_COLS;

/// App 侧回退调色板（LotColor1-4 缺失时；lot_compose.rs 同款，便于跨探针对照）。
const DEFAULT_PALETTE: [[f32; 4]; 4] = [
    [0.0, 0.0, 0.0, 0.0],
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
];

/// 胜出通道 → 诊断灰阶（0 未覆盖 / 64 R / 128 G / 192 B / 255 A）。
const SELECTOR_GRAY: [u8; 4] = [64, 128, 192, 255];

/// 8 级瀑布顺序（w→z→y→x，边框带先于同通道主色）。
const WATERFALL: [(usize, bool); 8] = [
    (3, true),
    (3, false),
    (2, true),
    (2, false),
    (1, true),
    (1, false),
    (0, true),
    (0, false),
];

#[derive(Clone)]
struct Mask {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
}

#[derive(Clone)]
struct Atlas {
    cells: Vec<(usize, usize, Vec<u8>)>,
}

impl Atlas {
    fn new(width: usize, rgba: Vec<u8>) -> Self {
        let height = if width == 0 { 0 } else { rgba.len() / 4 / width };
        let cells = (0..ATLAS_CELLS)
            .map(|index| extract_cell(&rgba, width, height, index))
            .collect();
        Self { cells }
    }

    /// 采样第 `index` 格，整格拉伸铺满 0..1（= shader 的 `lerp(min,max,uv)`）。
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

/// 一个 lot 的全部 per-lot 渲染输入（property 展平继承后提取）。
struct LotData {
    colors8: [[u8; 4]; 4],
    borders8: [[u8; 4]; 4],
    border_widths: [f32; 4],
    /// colorNormalsIdx = floor(LotColor.A)（图案/法线图集格号）。
    normal_index: [usize; 4],
    /// borderNormalsIdx = floor(LotBorderColor.A)。
    border_normal_index: [usize; 4],
    colors_authored: bool,
    base_tile: usize,
    base_src: &'static str,
    textures_key: Option<Key>,
    /// LotSize（米）；缺失时回退 64×64 并标记。
    lot_size: (f32, f32),
    lot_size_authored: bool,
    /// 0x0CCB7FD0 世界锚定细节层（米/格，默认 8.0）。
    detail_meters: (f32, f32),
}

fn extract_lot_data(file: &PropertyFile) -> LotData {
    let colors_lin: Vec<Option<[f32; 4]>> =
        H_LOT_COLORS.iter().map(|h| color_at(file, *h)).collect();
    let border_lin: Vec<Option<[f32; 4]>> = H_LOT_BORDER_COLORS
        .iter()
        .map(|h| color_at(file, *h))
        .collect();
    let colors8: [[u8; 4]; 4] = std::array::from_fn(|i| {
        colors_lin[i].map_or(to_rgba8(DEFAULT_PALETTE[i]), to_rgba8)
    });
    let borders8: [[u8; 4]; 4] = std::array::from_fn(|i| {
        border_lin[i].map_or(to_rgba8(DEFAULT_PALETTE[i]), to_rgba8)
    });
    let mut border_widths = [0.0f32; 4];
    for (c, h) in H_BORDER_WIDTHS.iter().enumerate() {
        if let Some(Value::Float(v)) = scalar(file, *h) {
            border_widths[c] = *v;
        }
    }
    // LotColor.A 是整数索引本体（实测 3/1/8/3），不是 0..1 颜色分量。
    let index_of = |c: &Option<[f32; 4]>| {
        c.map_or(0, |c| (c[3].max(0.0).round() as usize) % ATLAS_CELLS)
    };
    let (base_tile, base_src) = base_tile_of(file);
    let lot_size = vector2_at(file, H_LOT_SIZE);
    let detail = vector2_at(file, H_DETAIL_METERS);
    LotData {
        colors8,
        borders8,
        border_widths,
        normal_index: std::array::from_fn(|i| index_of(&colors_lin[i])),
        border_normal_index: std::array::from_fn(|i| index_of(&border_lin[i])),
        colors_authored: colors_lin.iter().any(Option::is_some),
        base_tile,
        base_src,
        textures_key: key_at(file, H_LOT_TEXTURES),
        lot_size: lot_size.map_or((64.0, 64.0), |s| (s[0], s[1])),
        lot_size_authored: lot_size.is_some(),
        detail_meters: detail.map_or((8.0, 8.0), |s| (s[0], s[1])),
    }
}

fn main() -> dbpf::Result<()> {
    let mut limit = DEFAULT_LIMIT;
    let mut lot_arg: Option<u32> = None;
    let mut ppm = DEFAULT_PPM;
    let mut out_dir = OUT_DIR.to_owned();
    for arg in std::env::args().skip(1) {
        if let Some(value) = arg.strip_prefix("--limit=") {
            limit = value.parse().unwrap_or(DEFAULT_LIMIT);
        } else if let Some(value) = arg.strip_prefix("--lot=") {
            lot_arg = u32::from_str_radix(value.trim_start_matches("0x"), 16).ok();
        } else if let Some(value) = arg.strip_prefix("--ppm=") {
            ppm = value.parse().unwrap_or(DEFAULT_PPM);
        } else if let Some(value) = arg.strip_prefix("--out=") {
            out_dir = value.to_owned();
        }
    }
    let ep1 = r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCityDataEP1.package";
    let graphics = r"D:\ea-games\simcity_offline\SimCity：Cites of Tomorrow\SimCityData\SimCity_Graphics.package";
    // Game 包（另一安装）：市政厅族 lot（0x457EA9DB 等只在这里）。
    let game = r"D:\ea-games\SimCity\SimCityData\SimCity_Game.package";
    std::fs::create_dir_all(&out_dir)?;

    let game_pkg = Package::open(game).ok();
    let ep1_pkg = Package::open(ep1)?;
    let gfx = Package::open(graphics)?;
    // 索引顺序 = 优先级从低到高（后插覆盖）：game → ep1 → graphics。
    let mut packages: Vec<&Package> = game_pkg.iter().collect();
    packages.push(&ep1_pkg);
    packages.push(&gfx);

    // ---- 索引（Graphics 后插 → raster/RW4 同 ID 时优先 Graphics，与旧探针一致）----
    let mut raster_index: HashMap<u32, (usize, IndexEntry)> = HashMap::new();
    let mut rw4_index: HashMap<u32, (usize, IndexEntry)> = HashMap::new();
    let mut prop_index: HashMap<u32, (usize, IndexEntry)> = HashMap::new();
    for (pkg_idx, pkg) in packages.iter().enumerate() {
        for e in pkg.entries() {
            let table = match e.id.type_id {
                RASTER_TYPE => &mut raster_index,
                RW4_TYPE => &mut rw4_index,
                PROPERTY_TYPE => &mut prop_index,
                _ => continue,
            };
            table.insert(e.id.instance, (pkg_idx, e.clone()));
        }
    }
    println!(
        "索引：raster {} · RW4 {} · property {}",
        raster_index.len(),
        rw4_index.len(),
        prop_index.len()
    );

    let mut mask_cache: HashMap<u32, Option<Mask>> = HashMap::new();
    let mut atlas_cache: HashMap<u32, Option<Vec<Atlas>>> = HashMap::new();

    if let Some(lot_id) = lot_arg {
        return run_hires(lot_id, ppm, &out_dir, &packages, &prop_index, &raster_index, &rw4_index, &mut mask_cache, &mut atlas_cache);
    }

    // ---- 逐 lot property 合成（批量、原生分辨率）----
    let mut sheets: Vec<(u32, u32, RgbaImage)> = Vec::new();
    let mut inherit_count = 0usize;
    let mut count = 0usize;
    for entry in ep1_pkg.entries() {
        if entry.id.type_id != PROPERTY_TYPE {
            continue;
        }
        if count >= limit {
            break;
        }
        let entry = entry.clone();
        let Ok(data) = ep1_pkg.read(&entry) else { continue };
        let Ok(pf) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
            continue;
        };
        // Parent(0x00B2CCCB) 继承展平（引擎口径：lot 变体常在父级挂地表授权）。
        let (file, chain) = flatten(pf, &packages, &prop_index);
        if !chain.is_empty() {
            inherit_count += 1;
        }
        let Some(mask_key) = key_at(&file, H_LOT_MASK) else { continue };
        let Some(mask) = load_mask(&packages, &raster_index, &mut mask_cache, mask_key.instance)
        else {
            continue;
        };
        let data = extract_lot_data(&file);
        let diffuse = data
            .textures_key
            .and_then(|k| pick_atlas(&packages, &rw4_index, &mut atlas_cache, k.instance, false));

        // ---- 原生分辨率引擎合成（硬阈值 + 8 级瀑布 + 平色 + 底图格）----
        let oriented = orient_mask(&mask.rgba, mask.w, mask.h);
        let (w, h) = (mask.w, mask.h);
        let mut out = vec![0u8; w * h * 4];
        let mut selector = vec![0u8; w * h * 4];
        let mut chan_share = [0usize; 5]; // R/G/B/A 胜出（含边框带）+ 未覆盖
        for i in 0..w * h {
            let p = i * 4;
            let value = [
                f32::from(oriented[p]) / 255.0,
                f32::from(oriented[p + 1]) / 255.0,
                f32::from(oriented[p + 2]) / 255.0,
                f32::from(oriented[p + 3]) / 255.0,
            ];
            let (masks, borders) = overlay_waterfall(value, data.border_widths);
            let winner = WATERFALL
                .into_iter()
                .find(|(c, is_border)| {
                    (if *is_border { borders[*c] } else { masks[*c] }) > 0.0
                });

            let x = i % w;
            let y = i / w;
            // 图集 U 轴与 mask 列序相反（§5b）；底图格整格拉伸铺满地块。
            let u = 1.0 - (x as f32 + 0.5) / w as f32;
            let v = (y as f32 + 0.5) / h as f32;
            let rgb = match winner {
                Some((c, true)) => {
                    chan_share[c] += 1;
                    data.borders8[c]
                }
                Some((c, false)) => {
                    chan_share[c] += 1;
                    data.colors8[c]
                }
                None => {
                    chan_share[4] += 1;
                    diffuse
                        .as_ref()
                        .and_then(|a| a.sample(data.base_tile, u, v))
                        .map(|b| [b[0], b[1], b[2], 255])
                        .unwrap_or([255, 255, 255, 255])
                }
            };
            out[p..p + 4].copy_from_slice(&rgb);
            let gray = match winner {
                Some((c, _)) => SELECTOR_GRAY[c],
                None => 0,
            };
            selector[p..p + 4].copy_from_slice(&[gray, gray, gray, 255]);
        }

        let lot_id = entry.id.instance;
        let total = (w * h) as f64;
        let pct = |n: usize| n as f64 / total * 100.0;
        println!(
            "#{:03} lot={:08x} mask={:08x} {}x{} base={}({}) atlas={} bw={:.3?} \
             A索引={:?} R={:.1}% G={:.1}% B={:.1}% A={:.1}% idle={:.1}%",
            count,
            lot_id,
            mask_key.instance,
            w,
            h,
            data.base_tile,
            data.base_src,
            data
                .textures_key
                .map(|k| format!("{:08x}/{}", k.instance, diffuse.is_some()))
                .unwrap_or_else(|| "none".to_owned()),
            data.border_widths,
            data.normal_index,
            pct(chan_share[0]),
            pct(chan_share[1]),
            pct(chan_share[2]),
            pct(chan_share[3]),
            pct(chan_share[4]),
        );

        save_png(
            OUT_DIR,
            &format!("lot_{lot_id:08x}_albedo.png"),
            w,
            h,
            &out,
            2,
        );
        save_png(
            OUT_DIR,
            &format!("lot_{lot_id:08x}_selector.png"),
            w,
            h,
            &selector,
            2,
        );

        let scaled = image::imageops::resize(
            &RgbaImage::from_raw(w as u32, h as u32, out.clone()).expect("native buf"),
            132,
            132,
            image::imageops::FilterType::Nearest,
        );
        sheets.push((lot_id, mask_key.instance, scaled));
        count += 1;
    }

    println!(
        "\n合成 {count} 个 lot（Parent 继承链命中 {inherit_count}）→ {OUT_DIR}"
    );

    // ---- contact sheet（每格 132px，12 列）----
    if !sheets.is_empty() {
        let cell = 132usize;
        let cols = 12usize;
        let rows = (sheets.len() + cols - 1) / cols;
        let mut sheet = RgbaImage::new((cols * (cell + 4)) as u32, (rows * (cell + 4)) as u32);
        for (i, (lot_id, mask_id, img)) in sheets.iter().enumerate() {
            let ox = (i % cols) * (cell + 4) + 2;
            let oy = (i / cols) * (cell + 4) + 2;
            for y in 0..cell.min(img.height() as usize) {
                for x in 0..cell.min(img.width() as usize) {
                    let px = img.get_pixel(x as u32, y as u32);
                    sheet.put_pixel((ox + x) as u32, (oy + y) as u32, *px);
                }
            }
            print!("{:03}:lot={:08x}/mask={:08x}  ", i, lot_id, mask_id);
            if (i + 1) % 4 == 0 {
                println!();
            }
        }
        println!();
        sheet
            .save(format!("{OUT_DIR}/_contact_sheet.png"))
            .map_err(|e| dbpf::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        println!("contact sheet -> {OUT_DIR}/_contact_sheet.png");
    }
    Ok(())
}

// ---- 单 lot 高清模式 ----

#[allow(clippy::too_many_arguments)]
fn run_hires(
    lot_id: u32,
    ppm: f32,
    out_dir: &str,
    packages: &[&Package],
    prop_index: &HashMap<u32, (usize, IndexEntry)>,
    raster_index: &HashMap<u32, (usize, IndexEntry)>,
    rw4_index: &HashMap<u32, (usize, IndexEntry)>,
    mask_cache: &mut HashMap<u32, Option<Mask>>,
    atlas_cache: &mut HashMap<u32, Option<Vec<Atlas>>>,
) -> dbpf::Result<()> {
    let Some((pkg_idx, entry)) = prop_index.get(&lot_id) else {
        println!("lot property {lot_id:08x} 未找到");
        return Ok(());
    };
    let data = packages[*pkg_idx].read(entry)?;
    let Ok(pf) = PropertyFile::parse_with_limits(&data, ParseLimits::default()) else {
        println!("property 解析失败");
        return Ok(());
    };
    let (file, chain) = flatten(pf, packages, prop_index);
    println!(
        "lot {:08x}（Parent 继承 {} 级）",
        lot_id,
        chain.len()
    );
    let Some(mask_key) = key_at(&file, H_LOT_MASK) else {
        println!("该 property 无 LotMask");
        return Ok(());
    };
    let Some(mask) = load_mask(packages, raster_index, mask_cache, mask_key.instance) else {
        println!("LotMask raster {:08x} 解码失败", mask_key.instance);
        return Ok(());
    };
    let lot = extract_lot_data(&file);
    let textures = lot
        .textures_key
        .and_then(|k| load_atlas_textures(packages, rw4_index, atlas_cache, k.instance));
    let diffuse = textures.as_ref().and_then(|t| pick(t, false));
    let normal_textures = load_atlas_textures(packages, rw4_index, atlas_cache, H_NORMAL_ATLAS);
    let normal_atlas = normal_textures.as_ref().and_then(|t| pick(t, true));

    // 输出尺度：LotSize（米）× ppm；超 4096 上限时**等比**缩放（保长宽比）。
    const MAX_SIDE: usize = 4096;
    let raw_w = (lot.lot_size.0 * ppm).round().max(1.0) as usize;
    let raw_h = (lot.lot_size.1 * ppm).round().max(1.0) as usize;
    let shrink = (MAX_SIDE as f32 / raw_w as f32).min(MAX_SIDE as f32 / raw_h as f32).min(1.0);
    let out_w = ((raw_w as f32 * shrink) as usize).clamp(64, MAX_SIDE);
    let out_h = ((raw_h as f32 * shrink) as usize).clamp(64, MAX_SIDE);
    // 图案平铺密度（米/格 → 地块内重复次数；uv1 世界锚定，相位对单 lot 图无意义）。
    let reps = (
        lot.lot_size.0 / lot.detail_meters.0.max(0.1),
        lot.lot_size.1 / lot.detail_meters.1.max(0.1),
    );
    println!(
        "mask={:08x} {}x{} → 输出 {}x{}（LotSize {:.1}x{:.1} m @ {ppm} px/m{}）\
         base={}({}) diffuse={} normal_atlas={:08x}?{} A索引={:?} 边框A={:?} \
         平铺={:.2}x{:.2}（{:.1}/{:.1} 米每格）",
        mask_key.instance,
        mask.w,
        mask.h,
        out_w,
        out_h,
        lot.lot_size.0,
        lot.lot_size.1,
        if lot.lot_size_authored {
            ""
        } else {
            "，LotSize 缺失回退"
        },
        lot.base_tile,
        lot.base_src,
        lot.textures_key.map_or("none".to_owned(), |k| format!("{:08x}", k.instance)),
        H_NORMAL_ATLAS,
        normal_atlas.is_some(),
        lot.normal_index,
        lot.border_normal_index,
        reps.0,
        reps.1,
        lot.detail_meters.0,
        lot.detail_meters.1,
    );

    let oriented = orient_mask(&mask.rgba, mask.w, mask.h);
    for (name, mode) in [
        ("pattern", ShadeMode::Pattern),
        ("tile", ShadeMode::Tile),
    ] {
        let image = compose_hires(
            &oriented, &mask, out_w, out_h, &lot, diffuse, normal_atlas, reps, mode,
        );
        let path = format!("{out_dir}/lot_{lot_id:08x}_hires_{name}.png");
        RgbaImage::from_raw(out_w as u32, out_h as u32, image)
            .expect("hires buf")
            .save(&path)
            .map_err(|e| dbpf::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        println!("  -> {path}");
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ShadeMode {
    /// 引擎口径：平色 × 图案/法线图集坡度明暗。
    Pattern,
    /// 渲染器口径：漫反射平行格 × LotColor 染色。
    Tile,
}

/// 游戏级尺度合成：GPU 口径双线性 mask → 瀑布 → 纹理化 + alpha=overlayMask。
#[allow(clippy::too_many_arguments)]
fn compose_hires(
    oriented: &[u8],
    mask: &Mask,
    out_w: usize,
    out_h: usize,
    lot: &LotData,
    diffuse: Option<&Atlas>,
    normal_atlas: Option<&Atlas>,
    reps: (f32, f32),
    mode: ShadeMode,
) -> Vec<u8> {
    let mut out = vec![0u8; out_w * out_h * 4];
    // 世界锚定细节层的相位对单 lot 图无意义，+1024 保证 fract 输入为正。
    for py in 0..out_h {
        let v = (py as f32 + 0.5) / out_h as f32;
        let pv = ((v - 0.5) * reps.1 + 1024.0).fract();
        let bv = v;
        for px in 0..out_w {
            let u = (px as f32 + 0.5) / out_w as f32;
            let pu = ((0.5 - u) * reps.0 + 1024.0).fract(); // 细节层 U 镜像（§5b）
            let bu = 1.0 - u;
            let value = sample_mask_bilinear(oriented, mask.w, mask.h, u, v);
            let (masks, borders) = overlay_waterfall(value, lot.border_widths);
            let winner = WATERFALL
                .into_iter()
                .find(|(c, is_border)| {
                    (if *is_border { borders[*c] } else { masks[*c] }) > 0.0
                });
            let coverage = masks.iter().sum::<f32>().min(1.0);

            // 线性空间合成再编码（颜色与纹理都在 sRGB 编码端输出）。
            let (mut lin, alpha) = match winner {
                Some((c, true)) => match mode {
                    ShadeMode::Pattern => (
                        tinted(
                            lot.borders8[c],
                            shade_of(normal_atlas, lot.border_normal_index[c], pu, pv),
                        ),
                        1.0,
                    ),
                    ShadeMode::Tile => tiled_linear(diffuse, lot.border_normal_index[c], pu, pv, lot.borders8[c]),
                },
                Some((c, false)) => match mode {
                    ShadeMode::Pattern => (
                        tinted(
                            lot.colors8[c],
                            shade_of(normal_atlas, lot.normal_index[c], pu, pv),
                        ),
                        1.0,
                    ),
                    ShadeMode::Tile => tiled_linear(diffuse, lot.normal_index[c], pu, pv, lot.colors8[c]),
                },
                None => (
                    diffuse
                        .as_ref()
                        .and_then(|a| a.sample(lot.base_tile, bu, bv))
                        .map_or([1.0; 3], |b| srgb3_to_linear3([b[0], b[1], b[2]])),
                    coverage,
                ),
            };
            let offset = (py * out_w + px) * 4;
            for ch in 0..3 {
                out[offset + ch] = linear_to_srgb_u8(lin[ch]);
            }
            out[offset + 3] = (alpha * 255.0).round() as u8;
        }
    }
    out
}

/// 图案格法线 → 坡度明暗（近似 lotCalcLighting：平整=1.0，逆光面暗、向光面亮）。
fn shade_of(atlas: Option<&Atlas>, cell: usize, pu: f32, pv: f32) -> f32 {
    const LIGHT: (f32, f32) = (-0.5, -0.5); // 世界光方向的水平分量
    const STRENGTH: f32 = 1.4;
    let Some(px) = atlas.and_then(|a| a.sample(cell, pu, pv)) else {
        return 1.0;
    };
    let nx = f32::from(px[0]) / 127.5 - 1.0;
    let ny = f32::from(px[1]) / 127.5 - 1.0;
    (1.0 + STRENGTH * (nx * LIGHT.0 + ny * LIGHT.1)).clamp(0.55, 1.45)
}

/// 平色（sRGB u8）× 明暗因子，线性空间相乘后回 sRGB。
fn tinted(color: [u8; 4], shade: f32) -> [f32; 3] {
    let lin = srgb3_to_linear3([color[0], color[1], color[2]]);
    [lin[0] * shade, lin[1] * shade, lin[2] * shade]
}

/// 渲染器口径：漫反射格 texel × 染色（线性空间），图集缺失回退平色。
fn tiled_linear(
    diffuse: Option<&Atlas>,
    cell: usize,
    pu: f32,
    pv: f32,
    tint: [u8; 4],
) -> ([f32; 3], f32) {
    let tint_lin = srgb3_to_linear3([tint[0], tint[1], tint[2]]);
    match diffuse.and_then(|a| a.sample(cell, pu, pv)) {
        Some(tex) => {
            let tex_lin = srgb3_to_linear3([tex[0], tex[1], tex[2]]);
            (
                [
                    tex_lin[0] * tint_lin[0],
                    tex_lin[1] * tint_lin[1],
                    tex_lin[2] * tint_lin[2],
                ],
                1.0,
            )
        }
        None => (tint_lin, 1.0),
    }
}

// ---- mask 采样 / 瀑布 ----

/// mask 双线性采样（半 texel 中心 + clamp-to-edge，= 引擎/前端 GPU 口径）。
fn sample_mask_bilinear(rgba: &[u8], w: usize, h: usize, u: f32, v: f32) -> [f32; 4] {
    let fx = (u * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let fy = (v * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let x0 = fx.floor() as usize;
    let y0 = fy.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let at = |x: usize, y: usize, c: usize| f32::from(rgba[((y * w + x) * 4 + c)]) / 255.0;
    let mut out = [0.0f32; 4];
    for c in 0..4 {
        let top = at(x0, y0, c) * (1.0 - tx) + at(x1, y0, c) * tx;
        let bottom = at(x0, y1, c) * (1.0 - tx) + at(x1, y1, c) * tx;
        out[c] = top * (1.0 - ty) + bottom * ty;
    }
    out
}

/// addOverlay 精确语义：8 级瀑布 A边框 > A主色 > … > R主色，互不重叠。
/// 返回 (masks, borderMask)。
fn overlay_waterfall(value: [f32; 4], border_widths: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let mut masks = [0.0f32; 4];
    let mut borders = [0.0f32; 4];
    for c in 0..4 {
        if value[c] > 0.5 - border_widths[c] {
            masks[c] = 1.0;
        }
        if masks[c] > 0.0 && value[c] <= 0.5 + border_widths[c] {
            borders[c] = 1.0;
        }
    }
    let mut last_max = 1.0f32;
    for (c, is_border) in WATERFALL {
        let mut level = if is_border { borders[c] } else { masks[c] };
        level = level.min(last_max);
        last_max = (last_max - level).clamp(0.0, 1.0);
        if is_border {
            borders[c] = level;
        } else {
            masks[c] = level;
        }
    }
    (masks, borders)
}

// ---- 资源加载（带缓存） ----

fn flatten(
    pf: PropertyFile,
    packages: &[&Package],
    prop_index: &HashMap<u32, (usize, IndexEntry)>,
) -> (PropertyFile, Vec<Key>) {
    sc_properties::flatten_parent_inheritance_traced(pf, |key| {
        let (pkg_idx, parent) = prop_index.get(&key.instance)?;
        let data = packages[*pkg_idx].read(parent).ok()?;
        PropertyFile::parse_with_limits(&data, ParseLimits::default()).ok()
    })
}

fn load_mask(
    packages: &[&Package],
    index: &HashMap<u32, (usize, IndexEntry)>,
    cache: &mut HashMap<u32, Option<Mask>>,
    instance: u32,
) -> Option<Mask> {
    if let Some(hit) = cache.get(&instance) {
        return hit.clone();
    }
    let loaded = index.get(&instance).and_then(|(pkg_idx, entry)| {
        let bytes = packages[*pkg_idx].read(entry).ok()?;
        let raster = rw4::RasterImage::parse(&bytes).ok()?;
        let w = raster.width as usize;
        let h = raster.height as usize;
        let rgba = raster.decode_top_mip_rgba().ok()?;
        (w > 0 && h > 0 && rgba.len() >= w * h * 4).then_some(Mask { w, h, rgba })
    });
    cache.insert(instance, loaded.clone());
    loaded
}

/// 解容器内全部 texture section（Lot Textures 通常单张；法线图集按需挑选）。
fn load_atlas_textures(
    packages: &[&Package],
    index: &HashMap<u32, (usize, IndexEntry)>,
    cache: &mut HashMap<u32, Option<Vec<Atlas>>>,
    instance: u32,
) -> Option<Vec<Atlas>> {
    if let Some(hit) = cache.get(&instance) {
        return hit.clone();
    }
    let loaded = index.get(&instance).and_then(|(pkg_idx, entry)| {
        let bytes = packages[*pkg_idx].read(entry).ok()?;
        let rw4_file = rw4::Rw4File::parse(&bytes).ok()?;
        let mut atlases = Vec::new();
        for section in rw4_file.sections_of_type(rw4::SectionType::TEXTURE) {
            let Ok(texture) = rw4_file.decode_texture(&bytes, section.number) else {
                continue;
            };
            let Ok(rgba) = texture.decode_top_mip_rgba() else {
                continue;
            };
            let width = u32::from(texture.width) as usize;
            if width == 0 {
                continue;
            }
            atlases.push(Atlas::new(width, rgba));
        }
        (!atlases.is_empty()).then_some(atlases)
    });
    cache.insert(instance, loaded.clone());
    loaded
}

/// 法线图相似度：平坦法线区接近 (128,128,255)；漫反射图集应远离该分布。
fn normal_likeness_of(atlas: &Atlas) -> f32 {
    let mut hits = 0usize;
    let mut total = 0usize;
    for (cw, ch, pixels) in &atlas.cells {
        if *cw == 0 || *ch == 0 {
            continue;
        }
        for px in pixels.as_chunks::<4>().0.iter().step_by(7) {
            total += 1;
            if px[2] > 200 && px[0].abs_diff(128) < 40 && px[1].abs_diff(128) < 40 {
                hits += 1;
            }
        }
    }
    if total == 0 { 0.0 } else { hits as f32 / total as f32 }
}

/// 从容器多张纹理里挑一张：`want_normal=false` 取最不像法线图（漫反射），
/// `true` 取最像法线图（图案/法线图集）。
fn pick(atlases: &[Atlas], want_normal: bool) -> Option<&Atlas> {
    atlases
        .iter()
        .min_by(|a, b| {
            let (ka, kb) = (normal_likeness_of(a), normal_likeness_of(b));
            if want_normal {
                ka.total_cmp(&kb)
            } else {
                kb.total_cmp(&ka)
            }
        })
}

/// 兼容批量路径的便捷封装。
fn pick_atlas(
    packages: &[&Package],
    index: &HashMap<u32, (usize, IndexEntry)>,
    cache: &mut HashMap<u32, Option<Vec<Atlas>>>,
    instance: u32,
    want_normal: bool,
) -> Option<Atlas> {
    let textures = load_atlas_textures(packages, index, cache, instance)?;
    pick(&textures, want_normal).cloned()
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

/// 底图格三级来源（§3d）：FD6 Int32 → FD2/FD3 逐分量 min 推导 → 顶点默认 8.0。
fn base_tile_of(file: &PropertyFile) -> (usize, &'static str) {
    if let Some(Value::Int32(v)) = scalar(file, H_BASE_TILE) {
        return ((*v).clamp(0, 15) as usize, "FD6");
    }
    if let (Some(a), Some(b)) = (
        vector2_at(file, H_BASE_TILE_MIN_A),
        vector2_at(file, H_BASE_TILE_MIN_B),
    ) {
        let mx = a[0].min(b[0]);
        let my = a[1].min(b[1]);
        let round = |v: f32| (v + 0.5).floor() as i32;
        let rx = round((mx + 1.0 / 64.0) * 4.0);
        let ry = round((my + 1.0 / 64.0) * 4.0);
        return (((rx + ry * 4).max(0) as usize) % ATLAS_CELLS, "FD2/FD3");
    }
    (8, "默认")
}

// ---- 像素杂项 ----

/// §5b 朝向对齐：行序翻转（V）+ 列序镜像（U）= 原始栅格旋转 180°。
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

fn to_rgba8(color: [f32; 4]) -> [u8; 4] {
    // LotColor 是线性颜色（引擎在 sRGB 输出端做伽马编码），逐分量 linear→sRGB。
    [
        linear_to_srgb_u8(color[0]),
        linear_to_srgb_u8(color[1]),
        linear_to_srgb_u8(color[2]),
        (color[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn linear_to_srgb_u8(linear: f32) -> u8 {
    (linear_to_srgb_f32(linear) * 255.0).round().clamp(0.0, 255.0) as u8
}

fn linear_to_srgb_f32(linear: f32) -> f32 {
    let linear = if linear.is_finite() {
        linear.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear_f32(srgb: f32) -> f32 {
    if srgb <= 0.040_45 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

fn srgb3_to_linear3(px: [u8; 3]) -> [f32; 3] {
    std::array::from_fn(|i| srgb_to_linear_f32(f32::from(px[i]) / 255.0))
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
    let Some(image) = RgbaImage::from_raw(ow as u32, oh as u32, pixels) else {
        return;
    };
    image
        .save(format!("{dir}/{name}"))
        .unwrap_or_else(|error| panic!("save {name}: {error}"));
}
