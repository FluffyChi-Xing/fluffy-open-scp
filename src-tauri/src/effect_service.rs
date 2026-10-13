//! On-demand Swarm effect resolution for the property editor.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sc_properties::swarm::{BlockTransform, EFFECT_COLLECTION_TYPE, SwarmCollection};
use sc_properties::swarm_particle::{
    ParticleComponent, TextureRef, decode_meta_particle_components,
    decode_particle_components_prefix,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::effect_resources::{decode_effect_texture, locate_effect_packages};

const MAX_REQUESTED_EFFECTS: usize = 64;
const MAX_EMITTERS_PER_EFFECT: usize = 32;
const MAX_TOTAL_EMITTERS: usize = 128;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPreviewRequest {
    package_id: u64,
    effect_ids: Vec<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectEmitter {
    pub transform: BlockTransform,
    pub particle: ParticleComponent,
    pub emission: Option<ParticleComponent>,
    pub texture_png_base64: Option<String>,
    /// Fast-particle payloads are not decoded yet; this keeps the limitation
    /// visible while still giving the editor an animated preview.
    pub fallback: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectDefinition {
    pub effect_id: u32,
    pub emitters: Vec<EffectEmitter>,
    pub unsupported_blocks: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectPreviewResponse {
    pub effects: Vec<EffectDefinition>,
    pub diagnostics: Vec<String>,
}

fn fast_particle_fallback() -> ParticleComponent {
    ParticleComponent {
        type_code: 12,
        index: 0,
        flags: 0,
        life: [1.6, 2.8],
        emit_direction_min: [-0.2, 0.35, -0.2],
        emit_direction_max: [0.2, 1.0, 0.2],
        emit_speed: [0.25, 0.9],
        emit_volume_min: [-0.12, 0.0, -0.12],
        emit_volume_max: [0.12, 0.18, 0.12],
        rate: vec![18.0],
        rate_loop: 0.0,
        size: vec![0.18, 0.55, 0.9],
        alpha: vec![0.0, 0.62, 0.0],
        color: vec![[0.72, 0.76, 0.8]],
        texture: TextureRef {
            group: u32::MAX,
            instance: u32::MAX,
            draw_mode: 0,
        },
        direction_force: [0.0, 0.12, 0.0],
        wind_strength: 0.18,
        gravity_strength: 0.0,
        drag: 0.7,
    }
}

#[tauri::command]
pub async fn read_effect_preview(
    state: State<'_, AppState>,
    request: EffectPreviewRequest,
) -> Result<EffectPreviewResponse, CommandError> {
    if request.effect_ids.len() > MAX_REQUESTED_EFFECTS {
        return Err(CommandError::new("invalid_argument", "too many effect IDs"));
    }
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || {
        let current = manager
            .get(request.package_id)
            .map_err(CommandError::from)?;
        let packages = locate_effect_packages(&current, &manager, &store);
        let requested: HashSet<u32> = request.effect_ids.iter().copied().collect();
        let mut resolved = HashMap::<u32, EffectDefinition>::new();
        let mut diagnostics = Vec::new();
        let mut texture_cache = HashMap::<(u32, u32), Option<String>>::new();
        let mut remaining_emitters = MAX_TOTAL_EMITTERS;
        for package in &packages {
            for entry in package
                .entries()
                .iter()
                .filter(|e| e.id.type_id == EFFECT_COLLECTION_TYPE)
            {
                let Ok(bytes) = package.read(entry) else {
                    continue;
                };
                let Ok(collection) = SwarmCollection::parse(&bytes) else {
                    continue;
                };
                let matches: Vec<_> = collection
                    .exports
                    .iter()
                    .filter(|export| {
                        requested.contains(&export.name_hash)
                            && !resolved.contains_key(&export.name_hash)
                    })
                    .collect();
                if matches.is_empty() {
                    continue;
                }
                let (particles, particle_error) =
                    decode_particle_components_prefix(&collection, &bytes);
                if let Some(error) = particle_error {
                    diagnostics.push(format!(
                        "effect collection {:08X}: {error}",
                        entry.id.instance
                    ));
                }
                let metas = match decode_meta_particle_components(&collection, &bytes) {
                    Ok(metas) => metas,
                    Err(error) => {
                        diagnostics.push(format!(
                            "effect collection {:08X}: {error}",
                            entry.id.instance
                        ));
                        continue;
                    }
                };
                for export in matches {
                    let Some(index) = export.effect_index else {
                        continue;
                    };
                    let Some(visual) = collection.visual_effects.get(index) else {
                        continue;
                    };
                    let mut emitters = Vec::new();
                    let mut unsupported_blocks = 0;
                    for block in &visual.blocks {
                        if block.component_index < 0
                            || emitters.len() >= MAX_EMITTERS_PER_EFFECT
                            || remaining_emitters == 0
                        {
                            unsupported_blocks += 1;
                            continue;
                        }
                        let (particle, emission, fast_reference) = if block.type_code == 2 {
                            let meta = metas
                                .iter()
                                .find(|m| m.index == block.component_index as usize);
                            let child = meta.and_then(|m| m.child.as_ref());
                            let particle = child.and_then(|c| {
                                particles
                                    .iter()
                                    .find(|p| p.type_code == c.type_code && p.index == c.index)
                            });
                            (
                                particle,
                                meta.map(|m| &m.emitter),
                                child.is_some_and(|child| child.type_code == 12),
                            )
                        } else {
                            (
                                particles.iter().find(|p| {
                                    p.type_code == block.type_code as u16
                                        && p.index == block.component_index as usize
                                }),
                                None,
                                block.type_code == 12,
                            )
                        };
                        let fallback = particle.is_none() && fast_reference;
                        let particle = match particle {
                            Some(particle) => particle.clone(),
                            None if fallback => {
                                unsupported_blocks += 1;
                                fast_particle_fallback()
                            }
                            None => {
                                unsupported_blocks += 1;
                                continue;
                            }
                        };
                        let texture_png_base64 = if particle.texture.instance == u32::MAX {
                            None
                        } else {
                            texture_cache
                                .entry((particle.texture.group, particle.texture.instance))
                                .or_insert_with(|| {
                                    decode_effect_texture(
                                        &packages,
                                        particle.texture.group,
                                        particle.texture.instance,
                                    )
                                })
                                .clone()
                        };
                        if particle.texture.instance != u32::MAX && texture_png_base64.is_none() {
                            unsupported_blocks += 1;
                            continue;
                        }
                        emitters.push(EffectEmitter {
                            transform: block.transform.clone(),
                            particle,
                            emission: emission.cloned(),
                            texture_png_base64,
                            fallback,
                        });
                        remaining_emitters -= 1;
                    }
                    resolved.insert(
                        export.name_hash,
                        EffectDefinition {
                            effect_id: export.name_hash,
                            emitters,
                            unsupported_blocks,
                        },
                    );
                }
            }
            if resolved.len() == requested.len() {
                break;
            }
        }
        let effects = request
            .effect_ids
            .into_iter()
            .filter_map(|id| resolved.remove(&id))
            .collect();
        Ok(EffectPreviewResponse {
            effects,
            diagnostics,
        })
    })
    .await
    .map_err(|error| CommandError::internal(error.to_string()))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use dbpf::Package;
    use std::path::PathBuf;

    #[test]
    fn retail_particle_texture_resolves_when_game_is_installed() {
        let path = PathBuf::from("D:/ea-games/SimCity/SimCityData/SimCity_App.package");
        let Ok(package) = Package::open(&path) else {
            return;
        };
        let Ok(bytes) = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../tmp/gltf_fixture/effects_main.col"
        )) else {
            return;
        };
        let collection = SwarmCollection::parse(&bytes).unwrap();
        let particle = decode_particle_components_prefix(&collection, &bytes)
            .0
            .remove(0);
        let png = decode_effect_texture(
            &[Arc::new(package)],
            particle.texture.group,
            particle.texture.instance,
        );
        assert!(
            png.is_some(),
            "texture {:08X}:{:08X} did not resolve",
            particle.texture.group,
            particle.texture.instance
        );
        assert!(
            STANDARD
                .decode(png.unwrap())
                .unwrap()
                .starts_with(b"\x89PNG")
        );
    }

    #[test]
    fn retail_effect_collections_decode_when_game_is_installed() {
        let path = PathBuf::from("D:/ea-games/SimCity/SimCityData/SimCity_App.package");
        let Ok(package) = Package::open(&path) else {
            return;
        };
        let mut parsed = 0;
        let mut particles = 0;
        let mut incomplete = 0;
        let mut fast_sections = 0;
        for entry in package
            .entries()
            .iter()
            .filter(|entry| entry.id.type_id == EFFECT_COLLECTION_TYPE)
        {
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(collection) = SwarmCollection::parse(&bytes) else {
                continue;
            };
            parsed += 1;
            let (decoded, error) = decode_particle_components_prefix(&collection, &bytes);
            particles += decoded.len();
            if error.is_some() {
                incomplete += 1;
            }
            decode_meta_particle_components(&collection, &bytes).unwrap();
            let mut fast_only = collection.clone();
            fast_only
                .components
                .retain(|section| section.type_code == 12);
            if !fast_only.components.is_empty() {
                fast_sections += 1;
                let (fast_particles, fast_error) =
                    decode_particle_components_prefix(&fast_only, &bytes);
                assert!(fast_particles.is_empty());
                assert!(fast_error.is_none());
            }
        }
        assert!(parsed > 0);
        assert!(particles > 500);
        assert!(incomplete < parsed);
        assert!(fast_sections > 0);
    }
}
