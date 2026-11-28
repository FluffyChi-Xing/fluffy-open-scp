//! Read-only RW4 AnimModel preview. The static LOTM pipeline is independent.
use std::sync::Arc;

use dbpf::{Package, ResourceId};
use rw4::{DecodedAnim, DecodedMesh, DecodedSkeleton, Rw4File, SectionType};
use sc_properties::{PropertyFile, Value};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::activity::{AppState, CommandError};
use crate::building_animation_states::matching_animation_state;
use crate::building_motion_timeline::{MotionTimeline, motion_timeline};
use crate::building_vertex_motion::packed_motion;
use crate::package_service::{
    PackageManager, RESOURCE_DATA_MAX, TgiDto, build_lot_model_payload_for_test,
    flatten_lot_parents_with_sources, locate_sim_source_packages,
};

const PROPERTY: u32 = 0x00B1_B104;
const MODEL: u32 = 0x2F4E_681B;
const MAX_PREVIEW_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationRequest {
    package_id: u64,
    tgi: TgiDto,
    animation_id: Option<u32>,
    #[serde(default)]
    refined: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Binding {
    id: u32,
    resource_id: Option<u32>,
    start: f32,
    end: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<MotionTimeline>,
}

fn bindings(props: &PropertyFile) -> impl Iterator<Item = Binding> + '_ {
    // cUnitModel::AddBundle.c:91–137: the AnimModel table has parallel columns.
    let column = |hash| props.get(hash).and_then(|p| p.array()).unwrap_or_default();
    let keys = column(0x09F3_E6B4);
    [0x09F3_E6B3, 0x09F3_E6B5, 0x09F3_E6B6, 0x09F3_E6B7]
        .into_iter()
        .all(|hash| column(hash).len() == keys.len())
        .then_some(keys.iter().enumerate())
        .into_iter()
        .flatten()
        .filter_map(move |(i, value)| {
            match (
                value,
                &column(0x09F3_E6B3)[i],
                &column(0x09F3_E6B5)[i],
                &column(0x09F3_E6B6)[i],
                &column(0x09F3_E6B7)[i],
            ) {
                (
                    Value::Key(id),
                    Value::Key(resource),
                    Value::Float(speed),
                    Value::Float(start),
                    Value::Float(end),
                ) if id.instance != 0
                    && resource.instance != 0
                    && speed.is_finite()
                    && start.is_finite()
                    && end.is_finite() =>
                {
                    Some(Binding {
                        id: id.instance,
                        resource_id: Some(resource.instance),
                        start: *start,
                        end: *end,
                        timeline: None,
                    })
                }
                _ => None,
            }
        })
}

/// RW4 ANIMATIONS: little endian flags, count, then pairs of ID/section index.
/// Verified on the original 34E0E232 (coal storage) and 5607DF24 (delivery garage).
fn animation_entry(payload: &[u8], id: u32) -> Option<(usize, u32)> {
    let word = |b: &[u8]| <[u8; 4]>::try_from(b).ok().map(u32::from_le_bytes);
    let count = word(payload.get(4..8)?)? as usize;
    let rows = payload.get(8..)?.chunks_exact(8);
    if count > rows.len() {
        return None;
    }
    let mut found = rows.take(count).enumerate().filter_map(|(slot, r)| {
        (word(&r[..4])? == id)
            .then(|| word(&r[4..]).map(|section| (slot, section)))
            .flatten()
    });
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// Model-track references from cChoreographyAnimations::FillFromProps (table
/// columns 0, 1 and 11). Preview the referenced clip in isolation.
fn choreography_clips(props: &PropertyFile) -> impl Iterator<Item = Binding> + '_ {
    let column = |hash| props.get(hash).and_then(|p| p.array()).unwrap_or_default();
    let channels = column(0x0DA7_CAD4);
    let sequences = column(0x0DA7_CAD5);
    let types = column(0x0DA7_CB06);
    channels
        .iter()
        .zip(sequences)
        .enumerate()
        .filter_map(move |(index, (channel, sequence))| {
            if !matches!(sequence, Value::Key(key) if key.instance != 0)
                || (!types.is_empty()
                    && !matches!(types.get(index), Some(Value::Int32(0) | Value::UInt32(0))))
            {
                return None;
            }
            let Value::Key(key) = channel else {
                return None;
            };
            (key.instance != 0).then_some(Binding {
                id: key.instance,
                resource_id: None,
                start: 0.0,
                end: 0.0,
                timeline: None,
            })
        })
}

fn transforms_change(channel: &rw4::Channel) -> bool {
    let Some(first) = channel.keys.first() else {
        return false;
    };
    channel.keys.iter().skip(1).any(|k| {
        if k.time <= first.time {
            return false;
        }
        let moved = [
            k.tx - first.tx,
            k.ty - first.ty,
            k.tz - first.tz,
            k.sx - first.sx,
            k.sy - first.sy,
            k.sz - first.sz,
        ]
        .into_iter()
        .any(|delta| delta.abs() > 1e-5);
        // q and -q represent the same orientation.
        let q = [k.qx, k.qy, k.qz, k.qw];
        let rest = [first.qx, first.qy, first.qz, first.qw];
        let distance = |sign: f32| {
            q.into_iter()
                .zip(rest)
                .map(|(a, b)| (a - sign * b).powi(2))
                .sum::<f32>()
        };
        moved || distance(1.0).min(distance(-1.0)) > 1e-10
    })
}

fn affects_mesh(mesh: &DecodedMesh, skeleton: &DecodedSkeleton, clip: &DecodedAnim) -> bool {
    let Some(skin) = sc_exporter::extract_skin(mesh, skeleton, std::slice::from_ref(clip)) else {
        return false;
    };
    let changed: Vec<_> = skeleton
        .hierarchy
        .joints
        .iter()
        .map(|joint| {
            clip.channels
                .iter()
                .any(|ch| ch.id == joint.name_fnv && transforms_change(ch))
        })
        .collect();
    skin.v_joints
        .iter()
        .zip(&skin.v_weights)
        .any(|(&joint, &weight)| {
            if weight <= 0.0 {
                return false;
            }
            let mut joint = i32::from(joint);
            for _ in 0..skin.joint_count {
                let Some(&moves) = usize::try_from(joint).ok().and_then(|j| changed.get(j)) else {
                    return false;
                };
                if moves {
                    return true;
                }
                joint = skin.parent[joint as usize];
            }
            false
        })
}

fn model_entry(package: &Package, key: &sc_properties::Key) -> Option<ResourceId> {
    let direct = ResourceId {
        type_id: MODEL,
        group: key.group,
        instance: key.instance,
    };
    package.entry(direct).map(|e| e.id).or_else(|| {
        // Some model keys omit the group. Accept fallback only when unambiguous.
        if key.group != 0 {
            return None;
        }
        let mut entries = package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == MODEL && e.id.instance == key.instance);
        let first = entries.next()?.id;
        entries.next().is_none().then_some(first)
    })
}

fn read_bounded(package: &Package, id: ResourceId) -> Option<Vec<u8>> {
    let entry = package.entry(id)?;
    (u64::from(entry.decompressed_size) <= RESOURCE_DATA_MAX)
        .then(|| package.read(entry).ok())
        .flatten()
}

fn inspect_model(
    data: &[u8],
    props: &PropertyFile,
    animation_state: Option<&PropertyFile>,
    candidates: &[Binding],
    export_id: Option<u32>,
) -> (Vec<Binding>, Vec<u8>) {
    let empty = || (Vec::new(), Vec::new());
    let Ok(file) = Rw4File::parse(data) else {
        return empty();
    };
    let Some(directory) = file
        .sections_of_type(SectionType::ANIMATIONS)
        .next()
        .and_then(|section| file.payload(data, section.number).ok())
    else {
        return empty();
    };
    let Some(skeleton) = file
        .sections_of_type(SectionType::RW4_SKELETON)
        .next()
        .and_then(|section| file.decode_skeleton(data, section.number).ok())
    else {
        return empty();
    };
    // Reject malformed hierarchies before passing them to the existing exporter.
    if skeleton
        .hierarchy
        .joints
        .iter()
        .any(|joint| joint.parent >= skeleton.hierarchy.joints.len() as i32)
    {
        return empty();
    }
    let meshes: Vec<_> = file
        .sections_of_type(SectionType::MESH)
        .filter_map(|section| file.decode_mesh(data, section.number).ok())
        .filter(DecodedMesh::is_exportable)
        .collect();
    let packed: Vec<_> = meshes.iter().map(packed_motion).collect();
    let mut supported = Vec::new();
    for candidate in candidates {
        let mut binding = candidate.clone();
        if export_id.is_some_and(|id| id != binding.id) {
            continue;
        }
        // LoadModelInfo assigns mAnimData slots in GetAnimationIDs order.
        // AddAnimationsFromArena copies the FF0001 rows in table order.
        let affects_slot = |slot: u8| {
            packed
                .iter()
                .flatten()
                .any(|vertices| vertices.iter().any(|v| v.affects_slot(slot)))
        };
        let resolve_slot = |channel| {
            let (slot, _) = animation_entry(directory, channel)?;
            (slot < 4 && affects_slot(slot as u8)).then_some(slot as u8)
        };
        binding.timeline = motion_timeline(props, binding.id, resolve_slot).or_else(|| {
            animation_state.and_then(|state| motion_timeline(state, binding.id, resolve_slot))
        });
        let entry = animation_entry(directory, binding.id).or_else(|| {
            let slot = usize::from(binding.timeline.as_ref()?.tracks.first()?.slot);
            let offset = 12 + slot * 8;
            let section = u32::from_le_bytes(directory.get(offset..offset + 4)?.try_into().ok()?);
            Some((slot, section))
        });
        let Some((slot, section)) = entry else {
            continue;
        };
        let Some(mut anim) = file.decode_anim(data, section).ok() else {
            continue;
        };
        if anim.skeleton_id != skeleton.hierarchy.id {
            continue;
        }
        let vertex_motion = binding.timeline.is_some() || (slot < 4 && affects_slot(slot as u8));
        if binding.resource_id.is_none() {
            binding.end = binding
                .timeline
                .as_ref()
                .map_or(if vertex_motion { 1.0 } else { anim.length }, |timeline| {
                    timeline.duration
                });
        }
        // The generic exporter has an ordinal fallback. This branch only
        // advertises tracks whose joint IDs actually resolve.
        anim.channels.retain(|ch| {
            matches!(
                ch.components,
                rw4::COMPONENTS_LOC_ROT | rw4::COMPONENTS_LOC_ROT_SCALE
            ) && skeleton
                .hierarchy
                .joints
                .iter()
                .any(|joint| joint.name_fnv == ch.id)
                && ch.keys.iter().all(|k| {
                    [
                        k.time, k.tx, k.ty, k.tz, k.qx, k.qy, k.qz, k.qw, k.sx, k.sy, k.sz,
                    ]
                    .into_iter()
                    .all(f32::is_finite)
                })
        });
        if !vertex_motion
            && !meshes
                .iter()
                .zip(&packed)
                .any(|(mesh, motion)| motion.is_none() && affects_mesh(mesh, &skeleton, &anim))
        {
            continue;
        }
        if export_id.is_some() {
            let mut buffers = vec![if slot < 4 { slot as u8 } else { u8::MAX }];
            for (mesh, motion) in meshes.iter().zip(&packed) {
                let indices: Vec<_> = mesh
                    .vertices
                    .iter()
                    .map(|v| f32::from(v.d3d_color_g().unwrap_or(0)))
                    .collect();
                let glb = sc_exporter::export_glb_with_colors(
                    mesh,
                    motion.is_none().then_some(&skeleton),
                    std::slice::from_ref(&anim),
                    sc_exporter::EmbeddedTextures::default(),
                    None,
                    Some(&indices),
                );
                let motion_bytes = motion.as_ref().map_or(0, |v| v.len() * 12);
                if buffers.len() + glb.bytes.len() + motion_bytes + 8 > MAX_PREVIEW_BYTES {
                    return empty();
                }
                buffers.extend_from_slice(&(glb.bytes.len() as u32).to_le_bytes());
                buffers.extend_from_slice(&glb.bytes);
                buffers.extend_from_slice(&(motion_bytes as u32).to_le_bytes());
                if let Some(motion) = motion {
                    for vertex in motion {
                        vertex.write(&mut buffers);
                    }
                }
            }
            return (vec![binding], buffers);
        }
        if !supported.iter().any(|b: &Binding| b.id == binding.id) {
            supported.push(binding);
        }
    }
    (supported, Vec::new())
}

fn inspect(
    manager: &PackageManager,
    store: &sc_store::Store,
    request: AnimationRequest,
) -> Result<(Vec<Binding>, Vec<u8>), CommandError> {
    let id: ResourceId = request.tgi.into();
    if id.type_id != PROPERTY {
        return Ok((Vec::new(), Vec::new()));
    }
    let current = manager
        .get(request.package_id)
        .map_err(CommandError::from)?;
    let Some(root) = read_bounded(&current, id).and_then(|d| PropertyFile::parse(&d).ok()) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let sources = locate_sim_source_packages(manager, store).unwrap_or_default();
    let props = flatten_lot_parents_with_sources(root, &current, manager, &sources);
    let packages = manager
        .all_packages_with_ids()
        .map_err(CommandError::from)?;
    let search = std::iter::once(current.as_ref())
        .chain(
            packages
                .iter()
                .filter(|(i, _)| *i != request.package_id)
                .map(|(_, p)| p.as_ref()),
        )
        .chain(sources.iter().map(|(_, p)| p));
    // Same model-key order as PE (LOD0 first). This preview keeps one model for
    // all clips; it does not borrow a clip from another LOD or graphics state.
    for hash in [0x00F9_EFBB, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE] {
        let Some(Value::Key(key)) = props.get(hash).and_then(|p| p.scalar()) else {
            continue;
        };
        if let Some((bytes, package, model_id)) = search.clone().find_map(|p| {
            let id = model_entry(p, key)?;
            Some((read_bounded(p, id)?, p, id))
        }) {
            let animation_state = matching_animation_state(&props, id, hash, |state| {
                search.clone().find_map(|p| {
                    let raw = PropertyFile::parse(&read_bounded(p, state)?).ok()?;
                    Some(flatten_lot_parents_with_sources(raw, p, manager, &sources))
                })
            });
            let candidates: Vec<_> = bindings(&props)
                .chain(choreography_clips(&props))
                .chain(animation_state.iter().flat_map(choreography_clips))
                .collect();
            if candidates.is_empty() {
                return Ok((Vec::new(), Vec::new()));
            }
            let (found, mut payload) = inspect_model(
                &bytes,
                &props,
                animation_state.as_ref(),
                &candidates,
                request.animation_id,
            );
            if !payload.is_empty() {
                let lotm = if request.refined {
                    Rw4File::parse(&bytes)
                        .ok()
                        .map(|file| {
                            build_lot_model_payload_for_test(
                                &file,
                                &bytes,
                                package,
                                manager,
                                model_id.instance,
                            )
                        })
                        .filter(|lotm| payload.len() + lotm.len() <= MAX_PREVIEW_BYTES)
                } else {
                    None
                };
                let len = lotm.as_ref().map_or(0, Vec::len) as u32;
                payload.extend(lotm.into_iter().flatten());
                payload.extend(len.to_le_bytes());
            }
            return Ok((found, payload));
        }
    }
    Ok((Vec::new(), Vec::new()))
}

async fn inspect_async(
    state: State<'_, AppState>,
    request: AnimationRequest,
) -> Result<(Vec<Binding>, Vec<u8>), CommandError> {
    let manager = Arc::clone(&state.packages);
    let store = Arc::clone(&state.store);
    tauri::async_runtime::spawn_blocking(move || inspect(&manager, &store, request))
        .await
        .map_err(|e| CommandError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn list_building_animations(
    state: State<'_, AppState>,
    mut request: AnimationRequest,
) -> Result<Vec<Binding>, CommandError> {
    request.animation_id = None;
    Ok(inspect_async(state, request).await?.0)
}

#[tauri::command]
pub async fn read_building_animation(
    state: State<'_, AppState>,
    request: AnimationRequest,
) -> Result<tauri::ipc::Response, CommandError> {
    if request.animation_id.is_none() {
        return Err(CommandError::internal("Missing animation ID"));
    }
    let (_, bytes) = inspect_async(state, request).await?;
    if bytes.is_empty() {
        return Err(CommandError::internal("Animation preview unavailable"));
    }
    Ok(tauri::ipc::Response::new(bytes))
}
