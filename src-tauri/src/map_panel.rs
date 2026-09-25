//! 地图开发面板：区域枚举 + 彩色俯视渲染。仅预览用途；
//! 未来在渲染缓冲上叠加多层资源视图与笔刷能力。
//! ED 网格已确认为全游戏共享布局（region_map::SHARED_ED_GRID），
//! 无需按区域求解/缓存。
use base64::Engine as _;
use serde::Serialize;

use sc_properties::{region_edit, region_map, region_write};

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
    water_override: Option<f64>,
) -> Result<RegionRenderDto, String> {
    let group = u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;

    // 水位实时预览：调用方可传世界米（编辑器输入），换算 raw 后覆盖区域 desc 值
    let water_raw = match water_override {
        Some(meters) if meters.is_finite() => Some(
            (((meters + 1024.0) * 32.0).round() as f64)
                .clamp(0.0, 65535.0) as i32,
        ),
        _ => None,
    };
    let out = region_map::render_region_png(&package, group, water_raw).map_err(|e| e.to_string())?;
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

/// overlay 写回结果（单条目 property 补丁共用）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayWriteDto {
    pub entry_count: usize,
    pub size_bytes: usize,
    pub out_path: String,
}

/// 把单条目 overlay 原子落盘（父目录自动创建）。
fn write_overlay(out_path: &str, bytes: Vec<u8>) -> Result<(usize, usize), String> {
    let size_bytes = bytes.len();
    if let Some(parent) = std::path::Path::new(out_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    crate::atomic_fs::write_atomic(std::path::Path::new(out_path), &bytes, true)
        .map_err(|e| e.to_string())?;
    Ok((1, size_bytes))
}

/// 画刷清单（含条目 instance，供编辑命令定位；stamps 为世界坐标米）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrushListDto {
    pub instance: String,
    pub name: String,
    pub map_name: Option<String>,
    pub stamps: Vec<(f32, f32)>,
}

/// 枚举某区域组的画刷清单（E4 编辑器的数据源）。
#[tauri::command]
pub fn map_panel_list_brushes(
    package_path: String,
    group: String,
) -> Result<Vec<BrushListDto>, String> {
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    Ok(region_edit::brush_lists(&package, group)?
        .into_iter()
        .map(|b| BrushListDto {
            instance: format!("{:08X}", b.instance),
            name: b.name,
            map_name: b.map_name,
            stamps: b.stamps,
        })
        .collect())
}

/// 写回水位（E2）：`meters` 为世界米（如 -870.0），补丁区域 desc 的
/// 0x0E16BE1A 并生成单条目 overlay 包（原子落盘）。
#[tauri::command]
pub fn map_panel_set_water_level(
    package_path: String,
    group: String,
    meters: f64,
    out_path: String,
) -> Result<OverlayWriteDto, String> {
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let meters = if meters.is_finite() { meters as f32 } else { return Err("水位必须为有限数".to_string()) };
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    let Some(entry) = package.entries().iter().find(|e| {
        e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == region_edit::REGION_DESC_INSTANCE
    }) else {
        return Err(format!("组 0x{group:08X} 缺少区域 desc（0x{:08X}）", region_edit::REGION_DESC_INSTANCE));
    };
    let data = package.read(entry).map_err(|e| e.to_string())?;
    let patched = region_edit::patch_property_float(
        &data,
        region_edit::HASH_WATER_LEVEL,
        meters,
    )?;
    let bytes = region_edit::single_entry_overlay(region_edit::REGION_DESC_INSTANCE, group, patched)?;
    let (entry_count, size_bytes) = write_overlay(&out_path, bytes)?;
    Ok(OverlayWriteDto {
        entry_count,
        size_bytes,
        out_path,
    })
}

/// 编辑画刷 stamp（E4）：对指定清单 property 增/删 stamp 并生成单条目
/// overlay 包（原子落盘）。`add` 为世界坐标 (x, y) 米；`remove` 为现有
/// stamp 索引；`strength` 提供时覆盖新增 stamp 的末位强度。
#[tauri::command]
pub fn map_panel_edit_brush_stamps(
    package_path: String,
    group: String,
    brush_instance: String,
    add: Vec<(f64, f64)>,
    remove: Vec<u32>,
    strength: Option<f64>,
    out_path: String,
) -> Result<OverlayWriteDto, String> {
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let instance =
        u32::from_str_radix(brush_instance.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    let Some(entry) = package.entries().iter().find(|e| {
        e.id.type_id == 0x00B1_B104 && e.id.group == group && e.id.instance == instance
    }) else {
        return Err(format!("未找到画刷清单 0x{instance:08X}"));
    };
    let data = package.read(entry).map_err(|e| e.to_string())?;
    let add: Vec<(f32, f32)> = add
        .iter()
        .map(|&(x, y)| {
            if !x.is_finite() || !y.is_finite() {
                return Err("坐标必须为有限数".to_string());
            }
            Ok((x as f32, y as f32))
        })
        .collect::<Result<_, String>>()?;
    let strength = match strength {
        Some(s) if s.is_finite() => Some(s as f32),
        Some(_) => return Err("强度必须为有限数".to_string()),
        None => None,
    };
    let remove: Vec<usize> = remove.iter().map(|&i| i as usize).collect();
    let (patched, _) = region_edit::edit_brush_stamps(&data, &add, &remove, strength)?;
    let bytes = region_edit::single_entry_overlay(instance, group, patched)?;
    let (entry_count, size_bytes) = write_overlay(&out_path, bytes)?;
    Ok(OverlayWriteDto {
        entry_count,
        size_bytes,
        out_path,
    })
}

/// 窗口渲染（L1）：`x0/y0/x1/y1` 为世界坐标（米），裁剪到区域范围后
/// 按原生 8 m/px 输出该窗口的高保真渲染（同整图管线，含地块/画刷/资源图层，
/// 坐标相对窗口原点）。用于放大后的快速局部重渲染。
#[tauri::command]
pub fn map_panel_render_region_window(
    package_path: String,
    group: String,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) -> Result<RegionRenderDto, String> {
    const HALF: f64 = 16384.0;
    const CELL: f64 = 8.0;
    const MAX_WINDOW_PX: f64 = 2048.0;
    let group =
        u32::from_str_radix(group.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
    let package = dbpf::Package::open(std::path::PathBuf::from(&package_path))
        .map_err(|e| e.to_string())?;
    // 世界 → 场像素（region_map 同一换算：tile = (extent/2 + world) / 2048）
    let to_px = |v: f64| ((v + HALF) / CELL).round();
    let mut px0 = to_px(x0);
    let mut py0 = to_px(y0);
    let mut px1 = to_px(x1);
    let mut py1 = to_px(y1);
    // 限幅：区域范围 + 单窗口最大边
    let clamp = |v: f64, lo: f64, hi: f64| v.clamp(lo, hi);
    px0 = clamp(px0, 0.0, 4096.0);
    py0 = clamp(py0, 0.0, 4096.0);
    px1 = clamp(px1, 0.0, 4096.0).max(px0 + 1.0);
    py1 = clamp(py1, 0.0, 4096.0).max(py0 + 1.0);
    if px1 - px0 > MAX_WINDOW_PX {
        px1 = px0 + MAX_WINDOW_PX;
    }
    if py1 - py0 > MAX_WINDOW_PX {
        py1 = py0 + MAX_WINDOW_PX;
    }
    let out = region_map::render_region_png_window(
        &package,
        group,
        None,
        Some((px0 as usize, py0 as usize, px1 as usize, py1 as usize)),
    )
    .map_err(|e| e.to_string())?;
    let to_window_px = |(wx, wy): (f32, f32)| -> (f32, f32) {
        (
            (wx - out.origin_world.0) / out.meters_per_pixel,
            (wy - out.origin_world.1) / out.meters_per_pixel,
        )
    };
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
        plots: out.plots.iter().copied().map(to_window_px).collect(),
        brushes: out
            .brushes
            .iter()
            .map(|(name, stamps)| {
                (
                    name.clone(),
                    stamps.iter().copied().map(to_window_px).collect::<Vec<_>>(),
                )
            })
            .collect(),
        resource_layers: out
            .resource_layers
            .iter()
            .map(|(kind, png)| ResourceLayerDto {
                kind: kind.clone(),
                png_base64: base64::engine::general_purpose::STANDARD.encode(png),
            })
            .collect(),
    })
}
