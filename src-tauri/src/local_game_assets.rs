//! @summary Read-only installed-game preview provider; persists identities, never extracted game payloads.
use std::{collections::HashMap, fs, io::Read, path::{Component, Path, PathBuf}, sync::{Arc, Mutex}, time::UNIX_EPOCH};
use base64::{Engine, engine::general_purpose::STANDARD};
use dbpf::{Package, ResourceId};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{Manager, State};
use crate::activity::CommandError;

pub(crate) const MAX_RESOURCE: u64 = 16 * 1024 * 1024;
const PROVENANCE: &str = "installed-game-direct; no-imported-payload";

/// @summary Serialize attachment changes and retain read-only package indexes for one generation.
#[derive(Default)]
pub struct LocalGameAssets(pub Arc<Mutex<ProviderSession>>);
/// @summary In-memory indexes over original installed packages; no content is extracted to disk.
#[derive(Default)]
pub(crate) struct ProviderSession {
    manifest: Option<Manifest>,
    pub(crate) packages: Vec<Package>,
    pub(crate) index: HashMap<ResourceId, (usize, usize)>,
}
/// @summary Explicit user-selected installation directory, with no extraction-source argument.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrepareRequest { pub game_data_path: String }
/// @summary Availability and content identity of an installed-game attachment.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetStatus {
    pub ready: bool,
    pub game_data_path: Option<String>,
    pub generation: Option<String>,
    pub asset_count: usize,
    pub provenance: Option<String>,
    pub available_locales: Vec<String>,
}
/// @summary A bounded non-executable original resource returned through the local command boundary.
#[derive(Serialize)]
pub struct AssetBytes { pub mime: String, pub base64: String }
/// @summary Identity metadata is the only persisted game-related data.
#[derive(Serialize, Deserialize, Clone)]
struct Identity { path: String, sha256: String, size: u64, modified_ns: u128 }
/// @summary Content-bound installation configuration; contains no game data or extracted preview files.
#[derive(Serialize, Deserialize, Clone)]
struct Manifest { schema: u32, generation: String, game_data_path: String, identities: Vec<Identity> }

/// @summary Create an actionable structured provider failure without leaking resource contents.
pub(crate) fn fail(message: impl Into<String>) -> CommandError { CommandError::new("local_assets_error", message) }
/// @summary Map local IO/parser failures to the stable provider error envelope.
pub(crate) fn err(error: impl std::fmt::Display) -> CommandError { fail(error.to_string()) }
/// @summary Compute the existing game filename-stem hash, not FNV-1a.
pub(crate) fn fnv(name: &str) -> u32 { name.to_ascii_lowercase().bytes().fold(0x811c9dc5, |h, b| h.wrapping_mul(0x01000193) ^ u32::from(b)) }

/// @summary Reject relative/network/device paths and every symbolic-link/reparse component.
fn safe_path(path: &Path) -> Result<PathBuf, CommandError> {
    if !path.is_absolute() { return Err(fail("An absolute local path is required")); }
    #[cfg(windows)]
    if let Some(Component::Prefix(prefix)) = path.components().next() {
        if !matches!(prefix.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)) { return Err(fail("Network and device namespace paths are forbidden")); }
    }
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::ParentDir | Component::CurDir) { return Err(fail("Relative components are forbidden")); }
        current.push(component.as_os_str());
        if matches!(component, Component::Prefix(_)) { continue; }
        let metadata = fs::symlink_metadata(&current).map_err(err)?;
        let mut forbidden = metadata.file_type().is_symlink();
        #[cfg(windows)] { use std::os::windows::fs::MetadataExt; forbidden |= metadata.file_attributes() & 0x400 != 0; }
        if forbidden { return Err(fail("Symbolic links and reparse points are forbidden")); }
    }
    fs::canonicalize(path).map_err(err)
}

/// @summary Collect installed packages without following links or entering mods, saves or caches.
fn package_paths(directory: &Path, depth: usize, result: &mut Vec<PathBuf>) -> Result<(), CommandError> {
    if depth > 8 || result.len() > 2048 { return Err(fail("Installed package tree exceeds provider bounds")); }
    for entry in fs::read_dir(directory).map_err(err)? {
        let path = safe_path(&entry.map_err(err)?.path())?;
        if path.is_dir() {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
            if ["mods", "simoder", "simcityuserdata", "saves", "backup", "backups", "cache"].contains(&name.as_str()) { continue; }
            package_paths(&path, depth + 1, result)?;
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("package")) { result.push(path); }
    }
    Ok(())
}
/// @summary Fingerprint a regular file and reject mutation while hashing.
fn identity(path: &Path, full: bool) -> Result<Identity, CommandError> {
    let path = safe_path(path)?;
    let mut file = fs::File::open(&path).map_err(err)?;
    let before = file.metadata().map_err(err)?;
    if !before.is_file() { return Err(fail("Game identities require regular files")); }
    let mut digest = Sha256::new();
    if full { let mut block = [0u8; 65536]; loop { let count = file.read(&mut block).map_err(err)?; if count == 0 { break; } digest.update(&block[..count]); } }
    let after = file.metadata().map_err(err)?;
    if before.len() != after.len() || before.modified().map_err(err)? != after.modified().map_err(err)? { return Err(fail("Game files changed during verification")); }
    Ok(Identity { path: path.to_string_lossy().into_owned(), sha256: if full { format!("{:x}", digest.finalize()) } else { String::new() }, size: after.len(), modified_ns: after.modified().map_err(err)?.duration_since(UNIX_EPOCH).map_err(err)?.as_nanos() })
}
/// @summary Verify the expected SimCity executable and package container markers before binding.
fn identities(game: &Path, full: bool) -> Result<Vec<Identity>, CommandError> {
    let game = safe_path(game)?;
    if !game.is_dir() || game.file_name().and_then(|x| x.to_str()).is_none_or(|n| !n.eq_ignore_ascii_case("SimCityData")) { return Err(fail("Select the installed SimCityData directory")); }
    let executable = game.parent().ok_or_else(|| fail("Invalid installation root"))?.join("SimCity").join("SimCity.exe");
    let executable = safe_path(&executable)?;
    let mut marker = [0u8; 2]; fs::File::open(&executable).map_err(err)?.read_exact(&mut marker).map_err(err)?;
    if &marker != b"MZ" { return Err(fail("SimCity.exe is not a Windows executable")); }
    let mut paths = Vec::new(); package_paths(&game, 0, &mut paths)?;
    if paths.is_empty() { return Err(fail("No installed game packages were found")); }
    paths.sort();
    for path in &paths {
        let mut marker = [0u8; 4]; fs::File::open(path).map_err(err)?.read_exact(&mut marker).map_err(err)?;
        if &marker != b"DBPF" && &marker != b"DBBF" { return Err(fail("An installed package has an invalid DBPF header")); }
    }
    paths.push(executable);
    paths.iter().map(|p| identity(p, full)).collect()
}

/// @summary Use application-local configuration storage, excluding repositories and reparse targets.
fn config_path(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    let directory = app.path().app_local_data_dir().map_err(err)?;
    if directory.ancestors().any(|p| p.join(".git").exists()) { return Err(fail("Provider configuration cannot be stored inside a repository")); }
    fs::create_dir_all(&directory).map_err(err)?;
    let directory = safe_path(&directory)?;
    Ok(directory.join("installed-game-preview.json"))
}
/// @summary Recheck complete identity on status/startup and fast metadata on each resource request.
fn verify(manifest: &Manifest, full: bool) -> Result<(), CommandError> {
    if manifest.schema != 2 { return Err(fail("Unsupported installed-game provider configuration; select the game again")); }
    let observed = identities(Path::new(&manifest.game_data_path), full)?;
    if observed.len() != manifest.identities.len() || observed.iter().zip(&manifest.identities).any(|(a,b)| a.path != b.path || a.size != b.size || a.modified_ns != b.modified_ns || (full && a.sha256 != b.sha256)) { return Err(fail("The installed game changed; explicitly select it again before previewing")); }
    let computed = generation(&manifest.identities)?;
    if computed != manifest.generation { return Err(fail("Provider configuration identity is inconsistent")); }
    Ok(())
}
/// @summary Bind a generation to exact executable and package bytes plus their stable identity metadata.
fn generation(files: &[Identity]) -> Result<String, CommandError> { Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(files).map_err(err)?))) }
/// @summary Classify package override tiers explicitly for deterministic static authoring previews.
fn priority(path: &str) -> (u8, String) {
    let name = Path::new(path).file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    let rank = if name.contains("ep1") { 4 } else if name.contains("dlc") { 3 } else if name.contains("graphics") { 2 } else if name.contains("game") { 1 } else { 0 };
    (rank, path.to_ascii_lowercase())
}
/// @summary Open read-only package indexes and resolve exact TGIs by deterministic override order.
fn initialize(session: &mut ProviderSession, manifest: Manifest) -> Result<(), CommandError> {
    let mut paths: Vec<_> = manifest.identities.iter().filter(|i| i.path.to_ascii_lowercase().ends_with(".package")).collect();
    paths.sort_by_key(|i| priority(&i.path));
    let mut packages = Vec::new(); let mut index = HashMap::new();
    for item in paths {
        let package = Package::open(safe_path(Path::new(&item.path))?).map_err(err)?;
        for (ordinal, entry) in package.entries().iter().enumerate() {
            // @summary Locale resources are selected by language in the model builder, never merged across languages here.
            if !Path::new(&item.path).components().any(|c| c.as_os_str().eq_ignore_ascii_case("Locale")) { index.insert(entry.id, (packages.len(), ordinal)); }
        }
        packages.push(package);
    }
    session.manifest = Some(manifest); session.packages = packages; session.index = index;
    Ok(())
}
/// @summary Load only persisted installation identities; legacy extracted caches are never consulted.
fn ensure(session: &mut ProviderSession, config: &Path, expected: Option<&str>, full: bool) -> Result<(), CommandError> {
    let path = safe_path(config)?;
    if fs::metadata(&path).map_err(err)?.len() > 1024 * 1024 { return Err(fail("Provider configuration exceeds 1 MiB")); }
    let manifest: Manifest = serde_json::from_slice(&fs::read(path).map_err(err)?).map_err(err)?;
    verify(&manifest, full || session.manifest.as_ref().is_none_or(|m| m.generation != manifest.generation))?;
    if expected.is_some_and(|g| g != manifest.generation) { return Err(fail("Preview generation changed; reload the preview")); }
    if session.manifest.as_ref().is_none_or(|m| m.generation != manifest.generation) { initialize(session, manifest)?; }
    Ok(())
}
/// @summary Determine installed language folders from actually opened locale packages.
pub(crate) fn available_locales(session: &ProviderSession) -> Vec<String> {
    let mut result = Vec::new();
    for package in &session.packages {
        let parts: Vec<_> = package.path().components().map(|p| p.as_os_str().to_string_lossy().into_owned()).collect();
        if let Some(position) = parts.iter().position(|s| s.eq_ignore_ascii_case("Locale")) {
            if let Some(locale) = parts.get(position + 1) { let normalized = normalize_locale(locale); if !result.contains(&normalized) { result.push(normalized); } }
        }
    }
    result.sort(); result
}
/// @summary Normalize actual locale directory identifiers without substituting an unavailable language.
pub(crate) fn normalize_locale(locale: &str) -> String {
    let parts: Vec<_> = locale.split(['-', '_']).collect();
    if parts.len() == 2 { format!("{}-{}", parts[0].to_ascii_lowercase(), parts[1].to_ascii_uppercase()) } else { locale.to_owned() }
}
/// @summary Summarize installation readiness without pretending that static previews are live simulation.
fn status(session: &ProviderSession) -> AssetStatus {
    let Some(manifest) = &session.manifest else { return AssetStatus::default(); };
    AssetStatus { ready: true, game_data_path: Some(manifest.game_data_path.clone()), generation: Some(manifest.generation.clone()), asset_count: session.index.len(), provenance: Some(PROVENANCE.into()), available_locales: available_locales(session) }
}
/// @summary Bounded exact original resource reads; never execute script bytes or export them to a file.
pub(crate) fn read(session: &ProviderSession, id: ResourceId) -> Result<Vec<u8>, CommandError> {
    let &(package, ordinal) = session.index.get(&id).ok_or_else(|| fail(format!("Installed resource unavailable: {id}")))?;
    let entry = &session.packages[package].entries()[ordinal];
    if u64::from(entry.decompressed_size) > MAX_RESOURCE || entry.stored_len() > MAX_RESOURCE { return Err(fail(format!("Installed resource exceeds 16 MiB: {id}"))); }
    let bytes = session.packages[package].read(entry).map_err(err)?;
    if bytes.len() as u64 > MAX_RESOURCE { return Err(fail("Installed resource exceeds 16 MiB")); }
    Ok(bytes)
}
/// @summary Exact typed references avoid guessing a resource's group or image identity.
pub(crate) fn typed(id: ResourceId) -> String { format!("installed-game:{:08X}:{:08X}:{:08X}", id.type_id, id.group, id.instance) }
/// @summary Parse three exact eight-digit hexadecimal identifiers, never a filesystem or network URL.
fn parse_typed(reference: &str) -> Result<Option<ResourceId>, CommandError> {
    let Some(value) = reference.strip_prefix("installed-game:") else { return Ok(None); };
    let parts: Vec<_> = value.split(':').collect();
    if parts.len() != 3 || parts.iter().any(|p| p.len() != 8 || !p.bytes().all(|b| b.is_ascii_hexdigit())) { return Err(fail("Invalid installed-game TGI reference")); }
    Ok(Some(ResourceId { type_id: u32::from_str_radix(parts[0],16).map_err(err)?, group: u32::from_str_radix(parts[1],16).map_err(err)?, instance: u32::from_str_radix(parts[2],16).map_err(err)? }))
}
/// @summary Resolve a validated image filename using the verified filename-stem convention only.
fn image_key(reference: &str) -> Result<ResourceId, CommandError> {
    if let Some(id) = parse_typed(reference)? { return Ok(id); }
    if reference.len() > 512 || !reference.is_ascii() || reference.contains([':', '\0', '?', '#', '%']) { return Err(fail("Invalid image reference")); }
    let name = reference.rsplit(['/', '\\']).next().unwrap_or_default();
    let (stem, extension) = name.rsplit_once('.').ok_or_else(|| fail("Image extension is required"))?;
    if stem.is_empty() || !stem.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'_'|b'-'|b' ')) { return Err(fail("Invalid image name")); }
    let type_id = match extension.to_ascii_lowercase().as_str() { "png" => 0x2F7D0004, "jpg"|"jpeg" => 0x3F8662EA, "gif" => 0x2F7D0007, _ => return Err(fail("Only PNG, JPEG and GIF images are supported")) };
    Ok(ResourceId { type_id, group:0, instance:fnv(stem) })
}
/// @summary Validate original binary type/signature before exposing it as a raster data URL.
fn image(session: &ProviderSession, reference: &str) -> Result<AssetBytes, CommandError> {
    let id = image_key(reference)?;
    let mime = match id.type_id { 0x2F7D0004 => "image/png", 0x3F8662EA => "image/jpeg", 0x2F7D0007 => "image/gif", _ => return Err(fail("Typed reference is not an allowed image type")) };
    let bytes = read(session,id)?;
    let valid = match mime { "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "image/jpeg" => bytes.starts_with(b"\xff\xd8\xff"), "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"), _=>false };
    if !valid { return Err(fail("Original image signature does not match its resource type")); }
    Ok(AssetBytes { mime:mime.into(),base64:STANDARD.encode(bytes) })
}

/// @summary Execute blocking package hashing/parsing off the async command executor.
async fn background<T:Send+'static>(f:impl FnOnce()->Result<T,CommandError>+Send+'static)->Result<T,CommandError>{tauri::async_runtime::spawn_blocking(f).await.map_err(err)?}
/// @summary Configure the selected installation and persist only content-identity metadata.
#[tauri::command]
pub async fn local_game_assets_prepare(app:tauri::AppHandle,state:State<'_,LocalGameAssets>,request:PrepareRequest)->Result<AssetStatus,CommandError>{
    let shared=Arc::clone(&state.0); background(move||{
        let mut session=shared.lock().map_err(err)?;
        let game=safe_path(Path::new(&request.game_data_path))?;let files=identities(&game,true)?;
        let manifest=Manifest{schema:2,generation:generation(&files)?,game_data_path:game.to_string_lossy().into_owned(),identities:files};
        let config=config_path(&app)?; initialize(&mut session,manifest.clone())?;
        crate::atomic_fs::write_atomic(&config,&serde_json::to_vec_pretty(&manifest).map_err(err)?,true).map_err(err)?;
        Ok(status(&session))
    }).await
}
/// @summary Fully verify persisted installation identities and expose actual installed locales.
#[tauri::command]
pub async fn local_game_assets_status(app:tauri::AppHandle,state:State<'_,LocalGameAssets>)->Result<AssetStatus,CommandError>{
    let shared=Arc::clone(&state.0);background(move||{let config=config_path(&app)?;if !config.exists(){return Ok(AssetStatus::default())}let mut session=shared.lock().map_err(err)?;ensure(&mut session,&config,None,true)?;Ok(status(&session))}).await
}
/// @summary Read only an allowed original image by exact typed identity or validated filename.
#[tauri::command]
pub async fn local_game_assets_resource(app:tauri::AppHandle,state:State<'_,LocalGameAssets>,reference:String,generation:String)->Result<AssetBytes,CommandError>{
    let shared=Arc::clone(&state.0);background(move||{let mut session=shared.lock().map_err(err)?;ensure(&mut session,&config_path(&app)?,Some(&generation),false)?;image(&session,&reference)}).await
}
/// @summary Read original CSS by exact typed resource identity; filename lookup must be unambiguous.
#[tauri::command]
pub async fn local_game_assets_stylesheet(app:tauri::AppHandle,state:State<'_,LocalGameAssets>,name:String,generation:String)->Result<AssetBytes,CommandError>{
    let shared=Arc::clone(&state.0);background(move||{let mut session=shared.lock().map_err(err)?;ensure(&mut session,&config_path(&app)?,Some(&generation),false)?;
        let id=if let Some(id)=parse_typed(&name)?{id}else{
            if name.len()>128||!name.is_ascii()||!name.bytes().all(|b|b.is_ascii_alphanumeric()||matches!(b,b'_'|b'-'|b'.')){return Err(fail("Invalid stylesheet name"))}
            let stem=name.strip_suffix(".css").unwrap_or(&name);let hash=fnv(stem);let matches:Vec<_>=session.index.keys().filter(|id|id.type_id==0x2C978DB6&&id.instance==hash).copied().collect();
            if matches.len()!=1{return Err(fail("Stylesheet name is unavailable or ambiguous; use its typed reference"))}matches[0]
        };
        if id.type_id!=0x2C978DB6{return Err(fail("Reference is not an original stylesheet"))}let bytes=read(&session,id)?;std::str::from_utf8(&bytes).map_err(err)?;Ok(AssetBytes{mime:"text/css".into(),base64:STANDARD.encode(bytes)})
    }).await
}
/// @summary Assemble renderer data directly from validated installed declarative layouts, properties and locale tables.
#[tauri::command]
pub async fn local_game_assets_model(app:tauri::AppHandle,state:State<'_,LocalGameAssets>,locale:String,generation:String)->Result<serde_json::Value,CommandError>{
    let shared=Arc::clone(&state.0);background(move||{let mut session=shared.lock().map_err(err)?;ensure(&mut session,&config_path(&app)?,Some(&generation),false)?;crate::installed_game_preview::assemble(&session,&normalize_locale(&locale),&generation)}).await
}

#[cfg(test)]
mod tests {
    use super::*;
    /// @summary Exact typed references retain groups and reject filesystem/script schemes.
    #[test] fn typed_references_are_narrow(){let id=parse_typed("installed-game:2F7D0004:40E02400:12345678").unwrap().unwrap();assert_eq!(id.group,0x40E02400);for r in ["installed-game:2F7D0004:0:1","installed-game:67771F5C:00000000:12345678:extra"]{assert!(parse_typed(r).is_err())}assert!(image_key("file:///x.png").is_err());assert!(image_key("x.png?url").is_err());assert_eq!(image_key("puck-base.png").unwrap().instance,0x7AF20E3C);}
    /// @summary A detached provider has no package or resource authority.
    #[test] fn detached_reads_fail(){assert!(read(&ProviderSession::default(),ResourceId{type_id:0x2F7D0004,group:0,instance:1}).is_err())}
    /// @summary Removed extraction arguments are rejected rather than silently enabling the retired import route.
    #[test] fn configuration_has_no_extraction_source(){assert!(serde_json::from_value::<PrepareRequest>(serde_json::json!({"gameDataPath":"C:/Game/SimCityData","sourcePath":"C:/old/extraction"})).is_err());assert!(serde_json::from_value::<PrepareRequest>(serde_json::json!({"gameDataPath":"C:/Game/SimCityData"})).is_ok());}
    /// @summary Windows shares and device namespaces cannot become original-game providers.
    #[cfg(windows)] #[test] fn network_roots_are_forbidden(){for p in [r"\\server\share\SimCityData",r"\\?\UNC\server\share\SimCityData",r"\\.\PhysicalDrive0"]{assert!(safe_path(Path::new(p)).unwrap_err().message.contains("Network and device"));}}
    /// @summary Read-only installed-game test needs only installation selection, never an extracted cache.
    #[test] #[ignore="requires OPEN_SCP_GAME_DATA and explicit local installation access"]
    fn installed_game_direct_model_smoke(){let game=std::env::var("OPEN_SCP_GAME_DATA").unwrap();let files=identities(Path::new(&game),true).unwrap();let manifest=Manifest{schema:2,generation:generation(&files).unwrap(),game_data_path:game,identities:files};let generation=manifest.generation.clone();let mut session=ProviderSession::default();initialize(&mut session,manifest).unwrap();let locale=available_locales(&session).into_iter().next().unwrap();let model=crate::installed_game_preview::assemble(&session,&locale,&generation).unwrap();assert!(!model["model"]["categories"].as_array().unwrap().is_empty());assert_eq!(image(&session,"puck-base.png").unwrap().mime,"image/png");println!("DIRECT_PREVIEW_SMOKE locale={locale} categories={} tools={} styles={} provenance={PROVENANCE}",model["model"]["categories"].as_array().unwrap().len(),model["tools"].as_object().unwrap().values().map(|a|a.as_array().unwrap().len()).sum::<usize>(),model["requiredStyleClasses"].as_array().unwrap().len());println!("DIRECT_PREVIEW_METADATA {}",serde_json::json!({"drawableTypes":model["originalDrawableTypeCounts"],"stylesheets":model["stylesheets"],"diagnostics":model["diagnostics"]}));}
}
