//! Raster 绘制器的读写命令：
//! - `read_image_rgba`：导入外部 PNG/JPG → RGBA（透明背景进画布）；
//! - `read_raster_rgba`：包内 raster（pixFmt 21）解码 → RGBA 画布；
//! - `save_raster_overlay`：画布 RGBA → raster 字节（pixFmt21 + mip 链）→
//!   写出 overlay package（副本保存，源包不被触碰）。
//!
//! 编码/解码本体在 `rw4::raster`；本模块只做命令编排与上限校验。

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine;
use dbpf::ResourceId;
use rw4::RasterImage;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::package_service::{write_export, TgiDto};

const RASTER_TYPE_ID: u32 = 0x2F4E_681C;
/// 导入图片的像素上限（4096²）：与 RESOURCE_DATA_MAX 同数量级的防失控闸。
const IMPORT_IMAGE_MAX_PIXELS: usize = 4096 * 4096;

#[derive(Debug, thiserror::Error)]
enum RasterEditError {
    #[error("image path is empty")]
    InvalidPath,
    #[error("image exceeds the {0}px pixel budget")]
    TooLarge(usize),
    #[error("raster image is not writable (compressed pixel format)")]
    NotWritable,
    #[error("output path already exists")]
    OutputExists(String),
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
            Self::TooLarge(_) => "limit_exceeded",
            Self::NotWritable => "not_writable",
            Self::OutputExists(_) => "output_exists",
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
        let entry = package
            .entry(tgi)
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
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRasterOverlayResult {
    pub output_path: String,
    pub tgi: TgiDto,
    pub raster_bytes: u64,
    pub mip_count: u32,
}

#[tauri::command]
pub async fn save_raster_overlay(
    request: SaveRasterOverlayRequest,
) -> Result<SaveRasterOverlayResult, CommandError> {
    use base64::engine::general_purpose::STANDARD;

    tauri::async_runtime::spawn_blocking(move || {
        if request.tgi.type_id != RASTER_TYPE_ID {
            return Err(RasterEditError::InvalidPath);
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

        let overlay = dbpf::write_uncompressed_overlay(&[dbpf::OverlayEntry::new(
            ResourceId {
                type_id: RASTER_TYPE_ID,
                group: request.tgi.group,
                instance: request.tgi.instance,
            },
            raster_bytes.clone(),
        )])
        .map_err(|_| RasterEditError::InvalidPath)?;

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
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
    .map_err(CommandError::from)
}
