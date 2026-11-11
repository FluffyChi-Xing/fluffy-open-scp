use crate::activity::{AppState, CommandError};
use crate::package_service::{
    DecalUnitTextureDto, PackageManager, TgiDto, decode_decal_entry_rgba,
    resolve_shader_def_instance,
};
use dbpf::Package;
use sc_properties::lot_unit::{DECAL_CATEGORIES, DECAL_ID_BASE};
use sc_properties::{
    DecalDictionary, Key, Kind, LotEditorDocument, LotUnit, PROPERTY_RESOURCE_TYPE, PropertyFile,
    Value, is_decal_dictionary_group,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalPackage {
    package_id: u64,
    path: String,
    atlases: Vec<DecalAtlas>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecalAtlas {
    tgi: TgiDto,
    entry_count: usize,
}

fn catalog(manager: &PackageManager) -> Result<Vec<DecalPackage>, CommandError> {
    let mut packages = Vec::new();
    for (package_id, package) in manager.all_packages_with_ids()? {
        let mut atlases = Vec::new();
        for entry in package.entries().iter().filter(|entry| {
            entry.id.type_id == PROPERTY_RESOURCE_TYPE && is_decal_dictionary_group(entry.id.group)
        }) {
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(dictionary) = DecalDictionary::parse(&bytes) else {
                continue;
            };
            if dictionary.entries.is_empty() {
                continue;
            }
            atlases.push(DecalAtlas {
                tgi: TgiDto {
                    type_id: entry.id.type_id,
                    group: entry.id.group,
                    instance: entry.id.instance,
                },
                entry_count: dictionary.entries.len(),
            });
        }
        atlases.sort_by_key(|atlas| (atlas.tgi.group, atlas.tgi.instance));
        if !atlases.is_empty() {
            packages.push(DecalPackage {
                package_id,
                path: package.path().to_string_lossy().into_owned(),
                atlases,
            });
        }
    }
    packages.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(packages)
}

#[tauri::command]
pub async fn list_decal_library(
    state: State<'_, AppState>,
) -> Result<Vec<DecalPackage>, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || catalog(&manager))
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecalSource {
    pub package_path: String,
    pub atlas: TgiDto,
    pub raster_package_path: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveWorkbenchDecal {
    source: DecalSource,
    id: TgiDto,
    placement: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchDecal {
    source: DecalSource,
    texture: DecalUnitTextureDto,
    placement: Option<DecalPlacement>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DecalPlacement {
    category: usize,
    basis: [f32; 9],
    scale: f32,
    depth: f32,
    material_data: [f32; 3],
}

fn error(message: impl Into<String>) -> CommandError {
    CommandError::new("workbench_decal_unavailable", message)
}

pub(crate) fn read_atlas(package: &Package, tgi: &TgiDto) -> Result<DecalDictionary, String> {
    if tgi.type_id != PROPERTY_RESOURCE_TYPE || !is_decal_dictionary_group(tgi.group) {
        return Err("invalid decal atlas TGI".into());
    }
    let entry = package
        .entry(tgi.clone().into())
        .ok_or("decal atlas missing")?;
    let bytes = package.read(entry).map_err(|cause| cause.to_string())?;
    DecalDictionary::parse(&bytes).map_err(|cause| cause.to_string())
}

fn find_placement(
    packages: &[Arc<Package>],
    id: u32,
    atlas_ids: &HashSet<u32>,
) -> Option<DecalPlacement> {
    let mut same_atlas = None;
    for package in packages {
        for entry in package.entries().iter().filter(|entry| {
            entry.id.type_id == PROPERTY_RESOURCE_TYPE
                && matches!(entry.id.group, 0x40E1_C000 | 0x58F0_C000)
        }) {
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse(&bytes) else {
                continue;
            };
            let matches: Vec<_> = (0..DECAL_CATEGORIES)
                .flat_map(|category| {
                    let values = file
                        .get(DECAL_ID_BASE + category as u32)
                        .and_then(|column| match &column.kind {
                            Kind::Array(values) => Some(values.as_slice()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    values.iter().enumerate().filter_map(move |(index, value)| {
                        let Value::Key(key) = value else { return None };
                        (key.instance == id || atlas_ids.contains(&key.instance)).then_some((
                            category,
                            index,
                            key.instance == id,
                        ))
                    })
                })
                .collect();
            if matches.is_empty() || (same_atlas.is_some() && !matches.iter().any(|row| row.2)) {
                continue;
            }
            for unit in LotEditorDocument::from_property_file(file)
                .assemble_units()
                .units
            {
                let LotUnit::Decal {
                    category,
                    index,
                    transform: Some(transform),
                    scale: Some(scale),
                    depth: Some(depth),
                    material_data: Some(material_data),
                    ..
                } = unit
                else {
                    continue;
                };
                let Some((_, _, exact)) = matches
                    .iter()
                    .find(|row| row.0 == category && row.1 == index)
                else {
                    continue;
                };
                if !scale.is_finite()
                    || scale <= 0.0
                    || !depth.is_finite()
                    || material_data.iter().any(|value| !value.is_finite())
                {
                    continue;
                }
                let basis: [f32; 9] = transform.matrix[..9].try_into().ok()?;
                if basis.iter().any(|value| !value.is_finite()) {
                    continue;
                }
                let preset = DecalPlacement {
                    category,
                    basis,
                    scale,
                    depth,
                    material_data,
                };
                if *exact {
                    return Some(preset);
                }
                same_atlas.get_or_insert(preset);
            }
        }
    }
    same_atlas
}

pub(crate) fn raster_resource(package: &Package, key: Key) -> Option<dbpf::ResourceId> {
    let exact = dbpf::ResourceId {
        type_id: key.type_id,
        group: key.group,
        instance: key.instance,
    };
    if matches!(key.type_id, 0x2F4E_681C | 0x2F4E_681B) && package.entry(exact).is_some() {
        return Some(exact);
    }
    package.entries().iter().find_map(|entry| {
        (entry.id.instance == key.instance && matches!(entry.id.type_id, 0x2F4E_681C | 0x2F4E_681B))
            .then_some(entry.id)
    })
}

fn resolve(
    manager: &PackageManager,
    request: ResolveWorkbenchDecal,
) -> Result<WorkbenchDecal, CommandError> {
    let mut source = request.source;
    let mut packages: Vec<_> = manager
        .all_packages_with_ids()?
        .into_iter()
        .map(|(_, package)| package)
        .collect();
    packages.sort_by(|left, right| left.path().cmp(right.path()));
    let open = |path: &str| -> Result<Arc<Package>, CommandError> {
        let canonical = std::fs::canonicalize(path).map_err(|cause| error(cause.to_string()))?;
        if let Some(package) = packages.iter().find(|package| package.path() == canonical) {
            return Ok(Arc::clone(package));
        }
        Package::open(Path::new(path))
            .map(Arc::new)
            .map_err(|cause| error(cause.to_string()))
    };
    let atlas = open(&source.package_path)?;
    let dictionary = read_atlas(&atlas, &source.atlas).map_err(error)?;
    let entry = dictionary
        .entries
        .iter()
        .find(|entry| {
            entry.id.is_some_and(|id| {
                id.instance == request.id.instance
                    && id.group == request.id.group
                    && id.type_id == request.id.type_id
            })
        })
        .ok_or_else(|| error("decal ID missing in atlas"))?;
    let raster = entry
        .raster
        .ok_or_else(|| error("decal raster key missing"))?;
    let saved_raster = source
        .raster_package_path
        .as_deref()
        .and_then(|path| open(path).ok());
    let raster_package = saved_raster
        .iter()
        .chain(std::iter::once(&atlas))
        .chain(packages.iter())
        .find(|package| raster_resource(package, raster).is_some())
        .ok_or_else(|| error("decal raster package is unavailable"))?;
    let raw = decode_decal_entry_rgba(entry, raster_package, manager);
    let png = raw
        .png_base64
        .ok_or_else(|| error(raw.error.unwrap_or_default()))?;
    source.raster_package_path = Some(raster_package.path().to_string_lossy().into_owned());
    let material_instance = dictionary.material.map(|key| key.instance);
    let colors = entry.colors.iter().any(Option::is_some).then(|| {
        entry
            .colors
            .iter()
            .map(|color| color.unwrap_or([0.0; 4]).map(|value| value * 2.0).to_vec())
            .collect()
    });
    let variant = colors.is_none().then(|| "hole".into());
    let placement = if request.placement {
        let atlas_ids = dictionary
            .entries
            .iter()
            .filter_map(|entry| entry.id.map(|id| id.instance))
            .collect();
        Some(
            find_placement(&packages, request.id.instance, &atlas_ids).ok_or_else(|| {
                error("decal atlas has no native placement sample in the open packages")
            })?,
        )
    } else {
        None
    };
    Ok(WorkbenchDecal {
        texture: DecalUnitTextureDto {
            category: 0,
            index: 0,
            id_instance: request.id.instance,
            atlas_instance: Some(source.atlas.instance),
            aspect_ratio: entry.aspect_ratio,
            width: raw.width,
            height: raw.height,
            png: Some(png),
            error: None,
            material_instance,
            shader_def_instance: material_instance
                .and_then(|id| resolve_shader_def_instance(id, &atlas, manager)),
            variant,
            quantized: false,
            blend: None,
            colors,
        },
        source,
        placement,
    })
}

#[tauri::command]
pub async fn resolve_workbench_decal(
    state: State<'_, AppState>,
    request: ResolveWorkbenchDecal,
) -> Result<WorkbenchDecal, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || resolve(&manager, request))
        .await
        .map_err(|cause| CommandError::internal(cause.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the local original SimCity packages"]
    fn resolves_native_workbench_decal_and_reopens_cross_package_texture() {
        let game = std::env::var("OPENSCP_DECAL_GAME_DATA").unwrap();
        let game = Path::new(&game);
        let atlas = game.join("SimCity_Game.package");
        let graphics = game.join("SimCity_Graphics.package");
        let manager = PackageManager::new();
        manager.insert(Package::open(&atlas).unwrap()).unwrap();
        manager.insert(Package::open(&graphics).unwrap()).unwrap();
        let id = TgiDto {
            type_id: 0,
            group: 0,
            instance: 0xD2DD_48C5,
        };
        let resolved = resolve(
            &manager,
            ResolveWorkbenchDecal {
                source: DecalSource {
                    package_path: atlas.to_string_lossy().into_owned(),
                    atlas: TgiDto {
                        type_id: PROPERTY_RESOURCE_TYPE,
                        group: 0xC67F_B185,
                        instance: 0xEEFD_390C,
                    },
                    raster_package_path: None,
                },
                id: id.clone(),
                placement: true,
            },
        )
        .unwrap();
        assert!(resolved.placement.as_ref().unwrap().scale > 0.0);
        assert!(
            resolved
                .source
                .raster_package_path
                .as_deref()
                .unwrap()
                .ends_with("SimCity_Graphics.package")
        );
        assert!(resolved.texture.png.is_some());
        let reopened = resolve(
            &PackageManager::new(),
            ResolveWorkbenchDecal {
                source: resolved.source,
                id,
                placement: false,
            },
        )
        .unwrap();
        assert_eq!(resolved.texture.png, reopened.texture.png);
    }
}
