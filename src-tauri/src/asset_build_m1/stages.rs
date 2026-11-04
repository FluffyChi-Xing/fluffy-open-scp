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
}

impl BuildContext {
    pub fn new(root: PathBuf, plan: PropertyBuildPlan) -> Self {
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

pub(crate) const CHAIN: [&str; 8] = [
    "load-inputs",
    "insert-textures",
    "insert-model",
    "validate-paths",
    "compute-identity",
    "property-patches",
    "scene-semantics",
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
            "materialize-and-link" => materialize_and_link(ctx),
            _ => unreachable!("unknown chain stage {stage}"),
        };
        match outcome {
            Ok(detail) => emit(ctx, finished(stage, detail, started_at.elapsed().as_millis() as u64)),
            Err(error) => {
                emit(ctx, super::events::failed(stage, &error, started_at.elapsed().as_millis() as u64));
                return Err(format!("[{stage}] {error}"));
            }
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
    Ok(serde_json::json!({
        "identityPackage": ctx.identity_resources.is_some(),
        "sceneDocument": ctx.scene_document.is_some(),
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
    let material_contract = ctx
        .plan
        .intermediate
        .scene
        .model
        .as_deref()
        .and_then(|path| project_file(&ctx.root, path).ok())
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| super::lotm::is_container(&bytes).then(|| super::lotm::parse(&bytes).ok()).flatten())
        .map(|doc| serde_json::json!({
            "materialGroups": doc.materials.len(),
            "parameterColumns": doc.materials.iter().map(|m| m.param_cols).collect::<Vec<_>>(),
            "meshCount": doc.meshes.len(),
        }))
        .unwrap_or(serde_json::Value::Null);
    let (model_ids, material_resource_count) = insert_scene_model(
        &ctx.root,
        &ctx.plan.intermediate.scene.model,
        ctx.plan.intermediate.scene.model_shape.as_deref(),
        &mut ctx.resources,
        &ctx.texture_instances,
        ctx.scene_document.as_ref(),
        rotation,
    )?;
    ctx.model_ids = model_ids;
    ctx.material_resource_count = material_resource_count;
    Ok(serde_json::json!({
        "materialResources": material_resource_count,
        "materialContract": material_contract,
    }))
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
    Ok(serde_json::json!({
        "menuMode": menu_mode,
        "menuInstance": ctx.menu_instance,
        "loweredUnits": ctx.lowered_units,
    }))
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
    super::package::materialize_scene_resources(
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
    crate::atomic_fs::write_atomic(&ledger_path, &ledger, false)
        .map_err(|error| format!("ledger write: {error}"))?;
    // Publish only after the task evidence is durable. A failed ledger write
    // therefore cannot leave a package that has no reviewable provenance.
    staging.publish()?;
    Ok(serde_json::json!({
        "resources": ctx.written_count,
        "delta": ctx.delta_mode,
    }))
}
