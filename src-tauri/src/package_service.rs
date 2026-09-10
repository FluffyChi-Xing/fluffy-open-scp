use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

use dbpf::{IndexEntry, Package, ResourceId};
use sc_store::{EventInput, PackageInput};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::activity::{ACTIVITY_EVENT, AppState, CommandError};
use crate::media_tools::{self, MediaTools, ToolError};

pub const RESOURCE_PAGE_DEFAULT_LIMIT: usize = 100;
pub const RESOURCE_PAGE_MAX_LIMIT: usize = 1_000;
pub const RESOURCE_BYTES_MAX: usize = 4 * 1024;
pub const RESOURCE_DATA_MAX: u64 = 32 * 1024 * 1024;
pub const RESOURCE_DECOMPRESSED_MAX: u64 = 256 * 1024 * 1024;
pub const PROPERTY_PATCH_MAX: usize = 256;
pub const PROPERTY_OVERLAY_MAX: u64 = 64 * 1024 * 1024;
const PROPERTY_RESOURCE_TYPE: u32 = sc_properties::PROPERTY_RESOURCE_TYPE;
pub const EXPORT_PROGRESS_EVENT: &str = "export:progress";
pub const MAX_OPEN_PACKAGES: usize = 32;
pub const MAX_EXPORT_JOBS: usize = 128;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct RegistryKey {
    main: PathBuf,
    user: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportStatus {
    pub job_id: u64,
    pub phase: String,
    pub output_path: String,
    pub error: Option<String>,
}

#[derive(Debug)]
pub struct PackageManager {
    next_package_id: AtomicU64,
    next_job_id: AtomicU64,
    packages: Mutex<HashMap<u64, Arc<Package>>>,
    registries: Mutex<HashMap<RegistryKey, Arc<sc_registry::Registry>>>,
    jobs: Mutex<HashMap<u64, ExportStatus>>,
}

impl PackageManager {
    pub fn new() -> Self {
        Self {
            next_package_id: AtomicU64::new(1),
            next_job_id: AtomicU64::new(1),
            packages: Mutex::new(HashMap::new()),
            registries: Mutex::new(HashMap::new()),
            jobs: Mutex::new(HashMap::new()),
        }
    }

    fn insert(&self, package: Package) -> Result<(u64, Arc<Package>), PackageError> {
        let mut packages = self
            .packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?;
        if packages.len() >= MAX_OPEN_PACKAGES {
            return Err(PackageError::PackageLimitExceeded(MAX_OPEN_PACKAGES));
        }
        let id = self.next_package_id.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            return Err(PackageError::InvalidArgument(
                "package id space exhausted".into(),
            ));
        }
        let package = Arc::new(package);
        packages.insert(id, Arc::clone(&package));
        Ok((id, package))
    }

    fn close(&self, id: u64) -> Result<(), PackageError> {
        self.packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .remove(&id)
            .map(|_| ())
            .ok_or(PackageError::PackageNotFound(id))
    }

    pub(crate) fn get(&self, id: u64) -> Result<Arc<Package>, PackageError> {
        self.packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .get(&id)
            .cloned()
            .ok_or(PackageError::PackageNotFound(id))
    }

    /// 所有已打开包的快照（跨包资源查找用，如 LotMask）。
    fn all_packages(&self) -> Result<Vec<Arc<Package>>, PackageError> {
        Ok(self
            .packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .values()
            .cloned()
            .collect())
    }

    /// 所有已打开包的 (id, package) 快照（需要回报资源所在包 id 时使用）。
    fn all_packages_with_ids(&self) -> Result<Vec<(u64, Arc<Package>)>, PackageError> {
        Ok(self
            .packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .iter()
            .map(|(id, package)| (*id, Arc::clone(package)))
            .collect())
    }

    fn next_job_id(&self) -> Result<u64, PackageError> {
        let id = self.next_job_id.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            return Err(PackageError::InvalidArgument(
                "export job id space exhausted".into(),
            ));
        }
        Ok(id)
    }

    fn reserve_job(&self, output_path: String) -> Result<u64, PackageError> {
        let id = self.next_job_id()?;
        let mut jobs = self.jobs.lock().map_err(|_| PackageError::StatePoisoned)?;
        if jobs.len() >= MAX_EXPORT_JOBS {
            return Err(PackageError::JobLimitExceeded(MAX_EXPORT_JOBS));
        }
        jobs.insert(
            id,
            ExportStatus {
                job_id: id,
                phase: "queued".into(),
                output_path,
                error: None,
            },
        );
        Ok(id)
    }

    fn update_job(&self, id: u64, phase: &str, error: Option<String>) -> Result<(), PackageError> {
        let mut jobs = self.jobs.lock().map_err(|_| PackageError::StatePoisoned)?;
        let job = jobs.get_mut(&id).ok_or(PackageError::JobNotFound(id))?;
        job.phase = phase.into();
        job.error = error;
        Ok(())
    }

    fn job(&self, id: u64) -> Result<ExportStatus, PackageError> {
        self.jobs
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .get(&id)
            .cloned()
            .ok_or(PackageError::JobNotFound(id))
    }

    fn registry(
        &self,
        main_path: &str,
        user_path: Option<&str>,
    ) -> Result<Arc<sc_registry::Registry>, PackageError> {
        let key = RegistryKey {
            main: fs::canonicalize(main_path)?,
            user: user_path.map(fs::canonicalize).transpose()?,
        };
        let mut registries = self
            .registries
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?;
        if let Some(registry) = registries.get(&key) {
            return Ok(Arc::clone(registry));
        }
        let registry = match &key.user {
            Some(user) => sc_registry::Registry::open_with_user(&key.main, user),
            None => sc_registry::Registry::open(&key.main),
        }?;
        let registry = Arc::new(registry);
        registries.insert(key, Arc::clone(&registry));
        Ok(registry)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TgiDto {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

impl From<ResourceId> for TgiDto {
    fn from(id: ResourceId) -> Self {
        Self {
            type_id: id.type_id,
            group: id.group,
            instance: id.instance,
        }
    }
}

impl From<TgiDto> for ResourceId {
    fn from(id: TgiDto) -> Self {
        Self {
            type_id: id.type_id,
            group: id.group,
            instance: id.instance,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSummary {
    pub package_id: u64,
    pub path: String,
    pub size: u64,
    pub kind: String,
    pub major_version: i32,
    pub minor_version: i32,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub tgi: TgiDto,
    pub offset: u64,
    pub stored_size: u64,
    pub decompressed_size: u64,
    pub compressed: bool,
}

impl From<&IndexEntry> for ResourceSummary {
    fn from(entry: &IndexEntry) -> Self {
        Self {
            tgi: entry.id.into(),
            offset: entry.offset,
            stored_size: entry.stored_len(),
            decompressed_size: u64::from(entry.decompressed_size),
            compressed: entry.compressed,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeCount {
    pub type_id: u32,
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePage {
    pub items: Vec<ResourceSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub type_counts: Vec<TypeCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPackageResponse {
    pub package: PackageSummary,
    pub resources: ResourcePage,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPackageRequest {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesRequest {
    pub package_id: u64,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub filter: Option<String>,
    pub type_id: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceBytesRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
    pub offset: u64,
    pub length: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadResourceDataRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceData {
    pub total_length: u64,
    pub data_base64: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
    pub patches: Vec<PropertyPatch>,
    pub output_path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum PropertyPatchValue {
    Bool(bool),
    Int32(i32),
    UInt32(u32),
    Float(f32),
    String8(String),
    String16(String),
    Key {
        instance: u32,
        type_id: u32,
        group: u32,
    },
    Text {
        table_id: u32,
        instance_id: u32,
    },
    Vector2([f32; 2]),
    Vector3([f32; 3]),
    ColorRgb {
        r: f32,
        g: f32,
        b: f32,
    },
    Vector4([f32; 4]),
    ColorRgba {
        r: f32,
        g: f32,
        b: f32,
        a: f32,
    },
    Transform(sc_properties::Transform),
    BoundingBox {
        min: [f32; 3],
        max: [f32; 3],
    },
    Array(Vec<PropertyPatchValue>),
    Empty,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatch {
    pub hash: u32,
    pub value: PropertyPatchValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPatchResult {
    pub output_path: String,
    pub tgi: TgiDto,
    pub changed_hashes: Vec<u32>,
    pub property_bytes: u64,
    pub overlay_bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LotEditorSessionRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterPreviewRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RasterPreviewData {
    pub raster_type: u32,
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub pixel_size: u32,
    pub pixel_format: u32,
    /// pixFmt 21（D3DFMT_A8R8G8B8，未压缩）可解码为 PNG；压缩变体仅元数据。
    pub decodable: bool,
    pub png_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LotEditorSession {
    pub tgi: TgiDto,
    pub asset_name: Option<String>,
    pub document: sc_properties::LotEditorDocument,
    pub model_available: bool,
    /// LOD1 模型 TGI（camelCase 拷贝，前端无需触碰 document 内部结构）。
    pub model_key: Option<TgiDto>,
    /// LOD1~LOD4 模型资源位置（跨包解析；None = 该级缺失）。
    pub model_lods: Vec<Option<LodModelRef>>,
    /// Lot 地面尺寸（LotSize 0x0CCB7FC8，camelize 拷贝）。
    pub lot_size: Option<[f32; 2]>,
    /// LotPlacementTransform（0x0DB7FB17）行主序 12 floats；地面矩形需取其逆。
    pub lot_placement: Option<[f32; 12]>,
    /// LotMask 四色量化地面图 PNG（LotColor1-4 着色，服务端解码）。
    pub lot_mask_png: Option<String>,
    /// LotColor1-4 的 RGBA（A = 地面贴图索引 0-15，SCP GroundTextures 图集）。
    pub lot_colors: [[u8; 4]; 4],
    /// LotColor1-4 是否实际存在于 property（false = 黑/红/绿/蓝回退，
    /// 精细渲染不应使用回退色着色）。
    pub lot_colors_authored: [bool; 4],
    /// 由属性字典装配的 Unit 列表（灯光/效果/贴花/道具槽/路径点/生成器）。
    pub units: Vec<sc_properties::LotUnit>,
    /// `0x0CAA6841` 的 Int32 对（路径点区间）。
    pub path_pairs: Vec<i32>,
    pub diagnostics: Vec<String>,
}

/// 单级 LOD 模型的资源位置（跨包解析结果）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LodModelRef {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceBytes {
    pub package_id: u64,
    pub tgi: TgiDto,
    pub offset: u64,
    pub total_length: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveNameRequest {
    pub tgi: TgiDto,
    pub main_registry_path: String,
    pub user_registry_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveNamesRequest {
    pub package_id: u64,
    pub tgis: Vec<TgiDto>,
}

/// PE 精细渲染二进制容器魔数："LOTM"（小端字节序）。
///
/// v4 布局：`magic | version | mesh_count`，每 mesh `u32 len + GLB`
/// （COLOR_0 调色板烘焙 + TEXCOORD_1.x=materialIndex/255）；`material_count`，
/// 每材质 6 张 `u32 len + PNG`（base/normal/rough/ao/tintRaw/palette）+
/// `u32 params_len + f32[] + u32 param_cols`（slot0 参数表）；每 mesh
/// `u32 material_index + u8 uv_kind(0无/1贴图/2tint)`。解析见 `three-gltf.ts`。
pub const LOT_MODEL_PAYLOAD_MAGIC: u32 = 0x4D54_4F4C;

/// 单个网格 GLB 的体积上限（异常模型防御，对齐原 OBJ 8MB 量级）。
const MESH_GLB_MAX_BYTES: usize = 8 * 1024 * 1024;

/// 文本预览全量字节上限（虚拟滚动渲染下超大文本的内存护栏）。
const TEXT_PREVIEW_MAX_BYTES: usize = 8 * 1024 * 1024;

/// 文本预览：全量原始字节（`tauri::ipc::Response` 通道，避免 JSON 数字数组
/// 膨胀）；由前端做字符集判定（严格 UTF-8 → BOM → 宽松解码 + 二进制段标记）
/// 与虚拟滚动渲染。超出上限截断，`totalLength` 由前端对比得出。
#[tauri::command]
pub async fn read_resource_text(
    state: State<'_, AppState>,
    request: ReadResourceDataRequest,
) -> Result<tauri::ipc::Response, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let bytes = read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, _package, _manager, _store| {
            let take = data.len().min(TEXT_PREVIEW_MAX_BYTES);
            Ok(data[..take].to_vec())
        },
    )
    .await?;
    Ok(tauri::ipc::Response::new(bytes))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LotModelMeshesRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedResourceName {
    pub tgi: TgiDto,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NameResolution {
    pub tgi: TgiDto,
    pub type_name: String,
    pub group_name: String,
    pub instance_name: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Raw,
    Obj,
    Glb,
    Png,
    Jpg,
    Gif,
    Tga,
    Dds,
    Wav,
    Mp3,
    Ogg,
    Flac,
    Vp6,
    Mp4,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
    pub format: ExportFormat,
    pub output_path: String,
    pub media_id: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportAccepted {
    pub job_id: u64,
    pub output_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProgress {
    pub job_id: u64,
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub tgi: Option<TgiDto>,
    pub output_path: String,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PackageError {
    #[error("package {0} was not found")]
    PackageNotFound(u64),
    #[error("resource {0} was not found in package")]
    ResourceNotFound(ResourceId),
    #[error("too many open packages (limit {0})")]
    PackageLimitExceeded(usize),
    #[error("export job {0} was not found")]
    JobNotFound(u64),
    #[error("too many export jobs (limit {0})")]
    JobLimitExceeded(usize),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("resource byte range is invalid")]
    InvalidRange,
    #[error("resource range exceeds the {0} byte limit")]
    LimitExceeded(usize),
    #[error("resource declares more than the {0} byte decompression limit")]
    DecompressionLimitExceeded(u64),
    #[error("generated export exceeds the {0} byte limit")]
    OutputLimitExceeded(u64),
    #[error("media tool was not found: {0}")]
    ToolNotFound(&'static str),
    #[error("media tool failed to start: {0}")]
    ToolSpawn(#[from] ToolError),
    #[error("media tool output is invalid: {0}")]
    InvalidMediaOutput(&'static str),
    #[error("output path already exists: {0}")]
    OutputExists(String),
    #[error("package state is unavailable")]
    StatePoisoned,
    #[error("dbpf error: {0}")]
    Dbpf(#[from] dbpf::Error),
    #[error("dbpf writer error: {0}")]
    Overlay(#[from] dbpf::WriterError),
    #[error("properties error: {0}")]
    Properties(#[from] sc_properties::Error),
    #[error("Wwise SoundBank error: {0}")]
    Wwise(#[from] crate::wwise::WwiseError),
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("registry error: {0}")]
    Registry(#[from] sc_registry::Error),
    #[error("rw4 error: {0}")]
    Rw4(#[from] rw4::Error),
    #[error("export format is not supported for this resource")]
    UnsupportedExport,
    #[error("exporter error: {0}")]
    Exporter(#[from] sc_exporter::Error),
    #[error("export io error: {0}")]
    Io(#[from] std::io::Error),
}

impl PackageError {
    fn code(&self) -> &'static str {
        match self {
            Self::PackageNotFound(_) | Self::ResourceNotFound(_) | Self::JobNotFound(_) => {
                "not_found"
            }
            Self::PackageLimitExceeded(_) | Self::JobLimitExceeded(_) => "limit_exceeded",
            Self::InvalidArgument(_) => "invalid_argument",
            Self::InvalidRange => "invalid_range",
            Self::LimitExceeded(_)
            | Self::DecompressionLimitExceeded(_)
            | Self::OutputLimitExceeded(_) => "limit_exceeded",
            Self::OutputExists(_) => "output_exists",
            Self::StatePoisoned => "internal_error",
            Self::Dbpf(_) => "corrupt_package",
            Self::Overlay(_) => "overlay_write_failed",
            Self::Properties(_) => "corrupt_resource",
            Self::Wwise(_) => "corrupt_resource",
            Self::Image(_) => "corrupt_resource",
            Self::Registry(_) => "registry",
            Self::Rw4(_) => "corrupt_resource",
            Self::UnsupportedExport => "unsupported",
            Self::Exporter(_) => "export_failed",
            Self::ToolNotFound(_) => "tool_not_found",
            Self::ToolSpawn(ToolError::Spawn { .. }) => "tool_spawn_failed",
            Self::ToolSpawn(ToolError::Failed { .. }) => "tool_failed",
            Self::ToolSpawn(ToolError::Timeout { .. }) => "tool_timeout",
            Self::ToolSpawn(ToolError::Wait { .. }) => "tool_failed",
            Self::InvalidMediaOutput(_) => "tool_output_invalid",
            Self::Io(_) => "io",
        }
    }
}

impl From<PackageError> for CommandError {
    fn from(error: PackageError) -> Self {
        CommandError::new(error.code(), error.to_string())
    }
}

fn page_limit(limit: Option<usize>) -> Result<usize, PackageError> {
    let limit = limit.unwrap_or(RESOURCE_PAGE_DEFAULT_LIMIT);
    if limit == 0 {
        return Err(PackageError::InvalidArgument(
            "limit must be greater than zero".into(),
        ));
    }
    if limit > RESOURCE_PAGE_MAX_LIMIT {
        return Err(PackageError::LimitExceeded(RESOURCE_PAGE_MAX_LIMIT));
    }
    Ok(limit)
}

fn package_summary(package_id: u64, package: &Package) -> Result<PackageSummary, PackageError> {
    let metadata = fs::metadata(package.path())?;
    let header = package.header();
    Ok(PackageSummary {
        package_id,
        path: package.path().to_string_lossy().into_owned(),
        size: metadata.len(),
        kind: format!("{:?}", header.kind),
        major_version: header.major_version,
        minor_version: header.minor_version,
        entry_count: package.entries().len(),
    })
}

fn resource_matches(entry: &IndexEntry, filter: &str) -> bool {
    let id = entry.id;
    let text = format!("{:08x}:{:08x}:{:08x}", id.type_id, id.group, id.instance);
    if text.contains(filter) {
        return true;
    }
    if id.type_id.to_string() == filter
        || id.group.to_string() == filter
        || id.instance.to_string() == filter
    {
        return true;
    }
    // TGI 搜索：接受 0x 前缀与任意分段（"0x2f4e681b"、"2f4e681b"、
    // "0x2f4e681b:0:1a2b"、十进制）；分段 hex 需匹配对应组件。
    let stripped = filter.trim_start_matches("0x");
    let parts: Vec<&str> = stripped.split(':').collect();
    let parse_part = |part: &str| u32::from_str_radix(part.trim_start_matches("0x"), 16).ok();
    match parts.as_slice() {
        [single] => {
            // 单值匹配任意分量（type / group / instance）
            parse_part(single).is_some_and(|v| v == id.type_id || v == id.group || v == id.instance)
        }
        [type_part, group_part] => {
            let (t, g) = (parse_part(type_part), parse_part(group_part));
            // 两段可解读为 (type,group) 或 (group,instance)
            (t.is_some_and(|v| v == id.type_id) && g.is_some_and(|v| v == id.group))
                || (t.is_some_and(|v| v == id.group) && g.is_some_and(|v| v == id.instance))
        }
        [type_part, group_part, instance_part] => {
            parse_part(type_part).is_some_and(|v| v == id.type_id)
                && parse_part(group_part).is_some_and(|v| v == id.group)
                && parse_part(instance_part).is_some_and(|v| v == id.instance)
        }
        _ => false,
    }
}

fn normalized_filter(filter: Option<&str>) -> Result<Option<String>, PackageError> {
    let Some(filter) = filter else {
        return Ok(None);
    };
    let filter = filter.trim().to_ascii_lowercase();
    if filter.len() > 128 {
        return Err(PackageError::LimitExceeded(128));
    }
    Ok(Some(filter))
}

fn resource_page(
    package: &Package,
    offset: usize,
    limit: usize,
    filter: Option<&str>,
    type_filter: Option<u32>,
    type_counts: Vec<TypeCount>,
) -> Result<ResourcePage, PackageError> {
    let filter = normalized_filter(filter)?;
    let mut total = 0usize;
    let mut items = Vec::with_capacity(limit);
    for entry in package.entries() {
        if type_filter.is_some_and(|type_id| entry.id.type_id != type_id) {
            continue;
        }
        if filter
            .as_deref()
            .is_some_and(|filter| !resource_matches(entry, filter))
        {
            continue;
        }
        if total >= offset && items.len() < limit {
            items.push(ResourceSummary::from(entry));
        }
        total = total
            .checked_add(1)
            .ok_or(PackageError::LimitExceeded(usize::MAX))?;
    }
    Ok(ResourcePage {
        items,
        total,
        offset,
        limit,
        type_counts,
    })
}

fn type_counts(package: &Package, registry: Option<&sc_registry::Registry>) -> Vec<TypeCount> {
    let mut counts = HashMap::new();
    for entry in package.entries() {
        *counts.entry(entry.id.type_id).or_insert(0usize) += 1;
    }
    let mut counts: Vec<(u32, usize)> = counts.into_iter().collect();
    counts.sort_by_key(|(type_id, _)| *type_id);
    counts
        .into_iter()
        .map(|(type_id, count)| TypeCount {
            type_id,
            name: registry
                .map(|registry| registry.type_name(type_id))
                .unwrap_or_else(|| format!("{type_id:08X}")),
            count,
        })
        .collect()
}

fn emit_event(
    app: &AppHandle,
    store: &sc_store::Store,
    operation_id: Option<i64>,
    level: &str,
    topic: &str,
    message: &str,
    payload: serde_json::Value,
) {
    let input = EventInput {
        operation_id,
        level: level.into(),
        topic: topic.into(),
        message: message.into(),
        payload: Some(payload),
    };
    if let Ok(event) = store.append_event(&input) {
        let _ = app.emit(ACTIVITY_EVENT, &event);
    }
}

fn finish_operation(
    store: &sc_store::Store,
    operation_id: i64,
    status: &str,
    started: Instant,
    bytes_in: Option<i64>,
    bytes_out: Option<i64>,
    detail: Option<&serde_json::Value>,
) {
    let _ = store.finish_operation(
        operation_id,
        status,
        started.elapsed().as_millis() as i64,
        bytes_in,
        bytes_out,
        detail,
    );
}

fn read_resource(
    package: &Package,
    tgi: ResourceId,
    offset: u64,
    length: usize,
) -> Result<ResourceBytes, PackageError> {
    if length > RESOURCE_BYTES_MAX {
        return Err(PackageError::LimitExceeded(RESOURCE_BYTES_MAX));
    }
    let entry = package
        .entry(tgi)
        .ok_or(PackageError::ResourceNotFound(tgi))?;
    let total_length = if entry.compressed {
        u64::from(entry.decompressed_size)
    } else {
        entry.stored_len()
    };
    let end = offset
        .checked_add(length as u64)
        .ok_or(PackageError::InvalidRange)?;
    if end > total_length {
        return Err(PackageError::InvalidRange);
    }
    let bytes = if entry.compressed {
        if total_length > RESOURCE_DECOMPRESSED_MAX {
            return Err(PackageError::DecompressionLimitExceeded(
                RESOURCE_DECOMPRESSED_MAX,
            ));
        }
        package.read(entry)?[offset as usize..end as usize].to_vec()
    } else {
        package.read_raw(entry)?[offset as usize..end as usize].to_vec()
    };
    Ok(ResourceBytes {
        package_id: 0,
        tgi: tgi.into(),
        offset,
        total_length,
        bytes,
    })
}

#[tauri::command]
pub async fn open_package(
    state: State<'_, AppState>,
    request: OpenPackageRequest,
) -> Result<OpenPackageResponse, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let app = state.app.clone();
    let bundled_registry = bundled_registry_path(&state.app);
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let operation_id = store
            .start_operation("package_open", &request.path, None)
            .map_err(CommandError::from)?;
        let path = PathBuf::from(&request.path);
        let result = (|| -> Result<OpenPackageResponse, PackageError> {
            let canonical = fs::canonicalize(path)?;
            let package = Package::open(&canonical)?;
            let mut summary = package_summary(0, &package)?;
            let counts = type_counts(
                &package,
                package_registry(&store, &manager, &package, bundled_registry.as_deref())
                    .as_deref(),
            );
            let page = resource_page(&package, 0, RESOURCE_PAGE_DEFAULT_LIMIT, None, None, counts)?;
            let (package_id, _) = manager.insert(package)?;
            summary.package_id = package_id;
            let _ = store.record_package_open(&PackageInput {
                path: summary.path.clone(),
                size: i64::try_from(summary.size).map_err(|_| {
                    PackageError::InvalidArgument("package size exceeds storage range".into())
                })?,
                entry_count: i64::try_from(summary.entry_count).map_err(|_| {
                    PackageError::InvalidArgument("entry count exceeds storage range".into())
                })?,
                version: i64::from(summary.major_version),
            });
            Ok(OpenPackageResponse {
                package: summary,
                resources: page,
            })
        })();
        match result {
            Ok(response) => {
                finish_operation(&store, operation_id, "success", started, None, None, None);
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "info",
                    "package",
                    "Package opened",
                    json!({"packageId": response.package.package_id}),
                );
                Ok(response)
            }
            Err(error) => {
                finish_operation(
                    &store,
                    operation_id,
                    "failed",
                    started,
                    None,
                    None,
                    Some(&json!({"error": error.to_string()})),
                );
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "error",
                    "package",
                    "Package open failed",
                    json!({"error": error.to_string()}),
                );
                Err(CommandError::from(error))
            }
        }
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn close_package(
    state: State<'_, AppState>,
    package_id: u64,
) -> Result<(), CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let app = state.app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let operation_id = store
            .start_operation("package_close", &package_id.to_string(), None)
            .map_err(CommandError::from)?;
        match manager.close(package_id) {
            Ok(()) => {
                finish_operation(&store, operation_id, "success", started, None, None, None);
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "info",
                    "package",
                    "Package closed",
                    json!({"packageId": package_id}),
                );
                Ok(())
            }
            Err(error) => {
                finish_operation(
                    &store,
                    operation_id,
                    "failed",
                    started,
                    None,
                    None,
                    Some(&json!({"error": error.to_string()})),
                );
                Err(CommandError::from(error))
            }
        }
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn export_status(
    state: State<'_, AppState>,
    job_id: u64,
) -> Result<ExportStatus, CommandError> {
    state.packages.job(job_id).map_err(CommandError::from)
}
#[tauri::command]
pub async fn list_resources(
    state: State<'_, AppState>,
    request: ListResourcesRequest,
) -> Result<ResourcePage, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let app = state.app.clone();
    let bundled_registry = bundled_registry_path(&state.app);
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let operation_id = store
            .start_operation("package_scan", &request.package_id.to_string(), None)
            .map_err(CommandError::from)?;
        let result = (|| -> Result<ResourcePage, PackageError> {
            let limit = page_limit(request.limit)?;
            let package = manager.get(request.package_id)?;
            let counts = type_counts(
                &package,
                package_registry(&store, &manager, &package, bundled_registry.as_deref())
                    .as_deref(),
            );
            resource_page(
                &package,
                request.offset.unwrap_or(0),
                limit,
                request.filter.as_deref(),
                request.type_id,
                counts,
            )
        })();
        match result {
            Ok(page) => {
                finish_operation(&store, operation_id, "success", started, None, None, None);
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "info",
                    "package",
                    "Resources listed",
                    json!({"packageId": request.package_id, "count": page.items.len()}),
                );
                Ok(page)
            }
            Err(error) => {
                finish_operation(
                    &store,
                    operation_id,
                    "failed",
                    started,
                    None,
                    None,
                    Some(&json!({"error": error.to_string()})),
                );
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "error",
                    "package",
                    "Resource listing failed",
                    json!({"error": error.to_string()}),
                );
                Err(CommandError::from(error))
            }
        }
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn read_resource_bytes(
    state: State<'_, AppState>,
    request: ReadResourceBytesRequest,
) -> Result<ResourceBytes, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let app = state.app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let tgi: ResourceId = request.tgi.clone().into();
        let started = Instant::now();
        let operation_id = store
            .start_operation(
                "resource_read",
                &format!("{}:{:?}", request.package_id, tgi),
                None,
            )
            .map_err(CommandError::from)?;
        let result = (|| -> Result<ResourceBytes, PackageError> {
            let package = manager.get(request.package_id)?;
            let entry = package
                .entry(tgi)
                .ok_or(PackageError::ResourceNotFound(tgi))?;
            let bytes_in = i64::try_from(entry.stored_len()).ok();
            let result = read_resource(&package, tgi, request.offset, request.length)?;
            let bytes_out = if entry.compressed {
                Some(i64::from(entry.decompressed_size))
            } else {
                i64::try_from(result.bytes.len()).ok()
            };
            let mut result = result;
            result.package_id = request.package_id;
            finish_operation(
                &store,
                operation_id,
                "success",
                started,
                bytes_in,
                bytes_out,
                None,
            );
            Ok(result)
        })();
        match result {
            Ok(result) => {
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "info",
                    "resource",
                    "Resource read",
                    json!({"packageId": result.package_id, "offset": result.offset, "length": result.bytes.len()}),
                );
                Ok(result)
            }
            Err(error) => {
                finish_operation(
                    &store,
                    operation_id,
                    "failed",
                    started,
                    None,
                    None,
                    Some(&json!({"error": error.to_string()})),
                );
                emit_event(
                    &app,
                    &store,
                    Some(operation_id),
                    "error",
                    "resource",
                    "Resource read failed",
                    json!({"error": error.to_string()}),
                );
                Err(CommandError::from(error))
            }
        }
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn read_resource_data(
    state: State<'_, AppState>,
    request: ReadResourceDataRequest,
) -> Result<ResourceData, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let tgi: ResourceId = request.tgi.into();
        let package = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let entry = package
            .entry(tgi)
            .ok_or_else(|| CommandError::from(PackageError::ResourceNotFound(tgi)))?;
        if u64::from(entry.decompressed_size) > RESOURCE_DATA_MAX {
            return Err(CommandError::from(PackageError::LimitExceeded(
                RESOURCE_DATA_MAX as usize,
            )));
        }
        let data = package.read(entry).map_err(PackageError::Dbpf)?;
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        Ok(ResourceData {
            total_length: u64::from(entry.decompressed_size),
            data_base64: STANDARD.encode(data),
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

const PROPERTY_PREVIEW_MAX_VALUES: usize = 512;
const RW4_HEX_DUMP_BYTES: usize = 128;
const MESH_OBJ_MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPreviewEntry {
    pub hash: u32,
    pub name: Option<String>,
    pub type_name: String,
    pub value: String,
    pub array_len: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PropertyPreviewData {
    pub claimed_count: u32,
    pub entries: Vec<PropertyPreviewEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4SectionDto {
    pub number: u32,
    pub type_code: u32,
    pub type_name: Option<String>,
    pub size: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4PreviewData {
    pub file_type: String,
    pub sections: Vec<Rw4SectionDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4MeshDetail {
    pub triangle_count: u32,
    pub vertex_count: u32,
    pub decoded_triangles: usize,
    pub decoded_vertices: usize,
    pub exportable: bool,
    pub bounds_min: Option<[f32; 3]>,
    pub bounds_max: Option<[f32; 3]>,
    pub obj_base64: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4TextureDetail {
    pub width: u16,
    pub height: u16,
    pub mip_count: u32,
    pub texture_type: u32,
    pub png_base64: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4SectionDetail {
    pub number: u32,
    pub type_code: u32,
    pub type_name: Option<String>,
    pub size: u32,
    pub pos: u32,
    pub mesh: Option<Rw4MeshDetail>,
    pub texture: Option<Rw4TextureDetail>,
    pub hex_dump: Option<String>,
}

fn property_value_text(kind: &sc_properties::Kind) -> (String, Option<usize>) {
    match kind {
        sc_properties::Kind::Scalar(value) => (value.to_string(), None),
        sc_properties::Kind::Array(values) => {
            let mut text = values
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            if let Some(cut) = text
                .char_indices()
                .nth(PROPERTY_PREVIEW_MAX_VALUES)
                .map(|(i, _)| i)
            {
                text.truncate(cut);
                text.push('…');
            }
            (text, Some(values.len()))
        }
        sc_properties::Kind::Empty => (String::new(), None),
    }
}

fn property_preview(
    data: &[u8],
    registry: Option<&Arc<sc_registry::Registry>>,
) -> Result<PropertyPreviewData, PackageError> {
    let file = sc_properties::PropertyFile::parse(data)?;
    let entries = file
        .values
        .iter()
        .map(|property| {
            let name = registry
                .and_then(|registry| registry.properties().get(&property.hash))
                .map(|record| record.name.clone())
                .filter(|name| !name.is_empty());
            let (value, array_len) = property_value_text(&property.kind);
            PropertyPreviewEntry {
                hash: property.hash,
                name,
                type_name: property.prop_type.name().to_string(),
                value,
                array_len,
            }
        })
        .collect();
    Ok(PropertyPreviewData {
        claimed_count: file.claimed_count,
        entries,
    })
}

fn rw4_sections(data: &[u8]) -> Result<Rw4PreviewData, PackageError> {
    let file = rw4::Rw4File::parse(data)?;
    let sections = file
        .sections()
        .iter()
        .map(|section| Rw4SectionDto {
            number: section.number,
            type_code: section.type_code,
            type_name: section.type_name().map(str::to_owned),
            size: section.size,
        })
        .collect();
    Ok(Rw4PreviewData {
        file_type: format!("{:?}", file.file_type()),
        sections,
    })
}

fn mesh_bounds(vertices: &[rw4::DecodedVertex]) -> (Option<[f32; 3]>, Option<[f32; 3]>) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for vertex in vertices {
        if let Some(position) = vertex.position() {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
    }
    let any = vertices.iter().any(|vertex| vertex.position().is_some());
    (any.then_some(min), any.then_some(max))
}

fn rw4_section_detail(data: &[u8], number: u32) -> Result<Rw4SectionDetail, PackageError> {
    let file = rw4::Rw4File::parse(data)?;
    let section = file.section(number).ok_or_else(|| {
        PackageError::Rw4(rw4::Error::SectionNumberOutOfRange {
            number,
            count: file.sections().len() as u32,
        })
    })?;
    let mut detail = Rw4SectionDetail {
        number: section.number,
        type_code: section.type_code,
        type_name: section.type_name().map(str::to_owned),
        size: section.size,
        pos: section.pos,
        mesh: None,
        texture: None,
        hex_dump: None,
    };
    if section.type_code == rw4::SectionType::MESH {
        let mesh = file.decode_mesh(data, number)?;
        let (bounds_min, bounds_max) = mesh_bounds(&mesh.vertices);
        let obj_text = sc_exporter::export_obj(&mesh);
        let obj_base64 = (obj_text.len() <= MESH_OBJ_MAX_BYTES).then(|| {
            use base64::{Engine as _, engine::general_purpose::STANDARD};
            STANDARD.encode(obj_text.as_bytes())
        });
        detail.mesh = Some(Rw4MeshDetail {
            triangle_count: mesh.header.triangle_count,
            vertex_count: mesh.header.vertex_count,
            decoded_triangles: mesh.triangles.len(),
            decoded_vertices: mesh.vertices.len(),
            exportable: mesh.is_exportable(),
            bounds_min,
            bounds_max,
            obj_base64,
        });
    } else if section.type_code == rw4::SectionType::TEXTURE {
        let texture = file.decode_texture(data, number)?;
        validate_texture_budget(&texture)?;
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let png = sc_exporter::export_texture(&texture, sc_exporter::TextureOutputFormat::Png)?;
        detail.texture = Some(Rw4TextureDetail {
            width: texture.width,
            height: texture.height,
            mip_count: texture.mip_count(),
            texture_type: texture.texture_type,
            png_base64: STANDARD.encode(png),
        });
    } else {
        let payload = file.payload(data, number)?;
        detail.hex_dump = Some(hex_dump(payload, RW4_HEX_DUMP_BYTES));
    }
    Ok(detail)
}

fn hex_dump(data: &[u8], max_bytes: usize) -> String {
    let mut out = String::new();
    let take = data.len().min(max_bytes);
    for (index, chunk) in data[..take].as_chunks::<16>().0.iter().enumerate() {
        out.push_str(&format!("{:08X}  ", index * 16));
        for byte in *chunk {
            out.push_str(&format!("{byte:02X} "));
        }
        for _ in chunk.len()..16 {
            out.push_str("   ");
        }
        out.push_str(" ");
        for byte in *chunk {
            out.push(if (32..=126).contains(&byte) {
                byte as char
            } else {
                '.'
            });
        }
        out.push('\n');
    }
    if data.len() > max_bytes {
        out.push_str("…\n");
    }
    out
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rw4SectionRequest {
    pub package_id: u64,
    pub tgi: TgiDto,
    pub number: u32,
}

async fn read_resource_with<T, F>(
    manager: Arc<PackageManager>,
    store: Arc<sc_store::Store>,
    package_id: u64,
    tgi: TgiDto,
    parse: F,
) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce(&[u8], &Package, &PackageManager, &sc_store::Store) -> Result<T, PackageError>
        + Send
        + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let tgi: ResourceId = tgi.into();
        let package = manager.get(package_id).map_err(CommandError::from)?;
        let entry = package
            .entry(tgi)
            .ok_or_else(|| CommandError::from(PackageError::ResourceNotFound(tgi)))?;
        if u64::from(entry.decompressed_size) > RESOURCE_DATA_MAX {
            return Err(CommandError::from(PackageError::LimitExceeded(
                RESOURCE_DATA_MAX as usize,
            )));
        }
        let data = package.read(entry).map_err(PackageError::Dbpf)?;
        parse(&data, &package, &manager, &store).map_err(CommandError::from)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn patch_value(
    expected: sc_properties::PropType,
    value: &PropertyPatchValue,
) -> Result<sc_properties::Value, PackageError> {
    use sc_properties::Value;
    let result = match (expected, value) {
        (sc_properties::PropType::Bool, PropertyPatchValue::Bool(value)) => Value::Bool(*value),
        (sc_properties::PropType::Int32, PropertyPatchValue::Int32(value)) => Value::Int32(*value),
        (sc_properties::PropType::UInt32, PropertyPatchValue::UInt32(value)) => {
            Value::UInt32(*value)
        }
        (sc_properties::PropType::Float, PropertyPatchValue::Float(value)) => Value::Float(*value),
        (sc_properties::PropType::String8, PropertyPatchValue::String8(value)) => {
            Value::String8(value.clone())
        }
        (sc_properties::PropType::String16, PropertyPatchValue::String16(value)) => {
            Value::String16(value.clone())
        }
        (
            sc_properties::PropType::Key,
            PropertyPatchValue::Key {
                instance,
                type_id,
                group,
            },
        ) => Value::Key(sc_properties::Key {
            instance: *instance,
            type_id: *type_id,
            group: *group,
        }),
        (
            sc_properties::PropType::Text,
            PropertyPatchValue::Text {
                table_id,
                instance_id,
            },
        ) => Value::Text(sc_properties::Text {
            table_id: *table_id,
            instance_id: *instance_id,
        }),
        (sc_properties::PropType::Vector2, PropertyPatchValue::Vector2(value)) => {
            Value::Vector2(*value)
        }
        (sc_properties::PropType::Vector3, PropertyPatchValue::Vector3(value)) => {
            Value::Vector3(*value)
        }
        (sc_properties::PropType::ColorRgb, PropertyPatchValue::ColorRgb { r, g, b }) => {
            Value::ColorRgb {
                r: *r,
                g: *g,
                b: *b,
            }
        }
        (sc_properties::PropType::Vector4, PropertyPatchValue::Vector4(value)) => {
            Value::Vector4(*value)
        }
        (sc_properties::PropType::ColorRgba, PropertyPatchValue::ColorRgba { r, g, b, a }) => {
            Value::ColorRgba {
                r: *r,
                g: *g,
                b: *b,
                a: *a,
            }
        }
        (sc_properties::PropType::Transform, PropertyPatchValue::Transform(value)) => {
            Value::Transform(value.clone())
        }
        (sc_properties::PropType::BoundingBox, PropertyPatchValue::BoundingBox { min, max }) => {
            Value::BoundingBox {
                min: *min,
                max: *max,
            }
        }
        _ => {
            return Err(PackageError::InvalidArgument(
                "property patch value type does not match the property".into(),
            ));
        }
    };
    Ok(result)
}

fn apply_property_patches(
    file: &mut sc_properties::PropertyFile,
    patches: &[PropertyPatch],
) -> Result<Vec<u32>, PackageError> {
    if patches.is_empty() || patches.len() > PROPERTY_PATCH_MAX {
        return Err(PackageError::LimitExceeded(PROPERTY_PATCH_MAX));
    }
    let mut changed = Vec::with_capacity(patches.len());
    for patch in patches {
        if changed.contains(&patch.hash) {
            return Err(PackageError::InvalidArgument(format!(
                "duplicate property patch hash {:#010x}",
                patch.hash
            )));
        }
        let property = file
            .values
            .iter_mut()
            .find(|property| property.hash == patch.hash)
            .ok_or_else(|| {
                PackageError::InvalidArgument(format!(
                    "property hash {:#010x} was not found",
                    patch.hash
                ))
            })?;
        match (&mut property.kind, &patch.value) {
            (sc_properties::Kind::Scalar(current), value) => {
                *current = patch_value(property.prop_type, value)?;
            }
            (sc_properties::Kind::Array(current), PropertyPatchValue::Array(values)) => {
                if values.len() != current.len() {
                    return Err(PackageError::InvalidArgument(format!(
                        "property array {:#010x} length cannot change",
                        patch.hash
                    )));
                }
                for (current, value) in current.iter_mut().zip(values) {
                    *current = patch_value(property.prop_type, value)?;
                }
            }
            (sc_properties::Kind::Empty, PropertyPatchValue::Empty) => {}
            _ => {
                return Err(PackageError::InvalidArgument(format!(
                    "property {:#010x} shape cannot change",
                    patch.hash
                )));
            }
        }
        changed.push(patch.hash);
    }
    Ok(changed)
}

#[tauri::command]
pub async fn read_lot_editor_session(
    state: State<'_, AppState>,
    request: LotEditorSessionRequest,
) -> Result<LotEditorSession, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let bundled_registry = bundled_registry_path(&state.app);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi.clone(),
        move |data, package, manager, store| {
            let tgi = request.tgi;
            if tgi.type_id != PROPERTY_RESOURCE_TYPE {
                return Err(PackageError::InvalidArgument(
                    "lot editor requires a property resource".into(),
                ));
            }
            let properties = sc_properties::PropertyFile::parse_with_limits(
                data,
                sc_properties::ParseLimits::default(),
            )?;
            let document = sc_properties::LotEditorDocument::from_property_file(properties);
            let lot_units = document.assemble_units();
            let mut diagnostics = lot_units.diagnostics;
            let (model_lods, lod_diagnostics) = resolve_lod_model_refs(
                package,
                request.package_id,
                manager,
                document.model_lods.clone(),
            );
            diagnostics.extend(lod_diagnostics);
            let model_available = model_lods.iter().any(|lod| lod.is_some());
            let model_key = model_lods
                .iter()
                .find_map(|lod| lod.as_ref().map(|reference| reference.tgi.clone()));
            let registry = package_registry(store, manager, package, bundled_registry.as_deref());
            let asset_name = semantic_instance_name(registry.as_deref(), tgi.instance);
            if registry.is_none() {
                diagnostics.push("property registry is unavailable; using hash identifiers".into());
            }
            let (colors, lot_colors_authored) = lot_colors(&document);
            let mut mask_dims: Option<(u32, u32)> = None;
            let lot_mask_png = document.lot_mask.and_then(|key| {
                match decode_lot_mask_png(package, manager, key, colors) {
                    Ok((png, dims)) => {
                        mask_dims = Some(dims);
                        Some(png)
                    }
                    Err(message) => {
                        diagnostics.push(message);
                        None
                    }
                }
            });
            // EP1 等部分 lot 无 LotSize（0x0CCB7FC8）属性但带 LotMask：地面
            // 矩形无法构建。回退：mask 光栅尺寸 × 0.75 m/px（主流换算，
            // 如 64px↔48m、128px↔96m；0x5A6EC675 无 LotSize + bbox 71×57
            // 与 128px→96×96 相容）。
            let lot_size = document.lot_size.or_else(|| {
                mask_dims.map(|(w, h)| {
                    let size = [w as f32 * 0.75, h as f32 * 0.75];
                    diagnostics.push(format!(
                        "LotSize property missing; ground rect derived from LotMask raster {}x{}px -> {:.0}x{:.0}m",
                        w, h, size[0], size[1]
                    ));
                    size
                })
            });
            Ok(LotEditorSession {
                tgi,
                asset_name,
                model_key,
                model_lods,
                lot_size,
                // C# CreateLotModel 只消费 12 floats 的完整矩阵（取逆贴地）
                lot_placement: document
                    .placement
                    .clone()
                    .filter(|t| t.matrix.len() == 12)
                    .map(|t| t.matrix.try_into().unwrap()),
                lot_mask_png,
                lot_colors: colors,
                lot_colors_authored,
                units: lot_units.units,
                path_pairs: lot_units.path_pairs,
                document,
                model_available,
                diagnostics,
            })
        },
    )
    .await
}

/// LotMask 地面图：定位 raster 资源 → 四层量化 → PNG（任何失败转为诊断消息）。
/// 查找顺序：当前包 → 所有已打开包（SCP 在全部已加载索引中查找，
/// LotMask 引用常指向 graphics 包，Key 的 type/group 多为 0）。
fn decode_lot_mask_png(
    current: &Package,
    manager: &PackageManager,
    key: sc_properties::Key,
    colors: [[u8; 4]; 4],
) -> Result<(String, (u32, u32)), String> {
    if let Some(entry_id) = find_raster_entry(current, key) {
        return decode_lot_mask_entry(current, &entry_id, colors);
    }
    if let Ok(packages) = manager.all_packages() {
        for package in &packages {
            if let Some(entry_id) = find_raster_entry(package, key) {
                return decode_lot_mask_entry(package, &entry_id, colors);
            }
        }
    }
    Err(format!(
        "LotMask raster 0x{:08X} is missing (it may live in a package that is not open)",
        key.instance
    ))
}

/// LOD1~4 模型跨包定位：精确 TGI（须为模型类型）→ 当前包按 instance 扫描
/// （忽略 group）→ 其它已打开包。返回各级位置与缺失诊断；LOD2-4 常与
/// property 不同包（EP1/graphics），故必须回报 package_id。
fn resolve_lod_model_refs(
    current: &Package,
    current_package_id: u64,
    manager: &PackageManager,
    keys: [Option<sc_properties::Key>; 4],
) -> (Vec<Option<LodModelRef>>, Vec<String>) {
    let mut refs = Vec::with_capacity(4);
    let mut diagnostics = Vec::new();
    for (index, key) in keys.into_iter().enumerate() {
        let Some(key) = key else {
            refs.push(None);
            continue;
        };
        let exact = ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        };
        let locate = |package: &Package| -> Option<ResourceId> {
            if let Some(entry) = package.entry(exact) {
                if entry.id.type_id == RW4_MODEL_TYPE {
                    return Some(entry.id);
                }
            }
            package
                .entries()
                .iter()
                .find(|entry| {
                    entry.id.type_id == RW4_MODEL_TYPE && entry.id.instance == key.instance
                })
                .map(|entry| entry.id)
        };
        let mut found: Option<(u64, ResourceId)> =
            locate(current).map(|tgi| (current_package_id, tgi));
        if found.is_none() {
            found = manager.all_packages_with_ids().ok().and_then(|packages| {
                packages
                    .iter()
                    .find_map(|(id, package)| locate(package).map(|tgi| (*id, tgi)))
            });
        }
        match found {
            Some((package_id, tgi)) => refs.push(Some(LodModelRef {
                package_id,
                tgi: TgiDto {
                    type_id: tgi.type_id,
                    group: tgi.group,
                    instance: tgi.instance,
                },
            })),
            None => {
                diagnostics.push(format!(
                    "LOD{} model resource is missing (it may live in a package that is not open)",
                    index + 1
                ));
                refs.push(None);
            }
        }
    }
    (refs, diagnostics)
}

/// SCP 定位语义：先精确 TGI（须为 raster 类型），再按 instance + raster
/// 类型扫描（忽略 group）。
fn find_raster_entry(package: &Package, key: sc_properties::Key) -> Option<ResourceId> {
    const RASTER_TYPE: u32 = 0x2F4E_681C;
    let exact = ResourceId {
        type_id: key.type_id,
        group: key.group,
        instance: key.instance,
    };
    if let Some(entry) = package.entry(exact) {
        if entry.id.type_id == RASTER_TYPE {
            return Some(exact);
        }
    }
    package
        .entries()
        .iter()
        .find(|entry| entry.id.type_id == RASTER_TYPE && entry.id.instance == key.instance)
        .map(|entry| entry.id)
}

fn decode_lot_mask_entry(
    package: &Package,
    entry_id: &ResourceId,
    colors: [[u8; 4]; 4],
) -> Result<(String, (u32, u32)), String> {
    let entry = package
        .entry(*entry_id)
        .ok_or_else(|| "LotMask raster resource is missing".to_string())?;
    if u64::from(entry.decompressed_size) > RESOURCE_DATA_MAX {
        return Err("LotMask raster exceeds the size limit".to_string());
    }
    let data = package
        .read(entry)
        .map_err(|error| format!("LotMask raster read failed: {error}"))?;
    let raster = rw4::RasterImage::parse(&data)
        .map_err(|error| format!("LotMask raster parse failed: {error}"))?;
    if !raster.is_raw_rgba() {
        return Err(format!(
            "LotMask raster uses compressed pixel format {}",
            raster.pixel_format
        ));
    }
    let rgba = raster
        .decode_lot_mask_rgba(&colors)
        .map_err(|error| error.to_string())?;
    let dims = (raster.width, raster.height);
    encode_rgba_png(raster.width, raster.height, rgba).map(|png| (png, dims))
}

/// LotColor1-4（0x0D02D586..89）RGBA；A = 地面贴图索引（引擎 16 格图集，
/// C# GroundTextureConverter 即此语义）。缺失用 SCP 的默认黑/红/绿/蓝。
fn lot_colors(document: &sc_properties::LotEditorDocument) -> ([[u8; 4]; 4], [bool; 4]) {
    const LOT_COLOR_HASHES: [u32; 4] = [0x0D02_D586, 0x0D02_D587, 0x0D02_D588, 0x0D02_D589];
    const FALLBACKS: [[u8; 4]; 4] = [[0, 0, 0, 0], [255, 0, 0, 0], [0, 255, 0, 0], [0, 0, 255, 0]];
    let mut colors = FALLBACKS;
    let mut authored = [false; 4];
    for (index, hash) in LOT_COLOR_HASHES.iter().enumerate() {
        if let Some(sc_properties::Value::ColorRgba { r, g, b, a }) = document
            .properties
            .get(*hash)
            .and_then(|property| property.scalar())
        {
            authored[index] = true;
            colors[index] = [
                (r * 255.0).clamp(0.0, 255.0) as u8,
                (g * 255.0).clamp(0.0, 255.0) as u8,
                (b * 255.0).clamp(0.0, 255.0) as u8,
                // ColorRgba 的 a 为 0-1 标量；贴图索引存整数格
                (a * 16.0).clamp(0.0, 15.0).round() as u8,
            ];
        }
    }
    (colors, authored)
}

fn encode_rgba_png_bytes(width: u32, height: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let image = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| "raster pixel buffer size mismatch".to_string())?;
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|error| format!("raster PNG encode failed: {error}"))?;
    Ok(png)
}

fn encode_rgba_png(width: u32, height: u32, rgba: Vec<u8>) -> Result<String, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    encode_rgba_png_bytes(width, height, rgba).map(|png| STANDARD.encode(png))
}

// ---- 通用图像预览：TGA / CUR(ICO) / Greyscale Map ----

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericImagePreviewData {
    pub image_kind: String,
    pub width: u32,
    pub height: u32,
    pub png_base64: String,
}

/// TGA（type 2/10，24/32bpp；10 为 RLE）。header 18B：
/// `[id_len, cmap_type, image_type, cmap_spec(5), x(2), y(2), w(2), h(2), bpp, desc]`。
fn decode_tga(data: &[u8]) -> Result<GenericImagePreviewData, String> {
    if data.len() < 18 {
        return Err("tga: truncated header".into());
    }
    let id_len = data[0] as usize;
    let cmap_type = data[1];
    let image_type = data[2];
    let width = u16::from_le_bytes([data[12], data[13]]) as u32;
    let height = u16::from_le_bytes([data[14], data[15]]) as u32;
    let bpp = data[16] as usize;
    let top_down = data[17] & 0x20 != 0;
    let rle = image_type == 10;
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(format!("tga: implausible dims {width}x{height}"));
    }
    if cmap_type != 0 || (image_type != 2 && image_type != 10) || (bpp != 24 && bpp != 32) {
        return Err(format!(
            "tga: unsupported type {image_type}/{cmap_type}/{bpp}bpp"
        ));
    }
    let bytes_pp = bpp / 8;
    let px_count = width as usize * height as usize;
    let mut bgr = vec![0u8; px_count * bytes_pp];
    let src = &data[18 + id_len..];
    if rle {
        let mut read = 0usize;
        let mut written = 0usize;
        while written < px_count {
            if read >= src.len() {
                return Err("tga: rle packet stream truncated".into());
            }
            let packet = src[read];
            read += 1;
            let count = (packet & 0x7f) as usize + 1;
            if written + count > px_count {
                return Err("tga: rle overflow".into());
            }
            if packet & 0x80 != 0 {
                if read + bytes_pp > src.len() {
                    return Err("tga: rle packet pixel truncated".into());
                }
                for i in 0..count {
                    bgr[(written + i) * bytes_pp..(written + i + 1) * bytes_pp]
                        .copy_from_slice(&src[read..read + bytes_pp]);
                }
                read += bytes_pp;
            } else {
                let take = count * bytes_pp;
                if read + take > src.len() {
                    return Err("tga: raw packet truncated".into());
                }
                bgr[written * bytes_pp..(written + count) * bytes_pp]
                    .copy_from_slice(&src[read..read + take]);
                read += take;
            }
            written += count;
        }
    } else {
        let take = px_count * bytes_pp;
        if src.len() < take {
            return Err("tga: pixel data truncated".into());
        }
        bgr.copy_from_slice(&src[..take]);
    }
    let mut rgba = Vec::with_capacity(px_count * 4);
    for row in 0..height as usize {
        // TGA 默认自底向上存储；top_down 位置位时才按原序。
        let src_row = if top_down {
            row
        } else {
            height as usize - 1 - row
        };
        for col in 0..width as usize {
            let at = (src_row * width as usize + col) * bytes_pp;
            rgba.extend_from_slice(&[
                bgr[at + 2],
                bgr[at + 1],
                bgr[at],
                bgr.get(at + 3).copied().unwrap_or(255),
            ]);
        }
    }
    Ok(GenericImagePreviewData {
        image_kind: "tga".into(),
        width,
        height,
        png_base64: encode_rgba_png(width, height, rgba)?,
    })
}

/// CUR/ICO 容器。取第一个目录项；32bpp 位为 BGRA（bottom-up），16bpp 为
/// RGB555，另带 1bpp AND mask（直接忽略，预览透明度用 alpha 通道）。
fn decode_cursor(data: &[u8]) -> Result<GenericImagePreviewData, String> {
    if data.len() < 6 || data[0] != 0 || data[1] != 0 {
        return Err("cursor: bad magic".into());
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    if count == 0 || data.len() < 6 + count * 16 {
        return Err("cursor: empty directory".into());
    }
    let entry = &data[6..22];
    let width = if entry[0] == 0 { 256 } else { entry[0] as u32 };
    let height = if entry[1] == 0 { 256 } else { entry[1] as u32 };
    let size = u32::from_le_bytes([entry[8], entry[9], entry[10], entry[11]]) as usize;
    let offset = u32::from_le_bytes([entry[12], entry[13], entry[14], entry[15]]) as usize;
    if offset + size > data.len() || width == 0 || height == 0 {
        return Err("cursor: entry out of range".into());
    }
    let bmp = &data[offset..offset + size];
    // PNG 内嵌变体（Vista+）
    if bmp.len() >= 8 && bmp[0] == 0x89 && &bmp[1..4] == b"PNG" {
        let png = encode_rgba_png(
            width,
            height,
            image::load_from_memory(bmp)
                .map_err(|e| format!("cursor: embedded png decode failed: {e}"))?
                .to_rgba8()
                .into_raw(),
        )?;
        return Ok(GenericImagePreviewData {
            image_kind: "cursor".into(),
            width,
            height,
            png_base64: png,
        });
    }
    if bmp.len() < 40 {
        return Err("cursor: missing bitmap header".into());
    }
    let bpp = u16::from_le_bytes([bmp[14], bmp[15]]) as usize;
    let stride = ((width as usize * bpp + 31) / 32) * 4;
    let xor_len = stride * height as usize;
    if bpp != 32 && bpp != 24 && bpp != 16 {
        return Err(format!("cursor: unsupported {bpp}bpp"));
    }
    if bmp.len() < 40 + xor_len {
        return Err("cursor: pixel data truncated".into());
    }
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height as usize {
        let src_row = height as usize - 1 - row;
        let line = &bmp[40 + src_row * stride..40 + (src_row + 1) * stride];
        for col in 0..width as usize {
            let at = col * (bpp / 8);
            let (r, g, b, a) = match bpp {
                32 => (line[at + 2], line[at + 1], line[at], line[at + 3]),
                24 => (line[at + 2], line[at + 1], line[at], 255),
                _ => {
                    let v = u16::from_le_bytes([line[at], line[at + 1]]);
                    (
                        (((v >> 10) & 0x1f) as u8) << 3,
                        (((v >> 5) & 0x1f) as u8) << 3,
                        ((v & 0x1f) as u8) << 3,
                        255,
                    )
                }
            };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    Ok(GenericImagePreviewData {
        image_kind: "cursor".into(),
        width,
        height,
        png_base64: encode_rgba_png(width, height, rgba)?,
    })
}

/// Greyscale Map：20 字节大端头 `[0, width, height, channel_code, byte_count]`，
/// code 1 = 单通道灰度、2 = RGBA；像素紧随（实测 64²/128²/256² 均吻合）。
fn decode_greyscale(data: &[u8]) -> Result<GenericImagePreviewData, String> {
    if data.len() < 20 {
        return Err("greyscale: truncated header".into());
    }
    let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let channel_code = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let declared = u32::from_be_bytes([data[16], data[17], data[18], data[19]]) as usize;
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(format!("greyscale: implausible dims {width}x{height}"));
    }
    let px_count = width as usize * height as usize;
    let body = &data[20..];
    let mut rgba = Vec::with_capacity(px_count * 4);
    match channel_code {
        1 => {
            if body.len() < px_count {
                return Err("greyscale: 8-bit data truncated".into());
            }
            for &g in &body[..px_count] {
                rgba.extend_from_slice(&[g, g, g, 255]);
            }
        }
        2 => {
            if body.len() < px_count * 4 {
                return Err("greyscale: rgba data truncated".into());
            }
            for px in body[..px_count * 4].chunks_exact(4) {
                rgba.extend_from_slice(&[px[0], px[1], px[2], px[3]]);
            }
        }
        code => return Err(format!("greyscale: unknown channel code {code}")),
    }
    let _ = declared;
    Ok(GenericImagePreviewData {
        image_kind: "greyscale".into(),
        width,
        height,
        png_base64: encode_rgba_png(width, height, rgba)?,
    })
}

#[tauri::command]
pub async fn read_image_preview(
    state: State<'_, AppState>,
    request: RasterPreviewRequest,
) -> Result<GenericImagePreviewData, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, _package, _manager, _store| {
            if data.len() >= 18 && data[0] == 0 && data[1] == 0 {
                if let Ok(cursor) = decode_cursor(data) {
                    return Ok(cursor);
                }
            }
            if data.len() >= 18 && data[1] == 0 && (data[2] == 2 || data[2] == 10) {
                if let Ok(tga) = decode_tga(data) {
                    return Ok(tga);
                }
            }
            decode_greyscale(data).map_err(PackageError::InvalidArgument)
        },
    )
    .await
}

// ---- PE 精细渲染：模型材质解析（migration.md §18.4） ----

const RW4_MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_IMAGE_TYPE: u32 = 0x2F4E_681C;

/// 跨包定位资源（当前包 → 所有已打开包，对齐 SCP"全部已加载索引"语义）。
/// 跨包查找资源，附来源包文件名（诊断信息用）。
fn find_resource_across_packages_named(
    current: &Package,
    manager: &PackageManager,
    instance: u32,
    type_ids: &[u32],
) -> Option<(Vec<u8>, u32, String)> {
    let lookup = |pkg: &Package| -> Option<(Vec<u8>, u32, String)> {
        let entry = pkg
            .entries()
            .iter()
            .find(|e| type_ids.contains(&e.id.type_id) && e.id.instance == instance)?
            .clone();
        let name = pkg
            .path()
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Some((pkg.read(&entry).ok()?, entry.id.type_id, name))
    };
    lookup(current).or_else(|| {
        manager
            .all_packages()
            .ok()?
            .iter()
            .find_map(|pkg| lookup(pkg))
    })
}

/// 从 raster 或 RW4 包裹资源解出顶层 RGBA（不可解返回 None）。

/// 槽位贴图跨包解析：解出顶层 RGBA + 诊断串（instance/来源包/格式/尺寸）。
///
/// slot0 的 f32 参数表（paletteF32）不是颜色——2026-09-08 源码实证
/// row0=(palU,palU2,interiorScale,interiorOffset)，旧"资产调色板"解读作废。
fn resolve_slot_texture(
    bytes: &[u8],
    type_id: u32,
    instance: u32,
    pkg_name: &str,
) -> Option<(Vec<u8>, u32, u32, String)> {
    if type_id == RASTER_IMAGE_TYPE {
        let raster = rw4::RasterImage::parse(bytes).ok()?;
        let rgba = raster.decode_top_mip_rgba().ok()?;
        return Some((
            rgba,
            raster.width,
            raster.height,
            format!(
                "0x{instance:08X} [{pkg_name} raster pixFmt{} {}x{}]",
                raster.pixel_format, raster.width, raster.height
            ),
        ));
    }
    let file = rw4::Rw4File::parse(bytes).ok()?;
    let number = file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()?
        .number;
    let texture = file.decode_texture(bytes, number).ok()?;
    let kind = match texture.texture_type {
        rw4::TEXTURE_TYPE_DXT1 => "DXT1",
        rw4::TEXTURE_TYPE_DXT5 => "DXT5",
        rw4::TEXTURE_TYPE_RAW_BGRA => "rawBGRA",
        rw4::TEXTURE_TYPE_PALETTE_F32 => "paletteF32",
        _ => "other",
    };
    let rgba = texture.decode_top_mip_rgba().ok()?;
    Some((
        rgba,
        u32::from(texture.width),
        u32::from(texture.height),
        format!(
            "0x{instance:08X} [{pkg_name} rw4 {kind} {}x{}]",
            texture.width, texture.height
        ),
    ))
}

/// 材质烘焙上下文（§27 公式）：slot0 参数表 + slot1 tint + slot4 palette 原始像素。
struct MaterialBake {
    params: Vec<[f32; 4]>,
    param_cols: usize,
    tint_rgba: Vec<u8>,
    tint_w: usize,
    tint_h: usize,
    palette_rgba: Vec<u8>,
    pal_w: usize,
    pal_h: usize,
}

/// 单个材质的跨包解析产物（调色板 + 四张通道 PNG，均可为 None）。
struct MaterialResources {
    bake: Option<MaterialBake>,
    base_color_png: Option<Vec<u8>>,
    normal_png: Option<Vec<u8>>,
    roughness_png: Option<Vec<u8>>,
    ao_png: Option<Vec<u8>>,
    /// slot1 原始 color control map（tint 着色器查表键）
    tint_png: Option<Vec<u8>>,
    /// slot4 原始 256×8 调色板
    palette_png: Option<Vec<u8>>,
    /// slot3 原始 shader map（源码语义：B=specularity、A=窗洞/Interior 位置
    /// 预留 5b；kind1 simple 路径的 roughness 反转灰度独立导出，互不影响）
    shader_png: Option<Vec<u8>>,
    /// slot5 原始 interior map（预渲染房间图集；alpha=逐窗灯亮通道，5b。
    /// 旧"relief 高度图"解读作废——图内容即房间/亮灯，与 building4 六采样器
    /// 一一对应：slot0=参数表/1=tint/2=法线/3=shaderMap/4=调色板/5=interiorMap）
    interior_png: Option<Vec<u8>>,
    /// slot5 alpha = relief 高度灰度（building4Clip 的 reliefMap 采样源，
    /// kFlatLevel=23/255：≤23 视为平面；供前端 bumpMap 浮雕开关）
    bump_png: Option<Vec<u8>>,
    /// slot0 参数表 f32 字节（row-major cols×4）
    params_f32: Option<Vec<u8>>,
    param_cols: usize,
    /// 槽位解析诊断（instance/来源包/格式/尺寸，逐行）
    diag: String,
}

/// 从 RGBA 取单通道转灰度 PNG（`invert` 时按 255-v 反转）。
fn channel_gray_png(
    rgba: &[u8],
    width: u32,
    height: u32,
    channel: usize,
    invert: bool,
) -> Option<Vec<u8>> {
    let mut gray = Vec::with_capacity(rgba.len());
    for px in rgba.as_chunks::<4>().0 {
        let v = px[channel];
        let v = if invert { 255 - v } else { v };
        gray.extend_from_slice(&[v, v, v, 255]);
    }
    encode_rgba_png_bytes(width, height, gray).ok()
}

/// 解析指定 MATERIAL section 的槽位资源（源码实证语义，migration.md §27）：
/// slot0 = 参数表（f32：row0=(palU,palU2,interiorScale,interiorOffset)、
/// row1=regionXform、row2=regionXform2）/ slot1 = color control map（有参数表
/// 时为 tint 查表键；无参数表时即 simple diffuse 漫反射色）/ slot2 = normal
/// （标准切线空间 RGB，B=沿法线轴；A=spec→AO 代理）/ slot3 = shader map
/// （B=spec → 反转成 roughness）/ slot4 = 256×8 tint palette。
fn resolve_material_resources(
    file: &rw4::Rw4File,
    data: &[u8],
    package: &Package,
    manager: &PackageManager,
    material_section: u32,
) -> MaterialResources {
    let slots: Vec<rw4::TextureSlotRef> = file
        .decode_material(data, material_section)
        .ok()
        .and_then(|m| match m {
            rw4::MaterialSection::Decoded(decoded) => {
                Some(decoded.texture_slots().copied().collect::<Vec<_>>())
            }
            rw4::MaterialSection::Raw(_) => None,
        })
        .unwrap_or_default();
    let slot_diag: std::cell::RefCell<std::collections::BTreeMap<u32, String>> = Default::default();
    let slot_rgba = |slot: u32| -> Option<(Vec<u8>, u32, u32)> {
        let instance = slots
            .iter()
            .find(|r| r.slot_byte() as u32 == slot)
            .map(|r| r.texture_instance)
            .filter(|i| *i != 0)?;
        let (bytes, type_id, pkg_name) = find_resource_across_packages_named(
            package,
            manager,
            instance,
            &[RASTER_IMAGE_TYPE, RW4_MODEL_TYPE],
        )?;
        let (rgba, w, h, diag) = resolve_slot_texture(&bytes, type_id, instance, &pkg_name)?;
        slot_diag.borrow_mut().insert(slot, diag);
        Some((rgba, w, h))
    };

    let mut resources = MaterialResources {
        bake: None,
        base_color_png: None,
        normal_png: None,
        roughness_png: None,
        ao_png: None,
        tint_png: None,
        palette_png: None,
        shader_png: None,
        interior_png: None,
        bump_png: None,
        params_f32: None,
        param_cols: 0,
        diag: String::new(),
    };
    // slot0 f32 参数表（源码 building5UnpackDataViewPS：palU/palU2/interior/
    // regionXform/regionXform2 逐行对应 row0-3）
    let mut params: Option<(Vec<[f32; 4]>, usize)> = None;
    if let Some(instance) = slots
        .iter()
        .find(|r| r.slot_byte() == 0)
        .map(|r| r.texture_instance)
        .filter(|i| *i != 0)
    {
        if let Some((bytes, _, pkg_name)) =
            find_resource_across_packages_named(package, manager, instance, &[RW4_MODEL_TYPE])
        {
            if let Ok(tex_file) = rw4::Rw4File::parse(&bytes) {
                if let Some(sec) = tex_file
                    .sections_of_type(rw4::SectionType::TEXTURE)
                    .next()
                    .map(|s| s.number)
                {
                    if let Ok(tex) = tex_file.decode_texture(&bytes, sec) {
                        if let Ok(pixels) = tex.decode_palette_f32() {
                            slot_diag.borrow_mut().insert(
                                0,
                                format!(
                                    "0x{instance:08X} [{pkg_name} rw4 paletteF32 {}x{}（参数表：row0=palU/palU2/interior、row1=regionXform、row2=top、row3=padding/room）]",
                                    tex.width, tex.height
                                ),
                            );
                            params = Some((pixels, usize::from(tex.width)));
                        }
                    }
                }
            }
        }
    }
    let slot1 = slot_rgba(1);
    // slot4 = 256×8 tint palette（512×16，2×2 像素块）——烘焙链最终查色表
    let palette_raw = slot_rgba(4);
    // 组装烘焙上下文（building4 链：有 slot0 参数表时 slot1 = tint 查表键）
    let mut baked = false;
    if let (
        Some((params_table, param_cols)),
        Some((tint_rgba, tint_w, tint_h)),
        Some((pal_rgba, pal_w, pal_h)),
    ) = (params, slot1.clone(), palette_raw)
    {
        if param_cols > 0 {
            resources.tint_png = encode_rgba_png_bytes(tint_w, tint_h, tint_rgba.clone()).ok();
            resources.palette_png = encode_rgba_png_bytes(pal_w, pal_h, pal_rgba.clone()).ok();
            let mut f32_bytes = Vec::with_capacity(params_table.len() * 16);
            for texel in &params_table {
                for value in texel {
                    f32_bytes.extend_from_slice(&value.to_le_bytes());
                }
            }
            resources.params_f32 = Some(f32_bytes);
            resources.param_cols = param_cols;
            resources.bake = Some(MaterialBake {
                params: params_table,
                param_cols,
                tint_rgba,
                tint_w: usize::try_from(tint_w).unwrap_or(0),
                tint_h: usize::try_from(tint_h).unwrap_or(0),
                palette_rgba: pal_rgba,
                pal_w: usize::try_from(pal_w).unwrap_or(0),
                pal_h: usize::try_from(pal_h).unwrap_or(0),
            });
            baked = true;
        }
    }
    if !baked {
        // 无 building4 参数表 = simple diffuse 链：slot1 即漫反射贴图。
        // 旧"R→调色板 LUT"产物为 (palU,palU2,0.125) 垃圾色，已废弃。
        if let Some((rgba, width, height)) = slot1 {
            resources.base_color_png = encode_rgba_png_bytes(width, height, rgba).ok();
        }
    }
    // slot2：标准切线空间法线（B=沿法线轴，平坦≈128,128,255）+ A=spec（AO 代理）
    if let Some((rgba, width, height)) = slot_rgba(2) {
        resources.ao_png = channel_gray_png(&rgba, width, height, 3, false);
        resources.normal_png = encode_rgba_png_bytes(width, height, rgba).ok();
    }
    // slot3：源码语义（migration.md §28）B=specularity（×kBuildingSpecOverdrive=2
    // 为 specStrength）、A=窗洞/Interior 位置（5b）→ tint 链导出原始 RGBA；
    // kind1 simple 路径沿用旧 roughness 反转灰度
    if let Some((rgba, width, height)) = slot_rgba(3) {
        resources.roughness_png = channel_gray_png(&rgba, width, height, 2, true);
        resources.shader_png = encode_rgba_png_bytes(width, height, rgba).ok();
    }
    // slot5：interior map 图集（5b 假内景；用户目视 mat6_slot5.png 证实为
    // 预渲染房间图+亮灯，旧 relief 高度图解读作废）。alpha 通道即
    // reliefMap 高度（DXT5 高精度 alpha；kFlatLevel=23/255 平面钳制）
    if let Some((rgba, width, height)) = slot_rgba(5) {
        resources.interior_png = encode_rgba_png_bytes(width, height, rgba.clone()).ok();
        let mut height_rgba = rgba;
        for px in height_rgba.chunks_exact_mut(4) {
            let h = if px[3] <= 23 { 0 } else { px[3] };
            px[0] = h;
            px[1] = h;
            px[2] = h;
            px[3] = 255;
        }
        resources.bump_png = encode_rgba_png_bytes(width, height, height_rgba).ok();
    }
    for slot in 0..=5u32 {
        if let Some(line) = slot_diag.borrow().get(&slot) {
            resources.diag.push_str(&format!("  slot{slot} {line}\n"));
        }
    }
    resources
}

/// 逐 mesh UV 类型：0=无 / 1=常规贴图（FLOAT2 或 FLOAT4≤8）/ 2=tint 着色器
/// （FLOAT4 大坐标 facade 世界投影）。
fn mesh_uv_kind(mesh: &rw4::DecodedMesh) -> u8 {
    let mut kind = 0u8;
    for vertex in &mesh.vertices {
        for (element, value) in &vertex.components {
            if element.usage != rw4::DeclarationUsage::TexCoord {
                continue;
            }
            match value {
                rw4::ComponentValue::Float2(_) => kind = kind.max(1),
                rw4::ComponentValue::Float4(f) => {
                    if f[0].abs() <= 8.0 && f[1].abs() <= 8.0 {
                        kind = kind.max(1);
                    } else {
                        kind = kind.max(2);
                    }
                }
                _ => {}
            }
        }
    }
    kind
}

/// facade 顶点色烘焙（building4 源码逐字，migration.md §27）：
/// `baseUv = frac(uv) * regionXform.xy + regionXform.zw` → tint 查表（slot1）
/// → palette 查色（slot4，`BuildingPaletteSample`：subsample = tint.rg ×
/// kPaletteInvSize×0.5 + kPaletteInvSize×0.25；tint.b 为亮度 ×2）。
/// 源码行绑定：row0=(palU,palU2,interiorScale,interiorOffset)、row1=regionXform、
/// row2=regionXform2；**palette V = buildingVariation（实例数据 × 1/8，
/// 0..7 行）不在参数表内**——查看器无实例数据，固定取 variation 行 0
/// （kPaletteSize = int2(256,8)，512×16 物理 = 2×2 块/采样点）。
/// 顶点 materialIndex = D3DCOLOR.G（= 游戏 In.color.r，D3DCOLOR 内存为
/// B,G,R,A 字节序）。无 D3DCOLOR 或参数表的网格返回 None。
fn bake_vertex_colors(mesh: &rw4::DecodedMesh, bake: &MaterialBake) -> Option<Vec<[f32; 3]>> {
    // BuildingPaletteVariationVS(buildingType) = buildingType / 8；无实例数据取行 0
    const BUILDING_VARIATION: f32 = 0.0;
    const K_PALETTE_INV_SIZE: [f32; 2] = [1.0 / 256.0, 1.0 / 8.0];
    let any = mesh.vertices.iter().any(|v| v.d3d_color_g().is_some());
    if !any || bake.param_cols == 0 || bake.tint_w == 0 || bake.pal_w == 0 {
        return None;
    }
    Some(
        mesh.vertices
            .iter()
            .map(|v| {
                const FALLBACK: [f32; 3] = [1.0, 1.0, 1.0];
                let Some(m) = v.d3d_color_g().map(|g| g as usize) else {
                    return FALLBACK;
                };
                let Some(xform) = bake.params.get(bake.param_cols + m).copied() else {
                    return FALLBACK;
                };
                let Some(pal_origin) = bake.params.get(m).copied() else {
                    return FALLBACK;
                };
                let Some(f) = v
                    .components
                    .iter()
                    .find(|(e, _)| e.usage == rw4::DeclarationUsage::TexCoord)
                    .and_then(|(_, value)| match value {
                        rw4::ComponentValue::Float4(f) => Some(*f),
                        _ => None,
                    })
                else {
                    return FALLBACK;
                };
                let bu = (f[0] - f[0].floor()) * xform[0] + xform[2];
                let bv = (f[1] - f[1].floor()) * xform[1] + xform[3];
                let tx = ((bu - bu.floor()).clamp(0.0, 0.999) * bake.tint_w as f32) as usize;
                let ty = ((bv - bv.floor()).clamp(0.0, 0.999) * bake.tint_h as f32) as usize;
                let Some(t) = bake.tint_rgba.get((ty * bake.tint_w + tx) * 4..) else {
                    return FALLBACK;
                };
                // uvsBase = (palU, buildingVariation, palU, kSurfacePalV)；
                // row0.y 是 palU2（Top 层列号），不是 V。
                let pu = pal_origin[0]
                    + f32::from(t[0]) / 255.0 * (K_PALETTE_INV_SIZE[0] * 0.5)
                    + K_PALETTE_INV_SIZE[0] * 0.25;
                let pv = BUILDING_VARIATION
                    + f32::from(t[1]) / 255.0 * (K_PALETTE_INV_SIZE[1] * 0.5)
                    + K_PALETTE_INV_SIZE[1] * 0.25;
                let px = ((pu - pu.floor()).clamp(0.0, 0.999) * bake.pal_w as f32) as usize;
                let py = ((pv - pv.floor()).clamp(0.0, 0.999) * bake.pal_h as f32) as usize;
                let Some(p) = bake.palette_rgba.get((py * bake.pal_w + px) * 4..) else {
                    return FALLBACK;
                };
                let bright = f32::from(t[2]) / 255.0 * 2.0;
                [
                    (f32::from(p[0]) * bright / 255.0).clamp(0.0, 1.0),
                    (f32::from(p[1]) * bright / 255.0).clamp(0.0, 1.0),
                    (f32::from(p[2]) * bright / 255.0).clamp(0.0, 1.0),
                ]
            })
            .collect(),
    )
}

/// PE 精细渲染：一次返回模型全部网格 GLB（COLOR_0 按**每 mesh 材质**烘焙）
/// + 逐材质贴图 PNG（0x2001A 绑定表）+ 槽位诊断文本，经原始字节通道传输
/// （`LOT_MODEL_PAYLOAD_MAGIC` v5 容器），取代前端 1+N 次 section 请求。
#[tauri::command]
pub async fn read_lot_model_meshes(
    state: State<'_, AppState>,
    request: LotModelMeshesRequest,
) -> Result<tauri::ipc::Response, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let payload = read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi.clone(),
        move |data, package, manager, _store| {
            if request.tgi.type_id != RW4_MODEL_TYPE {
                return Err(PackageError::InvalidArgument(
                    "lot model meshes requires an RW4 model resource".into(),
                ));
            }
            let file = rw4::Rw4File::parse(data)?;
            Ok(build_lot_model_payload(
                &file,
                data,
                package,
                manager,
                request.tgi.instance,
            ))
        },
    )
    .await?;
    Ok(tauri::ipc::Response::new(payload))
}

/// 组装 LOTM v8 容器：按 MeshMaterialAssignment（0x2001A）逐 mesh 配材质。
///
/// 布局（小端）：`magic | version=7 | mesh_count`，每 mesh `u32 len + GLB`
/// （COLOR_0 烘焙 + TEXCOORD_1.xy=materialIndex/255+内景种子 + TEXCOORD_2/3=
/// facade 世界投影 UV）；`material_count`，每材质 8 张 PNG（base/normal/rough/
/// ao/tintRaw/palette/shaderMap/interiorMap）+ 参数表 f32 + paramCols；每 mesh
/// `u32 material_index + u8 uv_kind`；末尾 `u32 diag_len + UTF-8 诊断文本`
/// （mesh↔material↔slot 贴图及其来源包，供 info 面板复制复盘）。绑定缺失/
/// 材质不可解时回退第一个可解码材质；完全无材质则指向占位空材质。
/// v7 = v6 + 每材质第 8 张 PNG（slot5 interior map，5b 假内景）+
/// TEXCOORD_1 升 VEC4（.y = 内景随机种子）。
/// v8 = v7 + 每材质第 9 张 PNG（slot5 alpha = relief 高度灰度，浮雕 bumpMap）。
fn build_lot_model_payload(
    file: &rw4::Rw4File,
    data: &[u8],
    package: &Package,
    manager: &PackageManager,
    model_instance: u32,
) -> Vec<u8> {
    let bindings = file.decode_mesh_material_bindings(data);
    let fallback_material = file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .find_map(|s| {
            let decoded = file.decode_material(data, s.number).ok()?;
            matches!(decoded, rw4::MaterialSection::Decoded(_)).then_some(s.number)
        });
    let pkg_name = package
        .path()
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut glbs: Vec<Vec<u8>> = Vec::new();
    let mut mesh_material: Vec<u32> = Vec::new();
    let mut mesh_uv_kinds: Vec<u8> = Vec::new();
    let mut material_sections: Vec<u32> = Vec::new();
    let mut material_resources: Vec<MaterialResources> = Vec::new();
    // 诊断：mesh 行 + 材质块（slot0-5 instance/来源包/格式/尺寸）
    let mut mesh_diag: Vec<String> = Vec::new();
    const MESH_DIAG_CAP: usize = 400;
    let mut mesh_diag_truncated = 0usize;

    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let mesh = match file.decode_mesh(data, section.number) {
            Ok(mesh) => mesh,
            Err(_) => continue,
        };
        if !mesh.is_exportable() {
            continue;
        }
        let bound_section = bindings
            .iter()
            .find(|b| b.mesh_section == section.number)
            .map(|b| b.material_section)
            .or(fallback_material)
            .unwrap_or(u32::MAX);
        let material_index = match material_sections
            .iter()
            .position(|number| *number == bound_section)
        {
            Some(index) => index as u32,
            None => {
                material_resources.push(resolve_material_resources(
                    file,
                    data,
                    package,
                    manager,
                    bound_section,
                ));
                material_sections.push(bound_section);
                (material_resources.len() - 1) as u32
            }
        };
        let colors = material_resources[material_index as usize]
            .bake
            .as_ref()
            .and_then(|bake| bake_vertex_colors(&mesh, bake));
        let uv_kind = mesh_uv_kind(&mesh);
        let mut diag_line = format!(
            "mesh #{:<4} verts={:<6} tris={:<6} uvKind={} → material #{} (idx {})",
            section.number,
            mesh.vertices.len(),
            mesh.triangles.len(),
            uv_kind,
            bound_section,
            material_index,
        );
        let mat_indices: Vec<f32> = mesh
            .vertices
            .iter()
            .map(|v| f32::from(v.d3d_color_g().unwrap_or(0)))
            .collect();
        let glb = sc_exporter::export_glb_with_colors(
            &mesh,
            None,
            &[],
            sc_exporter::EmbeddedTextures::default(),
            colors.as_deref(),
            Some(&mat_indices),
        );
        if glb.bytes.len() > MESH_GLB_MAX_BYTES {
            diag_line.push_str("  [GLB 超限跳过]");
            mesh_diag_truncated += 1;
        } else {
            glbs.push(glb.bytes);
            mesh_material.push(material_index);
            mesh_uv_kinds.push(uv_kind);
        }
        if mesh_diag.len() < MESH_DIAG_CAP {
            mesh_diag.push(diag_line);
        } else {
            mesh_diag_truncated += 1;
        }
    }

    // 诊断文本
    let mut diag = format!(
        "LOTM v7 | model: {pkg_name} instance=0x{model_instance:08X} | meshes={} materials={} | bindings={}\n",
        glbs.len(),
        material_resources.len(),
        bindings.len(),
    );
    diag.push_str(&mesh_diag.join("\n"));
    diag.push('\n');
    if mesh_diag_truncated > 0 {
        diag.push_str(&format!("…（另有 {mesh_diag_truncated} 条 mesh 略）\n"));
    }
    for (index, (section, resources)) in material_sections
        .iter()
        .zip(&material_resources)
        .enumerate()
    {
        diag.push_str(&format!(
            "material #{} (idx {}) paramCols={} bake={}\n{}",
            section,
            index,
            resources.param_cols,
            if resources.bake.is_some() {
                "有"
            } else {
                "无（slot1 走 diffuse）"
            },
            resources.diag,
        ));
    }

    let mut out = Vec::new();
    out.extend_from_slice(&LOT_MODEL_PAYLOAD_MAGIC.to_le_bytes());
    out.extend_from_slice(&8u32.to_le_bytes());
    out.extend_from_slice(&(glbs.len() as u32).to_le_bytes());
    for glb in &glbs {
        out.extend_from_slice(&(glb.len() as u32).to_le_bytes());
        out.extend_from_slice(glb);
    }
    out.extend_from_slice(&(material_resources.len() as u32).to_le_bytes());
    for material in &material_resources {
        for png in [
            &material.base_color_png,
            &material.normal_png,
            &material.roughness_png,
            &material.ao_png,
            &material.tint_png,
            &material.palette_png,
            &material.shader_png,
            &material.interior_png,
            &material.bump_png,
        ] {
            match png {
                Some(bytes) => {
                    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                    out.extend_from_slice(bytes);
                }
                None => out.extend_from_slice(&0u32.to_le_bytes()),
            }
        }
        match &material.params_f32 {
            Some(bytes) => {
                out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                out.extend_from_slice(bytes);
            }
            None => out.extend_from_slice(&0u32.to_le_bytes()),
        }
        out.extend_from_slice(&(material.param_cols as u32).to_le_bytes());
    }
    for (index, uv_kind) in mesh_material.iter().zip(&mesh_uv_kinds) {
        out.extend_from_slice(&index.to_le_bytes());
        out.push(*uv_kind);
    }
    out.extend_from_slice(&(diag.len() as u32).to_le_bytes());
    out.extend_from_slice(diag.as_bytes());
    out
}

#[tauri::command]
pub async fn read_raster_preview(
    state: State<'_, AppState>,
    request: RasterPreviewRequest,
) -> Result<RasterPreviewData, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, _package, _manager, _store| {
            let raster = rw4::RasterImage::parse(data)?;
            let mut decodable = false;
            let mut png_base64 = None;
            if raster.is_raw_rgba() {
                if let Ok(rgba) = raster.decode_top_mip_rgba() {
                    if let Ok(png) = encode_rgba_png(raster.width, raster.height, rgba) {
                        png_base64 = Some(png);
                        decodable = true;
                    }
                }
            }
            Ok(RasterPreviewData {
                raster_type: raster.raster_type,
                width: raster.width,
                height: raster.height,
                mip_count: raster.mip_count,
                pixel_size: raster.pixel_size,
                pixel_format: raster.pixel_format,
                decodable,
                png_base64,
            })
        },
    )
    .await
}

#[tauri::command]
pub async fn patch_property_overlay(
    state: State<'_, AppState>,
    request: PropertyPatchRequest,
) -> Result<PropertyPatchResult, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let tgi: ResourceId = request.tgi.clone().into();
        if tgi.type_id != PROPERTY_RESOURCE_TYPE {
            return Err(CommandError::from(PackageError::InvalidArgument(
                "property patch requires a property resource".into(),
            )));
        }
        let package = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let package_path = fs::canonicalize(package.path())
            .map_err(PackageError::Io)
            .map_err(CommandError::from)?;
        let output_path = PathBuf::from(&request.output_path);
        if output_path.exists()
            && fs::canonicalize(&output_path).ok().as_deref() == Some(package_path.as_path())
        {
            return Err(CommandError::from(PackageError::InvalidArgument(
                "property overlay output cannot replace the source package".into(),
            )));
        }
        let entry = package
            .entry(tgi)
            .ok_or(PackageError::ResourceNotFound(tgi))
            .map_err(CommandError::from)?;
        if u64::from(entry.decompressed_size) > RESOURCE_DATA_MAX {
            return Err(CommandError::from(PackageError::LimitExceeded(
                RESOURCE_DATA_MAX as usize,
            )));
        }
        let data = package
            .read(entry)
            .map_err(PackageError::Dbpf)
            .map_err(CommandError::from)?;
        let mut file = sc_properties::PropertyFile::parse_with_limits(
            &data,
            sc_properties::ParseLimits::default(),
        )
        .map_err(PackageError::Properties)
        .map_err(CommandError::from)?;
        let changed_hashes =
            apply_property_patches(&mut file, &request.patches).map_err(CommandError::from)?;
        let property_data = file
            .encode_canonical()
            .map_err(PackageError::Properties)
            .map_err(CommandError::from)?;
        let overlay = dbpf::write_uncompressed_overlay(&[dbpf::OverlayEntry::new(
            tgi,
            property_data.clone(),
        )])
        .map_err(PackageError::Overlay)
        .map_err(CommandError::from)?;
        if overlay.len() as u64 > PROPERTY_OVERLAY_MAX {
            return Err(CommandError::from(PackageError::OutputLimitExceeded(
                PROPERTY_OVERLAY_MAX,
            )));
        }
        let job_id = manager.next_job_id().map_err(CommandError::from)?;
        write_export(&output_path, &overlay, job_id).map_err(CommandError::from)?;
        Ok(PropertyPatchResult {
            output_path: request.output_path,
            tgi: tgi.into(),
            changed_hashes,
            property_bytes: property_data.len() as u64,
            overlay_bytes: overlay.len() as u64,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}
#[tauri::command]
pub async fn read_wwise_bank(
    state: State<'_, AppState>,
    request: ReadResourceDataRequest,
) -> Result<crate::wwise::WwiseBank, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let tgi = request.tgi.clone();
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, _, _, _| {
            if tgi.type_id != WWISE_BANK_TYPE_ID {
                return Err(PackageError::InvalidArgument(
                    "Wwise bank inspection requires a BKHD resource".into(),
                ));
            }
            Ok(crate::wwise::WwiseBankData::parse(data)?.bank)
        },
    )
    .await
}

#[tauri::command]
pub async fn read_property_preview(
    state: State<'_, AppState>,
    request: ReadResourceDataRequest,
) -> Result<PropertyPreviewData, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let bundled_registry = bundled_registry_path(&state.app);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, package, manager, store| {
            let registry = package_registry(store, manager, package, bundled_registry.as_deref());
            property_preview(data, registry.as_ref())
        },
    )
    .await
}

#[tauri::command]
pub async fn read_rw4_preview(
    state: State<'_, AppState>,
    request: ReadResourceDataRequest,
) -> Result<Rw4PreviewData, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        |data, _, _, _| rw4_sections(data),
    )
    .await
}

#[tauri::command]
pub async fn read_rw4_section_detail(
    state: State<'_, AppState>,
    request: Rw4SectionRequest,
) -> Result<Rw4SectionDetail, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    read_resource_with(
        manager,
        store,
        request.package_id,
        request.tgi,
        move |data, _, _, _| rw4_section_detail(data, request.number),
    )
    .await
}

#[tauri::command]
pub async fn resolve_name(
    state: State<'_, AppState>,
    request: ResolveNameRequest,
) -> Result<NameResolution, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let tgi = request.tgi.clone();
        let registry = manager
            .registry(
                &request.main_registry_path,
                request.user_registry_path.as_deref(),
            )
            .map_err(CommandError::from)?;
        Ok(NameResolution {
            type_name: registry.type_name(tgi.type_id),
            group_name: registry.group_name(tgi.group),
            instance_name: registry.instance_name(tgi.instance),
            tgi,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

pub const RESOLVE_NAMES_MAX: usize = 500;
const REGISTRY_DATABASE: &str = "database_main.s3db";
const REGISTRY_USER_DATABASE: &str = "database_user.s3db";
const REGISTRY_SEARCH_DEPTH: usize = 3;

fn find_registry_database(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start.to_path_buf());
    for _ in 0..=REGISTRY_SEARCH_DEPTH {
        let Some(directory) = current else {
            break;
        };
        let candidate = directory.join(REGISTRY_DATABASE);
        if candidate.is_file() {
            return Some(candidate);
        }
        current = directory.parent().map(Path::to_path_buf);
    }
    None
}

fn semantic_instance_name(
    registry: Option<&sc_registry::Registry>,
    instance: u32,
) -> Option<String> {
    let record = registry?.instances().get(&instance)?;
    (!record.name.is_empty()).then(|| record.name.clone())
}

fn registry_candidate(main: PathBuf) -> (PathBuf, Option<String>) {
    let user = main.with_file_name(REGISTRY_USER_DATABASE);
    let user = user.is_file().then(|| user.to_string_lossy().into_owned());
    (main, user)
}

pub(crate) fn bundled_registry_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .resolve(
            "resources/database_main.s3db",
            tauri::path::BaseDirectory::Resource,
        )
        .ok()
        .filter(|path| path.is_file())
}

fn package_registry(
    store: &sc_store::Store,
    manager: &PackageManager,
    package: &Package,
    bundled_main: Option<&Path>,
) -> Option<Arc<sc_registry::Registry>> {
    let mut candidates: Vec<(PathBuf, Option<String>)> = Vec::new();
    let push_candidate = |main: PathBuf, candidates: &mut Vec<_>| {
        let candidate = registry_candidate(main);
        if !candidates.iter().any(|(main, _)| *main == candidate.0) {
            candidates.push(candidate);
        }
    };
    if let Some(path) = find_registry_database(package.path()) {
        push_candidate(path, &mut candidates);
    }
    if let Ok(Some(game_data_path)) = store
        .app_settings()
        .map(|settings| settings.and_then(|settings| settings.game_data_path))
    {
        if let Some(path) = find_registry_database(Path::new(&game_data_path)) {
            push_candidate(path, &mut candidates);
        }
    }
    if let Some(path) = bundled_main {
        push_candidate(path.to_path_buf(), &mut candidates);
    }
    candidates.iter().find_map(|(main, user)| {
        manager
            .registry(&main.to_string_lossy(), user.as_deref())
            .ok()
    })
}

#[tauri::command]
pub async fn resolve_names(
    state: State<'_, AppState>,
    request: ResolveNamesRequest,
) -> Result<Vec<ResolvedResourceName>, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let bundled_registry = bundled_registry_path(&state.app);
    tauri::async_runtime::spawn_blocking(move || {
        if request.tgis.len() > RESOLVE_NAMES_MAX {
            return Err(CommandError::from(PackageError::LimitExceeded(
                RESOLVE_NAMES_MAX,
            )));
        }
        let package = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let registry = package_registry(&store, &manager, &package, bundled_registry.as_deref());
        Ok(request
            .tgis
            .iter()
            .map(|tgi| ResolvedResourceName {
                tgi: tgi.clone(),
                display_name: semantic_instance_name(registry.as_deref(), tgi.instance),
            })
            .collect())
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn export_image_resource(
    data: Vec<u8>,
    type_id: u32,
    format: ExportFormat,
) -> Result<Vec<u8>, PackageError> {
    let output = match format {
        ExportFormat::Png => image::ImageFormat::Png,
        ExportFormat::Jpg => image::ImageFormat::Jpeg,
        ExportFormat::Gif => image::ImageFormat::Gif,
        _ => return Err(PackageError::UnsupportedExport),
    };
    let same_format = matches!(
        (type_id, format),
        (PNG_TYPE_ID, ExportFormat::Png)
            | (JPG_TYPE_ID, ExportFormat::Jpg)
            | (GIF_TYPE_ID, ExportFormat::Gif)
    );
    if same_format {
        return Ok(data);
    }
    let decoded = image::load_from_memory(&data)?;
    let mut output_data = Cursor::new(Vec::new());
    decoded.write_to(&mut output_data, output)?;
    Ok(output_data.into_inner())
}
fn export_bytes(
    package: &Package,
    tgi: ResourceId,
    format: ExportFormat,
) -> Result<Vec<u8>, PackageError> {
    let entry = package
        .entry(tgi)
        .ok_or(PackageError::ResourceNotFound(tgi))?;
    if u64::from(entry.decompressed_size) > RESOURCE_DECOMPRESSED_MAX {
        return Err(PackageError::DecompressionLimitExceeded(
            RESOURCE_DECOMPRESSED_MAX,
        ));
    }
    let data = package.read(entry)?;
    match format {
        ExportFormat::Raw => Ok(data),
        ExportFormat::Vp6 => {
            if tgi.type_id == VIDEO_TYPE_ID {
                Ok(data)
            } else {
                Err(PackageError::UnsupportedExport)
            }
        }
        ExportFormat::Png | ExportFormat::Jpg | ExportFormat::Gif
            if matches!(tgi.type_id, PNG_TYPE_ID | JPG_TYPE_ID | GIF_TYPE_ID) =>
        {
            export_image_resource(data, tgi.type_id, format)
        }
        ExportFormat::Wav
        | ExportFormat::Mp3
        | ExportFormat::Ogg
        | ExportFormat::Flac
        | ExportFormat::Mp4 => Err(PackageError::UnsupportedExport),
        ExportFormat::Obj | ExportFormat::Glb => {
            let file = rw4::Rw4File::parse(&data)?;
            let section = file
                .sections_of_type(rw4::SectionType::MESH)
                .next()
                .ok_or(PackageError::UnsupportedExport)?;
            let mesh = file.decode_mesh(&data, section.number)?;
            if !mesh.is_exportable() {
                return Err(PackageError::UnsupportedExport);
            }
            if matches!(format, ExportFormat::Obj) {
                Ok(sc_exporter::export_obj(&mesh).into_bytes())
            } else {
                let skeleton = file
                    .sections_of_type(rw4::SectionType::RW4_SKELETON)
                    .next()
                    .map(|section| file.decode_skeleton(&data, section.number))
                    .transpose()?;
                let skeleton_id = skeleton.as_ref().map(|value| value.hierarchy.id);
                let mut animations = Vec::new();
                for section in file.sections_of_type(rw4::SectionType::ANIM) {
                    let animation = file.decode_anim(&data, section.number)?;
                    if skeleton_id.is_none_or(|id| id == animation.skeleton_id) {
                        animations.push(animation);
                    }
                }
                Ok(sc_exporter::export_glb(&mesh, skeleton.as_ref(), &animations).bytes)
            }
        }
        ExportFormat::Png | ExportFormat::Jpg | ExportFormat::Gif
            if matches!(tgi.type_id, PNG_TYPE_ID | JPG_TYPE_ID | GIF_TYPE_ID) =>
        {
            export_image_resource(data, tgi.type_id, format)
        }
        ExportFormat::Gif => Err(PackageError::UnsupportedExport),
        ExportFormat::Png | ExportFormat::Jpg | ExportFormat::Tga | ExportFormat::Dds => {
            let file = rw4::Rw4File::parse(&data)?;
            let section = file
                .sections_of_type(rw4::SectionType::TEXTURE)
                .next()
                .ok_or(PackageError::UnsupportedExport)?;
            let texture = file.decode_texture(&data, section.number)?;
            validate_texture_budget(&texture)?;
            let format = match format {
                ExportFormat::Png => sc_exporter::TextureOutputFormat::Png,
                ExportFormat::Jpg => sc_exporter::TextureOutputFormat::Jpg,
                ExportFormat::Tga => sc_exporter::TextureOutputFormat::Tga,
                ExportFormat::Dds => sc_exporter::TextureOutputFormat::Dds,
                _ => unreachable!(),
            };
            Ok(sc_exporter::export_texture(&texture, format)?)
        }
    }
}

fn validate_texture_budget(texture: &rw4::DecodedTexture) -> Result<(), PackageError> {
    let pixel_bytes = u64::from(texture.width)
        .checked_mul(u64::from(texture.height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(PackageError::LimitExceeded(
            RESOURCE_DECOMPRESSED_MAX as usize,
        ))?;
    if pixel_bytes > RESOURCE_DECOMPRESSED_MAX
        || texture.blob.len() as u64 > RESOURCE_DECOMPRESSED_MAX
    {
        return Err(PackageError::DecompressionLimitExceeded(
            RESOURCE_DECOMPRESSED_MAX,
        ));
    }
    Ok(())
}

fn validate_output_path(path: &Path) -> Result<(), PackageError> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| PackageError::InvalidArgument("output path has no parent".into()))?;
    if !parent.is_dir() {
        return Err(PackageError::InvalidArgument(
            "output directory does not exist".into(),
        ));
    }
    if path.file_name().is_none() {
        return Err(PackageError::InvalidArgument(
            "output path has no file name".into(),
        ));
    }
    if path.exists() {
        return Err(PackageError::OutputExists(
            path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

const AUDIO_TYPE_ID: u32 = 0x0D9E_5710;
const WWISE_BANK_TYPE_ID: u32 = 0x0A4D_8D09;
const VIDEO_TYPE_ID: u32 = 0x3768_40D7;
const PNG_TYPE_ID: u32 = 0x2F7D_0004;
const JPG_TYPE_ID: u32 = 0x3F86_62EA;
const GIF_TYPE_ID: u32 = 0x2F7D_0007;

fn required_tool(tools: &MediaTools, format: ExportFormat) -> Result<&str, PackageError> {
    let tool = match format {
        ExportFormat::Wav => &tools.vgmstream,
        ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac | ExportFormat::Mp4 => {
            &tools.ffmpeg
        }
        _ => return Err(PackageError::UnsupportedExport),
    };
    tool.path
        .as_deref()
        .ok_or(PackageError::ToolNotFound(match format {
            ExportFormat::Wav => "vgmstream",
            ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac | ExportFormat::Mp4 => {
                "ffmpeg"
            }
            _ => "media tool",
        }))
}

fn validate_media_request(tgi: ResourceId, format: ExportFormat) -> Result<(), PackageError> {
    match format {
        ExportFormat::Wav | ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac
            if tgi.type_id == AUDIO_TYPE_ID || tgi.type_id == WWISE_BANK_TYPE_ID =>
        {
            Ok(())
        }
        ExportFormat::Vp6 | ExportFormat::Mp4 if tgi.type_id == VIDEO_TYPE_ID => Ok(()),
        ExportFormat::Wav
        | ExportFormat::Mp3
        | ExportFormat::Ogg
        | ExportFormat::Flac
        | ExportFormat::Vp6
        | ExportFormat::Mp4 => Err(PackageError::UnsupportedExport),
        _ => Ok(()),
    }
}

fn create_temp_file(path: &Path, bytes: &[u8]) -> Result<(), PackageError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn validate_wave_bytes(data: &[u8]) -> bool {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return false;
    }
    let declared = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let Some(limit) = declared.checked_add(8) else {
        return false;
    };
    if limit < 12 || limit > data.len() {
        return false;
    }
    let mut offset = 12;
    let mut has_fmt = false;
    let mut has_data = false;
    while offset < limit {
        if limit - offset < 8 {
            return false;
        }
        let size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let Some(end) = offset
            .checked_add(8)
            .and_then(|value| value.checked_add(size))
        else {
            return false;
        };
        let Some(padded_end) = end.checked_add(size & 1) else {
            return false;
        };
        if padded_end > limit {
            return false;
        }
        match &data[offset..offset + 4] {
            b"fmt " if size >= 16 => has_fmt = true,
            b"data" => has_data = true,
            _ => {}
        }
        offset = padded_end;
    }
    offset == limit && has_fmt && has_data
}

fn validate_mp4_bytes(data: &[u8]) -> bool {
    let mut offset = 0usize;
    let mut has_ftyp = false;
    let mut has_media = false;
    while offset < data.len() {
        if data.len() - offset < 8 {
            return false;
        }
        let size32 = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        let size = if size32 == 1 {
            if data.len() - offset < 16 {
                return false;
            }
            u64::from_be_bytes(data[offset + 8..offset + 16].try_into().unwrap())
        } else if size32 == 0 {
            (data.len() - offset) as u64
        } else {
            u64::from(size32)
        };
        if size < 8 || size > (data.len() - offset) as u64 {
            return false;
        }
        match &data[offset + 4..offset + 8] {
            b"ftyp" => has_ftyp = true,
            b"moov" | b"mdat" => has_media = true,
            _ => {}
        }
        offset += size as usize;
    }
    offset == data.len() && has_ftyp && has_media
}

fn validate_media_output(path: &Path, format: ExportFormat) -> Result<(), PackageError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > RESOURCE_DECOMPRESSED_MAX {
        return Err(PackageError::InvalidMediaOutput(
            "media output size is invalid",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)?;
    let valid = match format {
        ExportFormat::Wav => validate_wave_bytes(&data),
        ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac => {
            validate_audio_output(&data, format)
        }
        ExportFormat::Mp4 => validate_mp4_bytes(&data),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(PackageError::InvalidMediaOutput(
            "media output signature is invalid",
        ))
    }
}

fn install_media_output(source: &Path, target: &Path) -> Result<usize, PackageError> {
    if let Err(error) = fs::rename(source, target) {
        let _ = fs::remove_file(source);
        return Err(error.into());
    }
    Ok(fs::metadata(target)?.len() as usize)
}

fn validate_audio_output(data: &[u8], format: ExportFormat) -> bool {
    match format {
        ExportFormat::Wav => validate_wave_bytes(data),
        ExportFormat::Mp3 => {
            data.starts_with(b"ID3")
                || (data.len() >= 2 && data[0] == 0xFF && data[1] & 0xE0 == 0xE0)
        }
        ExportFormat::Ogg => data.starts_with(b"OggS"),
        ExportFormat::Flac => data.starts_with(b"fLaC"),
        _ => false,
    }
}

fn media_args(
    format: ExportFormat,
    input: &Path,
    output: &Path,
) -> Result<Vec<OsString>, PackageError> {
    let mut args = vec![
        OsString::from("-y"),
        OsString::from("-nostdin"),
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
    ];
    match format {
        ExportFormat::Wav => {
            args.clear();
            args.extend([
                OsString::from("-o"),
                output.as_os_str().to_owned(),
                input.as_os_str().to_owned(),
            ]);
        }
        ExportFormat::Mp3 => args.extend([
            OsString::from("-i"),
            input.as_os_str().to_owned(),
            OsString::from("-vn"),
            OsString::from("-c:a"),
            OsString::from("libmp3lame"),
            output.as_os_str().to_owned(),
        ]),
        ExportFormat::Ogg => args.extend([
            OsString::from("-i"),
            input.as_os_str().to_owned(),
            OsString::from("-vn"),
            OsString::from("-c:a"),
            OsString::from("libvorbis"),
            output.as_os_str().to_owned(),
        ]),
        ExportFormat::Flac => args.extend([
            OsString::from("-i"),
            input.as_os_str().to_owned(),
            OsString::from("-vn"),
            OsString::from("-c:a"),
            OsString::from("flac"),
            output.as_os_str().to_owned(),
        ]),
        ExportFormat::Mp4 => args.extend([
            OsString::from("-i"),
            input.as_os_str().to_owned(),
            OsString::from("-c:v"),
            OsString::from("libx264"),
            OsString::from("-pix_fmt"),
            OsString::from("yuv420p"),
            OsString::from("-an"),
            output.as_os_str().to_owned(),
        ]),
        _ => return Err(PackageError::UnsupportedExport),
    }
    Ok(args)
}

fn export_media(
    package: &Package,
    tgi: ResourceId,
    media_id: Option<u32>,
    format: ExportFormat,
    target: &Path,
    job_id: u64,
    tools: &MediaTools,
) -> Result<usize, PackageError> {
    let tool_name = match format {
        ExportFormat::Wav => "vgmstream",
        ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac | ExportFormat::Mp4 => "ffmpeg",
        _ => return Err(PackageError::UnsupportedExport),
    };
    let tool_path = required_tool(tools, format)?;
    let entry = package
        .entry(tgi)
        .ok_or(PackageError::ResourceNotFound(tgi))?;
    if u64::from(entry.decompressed_size) > RESOURCE_DECOMPRESSED_MAX {
        return Err(PackageError::DecompressionLimitExceeded(
            RESOURCE_DECOMPRESSED_MAX,
        ));
    }
    let data = package.read(entry)?;
    let (data, input_extension) = if tgi.type_id == WWISE_BANK_TYPE_ID {
        let bank = crate::wwise::WwiseBankData::parse(&data)?;
        let media = bank
            .bank
            .media
            .iter()
            .find(|entry| media_id.is_none_or(|id| id == entry.id))
            .ok_or(crate::wwise::WwiseError::NoMedia)?;
        (bank.wem(media.id)?.to_vec(), "wem")
    } else if tgi.type_id == VIDEO_TYPE_ID {
        (data, "vp6")
    } else {
        (data, "wem")
    };
    let parent = target
        .parent()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no parent".into()))?;
    let file_name = target
        .file_name()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no file name".into()))?
        .to_string_lossy();
    let output_extension = match format {
        ExportFormat::Wav => "wav",
        ExportFormat::Mp3 => "mp3",
        ExportFormat::Ogg => "ogg",
        ExportFormat::Flac => "flac",
        ExportFormat::Mp4 => "mp4",
        _ => return Err(PackageError::UnsupportedExport),
    };
    let input = parent.join(format!(".{file_name}.openscp-{job_id}.{input_extension}"));
    let decoded_wav = parent.join(format!(".{file_name}.openscp-{job_id}.decoded.wav"));
    let output = parent.join(format!(".{file_name}.openscp-{job_id}.{output_extension}"));
    let result = (|| -> Result<usize, PackageError> {
        create_temp_file(&input, &data)?;
        if matches!(
            format,
            ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac
        ) {
            let vgmstream = required_tool(tools, ExportFormat::Wav)?;
            let decode_args = media_args(ExportFormat::Wav, &input, &decoded_wav)?;
            media_tools::run("vgmstream", Path::new(vgmstream), &decode_args)?;
            let encode_args = media_args(format, &decoded_wav, &output)?;
            media_tools::run("ffmpeg", Path::new(tool_path), &encode_args)?;
        } else {
            let args = media_args(format, &input, &output)?;
            media_tools::run(tool_name, Path::new(tool_path), &args)?;
        }
        validate_media_output(&output, format)?;
        install_media_output(&output, target)
    })();
    let _ = fs::remove_file(&input);
    let _ = fs::remove_file(&decoded_wav);
    if result.is_err() {
        let _ = fs::remove_file(&output);
    }
    result
}

fn write_export(path: &Path, bytes: &[u8], job_id: u64) -> Result<(), PackageError> {
    validate_output_path(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no parent".into()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no file name".into()))?
        .to_string_lossy();
    let temporary = parent.join(format!(".{file_name}.openscp-{job_id}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let write_result = file.write_all(bytes).and_then(|()| file.sync_all());
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}

#[tauri::command]
pub async fn export(
    state: State<'_, AppState>,
    request: ExportRequest,
) -> Result<ExportAccepted, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    let app = state.app.clone();
    let package = manager
        .get(request.package_id)
        .map_err(CommandError::from)?;
    let output_path = PathBuf::from(&request.output_path);
    validate_output_path(&output_path).map_err(CommandError::from)?;
    let tgi = request.tgi.clone().into();
    validate_media_request(tgi, request.format).map_err(CommandError::from)?;
    let tools = media_tools::application_dir()
        .map(|directory| {
            media_tools::resolve_tools(&directory, std::env::var_os("PATH").as_deref())
        })
        .unwrap_or_else(|| media_tools::resolve_tools(Path::new("."), None));
    if matches!(
        request.format,
        ExportFormat::Wav
            | ExportFormat::Mp3
            | ExportFormat::Ogg
            | ExportFormat::Flac
            | ExportFormat::Mp4
    ) {
        required_tool(&tools, request.format).map_err(CommandError::from)?;
        if matches!(
            request.format,
            ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Flac
        ) {
            required_tool(&tools, ExportFormat::Wav).map_err(CommandError::from)?;
        }
    }
    let job_id = manager
        .reserve_job(request.output_path.clone())
        .map_err(CommandError::from)?;
    let accepted = ExportAccepted {
        job_id,
        output_path: request.output_path.clone(),
    };
    let jobs = Arc::clone(&manager);
    tauri::async_runtime::spawn_blocking(move || {
        let started = Instant::now();
        let operation_id = match store.start_operation("export", &request.output_path, None) {
            Ok(id) => id,
            Err(error) => {
                let _ = jobs.update_job(job_id, "failed", Some(error.to_string()));
                return;
            }
        };
        let _ = jobs.update_job(job_id, "running", None);
        emit_progress(
            &app,
            &store,
            operation_id,
            ExportProgress {
                job_id,
                phase: "running".into(),
                completed: 0,
                total: 1,
                tgi: Some(request.tgi.clone()),
                output_path: request.output_path.clone(),
            },
        );
        let result = match request.format {
            ExportFormat::Wav
            | ExportFormat::Mp3
            | ExportFormat::Ogg
            | ExportFormat::Flac
            | ExportFormat::Mp4 => export_media(
                &package,
                request.tgi.clone().into(),
                request.media_id,
                request.format,
                &output_path,
                job_id,
                &tools,
            ),
            _ => export_bytes(&package, request.tgi.clone().into(), request.format).and_then(
                |bytes| {
                    if bytes.len() as u64 > RESOURCE_DECOMPRESSED_MAX {
                        return Err(PackageError::OutputLimitExceeded(RESOURCE_DECOMPRESSED_MAX));
                    }
                    write_export(&output_path, &bytes, job_id).map(|()| bytes.len())
                },
            ),
        };
        match result {
            Ok(bytes_out) => {
                let _ = jobs.update_job(job_id, "succeeded", None);
                finish_operation(
                    &store,
                    operation_id,
                    "success",
                    started,
                    None,
                    i64::try_from(bytes_out).ok(),
                    None,
                );
                emit_progress(
                    &app,
                    &store,
                    operation_id,
                    ExportProgress {
                        job_id,
                        phase: "succeeded".into(),
                        completed: 1,
                        total: 1,
                        tgi: Some(request.tgi),
                        output_path: request.output_path,
                    },
                );
            }
            Err(error) => {
                finish_operation(
                    &store,
                    operation_id,
                    "failed",
                    started,
                    None,
                    None,
                    Some(&json!({"error": error.to_string()})),
                );
                emit_progress(
                    &app,
                    &store,
                    operation_id,
                    ExportProgress {
                        job_id,
                        phase: "failed".into(),
                        completed: 0,
                        total: 1,
                        tgi: Some(request.tgi),
                        output_path: request.output_path,
                    },
                );
                let _ = jobs.update_job(job_id, "failed", Some(error.to_string()));
            }
        }
    });
    Ok(accepted)
}

fn emit_progress(
    app: &AppHandle,
    store: &sc_store::Store,
    operation_id: i64,
    progress: ExportProgress,
) {
    if let Ok(event) = store.append_event(&EventInput {
        operation_id: Some(operation_id),
        level: if progress.phase == "failed" {
            "error".into()
        } else {
            "info".into()
        },
        topic: EXPORT_PROGRESS_EVENT.into(),
        message: progress.phase.clone(),
        payload: serde_json::to_value(&progress).ok(),
    }) {
        let _ = app.emit(EXPORT_PROGRESS_EVENT, &progress);
        let _ = app.emit(ACTIVITY_EVENT, &event);
    }
}

#[cfg(test)]
mod lot_payload_tests {
    use super::*;

    /// LOTM v2 金样本：EP1 静态模型 0x63D180B9（1 mesh / 1 material /
    /// 1 assignment）。多材质分组路径无法集成测试——EP1/DLC0/app 全部
    /// 2-mesh 模型均为蒙皮变体、decode_mesh 不支持（见 §21.5 开放问题），
    /// 绑定正确性由 rw4 real_package 金样本（0x41B1BAC0 双 assignment
    /// 字节）保证。
    #[test]
    fn lot_model_payload_v2_groups_meshes_by_assignment() {
        const EP1: &str = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../docs/packages/m3/SimCityDataEP1.package"
        );
        const MODEL: u32 = 0x63D1_80B9;
        let package = match dbpf::Package::open(EP1) {
            Ok(package) => package,
            Err(error) => {
                eprintln!("skipping: {EP1}: {error}");
                return;
            }
        };
        let manager = PackageManager::new();
        let entry = package
            .entries()
            .iter()
            .find(|e| e.id.type_id == RW4_MODEL_TYPE && e.id.instance == MODEL)
            .cloned()
            .expect("golden model in EP1");
        let data = package.read(&entry).unwrap();
        let file = rw4::Rw4File::parse(&data).unwrap();

        let started = std::time::Instant::now();
        let payload = build_lot_model_payload(&file, &data, &package, &manager, MODEL);
        let elapsed = started.elapsed();

        let read_u32 =
            |offset: usize| u32::from_le_bytes(payload[offset..offset + 4].try_into().unwrap());
        assert_eq!(read_u32(0), LOT_MODEL_PAYLOAD_MAGIC);
        assert_eq!(read_u32(4), 8, "container version 8");
        let mesh_count = read_u32(8) as usize;
        assert_eq!(mesh_count, 1, "1 exportable mesh");

        // 跳过 mesh GLB 段
        let mut offset = 12usize;
        for _ in 0..mesh_count {
            offset += 4 + read_u32(offset) as usize;
        }
        let material_count = read_u32(offset) as usize;
        offset += 4;
        assert_eq!(material_count, 1, "1 material via assignment");
        for _ in 0..material_count {
            for _ in 0..9 {
                // baseColor / normal / roughness / ao / tint / palette / shaderMap / interiorMap / reliefMap
                offset += 4 + read_u32(offset) as usize;
            }
            // params f32 + paramCols
            offset += 4 + read_u32(offset) as usize;
            offset += 4;
        }
        let mesh0_material = read_u32(offset) as usize;
        let mesh0_uv_kind = payload[offset + 4];
        offset += 5;
        // 诊断文本
        let diag_len = read_u32(offset) as usize;
        offset += 4;
        let diag = std::str::from_utf8(&payload[offset..offset + diag_len]).unwrap();
        eprintln!(
            "lot payload v8: {mesh_count} mesh, {material_count} material, mesh material = [{mesh0_material}], uv_kind = [{mesh0_uv_kind}], diag {diag_len} bytes, {} bytes in {elapsed:?}",
            payload.len()
        );
        eprintln!("{diag}");
        assert_eq!(mesh0_material, 0, "single mesh binds material 0");
        assert!(diag.contains("mesh #"), "diagnostics list meshes");
        assert!(diag.contains("slot0"), "diagnostics list slot0 params");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbpf::{OverlayEntry, write_uncompressed_overlay};

    #[test]
    fn tgi_filter_matches_hex_prefix_and_segments() {
        let entry = dbpf::IndexEntry {
            id: dbpf::ResourceId {
                type_id: 0x2F4E_681B,
                group: 0,
                instance: 0x1A2B,
            },
            unknown: 0,
            offset: 0,
            compressed_size: 0,
            decompressed_size: 0,
            flags: 0,
            compressed: false,
        };
        assert!(resource_matches(&entry, "0x2f4e681b"));
        assert!(resource_matches(&entry, "2f4e681b"));
        assert!(resource_matches(&entry, "0x2f4e681b:0:1a2b"));
        assert!(resource_matches(&entry, "2f4e681b:0:0x1a2b"));
        assert!(resource_matches(&entry, "793667611")); // type 十进制
        assert!(!resource_matches(&entry, "0x2f4e681c"));
        assert!(!resource_matches(&entry, "0x2f4e681b:0:1a2c"));
        // 单值匹配任意分量：00b1b104:098a44f2:0bbb7cef 应命中 instance
        let b1b104 = dbpf::IndexEntry {
            id: dbpf::ResourceId {
                type_id: 0x00B1_B104,
                group: 0x098A_44F2,
                instance: 0x0BBB_7CEF,
            },
            unknown: 0,
            offset: 0,
            compressed_size: 0,
            decompressed_size: 0,
            flags: 0,
            compressed: false,
        };
        assert!(resource_matches(&b1b104, "0x0bbb7cef"));
        assert!(resource_matches(&b1b104, "0x098a44f2"));
    }

    #[test]
    fn greyscale_header_matches_real_samples() {
        // app.package 0x03e421ec 实测：20B 大端头 [0, w, h, 1, byte_count] + 像素。
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&4u32.to_be_bytes());
        data.extend_from_slice(&2u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&[10, 40, 90, 200, 0, 30, 80, 160]);
        let decoded = decode_greyscale(&data).unwrap();
        assert_eq!((decoded.width, decoded.height), (4, 2));
        assert_eq!(decoded.image_kind, "greyscale");
        assert!(!decoded.png_base64.is_empty());
    }

    #[test]
    fn greyscale_rgba_variant_decodes() {
        // 0x03e421ed（"32-bit"）channel_code=2，实为 RGBA。
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&1u32.to_be_bytes());
        data.extend_from_slice(&2u32.to_be_bytes());
        data.extend_from_slice(&4u32.to_be_bytes());
        data.extend_from_slice(&[0xf9, 0x8a, 0x7e, 0xff]);
        let decoded = decode_greyscale(&data).unwrap();
        assert_eq!((decoded.width, decoded.height), (1, 1));
    }

    #[test]
    fn cursor_sample_decodes() {
        // app.package 0x02393756 实测头：ICO 目录 type=2、BITMAPINFOHEADER 32bpp。
        // 2×2 32bpp：stride = ((2*32+31)/32)*4 = 8，xor_len = 16。
        let mut data = Vec::new();
        data.extend_from_slice(&[0, 0, 2, 0, 1, 0]); // ICONDIR: cursor, 1 entry
        data.extend_from_slice(&[2, 2, 1, 0, 1, 0, 32, 0]); // entry: 2x2, 32bpp
        data.extend_from_slice(&57u32.to_le_bytes()); // size（含 AND mask）
        data.extend_from_slice(&22u32.to_le_bytes()); // offset = 6+16
        data.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER
        data.extend_from_slice(&2i32.to_le_bytes());
        data.extend_from_slice(&(-2i32).to_le_bytes());
        data.extend_from_slice(&[1, 0]); // planes
        data.extend_from_slice(&32u16.to_le_bytes()); // bpp
        data.extend_from_slice(&[0; 24]);
        // 2 行 × 8 字节 BGRA（bottom-up，行尾补齐）
        data.extend_from_slice(&[0x10, 0x20, 0x30, 0xff, 0x40, 0x50, 0x60, 0xff]);
        data.extend_from_slice(&[0x7e, 0x8a, 0xf9, 0xff, 0, 0, 0, 0]);
        data.push(0); // AND mask 1 字节
        let decoded = decode_cursor(&data).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(decoded.image_kind, "cursor");
    }

    /// 真实样本验证：样本目录由 dbpf examples/scan_preview_gaps 生成，
    /// 经 OPENSCP_PREVIEW_SAMPLES 指定（无则跳过）。
    #[test]
    #[ignore = "需要 OPENSCP_PREVIEW_SAMPLES 指向 dump 目录"]
    fn real_dump_samples_decode() {
        let Ok(dir) = std::env::var("OPENSCP_PREVIEW_SAMPLES") else {
            return;
        };
        let dir = PathBuf::from(dir);
        assert!(dir.is_dir(), "sample dir not found: {dir:?}");
        let mut decoded = 0;
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let data = std::fs::read(&path).unwrap();
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let result = if name.starts_with("cursor_") {
                decode_cursor(&data).map(|d| (d.width, d.height))
            } else if name.starts_with("grey8_") || name.starts_with("grey32_") {
                decode_greyscale(&data).map(|d| (d.width, d.height))
            } else {
                continue;
            };
            match result {
                Ok(dims) => {
                    println!("{name}: {dims:?}");
                    decoded += 1;
                }
                Err(error) => panic!("{name}: {error}"),
            }
        }
        assert!(decoded >= 7, "expected real samples, decoded {decoded}");
    }

    #[test]
    fn tga_uncompressed_24bpp_decodes() {
        let mut data = vec![0, 0, 2]; // no id, no cmap, uncompressed truecolor
        data.extend_from_slice(&[0; 9]); // cmap spec + origin
        data.extend_from_slice(&1u16.to_le_bytes()); // w
        data.extend_from_slice(&1u16.to_le_bytes()); // h
        data.push(24); // bpp
        data.push(0); // desc: bottom-up
        data.extend_from_slice(&[0x7e, 0x8a, 0xf9]); // BGR
        let decoded = decode_tga(&data).unwrap();
        assert_eq!((decoded.width, decoded.height), (1, 1));
        assert_eq!(decoded.image_kind, "tga");
    }

    fn test_package_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!("openscp-m5-{label}-{}.package", std::process::id()))
    }

    fn test_package(label: &str) -> (PathBuf, Package) {
        let path = test_package_path(label);
        let entries = [
            OverlayEntry::new(
                ResourceId {
                    type_id: 1,
                    group: 2,
                    instance: 3,
                },
                b"abcd",
            ),
            OverlayEntry::new(
                ResourceId {
                    type_id: 4,
                    group: 5,
                    instance: 6,
                },
                b"efgh",
            ),
        ];
        fs::write(&path, write_uncompressed_overlay(&entries).unwrap()).unwrap();
        let package = Package::open(&path).unwrap();
        (path, package)
    }

    #[test]
    fn page_limits_are_enforced() {
        assert_eq!(page_limit(None).unwrap(), RESOURCE_PAGE_DEFAULT_LIMIT);
        assert!(matches!(
            page_limit(Some(0)),
            Err(PackageError::InvalidArgument(_))
        ));
        assert!(matches!(
            page_limit(Some(RESOURCE_PAGE_MAX_LIMIT + 1)),
            Err(PackageError::LimitExceeded(_))
        ));
    }

    #[test]
    fn property_patches_replace_existing_values_without_changing_shape() {
        let mut file = sc_properties::PropertyFile {
            values: vec![sc_properties::Property {
                hash: 1,
                prop_type: sc_properties::PropType::UInt32,
                kind: sc_properties::Kind::Scalar(sc_properties::Value::UInt32(7)),
                encoding: sc_properties::PropertyEncoding::default(),
            }],
            claimed_count: 1,
        };
        let changed = apply_property_patches(
            &mut file,
            &[PropertyPatch {
                hash: 1,
                value: PropertyPatchValue::UInt32(42),
            }],
        )
        .unwrap();
        assert_eq!(changed, [1]);
        assert_eq!(
            file.get(1).unwrap().scalar(),
            Some(&sc_properties::Value::UInt32(42))
        );
    }

    #[test]
    fn property_patches_reject_duplicate_and_shape_changes() {
        let mut file = sc_properties::PropertyFile {
            values: vec![sc_properties::Property {
                hash: 1,
                prop_type: sc_properties::PropType::UInt32,
                kind: sc_properties::Kind::Scalar(sc_properties::Value::UInt32(7)),
                encoding: sc_properties::PropertyEncoding::default(),
            }],
            claimed_count: 1,
        };
        let duplicate = vec![
            PropertyPatch {
                hash: 1,
                value: PropertyPatchValue::UInt32(1),
            },
            PropertyPatch {
                hash: 1,
                value: PropertyPatchValue::UInt32(2),
            },
        ];
        assert!(matches!(
            apply_property_patches(&mut file, &duplicate),
            Err(PackageError::InvalidArgument(_))
        ));
        assert!(matches!(
            apply_property_patches(
                &mut file,
                &[PropertyPatch {
                    hash: 1,
                    value: PropertyPatchValue::Array(Vec::new())
                }]
            ),
            Err(PackageError::InvalidArgument(_))
        ));
    }

    #[test]
    fn lot_document_extracts_editor_references_and_preserves_unknowns() {
        let file = sc_properties::PropertyFile {
            values: vec![
                sc_properties::Property {
                    hash: sc_properties::LOD1_MODEL_HASH,
                    prop_type: sc_properties::PropType::Key,
                    kind: sc_properties::Kind::Scalar(sc_properties::Value::Key(
                        sc_properties::Key {
                            instance: 2,
                            type_id: 3,
                            group: 4,
                        },
                    )),
                    encoding: sc_properties::PropertyEncoding::default(),
                },
                sc_properties::Property {
                    hash: 0xDEAD_BEEF,
                    prop_type: sc_properties::PropType::UInt32,
                    kind: sc_properties::Kind::Scalar(sc_properties::Value::UInt32(9)),
                    encoding: sc_properties::PropertyEncoding::default(),
                },
            ],
            claimed_count: 2,
        };
        let document = sc_properties::LotEditorDocument::from_property_file(file.clone());
        assert_eq!(document.model.unwrap().instance, 2);
        assert_eq!(document.unknown_property_count, 1);
        assert_eq!(document.into_property_file(), file);
    }

    #[test]
    fn media_formats_match_original_contract() {
        let audio = ResourceId {
            type_id: AUDIO_TYPE_ID,
            group: 0,
            instance: 0,
        };
        let video = ResourceId {
            type_id: VIDEO_TYPE_ID,
            group: 0,
            instance: 0,
        };
        let bank = ResourceId {
            type_id: WWISE_BANK_TYPE_ID,
            group: 0,
            instance: 0,
        };
        assert!(validate_media_request(audio, ExportFormat::Wav).is_ok());
        assert!(validate_media_request(video, ExportFormat::Vp6).is_ok());
        assert!(validate_media_request(video, ExportFormat::Mp4).is_ok());
        assert!(validate_media_request(bank, ExportFormat::Wav).is_ok());
        assert!(matches!(
            validate_media_request(audio, ExportFormat::Mp4),
            Err(PackageError::UnsupportedExport)
        ));
    }

    #[test]
    fn ffmpeg_arguments_drop_audio_and_use_h264() {
        let args = media_args(
            ExportFormat::Mp4,
            Path::new("input.vp6"),
            Path::new("output.mp4"),
        )
        .unwrap();
        let args = args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "-y",
                "-nostdin",
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                "input.vp6",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-an",
                "output.mp4"
            ]
        );
    }
    #[test]
    fn media_output_signatures_are_checked() {
        let path =
            std::env::temp_dir().join(format!("openscp-media-output-{}.tmp", std::process::id()));
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&36u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8_000u32.to_le_bytes());
        wav.extend_from_slice(&16_000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&0u32.to_le_bytes());
        fs::write(&path, wav).unwrap();
        assert!(validate_media_output(&path, ExportFormat::Wav).is_ok());
        fs::write(&path, b"not-media").unwrap();
        assert!(matches!(
            validate_media_output(&path, ExportFormat::Wav),
            Err(PackageError::InvalidMediaOutput(_))
        ));
        fs::write(
            &path,
            [
                &16u32.to_be_bytes()[..],
                b"ftyp",
                b"isom",
                b"\x00\x00\x00\x00",
                &8u32.to_be_bytes()[..],
                b"mdat",
            ]
            .concat(),
        )
        .unwrap();
        assert!(validate_media_output(&path, ExportFormat::Mp4).is_ok());
        let _ = fs::remove_file(path);
    }
    #[test]
    fn byte_limits_are_fixed() {
        assert_eq!(RESOURCE_BYTES_MAX, 4096);
        assert_eq!(RESOURCE_DECOMPRESSED_MAX, 256 * 1024 * 1024);
    }

    #[test]
    fn package_handles_can_be_closed_and_jobs_are_queryable() {
        let (path, package) = test_package("lifecycle");
        let manager = PackageManager::new();
        let (package_id, _) = manager.insert(package).unwrap();
        assert!(manager.get(package_id).is_ok());
        manager.close(package_id).unwrap();
        assert!(matches!(
            manager.get(package_id),
            Err(PackageError::PackageNotFound(_))
        ));
        let job_id = manager.reserve_job("output.bin".into()).unwrap();
        manager
            .update_job(job_id, "failed", Some("test failure".into()))
            .unwrap();
        let status = manager.job(job_id).unwrap();
        assert_eq!(status.phase, "failed");
        assert_eq!(status.error.as_deref(), Some("test failure"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn texture_budget_uses_checked_dimensions() {
        let texture = rw4::DecodedTexture {
            texture_type: rw4::TEXTURE_TYPE_RAW_BGRA,
            unknown1: 0,
            width: 16_000,
            height: 16_000,
            mipmap_info: 0,
            data_section: 0,
            blob: Vec::new(),
        };
        assert!(matches!(
            validate_texture_budget(&texture),
            Err(PackageError::DecompressionLimitExceeded(_))
        ));
    }
    #[test]
    fn resource_pages_filter_and_slice_entries() {
        let (path, package) = test_package("page");
        let page = resource_page(&package, 0, 1, Some("00000001"), None, Vec::new()).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].tgi.instance, 3);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn type_counts_aggregate_and_type_filter_paginates() {
        let (path, package) = test_package("types");
        let counts = type_counts(&package, None);
        assert_eq!(counts.len(), 2);
        assert!(
            counts.iter().all(|entry| {
                entry.count == 1 && entry.name == format!("{:08X}", entry.type_id)
            })
        );
        let page = resource_page(&package, 0, 10, None, Some(4), counts).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].tgi.type_id, 4);
        let empty = resource_page(&package, 0, 10, None, Some(99), Vec::new()).unwrap();
        assert_eq!(empty.total, 0);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn resource_range_is_bounded() {
        let (path, package) = test_package("read");
        let tgi = ResourceId {
            type_id: 1,
            group: 2,
            instance: 3,
        };
        let result = read_resource(&package, tgi, 1, 2).unwrap();
        assert_eq!(result.bytes, b"bc");
        assert!(matches!(
            read_resource(&package, tgi, 3, 2),
            Err(PackageError::InvalidRange)
        ));
        assert!(matches!(
            read_resource(&package, tgi, 0, RESOURCE_BYTES_MAX + 1),
            Err(PackageError::LimitExceeded(_))
        ));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn raw_export_uses_logical_resource_bytes() {
        let (path, package) = test_package("export");
        let tgi = ResourceId {
            type_id: 4,
            group: 5,
            instance: 6,
        };
        assert_eq!(
            export_bytes(&package, tgi, ExportFormat::Raw).unwrap(),
            b"efgh"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_writes_via_sibling_temporary_file() {
        let (package_path, package) = test_package("output");
        let target = test_package_path("output-result");
        let bytes = export_bytes(
            &package,
            ResourceId {
                type_id: 1,
                group: 2,
                instance: 3,
            },
            ExportFormat::Raw,
        )
        .unwrap();
        write_export(&target, &bytes, 7).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"abcd");
        let _ = fs::remove_file(package_path);
        let _ = fs::remove_file(target);
    }

    #[test]
    fn export_progress_is_serializable() {
        let progress = ExportProgress {
            job_id: 1,
            phase: "running".into(),
            completed: 0,
            total: 1,
            tgi: None,
            output_path: "out.bin".into(),
        };
        assert_eq!(serde_json::to_value(progress).unwrap()["total"], 1);
    }

    #[test]
    fn registry_discovery_walks_ancestors_and_names_fall_back() {
        let root = test_package_path("registry-root");
        let nested = root.join("SimCityData");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.join("database_main.s3db"), b"stub").unwrap();
        assert_eq!(
            find_registry_database(&nested),
            Some(root.join("database_main.s3db"))
        );
        assert_eq!(
            find_registry_database(Path::new("Q:/definitely/missing")),
            None
        );
        assert_eq!(semantic_instance_name(None, 7), None);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn perf_type_counts_and_first_page_on_real_package() {
        let Some(path) = std::env::var_os("OPENSCP_PERF_PACKAGE").map(PathBuf::from) else {
            return;
        };
        let package = Package::open(&path).unwrap();
        let registry = path
            .parent()
            .and_then(|parent| {
                parent
                    .ancestors()
                    .find(|dir| dir.join("database_main.s3db").is_file())
            })
            .and_then(|dir| sc_registry::Registry::open(dir.join("database_main.s3db")).ok());
        let started = Instant::now();
        let counts = type_counts(&package, registry.as_ref());
        let counts_elapsed = started.elapsed();
        let started = Instant::now();
        let page = resource_page(&package, 0, 100, None, None, counts).unwrap();
        let page_elapsed = started.elapsed();
        for entry in &page.type_counts {
            println!(
                "type {:08X} {:>6}  {}",
                entry.type_id, entry.count, entry.name
            );
        }
        println!(
            "type_counts: {} types in {counts_elapsed:?}; first page: {} items in {page_elapsed:?}",
            page.type_counts.len(),
            page.items.len()
        );
    }

    #[test]
    fn image_resource_data_matches_file_signature() {
        let Some(path) = std::env::var_os("OPENSCP_PERF_PACKAGE").map(PathBuf::from) else {
            return;
        };
        let package = Package::open(&path).unwrap();
        let png = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x2F7D_0004)
            .expect("png entry");
        let data = package.read(png).unwrap();
        assert_eq!(&data[..4], &[0x89, 0x50, 0x4E, 0x47]);
        let jpg = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x3F86_62EA)
            .expect("jpg entry");
        let data = package.read(jpg).unwrap();
        assert_eq!(&data[..3], &[0xFF, 0xD8, 0xFF]);
        let gif = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x2F7D_0007)
            .expect("gif entry");
        let data = package.read(gif).unwrap();
        assert!(data.starts_with(b"GIF8"));
    }

    #[test]
    fn property_and_rw4_previews_parse_real_package() {
        let Some(path) = std::env::var_os("OPENSCP_PERF_PACKAGE").map(PathBuf::from) else {
            return;
        };
        let package = Package::open(&path).unwrap();
        let property = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x00B1_B104)
            .expect("property entry");
        let data = package.read(property).unwrap();
        let preview = property_preview(&data, None).unwrap();
        assert!(!preview.entries.is_empty());
        assert!(preview.entries.iter().any(|entry| !entry.value.is_empty()));

        let model = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == 0x2F4E_681B)
            .expect("rw4 entry");
        let data = package.read(model).unwrap();
        let preview = rw4_sections(&data).unwrap();
        assert!(!preview.sections.is_empty());
        let first = preview.sections[0].number;
        let detail = rw4_section_detail(&data, first).unwrap();
        assert_eq!(detail.number, first);
    }

    #[test]
    fn perf_real_media_exports_when_tools_are_configured() {
        let Some(audio_package_path) =
            std::env::var_os("OPENSCP_PERF_AUDIO_PACKAGE").map(PathBuf::from)
        else {
            return;
        };
        let Some(video_package_path) =
            std::env::var_os("OPENSCP_PERF_VIDEO_PACKAGE").map(PathBuf::from)
        else {
            return;
        };
        let tools =
            media_tools::resolve_tools(Path::new("missing"), std::env::var_os("PATH").as_deref());
        if !tools.ffmpeg.available || !tools.vgmstream.available {
            return;
        }
        let package = Package::open(&audio_package_path).unwrap();
        let audio = package
            .entries()
            .iter()
            .find(|entry| entry.id.type_id == AUDIO_TYPE_ID)
            .expect("audio entry");
        let audio_output = test_package_path("real-audio-wav");
        let _ = fs::remove_file(&audio_output);
        let audio_started = Instant::now();
        let audio_bytes = export_media(
            &package,
            audio.id,
            None,
            ExportFormat::Wav,
            &audio_output,
            991,
            &tools,
        )
        .unwrap();
        println!(
            "Wwise WAV: {audio_bytes} bytes in {:?}",
            audio_started.elapsed()
        );
        assert!(audio_bytes > 44);
        let _ = fs::remove_file(&audio_output);

        let video_package = Package::open(&video_package_path).unwrap();
        let video = video_package
            .entries()
            .iter()
            .find(|entry| {
                entry.id.type_id == VIDEO_TYPE_ID && entry.decompressed_size < 16 * 1024 * 1024
            })
            .expect("small VP6 entry");
        let video_output = test_package_path("real-video-mp4");
        let _ = fs::remove_file(&video_output);
        let video_started = Instant::now();
        let video_bytes = export_media(
            &video_package,
            video.id,
            None,
            ExportFormat::Mp4,
            &video_output,
            992,
            &tools,
        )
        .unwrap();
        println!(
            "VP6 MP4: {video_bytes} bytes in {:?}",
            video_started.elapsed()
        );
        assert!(video_bytes > 0);
        let _ = fs::remove_file(&video_output);
    }
}
