//! Versioned project manifest -> validated intermediate model -> DBPF overlay.
//! The engine reads schema files itself; UI validation never authorizes a build.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

pub const FORMAT: &str = "openscp.mod";
/// Stable resource locations shared by project creation, existing projects and schemas.
pub fn ensure_project_layout(root: &Path, mod_type: &str) -> Result<Value, String> {
    let assets = match mod_type {
        "map" => vec![
            "assets/maps/heightmaps",
            "assets/maps/resources",
            "assets/maps/roads",
            "assets/maps/vegetation",
            "assets/maps/water",
            "assets/maps/metadata",
        ],
        "code" => vec!["scripts", "assets/config"],
        "assets" => vec![
            "assets/models",
            "assets/textures",
            "assets/materials",
            "assets/audio",
        ],
        "gameplay" => vec!["assets/properties", "assets/tuning", "assets/localization"],
        _ => return Err("Unknown mod type".into()),
    };
    let schemas = format!("schemas/{mod_type}");
    for relative in assets.iter().copied().chain([schemas.as_str(), "build"]) {
        fs::create_dir_all(safe_path(root, relative)?).map_err(|e| e.to_string())?;
    }
    Ok(json!({"schemas":schemas,"assets":assets,"output":"build"}))
}
pub(crate) static FLOW_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub fn dispatch(root: &Path, name: &str, action: &str, payload: Value) -> Result<Value, String> {
    let _guard = FLOW_LOCK.lock().map_err(|_| "Workflow lock failed")?;
    let string = |key: &str| payload[key].as_str().unwrap_or("");
    match action {
        "inspect" => serde_json::to_value(inspect(root)?).map_err(|e| e.to_string()),
        "initialize" => serde_json::to_value(initialize(
            root,
            name,
            string("author"),
            string("description"),
            string("mod_type"),
            string("workflow"),
        )?)
        .map_err(|e| e.to_string()),
        "save-node" => serde_json::to_value(save_node(
            root,
            string("revision"),
            serde_json::from_value(payload["schema"].clone()).map_err(|e| e.to_string())?,
        )?)
        .map_err(|e| e.to_string()),
        "build" => build(
            root,
            &serde_json::from_value(payload["im"].clone()).map_err(|e| e.to_string())?,
        ),
        "save-graph" => {
            let state = inspect(root)?;
            if state.revision != string("revision") {
                return Err("Project changed; refresh first".into());
            }
            let nodes: Vec<Node> =
                serde_json::from_value(payload["nodes"].clone()).map_err(|e| e.to_string())?;
            if nodes.len() != state.nodes.len()
                || nodes.iter().map(|n| &n.id).collect::<BTreeSet<_>>().len() != nodes.len()
                || nodes.iter().any(|n| {
                    !n.position.iter().all(|v| v.is_finite())
                        || !state
                            .nodes
                            .iter()
                            .any(|o| o.id == n.id && o.kind == n.kind && o.schema == n.schema)
                })
            {
                return Err("Graph cannot change node identities".into());
            }
            let deps: BTreeMap<String, Vec<String>> =
                serde_json::from_value(payload["internal_dependencies"].clone())
                    .map_err(|e| e.to_string())?;
            let mut manifest = state.manifest;
            manifest["nodes"] = json!(nodes);
            manifest["internal_dependencies"] = json!(deps);
            save_json(root, "package.json", &manifest, true)?;
            serde_json::to_value(inspect(root)?).map_err(|e| e.to_string())
        }
        _ => Err("Unknown workflow action".into()),
    }
}

/// Index package headers, not decompressed payloads. Unknown source coverage is
/// reported explicitly; a TGI in a community package alone is not an override.
pub fn detect_overrides(root: &Path, game: Option<&Path>) -> Value {
    fn packages(root: &Path, depth: usize, out: &mut Vec<PathBuf>, incomplete: &mut bool) {
        if depth > 8 || out.len() >= 10000 {
            *incomplete = true;
            return;
        }
        let Ok(entries) = fs::read_dir(root) else {
            *incomplete = true;
            return;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                *incomplete = true;
                continue;
            };
            let path = entry.path();
            let Ok(m) = fs::symlink_metadata(&path) else {
                *incomplete = true;
                continue;
            };
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    continue;
                }
            }
            if m.file_type().is_symlink() {
                continue;
            }
            if m.is_dir() {
                if !["build", ".git", "node_modules"]
                    .contains(&entry.file_name().to_string_lossy().as_ref())
                {
                    packages(&path, depth + 1, out, incomplete);
                }
            } else if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("package"))
            {
                out.push(path);
            }
        }
    }
    let Some(game) = game.filter(|g| g.is_dir()) else {
        return json!({"status":"unavailable","mappings":[]});
    };
    let mut incomplete = false;
    let mut mods = vec![];
    packages(root, 0, &mut mods, &mut incomplete);
    let mut mod_entries = vec![];
    let mut wanted = BTreeSet::new();
    for path in mods {
        match dbpf::Package::open(&path) {
            Ok(p) => {
                for e in p.entries() {
                    let tgi = format!(
                        "{:08X}:{:08X}:{:08X}",
                        e.id.type_id, e.id.group, e.id.instance
                    );
                    wanted.insert(tgi.clone());
                    mod_entries.push((path.clone(), tgi));
                }
            }
            Err(_) => incomplete = true,
        }
    }
    let mut sources = vec![];
    packages(game, 0, &mut sources, &mut incomplete);
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in sources {
        match dbpf::Package::open(&path) {
            Ok(p) => {
                for e in p.entries() {
                    if !wanted.contains(&format!(
                        "{:08X}:{:08X}:{:08X}",
                        e.id.type_id, e.id.group, e.id.instance
                    )) {
                        continue;
                    }
                    index
                        .entry(format!(
                            "{:08X}:{:08X}:{:08X}",
                            e.id.type_id, e.id.group, e.id.instance
                        ))
                        .or_default()
                        .push(path.to_string_lossy().into_owned());
                }
            }
            Err(_) => incomplete = true,
        }
    }
    let mut mappings = vec![];
    for (path, tgi) in mod_entries {
        if let Some(source) = index.get(&tgi) {
            mappings.push(json!({"package":path.strip_prefix(root).unwrap_or(&path).to_string_lossy().replace('\\',"/"),"tgi":tgi,"source_packages":source}));
        }
    }
    json!({"status":if incomplete{"partial"}else{"complete"},"mappings":mappings})
}
pub fn engine_manifest(v: &Value) -> bool {
    v["format"] == FORMAT
        && v["manifest_version"] == 2
        && v["origin"] == "openscp"
        && v["engine"]["enabled"] == true
        && v["engine"]["version"] == 1
}
pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn file_digest(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn safe_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    if relative.is_empty() || relative.contains(['\\', ':', '\0']) {
        return Err("Invalid project path".into());
    }
    let mut path = root.to_path_buf();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err("Path escapes project".into());
        }
        path.push(part);
        if let Ok(m) = fs::symlink_metadata(&path) {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if m.file_attributes() & 0x400 != 0 {
                    return Err("Reparse point is not allowed".into());
                }
            }
            if m.file_type().is_symlink() {
                return Err("Symbolic link is not allowed".into());
            }
        }
    }
    Ok(path)
}
fn read_json(path: &Path) -> Result<Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("JSON exceeds 4 MiB".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn save_json(root: &Path, relative: &str, v: &Value, replace: bool) -> Result<(), String> {
    let path = safe_path(root, relative)?;
    let bytes = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    crate::atomic_fs::write_atomic(&path, &bytes, replace).map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub schema: String,
    pub kind: String,
    pub position: [f64; 2],
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schema {
    pub schema_version: u32,
    pub id: String,
    pub kind: String,
    pub config: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntermediateModel {
    pub version: u32,
    pub manifest_hash: String,
    pub order: Vec<String>,
    pub schemas: Vec<Schema>,
    pub inputs: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub node: Option<String>,
    pub code: String,
    pub detail: String,
}
#[derive(Serialize)]
pub struct FlowState {
    pub manifest: Value,
    pub revision: String,
    pub nodes: Vec<Node>,
    pub schemas: Vec<Schema>,
    pub diagnostics: Vec<Diagnostic>,
    pub im: Option<IntermediateModel>,
}

pub fn initialize(
    root: &Path,
    name: &str,
    author: &str,
    description: &str,
    mod_type: &str,
    mode: &str,
) -> Result<FlowState, String> {
    if !["map", "code", "assets", "gameplay"].contains(&mod_type) {
        return Err("Unknown mod type".into());
    }
    if !["original", "whole-region", "generated", "assets"].contains(&mode) {
        return Err("Unknown workflow".into());
    }
    if (mod_type == "map") == (mode == "assets") {
        return Err("Workflow does not match mod type".into());
    }
    if author.trim().is_empty() || description.trim().is_empty() {
        return Err("Author and description are required".into());
    }
    let manifest_path = safe_path(root, "package.json")?;
    let previous = if manifest_path.exists() {
        read_json(&manifest_path)?
    } else {
        json!({})
    };
    if !previous.is_object() {
        return Err("Manifest must be an object".into());
    }
    if engine_manifest(&previous) {
        return Err("Project already has an engine workflow".into());
    }
    let kinds: Vec<&str> = match mode {
        "original" => vec![
            "map-source",
            "height-edit",
            "city-slots",
            "resources",
            "roads",
            "map-metadata",
            "output",
        ],
        "whole-region" => vec![
            "map-source",
            "city-slots",
            "coordinate-alignment",
            "resources",
            "map-metadata",
            "output",
        ],
        "generated" => vec![
            "map-source",
            "noise-height",
            "city-slots",
            "resources",
            "roads",
            "map-metadata",
            "output",
        ],
        _ => vec!["static-resource", "output"],
    };
    let layout = ensure_project_layout(root, mod_type)?;
    let schema_dir = layout["schemas"].as_str().unwrap();
    for (i, kind) in kinds.iter().enumerate() {
        if safe_path(root, &format!("{schema_dir}/{kind}-{}.json", i + 1))?.exists() {
            return Err("Schema file already exists; initialization did not replace it".into());
        }
    }
    let mut nodes = vec![];
    let mut deps = BTreeMap::new();
    let mut schema_files = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        let id = format!("{kind}-{}", i + 1);
        let schema = format!("{schema_dir}/{id}.json");
        let config = match *kind {
            "map-source" => json!({"package":"","group":""}),
            "height-edit" => json!({"delta_meters":0}),
            "noise-height" => {
                json!({"seed":1,"amplitude_meters":80,"wavelength_meters":2048,"base_meters":-850})
            }
            "static-resource" => json!({"asset":"","tgi":"","source_package":""}),
            "output" => json!({"file":"mod.package"}),
            "coordinate-alignment" => json!({"action":"configure"}),
            _ => json!({"action":if mode=="original"{"preserve"}else{"configure"}}),
        };
        schema_files.push((
            schema.clone(),
            json!(Schema {
                schema_version: 1,
                id: id.clone(),
                kind: kind.to_string(),
                config
            }),
        ));
        deps.insert(
            id.clone(),
            if i == 0 {
                vec![]
            } else {
                vec![nodes.last().map(|n: &Node| n.id.clone()).unwrap()]
            },
        );
        nodes.push(Node {
            id,
            schema,
            kind: kind.to_string(),
            position: [(i % 3) as f64 * 290. + 40., (i / 3) as f64 * 210. + 60.],
        });
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let manifest = json!({"format":FORMAT,"manifest_version":2,"name":name,"version":"0.1.0","description":description,"author":author,"mod_type":mod_type,"origin":"openscp","created_at":now,"engine":{"enabled":true,"version":1},"workflow":mode,"nodes":nodes,"internal_dependencies":deps,"outer_dependencies":[],"resource_layout":layout,"legacy_metadata":previous});
    let mut created = Vec::new();
    let result = (|| {
        for (file, value) in &schema_files {
            save_json(root, file, value, false)?;
            created.push(safe_path(root, file)?);
        }
        save_json(root, "package.json", &manifest, true)
    })();
    if let Err(error) = result {
        for path in created {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    inspect(root)
}

fn diagnostic(
    out: &mut Vec<Diagnostic>,
    node: Option<&str>,
    code: &str,
    detail: impl Into<String>,
) {
    out.push(Diagnostic {
        node: node.map(str::to_string),
        code: code.into(),
        detail: detail.into(),
    });
}
pub fn inspect(root: &Path) -> Result<FlowState, String> {
    let manifest = read_json(&safe_path(root, "package.json")?)?;
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let mut revision = digest(&bytes);
    if !engine_manifest(&manifest) {
        return Ok(FlowState {
            manifest,
            revision,
            nodes: vec![],
            schemas: vec![],
            diagnostics: vec![],
            im: None,
        });
    }
    let nodes: Vec<Node> =
        serde_json::from_value(manifest["nodes"].clone()).map_err(|e| e.to_string())?;
    let deps: BTreeMap<String, Vec<String>> =
        serde_json::from_value(manifest["internal_dependencies"].clone())
            .map_err(|e| e.to_string())?;
    let mut diagnostics = vec![];
    let mut schemas = vec![];
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    if nodes.len() > 128 {
        return Err("Workflow exceeds 128 nodes".into());
    }
    for field in ["name", "version", "description", "author"] {
        if manifest[field].as_str().is_none_or(|x| x.trim().is_empty()) {
            diagnostic(
                &mut diagnostics,
                None,
                "metadata",
                format!("Missing {field}"),
            );
        }
    }
    for n in &nodes {
        if !ids.insert(n.id.clone()) || !paths.insert(n.schema.clone()) {
            diagnostic(
                &mut diagnostics,
                Some(&n.id),
                "duplicate",
                "Duplicate node or schema",
            );
        }
        match safe_path(root, &n.schema)
            .and_then(|p| read_json(&p))
            .and_then(|v| serde_json::from_value::<Schema>(v).map_err(|e| e.to_string()))
        {
            Ok(s) => {
                if s.id != n.id || s.kind != n.kind || s.schema_version != 1 {
                    diagnostic(
                        &mut diagnostics,
                        Some(&n.id),
                        "schema",
                        "Schema identity/version mismatch",
                    );
                }
                validate_schema(root, &s, &mut diagnostics);
                schemas.push(s);
            }
            Err(e) => diagnostic(&mut diagnostics, Some(&n.id), "schema", e),
        }
    }
    revision =
        digest(&serde_json::to_vec(&json!({"manifest":manifest,"schemas":schemas})).unwrap());
    for (id, parents) in &deps {
        if nodes
            .iter()
            .any(|n| &n.id == id && matches!(n.kind.as_str(), "map-source" | "static-resource"))
            && !parents.is_empty()
            || parents
                .iter()
                .any(|parent| nodes.iter().any(|n| &n.id == parent && n.kind == "output"))
        {
            diagnostic(
                &mut diagnostics,
                Some(id),
                "dependency",
                "Source nodes cannot have inputs; output nodes cannot have consumers",
            );
        }
        if !ids.contains(id)
            || parents.iter().any(|p| !ids.contains(p) || p == id)
            || parents.iter().collect::<BTreeSet<_>>().len() != parents.len()
        {
            diagnostic(
                &mut diagnostics,
                Some(id),
                "dependency",
                "Unknown, duplicate or self dependency",
            );
        }
    }
    if deps.len() != ids.len() {
        diagnostic(
            &mut diagnostics,
            None,
            "dependency",
            "Every node must declare dependencies",
        );
    }
    let mut order = vec![];
    let mut done = BTreeSet::new();
    loop {
        let next = nodes.iter().find(|n| {
            !done.contains(&n.id)
                && deps
                    .get(&n.id)
                    .is_some_and(|p| p.iter().all(|p| done.contains(p)))
        });
        match next {
            Some(n) => {
                done.insert(n.id.clone());
                order.push(n.id.clone());
            }
            None => break,
        }
    }
    if done.len() != nodes.len() {
        diagnostic(
            &mut diagnostics,
            None,
            "cycle",
            "Dependencies contain a cycle or missing node",
        );
    }
    let outputs: Vec<_> = nodes.iter().filter(|n| n.kind == "output").collect();
    if outputs.len() != 1 {
        diagnostic(
            &mut diagnostics,
            None,
            "output",
            "Exactly one output is required",
        );
    } else {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![outputs[0].id.clone()];
        while let Some(id) = pending.pop() {
            if reachable.insert(id.clone()) {
                pending.extend(deps.get(&id).cloned().unwrap_or_default());
            }
        }
        if reachable.len() != nodes.len() {
            diagnostic(
                &mut diagnostics,
                None,
                "disconnected",
                "Every node must reach the output",
            );
        }
    }
    if schemas.iter().filter(|s| s.kind == "map-source").count() > 1 {
        diagnostic(
            &mut diagnostics,
            None,
            "source",
            "Only one map source is supported per package",
        );
    }
    let height = schemas
        .iter()
        .filter(|s| matches!(s.kind.as_str(), "height-edit" | "noise-height"))
        .count();
    if height > 1 {
        diagnostic(
            &mut diagnostics,
            None,
            "conflict",
            "Multiple height writers conflict",
        );
    }
    if height > 0 && !schemas.iter().any(|s| s.kind == "map-source") {
        diagnostic(
            &mut diagnostics,
            None,
            "source",
            "Height node requires map source",
        );
    }
    for s in schemas
        .iter()
        .filter(|s| matches!(s.kind.as_str(), "height-edit" | "noise-height"))
    {
        let mut ancestors = BTreeSet::new();
        let mut pending = deps.get(&s.id).cloned().unwrap_or_default();
        while let Some(id) = pending.pop() {
            if ancestors.insert(id.clone()) {
                pending.extend(deps.get(&id).cloned().unwrap_or_default());
            }
        }
        if !schemas
            .iter()
            .any(|n| n.kind == "map-source" && ancestors.contains(&n.id))
        {
            diagnostic(
                &mut diagnostics,
                Some(&s.id),
                "source",
                "Map source must precede the height operation",
            );
        }
    }
    if height == 0 && !schemas.iter().any(|s| s.kind == "static-resource") {
        diagnostic(
            &mut diagnostics,
            None,
            "empty",
            "No resource-producing node",
        );
    }
    let mut inputs = BTreeMap::new();
    if diagnostics.is_empty() {
        for s in &schemas {
            let path = match s.kind.as_str() {
                "map-source" => Some(PathBuf::from(s.config["package"].as_str().unwrap())),
                "static-resource" => Some(safe_path(root, s.config["asset"].as_str().unwrap())?),
                _ => None,
            };
            if let Some(path) = path {
                match file_digest(&path) {
                    Ok(hash) => {
                        inputs.insert(s.id.clone(), hash);
                    }
                    Err(e) => diagnostic(&mut diagnostics, Some(&s.id), "input", e),
                }
            }
        }
    }
    let im = diagnostics.is_empty().then(|| IntermediateModel {
        version: 1,
        manifest_hash: revision.clone(),
        order,
        schemas: schemas.clone(),
        inputs,
    });
    Ok(FlowState {
        manifest,
        revision,
        nodes,
        schemas,
        diagnostics,
        im,
    })
}
fn validate_schema(root: &Path, s: &Schema, out: &mut Vec<Diagnostic>) {
    let error = match s.kind.as_str() {
        "map-source" => {
            let p = s.config["package"].as_str().unwrap_or("");
            let group = s.config["group"].as_str().unwrap_or("");
            if !Path::new(p).is_file()
                || u32::from_str_radix(group.trim_start_matches("0x"), 16).is_err()
            {
                Some("Select a source package and region group")
            } else {
                let group = u32::from_str_radix(group.trim_start_matches("0x"), 16).unwrap();
                match dbpf::Package::open(p) {
                    Ok(package)
                        if package
                            .entries()
                            .iter()
                            .filter(|e| {
                                e.id.type_id == sc_properties::region_write::F0_TYPE_ID
                                    && e.id.group == group
                            })
                            .count()
                            == sc_properties::region_write::TILES_PER_REGION =>
                    {
                        None
                    }
                    _ => Some("Source must contain all 341 height tiles for the selected region"),
                }
            }
        }
        "height-edit" => {
            if s.config["delta_meters"]
                .as_f64()
                .is_some_and(|v| v.is_finite() && v.abs() <= 1024.)
            {
                None
            } else {
                Some("Height delta must be between -1024 and 1024 metres")
            }
        }
        "noise-height" => {
            if s.config["seed"]
                .as_u64()
                .is_some_and(|v| v <= u32::MAX as u64)
                && s.config["amplitude_meters"]
                    .as_f64()
                    .is_some_and(|v| (0.0..=1024.).contains(&v))
                && s.config["wavelength_meters"]
                    .as_f64()
                    .is_some_and(|v| (16.0..=32768.).contains(&v))
                && s.config["base_meters"]
                    .as_f64()
                    .is_some_and(|v| (-1024.0..=1023.).contains(&v))
            {
                None
            } else {
                Some("Invalid noise parameters")
            }
        }
        "static-resource" => {
            if safe_path(root, s.config["asset"].as_str().unwrap_or("")).is_ok_and(|p| p.is_file())
                && parse_tgi(s.config["tgi"].as_str().unwrap_or("")).is_ok()
            {
                None
            } else {
                Some("Select an existing project asset and valid TGI")
            }
        }
        "output" => {
            if s.config["file"].as_str().is_some_and(|f| {
                !f.contains(['/', '\\', ':']) && f.ends_with(".package") && f.len() > 8
            }) {
                None
            } else {
                Some("Output must be a .package filename")
            }
        }
        "city-slots" | "resources" | "roads" | "map-metadata" => {
            if s.config["action"] == "preserve" {
                None
            } else {
                Some("This editor is not yet verified for writing; preserve source data or leave the workflow blocked")
            }
        }
        "coordinate-alignment" => {
            Some("Whole-region simulation alignment requires game validation before building")
        }
        _ => Some("Unsupported node kind"),
    };
    if let Some(e) = error {
        diagnostic(out, Some(&s.id), "configuration", e);
    }
}
fn parse_tgi(tgi: &str) -> Result<dbpf::ResourceId, String> {
    let v: Vec<_> = tgi
        .split(':')
        .map(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16))
        .collect();
    if v.len() != 3 || v.iter().any(Result::is_err) {
        return Err("TGI must be TYPE:GROUP:INSTANCE".into());
    }
    Ok(dbpf::ResourceId {
        type_id: v[0].clone().unwrap(),
        group: v[1].clone().unwrap(),
        instance: v[2].clone().unwrap(),
    })
}

pub fn save_node(root: &Path, revision: &str, schema: Schema) -> Result<FlowState, String> {
    let state = inspect(root)?;
    if state.revision != revision {
        return Err("Project changed; refresh before saving".into());
    }
    let node = state
        .nodes
        .iter()
        .find(|n| n.id == schema.id && n.kind == schema.kind)
        .ok_or("Unknown schema node")?;
    if schema.schema_version != 1 {
        return Err("Unsupported schema version".into());
    }
    let mut manifest = state.manifest;
    // Content-addressed schema revisions make package.json the atomic commit point.
    let value = json!(schema);
    let hash = digest(&serde_json::to_vec(&value).unwrap());
    let mod_type = manifest["mod_type"].as_str().ok_or("Missing mod type")?;
    let layout = ensure_project_layout(root, mod_type)?;
    let file = format!(
        "{}/{}-{}.json",
        layout["schemas"].as_str().unwrap(),
        node.kind,
        &hash[..16]
    );
    manifest["resource_layout"] = layout;
    let target = safe_path(root, &file)?;
    if !target.exists() {
        save_json(root, &file, &value, false)?;
    } else if read_json(&target)? != value {
        return Err("Schema revision content differs".into());
    }
    for n in manifest["nodes"].as_array_mut().ok_or("Missing nodes")? {
        if n["id"] == schema.id {
            n["schema"] = json!(file);
        }
    }
    let mut schemas = state.schemas;
    schemas.retain(|s| s.id != schema.id);
    schemas.push(schema);
    manifest["outer_dependencies"]=json!(schemas.iter().filter_map(|s|{
        if s.kind=="map-source" {Some(json!({"node":s.id,"source_package":s.config["package"],"group":s.config["group"],"type":"03E421F0"}))}
        else if s.kind=="static-resource" {Some(json!({"node":s.id,"source_package":s.config["source_package"],"tgi":s.config["tgi"]}))}else{None}
    }).collect::<Vec<_>>());
    save_json(root, "package.json", &manifest, true)?;
    inspect(root)
}

pub fn build(root: &Path, requested: &IntermediateModel) -> Result<Value, String> {
    let state = inspect(root)?;
    let im = state
        .im
        .ok_or("Workflow is incomplete; validate before building")?;
    if serde_json::to_value(&im).unwrap() != serde_json::to_value(requested).unwrap() {
        return Err("Build plan is stale; validate again".into());
    }
    let mut entries = vec![];
    let source = im.schemas.iter().find(|s| s.kind == "map-source");
    for id in &im.order {
        let s = im
            .schemas
            .iter()
            .find(|s| &s.id == id)
            .ok_or("Missing schema")?;
        match s.kind.as_str() {
            "static-resource" => {
                let p = safe_path(root, s.config["asset"].as_str().unwrap())?;
                if fs::metadata(&p).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
                    return Err("Asset exceeds 128 MiB".into());
                }
                entries.push(dbpf::OverlayEntry::new(
                    parse_tgi(s.config["tgi"].as_str().unwrap())?,
                    fs::read(p).map_err(|e| e.to_string())?,
                ));
            }
            "height-edit" | "noise-height" => {
                let src = source.ok_or("Missing map source")?;
                let pkg = dbpf::Package::open(src.config["package"].as_str().unwrap())
                    .map_err(|e| e.to_string())?;
                let group = u32::from_str_radix(
                    src.config["group"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches("0x"),
                    16,
                )
                .map_err(|e| e.to_string())?;
                let mut heights = sc_properties::region_write::read_region_field(&pkg, group)?;
                for (i, h) in heights.iter_mut().enumerate() {
                    let z = if s.kind == "height-edit" {
                        *h as f64 / 32. - 1024. + s.config["delta_meters"].as_f64().unwrap()
                    } else {
                        s.config["base_meters"].as_f64().unwrap()
                            + s.config["amplitude_meters"].as_f64().unwrap()
                                * noise(
                                    (i % 4096) as f64 * 8.
                                        / s.config["wavelength_meters"].as_f64().unwrap(),
                                    (i / 4096) as f64 * 8.
                                        / s.config["wavelength_meters"].as_f64().unwrap(),
                                    s.config["seed"].as_u64().unwrap() as u32,
                                )
                    };
                    *h = ((z + 1024.) * 32.).round().clamp(0., 65535.) as u16;
                }
                let bytes =
                    sc_properties::region_write::build_heightmap_overlay(&pkg, group, &heights)?;
                // Parse the engine output before assembling the final overlay.
                let nonce = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                let staging = safe_path(
                    root,
                    &format!(".openscp-height-{}-{nonce}.package", std::process::id()),
                )?;
                crate::atomic_fs::write_atomic(&staging, &bytes, false)
                    .map_err(|e| e.to_string())?;
                let result = (|| {
                    let p = dbpf::Package::open(&staging).map_err(|e| e.to_string())?;
                    for e in p.entries() {
                        entries.push(dbpf::OverlayEntry::new(
                            e.id,
                            p.read(e).map_err(|e| e.to_string())?,
                        ));
                    }
                    Ok::<(), String>(())
                })();
                let _ = fs::remove_file(staging);
                result?;
            }
            _ => {}
        }
    }
    if entries.is_empty() {
        return Err("Build produced no resources".into());
    }
    for s in &im.schemas {
        let path = match s.kind.as_str() {
            "map-source" => Some(PathBuf::from(s.config["package"].as_str().unwrap())),
            "static-resource" => Some(safe_path(root, s.config["asset"].as_str().unwrap())?),
            _ => None,
        };
        if let Some(path) = path {
            if im.inputs.get(&s.id) != Some(&file_digest(&path)?) {
                return Err("Source changed during build; validate again".into());
            }
        }
    }
    let bytes = dbpf::write_uncompressed_overlay(&entries).map_err(|e| e.to_string())?;
    let hash = digest(&bytes);
    let dir = safe_path(root, &format!("build/{}", &hash[..16]))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let output = im
        .schemas
        .iter()
        .find(|s| s.kind == "output")
        .unwrap()
        .config["file"]
        .as_str()
        .unwrap();
    let relative = format!("build/{}/{output}", &hash[..16]);
    let path = safe_path(root, &relative)?;
    if path.exists() {
        if fs::read(&path).map_err(|e| e.to_string())? != bytes {
            return Err("Existing artifact differs".into());
        }
    } else {
        crate::atomic_fs::write_atomic(&path, &bytes, false).map_err(|e| e.to_string())?;
    }
    let check = dbpf::Package::open(&path).map_err(|e| e.to_string())?;
    if check.entries().len() != entries.len() {
        return Err("Output readback failed".into());
    }
    for entry in check.entries() {
        let expected = entries
            .iter()
            .find(|e| e.id == entry.id)
            .ok_or("Unexpected output resource")?;
        if check.read(entry).map_err(|e| e.to_string())? != expected.data {
            return Err("Output payload readback failed".into());
        }
    }
    let record = json!({"im":im,"artifact":relative,"sha256":hash,"resources":entries.iter().map(|e|format!("{:08X}:{:08X}:{:08X}",e.id.type_id,e.id.group,e.id.instance)).collect::<Vec<_>>()});
    save_json(
        root,
        &format!("build/{}/build.json", &hash[..16]),
        &record,
        true,
    )?;
    Ok(json!({"path":path,"sha256":hash,"resourceCount":entries.len()}))
}
/// Seeded gradient Perlin noise, sampled in continuous world coordinates.
fn noise(x: f64, y: f64, seed: u32) -> f64 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let dx = x - x.floor();
    let dy = y - y.floor();
    let gradient = |ix: i32, iy: i32, px: f64, py: f64| {
        let mut h =
            (ix as u32).wrapping_mul(0x9E3779B9) ^ (iy as u32).wrapping_mul(0x85EBCA6B) ^ seed;
        h ^= h >> 16;
        h = h.wrapping_mul(0x7FEB352D);
        match h & 3 {
            0 => px + py,
            1 => px - py,
            2 => -px + py,
            _ => -px - py,
        }
    };
    let fade = |v: f64| v * v * v * (v * (v * 6. - 15.) + 10.);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    lerp(
        lerp(
            gradient(xi, yi, dx, dy),
            gradient(xi + 1, yi, dx - 1., dy),
            fade(dx),
        ),
        lerp(
            gradient(xi, yi + 1, dx, dy - 1.),
            gradient(xi + 1, yi + 1, dx - 1., dy - 1.),
            fade(dx),
        ),
        fade(dy),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "openscp-flow-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn asset(&self) -> FlowState {
            fs::write(self.0.join("asset.bin"), b"original resource payload").unwrap();
            let state = initialize(
                &self.0,
                "Example",
                "Author",
                "Description",
                "assets",
                "assets",
            )
            .unwrap();
            let mut schema = state.schemas[0].clone();
            schema.config =
                json!({"asset":"asset.bin","tgi":"12345678:00000000:00000001","source_package":""});
            save_node(&self.0, &state.revision, schema).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn builds_real_dbpf_and_rejects_changed_inputs() {
        let f = Fixture::new();
        let state = f.asset();
        let im = state.im.unwrap();
        let result = build(&f.0, &im).unwrap();
        let path = Path::new(result["path"].as_str().unwrap());
        let package = dbpf::Package::open(path).unwrap();
        assert_eq!(package.entries().len(), 1);
        assert_eq!(
            package.read(&package.entries()[0]).unwrap(),
            b"original resource payload"
        );
        drop(package);
        assert_eq!(result, build(&f.0, &im).unwrap());
        fs::write(f.0.join("asset.bin"), b"edited asset").unwrap();
        assert!(build(&f.0, &im).unwrap_err().contains("stale"));
    }
    #[test]
    fn recognizes_only_explicit_supported_manifests() {
        let f = Fixture::new();
        let mut value = f.asset().manifest;
        assert!(engine_manifest(&value));
        value["engine"]["version"] = json!(2);
        assert!(!engine_manifest(&value));
        value["engine"]["version"] = json!(1);
        value["origin"] = json!("community");
        assert!(!engine_manifest(&value));
        assert!(!engine_manifest(&json!({"generator":"openscp"})));
    }

    #[test]
    fn resource_layout_is_typed_and_preserves_existing_files() {
        for (kind, folder) in [
            ("map", "assets/maps/heightmaps"),
            ("code", "scripts"),
            ("assets", "assets/models"),
            ("gameplay", "assets/tuning"),
        ] {
            let f = Fixture::new();
            let layout = ensure_project_layout(&f.0, kind).unwrap();
            assert!(f.0.join(folder).is_dir());
            assert!(f.0.join(format!("schemas/{kind}")).is_dir());
            let asset = f.0.join(folder).join("keep.bin");
            fs::write(&asset, b"preserve").unwrap();
            assert_eq!(ensure_project_layout(&f.0, kind).unwrap(), layout);
            assert_eq!(fs::read(asset).unwrap(), b"preserve");
        }
    }

    #[test]
    fn source_and_output_cannot_be_used_as_intermediate_nodes() {
        let f = Fixture::new();
        let state = f.asset();
        let mut manifest = state.manifest;
        let source = state.nodes[0].id.clone();
        let output = state.nodes[1].id.clone();
        manifest["internal_dependencies"][&source] = json!([output]);
        save_json(&f.0, "package.json", &manifest, true).unwrap();
        assert!(inspect(&f.0)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.detail.contains("Source nodes cannot")));
    }
    #[test]
    fn external_schema_edits_invalidate_save_revision() {
        let f = Fixture::new();
        let state = f.asset();
        let mut schema = state.schemas[0].clone();
        schema.config["tgi"] = json!("12345678:00000000:00000002");
        save_json(&f.0, &state.nodes[0].schema, &json!(schema), true).unwrap();
        assert!(save_node(&f.0, &state.revision, schema).is_err());
    }
    #[test]
    fn cycles_and_disconnected_nodes_block_builds() {
        let f = Fixture::new();
        let state = f.asset();
        let mut manifest = state.manifest;
        let first = &state.nodes[0].id;
        let last = &state.nodes[1].id;
        manifest["internal_dependencies"][first] = json!([last]);
        save_json(&f.0, "package.json", &manifest, true).unwrap();
        let inspected = inspect(&f.0).unwrap();
        assert!(inspected.im.is_none());
        assert!(inspected.diagnostics.iter().any(|d| d.code == "cycle"));
        manifest["internal_dependencies"][first] = json!([]);
        manifest["internal_dependencies"][last] = json!([]);
        save_json(&f.0, "package.json", &manifest, true).unwrap();
        assert!(inspect(&f.0)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == "disconnected"));
    }
    #[test]
    fn protects_paths_and_existing_manifests() {
        let f = Fixture::new();
        for path in [
            "../escape",
            "/absolute",
            "C:/absolute",
            "a\\b",
            "a//b",
            "a/./b",
        ] {
            assert!(safe_path(&f.0, path).is_err());
        }
        fs::write(f.0.join("package.json"), b"broken json").unwrap();
        assert!(initialize(&f.0, "Name", "A", "D", "assets", "assets").is_err());
        assert_eq!(fs::read(f.0.join("package.json")).unwrap(), b"broken json");
        fs::write(
            f.0.join("package.json"),
            br#"{"custom":{"license":"community"}}"#,
        )
        .unwrap();
        let state = initialize(&f.0, "Name", "A", "D", "assets", "assets").unwrap();
        assert_eq!(
            state.manifest["legacy_metadata"]["custom"]["license"],
            "community"
        );
    }
    #[test]
    fn detects_actual_overrides_not_new_tgis() {
        let mods = Fixture::new();
        let game = Fixture::new();
        let id = parse_tgi("12345678:00000000:00000001").unwrap();
        let source =
            dbpf::write_uncompressed_overlay(&[dbpf::OverlayEntry::new(id, b"source")]).unwrap();
        fs::write(game.0.join("source.package"), source).unwrap();
        let overlay = dbpf::write_uncompressed_overlay(&[
            dbpf::OverlayEntry::new(id, b"override"),
            dbpf::OverlayEntry::new(parse_tgi("12345678:00000000:00000002").unwrap(), b"new"),
        ])
        .unwrap();
        fs::write(mods.0.join("mod.package"), overlay).unwrap();
        let result = detect_overrides(&mods.0, Some(&game.0));
        assert_eq!(result["status"], "complete");
        assert_eq!(result["mappings"].as_array().unwrap().len(), 1);
        assert_eq!(detect_overrides(&mods.0, None)["status"], "unavailable");
    }
    #[test]
    fn unsupported_map_operations_cannot_build() {
        let f = Fixture::new();
        let state = initialize(&f.0, "Map", "A", "D", "map", "whole-region").unwrap();
        assert!(state.im.is_none());
        assert!(state
            .diagnostics
            .iter()
            .any(|d| d.node.as_deref() == Some("coordinate-alignment-3")));
    }
    #[test]
    fn noise_is_continuous_and_seeded() {
        assert_eq!(noise(2.5, 3.75, 42), noise(2.5, 3.75, 42));
        assert!((noise(1. - 1e-7, 0.4, 42) - noise(1. + 1e-7, 0.4, 42)).abs() < 1e-5);
        assert_ne!(noise(0.31, 0.73, 42), noise(0.31, 0.73, 43));
    }

    /// Opt-in integration test: retail packages remain outside the repository.
    #[test]
    #[ignore = "requires SC_FLOW_MAP_PATH containing Titan Gorge"]
    fn retail_map_flow_builds_all_height_tiles() {
        let source = std::env::var("SC_FLOW_MAP_PATH").expect("SC_FLOW_MAP_PATH");
        let f = Fixture::new();
        let state = initialize(&f.0, "Map", "A", "D", "map", "original").unwrap();
        let mut schema = state.schemas[0].clone();
        schema.config = json!({"package":source,"group":"C04182E4"});
        let state = save_node(&f.0, &state.revision, schema).unwrap();
        assert!(state.diagnostics.is_empty(), "{:?}", state.diagnostics);
        let result = build(&f.0, &state.im.unwrap()).unwrap();
        assert_eq!(result["resourceCount"], 341);
        let output = dbpf::Package::open(result["path"].as_str().unwrap()).unwrap();
        let input = dbpf::Package::open(&source).unwrap();
        let titan = sc_properties::region_write::solve_f0_pyramid(&input, 0xC04182E4).unwrap();
        assert_eq!(titan.levels[0][0][0], 0x7ADCBE88);
        let reference = sc_properties::region_write::solve_f0_pyramid(&input, 0xBEAF0510).unwrap();
        assert_eq!(
            titan.levels, reference.levels,
            "Shared retail tile IDs must retain the same positions across regions"
        );
        let original = sc_properties::region_write::read_region_field(&input, 0xC04182E4).unwrap();
        let actual = sc_properties::region_write::read_region_field(&output, 0xC04182E4).unwrap();
        assert_eq!(original, actual);
        println!(
            "Retail Flow -> IM -> DBPF: 341 tiles; 4096² height samples unchanged with zero delta"
        );
    }
}
