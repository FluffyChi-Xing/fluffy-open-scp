//! 地图开发面板：区域枚举 + 彩色俯视渲染。仅预览用途；
//! 未来在渲染缓冲上叠加多层资源视图与笔刷能力。
//! ED 网格已确认为全游戏共享布局（region_map::SHARED_ED_GRID），
//! 无需按区域求解/缓存。
use base64::Engine as _;
use serde::Serialize;

use sc_properties::{region_map, region_write};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSummaryDto {
    pub group: String,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
    pub numeric_id: String,
    pub plot_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLayerDto {
    /// 资源 kind（coal/ore/oil/...）。
    pub kind: String,
    /// RGBA PNG（裁剪框 1/2 尺寸，前端拉伸到裁剪框显示）。
    pub png_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionRenderDto {
    pub png_base64: String,
    pub width: u32,
    pub height: u32,
    pub origin_world: (f32, f32),
    pub meters_per_pixel: f32,
    pub water_plane: i32,
    pub desert: bool,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
    pub plot_count: usize,
    /// 城市地块中心——**PNG 像素坐标**（前端零换算直接绘制）。
    pub plots: Vec<(f32, f32)>,
    /// 资源画刷：目标 map 名 + 各 stamp **PNG 像素坐标**。
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
    /// 资源分布图层（游戏数据视图同款等值线色带）。
    pub resource_layers: Vec<ResourceLayerDto>,
}

#[tauri::command]
pub fn map_panel_list_regions(package_path: String) -> Result<Vec<RegionSummaryDto>, String> {
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    // 真名链：区域模板 join → 注册表 → zh locale（旁包缺失时回退内置表）
    let parent = std::path::Path::new(&package_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let sibling = |name: &str| dbpf::Package::open(parent.join(name)).ok();
    let ep1 = sibling("SimCityDataEP1.package");
    let game = sibling("SimCity_Game.package");
    let locale = dbpf::Package::open(parent.join("Locale").join("zh-tw").join("Data.package")).ok();
    Ok(sc_properties::region_3d::region_list_named(
        &package,
        ep1.as_ref(),
        game.as_ref(),
        locale.as_ref(),
    )
    .into_iter()
    .map(|r| RegionSummaryDto {
        group: format!("{:08X}", r.group),
        display_name: r.display_name,
        display_name_en: r.display_name_en,
        numeric_id: r.numeric_id,
        plot_count: r.plot_count,
    })
    .collect())
}

#[tauri::command]
pub fn map_panel_render_region(
    package_path: String,
    group: String,
) -> Result<RegionRenderDto, String> {
    let group = u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;

    let out = region_map::render_region_png(&package, group, None).map_err(|e| e.to_string())?;
    // 世界坐标 → PNG 像素：前端零换算直接绘制（避免 originWorld 断链导致整体偏移）。
    // metersPerPixel = 米/像素，故除。
    let to_px = |(wx, wy): (f32, f32)| -> (f32, f32) {
        (
            (wx - out.origin_world.0) / out.meters_per_pixel,
            (wy - out.origin_world.1) / out.meters_per_pixel,
        )
    };
    let plots = out.plots.iter().copied().map(to_px).collect();
    let brushes = out
        .brushes
        .iter()
        .map(|(name, stamps)| {
            (
                name.clone(),
                stamps.iter().copied().map(to_px).collect::<Vec<_>>(),
            )
        })
        .collect();
    let resource_layers = out
        .resource_layers
        .iter()
        .map(|(kind, png)| ResourceLayerDto {
            kind: kind.clone(),
            png_base64: base64::engine::general_purpose::STANDARD.encode(png),
        })
        .collect();
    Ok(RegionRenderDto {
        png_base64: base64::engine::general_purpose::STANDARD.encode(&out.png),
        width: out.width,
        height: out.height,
        origin_world: out.origin_world,
        meters_per_pixel: out.meters_per_pixel,
        water_plane: out.water_plane,
        desert: out.desert,
        display_name: out.display_name,
        display_name_en: out.display_name_en,
        plot_count: out.plots.len(),
        plots,
        brushes,
        resource_layers,
    })
}

/// 3D 预览数据。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Region3DDto {
    /// 高度 PNG（RGBA：R=raw 高 8 位、G=低 8 位，无损 16-bit 编码）。
    pub height_png_base64: String,
    /// 生态 PNG（RGBA：R=土壤、G=森林、B=地下水、A=255）。
    pub ground_png_base64: String,
    pub terrain_textures: std::collections::HashMap<String, String>,
    pub size: u32,
    pub meters_per_pixel: f32,
    pub origin_world: (f32, f32),
    pub water_z: f32,
    pub height_div: f32,
    pub height_bias: f32,
    pub desert: bool,
    pub display_name: Option<String>,
    pub display_name_en: Option<String>,
    pub plots: Vec<sc_properties::region_3d::Region3DPlot>,
    /// 资源画刷清单（目标 map 名 + stamp 世界坐标）。
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
}

/// 区域 3D 数据（高度场 + 地面场 + 地块名 + 伟工位）。
/// 自动探测旁包：同目录 SimCityDataEP1.package / SimCity_Game.package /
/// Locale/zh-tw/Data.package（缺失时名字链与伟工位降级）。
#[tauri::command]
pub fn map_panel_region_3d(package_path: String, group: String) -> Result<Region3DDto, String> {
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    let parent = std::path::Path::new(&package_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let sibling = |name: &str| -> Option<dbpf::Package> {
        dbpf::Package::open(parent.join(name)).ok()
    };
    let ep1 = sibling("SimCityDataEP1.package");
    let game = sibling("SimCity_Game.package");
    let locale = parent
        .join("Locale")
        .join("zh-tw")
        .join("Data.package");
    let locale = dbpf::Package::open(locale).ok();
    let out = sc_properties::region_3d::region_3d(
        &package,
        group,
        ep1.as_ref(),
        game.as_ref(),
        locale.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    Ok(Region3DDto {
        terrain_textures: sibling("SimCity_App.package")
            .map(|p| sc_properties::region_3d::terrain_detail_pngs(&p))
            .unwrap_or_default().into_iter().map(|(name, png)|
                (name, base64::engine::general_purpose::STANDARD.encode(png))).collect(),
        height_png_base64: base64::engine::general_purpose::STANDARD.encode(&out.height_png),
        ground_png_base64: base64::engine::general_purpose::STANDARD.encode(&out.ground_png),
        size: out.size,
        meters_per_pixel: out.meters_per_pixel,
        origin_world: out.origin_world,
        water_z: out.water_z,
        height_div: out.height_div,
        height_bias: out.height_bias,
        desert: out.desert,
        display_name: out.display_name,
        display_name_en: out.display_name_en,
        plots: out.plots,
        brushes: out.brushes,
    })
}

/// 高度图写回结果。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeightmapWriteDto {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub out_path: String,
    /// 输入高度场边长（恒 4096）。
    pub region_px: u32,
}

/// 高度图回写（map-workbench E1 / priorities P0-1）。
///
/// 输入 16-bit 灰度 PNG（4096²；像素值 = 高度 raw 值，raw = (z+1024)×32，
/// 1 px = 8 m；8-bit PNG 会被升位，精度受限，导入 UI 应优先 16-bit），
/// 经 341-tile 金字塔重排（复用源包各 tile 原始 20 B 头）生成未压缩
/// overlay 包并原子落盘。同步命令，耗时约亚秒级（与 render_region 同量级）。
#[tauri::command]
pub fn map_panel_write_heightmap(
    package_path: String,
    group: String,
    heights_png_path: String,
    out_path: String,
) -> Result<HeightmapWriteDto, String> {
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    let img = image::open(&heights_png_path).map_err(|e| format!("读取高度图 PNG 失败：{e}"))?;
    let (width, height) = (img.width(), img.height());
    if width != region_write::REGION_FIELD_PX as u32
        || height != region_write::REGION_FIELD_PX as u32
    {
        return Err(format!("高度图尺寸 {width}×{height} ≠ 4096×4096"));
    }
    let heights = img.into_luma16().into_raw();
    let bytes = region_write::build_heightmap_overlay(&package, group, &heights)?;
    let size_bytes = bytes.len();
    if let Some(parent) = std::path::Path::new(&out_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    crate::atomic_fs::write_atomic(std::path::Path::new(&out_path), &bytes, true)
        .map_err(|e| e.to_string())?;
    Ok(HeightmapWriteDto {
        entry_count: region_write::TILES_PER_REGION,
        size_bytes,
        out_path,
        region_px: region_write::REGION_FIELD_PX as u32,
    })
}
