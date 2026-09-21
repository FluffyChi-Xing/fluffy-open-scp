//! Raster 绘制器的读写命令：
//! - `read_image_rgba`：导入外部 PNG/JPG → RGBA（透明背景进画布）；
//! - `read_raster_rgba`：包内 raster（pixFmt 21）解码 → RGBA 画布；
//! - `save_raster_overlay`：画布 RGBA → raster 字节（pixFmt21 + mip 链）→
//!   写出 overlay package（副本保存，源包不被触碰）；可选伴随 property
//!   （覆盖现有 lot 时同包写入源 lot property，副本包独立可达）；
//! - `list_property_documents`：扫描包内 lot 文档（有 LotMask 的 property）
//!   / decal atlas 文档，供覆盖目标与注册目标选择器使用；
//! - `register_decal_entry`：画布 → 新 raster + atlas property 追加/替换
//!   条目 → 两资源同包导出（decal 绘制→注册闭环）。
//!
//! 编码/解码本体在 `rw4::raster`；decal 条目写回在 `sc_properties::decal`；
//! 本模块只做命令编排与上限校验。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use dbpf::{OverlayEntry, ResourceId};
use rw4::RasterImage;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::package_service::{find_raster_entry, write_export, TgiDto};

const RASTER_TYPE_ID: u32 = 0x2F4E_681C;
const PROPERTY_TYPE_ID: u32 = 0x00B1_B104;
/// 导入图片的像素上限（4096²）：与 RESOURCE_DATA_MAX 同数量级的防失控闸。
const IMPORT_IMAGE_MAX_PIXELS: usize = 4096 * 4096;

#[derive(Debug, thiserror::Error)]
enum RasterEditError {
    #[error("image path is empty")]
    InvalidPath,
    #[error("invalid request: {0}")]
    InvalidArgument(String),
    #[error("image exceeds the {0}px pixel budget")]
    TooLarge(usize),
    #[error("companion resource must be a property ({0:#010x}) resource")]
    InvalidCompanion(u32),
    #[error("companion resource exceeds the data budget")]
    CompanionTooLarge,
    #[error("target resource is not a decal dictionary")]
    NotDecalDictionary,
    #[error("property parse error: {0}")]
    Properties(String),
    #[error("decal entry id was not found in the atlas")]
    EntryNotFound,
    #[error("raster image is not writable (compressed pixel format)")]
    NotWritable,
    #[error("output path already exists")]
    OutputExists(String),
    #[error("overlay package write failed: {0}")]
    Overlay(String),
    #[error("raster edit io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("raster edit image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("raster edit store error: {0}")]
    Package(#[from] crate::package_service::PackageError),
    #[error("raster format error: {0}")]
    Rw4(#[from] rw4::Error),
    #[error("raster edit dbpf error: {0}")]
    Dbpf(#[from] dbpf::Error),
    #[error("raster resource was not found")]
    NotFound,
}

impl RasterEditError {
    fn code(&self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::InvalidArgument(_) => "invalid_argument",
            Self::TooLarge(_) => "limit_exceeded",
            Self::InvalidCompanion(_) => "invalid_companion",
            Self::CompanionTooLarge => "limit_exceeded",
            Self::NotDecalDictionary => "not_decal_dictionary",
            Self::Properties(_) => "property_parse",
            Self::EntryNotFound => "entry_not_found",
            Self::NotWritable => "not_writable",
            Self::OutputExists(_) => "output_exists",
            Self::Overlay(_) => "overlay_write",
            Self::Rw4(_) => "raster_format",
            Self::Dbpf(_) | Self::NotFound => "raster_edit_error",
            Self::Io(_) | Self::Image(_) | Self::Package(_) => "raster_edit_error",
        }
    }
}

impl From<RasterEditError> for CommandError {
    fn from(error: RasterEditError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageFileRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageRgbaResponse {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
}

#[tauri::command]
pub async fn read_image_rgba(
    request: ImageFileRequest,
) -> Result<ImageRgbaResponse, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        if request.path.is_empty() {
            return Err(RasterEditError::InvalidPath);
        }
        let bytes = fs::read(&request.path).map_err(RasterEditError::from)?;
        let image = image::load_from_memory(&bytes).map_err(RasterEditError::from)?;
        let (width, height) = (image.width(), image.height());
        if width as usize * height as usize > IMPORT_IMAGE_MAX_PIXELS {
            return Err(RasterEditError::TooLarge(IMPORT_IMAGE_MAX_PIXELS));
        }
        let rgba = image.to_rgba8().into_raw();
        Ok(ImageRgbaResponse {
            width,
            height,
            rgba_base64: base64::engine::general_purpose::STANDARD.encode(rgba),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterRgbaRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterRgbaResponse {
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub pixel_format: u32,
    /// pixFmt 21 才可编辑；压缩变体仅元数据（前端禁用编辑）。
    pub decodable: bool,
    pub rgba_base64: Option<String>,
}

#[tauri::command]
pub async fn read_raster_rgba(
    state: State<'_, AppState>,
    request: RasterRgbaRequest,
) -> Result<RasterRgbaResponse, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
    let package = manager.get(request.package_id)?;
    let tgi: ResourceId = request.tgi.into();
    // SCP 定位语义（= find_raster_entry）：精确 TGI 须 raster 类型，失败时按
    // instance+RASTER_TYPE 扫描——LotMask key 的 type/group 惯例为 0，
    // 拿 key 原样精确查找必失配（Graphics lot 覆盖目标载入失败的根因）。
    let mask_key = sc_properties::Key {
        instance: tgi.instance,
        type_id: tgi.type_id,
        group: tgi.group,
    };
    let entry_id = find_raster_entry(&package, mask_key).ok_or(RasterEditError::NotFound)?;
    let entry = package
        .entry(entry_id)
        .ok_or(RasterEditError::NotFound)?;
    let data = package.read(entry)?;
        let image = RasterImage::parse(&data)?;
        let decodable = image.is_raw_rgba();
        let rgba_base64 = if decodable {
            Some(base64::engine::general_purpose::STANDARD.encode(
                image.decode_top_mip_rgba()?,
            ))
        } else {
            None
        };
        Ok::<RasterRgbaResponse, RasterEditError>(RasterRgbaResponse {
            width: image.width,
            height: image.height,
            mip_count: image.mip_count,
            pixel_format: image.pixel_format,
            decodable,
            rgba_base64,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

/// 同包伴随 property 引用：覆盖现有 lot 时指向源 lot property。服务端读取
/// 整份副本与 raster 写进同一 overlay package——游戏按 TGI 整资源覆盖，
/// 副本包因此独立可达（不要求玩家装有同版本的源包）。
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanionPropertyRef {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRasterOverlayRequest {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
    /// 副本保留源 TGI（type 固定 raster）；外部导入由前端生成 instance。
    pub tgi: TgiDto,
    pub output_path: String,
    pub generate_mips: bool,
    /// 覆盖现有 lot 时填源 lot property：同包写入整份副本。
    #[serde(default)]
    pub companion_property: Option<CompanionPropertyRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRasterOverlayResult {
    pub output_path: String,
    pub tgi: TgiDto,
    pub raster_bytes: u64,
    pub mip_count: u32,
    /// overlay 包内全部资源（raster + 伴随 property），供结果回显。
    pub resources: Vec<TgiDto>,
}

#[tauri::command]
pub async fn save_raster_overlay(
    state: State<'_, AppState>,
    request: SaveRasterOverlayRequest,
) -> Result<SaveRasterOverlayResult, CommandError> {
    use base64::engine::general_purpose::STANDARD;

    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        if request.tgi.type_id != RASTER_TYPE_ID {
            return Err(RasterEditError::InvalidArgument(
                "raster overlay requires a raster (0x2F4E681C) tgi".into(),
            ));
        }
        if request.output_path.is_empty() {
            return Err(RasterEditError::InvalidPath);
        }
        let rgba = STANDARD
            .decode(request.rgba_base64.as_bytes())
            .map_err(|_| RasterEditError::InvalidPath)?;
        let image = RasterImage::build_raw_bgra(
            request.width,
            request.height,
            &rgba,
            request.generate_mips,
        )?;
        let raster_bytes = image.to_bytes();

        // 与包内解码口径互检：编码产物必须能原样解回（防手滑写坏格式）。
        let reparsed = RasterImage::parse(&raster_bytes)?;
        debug_assert_eq!(reparsed.mip_count, image.mip_count);

        let raster_id = ResourceId {
            type_id: RASTER_TYPE_ID,
            group: request.tgi.group,
            instance: request.tgi.instance,
        };
        let mut entries = vec![OverlayEntry::new(raster_id, raster_bytes.clone())];
        let mut resources = vec![request.tgi.clone()];
        if let Some(companion) = request.companion_property.as_ref() {
            let companion_id: ResourceId = companion.tgi.clone().into();
            if companion_id.type_id != PROPERTY_TYPE_ID {
                return Err(RasterEditError::InvalidCompanion(companion_id.type_id));
            }
            let package = manager.get(companion.package_id)?;
            let entry = package
                .entry(companion_id)
                .ok_or(RasterEditError::NotFound)?;
            if u64::from(entry.decompressed_size)
                > crate::package_service::RESOURCE_DATA_MAX
            {
                return Err(RasterEditError::CompanionTooLarge);
            }
            let data = package.read(entry)?;
            resources.push(companion.tgi.clone());
            entries.push(OverlayEntry::new(companion_id, data));
        }

        let overlay = dbpf::write_uncompressed_overlay(&entries)
            .map_err(|error| RasterEditError::Overlay(error.to_string()))?;

        let output_path = PathBuf::from(&request.output_path);
        // 前端 save 对话框已确保目标不存在；再次确认防覆盖。
        if output_path.exists() {
            return Err(RasterEditError::OutputExists(
                output_path.to_string_lossy().into_owned(),
            ));
        }
        write_export(&output_path, &overlay)?;

        Ok(SaveRasterOverlayResult {
            output_path: output_path.to_string_lossy().into_owned(),
            tgi: request.tgi,
            raster_bytes: raster_bytes.len() as u64,
            mip_count: image.mip_count,
            resources,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

// ── 文档扫描：lot 覆盖目标 / decal atlas 注册目标 ──

/// 文档扫描种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocumentKind {
    Lot,
    Decal,
}

impl DocumentKind {
    fn parse(kind: &str) -> Result<Self, RasterEditError> {
        match kind {
            "lot" => Ok(Self::Lot),
            "decal" => Ok(Self::Decal),
            other => Err(RasterEditError::InvalidArgument(format!(
                "unknown document kind {other}"
            ))),
        }
    }
}

/// 单个 property 文档的扫描摘要（lot 覆盖目标 / decal 注册目标选择器的数据源）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyDocumentSummary {
    pub tgi: TgiDto,
    /// kind=lot：LotMask（0x0CCB7FD5）引用的 raster TGI（覆盖目标）。
    pub lot_mask: Option<TgiDto>,
    /// kind=lot：LotMask 光栅实际所在的 package id（跨包解析结果；
    /// None = 所有已打开包中都找不到）。
    #[serde(default)]
    pub lot_mask_package_id: Option<u64>,
    /// kind=lot：解析命中的 raster 完整 TGI。SCP 的 LotMask key 惯例为
    /// 残缺 TGI（type/group=0），读取与覆盖导出都必须用解析后的完整 TGI，
    /// 否则精确查找必失配（Graphics 包 1640/1640 实证）。
    #[serde(default)]
    pub lot_mask_resolved: Option<TgiDto>,
    /// kind=lot：LotSize（米）。
    pub lot_size: Option<[f32; 2]>,
    /// kind=decal：条目数。
    pub entry_count: Option<usize>,
    /// kind=decal：MaterialId instance。
    pub material_instance: Option<u32>,
}

/// 解析单个 property 资源并按种类摘要；不构成目标文档的资源返回 None。
fn summarize_property_document(
    kind: DocumentKind,
    tgi: TgiDto,
    data: &[u8],
) -> Option<PropertyDocumentSummary> {
    let file = sc_properties::PropertyFile::parse(data).ok()?;
    match kind {
        DocumentKind::Lot => {
            let document = sc_properties::LotEditorDocument::from_property_file(file);
            // 无 LotMask 的 property 没有 raster 可覆盖，不作为目标列出。
            let lot_mask = document.lot_mask.clone()?;
                Some(PropertyDocumentSummary {
                    tgi,
                    lot_mask: Some(key_to_tgi(&lot_mask)),
                    // 跨包解析在命令层做（此处拿不到 manager）。
                    lot_mask_package_id: None,
                    lot_mask_resolved: None,
                    lot_size: document.lot_size,
                    entry_count: None,
                    material_instance: None,
                })
        }
        DocumentKind::Decal => {
            if !sc_properties::is_decal_dictionary_group(tgi.group) {
                return None;
            }
            if !sc_properties::decal::looks_like_dictionary(&file) {
                return None;
            }
            let dictionary = sc_properties::DecalDictionary::from_file(&file);
            Some(PropertyDocumentSummary {
                entry_count: Some(dictionary.entries.len()),
                material_instance: dictionary.material.map(|key| key.instance),
                tgi,
                lot_mask: None,
                lot_mask_package_id: None,
                lot_mask_resolved: None,
                lot_size: None,
            })
        }
    }
}

/// 跨包 SCP 定位 LotMask 光栅：源包优先，其余已打开包按打开顺序兜底。
/// 返回 (package_id, 实际命中的 raster TGI)。查找语义与 property editor
/// 的 `find_raster_entry` 一致（精确 TGI 须 raster 类型 → instance +
/// RASTER_TYPE 扫描）：LotMask key 惯例为残缺 TGI（type/group=0），拿
/// key 原样精确查找必失配——这正是「LotMask 光栅在所有已打开的 package
/// 中都不存在」误报的根因（Graphics 包 1640 个 mask 全部可由此定位）。
fn locate_raster_for_mask<P: std::borrow::Borrow<dbpf::Package>>(
    packages: &[(u64, P)],
    source_package_id: u64,
    mask: &sc_properties::Key,
) -> Option<(u64, ResourceId)> {
    let locate = |(id, package): &(u64, P)| {
        find_raster_entry(package.borrow(), *mask).map(|entry| (*id, entry))
    };
    packages
        .iter()
        .find(|(id, _)| *id == source_package_id)
        .and_then(locate)
        .or_else(|| {
            packages
                .iter()
                .filter(|(id, _)| *id != source_package_id)
                .find_map(locate)
        })
}

#[tauri::command]
pub async fn list_property_documents(
    state: State<'_, AppState>,
    package_id: u64,
    kind: String,
) -> Result<Vec<PropertyDocumentSummary>, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let kind = DocumentKind::parse(&kind)?;
        let package = manager.get(package_id)?;
        let mut summaries = Vec::new();
        for entry in package.entries().iter() {
            if entry.id.type_id != PROPERTY_TYPE_ID {
                continue;
            }
            // decal 先按 group 低 16 位廉价预筛，避免全量解析。
            if kind == DocumentKind::Decal
                && !sc_properties::is_decal_dictionary_group(entry.id.group)
            {
                continue;
            }
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let tgi = TgiDto::from(entry.id);
            let mut summary = match summarize_property_document(kind, tgi, &data) {
                Some(summary) => summary,
                None => continue,
            };
            // lot 目标：SCP 语义解析 mask 光栅位置（Graphics 的 lot 引用
            // 本包/Game 包 raster，key 惯例残缺 TGI），前端据解析结果直读。
            if kind == DocumentKind::Lot {
                if let Some(mask) = summary.lot_mask.as_ref() {
                    let key = sc_properties::Key {
                        instance: mask.instance,
                        type_id: mask.type_id,
                        group: mask.group,
                    };
                    if let Ok(packages) = manager.all_packages_with_ids() {
                        if let Some((package_id, resolved)) =
                            locate_raster_for_mask(&packages, package_id, &key)
                        {
                            summary.lot_mask_package_id = Some(package_id);
                            summary.lot_mask_resolved = Some(TgiDto {
                                type_id: resolved.type_id,
                                group: resolved.group,
                                instance: resolved.instance,
                            });
                        }
                    }
                }
            }
            summaries.push(summary);
        }
        summaries.sort_by_key(|summary| summary.tgi.instance);
        Ok::<Vec<PropertyDocumentSummary>, RasterEditError>(summaries)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

// ── Lot 材质授权：覆盖目标的 LotColor1-4（渲染预览同步用） ──

/// 覆盖目标 lot 的地表材质授权。渲染预览据其给四通道染色：
/// 通道→颜色 R→LC1、G→LC2、B→LC3、A→LC4（rw4 decode_lot_mask_rgba 同口径）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotMaterialResponse {
    /// LotColor1-4（sRGB RGB + A = 图集 tile 索引 0-15）。
    pub colors: [[u8; 4]; 4],
    /// 各槽位是否真实存在于 property（false = SCP 黑/红/绿/蓝回退色）。
    pub colors_authored: [bool; 4],
    /// "Lot Textures" 地表共享纹理图集（4×4 格，DXT5 解码 PNG base64）；
    /// None = 引用缺失或解码失败（预览回退本地占位 tile）。
    pub surface_png: Option<String>,
    /// 地面贴图周期 0x0CCB7FD0（米/格）；None = 引擎回退实测拟合常量 9.6m。
    pub tile_period: Option<[f32; 2]>,
}

#[tauri::command]
pub async fn read_lot_material(
    state: State<'_, AppState>,
    package_id: u64,
    tgi: TgiDto,
) -> Result<LotMaterialResponse, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let package = manager.get(package_id)?;
        let id: ResourceId = tgi.into();
        if id.type_id != PROPERTY_TYPE_ID {
            return Err(RasterEditError::InvalidArgument(
                "lot material requires a property (0x00B1B104) resource".into(),
            ));
        }
        let entry = package.entry(id).ok_or(RasterEditError::NotFound)?;
        if u64::from(entry.decompressed_size)
            > crate::package_service::RESOURCE_DATA_MAX
        {
            return Err(RasterEditError::CompanionTooLarge);
        }
        let data = package.read(entry)?;
        let properties = sc_properties::PropertyFile::parse_with_limits(
            &data,
            sc_properties::ParseLimits::default(),
        )
        .map_err(|error| RasterEditError::Properties(error.to_string()))?;
        // Parent(0x00B2CCCB) 继承展平：lot 变体常把 LotColors 挂在父级
        //（本级只有 LotMask/LOD），不展平会全部落到回退色。
        let properties =
            crate::package_service::flatten_lot_parents(properties, &package, &manager);
        let document = sc_properties::LotEditorDocument::from_property_file(properties);
        let (colors, colors_authored) = crate::package_service::lot_colors(&document);
        // "Lot Textures" 图集（4×4 共享纹理）：开启材质的预览合成源；
        // 缺失/解码失败降级为 None（前端回退本地占位 tile）。
        let surface_png = document.lot_textures.and_then(|key| {
            crate::package_service::decode_lot_surface_png(&package, &manager, key)
                .ok()
                .map(|(png, _)| png)
        });
        // 地面贴图周期（米/格）：引擎除数保护同 session 口径（近零置 0.1）。
        let tile_period = document
            .properties
            .get(0x0CCB_7FD0)
            .and_then(|property| property.scalar())
            .and_then(|value| match value {
                sc_properties::Value::Vector2(values) => Some(*values),
                _ => None,
            })
            .map(|values| {
                values.map(|value| if value.abs() < 0.1 { 0.1 } else { value })
            });
        Ok::<LotMaterialResponse, RasterEditError>(LotMaterialResponse {
            colors,
            colors_authored,
            surface_png,
            tile_period,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

// ── Decal 注册：画布 → raster + atlas property 条目 → 同包导出 ──

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDecalEntryRequest {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
    pub generate_mips: bool,
    /// 目标 decal atlas property（服务端读取 → 追加/替换条目）。
    pub atlas: CompanionPropertyRef,
    /// 新 raster 资源 TGI（type 固定 0x2F4E681C，instance 由前端生成）。
    pub raster_tgi: TgiDto,
    /// 条目 ID Key（游戏惯例 type/group = 0，instance 全字典唯一）。
    pub entry_id: TgiDto,
    /// 条目宽高比（建议 = 画布宽 / 高）。
    pub aspect_ratio: f32,
    /// Color1-4 存储口径（线性分量的一半，与既有字典一致）。
    pub colors: [[f32; 4]; 4],
    /// Some(instance) 时按条目 ID 替换既有行；None 追加新行。
    #[serde(default)]
    pub replace_instance: Option<u32>,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterDecalEntryResult {
    pub output_path: String,
    pub raster_tgi: TgiDto,
    pub atlas_tgi: TgiDto,
    /// 写入的条目下标（追加或被替换行的位置）。
    pub entry_index: usize,
    /// 导出后字典的条目总数。
    pub entry_count: usize,
    pub raster_bytes: u64,
}

#[tauri::command]
pub async fn register_decal_entry(
    state: State<'_, AppState>,
    request: RegisterDecalEntryRequest,
) -> Result<RegisterDecalEntryResult, CommandError> {
    use base64::engine::general_purpose::STANDARD;

    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        if request.output_path.is_empty() {
            return Err(RasterEditError::InvalidPath);
        }
        if request.raster_tgi.type_id != RASTER_TYPE_ID {
            return Err(RasterEditError::InvalidArgument(
                "decal raster tgi must be a raster (0x2F4E681C) resource".into(),
            ));
        }
        if request.atlas.tgi.type_id != PROPERTY_TYPE_ID {
            return Err(RasterEditError::InvalidCompanion(
                request.atlas.tgi.type_id,
            ));
        }
        if !sc_properties::is_decal_dictionary_group(request.atlas.tgi.group) {
            return Err(RasterEditError::NotDecalDictionary);
        }
        if !request.aspect_ratio.is_finite() || request.aspect_ratio <= 0.0 {
            return Err(RasterEditError::InvalidArgument(
                "aspect ratio must be a positive finite number".into(),
            ));
        }

        // 画布 → raster 字节（与 save_raster_overlay 同编码口径 + 互检）。
        let rgba = STANDARD
            .decode(request.rgba_base64.as_bytes())
            .map_err(|_| RasterEditError::InvalidPath)?;
        let image = RasterImage::build_raw_bgra(
            request.width,
            request.height,
            &rgba,
            request.generate_mips,
        )?;
        let raster_bytes = image.to_bytes();
        let reparsed = RasterImage::parse(&raster_bytes)?;
        debug_assert_eq!(reparsed.mip_count, image.mip_count);

        // atlas property → 追加/替换条目 → canonical 编码。
        let atlas_id: ResourceId = request.atlas.tgi.clone().into();
        let package = manager.get(request.atlas.package_id)?;
        let entry = package.entry(atlas_id).ok_or(RasterEditError::NotFound)?;
        if u64::from(entry.decompressed_size) > crate::package_service::RESOURCE_DATA_MAX {
            return Err(RasterEditError::CompanionTooLarge);
        }
        let data = package.read(entry)?;
        let mut file = sc_properties::PropertyFile::parse_with_limits(
            &data,
            sc_properties::ParseLimits::default(),
        )
        .map_err(|error| RasterEditError::Properties(error.to_string()))?;
        if !sc_properties::decal::looks_like_dictionary(&file) {
            return Err(RasterEditError::NotDecalDictionary);
        }
        let upsert = sc_properties::DecalEntryUpsert {
            id: dbpf_key(&request.entry_id),
            raster: dbpf_key(&request.raster_tgi),
            aspect_ratio: request.aspect_ratio,
            colors: request.colors,
        };
        let entry_index = sc_properties::upsert_entry(
            &mut file,
            &upsert,
            request.replace_instance,
        )
        .map_err(|error| match error {
            sc_properties::Error::DecalEntryNotFound(_) => RasterEditError::EntryNotFound,
            other => RasterEditError::InvalidArgument(other.to_string()),
        })?;
        let property_data = file.encode_canonical().map_err(|error| {
            RasterEditError::InvalidArgument(format!("atlas encode failed: {error}"))
        })?;
        let entry_count = sc_properties::DecalDictionary::from_file(&file).entries.len();

        // 两资源同包导出（raster + atlas property，副本语义）。
        let raster_id = ResourceId {
            type_id: RASTER_TYPE_ID,
            group: request.raster_tgi.group,
            instance: request.raster_tgi.instance,
        };
        let overlay = dbpf::write_uncompressed_overlay(&[
            OverlayEntry::new(raster_id, raster_bytes.clone()),
            OverlayEntry::new(atlas_id, property_data),
        ])
        .map_err(|error| RasterEditError::Overlay(error.to_string()))?;

        let output_path = PathBuf::from(&request.output_path);
        if output_path.exists() {
            return Err(RasterEditError::OutputExists(
                output_path.to_string_lossy().into_owned(),
            ));
        }
        write_export(&output_path, &overlay)?;

        Ok(RegisterDecalEntryResult {
            output_path: output_path.to_string_lossy().into_owned(),
            raster_tgi: request.raster_tgi,
            atlas_tgi: request.atlas.tgi,
            entry_index,
            entry_count,
            raster_bytes: raster_bytes.len() as u64,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

/// TgiDto → sc_properties::Key（条目 ID / raster 引用共用）。
fn dbpf_key(tgi: &TgiDto) -> sc_properties::Key {
    sc_properties::Key {
        instance: tgi.instance,
        type_id: tgi.type_id,
        group: tgi.group,
    }
}

// ── 新建 Lot：画布 → raster + 最小 Lot property → 同包导出 ──

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLotOverlayRequest {
    pub width: u32,
    pub height: u32,
    pub rgba_base64: String,
    pub generate_mips: bool,
    /// 新 raster 资源 TGI（type 固定 0x2F4E681C，instance 前端生成）。
    pub raster_tgi: TgiDto,
    /// 新 lot property 的 TGI（type 固定 0x00B1B104，instance 前端生成）。
    pub lot_tgi: TgiDto,
    /// LotSize（米）；缺省 = 画布 px × 0.75。
    #[serde(default)]
    pub lot_size: Option<[f32; 2]>,
    /// LotColor1-4：sRGB RGB + 地面图集 tile 索引 0-15（存储时换算线性）。
    pub colors: [[u8; 4]; 4],
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateLotOverlayResult {
    pub output_path: String,
    pub raster_tgi: TgiDto,
    pub lot_tgi: TgiDto,
    pub lot_size: [f32; 2],
    pub raster_bytes: u64,
}

/// sRGB 8bit → 线性分量（LotColor RGB 的存储口径，读取侧 linear_to_srgb_byte
/// 的逆变换）。
fn srgb_byte_to_linear(byte: u8) -> f32 {
    let srgb = f32::from(byte) / 255.0;
    if srgb <= 0.04045 {
        srgb / 12.92
    } else {
        ((srgb + 0.055) / 1.055).powf(2.4)
    }
}

/// 构造最小 Lot property：LotSize + LotMask + LotColor1-4（scalar ColorRgba，
/// RGB 线性、A = 图集 tile 索引 float；读取侧兼容 scalar/array 两种形态）。
fn build_new_lot_property(
    lot_size: [f32; 2],
    mask: sc_properties::Key,
    colors: [[u8; 4]; 4],
) -> sc_properties::PropertyFile {
    use sc_properties::{Kind, PropType, Property, PropertyEncoding, Value};

    let mut values = vec![
        Property {
            hash: sc_properties::LOT_SIZE_HASH,
            prop_type: PropType::Vector2,
            kind: Kind::Scalar(Value::Vector2(lot_size)),
            encoding: PropertyEncoding::default(),
        },
        Property {
            hash: sc_properties::LOT_MASK_HASH,
            prop_type: PropType::Key,
            kind: Kind::Scalar(Value::Key(mask)),
            encoding: PropertyEncoding::default(),
        },
    ];
    for (index, [r, g, b, tile]) in colors.iter().enumerate() {
        values.push(Property {
            hash: 0x0D02_D586 + index as u32,
            prop_type: PropType::ColorRgba,
            kind: Kind::Scalar(Value::ColorRgba {
                r: srgb_byte_to_linear(*r),
                g: srgb_byte_to_linear(*g),
                b: srgb_byte_to_linear(*b),
                a: f32::from(*tile),
            }),
            encoding: PropertyEncoding::default(),
        });
    }
    let claimed_count = values.len() as u32;
    sc_properties::PropertyFile {
        values,
        claimed_count,
    }
}

#[tauri::command]
pub async fn create_lot_overlay(
    request: CreateLotOverlayRequest,
) -> Result<CreateLotOverlayResult, CommandError> {
    use base64::engine::general_purpose::STANDARD;

    tauri::async_runtime::spawn_blocking(move || {
        if request.output_path.is_empty() {
            return Err(RasterEditError::InvalidPath);
        }
        if request.raster_tgi.type_id != RASTER_TYPE_ID {
            return Err(RasterEditError::InvalidArgument(
                "lot raster tgi must be a raster (0x2F4E681C) resource".into(),
            ));
        }
        if request.lot_tgi.type_id != PROPERTY_TYPE_ID {
            return Err(RasterEditError::InvalidArgument(
                "lot tgi must be a property (0x00B1B104) resource".into(),
            ));
        }
        if request.raster_tgi.instance == request.lot_tgi.instance
            && request.raster_tgi.group == request.lot_tgi.group
        {
            return Err(RasterEditError::InvalidArgument(
                "raster and lot property must have distinct TGIs".into(),
            ));
        }
        for color in &request.colors {
            if color[3] > 15 {
                return Err(RasterEditError::InvalidArgument(
                    "lot color atlas tile index must be within 0-15".into(),
                ));
            }
        }

        // 画布 → raster（与 save_raster_overlay 同编码口径 + 互检）。
        let rgba = STANDARD
            .decode(request.rgba_base64.as_bytes())
            .map_err(|_| RasterEditError::InvalidPath)?;
        let image = RasterImage::build_raw_bgra(
            request.width,
            request.height,
            &rgba,
            request.generate_mips,
        )?;
        let raster_bytes = image.to_bytes();
        let reparsed = RasterImage::parse(&raster_bytes)?;
        debug_assert_eq!(reparsed.mip_count, image.mip_count);

        let lot_size = request.lot_size.unwrap_or([
            f32::from(request.width as u16) * 0.75,
            f32::from(request.height as u16) * 0.75,
        ]);
        let property =
            build_new_lot_property(lot_size, dbpf_key(&request.raster_tgi), request.colors);
        let property_data = property.encode_canonical().map_err(|error| {
            RasterEditError::InvalidArgument(format!("lot property encode failed: {error}"))
        })?;

        // 两资源同包导出（raster + lot property，副本语义）。
        let raster_id = ResourceId {
            type_id: RASTER_TYPE_ID,
            group: request.raster_tgi.group,
            instance: request.raster_tgi.instance,
        };
        let lot_id: ResourceId = request.lot_tgi.clone().into();
        let overlay = dbpf::write_uncompressed_overlay(&[
            OverlayEntry::new(raster_id, raster_bytes.clone()),
            OverlayEntry::new(lot_id, property_data),
        ])
        .map_err(|error| RasterEditError::Overlay(error.to_string()))?;

        let output_path = PathBuf::from(&request.output_path);
        if output_path.exists() {
            return Err(RasterEditError::OutputExists(
                output_path.to_string_lossy().into_owned(),
            ));
        }
        write_export(&output_path, &overlay)?;

        Ok(CreateLotOverlayResult {
            output_path: output_path.to_string_lossy().into_owned(),
            raster_tgi: request.raster_tgi,
            lot_tgi: request.lot_tgi,
            lot_size,
            raster_bytes: raster_bytes.len() as u64,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}

/// sc_properties::Key → TgiDto（LotMask 引用回显共用）。
fn key_to_tgi(key: &sc_properties::Key) -> TgiDto {
    TgiDto {
        type_id: key.type_id,
        group: key.group,
        instance: key.instance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{Kind, PropType, Property, PropertyEncoding, Value};

    fn property_file_bytes(values: Vec<Property>) -> Vec<u8> {
        let claimed_count = values.len() as u32;
        let file = sc_properties::PropertyFile {
            values,
            claimed_count,
        };
        file.encode_canonical().unwrap()
    }

    fn scalar(hash: u32, prop_type: PropType, value: Value) -> Property {
        Property {
            hash,
            prop_type,
            kind: Kind::Scalar(value),
            encoding: PropertyEncoding::default(),
        }
    }

    fn raster_key(instance: u32) -> Value {
        Value::Key(sc_properties::Key {
            instance,
            type_id: RASTER_TYPE_ID,
            group: 0,
        })
    }

    #[test]
    fn lot_summary_requires_lot_mask_and_extracts_target() {
        use sc_properties::{LOT_MASK_HASH, LOT_SIZE_HASH};

        // 有 LotMask + LotSize：列出为覆盖目标。
        let bytes = property_file_bytes(vec![
            scalar(LOT_MASK_HASH, PropType::Key, raster_key(0xabcd)),
            scalar(
                LOT_SIZE_HASH,
                PropType::Vector2,
                Value::Vector2([48.0, 96.0]),
            ),
        ]);
        let summary = summarize_property_document(
            DocumentKind::Lot,
            TgiDto {
                type_id: PROPERTY_TYPE_ID,
                group: 0x40e1_c000,
                instance: 0x1234,
            },
            &bytes,
        )
        .unwrap();
        assert_eq!(summary.lot_mask.unwrap().instance, 0xabcd);
        assert_eq!(summary.lot_size, Some([48.0, 96.0]));
        assert_eq!(summary.entry_count, None);

        // 无 LotMask 的 property 不是覆盖目标。
        let bytes = property_file_bytes(vec![scalar(
            LOT_SIZE_HASH,
            PropType::Vector2,
            Value::Vector2([48.0, 96.0]),
        )]);
        assert!(
            summarize_property_document(
                DocumentKind::Lot,
                TgiDto {
                    type_id: PROPERTY_TYPE_ID,
                    group: 0,
                    instance: 1
                },
                &bytes
            )
            .is_none()
        );
    }

    #[test]
    fn decal_summary_counts_entries_and_requires_dictionary_shape() {
        use sc_properties::decal::DECAL_ENTRY_ID_HASH;

        let array_item_size = 12;
        let bytes = property_file_bytes(vec![Property {
            hash: DECAL_ENTRY_ID_HASH,
            prop_type: PropType::Key,
            kind: Kind::Array(vec![raster_key(0x1), raster_key(0x2)]),
            encoding: PropertyEncoding {
                flags: 0x30,
                array_item_size: Some(array_item_size),
            },
        }]);
        let summary = summarize_property_document(
            DocumentKind::Decal,
            TgiDto {
                type_id: PROPERTY_TYPE_ID,
                group: 0xc67f_b185,
                instance: 0x5678,
            },
            &bytes,
        )
        .unwrap();
        assert_eq!(summary.entry_count, Some(2));

        // 非 decal 家族 group 不列出。
        assert!(
            summarize_property_document(
                DocumentKind::Decal,
                TgiDto {
                    type_id: PROPERTY_TYPE_ID,
                    group: 0x40e1_c000,
                    instance: 0x5678
                },
                &bytes
            )
            .is_none()
        );
    }

    #[test]
    fn document_kind_parses_known_names_only() {
        assert_eq!(DocumentKind::parse("lot").unwrap(), DocumentKind::Lot);
        assert_eq!(DocumentKind::parse("decal").unwrap(), DocumentKind::Decal);
        assert!(DocumentKind::parse("model").is_err());
    }

    #[test]
    fn new_lot_property_round_trips_through_editor_document() {
        let mask = sc_properties::Key {
            instance: 0xabcd,
            type_id: RASTER_TYPE_ID,
            group: 0,
        };
        let colors = [[10, 20, 30, 0], [255, 0, 0, 1], [0, 128, 0, 8], [0, 0, 255, 15]];
        let file = build_new_lot_property([48.0, 96.0], mask, colors);

        // 编码必须可逆，且 LotEditorDocument 能读回语义字段。
        let encoded = file.encode_canonical().unwrap();
        let reparsed = sc_properties::PropertyFile::parse(&encoded).unwrap();
        let document =
            sc_properties::LotEditorDocument::from_property_file(reparsed);
        assert_eq!(document.lot_size, Some([48.0, 96.0]));
        assert_eq!(document.lot_mask.as_ref().map(|key| key.instance), Some(0xabcd));

        // LotColor1-4：读取侧的口径 = scalar/array 的 ColorRgba，RGB 线性、A = 索引。
        for (index, expected) in colors.iter().enumerate() {
            let property = document
                .properties
                .get(0x0D02_D586 + index as u32)
                .expect("lot color property");
            let value = property
                .scalar()
                .or_else(|| property.array().and_then(|values| values.first()))
                .expect("lot color value");
            let sc_properties::Value::ColorRgba {
                r: stored_r,
                g: stored_g,
                b: stored_b,
                a: stored_a,
            } = value
            else {
                panic!("lot color {index} is not ColorRgba");
            };
            let [r, g, b, tile] = *expected;
            assert_eq!(*stored_r, srgb_byte_to_linear(r));
            assert_eq!(*stored_g, srgb_byte_to_linear(g));
            assert_eq!(*stored_b, srgb_byte_to_linear(b));
            assert_eq!(*stored_a, f32::from(tile));
        }
    }

    #[test]
    fn mask_raster_resolves_through_scp_lookup_semantics() {
        use sc_properties::LOT_MASK_HASH;

        // 真实数据形态（Graphics 包 1640/1640 实证）：LotMask key 残缺
        // （type/group=0），raster 条目是完整 raster 类型 TGI——拿 key 原样
        // 精确查找必失配，须 instance+RASTER_TYPE 兜底并回报完整 TGI。
        let image = RasterImage::build_raw_bgra(4, 4, &[0u8; 4 * 4 * 4], false).unwrap();
        let raster_id = ResourceId {
            type_id: RASTER_TYPE_ID,
            group: 0x1a2b_0000,
            instance: 0x00c9_a566,
        };
        let property_bytes = property_file_bytes(vec![scalar(
            LOT_MASK_HASH,
            PropType::Key,
            Value::Key(sc_properties::Key {
                instance: 0x00c9_a566,
                type_id: 0,
                group: 0,
            }),
        )]);
        let raster_path = std::env::temp_dir()
            .join(format!("openscp-mask-raster-{}.package", std::process::id()));
        let property_path = std::env::temp_dir()
            .join(format!("openscp-mask-property-{}.package", std::process::id()));
        fs::write(
            &raster_path,
            dbpf::write_uncompressed_overlay(&[OverlayEntry::new(raster_id, image.to_bytes())])
                .unwrap(),
        )
        .unwrap();
        fs::write(
            &property_path,
            dbpf::write_uncompressed_overlay(&[OverlayEntry::new(
                ResourceId {
                    type_id: PROPERTY_TYPE_ID,
                    group: 0,
                    instance: 0x00c9_a566,
                },
                property_bytes,
            )])
            .unwrap(),
        )
        .unwrap();
        let mask = sc_properties::Key {
            instance: 0x00c9_a566,
            type_id: 0,
            group: 0,
        };

        // mask 与 lot 同包：源包优先命中，并回报解析后的完整 raster TGI。
        let source_raster = dbpf::Package::open(&raster_path).unwrap();
        let property_package = dbpf::Package::open(&property_path).unwrap();
        let packages = vec![(7u64, source_raster), (3u64, property_package)];
        let (package_id, resolved) =
            locate_raster_for_mask(&packages, 7, &mask).expect("resolved in source package");
        assert_eq!(package_id, 7);
        assert_eq!(resolved.type_id, RASTER_TYPE_ID);
        assert_eq!(resolved.group, 0x1a2b_0000);
        assert_eq!(resolved.instance, 0x00c9_a566);

        // mask 在其他包：跨包兜底命中。
        let cross_raster = dbpf::Package::open(&raster_path).unwrap();
        let property_package = dbpf::Package::open(&property_path).unwrap();
        let packages = vec![(3u64, property_package), (7u64, cross_raster)];
        let (package_id, _) =
            locate_raster_for_mask(&packages, 3, &mask).expect("resolved across packages");
        assert_eq!(package_id, 7);

        // 所有包都没有 raster：None（真实缺失，而非匹配语义误报）。
        let property_package = dbpf::Package::open(&property_path).unwrap();
        let packages = vec![(3u64, property_package)];
        assert!(locate_raster_for_mask(&packages, 3, &mask).is_none());
    }
}
