//! 地图开发面板：区域枚举 + 彩色俯视渲染。仅预览用途；
//! 未来在渲染缓冲上叠加多层资源视图与笔刷能力。
//! ED 网格已确认为全游戏共享布局（region_map::SHARED_ED_GRID），
//! 无需按区域求解/缓存。
use base64::Engine as _;
use serde::Serialize;

use sc_properties::region_map;

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
    Ok(region_map::list_regions(&package)
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
