use std::path::Path;
use std::sync::Arc;

use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::{Kind, PropType, Property, PropertyEncoding, PropertyFile, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::activity::{AppState, CommandError};

mod focus;
mod motion;

const MAIN: u32 = 0x9E5A_6813;
const ALTERNATE: u32 = 0xD9FE_A4B9;
// cCameraControllerBase::ConfigUpdated (dev VA 0x798BE0):
// zoom distance, vertical FOV, near/far clip, min/max pitch, heading presets.
const CURVES: [u32; 7] = [
    0x00FC_5228,
    0x00FC_6857,
    0x00FC_7047,
    0x00FC_704C,
    0x00FC_71FC,
    0x00FC_7205,
    0x00FC_78E7,
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraSource {
    revision: String,
    profiles: Vec<CameraProfile>,
    motion_strength: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraProfile {
    instance: u32,
    curves: Vec<CameraCurve>,
    auto_focus: Option<Vec<Vec<f32>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CameraCurve {
    hash: u32,
    samples: Vec<f32>,
    inherited: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraEdit {
    instance: u32,
    hash: u32,
    samples: Vec<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraExport {
    source_path: String,
    output_directory: String,
    revision: String,
    edits: Vec<CameraEdit>,
    motion_strength: Option<f32>,
}

struct Resource {
    id: ResourceId,
    bytes: Vec<u8>,
    properties: PropertyFile,
}

fn source_error(error: impl ToString) -> CommandError {
    CommandError::new("camera_source_invalid", error.to_string())
}

fn read_source(package: &Package) -> Result<Vec<Resource>, CommandError> {
    let mut resources = Vec::new();
    for instance in [MAIN, ALTERNATE] {
        let id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x4041_0100,
            instance,
        };
        if let Some(entry) = package.entry(id) {
            let bytes = package.read(entry).map_err(source_error)?;
            let properties = PropertyFile::parse(&bytes).map_err(source_error)?;
            if properties.claimed_count as usize != properties.values.len() {
                return Err(source_error("Incomplete camera property list"));
            }
            resources.push(Resource {
                id,
                bytes,
                properties,
            });
        }
    }
    if resources.is_empty() {
        return Err(source_error("No city camera resources"));
    }
    Ok(resources)
}

fn revision(resources: &[Resource], motion: Option<&motion::MotionOption>) -> String {
    let mut digest = Sha256::new();
    for resource in resources {
        digest.update(resource.id.instance.to_be_bytes());
        digest.update((resource.bytes.len() as u64).to_be_bytes());
        digest.update(&resource.bytes);
    }
    if let Some(option) = motion {
        digest.update(&option.bytes);
    }
    format!("{:x}", digest.finalize())
}

fn profiles(resources: &[Resource]) -> Result<Vec<CameraProfile>, CommandError> {
    resources
        .iter()
        .map(|resource| {
            let merged =
                sc_properties::flatten_parent_inheritance(resource.properties.clone(), |key| {
                    resources
                        .iter()
                        .find(|parent| {
                            parent.id.type_id == key.type_id
                                && parent.id.instance == key.instance
                                && (parent.id.group == key.group
                                    || (key.group == 0 && parent.id.group == resource.id.group))
                        })
                        .map(|parent| parent.properties.clone())
                });
            let mut curves = CURVES
                .iter()
                .filter_map(|&hash| {
                    let property = merged.get(hash)?;
                    Some((|| {
                        let samples = property
                            .array()
                            .ok_or_else(|| source_error(format!("{hash:08X}")))?;
                        let samples = samples
                            .iter()
                            .map(|value| match value {
                                Value::Float(number) if number.is_finite() => Ok(*number),
                                _ => Err(source_error(format!("{hash:08X}"))),
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        if samples.is_empty() {
                            return Err(source_error(format!("{hash:08X}")));
                        }
                        Ok(CameraCurve {
                            hash,
                            samples,
                            inherited: resource.properties.get(hash).is_none(),
                        })
                    })())
                })
                .collect::<Result<Vec<_>, CommandError>>()?;
            curves.extend(focus::editable(&resource.properties, &merged)?);
            Ok(CameraProfile {
                instance: resource.id.instance,
                curves,
                auto_focus: focus::auto_table(&merged),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn read_city_camera(
    state: tauri::State<'_, AppState>,
    source_path: Option<String>,
    package_id: Option<u64>,
) -> Result<CameraSource, CommandError> {
    let manager = Arc::clone(&state.packages);
    tauri::async_runtime::spawn_blocking(move || {
        let package = match (package_id, source_path) {
            (Some(id), None) => manager.get(id).map_err(source_error)?,
            (None, Some(path)) => Arc::new(Package::open(path).map_err(source_error)?),
            _ => return Err(source_error("Expected one camera source")),
        };
        let resources = read_source(&package)?;
        let motion = motion::read(&package)?;
        Ok(CameraSource {
            revision: revision(&resources, motion.as_ref()),
            profiles: profiles(&resources)?,
            motion_strength: motion
                .as_ref()
                .map_or(motion::DEFAULT_STRENGTH, |option| option.strength),
        })
    })
    .await
    .map_err(|e| CommandError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn city_camera_preview_packages(
    state: tauri::State<'_, AppState>,
    package_id: Option<u64>,
) -> Result<u64, CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let directory = if let Some(id) = package_id {
            let anchor = manager.get(id).map_err(source_error)?;
            anchor.path().parent().map(Path::to_path_buf)
        } else {
            store
                .app_settings()
                .map_err(CommandError::from)?
                .and_then(|settings| settings.game_data_path)
                .map(std::path::PathBuf::from)
        }
        .ok_or_else(|| source_error("Game data directory unavailable"))?;
        let game = manager
            .open_reusable(&directory.join("SimCity_Game.package"))
            .map_err(source_error)?;
        manager
            .open_reusable(&directory.join("SimCity_Graphics.package"))
            .map_err(source_error)?;
        Ok(game)
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

fn valid_samples(hash: u32, samples: &[f32]) -> bool {
    !samples.is_empty()
        && samples.len() <= 1024
        && samples.iter().all(|&value| {
            value.is_finite()
                && match hash {
                    0x00FC_5228 | 0x00FC_7047 | 0x00FC_704C => value > 0.0,
                    0x00FC_6857 => value > 0.0 && value < 180.0,
                    0x00FC_71FC | 0x00FC_7205 => value > -90.0 && value < 90.0,
                    0x00FC_78E7 => true,
                    _ => false,
                }
        })
        && (hash != CURVES[0] || samples.windows(2).all(|pair| pair[0] <= pair[1]))
}

fn build_overlay(request: &CameraExport) -> Result<Vec<u8>, CommandError> {
    let package = Package::open(&request.source_path).map_err(source_error)?;
    let mut resources = read_source(&package)?;
    let motion = motion::read(&package)?;
    if revision(&resources, motion.as_ref()) != request.revision {
        return Err(CommandError::new(
            "camera_source_changed",
            "Source camera resources changed",
        ));
    }
    let source_profiles = profiles(&resources)?;
    let mut changed = std::collections::HashSet::new();
    let mut edits = std::collections::HashSet::new();
    for edit in &request.edits {
        let invalid = || {
            CommandError::new(
                "camera_edit_invalid",
                format!("{:08X}:{:08X}", edit.instance, edit.hash),
            )
        };
        let curve = source_profiles
            .iter()
            .find(|profile| profile.instance == edit.instance)
            .and_then(|profile| profile.curves.iter().find(|curve| curve.hash == edit.hash))
            .ok_or_else(invalid)?;
        let shape = if valid_samples(edit.hash, &edit.samples) {
            Some((
                PropType::Float,
                Kind::Array(edit.samples.iter().copied().map(Value::Float).collect()),
            ))
        } else {
            focus::edit_kind(edit.hash, &edit.samples)
        };
        let (prop_type, kind) = shape.ok_or_else(invalid)?;
        if !edits.insert((edit.instance, edit.hash)) {
            return Err(invalid());
        }
        if !curve.inherited && edit.samples == curve.samples {
            continue;
        }
        let resource = resources
            .iter_mut()
            .find(|resource| resource.id.instance == edit.instance)
            .ok_or_else(invalid)?;
        if let Some(property) = resource
            .properties
            .values
            .iter_mut()
            .find(|property| property.hash == edit.hash)
        {
            property.kind = kind;
        } else {
            resource.properties.values.push(Property {
                hash: edit.hash,
                prop_type,
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: matches!(kind, Kind::Array(_)).then_some(4),
                },
                kind,
            });
            resource.properties.claimed_count += 1;
        }
        changed.insert(edit.instance);
    }
    for profile in profiles(&resources)? {
        let samples = |hash| {
            profile
                .curves
                .iter()
                .find(|curve| curve.hash == hash)
                .map(|curve| curve.samples.as_slice())
        };
        if samples(focus::MODE) == Some(&[1.0])
            && profile.auto_focus.is_none()
            && changed.contains(&profile.instance)
        {
            return Err(CommandError::new(
                "camera_focus_table",
                format!("{:08X}", profile.instance),
            ));
        }
        let Some(zoom) = samples(CURVES[0]) else {
            continue;
        };
        for i in 0..zoom.len() {
            let fraction = i as f32 / zoom.len() as f32;
            let clip = samples(CURVES[2]).zip(samples(CURVES[3]));
            let pitch = samples(CURVES[4]).zip(samples(CURVES[5]));
            if clip.is_some_and(|(near, far)| sample(near, fraction) >= sample(far, fraction))
                || pitch.is_some_and(|(min, max)| sample(min, fraction) > sample(max, fraction))
            {
                return Err(CommandError::new(
                    "camera_curve_order",
                    format!("{:08X}", profile.instance),
                ));
            }
        }
    }
    let mut entries = resources
        .into_iter()
        .map(|resource| {
            let data = if changed.contains(&resource.id.instance) {
                resource
                    .properties
                    .encode_canonical()
                    .map_err(|e| CommandError::new("camera_export_failed", e.to_string()))?
            } else {
                resource.bytes
            };
            Ok(OverlayEntry::new(resource.id, data))
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    entries.extend(motion::entry(motion, request.motion_strength)?);
    write_uncompressed_overlay(&entries)
        .map_err(|e| CommandError::new("camera_export_failed", e.to_string()))
}

fn sample(samples: &[f32], fraction: f32) -> f32 {
    let position = (samples.len() - 1) as f32 * fraction;
    let index = position as usize;
    let current = samples[index];
    current + (samples.get(index + 1).copied().unwrap_or(current) - current) * position.fract()
}

#[tauri::command]
pub async fn export_city_camera(request: CameraExport) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || export_camera_to_directory(&request))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
}

fn export_camera_to_directory(request: &CameraExport) -> Result<String, CommandError> {
    let selected = Path::new(&request.output_directory);
    let directory = selected.canonicalize().map_err(camera_write_error)?;
    if !directory.is_dir() {
        return Err(CommandError::new(
            "camera_export_failed",
            "Output directory unavailable",
        ));
    }
    if directory
        .components()
        .any(|part| part.as_os_str().eq_ignore_ascii_case("SimCityData"))
    {
        return Err(CommandError::new(
            "camera_export_game_folder",
            "Use the test-package ledger to deploy",
        ));
    }
    let bytes = build_overlay(request)?;
    for version in 1..=u64::MAX {
        let name = if version == 1 {
            "OpenSCP-City-Camera.package".to_owned()
        } else {
            format!("OpenSCP-City-Camera-{version}.package")
        };
        let path = directory.join(&name);
        if path.symlink_metadata().is_ok() {
            continue;
        }
        match crate::atomic_fs::write_atomic(&path, &bytes, false) {
            Ok(()) => return Ok(selected.join(name).to_string_lossy().into_owned()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(camera_write_error(error)),
        }
    }
    Err(CommandError::new(
        "camera_export_failed",
        "No available output filename",
    ))
}

fn camera_write_error(error: std::io::Error) -> CommandError {
    let code = if error.kind() == std::io::ErrorKind::PermissionDenied {
        "camera_export_permission"
    } else {
        "camera_export_failed"
    };
    CommandError::new(code, error.to_string())
}
