use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
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

    fn get(&self, id: u64) -> Result<Arc<Package>, PackageError> {
        self.packages
            .lock()
            .map_err(|_| PackageError::StatePoisoned)?
            .get(&id)
            .cloned()
            .ok_or(PackageError::PackageNotFound(id))
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
    Tga,
    Dds,
    Wav,
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
enum PackageError {
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
    #[error("properties error: {0}")]
    Properties(#[from] sc_properties::Error),
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
            Self::Properties(_) => "corrupt_resource",
            Self::Registry(_) => "registry",
            Self::Rw4(_) => "corrupt_resource",
            Self::UnsupportedExport => "unsupported",
            Self::Exporter(_) => "export_failed",
            Self::ToolNotFound(_) => "tool_not_found",
            Self::ToolSpawn(ToolError::Spawn { .. }) => "tool_spawn_failed",
            Self::ToolSpawn(ToolError::Failed { .. }) => "tool_failed",
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
    text.contains(filter)
        || id.type_id.to_string() == filter
        || id.group.to_string() == filter
        || id.instance.to_string() == filter
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

fn type_counts(
    package: &Package,
    registry: Option<&sc_registry::Registry>,
) -> Vec<TypeCount> {
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
        use base64::{engine::general_purpose::STANDARD, Engine as _};
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
            if let Some(cut) = text.char_indices().nth(PROPERTY_PREVIEW_MAX_VALUES).map(|(i, _)| i)
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
    (
        any.then_some(min),
        any.then_some(max),
    )
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
            use base64::{engine::general_purpose::STANDARD, Engine as _};
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
        use base64::{engine::general_purpose::STANDARD, Engine as _};
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
            let registry =
                package_registry(store, manager, package, bundled_registry.as_deref());
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
    read_resource_with(manager, store, request.package_id, request.tgi, |data, _, _, _| {
        rw4_sections(data)
    })
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
    let user = user
        .is_file()
        .then(|| user.to_string_lossy().into_owned());
    (main, user)
}

fn bundled_registry_path(app: &AppHandle) -> Option<PathBuf> {
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
    candidates
        .iter()
        .find_map(|(main, user)| {
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
        let registry =
            package_registry(&store, &manager, &package, bundled_registry.as_deref());
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
        ExportFormat::Wav | ExportFormat::Mp4 => Err(PackageError::UnsupportedExport),
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
const VIDEO_TYPE_ID: u32 = 0x3768_40D7;

fn required_tool(tools: &MediaTools, format: ExportFormat) -> Result<&str, PackageError> {
    let tool = match format {
        ExportFormat::Wav => &tools.vgmstream,
        ExportFormat::Mp4 => &tools.ffmpeg,
        _ => return Err(PackageError::UnsupportedExport),
    };
    tool.path
        .as_deref()
        .ok_or(PackageError::ToolNotFound(match format {
            ExportFormat::Wav => "vgmstream",
            ExportFormat::Mp4 => "ffmpeg",
            _ => "media tool",
        }))
}

fn validate_media_request(tgi: ResourceId, format: ExportFormat) -> Result<(), PackageError> {
    match format {
        ExportFormat::Wav if tgi.type_id == AUDIO_TYPE_ID => Ok(()),
        ExportFormat::Vp6 | ExportFormat::Mp4 if tgi.type_id == VIDEO_TYPE_ID => Ok(()),
        ExportFormat::Wav | ExportFormat::Vp6 | ExportFormat::Mp4 => {
            Err(PackageError::UnsupportedExport)
        }
        _ => Ok(()),
    }
}

fn create_temp_file(path: &Path, bytes: &[u8]) -> Result<(), PackageError> {
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn validate_media_output(path: &Path, format: ExportFormat) -> Result<(), PackageError> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > RESOURCE_DECOMPRESSED_MAX {
        return Err(PackageError::InvalidMediaOutput(
            "media output size is invalid",
        ));
    }
    let mut file = fs::File::open(path)?;
    let mut header = [0u8; 12];
    let count = file.read(&mut header)?;
    let valid = match format {
        ExportFormat::Wav => count >= 12 && &header[..4] == b"RIFF" && &header[8..12] == b"WAVE",
        ExportFormat::Mp4 => count >= 8 && &header[4..8] == b"ftyp",
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

fn media_args(
    format: ExportFormat,
    input: &Path,
    output: &Path,
) -> Result<Vec<OsString>, PackageError> {
    match format {
        ExportFormat::Wav => Ok(vec![
            OsString::from("-o"),
            output.as_os_str().to_owned(),
            input.as_os_str().to_owned(),
        ]),
        ExportFormat::Mp4 => Ok(vec![
            OsString::from("-y"),
            OsString::from("-hide_banner"),
            OsString::from("-loglevel"),
            OsString::from("error"),
            OsString::from("-i"),
            input.as_os_str().to_owned(),
            OsString::from("-c:v"),
            OsString::from("libx264"),
            OsString::from("-pix_fmt"),
            OsString::from("yuv420p"),
            OsString::from("-an"),
            output.as_os_str().to_owned(),
        ]),
        _ => Err(PackageError::UnsupportedExport),
    }
}

fn export_media(
    package: &Package,
    tgi: ResourceId,
    format: ExportFormat,
    target: &Path,
    job_id: u64,
    tools: &MediaTools,
) -> Result<usize, PackageError> {
    let tool_name = match format {
        ExportFormat::Wav => "vgmstream",
        ExportFormat::Mp4 => "ffmpeg",
        _ => "media tool",
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
    let parent = target
        .parent()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no parent".into()))?;
    let file_name = target
        .file_name()
        .ok_or_else(|| PackageError::InvalidArgument("output path has no file name".into()))?
        .to_string_lossy();
    let output_extension = match format {
        ExportFormat::Wav => "wav",
        ExportFormat::Mp4 => "mp4",
        _ => return Err(PackageError::UnsupportedExport),
    };
    let input = parent.join(format!(".{file_name}.openscp-{job_id}.input"));
    let output = parent.join(format!(".{file_name}.openscp-{job_id}.{output_extension}"));
    let result = (|| -> Result<usize, PackageError> {
        create_temp_file(&input, &data)?;
        let args = media_args(format, &input, &output)?;
        media_tools::run(tool_name, Path::new(tool_path), &args)?;
        validate_media_output(&output, format)?;
        install_media_output(&output, target)
    })();
    let _ = fs::remove_file(&input);
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
    if matches!(request.format, ExportFormat::Wav | ExportFormat::Mp4) {
        required_tool(&tools, request.format).map_err(CommandError::from)?;
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
            ExportFormat::Wav | ExportFormat::Mp4 => export_media(
                &package,
                request.tgi.clone().into(),
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
mod tests {
    use super::*;
    use dbpf::{OverlayEntry, write_uncompressed_overlay};

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
        assert!(validate_media_request(audio, ExportFormat::Wav).is_ok());
        assert!(validate_media_request(video, ExportFormat::Vp6).is_ok());
        assert!(validate_media_request(video, ExportFormat::Mp4).is_ok());
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
        fs::write(&path, b"RIFF0000WAVEfmt ").unwrap();
        assert!(validate_media_output(&path, ExportFormat::Wav).is_ok());
        fs::write(&path, b"not-media").unwrap();
        assert!(matches!(
            validate_media_output(&path, ExportFormat::Wav),
            Err(PackageError::InvalidMediaOutput(_))
        ));
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
        assert!(counts.iter().all(|entry| {
            entry.count == 1 && entry.name == format!("{:08X}", entry.type_id)
        }));
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
            .and_then(|parent| parent.ancestors().find(|dir| dir.join("database_main.s3db").is_file()))
            .and_then(|dir| sc_registry::Registry::open(dir.join("database_main.s3db")).ok());
        let started = Instant::now();
        let counts = type_counts(&package, registry.as_ref());
        let counts_elapsed = started.elapsed();
        let started = Instant::now();
        let page = resource_page(&package, 0, 100, None, None, counts).unwrap();
        let page_elapsed = started.elapsed();
        for entry in &page.type_counts {
            println!("type {:08X} {:>6}  {}", entry.type_id, entry.count, entry.name);
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
}
