//! On-demand Swarm effect resolution for the property editor.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use sc_properties::swarm::{BlockTransform, EFFECT_COLLECTION_TYPE, SwarmCollection};
use sc_properties::swarm_material::{EffectMaterial, decode_effect_materials};
use sc_properties::swarm_particle::{
    MetaParticleComponent, ParticleComponent, TextureRef, decode_meta_particle_components,
    decode_particle_components_prefix,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::effect_resources::{decode_effect_texture, locate_effect_packages};

const MAX_REQUESTED_EFFECTS: usize = 64;
const MAX_EMITTERS_PER_EFFECT: usize = 32;
const MAX_TOTAL_EMITTERS: usize = 128;
const MAX_NESTED_VISUAL_DEPTH: usize = 4;

struct EmitterCandidate<'a> {
    transform: BlockTransform,
    particle: Option<&'a ParticleComponent>,
    emission: Option<&'a ParticleComponent>,
    fast_reference: bool,
    approximate: bool,
}

fn combine_transforms(parent: &BlockTransform, child: &BlockTransform) -> BlockTransform {
    let mut rotation = [0.0; 9];
    for column in 0..3 {
        for row in 0..3 {
            rotation[column * 3 + row] = (0..3)
                .map(|axis| parent.rotation[axis * 3 + row] * child.rotation[column * 3 + axis])
                .sum();
        }
    }
    let mut offset = parent.offset;
    for (row, value) in offset.iter_mut().enumerate() {
        *value += parent.scale
            * (0..3)
                .map(|axis| parent.rotation[axis * 3 + row] * child.offset[axis])
                .sum::<f32>();
    }
    BlockTransform {
        scale: parent.scale * child.scale,
        rotation,
        offset,
    }
}

fn collect_emitter_candidates<'a>(
    collection: &SwarmCollection,
    visual_index: usize,
    particles: &'a [ParticleComponent],
    metas: &'a [MetaParticleComponent],
    parent_transform: Option<&BlockTransform>,
    inherited_emission: Option<&'a ParticleComponent>,
    path: &mut Vec<usize>,
    candidates: &mut Vec<EmitterCandidate<'a>>,
    unsupported: &mut usize,
) {
    if path.len() >= MAX_NESTED_VISUAL_DEPTH || path.contains(&visual_index) {
        *unsupported += 1;
        return;
    }
    let Some(visual) = collection.visual_effects.get(visual_index) else {
        *unsupported += 1;
        return;
    };
    path.push(visual_index);
    for block in &visual.blocks {
        if candidates.len() >= MAX_EMITTERS_PER_EFFECT {
            *unsupported += 1;
            continue;
        }
        let Some(index) = usize::try_from(block.component_index).ok() else {
            *unsupported += 1;
            continue;
        };
        let transform = parent_transform.map_or_else(
            || block.transform.clone(),
            |parent| combine_transforms(parent, &block.transform),
        );
        match block.type_code {
            0 => collect_emitter_candidates(
                collection,
                index,
                particles,
                metas,
                Some(&transform),
                inherited_emission,
                path,
                candidates,
                unsupported,
            ),
            1 => {
                let particle = particles
                    .iter()
                    .find(|p| p.type_code == 1 && p.index == index);
                if particle.is_none() {
                    *unsupported += 1;
                    continue;
                }
                candidates.push(EmitterCandidate {
                    transform,
                    particle,
                    emission: inherited_emission,
                    fast_reference: false,
                    approximate: inherited_emission.is_some(),
                });
            }
            2 => {
                let Some(meta) = metas.iter().find(|m| m.index == index) else {
                    *unsupported += 1;
                    continue;
                };
                let Some(child) = meta.child.as_ref() else {
                    *unsupported += 1;
                    continue;
                };
                if child.type_code == 0 {
                    // The preview uses the parent's emitter as a proxy for
                    // per-particle child effect instances.
                    collect_emitter_candidates(
                        collection,
                        child.index,
                        particles,
                        metas,
                        Some(&transform),
                        Some(&meta.emitter),
                        path,
                        candidates,
                        unsupported,
                    );
                    *unsupported += 1;
                } else if child.type_code == 1 || child.type_code == 12 {
                    let particle = particles
                        .iter()
                        .find(|p| p.type_code == child.type_code && p.index == child.index);
                    candidates.push(EmitterCandidate {
                        transform,
                        particle,
                        emission: Some(&meta.emitter),
                        fast_reference: child.type_code == 12,
                        approximate: inherited_emission.is_some(),
                    });
                } else {
                    *unsupported += 1;
                }
            }
            12 => candidates.push(EmitterCandidate {
                transform,
                particle: None,
                emission: inherited_emission,
                fast_reference: true,
                approximate: inherited_emission.is_some(),
            }),
            _ => *unsupported += 1,
        }
    }
    path.pop();
}

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
        aspect: vec![1.0],
        rotation: vec![0.0],
        alpha: vec![0.0, 0.62, 0.0],
        color: vec![[0.72, 0.76, 0.8]],
        texture: TextureRef {
            group: u32::MAX,
            instance: u32::MAX,
            draw_mode: 0,
        },
        tile_count: [1, 1],
        frame_start: 0,
        frame_count: 1,
        frame_random: 0,
        frame_speed: 0.0,
        frame_speed_by_life: false,
        direction_force: [0.0, 0.12, 0.0],
        wind_strength: 0.18,
        gravity_strength: 0.0,
        drag: 0.7,
    }
}

fn materialize_emitter(
    candidate: EmitterCandidate<'_>,
    packages: &[Arc<dbpf::Package>],
    materials: &HashMap<(u32, u32), EffectMaterial>,
    texture_cache: &mut HashMap<(u32, u32), Option<String>>,
    unsupported_blocks: &mut usize,
) -> Option<EffectEmitter> {
    let fallback = candidate.particle.is_none() && candidate.fast_reference;
    let particle = match candidate.particle {
        Some(particle) => particle.clone(),
        None if fallback => {
            *unsupported_blocks += 1;
            fast_particle_fallback()
        }
        None => {
            *unsupported_blocks += 1;
            return None;
        }
    };
    let texture_refs: Vec<_> = if particle.texture.draw_mode == 15 {
        materials
            .get(&(particle.texture.group, particle.texture.instance))
            .map(|material| {
                let mut refs = material.textures.clone();
                refs.sort_by_key(|(slot, _)| *slot);
                refs.into_iter()
                    .map(|(_, texture)| (texture.group, texture.instance))
                    .collect()
            })
            .unwrap_or_default()
    } else if particle.texture.instance != u32::MAX {
        vec![(particle.texture.group, particle.texture.instance)]
    } else {
        Vec::new()
    };
    let texture_png_base64 = texture_refs.into_iter().find_map(|(group, instance)| {
        texture_cache
            .entry((group, instance))
            .or_insert_with(|| decode_effect_texture(packages, group, instance))
            .clone()
    });
    let missing_texture = particle.texture.instance != u32::MAX && texture_png_base64.is_none();
    if missing_texture {
        *unsupported_blocks += 1;
    }
    Some(EffectEmitter {
        transform: candidate.transform,
        particle,
        emission: candidate.emission.cloned(),
        texture_png_base64,
        fallback: fallback || candidate.approximate || missing_texture,
    })
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
                let materials = match decode_effect_materials(&collection, &bytes) {
                    Ok(materials) => materials,
                    Err(error) => {
                        diagnostics.push(format!(
                            "effect collection {:08X}: {error}",
                            entry.id.instance
                        ));
                        HashMap::new()
                    }
                };
                for export in matches {
                    let Some(index) = export.effect_index else {
                        continue;
                    };
                    let mut emitters = Vec::new();
                    let mut unsupported_blocks = 0;
                    let mut candidates = Vec::new();
                    collect_emitter_candidates(
                        &collection,
                        index,
                        &particles,
                        &metas,
                        None,
                        None,
                        &mut Vec::new(),
                        &mut candidates,
                        &mut unsupported_blocks,
                    );
                    for candidate in candidates {
                        if remaining_emitters == 0 {
                            unsupported_blocks += 1;
                            continue;
                        }
                        if let Some(emitter) = materialize_emitter(
                            candidate,
                            &packages,
                            &materials,
                            &mut texture_cache,
                            &mut unsupported_blocks,
                        ) {
                            emitters.push(emitter);
                            remaining_emitters -= 1;
                        }
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
    use sc_properties::swarm::{VisualBlock, VisualEffect};
    use sc_properties::swarm_particle::ComponentRef;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn identity_transform(offset: [f32; 3], scale: f32) -> BlockTransform {
        BlockTransform {
            scale,
            rotation: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            offset,
        }
    }

    #[test]
    fn missing_texture_keeps_a_marked_particle_emitter() {
        let mut particle = fast_particle_fallback();
        particle.type_code = 1;
        particle.texture = TextureRef {
            group: 0,
            instance: 0x93DC_33AD,
            draw_mode: 15,
        };
        let mut cache = HashMap::new();
        let mut unsupported = 0;
        let emitter = materialize_emitter(
            EmitterCandidate {
                transform: identity_transform([0.0; 3], 1.0),
                particle: Some(&particle),
                emission: None,
                fast_reference: false,
                approximate: false,
            },
            &[],
            &HashMap::new(),
            &mut cache,
            &mut unsupported,
        )
        .unwrap();
        assert!(emitter.fallback);
        assert!(emitter.texture_png_base64.is_none());
        assert_eq!(emitter.particle.rate, particle.rate);
        assert_eq!(unsupported, 1);
    }

    #[test]
    fn nested_visual_uses_child_particle_and_bounded_transform() {
        let collection = SwarmCollection {
            components: Vec::new(),
            resources: Vec::new(),
            visual_version: 1,
            visual_effects: vec![
                VisualEffect {
                    flags: 0,
                    blocks: vec![VisualBlock {
                        type_code: 2,
                        flags: 0,
                        transform: identity_transform([1.0, 0.0, 0.0], 2.0),
                        component_index: 0,
                    }],
                },
                VisualEffect {
                    flags: 0,
                    blocks: vec![VisualBlock {
                        type_code: 1,
                        flags: 0,
                        transform: identity_transform([0.0, 3.0, 0.0], 0.5),
                        component_index: 0,
                    }],
                },
            ],
            exports: Vec::new(),
            imports: Vec::new(),
        };
        let particle = fast_particle_fallback();
        let mut sprite = particle.clone();
        sprite.type_code = 1;
        let sprites = [sprite];
        let metas = [MetaParticleComponent {
            index: 0,
            emitter: particle,
            child: Some(ComponentRef {
                type_code: 0,
                index: 1,
            }),
        }];
        let mut candidates = Vec::new();
        let mut unsupported = 0;
        collect_emitter_candidates(
            &collection,
            0,
            &sprites,
            &metas,
            None,
            None,
            &mut Vec::new(),
            &mut candidates,
            &mut unsupported,
        );
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].particle.unwrap().type_code, 1);
        assert_eq!(candidates[0].transform.offset, [1.0, 6.0, 0.0]);
        assert_eq!(candidates[0].transform.scale, 1.0);
        assert!(candidates[0].emission.is_some());
        assert!(candidates[0].approximate);
        assert_eq!(unsupported, 1);
    }

    #[test]
    fn nested_visual_cycle_stops_without_emitter() {
        let collection = SwarmCollection {
            components: Vec::new(),
            resources: Vec::new(),
            visual_version: 1,
            visual_effects: vec![VisualEffect {
                flags: 0,
                blocks: vec![VisualBlock {
                    type_code: 0,
                    flags: 0,
                    transform: identity_transform([0.0; 3], 1.0),
                    component_index: 0,
                }],
            }],
            exports: Vec::new(),
            imports: Vec::new(),
        };
        let mut candidates = Vec::new();
        let mut unsupported = 0;
        collect_emitter_candidates(
            &collection,
            0,
            &[],
            &[],
            None,
            None,
            &mut Vec::new(),
            &mut candidates,
            &mut unsupported,
        );
        assert!(candidates.is_empty());
        assert_eq!(unsupported, 1);
    }

    #[test]
    fn retail_nested_visuals_add_emitters_when_game_is_installed() {
        let path = PathBuf::from("D:/ea-games/SimCity/SimCityData/SimCity_App.package");
        let Ok(package) = Package::open(&path) else {
            return;
        };
        let mut baseline_empty = 0;
        let mut newly_visible = 0;
        let mut total_exports = 0;
        let mut new_candidates = Vec::<Vec<(u32, u32, bool)>>::new();
        let mut remaining_roots = BTreeMap::<u8, usize>::new();
        let mut failed_particle_collections = 0;
        let mut missing_direct_particles = 0;
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
            let (particles, particle_error) =
                decode_particle_components_prefix(&collection, &bytes);
            if let Some(error) = particle_error {
                failed_particle_collections += 1;
                if failed_particle_collections <= 8 {
                    let section = collection.component_section(1);
                    eprintln!(
                        "particle parse {:08X}: decoded={} section={:?} error={error}",
                        entry.id.instance,
                        particles.len(),
                        section.map(|section| (
                            section.version,
                            section.count,
                            section.payload_size
                        ))
                    );
                }
            }
            let Ok(metas) = decode_meta_particle_components(&collection, &bytes) else {
                continue;
            };
            for export in &collection.exports {
                let Some(index) = export.effect_index else {
                    continue;
                };
                total_exports += 1;
                let direct = collection.visual_effects[index].blocks.iter().any(|block| {
                    let Ok(component_index) = usize::try_from(block.component_index) else {
                        return false;
                    };
                    match block.type_code {
                        1 => particles.iter().any(|particle| {
                            particle.type_code == 1 && particle.index == component_index
                        }),
                        2 => metas.iter().any(|meta| {
                            meta.index == component_index
                                && meta.child.as_ref().is_some_and(|child| {
                                    child.type_code == 12
                                        || particles.iter().any(|particle| {
                                            particle.type_code == child.type_code
                                                && particle.index == child.index
                                        })
                                })
                        }),
                        12 => true,
                        _ => false,
                    }
                });
                if direct {
                    continue;
                }
                baseline_empty += 1;
                let mut candidates = Vec::new();
                let mut unsupported = 0;
                collect_emitter_candidates(
                    &collection,
                    index,
                    &particles,
                    &metas,
                    None,
                    None,
                    &mut Vec::new(),
                    &mut candidates,
                    &mut unsupported,
                );
                if !candidates.is_empty() {
                    newly_visible += 1;
                    new_candidates.push(
                        candidates
                            .iter()
                            .map(|candidate| {
                                let (group, instance) =
                                    candidate.particle.map_or((u32::MAX, u32::MAX), |particle| {
                                        (particle.texture.group, particle.texture.instance)
                                    });
                                let emits = candidate.emission.or(candidate.particle).is_none_or(
                                    |source| source.rate.iter().any(|rate| *rate > 0.0),
                                );
                                (group, instance, emits)
                            })
                            .collect(),
                    );
                } else {
                    for block in &collection.visual_effects[index].blocks {
                        *remaining_roots.entry(block.type_code).or_default() += 1;
                        if block.type_code == 1 {
                            missing_direct_particles += 1;
                        }
                    }
                }
            }
        }
        let packages = [Arc::new(package)];
        let mut texture_cache = HashMap::new();
        let renderable_new = new_candidates
            .iter()
            .filter(|textures| {
                textures.iter().any(|&(group, instance, emits)| {
                    emits
                        && (instance == u32::MAX
                            || texture_cache
                                .entry((group, instance))
                                .or_insert_with(|| {
                                    decode_effect_texture(&packages, group, instance)
                                })
                                .is_some())
                })
            })
            .count();
        eprintln!(
            "App Swarm exports: {total_exports}, baseline empty: {baseline_empty}, nested candidates: {newly_visible}, emitting with App textures: {renderable_new}"
        );
        eprintln!("remaining empty root blocks: {remaining_roots:?}");
        eprintln!(
            "failed particle collections: {failed_particle_collections}, missing direct blocks: {missing_direct_particles}"
        );
        assert!(total_exports > 0);
        assert!(newly_visible > 0);
        assert!(renderable_new > 0);
        assert_eq!(failed_particle_collections, 0);
        assert_eq!(missing_direct_particles, 0);
    }

    #[test]
    fn retail_ep1_property_effects_resolve_when_game_is_installed() {
        const IDS: [u32; 5] = [
            0x660B_C390,
            0x4D98_C606,
            0xE09E_DE1F,
            0xE6BC_ECD8,
            0x99DA_8CAB,
        ];
        let data_dir = PathBuf::from("D:/ea-games/SimCity/SimCityData");
        if !data_dir.join("SimCity_App.package").is_file() {
            return;
        }
        let packages = [
            "SimCityDataEP1.package",
            "SimCity_App.package",
            "SimCity_DLC0.package",
            "SimCity_Graphics.package",
        ]
        .into_iter()
        .filter_map(|name| Package::open(data_dir.join(name)).ok().map(Arc::new))
        .collect::<Vec<_>>();
        let mut found = HashSet::new();
        for package in &packages {
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
                let matches = collection
                    .exports
                    .iter()
                    .filter(|export| {
                        IDS.contains(&export.name_hash) && !found.contains(&export.name_hash)
                    })
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    continue;
                }
                let materials = decode_effect_materials(&collection, &bytes).unwrap();
                let (particles, _) = decode_particle_components_prefix(&collection, &bytes);
                let metas = decode_meta_particle_components(&collection, &bytes).unwrap();
                for export in matches {
                    let Some(index) = export.effect_index else {
                        continue;
                    };
                    let mut candidates = Vec::new();
                    let mut unsupported = 0;
                    collect_emitter_candidates(
                        &collection,
                        index,
                        &particles,
                        &metas,
                        None,
                        None,
                        &mut Vec::new(),
                        &mut candidates,
                        &mut unsupported,
                    );
                    let mut cache = HashMap::new();
                    let textured = candidates
                        .iter()
                        .filter(|candidate| {
                            let mut unsupported = 0;
                            materialize_emitter(
                                EmitterCandidate {
                                    transform: candidate.transform.clone(),
                                    particle: candidate.particle,
                                    emission: candidate.emission,
                                    fast_reference: candidate.fast_reference,
                                    approximate: candidate.approximate,
                                },
                                &packages,
                                &materials,
                                &mut cache,
                                &mut unsupported,
                            )
                            .is_some_and(|emitter| emitter.texture_png_base64.is_some())
                        })
                        .count();
                    eprintln!(
                        "EP1 property effect {:08X}: collection {:08X}/{:08X}, candidates={}, textured={}, unsupported={unsupported}",
                        export.name_hash,
                        entry.id.group,
                        entry.id.instance,
                        candidates.len(),
                        textured
                    );
                    assert!(!candidates.is_empty(), "effect {:08X}", export.name_hash);
                    assert!(
                        textured > 0,
                        "effect {:08X} has no resolved texture",
                        export.name_hash
                    );
                    assert!(
                        candidates.iter().any(|candidate| {
                            candidate.particle.is_some_and(|particle| {
                                candidate
                                    .emission
                                    .unwrap_or(particle)
                                    .rate
                                    .iter()
                                    .any(|rate| *rate > 0.0)
                                    && particle.size.iter().any(|size| *size > 0.0)
                                    && particle.alpha.iter().any(|alpha| *alpha > 0.0)
                            })
                        }),
                        "effect {:08X} cannot display a particle",
                        export.name_hash
                    );
                    found.insert(export.name_hash);
                }
            }
        }
        assert_eq!(found.len(), IDS.len());
    }

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
