use super::{
    hash, manifest, sources,
    spec::{Dependency, MissingFeedback, PanelSchema},
};
use crate::activity::{AppState, CommandError};
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value as Prop};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tauri::State;

#[tauri::command]
pub async fn panel_export_shared_runtime(game_data: String, output: String) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        super::export_shared_runtime(Path::new(&game_data), Path::new(&output))
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| CommandError::new("panel_runtime_export_failed", error))
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[tauri::command]
pub async fn panel_project_read(
    state: State<'_, AppState>,
    project: String,
) -> Result<Value, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = crate::workspace::code_root(&store, &project).map_err(|e| e.to_string())?;
        manifest::read(&root)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(|e| CommandError::new("panel_project_failed", e))
}
#[tauri::command]
pub async fn panel_project_save(
    state: State<'_, AppState>,
    project: String,
    request: Value,
) -> Result<Value, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _lock = crate::mod_flow::FLOW_LOCK
            .lock()
            .map_err(|e| e.to_string())?;
        let root = crate::workspace::code_root(&store, &project).map_err(|e| e.to_string())?;
        manifest::save(&root, &request)
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(|e| CommandError::new("panel_project_failed", e))
}
#[tauri::command]
pub async fn panel_library(
    state: State<'_, AppState>,
    project: String,
) -> Result<Value, CommandError> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let root = crate::workspace::code_root(&store, &project).map_err(|e| e.to_string())?;
        let saved = manifest::read(&root)?;
        let dependency = if saved["dependency"].is_object() {
            serde_json::from_value::<Dependency>(saved["dependency"].clone())
                .map_err(|e| e.to_string())?
        } else {
            let game = store
                .app_settings()
                .map_err(|e| e.to_string())?
                .and_then(|s| s.game_data_path)
                .ok_or("PANEL_GAME_DATA_REQUIRED")?;
            let files = discover_sources(Path::new(&game))?;
            Dependency {
                schema: manifest::SCHEMA_PATH.into(),
                sources: files
                    .iter()
                    .map(|p| sources::source(p))
                    .collect::<Result<_, _>>()?,
                runtime: manifest::RUNTIME_VERSION.into(),
            }
        };
        prepare(&root, &dependency, saved["schema"].clone())
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
    .map_err(|e| CommandError::new("panel_library_failed", e))
}
fn discover_sources(game: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    for n in [
        "SimCity_Game.package",
        "SimCity_App.package",
        "SimCity_Graphics.package",
    ] {
        let p = game.join(n);
        if !p.is_file() {
            return Err(format!("PANEL_SOURCE_MISSING:{}", p.display()));
        }
        out.push(p);
    }
    for locale in ["en-us", "zh-cn", "zh-tw"] {
        if let Ok(rd) = fs::read_dir(game.join("Locale").join(locale)) {
            let mut files: Vec<_> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "package"))
                .collect();
            files.sort();
            out.extend(files);
        }
    }
    if let Some(parent) = game.parent() {
        for d in [
            parent.join("SimCityUserData/EcoGame"),
            parent.join("SimCityUserData/Packages"),
        ] {
            if let Ok(rd) = fs::read_dir(d) {
                let mut files: Vec<_> = rd
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.file_name().is_some_and(|n| {
                            n.to_string_lossy().starts_with("SimCity-Scripts")
                                && n.to_string_lossy().ends_with(".package")
                        })
                    })
                    .collect();
                files.sort();
                if let Some(p) = files.last() {
                    out.push(p.clone());
                }
            }
        }
    }
    if out.len() > 24 {
        return Err("PANEL_SOURCE_LIMIT".into());
    }
    Ok(out)
}
fn inherited_shell(packages: &[Package]) -> Result<ResourceId, String> {
    let mut id = super::super::native_profile::INTERACTION_CHAIN;
    let mut seen = std::collections::HashSet::new();
    while seen.insert(id) {
        let file = PropertyFile::parse(&sources::read(packages, id)?).map_err(|e| e.to_string())?;
        if let Some(Prop::Key(key)) = file.get(0x0BC5_051B).and_then(|p| p.scalar()) {
            return Ok(ResourceId {
                type_id: key.type_id,
                group: key.group,
                instance: key.instance,
            });
        }
        let key = file
            .get(0x00B2_CCCB)
            .and_then(|p| match &p.kind {
                Kind::Scalar(Prop::Key(k)) => Some(*k),
                Kind::Array(a) => a
                    .first()
                    .and_then(|p| if let Prop::Key(k) = p { Some(*k) } else { None }),
                _ => None,
            })
            .ok_or("PANEL_SHELL_NOT_FOUND")?;
        id = ResourceId {
            type_id: if key.type_id == 0 {
                0x00B1_B104
            } else {
                key.type_id
            },
            group: key.group,
            instance: key.instance,
        };
    }
    Err("PANEL_SHELL_INHERITANCE_CYCLE".into())
}
pub(crate) fn prepare(
    root: &Path,
    dependency: &Dependency,
    schema: Value,
) -> Result<Value, String> {
    let mut dependency = dependency.clone();
    if let Some(base) = dependency.sources.iter().find(|source| Path::new(&source.path).file_name().is_some_and(|name| name == "SimCity_Game.package")) {
        let base = sources::path(root, &base.path)?;
        let ep1 = base.with_file_name("SimCityDataEP1.package");
        if ep1.is_file() && !dependency.sources.iter().any(|source| Path::new(&source.path).file_name().is_some_and(|name| name == "SimCityDataEP1.package")) {
            dependency.sources.push(sources::source(&ep1)?);
        }
    }
    let packages = sources::open(root, &dependency.sources)?;
    let fallback = if schema.is_null() {
        serde_json::to_value(PanelSchema {
            schema: "openscp.panel".into(),
            version: 1,
            shell: inherited_shell(&packages)?.to_string(),
            hover: vec![],
            click: vec!["maintenance".into()],
            power: true,
            missing_feedback: MissingFeedback::Hide,
        })
        .map_err(|e| e.to_string())?
    } else {
        schema
    };
    let dir = std::env::temp_dir()
        .join("openscp-panel-preview")
        .join(sources::digest(
            &serde_json::to_vec(&dependency.sources).map_err(|e| e.to_string())?,
        ));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    for (index, package) in packages.iter().enumerate() {
        let lang = Path::new(&dependency.sources[index].path)
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| ["en-us", "zh-cn", "zh-tw"].contains(s))
            .unwrap_or("");
        for entry in package.entries() {
            let id = entry.id;
            if ![
                0x6777_1F5C,
                0x2C97_8DB6,
                0x0A98_EAF0,
                0x2F7D_0004,
                0x2F7D_0007,
            ]
            .contains(&id.type_id)
            {
                continue;
            }
            let ext = match id.type_id {
                0x2C97_8DB6 => "css",
                0x2F7D_0004 => "png",
                0x2F7D_0007 => "gif",
                _ => "json",
            };
            let path = dir.join(format!(
                "{index}-{:08X}_{:08X}_{:08X}.{ext}",
                id.type_id, id.group, id.instance
            ));
            if !path.exists() {
                let bytes = package.read(entry).map_err(|e| e.to_string())?;
                fs::write(&path, bytes).map_err(|e| e.to_string())?;
            }
            rows.push(json!({"type":id.type_id,"group":id.group,"instance":id.instance,"path":path.to_string_lossy(),"locale":lang}));
        }
    }
    let text = |id| String::from_utf8(sources::read(&packages, id)?).map_err(|e| e.to_string());
    let script = text(sources::GAME_JS)?;
    let scrui_id = ResourceId {
        instance: 0x7D14_BE70,
        ..sources::GAME_JS
    };
    // CSS is addressed by its filename hash, as in the game's resource provider.
    let css = rows
        .iter()
        .rev()
        .find(|r| r["type"] == 0x2C97_8DB6u32 && r["instance"] == hash("simcity"));
    let css = match css {
        Some(r) => fs::read_to_string(r["path"].as_str().unwrap()).map_err(|e| e.to_string())?,
        None => {
            let mut combined = String::new();
            for r in rows.iter().filter(|r| r["type"] == 0x2C97_8DB6u32) {
                combined.push_str(
                    &fs::read_to_string(r["path"].as_str().unwrap()).map_err(|e| e.to_string())?,
                );
                combined.push('\n');
            }
            combined
        }
    };
    let mut contexts = serde_json::Map::new();
    for (kind, instance) in [
        ("residence", 0x4DC3_1F9B),
        ("education", 0xC264_A268),
        ("tourism", 0x9F35_D0EE),
        ("park", 0x5EC1_71E5),
    ] {
        let id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance,
        };
        if let Ok(flat) = flatten(&packages, id, &mut std::collections::HashSet::new()) {
            contexts.insert(kind.into(), flat);
        }
    }
    let mut native_locales = json!({"en-US":"en-us","zh-CN":"zh-cn"});
    // Some installations swap locale package contents. Inspect labels rather
    // than assuming the directory name identifies the actual language.
    for code in ["en-us", "zh-tw", "zh-cn"] {
        if let Some(row) = rows.iter().rev().find(|r| {
            r["locale"] == code
                && r["type"] == 0x0A98_EAF0u32
                && r["instance"] == hash("BuildingRollover")
        }) {
            let raw =
                fs::read_to_string(row["path"].as_str().unwrap()).map_err(|e| e.to_string())?;
            let table: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
                .map_err(|e| format!("PANEL_LOCALE_TABLE:{e}"))?;
            let cjk = table.as_object().is_some_and(|o| {
                o.values()
                    .filter_map(Value::as_str)
                    .any(|s| s.chars().any(|c| ('\u{3400}'..='\u{9fff}').contains(&c)))
            });
            native_locales[if cjk { "zh-CN" } else { "en-US" }] = json!(code);
        }
    }
    Ok(
        json!({"schema":fallback,"dependency":dependency,"resources":rows,"gameScript":script,"scruiScript":text(scrui_id)?,"serviceScript":text(super::native_services::SCRIPT).ok(),"serviceConstants":text(super::native_services::CONSTANTS).ok(),"css":css,"components":super::components(),"contexts":contexts,"nativeLocales":native_locales}),
    )
}
fn flatten(
    packages: &[Package],
    id: ResourceId,
    seen: &mut std::collections::HashSet<ResourceId>,
) -> Result<Value, String> {
    if !seen.insert(id) {
        return Err("PANEL_CONTEXT_CYCLE".into());
    }
    let file = PropertyFile::parse(&sources::read(packages, id)?).map_err(|e| e.to_string())?;
    let scalar = |p: &Prop| {
        serde_json::to_value(p)
            .ok()
            .and_then(|v| v.as_object().and_then(|o| o.values().next().cloned()))
            .unwrap_or(Value::Null)
    };
    let parent = file
        .get(0x00B2_CCCB)
        .and_then(|p| p.scalar().or_else(|| p.array().and_then(|a| a.first())))
        .and_then(|v| if let Prop::Key(k) = v { Some(*k) } else { None });
    let mut flat = if let Some(k) = parent {
        flatten(
            packages,
            ResourceId {
                type_id: if k.type_id == 0 {
                    0x00B1_B104
                } else {
                    k.type_id
                },
                group: k.group,
                instance: k.instance,
            },
            seen,
        )?
    } else {
        json!({})
    };
    for p in file.values {
        flat[p.hash.to_string()] = match p.kind {
            Kind::Scalar(v) => scalar(&v),
            Kind::Array(a) => json!(a.iter().map(scalar).collect::<Vec<_>>()),
            Kind::Empty => Value::Null,
        };
    }
    Ok(flat)
}

#[cfg(test)]
mod installed_tests {
    use super::*;
    #[test]
    #[ignore = "requires explicit OPEN_SCP_PANEL_GAME_DATA; reads installed packages, never deploys"]
    fn parses_installed_sources_without_vendored_ui_assets() {
        let game =
            std::env::var("OPEN_SCP_PANEL_GAME_DATA").expect("explicit local game-data path");
        let paths = discover_sources(Path::new(&game)).unwrap();
        let dependency = Dependency {
            schema: manifest::SCHEMA_PATH.into(),
            sources: paths.iter().map(|p| sources::source(p).unwrap()).collect(),
            runtime: manifest::RUNTIME_VERSION.into(),
        };
        let result = prepare(Path::new(&game), &dependency, Value::Null).unwrap();
        super::super::parse(&result["schema"].to_string()).unwrap();
        assert!(
            result["gameScript"]
                .as_str()
                .unwrap()
                .contains("simcity.RolloverItems.kMaintenance")
        );
        assert!(result["contexts"]["residence"].is_object());
        assert!(result["resources"].as_array().unwrap().len() > 10);
        let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tmp/panel-tests")
            .join(format!("installed-{}", std::process::id()));
        fs::create_dir_all(&project).unwrap();
        let project = project.canonicalize().unwrap();
        let raw=json!({"$schema":"openscp.asset","schema_version":2,"id":"panel","kind":"building-asset","reference":{"package":"source.package"},"scene":{"document":"asset.lot.json","authoredGround":true},"semantics":{"menu":{"title":"Panel"}},"outputs":{"package":"000_panel.package","mode":"full"}}).to_string();
        let mut plan = crate::asset_build_m1::plan::plan_from_schema(&raw).unwrap();
        plan.panel = Some(super::super::spec::PanelPlan {
            path: manifest::SCHEMA_PATH.into(),
            config: super::super::parse(&result["schema"].to_string()).unwrap(),
            dependency,
            fingerprint: "installed-source-test".into(),
        });
        let mut ctx = crate::asset_build_m1::stages::BuildContext::new(project.clone(), plan);
        ctx.plan.panel.as_mut().unwrap().config.click = vec!["profit".into()];
        ctx.menu_instance = Some(0xA123_B456);
        let id = ResourceId {
            type_id: 0x00B1B104,
            group: 0x40E1C000,
            instance: 0xA123_B456,
        };
        ctx.resources.insert(id, 0u32.to_be_bytes().to_vec());
        let unit_id = ResourceId { group: 0x40E0C000, ..id };
        ctx.resources.insert(unit_id, 0u32.to_be_bytes().to_vec());
        super::super::apply(&mut ctx).unwrap();
        let unit = sc_properties::PropertyFile::parse(ctx.resources.get(&unit_id).unwrap()).unwrap();
        assert!(format!("{:?}", unit.get(0x0A0209A4)).contains(&super::super::ledger::PROVIDER.to_string()));
        assert!(!ctx.resources.contains_key(&sources::GAME_JS));
        assert!(
            project.starts_with(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .parent()
                    .unwrap()
                    .join("tmp/panel-tests")
                    .canonicalize()
                    .unwrap()
            )
        );
        fs::remove_dir_all(project).unwrap();
        assert!(
            result["css"]
                .as_str()
                .unwrap()
                .contains(".Rollover_Background")
        );
    }
}
