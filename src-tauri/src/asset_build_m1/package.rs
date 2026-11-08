//! Application orchestration; processors and the connector own their formats.
use super::{
    atlas_prepare, connector,
    filesystem::{project_file, Staging},
    lotm, material_atlas, materials, native_model,
    plan::plan_from_schema,
    processor::defer_property,
    resource_closure,
    scene::{self, SceneDocument},
    stages,
};
use dbpf::ResourceId;
use rw4::{Rw4File, SectionType};
use sc_properties::{
    Key, Kind, PropertyFile, Value, LOD_MODEL_HASHES, LOT_MASK_HASH, LOT_OVERLAY_OFFSET_HASH,
    LOT_PLACEMENT_HASH, LOT_SIZE_HASH,
};
use serde::{Deserialize, Serialize};
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
    /// 责任链阶段事件(Actions 页数据源;不参与 build_hash/产物字节)。
    pub events: Vec<super::events::StageEvent>,
    pub started_at_ms: u64,
    pub finished_at_ms: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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
    /// 空集(stages.rs 的 BuildContext 初始化用;真实内容在 load_inputs 装载)。
    pub(crate) fn empty() -> Self {
        Self {
            all: Default::default(),
            touched: Default::default(),
        }
    }

    /// vanilla 影子剥离（2026-11-08 HopeTower 事故后新增）：移除
    /// shadow_groups 中 instance 已存在于游戏包的资源（游戏运行时自行
    /// 提供），返回被剥离清单。防止参照闭包把游戏基类模板带进部署包
    /// 遮蔽原版（东倒西歪事故根因）。
    pub(crate) fn strip_vanilla_shadows(
        &mut self,
        shadow_groups: &[u32],
        vanilla: &std::collections::HashSet<(u32, u32)>,
    ) -> Vec<String> {
        let mut dropped: Vec<String> = Vec::new();
        self.all.retain(|id, bytes| {
            let keep = !(id.type_id == 0x00B1_B104
                && shadow_groups.contains(&id.group)
                && vanilla.contains(&(id.group, id.instance)));
            if !keep {
                dropped.push(format!(
                    "{:08X}:{:08X}:{:08X} ({}B)",
                    id.type_id,
                    id.group,
                    id.instance,
                    bytes.len()
                ));
            }
            keep
        });
        dropped
    }

    /// 身份闭包（2026-11-08，用户验收标准：包里只允许出现"这一栋楼"）：
    /// 从身份根（Menu2/Unit/Lot/locale/图标/背景图，instance = 身份）出发，
    /// 沿引用边走（property 的 Key、RW4 的材质纹理行），可达者保留，其余
    /// 全部丢弃。00B2CCCB（parent）边**不跟**——继承根由游戏本体提供
    /// （s31 实证：包内无任何模板/父链条目）。返回被丢弃清单（审计用）。
    /// 这一步产出 s31 形状的包：property 文件只有身份自己的。
    pub(crate) fn retain_identity_closure(&mut self, identity: u32) -> Vec<String> {
        const PROPERTY: u32 = 0x00B1_B104;
        const MODEL: u32 = 0x2F4E_681B;
        const RASTER: u32 = 0x2F4E_681C;
        let is_root = |id: &ResourceId| {
            (id.type_id == PROPERTY && id.instance == identity)
                || (id.type_id == 0x0A98_EAF0 && id.group == 0x02FA_BF01 && id.instance == identity)
                || (id.type_id == 0x2F7D_0004 && id.instance == identity)
                || (id.type_id == 0x3F86_62EA && id.instance == identity)
        };
        let mut reachable: std::collections::HashSet<ResourceId> = Default::default();
        let mut queue: Vec<ResourceId> =
            self.all.keys().filter(|id| is_root(id)).cloned().collect();
        while let Some(id) = queue.pop() {
            if !self.all.contains_key(&id) || !reachable.insert(id) {
                continue;
            }
            let bytes = match self.all.get(&id) {
                Some(bytes) => bytes,
                None => continue,
            };
            // property → Key 引用（parent 边除外）
            if id.type_id == PROPERTY {
                if let Ok(file) = sc_properties::PropertyFile::parse(bytes) {
                    for property_value in &file.values {
                        if property_value.hash == 0x00B2_CCCB {
                            continue;
                        }
                        let mut keys: Vec<sc_properties::Key> = Vec::new();
                        match &property_value.kind {
                            Kind::Scalar(Value::Key(key)) => keys.push(*key),
                            Kind::Array(values) => {
                                for value in values {
                                    if let Value::Key(key) = value {
                                        keys.push(*key);
                                    }
                                }
                            }
                            _ => {}
                        }
                        for key in keys {
                            if key.type_id != 0 {
                                queue.push(ResourceId {
                                    type_id: key.type_id,
                                    group: key.group,
                                    instance: key.instance,
                                });
                                continue;
                            }
                            // type-0 语义键：按候选类型解析
                            for (type_id, group) in [
                                (MODEL, 0),
                                (RASTER, 0),
                                (0x2F7D_0004, 0x40E0_2400),
                                (0x3F86_62EA, 0),
                                (0x0A98_EAF0, 0x02FA_BF01),
                            ] {
                                queue.push(ResourceId {
                                    type_id,
                                    group,
                                    instance: key.instance,
                                });
                            }
                        }
                    }
                }
            }
            // RW4 → 材质纹理行引用的 raster
            if id.type_id == MODEL {
                if let Ok(file) = Rw4File::parse(bytes) {
                    for section in file.sections_of_type(SectionType::MATERIAL) {
                        if let Ok(material) = file.decode_material(bytes, section.number) {
                            for row in material.texture_refs() {
                                // slot0 = 参数表 RW4（2F4E681B），slot1..5 = raster
                                // （2F4E681C）。两种候选都入队，存在的才保留——
                                // 漏参数表 = 白模（2026-11-08 预览器实测）。
                                queue.push(ResourceId {
                                    type_id: MODEL,
                                    group: 0,
                                    instance: row.texture_instance,
                                });
                                queue.push(ResourceId {
                                    type_id: RASTER,
                                    group: 0,
                                    instance: row.texture_instance,
                                });
                            }
                        }
                    }
                }
            }
        }
        let mut dropped: Vec<String> = Vec::new();
        self.all.retain(|id, bytes| {
            let keep = reachable.contains(id);
            if !keep {
                dropped.push(format!(
                    "{:08X}:{:08X}:{:08X} ({}B)",
                    id.type_id,
                    id.group,
                    id.instance,
                    bytes.len()
                ));
            }
            keep
        });
        dropped
    }

    pub(crate) fn from_source(resources: connector::Resources) -> Self {
        Self {
            all: resources,
            touched: Default::default(),
        }
    }

    /// Record a created/rewritten resource. Inherent method wins over the
    /// `HashMap::insert` reachable through `Deref`, so every write site that
    /// goes through this is tracked for the delta package.
    pub(crate) fn insert(&mut self, id: ResourceId, bytes: Vec<u8>) {
        self.touched.insert(id);
        self.all.insert(id, bytes);
    }

    /// The full reference snapshot, used to resolve dependencies that were
    /// left in a cloned Property/RW4 but were not rewritten themselves.
    pub(crate) fn all(&self) -> &connector::Resources {
        &self.all
    }

    pub(crate) fn is_touched(&self, id: &ResourceId) -> bool {
        self.touched.contains(id)
    }

    /// Full-snapshot swap for the legacy full-output mode (stages.rs).
    pub(crate) fn take_all(&mut self) -> connector::Resources {
        std::mem::take(&mut self.all)
    }

    /// The delta slice: only entries this build created or rewrote.
    pub(crate) fn delta(&self) -> connector::Resources {
        self.touched
            .iter()
            .map(|id| (*id, self.all[id].clone()))
            .collect()
    }
}

/// 薄编排器(P1 责任链,2026-11-04):格式校验/降级(plan_from_schema)之后,
/// 各 schema section 交由 stages.rs 的责任链处理器按序消费;本函数只做
/// 调度与结果回收。缺节由阶段函数自身短路(无 slots/无模型/无 patches 均
/// 合法)。行为不变量 = golden_full_chain_asset_dict_columns +
/// deterministic_output_two_fresh_builds_identical(拆分前基线 331ab75)。
fn build_from_schema_inner(
    root: &Path,
    raw_schema: &str,
    progress: Option<std::sync::mpsc::Sender<super::events::StageEvent>>,
) -> Result<M1BuildResult, String> {
    let plan = plan_from_schema(raw_schema)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("project root must be a directory".into());
    }
    // G3a 全链复用（用户指令 2026-11-05）："已确认的无变更流程，跑通
    // 构建器自检就能保证结果正确"。输入指纹命中 + 产物包 sha256 自检
    // 通过 → 直接回放上次结果；任何不一致都走全量链重建缓存。
    let fingerprint =
        super::build_cache::input_fingerprint(&root, &plan, &plan.intermediate.scene.document);
    if let Some(entry) = super::build_cache::lookup_run(&root, &fingerprint) {
        let reuse_detail = serde_json::json!({
            "buildHash": entry.build_hash,
            "sha256": entry.sha256,
            "resourceCount": entry.resource_count,
        });
        if let Some(event) = progress.as_ref() {
            let _ = event.send(super::events::finished(
                "cache-reuse",
                reuse_detail.clone(),
                0,
            ));
        }
        return Ok(M1BuildResult {
            path: root
                .join(".build")
                .join(&entry.build_hash)
                .join(&entry.package_relative),
            schema_hash: entry.schema_hash,
            build_hash: entry.build_hash.clone(),
            source_hash: entry.source_hash,
            sha256: entry.sha256,
            resource_count: entry.resource_count,
            delta_mode: entry.delta_mode,
            menu_instance: entry.menu_instance,
            patched_resources: entry.patched_resources,
            scope: "property+scene-model",
            scene: serde_json::from_value(entry.scene)
                .map_err(|e| format!("run cache scene: {e}"))?,
            events: vec![super::events::finished("cache-reuse", reuse_detail, 0)],
            started_at_ms: entry.submitted_at_ms,
            finished_at_ms: entry.submitted_at_ms,
            duration_ms: 0,
        });
    }
    let package_relative = package_relative_name(&plan);
    let mut ctx = stages::BuildContext::new(root, plan);
    ctx.progress = progress;
    stages::run_chain(&mut ctx)?;
    // 链尾补 materialize 的 finished 事件后回填,保证 Actions 页拿到全量
    // 步骤;失败路径的事件随 Err 文本携带阶段名(P2 队列改结构化错误)。
    let mut result = ctx.result.take().expect("chain completed without result");
    result.events = ctx.events.clone();
    super::build_cache::store_run(
        &ctx.root,
        &fingerprint,
        &super::build_cache::RunCacheEntry {
            build_hash: result.build_hash.clone(),
            schema_hash: result.schema_hash.clone(),
            source_hash: result.source_hash.clone(),
            package_relative,
            sha256: result.sha256.clone(),
            resource_count: result.resource_count,
            delta_mode: result.delta_mode,
            menu_instance: result.menu_instance,
            scene: serde_json::to_value(&result.scene).unwrap_or(serde_json::Value::Null),
            patched_resources: result.patched_resources,
            submitted_at_ms: super::events::now_ms(),
        },
    );
    Ok(result)
}

/// 带进度通道的构建(P2 构建队列):每个阶段边界经通道流出 StageEvent。
pub fn build_from_schema_with_progress(
    root: &Path,
    raw_schema: &str,
    progress: Option<std::sync::mpsc::Sender<super::events::StageEvent>>,
) -> Result<M1BuildResult, String> {
    build_from_schema_inner(root, raw_schema, progress)
}

pub(crate) fn package_relative_name(plan: &super::plan::PropertyBuildPlan) -> String {
    format!("packages/{}", plan.intermediate.output.package)
}

pub(crate) fn read_scene_document(
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

pub(crate) fn read_scene_bytes(root: &Path, document: &str) -> Result<Option<Vec<u8>>, String> {
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
pub(crate) fn insert_slot_raster(
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
pub(crate) fn insert_scene_model(
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
        let route = super::geometry_route::derive(model_shape, &doc)?;
        let effective_shape = route.name;
        if effective_shape == "building40" {
            let author_model = super::author_building40::compile_model_with_transform(
                &doc,
                rotation_degrees,
                transform,
                include_bytes!("../../resources/sc5/175EFB4E.research-template.rw4"),
                include_bytes!("../../resources/sc5/collision-source.rw4"),
            )?;
            let author_materials = super::author_materials::compile_materials(&doc, true)?;
            let mut texture_instances = [0u32; 6];
            for (slot, payload) in author_materials.slots.iter().enumerate() {
                let type_id = if matches!(slot, 0 | 4 | 5) {
                    materials::rw4_model_type()
                } else {
                    materials::raster_type()
                };
                let instance = alloc_free_instance(resources, type_id, payload);
                let id = ResourceId {
                    type_id,
                    group: 0,
                    instance,
                };
                resources.insert(id, payload.clone());
                texture_instances[slot] = instance;
            }
            let rw4 = super::template_material::rewrite_texture_instances(
                &author_model.compiled.rw4,
                &texture_instances,
            )?;
            let model_ids = insert_compiled_model(resources, &rw4);
            materials::validate_model_resource_closure(&rw4, resources)
                .map_err(|error| format!("author Building40 resource closure: {error}"))?;
            return Ok((Some(model_ids), 6));
        }
        let mut material_plan = materials::resolve_materials(&doc)?;
        let mut atlas_cells: Option<Vec<[f32; 4]>> = None;
        if effective_shape == "nh-atlas" {
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
        let use_nh_partitioned = effective_shape == "nh-partitioned";
        // Select a real game RW4 from the reference package as the container
        // skeleton. Geometry is replaced later; its header/material/collision
        // sections remain byte-compatible with the game.
        let native_template = std::env::var("OPENSCP_NATIVE_TEMPLATE")
            .ok()
            .map(std::fs::read)
            .transpose()
            .map_err(|error| format!("read native template: {error}"))?
            .or_else(|| {
                Some(include_bytes!("../../resources/sc5/175EFB4E.research-template.rw4").to_vec())
            })
            .or_else(|| {
                resources
                    .iter()
                    .filter(|(id, bytes)| {
                        id.type_id == materials::rw4_model_type()
                            && rw4::Rw4File::parse(bytes)
                                .map(|file| {
                                    file.sections_of_type(rw4::SectionType::MESH)
                                        .next()
                                        .is_some()
                                })
                                .unwrap_or(false)
                    })
                    .min_by_key(|(id, _)| (id.group, id.instance))
                    .map(|(_, bytes)| bytes.clone())
            });
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
        } else if effective_shape == "native-multi" {
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
        } else if effective_shape == "nh-atlas" {
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
        } else if effective_shape == "nh-multimaterial" {
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
        } else if effective_shape == "building40" {
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
            let rw4 =
                super::template_material::rewrite_texture_instances(&compiled.rw4, &head_slots)?;
            native_model::CompiledNativeModel { rw4, ..compiled }
        } else if use_nh_partitioned {
            // NH 椤剁偣甯冨眬锛坰tride 40 鍘嬬缉娉曠嚎/鍒囩嚎 + facade F4 鍗?UV 娴侊級
            // 鐨勫崟褰㈢姸锛氬叏閮ㄥ嚑浣曞悎鍏ヤ竴涓?mesh锛屾潗璐ㄥ彇澶撮儴鏉愯川鐨勫叚妲斤紱缁?            // 椤剁偣 >65536 鏃?u32 绱㈠紩锛圱A 鏍囧織 102锛夈€?026-10-31 R9 涓婃父鎴?            // 楠岃瘉锛氳褰㈡€佸妤煎畬鏁存覆鏌撱€傞€?GLB mesh 涓€ RW4 mesh 鐨勫舰鐘?            // 锛?8-105 mesh锛夋槸 census 瀹炶瘉浼氫护鍔犺浇瀛樻。宕╂簝鐨勫鍎垮舰鐘躲€?
            let (dominant, _) = select_material_groups(&doc, &material_plan, 1)?;
            let head = dominant
                .first()
                .copied()
                .ok_or("nh-partitioned compile: material plan empty")?
                as u32;
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
        let max_meshes = if effective_shape == "native-multi" {
            128
        } else {
            4
        };
        if mesh_count > max_meshes {
            return Err(format!(
                "compiled LOTM model has {mesh_count} RW4 meshes; maximum supported is {max_meshes}"
            ));
        }
        let model_ids = if effective_shape == "native-multi" {
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
        let material = plan.mesh_material.get(mesh_index).copied().unwrap_or(0) as usize;
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
    order.sort_by(|a, b| {
        area[*b]
            .partial_cmp(&area[*a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
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

pub(crate) fn scene_model_transform(document: Option<&SceneDocument>) -> Option<[f32; 12]> {
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
pub(crate) fn apply_property_semantics(
    resources: &mut BuildResources,
    target: ResourceId,
    document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
    writes: &scene::SceneWrites,
    lowered_units: &mut usize,
) -> Result<(), String> {
    let Some(bytes) = resources.get(&target).cloned() else {
        return Ok(());
    };
    if let Some((encoded, units)) = apply_scene_lot_semantics(&bytes, document, model_ids, writes)?
    {
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
    writes: &scene::SceneWrites,
) -> Result<Option<(Vec<u8>, usize)>, String> {
    let mut file = PropertyFile::parse(bytes).map_err(|e| e.to_string())?;
    let mut touched = false;
    let mut lowered_units = 0usize;
    if let Some(document) = document {
        // delta 模式即使文档没有单元（用户删光了某族），只要声明了列族
        // 也必须执行写回——空的声明族 = "清空这些列"。
        if document.has_semantics() || !writes.is_empty() {
            scene::apply_units(&mut file, document, writes)?;
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
        for (hash, value) in scene::ground_values(document) {
            set_scalar(&mut file, hash, value);
            touched = true;
        }
    }
    if let Some(model_ids) = model_ids {
        touched |= retarget_lod_models(&mut file, model_ids);
        // A compiled interior material is inert without the per-building
        // interior controller.  native_profile already writes this field;
        // keep the cloned-Lot/menu route on the same s31/author contract.
        set_scalar(&mut file, 0x0F16_9C46, Value::Bool(true));
        touched = true;
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
            scene::verify_units(&readback, document, writes)?;
        }
    }
    if model_ids.is_some()
        && !matches!(
            readback.get(0x0F16_9C46).and_then(|property| property.scalar()),
            Some(Value::Bool(true))
        )
    {
        return Err(
            "night interior gate: compiled model requires 0F169C46=true after property readback"
                .into(),
        );
    }
    Ok(Some((encoded, lowered_units)))
}

/// Point LOD1..LOD4 at four distinct compiled model resources, matching the
/// native NightHouse asset graph.  Missing LOD keys are created.
fn retarget_lod_models(file: &mut PropertyFile, model_ids: LodModelIds) -> bool {
    for (index, hash) in LOD_MODEL_HASHES.into_iter().enumerate() {
        let model_id = model_ids[index];
        match file
            .values
            .iter_mut()
            .find(|property| property.hash == hash)
        {
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
    let prop_type = scalar_prop_type(&value);
    match file.values.iter_mut().find(|p| p.hash == hash) {
        Some(property) => {
            property.prop_type = prop_type;
            property.kind = Kind::Scalar(value);
        }
        None => file.values.push(make_scalar_property(hash, value)),
    }
}

fn scalar_prop_type(value: &Value) -> sc_properties::PropType {
    match value {
        Value::Bool(_) => sc_properties::PropType::Bool,
        Value::Int32(_) => sc_properties::PropType::Int32,
        Value::UInt32(_) => sc_properties::PropType::UInt32,
        Value::Float(_) => sc_properties::PropType::Float,
        Value::String8(_) => sc_properties::PropType::String8,
        Value::String16(_) => sc_properties::PropType::String16,
        Value::Key(_) => sc_properties::PropType::Key,
        Value::Text(_) => sc_properties::PropType::Text,
        Value::Vector2(_) => sc_properties::PropType::Vector2,
        Value::Vector3(_) => sc_properties::PropType::Vector3,
        Value::ColorRgb { .. } => sc_properties::PropType::ColorRgb,
        Value::Vector4(_) => sc_properties::PropType::Vector4,
        Value::ColorRgba { .. } => sc_properties::PropType::ColorRgba,
        Value::Transform(_) => sc_properties::PropType::Transform,
        Value::BoundingBox { .. } => sc_properties::PropType::BoundingBox,
    }
}

fn make_scalar_property(hash: u32, value: Value) -> sc_properties::Property {
    sc_properties::Property {
        hash,
        prop_type: scalar_prop_type(&value),
        kind: Kind::Scalar(value),
        encoding: sc_properties::PropertyEncoding::default(),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct SceneResourceManifest {
    pub(crate) lot: Vec<String>,
    pub(crate) prop: Vec<String>,
    pub(crate) bin: Vec<String>,
    pub(crate) path: Vec<String>,
    pub(crate) path_entry: Vec<String>,
    pub(crate) raster: Vec<String>,
    pub(crate) missing: Vec<String>,
}

pub(crate) fn materialize_scene_resources(
    root: &Path,
    intermediate: &crate::asset_build_m0::IntermediateModel,
    resources: &connector::Resources,
    staging: &mut Staging,
) -> Result<SceneResourceManifest, String> {
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
                            group: super::native_profile::path_entry_group(entry),
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
    Ok(manifest)
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

pub(crate) fn validate_scene_path_semantics(
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
        // 类型驱动（2026-11-07）：作者只选 type（pedestrian/driveway）；
        // 高级模式才用显式 entry。解析失败 = 构建失败（引擎会整条跳过）。
        let kind = path
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|kind| !kind.is_empty());
        let entry_text = path
            .get("entry")
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty());
        let scope_local = path
            .get("scope")
            .and_then(serde_json::Value::as_str)
            .map(|scope| scope != "global")
            .unwrap_or(true);
        let road_connect = path
            .get("roadConnect")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let recipe = match super::path_catalog::resolve(kind, entry_text, scope_local, road_connect)
        {
            Ok(recipe) => recipe,
            Err(error) => {
                errors.push(format!("{location}: {error}"));
                continue;
            }
        };
        path_entry_count += 1;
        // PathEntry 资源必须在构建输入池中（P009：缺失 = 引擎静默跳过该路径）。
        // 例外（2026-11-07）：目录类型是引擎内部语义 id——原版大学 lot
        // 41E1C000:D7A06E31 引用 418A18B1 时，任何游戏包都不存在该资源
        // （find_instance 全包扫描实证），引擎按 instance 解析、group 仅名义。
        // 自建/未知 entry 仍要求池内资源。
        let resource_exists = recipe.from_catalog
            || resources.keys().any(|id| {
                id.type_id == 0x00B1_B104
                    && id.instance == recipe.entry_instance
                    && (id.group & 0xFFFF) == 0xC400
            });
        if !resource_exists {
            let origin = if recipe.from_catalog {
                // 目录类型缺资源时给出可行动的替代：列出参照包里实际存在的
                // 目录 entry（作者不必猜哪个类型能用于当前 donor）。
                let available: Vec<&str> = super::path_catalog::PATH_TYPES
                    .iter()
                    .filter(|spec| {
                        resources.keys().any(|id| {
                            id.type_id == 0x00B1_B104
                                && id.instance == spec.entry
                                && (id.group & 0xFFFF) == 0xC400
                        })
                    })
                    .map(|spec| spec.id)
                    .collect();
                format!(
                    "path type '{}'; path entries present in the pool: {}",
                    recipe.type_id,
                    if available.is_empty() {
                        "none".to_string()
                    } else {
                        available.join(", ")
                    }
                )
            } else {
                "explicit entry".to_string()
            };
            errors.push(format!(
                "{location}: PathEntry resource instance 0x{:08X} ({origin}) is missing \
                 from the build input pool",
                recipe.entry_instance
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

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn protected_templates(
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

/// 无进度包装(命令行/黄金用例入口,行为与带进度版完全一致)。
pub fn build_from_schema(root: &Path, raw_schema: &str) -> Result<M1BuildResult, String> {
    build_from_schema_inner(root, raw_schema, None)
}

#[cfg(test)]
mod night_interior_tests {
    use super::*;

    #[test]
    fn compiled_model_enables_interior_controller_only_on_model_writeback() {
        let source = PropertyFile::default().encode_canonical().unwrap();
        let unchanged = apply_scene_lot_semantics(
            &source,
            None,
            None,
            &scene::SceneWrites::default(),
        )
        .unwrap();
        assert!(unchanged.is_none(), "donor-only s31 semantics stay byte-identical");

        let model_ids = std::array::from_fn(|index| ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 0xE2E8_2E9D + index as u32,
        });
        let (encoded, _) = apply_scene_lot_semantics(
            &source,
            None,
            Some(model_ids),
            &scene::SceneWrites::default(),
        )
        .unwrap()
        .expect("compiled model writes the asset property");
        let readback = PropertyFile::parse(&encoded).unwrap();
        assert!(matches!(
            readback.get(0x0F16_9C46).and_then(|property| property.scalar()),
            Some(Value::Bool(true))
        ));
    }
}
