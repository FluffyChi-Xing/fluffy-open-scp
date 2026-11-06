//! P1 责任链(2026-11-04):构建器 = 调度器,本模块 = 按 schema section
//! 切分的阶段处理器。每阶段一个函数,入参统一 `&mut BuildContext`,
//! 返回 StageEvent 向量;顺序执行,前序完成交由后续,缺节(如无
//! semantics.properties)由函数体自身短路。除 materialize_and_link
//! (staging/链接/写盘)外全部无副作用——符合用户架构指令:分层解耦、
//! 函数式、惰性求值、副作用收口在最末端。
//!
//! 行为不变量:拆分前后产物 byte 一致(golden_full_chain_asset_dict_columns
//! + deterministic_output_two_fresh_builds_identical 守卫)。

use super::connector;
use dbpf::ResourceId;
use super::events::{finished, started, StageEvent};
use super::filesystem::{project_file, Staging};
use super::package::{
    apply_property_semantics, digest, insert_scene_model, insert_slot_raster, protected_templates,
    read_scene_bytes, read_scene_document, validate_scene_path_semantics, BuildResources,
    M1BuildResult, SceneSemanticSummary,
};
use super::plan::PropertyBuildPlan;
use super::processor::defer_property;
use super::scene::SceneDocument;
use super::package::LodModelIds;
use std::path::PathBuf;
use std::time::Instant;

/// 责任链共享上下文:一次性构建,阶段按序消费/填充。
pub(crate) struct BuildContext {
    /// 可选进度通道(P2 构建队列):run_chain 在每个阶段边界把事件克隆
    /// 发出,Actions 页实时行消费;None 时零开销。
    pub progress: Option<std::sync::mpsc::Sender<super::events::StageEvent>>,
    pub root: PathBuf,
    pub plan: PropertyBuildPlan,
    pub resources: BuildResources,
    pub identity_resources: Option<connector::Resources>,
    pub scene_document: Option<SceneDocument>,
    pub scene_document_bytes_hash: Option<String>,
    pub texture_instances: [u32; 6],
    pub model_ids: Option<LodModelIds>,
    pub material_resource_count: usize,
    pub scene: SceneSemanticSummary,
    pub source_hash: String,
    pub build_hash: String,
    pub plan_bytes: Vec<u8>,
    pub lowered_units: usize,
    pub menu_instance: Option<u32>,
    pub targets: Vec<ResourceId>,
    pub delta_mode: bool,
    pub written_count: usize,
    pub events: Vec<StageEvent>,
    pub result: Option<M1BuildResult>,
    pub started_at_ms: u64,
    /// schema 声明 `semantics.materialChange: true`（G1 材质指纹闸门）：
    /// 显式声明材质输入有意变更，漂移放行并刷新基线。
    pub material_change_declared: bool,
    /// P3 属性验收闸门声明位（`semantics.lotChange`）：已验证 Asset/Unit
    /// 列有意变更时放行并刷新属性基线。
    pub lot_change_declared: bool,
    /// P3 池基线漂移：load-inputs 时的 reference 包库存（逐 TGI+字节），
    /// 成功构建后写 `.build/pool-last.json` 供下次对拍。
    pub pool_inventory: serde_json::Value,
    /// 本次构建相对上次成功构建的池漂移（added/removed/changed）。
    pub pool_drift: serde_json::Value,
}

impl BuildContext {
    pub fn new(root: PathBuf, plan: PropertyBuildPlan) -> Self {
        let material_change_declared = plan.intermediate.semantics.material_change;
        let lot_change_declared = plan.intermediate.semantics.lot_change;
        Self {
            root,
            plan,
            resources: BuildResources::empty(),
            identity_resources: None,
            scene_document: None,
            scene_document_bytes_hash: None,
            texture_instances: [0u32; 6],
            model_ids: None,
            material_resource_count: 0,
            scene: SceneSemanticSummary::default(),
            source_hash: String::new(),
            build_hash: String::new(),
            plan_bytes: Vec::new(),
            lowered_units: 0,
            menu_instance: None,
            targets: Vec::new(),
            delta_mode: true,
            written_count: 0,
            events: Vec::new(),
            progress: None,
            result: None,
            started_at_ms: super::events::now_ms(),
            material_change_declared,
            lot_change_declared,
            pool_inventory: serde_json::Value::Null,
            pool_drift: serde_json::Value::Null,
        }
    }
}

/// 边界广播:本地 events 追加 + 可选通道转发(队列实时行)。
fn emit(ctx: &mut BuildContext, event: StageEvent) {
    if let Some(tx) = &ctx.progress {
        let _ = tx.send(event.clone());
    }
    ctx.events.push(event);
}

pub(crate) const CHAIN: [&str; 9] = [
    "load-inputs",
    "insert-textures",
    "insert-model",
    "validate-paths",
    "compute-identity",
    "property-patches",
    "scene-semantics",
    "behavior-bind",
    "materialize-and-link",
];

/// 按序执行责任链;任一阶段失败即终止(错误文本携带阶段名)。
pub(crate) fn run_chain(ctx: &mut BuildContext) -> Result<(), String> {
    for stage in CHAIN {
        let started_at = Instant::now();
        emit(ctx, started(stage, serde_json::Value::Null));
        let outcome = match stage {
            "load-inputs" => load_inputs(ctx),
            "insert-textures" => insert_textures(ctx),
            "insert-model" => insert_model(ctx),
            "validate-paths" => validate_paths(ctx),
            "compute-identity" => compute_identity(ctx),
            "property-patches" => apply_property_patches(ctx),
            "scene-semantics" => apply_scene_semantics(ctx),
            "behavior-bind" => super::behavior::apply(ctx),
            "materialize-and-link" => materialize_and_link(ctx),
            _ => unreachable!("unknown chain stage {stage}"),
        };
        match outcome {
            Ok(detail) => {
                // G2 不变量：阶段"成功"后校验产出契约；违例等同阶段失败，
                // 错误固定格式 `[<step>] invariant <CODE> violated: <事实>`
                // —— 责任处理器名自带，不再整链复盘。
                if let Err(violation) = check_invariants(stage, ctx) {
                    emit(
                        ctx,
                        super::events::failed(stage, &violation, started_at.elapsed().as_millis() as u64),
                    );
                    return Err(format!("[{stage}] {violation}"));
                }
                emit(ctx, finished(stage, detail, started_at.elapsed().as_millis() as u64));
            }
            Err(error) => {
                emit(ctx, super::events::failed(stage, &error, started_at.elapsed().as_millis() as u64));
                return Err(format!("[{stage}] {error}"));
            }
        }
    }
    Ok(())
}

// ---- G2 Step 契约（设计文档 4.1，用户指令 2026-11-05） ----
//
// 处理器声明读写面与编译期不变量；reads/writes 是审计面（Actions 页/台账
// 可展示），invariants 由编排器强制执行。
//
// 保护区（"材质合成 / RW4 模型转换是基本能力，任何回归不允许"）：
// `texture_instances` / `model_ids` / `material_resource_count` 以及池内
// 2F4E681B / 2F4E681C 资源只允许 insert-textures / insert-model 写入；
// 其它处理器触碰即视为破坏风险隔离的 bug，评审直接拒绝。

/// 违例事实（Some = 违反）。
pub(crate) type InvariantCheck = fn(&BuildContext) -> Option<String>;

pub(crate) struct Invariant {
    pub code: &'static str,
    pub check: InvariantCheck,
}

pub(crate) struct StepContract {
    pub name: &'static str,
    pub reads: &'static [&'static str],
    pub writes: &'static [&'static str],
    pub invariants: &'static [Invariant],
}

/// s10 形态的就地拦截：模型编译产出存在 ⇒ 材质资源必须非空。
/// 0 材质静默通过就是立面贴图全丢的起点。
fn material_non_empty_when_model(ctx: &BuildContext) -> Option<String> {
    (ctx.model_ids.is_some() && ctx.material_resource_count == 0).then(|| {
        format!(
            "model compiled with {} material resources (model_ids present)",
            ctx.material_resource_count
        )
    })
}

/// 声明了纹理槽 ⇒ 六槽必须全部落位（槽计数漂移=贴图丢失前置信号）。
fn slots_declared_all_inserted(ctx: &BuildContext) -> Option<String> {
    let declared = ctx.plan.intermediate.scene.slots.len();
    let inserted = ctx.texture_instances.iter().filter(|i| **i != 0).count();
    (declared > 0 && inserted != declared)
        .then(|| format!("{declared} texture slots declared, {inserted} inserted"))
}

pub(crate) const INSERT_TEXTURES_CONTRACT: StepContract = StepContract {
    name: "insert-textures",
    reads: &["plan.scene.slots"],
    writes: &["texture_instances", "resources(raster)"],
    invariants: &[Invariant {
        code: "SLOTS_DECLARED_ALL_INSERTED",
        check: slots_declared_all_inserted,
    }],
};

pub(crate) const INSERT_MODEL_CONTRACT: StepContract = StepContract {
    name: "insert-model",
    reads: &["plan.scene.model", "plan.scene.modelShape", "texture_instances", "sceneDocument"],
    writes: &["model_ids", "material_resource_count", "resources(RW4/raster)", "material-baseline"],
    invariants: &[Invariant {
        code: "MATERIAL_NON_EMPTY_WHEN_MODEL",
        check: material_non_empty_when_model,
    }],
};

/// behavior-bind 契约：只有 Unit/Lot/locale(title 行)允许被写；菜单冻结与
/// 教育链不变量在 super::behavior::apply 内 fail-closed，这里声明审计面。
pub(crate) const BEHAVIOR_BIND_CONTRACT: StepContract = StepContract {
    name: "behavior-bind",
    reads: &["plan.behavior", "resources(Unit/Lot/Menu2/locale)"],
    writes: &["resources(Unit/Lot/locale title)"],
    invariants: &[],
};

fn check_invariants(stage: &str, ctx: &BuildContext) -> Result<(), String> {
    let contract = match stage {
        "insert-textures" => Some(&INSERT_TEXTURES_CONTRACT),
        "insert-model" => Some(&INSERT_MODEL_CONTRACT),
        "behavior-bind" => Some(&BEHAVIOR_BIND_CONTRACT),
        _ => None,
    };
    let Some(contract) = contract else {
        return Ok(());
    };
    for invariant in contract.invariants {
        if let Some(fact) = (invariant.check)(ctx) {
            return Err(format!(
                "invariant {} violated: {fact}",
                invariant.code
            ));
        }
    }
    Ok(())
}

/// 参考包/身份包/场景文档装载。
fn load_inputs(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let source_path = project_file(
        &ctx.root,
        ctx.plan
            .intermediate
            .references
            .package
            .as_deref()
            .ok_or("missing source package")?,
    )?;
    ctx.resources = BuildResources::from_source(connector::read_source(&source_path)?);
    ctx.identity_resources = match ctx
        .plan
        .menu
        .as_ref()
        .and_then(|menu| menu.identity_package.as_deref())
    {
        Some(path) => Some(connector::read_source(&project_file(&ctx.root, path)?)?),
        None => None,
    };
    ctx.scene_document = read_scene_document(
        &ctx.root,
        &ctx.plan.intermediate.scene.document,
        ctx.plan.intermediate.scene.model.as_deref(),
    )?;
    // The scene document is an input to the built bytes (unit columns, menu
    // ground numerics, light/prop matrices) but the schema only references it
    // by filename. Fold its bytes into the build hash so a scene-only edit
    // gets a fresh destination instead of colliding with the previous tree
    // (publish() then refuses to overwrite and the stale package silently
    // "wins" — University s4 shipped 5 of 8 lights this way on 2026-11-03).
    // Model/sidecar/texture bytes are already covered: they are inserted into
    // `resources` above hashing and captured by the source fingerprint.
    ctx.scene_document_bytes_hash =
        read_scene_bytes(&ctx.root, &ctx.plan.intermediate.scene.document)?
            .map(|bytes| digest(&bytes));
    // P3 池基线漂移（死命令：已验证输入不允许无声变化）：reference 包
    // 库存全量入台账，并与上次成功构建的池库存对拍；added/removed/changed
    // 直接进 load-inputs detail——reference 被外部工具改写（如手工并条目、
    // 坏覆盖）当场可见，不再事后排查。
    let inventory: Vec<serde_json::Value> = {
        let mut rows: Vec<((u32, u32, u32), usize)> = ctx
            .resources
            .all()
            .iter()
            .map(|(id, bytes)| ((id.type_id, id.group, id.instance), bytes.len()))
            .collect();
        rows.sort();
        rows.iter()
            .map(|((t, g, i), size)| {
                serde_json::json!({"tgi": format!("{t:08X}:{g:08X}:{i:08X}"), "bytes": size})
            })
            .collect()
    };
    let baseline_path = ctx.root.join(".build").join("pool-last.json");
    let previous: Option<std::collections::BTreeMap<String, usize>> = std::fs::read(&baseline_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok());
    let current: std::collections::BTreeMap<String, usize> = inventory
        .iter()
        .filter_map(|row| {
            let tgi = row.get("tgi")?.as_str()?.to_string();
            Some((tgi, row.get("bytes")?.as_u64()? as usize))
        })
        .collect();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    if let Some(previous) = &previous {
        for (tgi, size) in &current {
            match previous.get(tgi) {
                None => added.push(tgi.clone()),
                Some(prev) if prev != size => changed.push(format!("{tgi} {prev}B -> {size}B")),
                Some(_) => {}
            }
        }
        for tgi in previous.keys() {
            if !current.contains_key(tgi) {
                removed.push(tgi.clone());
            }
        }
    }
    let pool_drift = serde_json::json!({"added": added, "removed": removed, "changed": changed});
    ctx.pool_inventory = serde_json::Value::Object(
        current
            .iter()
            .map(|(tgi, size)| (tgi.clone(), serde_json::json!(size)))
            .collect(),
    );
    ctx.pool_drift = pool_drift.clone();
    Ok(serde_json::json!({
        "identityPackage": ctx.identity_resources.is_some(),
        "sceneDocument": ctx.scene_document.is_some(),
        "pool": {"count": current.len(), "bytes": current.values().sum::<usize>(), "drift": pool_drift},
    }))
}

/// 六槽贴图 → 原生 raster 插入(缺槽自动跳过)。
fn insert_textures(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let mut inserted = 0usize;
    for (slot, path) in &ctx.plan.intermediate.scene.slots {
        let index = slot
            .strip_prefix("slot")
            .and_then(|v| v.parse::<usize>().ok())
            .ok_or_else(|| format!("invalid raster slot {slot}"))?;
        if index >= ctx.texture_instances.len() {
            return Err(format!("raster slot out of range: {slot}"));
        }
        let root = ctx.root.clone();
        let instance = insert_slot_raster(&root, &mut ctx.resources, path)?;
        ctx.texture_instances[index] = instance;
        inserted += 1;
    }
    Ok(serde_json::json!({ "slots": inserted }))
}

/// 场景模型(LOTM/GLB)→ RW4 编译 + LOD id 分配(缺模型跳过)。
fn insert_model(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let rotation = ctx
        .plan
        .menu
        .as_ref()
        .and_then(|menu| menu.rotation_degrees)
        // Author orientation (backend settings.rotationDegrees, default 180);
        // parent-free builds carry it on the menu spec so geometry, ground
        // and prop/light matrices all share one value.
        .unwrap_or(180);
    // Capture the source contract in the stage event before lowering.  This
    // makes a material regression attributable to this processor from the
    // build report, without comparing entire DBPF files by hand.
    let model_bytes = ctx
        .plan
        .intermediate
        .scene
        .model
        .as_deref()
        .and_then(|path| project_file(&ctx.root, path).ok())
        .and_then(|path| std::fs::read(path).ok());
    let material_contract = model_bytes
        .as_deref()
        .and_then(|bytes| super::lotm::is_container(bytes).then(|| super::lotm::parse(bytes).ok()).flatten())
        .map(|doc| serde_json::json!({
            "materialGroups": doc.materials.len(),
            "parameterColumns": doc.materials.iter().map(|m| m.param_cols).collect::<Vec<_>>(),
            "meshCount": doc.meshes.len(),
        }))
        .unwrap_or(serde_json::Value::Null);
    // G1 材质指纹闸门：源模型 hash + 材质组/参数列/网格 + 六槽实例与载荷
    // + 材质资源数。与 .build/material-baseline.json 对拍；未声明的漂移在
    // 本处理器即失败（s10 贴图回归的构建期拦截点）。
    let gate = match (&model_bytes, ctx.plan.intermediate.scene.model.as_deref()) {
        (Some(bytes), Some(model_key)) => {
            let contract = super::lotm::parse(bytes).ok();
            let fingerprint = super::material_gate::MaterialFingerprint {
                model_sha256: digest(bytes),
                model_shape: ctx.plan.intermediate.scene.model_shape.clone(),
                material_groups: contract.as_ref().map_or(0, |doc| doc.materials.len()),
                parameter_columns: contract
                    .as_ref()
                    .map(|doc| doc.materials.iter().map(|m| m.param_cols).collect())
                    .unwrap_or_default(),
                mesh_count: contract.as_ref().map_or(0, |doc| doc.meshes.len()),
                texture_slot_instances: ctx.texture_instances,
                slot_payload_sha256: slot_payload_digest(&ctx.root, &ctx.plan.intermediate.scene.slots),
                material_resources: 0, // 在 insert_scene_model 之后回填
            };
            Some((model_key.to_string(), fingerprint))
        }
        _ => None,
    };
    let (model_ids, material_resource_count, cache_state) = insert_model_with_cache(
        ctx,
        rotation,
        model_bytes.as_deref(),
        gate
            .as_ref()
            .map(|(key, fingerprint)| (key.clone(), fingerprint.model_sha256.clone(), fingerprint.slot_payload_sha256.clone())),
    )?;
    ctx.model_ids = model_ids;
    ctx.material_resource_count = material_resource_count;
    let mut gate_json = serde_json::Value::Null;
    if let Some((model_key, mut fingerprint)) = gate {
        fingerprint.material_resources = material_resource_count;
        let outcome = super::material_gate::enforce(
            &ctx.root,
            &model_key,
            &fingerprint,
            ctx.material_change_declared,
        )?;
        gate_json = serde_json::json!({
            "verdict": outcome.label(),
            "model": model_key,
            "fingerprint": fingerprint,
        });
    }
    Ok(serde_json::json!({
        "materialResources": material_resource_count,
        "materialContract": material_contract,
        "materialGate": gate_json,
        "materialCache": cache_state,
    }))
}

/// G3b 材质缓存：指纹命中时跳过模型编译/图集编码，直接恢复材质与 RW4
/// 资源入池（alloc_free_instance 内容寻址 ⇒ 实例号与重编译一致）。
/// 返回 (model_ids, material_resource_count, "hit"|"miss"|"off")。
fn insert_model_with_cache(
    ctx: &mut BuildContext,
    rotation: u32,
    model_bytes: Option<&[u8]>,
    gate_key: Option<(String, String, String)>,
) -> Result<(Option<super::package::LodModelIds>, usize, &'static str), String> {
    let transform = super::package::scene_model_transform(ctx.scene_document.as_ref());
    let transform_repr = format!("{transform:?}");
    let fingerprint_key = gate_key.as_ref().map(|(key, model_sha, slot_sha)| {
        super::build_cache::material_fingerprint(
            &ctx.root,
            &ctx.plan,
            model_sha,
            slot_sha,
            rotation,
            &transform_repr,
        )
    });
    if let (Some(key), Some(_)) = (&fingerprint_key, model_bytes) {
        if let Some(entry) = super::build_cache::lookup_material(&ctx.root, key) {
            for (type_id, instance, file) in &entry.entries {
                let payload = super::build_cache::read_material_payload(&ctx.root, key, file)
                    .ok_or_else(|| format!("material cache payload missing: {file}"))?;
                ctx.resources.insert(
                    dbpf::ResourceId {
                        type_id: *type_id,
                        group: 0,
                        instance: *instance,
                    },
                    payload,
                );
            }
            let model_ids = entry.model_ids.map(|ids| {
                ids.map(|[type_id, group, instance]| dbpf::ResourceId {
                    type_id,
                    group,
                    instance,
                })
            });
            return Ok((model_ids, entry.material_resource_count, "hit"));
        }
    }
    // miss：编译，并把 insert-model 新增的资源写入缓存。
    let before: std::collections::HashSet<dbpf::ResourceId> =
        ctx.resources.all().keys().cloned().collect();
    let (model_ids, material_resource_count) = insert_scene_model(
        &ctx.root,
        &ctx.plan.intermediate.scene.model,
        ctx.plan.intermediate.scene.model_shape.as_deref(),
        &mut ctx.resources,
        &ctx.texture_instances,
        ctx.scene_document.as_ref(),
        rotation,
    )?;
    if let Some(key) = &fingerprint_key {
        let mut entries = Vec::new();
        let mut payloads = Vec::new();
        for (index, (id, bytes)) in ctx
            .resources
            .all()
            .iter()
            .filter(|(id, _)| !before.contains(*id))
            .enumerate()
        {
            let file = format!("{index:04}.bin");
            entries.push((id.type_id, id.instance, file.clone()));
            payloads.push((id.type_id, id.instance, bytes.clone()));
        }
        let entry = super::build_cache::MaterialCacheEntry {
            model_ids: model_ids.map(|ids| ids.map(|id| [id.type_id, id.group, id.instance])),
            material_resource_count,
            entries,
        };
        super::build_cache::store_material(&ctx.root, key, &entry, &payloads);
    }
    Ok((model_ids, material_resource_count, if fingerprint_key.is_some() { "miss" } else { "off" }))
}

/// 六槽源文件字节串联摘要：槽实例号受池内分配影响，载荷指纹不受。
fn slot_payload_digest(
    root: &std::path::Path,
    slots: &std::collections::BTreeMap<String, String>,
) -> String {
    let mut combined = Vec::new();
    for path in slots.values() {
        if let Ok(bytes) = std::fs::read(root.join(path)) {
            combined.extend_from_slice(&bytes);
        }
    }
    digest(&combined)
}

/// path 语义校验(entry 绑定 + PathEntry 资源存在性)。
fn validate_paths(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    ctx.scene = validate_scene_path_semantics(
        &ctx.root,
        &ctx.plan.intermediate.scene.document,
        &ctx.resources,
    )?;
    if ctx
        .scene_document
        .as_ref()
        .is_some_and(SceneDocument::has_semantics)
        && ctx.plan.intermediate.references.lot.is_none()
    {
        return Err(
            "scene document carries units/paths; reference.lot must name the Lot property to lower them into"
                .into(),
        );
    }
    Ok(serde_json::json!({
        "units": ctx.scene.unit_count,
        "paths": ctx.scene.path_count,
        "pathEntries": ctx.scene.path_entry_count,
    }))
}

/// source/plan/scene → source_hash + build_hash(确定性身份)。
fn compute_identity(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    ctx.source_hash = match &ctx.identity_resources {
        Some(identity) => format!(
            "{}:{}",
            connector::fingerprint(&ctx.resources),
            connector::fingerprint(identity)
        ),
        None => connector::fingerprint(&ctx.resources),
    };
    let plan_bytes = serde_json::to_vec_pretty(&ctx.plan).map_err(|e| e.to_string())?;
    ctx.build_hash = digest(&serde_json::to_vec(&(
        ctx.plan.processor_version,
        &ctx.source_hash,
        &plan_bytes,
        &ctx.scene_document_bytes_hash,
    ))
    .map_err(|e| e.to_string())?);
    ctx.plan_bytes = plan_bytes;
    Ok(serde_json::json!({ "buildHash": ctx.build_hash }))
}

/// semantics.properties → Property patch(目标排序/模板护栏/物化)。
fn apply_property_patches(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let protected = protected_templates(&ctx.resources, &ctx.plan.protected);
    let mut targets: Vec<_> = ctx.plan.typed_patches.iter().map(|p| p.target).collect();
    targets.sort_by_key(|id| (id.type_id, id.group, id.instance));
    targets.dedup();
    let mut tasks = Vec::new();
    for target in &targets {
        if !ctx.plan.intermediate.semantics.allow_shared_template && protected.contains(target) {
            return Err(format!("refusing to patch shared template {target}"));
        }
        let source = ctx
            .resources
            .get(target)
            .ok_or_else(|| format!("missing Property target {target}"))?;
        let patches = ctx
            .plan
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
        ctx.resources.insert(task.target, bytes);
    }
    ctx.targets = targets;
    Ok(serde_json::json!({ "patched": ctx.targets.len() }))
}

/// 场景语义(unit 列降级)+ 菜单注册(身份克隆)。
fn apply_scene_semantics(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
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
    // the Asset dictionary via native_profile (see native_profile::build).
    let menu_mode = ctx.plan.menu.is_some();
    let lot_target = to_id(ctx.plan.intermediate.references.lot);
    if let Some(target) = to_id(ctx.plan.intermediate.references.unit) {
        apply_property_semantics(
            &mut ctx.resources,
            target,
            None,
            if menu_mode { None } else { ctx.model_ids },
            &mut ctx.lowered_units,
        )?;
    }
    if let Some(target) = lot_target {
        apply_property_semantics(
            &mut ctx.resources,
            target,
            if menu_mode {
                None
            } else {
                ctx.scene_document.as_ref()
            },
            if menu_mode { None } else { ctx.model_ids },
            &mut ctx.lowered_units,
        )?;
    }
    ctx.menu_instance = match &ctx.plan.menu {
        Some(menu) => {
            let lot_source = lot_target
                .ok_or("menu registration requires reference.lot to clone the lot identity")?;
            let generated = super::menu::generate(
                menu,
                &ctx.resources,
                lot_source,
                ctx.scene_document.as_ref(),
                ctx.model_ids,
                ctx.identity_resources.as_ref(),
            )?;
            for (id, bytes) in generated.entries {
                ctx.resources.insert(id, bytes);
            }
            Some(generated.instance)
        }
        None => None,
    };
    // 菜单家族字段入台账（可观测性设计 §4.2：不开包即可回答"入口挂在哪、
    // 工具家族是什么"——s17 诊断 R1 的直接判别证据）。
    let menu_detail = ctx.plan.menu.as_ref().map(|menu| {
        serde_json::json!({
            "parentInstance": menu.menu_parent_instance
                .map(|value| format!("{value:#010X}")),
            "buildingCategoryInstance": menu.building_category_instance
                .map(|value| format!("{value:#010X}")),
            "categoryInstances": menu
                .category_instances
                .iter()
                .map(|value| format!("{value:#010X}"))
                .collect::<Vec<_>>(),
            "categoryInstance": menu
                .category_instance
                .map(|value| format!("{value:#010X}")),
        })
    });
    // P3 属性验收闸门：Asset/Unit 逐列指纹 vs 验收基线。材质/地面/树/灯/
    // 效果/道具/路网/锚点任一未声明漂移 = 构建失败并点名责任列（错误自带
    // [scene-semantics] 与分类）；有意变更须 schema 声明 lotChange。
    let mut detail = serde_json::json!({
        "menuMode": menu_mode,
        "menuInstance": ctx.menu_instance,
        "loweredUnits": ctx.lowered_units,
        "menu": menu_detail,
    });
    if let Some(instance) = ctx.menu_instance {
        let asset_id = dbpf::ResourceId {
            type_id: 0x00B1_B104,
            group: super::native_profile::ASSET_GROUP,
            instance,
        };
        let unit_id = dbpf::ResourceId {
            type_id: 0x00B1_B104,
            group: super::menu::UNIT_GROUP,
            instance,
        };
        let asset_file = sc_properties::PropertyFile::parse(&ctx.resources[&asset_id])
            .map_err(|error| format!("property gate: asset parse: {error}"))?;
        let unit_file = sc_properties::PropertyFile::parse(&ctx.resources[&unit_id])
            .map_err(|error| format!("property gate: unit parse: {error}"))?;
        let gate_report = super::property_gate::gate(
            &ctx.root,
            &asset_file,
            &unit_file,
            instance,
            ctx.lot_change_declared,
        )?;
        detail["propertyGate"] = gate_report;
    }
    Ok(detail)
}

/// 唯一副作用阶段:staging 写盘 → delta/全量 → 闭包补全 → 链接 → 读回校验。
fn materialize_and_link(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    ctx.delta_mode = ctx.plan.intermediate.output.mode != crate::asset_build_m0::OutputMode::Full;
    let mut staging = Staging::create(&ctx.root, &ctx.build_hash)?;
    staging.write("plan/plan.json", &ctx.plan_bytes)?;
    staging.write(
        "plan/im.json",
        &serde_json::to_vec_pretty(&ctx.plan.intermediate).map_err(|e| e.to_string())?,
    )?;
    if let Some(document) = read_scene_bytes(&ctx.root, &ctx.plan.intermediate.scene.document)? {
        staging.write("artifacts/scene/asset.lot.json", &document)?;
    }
    let manifest = super::package::materialize_scene_resources(
        &ctx.root,
        &ctx.plan.intermediate,
        &ctx.resources,
        &mut staging,
    )?;
    for target in &ctx.targets {
        let name = format!(
            "artifacts/property/{:08X}-{:08X}-{:08X}.property",
            target.type_id, target.group, target.instance
        );
        staging.write(&name, &ctx.resources[target])?;
    }
    // Delta output (default): only the resources this build created or
    // rewrote — the game resolves everything else from the base packages.
    // Full output re-serializes the whole reference snapshot (legacy
    // same-TGI overlay contract).
    let mut written: connector::Resources = if ctx.delta_mode {
        ctx.resources.delta()
    } else {
        ctx.resources.take_all()
    };
    if ctx.delta_mode {
        // A cloned Lot Property and a template-based RW4 can still point at
        // reference-package bytes.  Promote those bytes into the delta before
        // linking, and fail closed if a required reference is unresolved.
        let deps = super::resource_closure::discover(&written, ctx.plan.menu.is_some())?;
        super::resource_closure::complete(&mut written, ctx.resources.all(), &deps)?;
    }
    let output = connector::link(&written)?;
    staging.write(&super::package::package_relative_name(&ctx.plan), &output)?;
    super::connector::verify(
        &staging.path.join(&super::package::package_relative_name(&ctx.plan)),
        &written,
    )?;
    ctx.written_count = written.len();
    let result = M1BuildResult {
        path: staging.destination.join(&super::package::package_relative_name(&ctx.plan)),
        schema_hash: ctx.plan.intermediate.schema_hash.clone(),
        build_hash: ctx.build_hash.clone(),
        source_hash: ctx.source_hash.clone(),
        sha256: digest(&output),
        resource_count: written.len(),
        delta_mode: ctx.delta_mode,
        menu_instance: ctx.menu_instance,
        patched_resources: ctx.targets.len(),
        scope: "property+scene-model",
        scene: SceneSemanticSummary {
            unit_count: ctx.scene.unit_count,
            path_count: ctx.scene.path_count,
            path_entry_count: ctx.scene.path_entry_count,
            lowered_units: ctx.lowered_units,
            material_resources: ctx.material_resource_count,
        },
        events: ctx.events.clone(),
        started_at_ms: ctx.started_at_ms,
        finished_at_ms: super::events::now_ms(),
        duration_ms: super::events::now_ms().saturating_sub(ctx.started_at_ms),
    };
    staging.write(
        "reports/m1-build.json",
        &serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )?;
    ctx.result = Some(result);
    let ledger_dir = ctx.root.join(".build").join("ledger");
    std::fs::create_dir_all(&ledger_dir)
        .map_err(|error| format!("ledger directory: {error}"))?;
    let ledger_path = ledger_dir.join(format!("{}.json", ctx.build_hash));
    let ledger = serde_json::to_vec_pretty(ctx.result.as_ref().expect("result set"))
        .map_err(|error| format!("ledger encode: {error}"))?;
    // The ledger carries wall-clock run evidence (timestamps/durations), so a
    // repeat build with the same build_hash produces different bytes by
    // design: atomically replace it with the latest run's evidence instead of
    // failing create-new (os error 183 on the second identical build).
    crate::atomic_fs::write_atomic(&ledger_path, &ledger, true)
        .map_err(|error| format!("ledger write: {error}"))?;
    // P3：构建成功才刷新池基线——下次构建的 load-inputs 漂移即"相对本次
    // 已验收的输入池"的变化（reference 被外部改写当场可见）。
    let pool_last = ctx.root.join(".build").join("pool-last.json");
    std::fs::create_dir_all(pool_last.parent().expect("pool baseline parent"))
        .map_err(|error| format!("pool baseline dir: {error}"))?;
    std::fs::write(
        &pool_last,
        serde_json::to_vec_pretty(&ctx.pool_inventory)
            .map_err(|error| format!("pool baseline encode: {error}"))?,
    )
    .map_err(|error| format!("pool baseline write: {error}"))?;
    // Publish only after the task evidence is durable. A failed ledger write
    // therefore cannot leave a package that has no reviewable provenance.
    staging.publish()?;
    // P3 出货溯源：逐资源 TGI+字节进台账（按 TGI 排序，稳定可 diff）。
    let mut outputs: Vec<serde_json::Value> = written
        .iter()
        .map(|(id, bytes)| {
            serde_json::json!({
                "tgi": format!("{:08X}:{:08X}:{:08X}", id.type_id, id.group, id.instance),
                "bytes": bytes.len(),
            })
        })
        .collect();
    outputs.sort_by(|a, b| a["tgi"].as_str().cmp(&b["tgi"].as_str()));
    // P3 闭包 missing 升级为台账告警：场景 prop/贴图引用本地缺失不再只
    // 躺在 staging 的 manifest 文件里（树的模型缺失曾因此静默数周）。
    let closure_warnings = if manifest.missing.is_empty() {
        Vec::new()
    } else {
        vec![format!(
            "closure missing {} resource(s): {}",
            manifest.missing.len(),
            manifest.missing.join(", ")
        )]
    };
    Ok(serde_json::json!({
        "resources": ctx.written_count,
        "delta": ctx.delta_mode,
        "outputs": outputs,
        "closureManifest": {
            "lot": manifest.lot.len(),
            "prop": manifest.prop.len(),
            "bin": manifest.bin.len(),
            "path": manifest.path.len(),
            "pathEntry": manifest.path_entry.len(),
            "raster": manifest.raster.len(),
            "missing": manifest.missing,
        },
        "warnings": closure_warnings,
        "poolDrift": ctx.pool_drift,
    }))
}

// ---- G2 处理器级测试（设计文档 4.6：单处理器毫秒级驱动，不拉全链） ----
#[cfg(test)]
mod stage_tests {
    use super::*;
    use crate::asset_build_m1::plan::plan_from_schema;

    fn stage_root(label: &str) -> PathBuf {
        let root = crate::test_scratch::scratch_base()
            .join(format!("openscp-stage-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn model_schema() -> String {
        serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "stage-tower", "kind": "building-asset",
            "reference": {"package": "source.package"},
            "scene": {"document": "asset.lot.json", "model": "model.lotm"},
            "outputs": {"package": "stage-tower.package"}
        })
        .to_string()
    }

    /// insert-model 独立驱动：不经过 load-inputs/闭包/发布，毫秒级钉住
    /// "模型编译 ⇒ 模型身份 + 材质资源 + 闸门事件"这一保护区契约。
    #[test]
    fn insert_model_compiles_in_isolation() {
        let root = stage_root("insert-model");
        std::fs::write(root.join("model.lotm"), super::super::e2e_tests::workbench_lotm_with_meshes(2)).unwrap();
        let plan = plan_from_schema(&model_schema()).unwrap();
        let mut ctx = BuildContext::new(root.canonicalize().unwrap(), plan);
        let detail = insert_model(&mut ctx).unwrap();
        assert!(ctx.model_ids.is_some(), "compiled model identity produced");
        assert!(
            ctx.material_resource_count >= 2,
            "tint + params material resources produced"
        );
        assert_eq!(
            detail.pointer("/materialGate/verdict"),
            Some(&serde_json::json!("recorded")),
            "gate verdict in stage detail: {detail}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invariant_material_non_empty_fires_on_zero_resources() {
        let root = stage_root("invariant-material");
        let plan = plan_from_schema(&model_schema()).unwrap();
        let mut ctx = BuildContext::new(root.clone(), plan);
        ctx.model_ids = Some([dbpf::ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 1,
        }; 4]);
        ctx.material_resource_count = 0;
        let error = check_invariants("insert-model", &ctx).unwrap_err();
        assert!(error.contains("MATERIAL_NON_EMPTY_WHEN_MODEL"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invariant_slots_count_fires_on_missing_slots() {
        let root = stage_root("invariant-slots");
        let mut schema: serde_json::Value =
            serde_json::from_str(&model_schema()).unwrap();
        schema["scene"]["slots"] = serde_json::json!({
            "slot0": "slot0.raster", "slot1": "slot1.raster"
        });
        let plan = plan_from_schema(&schema.to_string()).unwrap();
        let mut ctx = BuildContext::new(root.clone(), plan);
        ctx.texture_instances = [0; 6];
        let error = check_invariants("insert-textures", &ctx).unwrap_err();
        assert!(error.contains("SLOTS_DECLARED_ALL_INSERTED"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invariants_silent_without_model_and_slots() {
        let root = stage_root("invariant-silent");
        let plan = plan_from_schema(&model_schema()).unwrap();
        let ctx = BuildContext::new(root.clone(), plan);
        check_invariants("insert-model", &ctx).unwrap();
        check_invariants("insert-textures", &ctx).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
