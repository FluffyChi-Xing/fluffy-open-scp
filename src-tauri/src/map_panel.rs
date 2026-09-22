//! 地图开发面板：区域枚举 + 彩色俯视渲染。仅预览用途；
//! 未来在渲染缓冲上叠加多层资源视图与笔刷能力。
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use base64::Engine as _;
use serde::Serialize;

use sc_properties::region_map;

/// ED 金字塔网格按区域缓存（每区域首次求解约数十秒，之后瞬时）。
pub struct MapPanelState {
    ed_grids: Mutex<HashMap<String, [[u32; 16]; 16]>>,
}

impl Default for MapPanelState {
    fn default() -> Self {
        Self { ed_grids: Mutex::new(HashMap::new()) }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionSummaryDto {
    pub group: String,
    pub display_name: Option<String>,
    pub numeric_id: String,
    pub plot_count: usize,
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
    pub plot_count: usize,
    pub brushes: Vec<(String, Vec<(f32, f32)>)>,
}

#[tauri::command]
pub fn map_panel_list_regions(package_path: String) -> Result<Vec<RegionSummaryDto>, String> {
    let package = dbpf::Package::open(PathBuf::from(&package_path)).map_err(|e| e.to_string())?;
    Ok(region_map::list_regions(&package)
        .into_iter()
        .map(|r| RegionSummaryDto {
            group: format!("{:08X}", r.group),
            display_name: r.display_name,
            numeric_id: r.numeric_id,
            plot_count: r.plot_count,
        })
        .collect())
}

#[tauri::command]
pub fn map_panel_render_region(
    state: tauri::State<MapPanelState>,
    package_path: String,
    group: String,
) -> Result<RegionRenderDto, String> {
    let group = u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(PathBuf::from(&package_path)).map_err(|e| e.to_string())?;

    let key = format!("{package_path}:{group:08X}");
    let mut cache = state.ed_grids.lock().map_err(|e| e.to_string())?;
    let ed_grid = match cache.get(&key) {
        Some(g) => Some(*g),
        None => {
            let g = region_map::ed_grid_for_region(&package, group).ok_or("ED 金字塔求解失败")?;
            cache.insert(key, g);
            Some(g)
        }
    };

    let out = region_map::render_region_png(&package, group, ed_grid.as_ref(), None)
        .map_err(|e| e.to_string())?;
    Ok(RegionRenderDto {
        png_base64: base64::engine::general_purpose::STANDARD.encode(&out.png),
        width: out.width,
        height: out.height,
        origin_world: out.origin_world,
        meters_per_pixel: out.meters_per_pixel,
        water_plane: out.water_plane,
        desert: out.desert,
        display_name: out.display_name,
        plot_count: out.plots.len(),
        brushes: out.brushes,
    })
}
