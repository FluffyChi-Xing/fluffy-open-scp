//! Application orchestration; processors and the connector own their formats.
use super::{
    connector,
    filesystem::{Staging, project_file},
    lotm, materials, native_model,
    plan::plan_from_schema,
    processor::defer_property,
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
    let scene_document = read_scene_document(&root, &plan.intermediate.scene.document)?;
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
    let (model_id, material_resource_count) = insert_scene_model(
        &root,
        &plan.intermediate.scene.model,
        plan.intermediate.scene.model_shape.as_deref(),
        &mut resources,
        &texture_instances,
        scene_document.as_ref(),
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
    let source_hash = connector::fingerprint(&resources);
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
    // mode the reference identity stays untouched — the scene lowers into
    // the cloned lot leaf instead.
    let menu_mode = plan.menu.is_some();
    let lot_target = to_id(plan.intermediate.references.lot);
    if let Some(target) = to_id(plan.intermediate.references.unit) {
        apply_property_semantics(
            &mut resources,
            target,
            None,
            if menu_mode { None } else { model_id },
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
            if menu_mode { None } else { model_id },
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
                model_id,
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
    // rewrote — the game resolves everything else from the base packages.
    // Full output re-serializes the whole reference snapshot (legacy
    // same-TGI overlay contract).
    let written: connector::Resources = if delta_mode {
        resources.delta()
    } else {
        std::mem::take(&mut resources.all)
    };
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

fn read_scene_document(root: &Path, document: &str) -> Result<Option<SceneDocument>, String> {
    let Some(bytes) = read_scene_bytes(root, document)? else {
        return Ok(None);
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("scene document is not valid JSON: {error}"))?;
    Ok(Some(scene::parse_document(&value)?))
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
) -> Result<(Option<ResourceId>, usize), String> {
    let Some(model_path) = model else {
        return Ok((None, 0));
    };
    let file = project_file(root, model_path)?;
    let bytes = std::fs::read(&file).map_err(|e| format!("scene model: {e}"))?;
    let transform = scene_model_transform(scene_document);
    let merged = model_shape == Some("merged-single");
    if lotm::is_container(&bytes) {
        let doc = lotm::parse(&bytes)?;
        let material_plan = materials::resolve_materials(&doc)?;
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
        // 商业 LOTM 常见为几十到上百个编辑器 mesh。游戏模型加载器只接受
        // 少量形状；默认路径必须自动收敛到 NH 单形状，避免 schema 漏写
        // modelShape 时重新生成“孤儿形状”RW4（历史上曾出现 105 mesh）。
        let safe_nh = doc.meshes.len() > 4;
        let use_nh_partitioned = model_shape == Some("nh-partitioned") || safe_nh;
        // Select a real game RW4 from the reference package as the container
        // skeleton. Geometry is replaced later; its header/material/collision
        // sections remain byte-compatible with the game.
        let native_template = resources
            .iter()
            .filter(|(id, bytes)| {
                id.type_id == materials::rw4_model_type()
                    && rw4::Rw4File::parse(bytes)
                        .map(|file| file.sections_of_type(rw4::SectionType::MESH).next().is_some())
                        .unwrap_or(false)
            })
            .min_by_key(|(id, _)| (id.group, id.instance))
            .map(|(_, bytes)| bytes.clone());
        let compiled = if merged {
            // 原版单形状：全部几何合并 + 只引用材质 0 的六槽（其余材质
            // raster 资源照常入库，模型不引用）。
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
        } else if use_nh_partitioned {
            // NH 顶点布局（stride 40 压缩法线/切线 + facade F4 单 UV 流）
            // 的单形状：全部几何合入一个 mesh，材质取头部材质的六槽；组
            // 顶点 >65536 时 u32 索引（TA 标志 102）。2026-10-31 R9 上游戏
            // 验证：该形态塔楼完整渲染。逐 GLB mesh 一 RW4 mesh 的形状
            // （98-105 mesh）是 census 实证会令加载存档崩溃的孤儿形状。
            let head = material_plan
                .mesh_material
                .first()
                .copied()
                .ok_or("nh-partitioned compile: material plan empty")?;
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
        if mesh_count > 4 {
            return Err(format!(
                "compiled LOTM model has {mesh_count} RW4 meshes; maximum supported is 4"
            ));
        }
        let model_id = insert_compiled_model(resources, &compiled.rw4);
        materials::validate_model_resource_closure(&compiled.rw4, resources)
            .map_err(|error| format!("compiled model resource closure: {error}"))?;
        Ok((Some(model_id), material_resources.payloads.len()))
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
        let model_id = insert_compiled_model(resources, &compiled.rw4);
        materials::validate_model_resource_closure(&compiled.rw4, resources)
            .map_err(|error| format!("compiled model resource closure: {error}"))?;
        Ok((Some(model_id), 0))
    }
}

fn scene_model_transform(document: Option<&SceneDocument>) -> Option<[f32; 12]> {
    document.and_then(|document| document.model_transform)
}

fn insert_compiled_model(resources: &mut BuildResources, rw4: &[u8]) -> ResourceId {
    let instance = alloc_free_instance(resources, materials::rw4_model_type(), rw4);
    let id = ResourceId {
        type_id: materials::rw4_model_type(),
        group: 0,
        instance,
    };
    resources.insert(id, rw4.to_vec());
    id
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
    model_id: Option<ResourceId>,
    lowered_units: &mut usize,
) -> Result<(), String> {
    let Some(bytes) = resources.get(&target).cloned() else {
        return Ok(());
    };
    if let Some((encoded, units)) = apply_scene_lot_semantics(&bytes, document, model_id)? {
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
    model_id: Option<ResourceId>,
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
    if let Some(model_id) = model_id {
        touched |= retarget_lod_models(&mut file, model_id);
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

/// Point every present LOD model hash (LOD1..4) at the compiled model.  A
/// property without any LOD key gains LOD1 — otherwise the built lot would
/// keep rendering its referenced building (问题3's "模型丢失").
fn retarget_lod_models(file: &mut PropertyFile, model_id: ResourceId) -> bool {
    let mut found = false;
    for hash in LOD_MODEL_HASHES {
        if let Some(property) = file.values.iter_mut().find(|p| p.hash == hash) {
            if let Kind::Scalar(Value::Key(key)) = &mut property.kind {
                key.type_id = model_id.type_id;
                key.group = model_id.group;
                key.instance = model_id.instance;
                found = true;
            }
        }
    }
    if found {
        return true;
    }
    file.values.push(sc_properties::Property {
        hash: sc_properties::LOD1_MODEL_HASH,
        prop_type: sc_properties::PropType::Key,
        kind: Kind::Scalar(Value::Key(Key {
            type_id: model_id.type_id,
            group: model_id.group,
            instance: model_id.instance,
        })),
        encoding: sc_properties::PropertyEncoding::default(),
    });
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
