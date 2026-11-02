//! Application orchestration; processors and the connector own their formats.
use super::{
    connector,
    filesystem::{Staging, project_file},
    atlas_prepare, lotm, material_atlas, materials, native_model,
    plan::plan_from_schema,
    processor::defer_property,
    resource_closure,
    scene::{self, SceneDocument},
};
use dbpf::ResourceId;
use rw4::{Rw4File, SectionType};
use sc_properties::{
    Key, Kind, LOD_MODEL_HASHES, LOT_MASK_HASH, LOT_OVERLAY_OFFSET_HASH, LOT_PLACEMENT_HASH,
    LOT_SIZE_HASH, PropertyFile, Value,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub(crate) type LodModelIds = [ResourceId; 4];

#[derive(Debug, Clone, Serialize)]
pub struct M1BuildResult {
    pub path: PathBuf,
    pub schema_hash: String,
    pub build_hash: String,
    pub source_hash: String,
    pub sha256: String,
    /// Entries written into the output package (delta slice in delta mode).
    pub resource_count: usize,
    /// False when `outputs.mode: "full"` re-serializes the reference clone.
    pub delta_mode: bool,
    /// Instance of the menu-registered new building (`semantics.menu`).
    pub menu_instance: Option<u32>,
    pub patched_resources: usize,
    pub scope: &'static str,
    pub scene: SceneSemanticSummary,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SceneSemanticSummary {
    pub unit_count: usize,
    pub path_count: usize,
    pub path_entry_count: usize,
    /// Unit columns written back into the Lot property (0 = untouched clone).
    pub lowered_units: usize,
    /// Material texture resources produced from the model container.
    pub material_resources: usize,
}

/// Reference snapshot plus the delta slice this build actually created or
/// rewrote. `all` keeps the untouched reference resources readable (closure
/// checks, lot cloning, instance allocation); `touched` is what a clean
/// delta package writes.
pub struct BuildResources {
    all: connector::Resources,
    touched: std::collections::HashSet<ResourceId>,
}

impl std::ops::Deref for BuildResources {
    type Target = connector::Resources;

    fn deref(&self) -> &Self::Target {
        &self.all
    }
}

impl BuildResources {
    fn from_source(resources: connector::Resources) -> Self {
        Self {
            all: resources,
            touched: Default::default(),
        }
    }

    /// Record a created/rewritten resource. Inherent method wins over the
    /// `HashMap::insert` reachable through `Deref`, so every write site that
    /// goes through this is tracked for the delta package.
    fn insert(&mut self, id: ResourceId, bytes: Vec<u8>) {
        self.touched.insert(id);
        self.all.insert(id, bytes);
    }

    /// The full reference snapshot, used to resolve dependencies that were
    /// left in a cloned Property/RW4 but were not rewritten themselves.
    fn all(&self) -> &connector::Resources {
        &self.all
    }

    /// The delta slice: only entries this build created or rewrote.
    fn delta(&self) -> connector::Resources {
        self.touched
            .iter()
            .map(|id| (*id, self.all[id].clone()))
            .collect()
    }
}

pub fn build_from_schema(root: &Path, raw_schema: &str) -> Result<M1BuildResult, String> {
    let plan = plan_from_schema(raw_schema)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("project root must be a directory".into());
    }
    let source_path = project_file(
        &root,
        plan.intermediate
            .references
            .package
            .as_deref()
            .ok_or("missing source package")?,
    )?;
    let mut resources = BuildResources::from_source(connector::read_source(&source_path)?);
    let identity_resources = match plan
        .menu
        .as_ref()
        .and_then(|menu| menu.identity_package.as_deref())
    {
        Some(path) => Some(connector::read_source(&project_file(&root, path)?)?),
        None => None,
    };
    let scene_document = read_scene_document(
        &root,
        &plan.intermediate.scene.document,
        plan.intermediate.scene.model.as_deref(),
    )?;
    // Hash the decompressed snapshot actually consumed by the processors, not a
    // second file read. Container compression differences do not change identity.
    let mut texture_instances = [0u32; 6];
    for (slot, path) in &plan.intermediate.scene.slots {
        let index = slot
            .strip_prefix("slot")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| format!("invalid raster slot {slot}"))?;
        if index >= texture_instances.len() {
            return Err(format!("raster slot out of range: {slot}"));
        }
        let instance = insert_slot_raster(&root, &mut resources, path)?;
        texture_instances[index] = instance;
    }
    let (model_ids, material_resource_count) = insert_scene_model(
        &root,
        &plan.intermediate.scene.model,
        plan.intermediate.scene.model_shape.as_deref(),
        &mut resources,
        &texture_instances,
        scene_document.as_ref(),
        // Author orientation (backend settings.rotationDegrees, default 180);
        // parent-free builds carry it on the menu spec so geometry, ground
        // and prop/light matrices all share one value.
        plan.menu
            .as_ref()
            .and_then(|menu| menu.rotation_degrees)
            .unwrap_or(180),
    )?;
    let scene =
        validate_scene_path_semantics(&root, &plan.intermediate.scene.document, &resources)?;
    if scene_document
        .as_ref()
        .is_some_and(SceneDocument::has_semantics)
        && plan.intermediate.references.lot.is_none()
    {
        return Err(
            "scene document carries units/paths; reference.lot must name the Lot property to lower them into"
                .into(),
        );
    }
    let source_hash = match &identity_resources {
        Some(identity) => format!(
            "{}:{}",
            connector::fingerprint(&resources),
            connector::fingerprint(identity)
        ),
        None => connector::fingerprint(&resources),
    };
    let plan_bytes = serde_json::to_vec_pretty(&plan).map_err(|e| e.to_string())?;
    let build_hash = digest(
        &serde_json::to_vec(&(plan.processor_version, &source_hash, &plan_bytes))
            .map_err(|e| e.to_string())?,
    );
    let protected = protected_templates(&resources, &plan.protected);
    let mut targets: Vec<_> = plan.typed_patches.iter().map(|p| p.target).collect();
    targets.sort_by_key(|id| (id.type_id, id.group, id.instance));
    targets.dedup();
    let mut tasks = Vec::new();
    for target in &targets {
        if !plan.intermediate.semantics.allow_shared_template && protected.contains(target) {
            return Err(format!("refusing to patch shared template {target}"));
        }
        let source = resources
            .get(target)
            .ok_or_else(|| format!("missing Property target {target}"))?;
        let patches = plan
            .typed_patches
            .iter()
            .filter(|p| p.target == *target)
            .cloned()
            .collect();
        tasks.push(defer_property(*target, source.clone(), patches));
    }
    // No output directories are created until every Property processor succeeds.
    for task in tasks {
        let bytes = (task.materialize)().map_err(|e| format!("{}: {e}", task.target))?;
        resources.insert(task.target, bytes);
    }
    let mut lowered_units = 0usize;
    let to_id = |tgi: Option<crate::asset_build_m0::Tgi>| {
        tgi.map(|tgi| ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        })
    };
    // Unit columns belong on the Lot property; the Unit property (building
    // definition) only gets the LOD model retarget.  In menu-registration
    // mode the reference identity stays untouched 鈥?the scene lowers into
    // the cloned lot leaf instead.
    let menu_mode = plan.menu.is_some();
    let lot_target = to_id(plan.intermediate.references.lot);
    if let Some(target) = to_id(plan.intermediate.references.unit) {
        apply_property_semantics(
            &mut resources,
            target,
            None,
            if menu_mode { None } else { model_ids },
            &mut lowered_units,
        )?;
    }
    if let Some(target) = lot_target {
        apply_property_semantics(
            &mut resources,
            target,
            if menu_mode {
                None
            } else {
                scene_document.as_ref()
            },
            if menu_mode { None } else { model_ids },
            &mut lowered_units,
        )?;
    }
    let menu_instance = match &plan.menu {
        Some(menu) => {
            let lot_source = lot_target
                .ok_or("menu registration requires reference.lot to clone the lot identity")?;
            let generated = super::menu::generate(
                menu,
                &resources,
                lot_source,
                scene_document.as_ref(),
                model_ids,
                identity_resources.as_ref(),
            )?;
            for (id, bytes) in generated.entries {
                resources.insert(id, bytes);
            }
            Some(generated.instance)
        }
        None => None,
    };
    let delta_mode = plan.intermediate.output.mode != crate::asset_build_m0::OutputMode::Full;
    let mut staging = Staging::create(&root, &build_hash)?;
    staging.write("plan/plan.json", &plan_bytes)?;
    staging.write(
        "plan/im.json",
        &serde_json::to_vec_pretty(&plan.intermediate).map_err(|e| e.to_string())?,
    )?;
    if let Some(document) = read_scene_bytes(&root, &plan.intermediate.scene.document)? {
        staging.write("artifacts/scene/asset.lot.json", &document)?;
    }
    materialize_scene_resources(&root, &plan.intermediate, &resources, &mut staging)?;
    for target in &targets {
        let name = format!(
            "artifacts/property/{:08X}-{:08X}-{:08X}.property",
            target.type_id, target.group, target.instance
        );
        staging.write(&name, &resources[target])?;
    }
    // Delta output (default): only the resources this build created or
    // rewrote 鈥?the game resolves everything else from the base packages.
    // Full output re-serializes the whole reference snapshot (legacy
    // same-TGI overlay contract).
    let mut written: connector::Resources = if delta_mode {
        resources.delta()
    } else {
        std::mem::take(&mut resources.all)
    };
    if delta_mode {
        // A cloned Lot Property and a template-based RW4 can still point at
        // reference-package bytes.  Promote those bytes into the delta before
        // linking, and fail closed if a required reference is unresolved.
        let deps = resource_closure::discover(&written, menu_mode)?;
        resource_closure::complete(&mut written, resources.all(), &deps)?;
    }
    let output = connector::link(&written)?;
    staging.write(&package_relative_name(&plan), &output)?;
    connector::verify(&staging.path.join(&package_relative_name(&plan)), &written)?;
    let result = M1BuildResult {
        path: staging.destination.join(&package_relative_name(&plan)),
        schema_hash: plan.intermediate.schema_hash,
        build_hash,
        source_hash,
        sha256: digest(&output),
        resource_count: written.len(),
        delta_mode,
        menu_instance,
        patched_resources: targets.len(),
        scope: "property+scene-model",
        scene: SceneSemanticSummary {
            unit_count: scene.unit_count,
            path_count: scene.path_count,
            path_entry_count: scene.path_entry_count,
            lowered_units,
            material_resources: material_resource_count,
        },
    };
    staging.write(
        "reports/m1-build.json",
        &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )?;
    staging.publish()?;
    Ok(result)
}

fn package_relative_name(plan: &super::plan::PropertyBuildPlan) -> String {
    format!("packages/{}", plan.intermediate.output.package)
}

fn read_scene_document(
    root: &Path,
    document: &str,
    model: Option<&str>,
) -> Result<Option<SceneDocument>, String> {
    let Some(bytes) = read_scene_bytes(root, document)? else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("scene document is not valid JSON: {error}"))?;
    let mut parsed = scene::parse_document(&value)?;
    if let Some(model) = model {
        scene::merge_sidecar_manifests(root, model, &mut parsed)?;
    }
    Ok(Some(parsed))
}

fn read_scene_bytes(root: &Path, document: &str) -> Result<Option<Vec<u8>>, String> {
    let candidate = root.join(document);
    if !candidate.is_file() {
        return Ok(None);
    }
    let path = project_file(root, document)?;
    std::fs::read(&path)
        .map(Some)
        .map_err(|error| format!("scene document: {error}"))
}

/// Insert one schema raster slot; PNG payloads are encoded as native rasters,
/// anything else is preserved verbatim (already a game raster).
fn insert_slot_raster(
    root: &Path,
    resources: &mut BuildResources,
    path: &str,
) -> Result<u32, String> {
    let file = project_file(root, path)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("raster slot {path}: {e}"))?;
    // Building slot0 is commonly exported as the editor's JSON float4
    // parameter table. It must be wrapped in an RW4 paletteF32 resource;
    // treating the JSON bytes as a raster makes the model load gray because
    // the game cannot resolve the slot0 lookup texture.
    let (type_id, payload) = if path.to_ascii_lowercase().ends_with(".json") {
        let value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("parameter slot {path}: invalid JSON: {e}"))?;
        let cols = value
            .get("paramCols")
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| format!("parameter slot {path}: missing paramCols"))?;
        let values = value
            .get("paramsF32")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("parameter slot {path}: missing paramsF32"))?;
        let mut raw = Vec::with_capacity(values.len() * 4);
        for value in values {
            let number = value
                .as_f64()
                .ok_or_else(|| format!("parameter slot {path}: paramsF32 contains non-number"))?;
            raw.extend_from_slice(&(number as f32).to_le_bytes());
        }
        (
            materials::rw4_model_type(),
            materials::encode_params_rw4(
                &raw,
                u32::try_from(cols).map_err(|_| "paramCols overflow")?,
            )?,
        )
    } else if materials::is_png(&bytes) {
        (
            materials::raster_type(),
            materials::encode_png_raster(&bytes)?,
        )
    } else {
        (materials::raster_type(), bytes)
    };
    let instance = alloc_free_instance(resources, type_id, &payload);
    let id = ResourceId {
        type_id,
        group: 0,
        instance,
    };
    resources.insert(id, payload);
    Ok(instance)
}

/// Compile the scene model (LOTM container or plain GLB) into one RW4 static
/// model, inserting its material rasters/parameter tables into the package.
/// Returns the compiled model's resource id and the number of produced
/// material resources.
fn insert_scene_model(
    root: &Path,
    model: &Option<String>,
    model_shape: Option<&str>,
    resources: &mut BuildResources,
    slot_instances: &[u32; 6],
    scene_document: Option<&SceneDocument>,
    rotation_degrees: u32,
) -> Result<(Option<LodModelIds>, usize), String> {
    let Some(model_path) = model else {
        return Ok((None, 0));
    };
    let file = project_file(root, model_path)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("scene model: {e}"))?;
    let transform = scene_model_transform(scene_document);
    let merged = model_shape == Some("merged-single");
    if lotm::is_container(&bytes) {
        let doc = lotm::parse(&bytes)?;
        let mut material_plan = materials::resolve_materials(&doc)?;
        let mut atlas_cells: Option<Vec<[f32; 4]>> = None;
        if model_shape == Some("nh-atlas") {
            let atlas = material_atlas::build_tint_atlas(&material_plan)?;
            let head = material_plan
                .unique
                .first_mut()
                .ok_or("atlas compile: material plan empty")?;
            let source_params = head.slots[0].as_ref().map(|(bytes, _)| bytes.as_slice());
            let identity = material_atlas::identity_params(source_params);
            let identity_rw4 = materials::encode_params_rw4(&identity, 1)?;
            head.slots[0] = Some((identity_rw4, false));
            head.slots[1] = Some((atlas.png, true));
            atlas_cells = Some(atlas.cells);
        }
        let material_resources =
            materials::build_resources(&material_plan, |type_id, prepared| {
                Ok(alloc_free_instance(resources, type_id, prepared))
            })?;
        for (type_id, instance, payload) in &material_resources.payloads {
            resources.insert(
                ResourceId {
                    type_id: *type_id,
                    group: 0,
                    instance: *instance,
                },
                payload.clone(),
            );
        }
        // 鍟嗕笟 LOTM 甯歌涓哄嚑鍗佸埌涓婄櫨涓紪杈戝櫒 mesh銆傛父鎴忔ā鍨嬪姞杞藉櫒鍙帴鍙?        // 灏戦噺褰㈢姸锛涢粯璁よ矾寰勫繀椤昏嚜鍔ㄦ敹鏁涘埌 NH 鍗曞舰鐘讹紝閬垮厤 schema 婕忓啓
        // modelShape 鏃堕噸鏂扮敓鎴愨€滃鍎垮舰鐘垛€漅W4锛堝巻鍙蹭笂鏇惧嚭鐜?105 mesh锛夈€?
        let safe_nh = doc.meshes.len() > 4;
        let use_nh_partitioned = model_shape == Some("nh-partitioned") || safe_nh;
        // Select a real game RW4 from the reference package as the container
        // skeleton. Geometry is replaced later; its header/material/collision
        // sections remain byte-compatible with the game.
        let native_template = std::env::var("OPENSCP_NATIVE_TEMPLATE")
            .ok()
            .map(std::fs::read)
            .transpose()
            .map_err(|error| format!("read native template: {error}"))?
            .or_else(|| {
                Some(
                    include_bytes!("../../resources/sc5/175EFB4E.research-template.rw4")
                        .to_vec(),
                )
            })
            .or_else(|| resources
            .iter()
            .filter(|(id, bytes)| {
                id.type_id == materials::rw4_model_type()
                    && rw4::Rw4File::parse(bytes)
                        .map(|file| file.sections_of_type(rw4::SectionType::MESH).next().is_some())
                        .unwrap_or(false)
            })
            .min_by_key(|(id, _)| (id.group, id.instance))
            .map(|(_, bytes)| bytes.clone()));
        let compiled = if merged {
            // 鍘熺増鍗曞舰鐘讹細鍏ㄩ儴鍑犱綍鍚堝苟 + 鍙紩鐢ㄦ潗璐?0 鐨勫叚妲斤紙鍏朵綑鏉愯川
            // raster 璧勬簮鐓у父鍏ュ簱锛屾ā鍨嬩笉寮曠敤锛夈€?
            let head = material_plan
                .mesh_material
                .first()
                .copied()
                .ok_or("merged compile: material plan empty")?;
            let head_slots = material_resources
                .unique_instances
                .get(head as usize)
                .copied()
                .ok_or("merged compile: head material slots missing")?;
            native_model::compile_lotm_merged(
                &doc,
                &material_plan.mesh_material,
                std::slice::from_ref(&head_slots),
                transform,
            )?
        } else if model_shape == Some("native-multi") {
            // Plugin-native multi-material export: keep one RW4 mesh per
            // unique LOTM material. This is the structure the SC5 exporter
            // writes for assets with many window/decal recipes; it must not
            // be flattened into the four-mesh/one-material safe shape.
            native_model::compile_lotm_nh_partitioned(
                &doc,
                &material_plan.mesh_material,
                &material_resources.unique_instances,
                transform,
            )?
        } else if model_shape == Some("nh-atlas") {
            let cells = atlas_cells.ok_or("atlas compile: cells missing")?;
            let mesh_cells: Vec<[f32; 4]> = material_plan
                .mesh_material
                .iter()
                .map(|material| {
                    cells
                        .get(*material as usize)
                        .copied()
                        .unwrap_or([1.0, 1.0, 0.0, 0.0])
                })
                .collect();
            let atlas_doc = atlas_prepare::remap_facade_uvs(&doc, &mesh_cells)?;
            let zeroed = vec![0u32; doc.meshes.len()];
            let head_slots = material_resources
                .unique_instances
                .first()
                .copied()
                .ok_or("atlas compile: head material slots missing")?;
            match native_template.as_deref() {
                Some(template) => native_model::compile_lotm_nh_template(
                    &atlas_doc,
                    &zeroed,
                    std::slice::from_ref(&head_slots),
                    transform,
                    template,
                )?,
                None => native_model::compile_lotm_nh_partitioned(
                    &atlas_doc,
                    &zeroed,
                    std::slice::from_ref(&head_slots),
                    transform,
                )?,
            }
        } else if model_shape == Some("nh-multimaterial") {
            // Preserve the four largest facade material groups instead of
            // collapsing every mesh to material 0.  The game's native census
            // allows up to four meshes per model; this keeps the main facade
            // regions while staying inside that envelope.
            let (top, mapped) = select_material_groups(&doc, &material_plan, 4)?;
            let instances: Vec<[u32; 6]> = top
                .iter()
                .map(|index| material_resources.unique_instances[*index])
                .collect();
            native_model::compile_lotm_nh_partitioned(&doc, &mapped, &instances, transform)?
        } else if model_shape == Some("building40") {
            let template = native_template
                .as_deref()
                .ok_or("building40 compile: native template missing")?;
            // Author semantics (compile_tower_asset/compile_geometry_contract):
            // materialRow = the mesh's material group — the author's LOTM
            // format pairs chunk i with material i, so this equals the group
            // order for native exports; honoring the container's own mapping
            // keeps third-party containers correct too.
            let rows: Vec<u32> = (0..doc.meshes.len())
                .map(|mesh| {
                    doc.mesh_material
                        .get(mesh)
                        .map(|reference| reference.material)
                        .unwrap_or(mesh as u32)
                })
                .collect();
            let compiled = native_model::compile_building40_template(
                &doc,
                &rows,
                transform,
                rotation_degrees,
                template,
                include_bytes!("../../resources/sc5/collision-source.rw4"),
            )?;
            let head_slots = material_resources
                .unique_instances
                .first()
                .copied()
                .ok_or("building40 compile: head material slots missing")?;
            let rw4 = super::template_material::rewrite_texture_instances(&compiled.rw4, &head_slots)?;
            native_model::CompiledNativeModel { rw4, ..compiled }
        } else if use_nh_partitioned {
            // NH 椤剁偣甯冨眬锛坰tride 40 鍘嬬缉娉曠嚎/鍒囩嚎 + facade F4 鍗?UV 娴侊級
            // 鐨勫崟褰㈢姸锛氬叏閮ㄥ嚑浣曞悎鍏ヤ竴涓?mesh锛屾潗璐ㄥ彇澶撮儴鏉愯川鐨勫叚妲斤紱缁?            // 椤剁偣 >65536 鏃?u32 绱㈠紩锛圱A 鏍囧織 102锛夈€?026-10-31 R9 涓婃父鎴?            // 楠岃瘉锛氳褰㈡€佸妤煎畬鏁存覆鏌撱€傞€?GLB mesh 涓€ RW4 mesh 鐨勫舰鐘?            // 锛?8-105 mesh锛夋槸 census 瀹炶瘉浼氫护鍔犺浇瀛樻。宕╂簝鐨勫鍎垮舰鐘躲€?
            let (dominant, _) = select_material_groups(&doc, &material_plan, 1)?;
            let head = dominant
                .first()
                .copied()
                .ok_or("nh-partitioned compile: material plan empty")? as u32;
            let head_slots = material_resources
                .unique_instances
                .get(head as usize)
                .copied()
                .ok_or("nh-partitioned compile: head material slots missing")?;
            let mesh_material_zeroed = vec![0u32; material_plan.mesh_material.len()];
            match native_template.as_deref() {
                Some(template) => native_model::compile_lotm_nh_template(
                    &doc,
                    &mesh_material_zeroed,
                    std::slice::from_ref(&head_slots),
                    transform,
                    template,
                )?,
                None => native_model::compile_lotm_nh_partitioned(
                    &doc,
                    &mesh_material_zeroed,
                    std::slice::from_ref(&head_slots),
                    transform,
                )?,
            }
        } else {
            native_model::compile_lotm(
                &doc,
                &material_plan.mesh_material,
                &material_resources.unique_instances,
                transform,
            )?
        };
        let compiled_file = Rw4File::parse(&compiled.rw4)
            .map_err(|error| format!("compiled model parse: {error}"))?;
        let mesh_count = compiled_file.sections_of_type(SectionType::MESH).count();
        let max_meshes = if model_shape == Some("native-multi") {
            128
        } else {
            4
        };
        if mesh_count > max_meshes {
            return Err(format!(
                "compiled LOTM model has {mesh_count} RW4 meshes; maximum supported is {max_meshes}"
            ));
        }
        let model_ids = if model_shape == Some("native-multi") {
            // Reuse one multi-material model for all four LOD keys. The four
            // copies were byte-identical; keeping one keeps the package below
            // the loader's practical size limit for a 98-material model.
            insert_compiled_model_single(resources, &compiled.rw4)
        } else {
            insert_compiled_model(resources, &compiled.rw4)
        };
        materials::validate_model_resource_closure(&compiled.rw4, resources)
            .map_err(|error| format!("compiled model resource closure: {error}"))?;
        Ok((Some(model_ids), material_resources.payloads.len()))
    } else {
        // Plain GLB: single material bound to the schema raster slots.
        let doc = lotm::LotmDocument {
            version: 0,
            meshes: vec![bytes],
            materials: Vec::new(),
            mesh_material: vec![lotm::MeshMaterialRef {
                material: 0,
                uv_kind: 0,
            }],
            diagnostics: String::new(),
        };
        let compiled = native_model::compile_lotm(
            &doc,
            &[0],
            std::slice::from_ref(slot_instances),
            transform,
        )?;
        let model_ids = insert_compiled_model(resources, &compiled.rw4);
        materials::validate_model_resource_closure(&compiled.rw4, resources)
            .map_err(|error| format!("compiled model resource closure: {error}"))?;
        Ok((Some(model_ids), 0))
    }
}

/// Rank materials by triangle area and remap every mesh into one of the top
/// groups.  This is the bridge between the LOTM's many per-facade materials
/// and the game's four-mesh budget.
fn select_material_groups(
    doc: &lotm::LotmDocument,
    plan: &materials::MaterialPlan,
    max_groups: usize,
) -> Result<(Vec<usize>, Vec<u32>), String> {
    let mut area = vec![0f64; plan.unique.len()];
    for (mesh_index, glb) in doc.meshes.iter().enumerate() {
        let primitives = native_model::parse_glb_primitives(glb)?;
        let material = plan
            .mesh_material
            .get(mesh_index)
            .copied()
            .unwrap_or(0) as usize;
        if material >= area.len() {
            continue;
        }
        for primitive in primitives {
            for triangle in primitive.indices.chunks_exact(3) {
                let a = primitive.positions[triangle[0] as usize];
                let b = primitive.positions[triangle[1] as usize];
                let c = primitive.positions[triangle[2] as usize];
                let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let cross = [
                    ab[1] * ac[2] - ab[2] * ac[1],
                    ab[2] * ac[0] - ab[0] * ac[2],
                    ab[0] * ac[1] - ab[1] * ac[0],
                ];
                area[material] += 0.5
                    * ((cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]) as f64)
                        .sqrt();
            }
        }
    }
    let mut order: Vec<usize> = (0..area.len()).collect();
    order.sort_by(|a, b| area[*b].partial_cmp(&area[*a]).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<usize> = order.into_iter().take(max_groups.max(1)).collect();
    if top.is_empty() {
        return Err("multi-material compile: no material groups".into());
    }
    let mut rank = vec![0u32; area.len()];
    for (index, material) in top.iter().enumerate() {
        rank[*material] = index as u32;
    }
    let mapped: Vec<u32> = plan
        .mesh_material
        .iter()
        .map(|material| rank.get(*material as usize).copied().unwrap_or(0))
        .collect();
    Ok((top, mapped))
}

fn scene_model_transform(document: Option<&SceneDocument>) -> Option<[f32; 12]> {
    document.and_then(|document| document.model_transform)
}

/// Native building assets carry four independent LOD resources, even when
/// their bytes are identical. Reuse the same bytes but allocate four
/// instances so the game's LOD ownership matches NightHouse/retail.
fn insert_compiled_model(resources: &mut BuildResources, rw4: &[u8]) -> LodModelIds {
    let mut ids = [ResourceId {
        type_id: materials::rw4_model_type(),
        group: 0,
        instance: 0,
    }; 4];
    for id in &mut ids {
        let instance = alloc_free_instance(resources, materials::rw4_model_type(), rw4);
        *id = ResourceId {
            type_id: materials::rw4_model_type(),
            group: 0,
            instance,
        };
        resources.insert(*id, rw4.to_vec());
    }
    ids
}

fn insert_compiled_model_single(resources: &mut BuildResources, rw4: &[u8]) -> LodModelIds {
    let instance = alloc_free_instance(resources, materials::rw4_model_type(), rw4);
    let id = ResourceId {
        type_id: materials::rw4_model_type(),
        group: 0,
        instance,
    };
    resources.insert(id, rw4.to_vec());
    [id; 4]
}

/// Deterministic instance allocation: content hash, linearly probed on
/// collision within the build's resource set.
fn alloc_free_instance(resources: &connector::Resources, type_id: u32, bytes: &[u8]) -> u32 {
    let mut instance = u32::from_be_bytes(Sha256::digest(bytes)[..4].try_into().unwrap());
    while resources.contains_key(&ResourceId {
        type_id,
        group: 0,
        instance,
    }) {
        instance = instance.wrapping_add(1);
    }
    instance
}

/// Fetch the target property, apply scene semantics + LOD retarget, and store
/// the re-encoded bytes when anything changed.
fn apply_property_semantics(
    resources: &mut BuildResources,
    target: ResourceId,
    document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
    lowered_units: &mut usize,
) -> Result<(), String> {
    let Some(bytes) = resources.get(&target).cloned() else {
        return Ok(());
    };
    if let Some((encoded, units)) = apply_scene_lot_semantics(&bytes, document, model_ids)? {
        *lowered_units = (*lowered_units).max(units);
        resources.insert(target, encoded);
    }
    Ok(())
}

/// Apply the scene semantics that live on the Lot/Unit property itself:
/// unit column writeback, LotSize/Offset/Placement, and LOD model retarget.
/// Returns `(encoded bytes, lowered unit count)`; `None` when nothing was
/// touched and the property can stay byte-identical.
pub(crate) fn apply_scene_lot_semantics(
    bytes: &[u8],
    document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
) -> Result<Option<(Vec<u8>, usize)>, String> {
    let mut file = PropertyFile::parse(bytes).map_err(|e| e.to_string())?;
    let mut touched = false;
    let mut lowered_units = 0usize;
    if let Some(document) = document {
        if document.has_semantics() {
            scene::apply_units(&mut file, document)?;
            lowered_units = document.units.len();
            touched = true;
        }
        if let Some([x, y]) = document.lot_size {
            set_scalar(&mut file, LOT_SIZE_HASH, Value::Vector2([x, y]));
            touched = true;
        }
        if let Some([x, y]) = document.lot_offset {
            set_scalar(&mut file, LOT_OVERLAY_OFFSET_HASH, Value::Vector2([x, y]));
            touched = true;
        }
        if let Some(matrix) = &document.placement {
            if matrix.len() == 12 {
                set_scalar(
                    &mut file,
                    LOT_PLACEMENT_HASH,
                    Value::Transform(sc_properties::Transform {
                        flags: 15,
                        unknown: None,
                        matrix: matrix.clone(),
                    }),
                );
                touched = true;
            }
        }
    }
    if let Some(model_ids) = model_ids {
        touched |= retarget_lod_models(&mut file, model_ids);
    }
    if !touched {
        return Ok(None);
    }
    let encoded = file
        .encode_canonical()
        .map_err(|e| format!("lot semantics encode: {e}"))?;
    let readback =
        PropertyFile::parse(&encoded).map_err(|e| format!("lot semantics readback: {e}"))?;
    if let Some(document) = document {
        if document.has_semantics() {
            scene::verify_units(&readback, document)?;
        }
    }
    Ok(Some((encoded, lowered_units)))
}

/// Point LOD1..LOD4 at four distinct compiled model resources, matching the
/// native NightHouse asset graph.  Missing LOD keys are created.
fn retarget_lod_models(file: &mut PropertyFile, model_ids: LodModelIds) -> bool {
    for (index, hash) in LOD_MODEL_HASHES.into_iter().enumerate() {
        let model_id = model_ids[index];
        match file.values.iter_mut().find(|property| property.hash == hash) {
            Some(property) => {
                property.kind = Kind::Scalar(Value::Key(Key {
                    type_id: model_id.type_id,
                    group: model_id.group,
                    instance: model_id.instance,
                }));
            }
            None => file.values.push(sc_properties::Property {
                hash,
                prop_type: sc_properties::PropType::Key,
                kind: Kind::Scalar(Value::Key(Key {
                    type_id: model_id.type_id,
                    group: model_id.group,
                    instance: model_id.instance,
                })),
                encoding: sc_properties::PropertyEncoding::default(),
            }),
        }
    }
    true
}

fn set_scalar(file: &mut PropertyFile, hash: u32, value: Value) {
    match file.values.iter_mut().find(|p| p.hash == hash) {
        Some(property) => {
            property.kind = Kind::Scalar(value);
        }
        None => file.values.push(make_scalar_property(hash, value)),
    }
}

fn make_scalar_property(hash: u32, value: Value) -> sc_properties::Property {
    let prop_type = match &value {
        Value::Vector2(_) => sc_properties::PropType::Vector2,
        Value::Transform(_) => sc_properties::PropType::Transform,
        Value::Key(_) => sc_properties::PropType::Key,
        _ => sc_properties::PropType::Float,
    };
    sc_properties::Property {
        hash,
        prop_type,
        kind: Kind::Scalar(value),
        encoding: sc_properties::PropertyEncoding::default(),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
struct SceneResourceManifest {
    lot: Vec<String>,
    prop: Vec<String>,
    bin: Vec<String>,
    path: Vec<String>,
    path_entry: Vec<String>,
    raster: Vec<String>,
    missing: Vec<String>,
}

fn materialize_scene_resources(
    root: &Path,
    intermediate: &crate::asset_build_m0::IntermediateModel,
    resources: &connector::Resources,
    staging: &mut Staging,
) -> Result<(), String> {
    let mut manifest = SceneResourceManifest::default();
    let mut requested: Vec<(&str, ResourceId)> = Vec::new();
    if let Some(lot) = intermediate.references.lot {
        requested.push((
            "lot",
            ResourceId {
                type_id: lot.type_id,
                group: lot.group,
                instance: lot.instance,
            },
        ));
    }
    if let Ok(Some(document)) = read_scene_json(root, &intermediate.scene.document) {
        if let Some(units) = document.get("units").and_then(serde_json::Value::as_array) {
            for unit in units {
                let kind = match unit.get("kind").and_then(serde_json::Value::as_str) {
                    Some("bin") => "bin",
                    _ => "prop",
                };
                if matches!(kind, "prop" | "bin") {
                    if let Some(tgi) = unit.get("modelTgi").and_then(scene::parse_json_tgi) {
                        requested.push((kind, tgi));
                    }
                }
            }
        }
        if let Some(paths) = document
            .pointer("/editor/workbench/paths")
            .and_then(serde_json::Value::as_array)
        {
            for path in paths {
                if let Some(tgi) = path
                    .get("source")
                    .and_then(|source| source.get("tgi"))
                    .or_else(|| path.get("source"))
                    .and_then(scene::parse_json_tgi)
                {
                    requested.push(("path", tgi));
                }
                if let Some(entry) = path
                    .get("entry")
                    .and_then(serde_json::Value::as_str)
                    .and_then(scene::parse_hex_u32)
                {
                    requested.push((
                        "path_entry",
                        ResourceId {
                            type_id: 0x00B1_B104,
                            group: 0x40E0_C400,
                            instance: entry,
                        },
                    ));
                }
            }
        }
    }
    let targets: Vec<_> = intermediate
        .semantics
        .properties
        .iter()
        .filter_map(|value| value.get("tgi").and_then(scene::parse_json_tgi))
        .collect();
    for id in intermediate
        .references
        .lot
        .into_iter()
        .map(|tgi| ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        })
        .chain(targets)
    {
        if let Some((_, bytes)) = find_resource(resources, id) {
            if let Ok(file) = PropertyFile::parse(bytes) {
                if let Some(key) = file.get(LOT_MASK_HASH).and_then(property_key) {
                    requested.push((
                        "raster",
                        ResourceId {
                            type_id: materials::raster_type(),
                            group: key.group,
                            instance: key.instance,
                        },
                    ));
                }
            }
        }
    }
    requested.sort_by_key(|(_, id)| (id.type_id, id.group, id.instance));
    requested.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    for (kind, id) in requested {
        let key = format!("{:08X}-{:08X}-{:08X}", id.type_id, id.group, id.instance);
        let Some((resolved_id, bytes)) = find_resource(resources, id) else {
            manifest.missing.push(format!("{kind}:{key}"));
            continue;
        };
        let resolved_key = format!(
            "{:08X}-{:08X}-{:08X}",
            resolved_id.type_id, resolved_id.group, resolved_id.instance
        );
        staging.write(&format!("artifacts/{kind}/{resolved_key}.resource"), bytes)?;
        match kind {
            "lot" => manifest.lot.push(resolved_key),
            "prop" => manifest.prop.push(resolved_key),
            "bin" => manifest.bin.push(resolved_key),
            "path" => manifest.path.push(resolved_key),
            "path_entry" => manifest.path_entry.push(resolved_key),
            "raster" => manifest.raster.push(resolved_key),
            _ => {}
        }
    }
    staging.write(
        "artifacts/resource-manifest.json",
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}

fn read_scene_json(root: &Path, document: &str) -> Result<Option<serde_json::Value>, String> {
    match read_scene_bytes(root, document)? {
        None => Ok(None),
        Some(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| format!("scene document is not valid JSON: {error}")),
    }
}

fn find_resource<'a>(
    resources: &'a connector::Resources,
    id: ResourceId,
) -> Option<(ResourceId, &'a Vec<u8>)> {
    resources
        .get_key_value(&id)
        .map(|(resolved, bytes)| (*resolved, bytes))
        .or_else(|| {
            resources
                .iter()
                .find(|(candidate, _)| {
                    candidate.type_id == id.type_id && candidate.instance == id.instance
                })
                .map(|(resolved, bytes)| (*resolved, bytes))
        })
}

fn property_key(property: &sc_properties::Property) -> Option<Key> {
    match &property.kind {
        Kind::Scalar(Value::Key(key)) => Some(*key),
        _ => None,
    }
}

fn validate_scene_path_semantics(
    root: &Path,
    document: &str,
    resources: &connector::Resources,
) -> Result<SceneSemanticSummary, String> {
    let Some(value) = read_scene_json(root, document)? else {
        return Ok(SceneSemanticSummary::default());
    };
    let Some(paths) = value
        .pointer("/editor/workbench/paths")
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(SceneSemanticSummary {
            unit_count: value
                .pointer("/units")
                .and_then(serde_json::Value::as_array)
                .map_or(0, Vec::len),
            ..Default::default()
        });
    };
    let mut errors = Vec::new();
    let unit_count = value
        .pointer("/units")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let mut path_entry_count = 0;
    for (index, path) in paths.iter().enumerate() {
        let location = format!("editor.workbench.paths[{index}]");
        let Some(entry_text) = path.get("entry").and_then(serde_json::Value::as_str) else {
            errors.push(format!("{location}.entry: missing path entry"));
            continue;
        };
        path_entry_count += 1;
        let Some(entry_instance) = scene::parse_hex_u32(entry_text) else {
            errors.push(format!("{location}.entry: invalid path entry {entry_text}"));
            continue;
        };
        let Some(tgi) = path
            .get("source")
            .and_then(|source| source.get("tgi"))
            .or_else(|| path.get("source"))
        else {
            errors.push(format!("{location}.source.tgi: missing path source"));
            continue;
        };
        let Some(source_tgi) = scene::parse_json_tgi(tgi) else {
            errors.push(format!("{location}.source.tgi: invalid path source"));
            continue;
        };
        let source = resources.get(&source_tgi).or_else(|| {
            resources
                .iter()
                .find(|(id, _)| {
                    id.type_id == source_tgi.type_id && id.instance == source_tgi.instance
                })
                .map(|(_, bytes)| bytes)
        });
        let Some(source) = source else {
            errors.push(format!(
                "{location}.source.tgi: source property {source_tgi} is missing"
            ));
            continue;
        };
        let Ok(file) = sc_properties::PropertyFile::parse(source) else {
            errors.push(format!("{location}.source.tgi: source property is invalid"));
            continue;
        };
        // Per-path Key property (0x0CC8FE7A) on the path source property must
        // bind this entry instance.
        let bound = file.get(0x0CC8_FE7A).is_some_and(|property| match &property.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) => key.instance == entry_instance,
            sc_properties::Kind::Array(values) => values.iter().any(|value| matches!(value, sc_properties::Value::Key(key) if key.instance == entry_instance)),
            _ => false,
        });
        if !bound {
            errors.push(format!(
                "{location}.entry: {entry_text} is not bound by source property 0x0CC8FE7A"
            ));
        }
        let resource_exists = resources.keys().any(|id| {
            id.type_id == 0x00B1_B104
                && id.instance == entry_instance
                && (id.group & 0xFFFF) == 0xC400
        });
        if !resource_exists {
            errors.push(format!(
                "{location}.entry: PathEntry resource instance 0x{entry_instance:08X} is missing"
            ));
        }
    }
    if errors.is_empty() {
        Ok(SceneSemanticSummary {
            unit_count,
            path_count: paths.len(),
            path_entry_count,
            ..Default::default()
        })
    } else {
        Err(format!(
            "path semantic validation failed: {}",
            errors.join("; ")
        ))
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn protected_templates(
    resources: &connector::Resources,
    declared: &HashSet<ResourceId>,
) -> HashSet<ResourceId> {
    let mut result = declared.clone();
    for (id, bytes) in resources {
        if id.type_id != sc_properties::PROPERTY_RESOURCE_TYPE {
            continue;
        }
        let Ok(file) = PropertyFile::parse(bytes) else {
            continue;
        };
        let Some(parent) = file.get(sc_properties::PARENT_HASH) else {
            continue;
        };
        let values = match &parent.kind {
            Kind::Scalar(v) => std::slice::from_ref(v),
            Kind::Array(v) => v.as_slice(),
            Kind::Empty => &[],
        };
        for value in values {
            if let Value::Key(key) = value {
                result.insert(ResourceId {
                    type_id: key.type_id,
                    group: key.group,
                    instance: key.instance,
                });
            }
        }
    }
    result
}
