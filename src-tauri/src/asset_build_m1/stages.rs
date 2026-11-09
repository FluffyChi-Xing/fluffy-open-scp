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
use super::events::{finished, started, StageEvent};
use super::filesystem::{project_file, Staging};
use super::package::LodModelIds;
use super::package::{
    apply_property_semantics, digest, insert_scene_model, insert_slot_raster, protected_templates,
    read_scene_bytes, read_scene_document, validate_scene_path_semantics, BuildResources,
    M1BuildResult, SceneSemanticSummary,
};
use super::plan::PropertyBuildPlan;
use super::processor::defer_property;
use super::scene::SceneDocument;
use dbpf::ResourceId;
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
    /// `semantics.behavior.presentationPackage`: 原版教育菜单/Unit 供体，
    /// behavior-bind 从中克隆统计行/uiCategories/图标（菜单/行为拆分的表现
    /// 层供体；参与 compute_identity 指纹，防缓存陈旧）。
    pub presentation_resources: Option<connector::Resources>,
    /// Independent editor ground Lot plus exactly its mask raster. This is not
    /// merged into the output package; it is evidence for cloning and readback.
    pub ground_source: Option<(connector::Resources, ResourceId)>,
    pub scene_document: Option<SceneDocument>,
    pub scene_document_bytes_hash: Option<String>,
    pub texture_instances: [u32; 6],
    pub model_ids: Option<LodModelIds>,
    /// The active compiler is the strict author Building40 route. This survives
    /// material-cache hits so the final DBPF gate checks the same geometry shape.
    pub author_building40: bool,
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
    /// 参照 Lot 的**原始字节**（load-inputs 时快照）。delta-audit 用它做
    /// "最终 lot vs 参照 lot" 的逐列对拍——这是 r10 事故的直接拦截点。
    pub reference_lot: Option<(ResourceId, Vec<u8>)>,
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
            presentation_resources: None,
            ground_source: None,
            scene_document: None,
            scene_document_bytes_hash: None,
            texture_instances: [0u32; 6],
            model_ids: None,
            author_building40: false,
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
            reference_lot: None,
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

pub(crate) const CHAIN: [&str; 16] = [
    "load-inputs",
    "insert-textures",
    "insert-model",
    "validate-paths",
    "compute-identity",
    "property-patches",
    "scene-semantics",
    "behavior-bind",
    "layout-contract",
    "menu-contract",
    "delta-audit",
    "vanilla-shadow-strip",
    "material-binding-contract",
    "identity-closure",
    "golden-shape",
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
            "vanilla-shadow-strip" => vanilla_shadow_strip(ctx),
            "material-binding-contract" => material_binding_contract(ctx),
            "identity-closure" => identity_closure(ctx),
            "golden-shape" => golden_shape(ctx),
            "layout-contract" => layout_contract(ctx),
            "menu-contract" => menu_contract(ctx),
            "delta-audit" => delta_audit(ctx),
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
                        super::events::failed(
                            stage,
                            &violation,
                            started_at.elapsed().as_millis() as u64,
                        ),
                    );
                    return Err(format!("[{stage}] {violation}"));
                }
                emit(
                    ctx,
                    finished(stage, detail, started_at.elapsed().as_millis() as u64),
                );
            }
            Err(error) => {
                emit(
                    ctx,
                    super::events::failed(stage, &error, started_at.elapsed().as_millis() as u64),
                );
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
    reads: &[
        "plan.scene.model",
        "plan.scene.modelShape",
        "texture_instances",
        "sceneDocument",
    ],
    writes: &[
        "model_ids",
        "material_resource_count",
        "resources(RW4/raster)",
        "material-baseline",
    ],
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
            return Err(format!("invariant {} violated: {fact}", invariant.code));
        }
    }
    Ok(())
}

fn select_ground_source_lot(
    document: &SceneDocument,
    resources: &connector::Resources,
    reference_lot: Option<ResourceId>,
    menu_registered: bool,
) -> Result<ResourceId, String> {
    match document.ground_source.as_ref() {
        Some(source)
            if source.package_path.is_some() || resources.get(&source.lot).is_some() =>
        {
            Ok(source.lot)
        }
        // A full, non-menu build edits the reference Lot in place. Older editor
        // documents can retain an unresolved imported sourceLot; the schema's
        // present reference Lot is the only complete source. Menu builds stay
        // strict: an unavailable explicit source cannot select the schema donor.
        Some(source) if !menu_registered => reference_lot
            .filter(|lot| resources.get(lot).is_some())
            .ok_or_else(|| {
                format!(
                    "lot.mask.kind=source: editor source Lot {} is unavailable and reference.lot is missing",
                    source.lot
                )
            }),
        Some(source) => Ok(source.lot),
        None => reference_lot.ok_or("lot.mask.kind=source requires sourceLot or reference.lot".into()),
    }
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
    // 参照 Lot 原始字节快照（delta-audit 的对照面）。必须在任何处理器改写
    // 池资源之前捕获；后续阶段只写不读这份快照。
    let lot_target = ctx.plan.intermediate.references.lot.map(|tgi| ResourceId {
        type_id: tgi.type_id,
        group: tgi.group,
        instance: tgi.instance,
    });
    ctx.reference_lot =
        lot_target.and_then(|id| ctx.resources.get(&id).cloned().map(|bytes| (id, bytes)));
    // donor lot 主体验证（2026-11-08 §7.1：参照 lot=40 字节 fixture → 克隆出
    // 20 列白板。游戏建筑常有变体，只有主体带全地面列；派生变体靠继承链
    // 补数据，克隆它 = 白板）。40E1C000（Asset 字典 lot）走 fail-closed
    // 契约；58F0C000 等 lot-config 形态不适用，跳过。继承链逐跳进台账。
    let master_report = match &ctx.reference_lot {
        Some((id, bytes)) => reference_lot_master_contract(id, bytes, &ctx.resources)?,
        None => serde_json::Value::Null,
    };
    ctx.identity_resources = match ctx
        .plan
        .menu
        .as_ref()
        .and_then(|menu| menu.identity_package.as_deref())
    {
        Some(path) => Some(connector::read_source(&project_file(&ctx.root, path)?)?),
        None => None,
    };
    ctx.presentation_resources = match ctx
        .plan
        .behavior
        .as_ref()
        .and_then(|behavior| behavior.presentation_package.as_deref())
    {
        Some(path) => Some(connector::read_source(&project_file(&ctx.root, path)?)?),
        None => None,
    };
    ctx.scene_document = read_scene_document(
        &ctx.root,
        &ctx.plan.intermediate.scene.document,
        ctx.plan.intermediate.scene.model.as_deref(),
    )?;
    ctx.ground_source = match ctx.scene_document.as_ref() {
        Some(document) if document.lot_mask == Some(super::scene::SceneLotMaskKind::Source) => {
            let source_lot = select_ground_source_lot(
                document,
                ctx.resources.all(),
                lot_target,
                ctx.plan.menu.is_some(),
            )?;
            let snapshot = if ctx.resources.get(&source_lot).is_some() {
                super::menu::ground_source_snapshot(ctx.resources.all(), source_lot)?
            } else {
                let package_path = document
                    .ground_source
                    .as_ref()
                    .and_then(|source| source.package_path.as_deref())
                    .ok_or_else(|| {
                        format!(
                            "ground source Lot {source_lot} is absent from reference.package and sourceLot.packagePath is missing"
                        )
                    })?;
                let package_path = super::filesystem::input_file(&ctx.root, package_path)?;
                let source_resources = connector::read_source(&package_path)?;
                super::menu::ground_source_snapshot(&source_resources, source_lot)?
            };
            Some((snapshot, source_lot))
        }
        _ => None,
    };
    // 显式 scene.writes 优先；缺省时按文档实际携带的列族派生（安全默认：
    // 没建模的列一律保留）。派生结果进入 plan → 参与 build_hash，可复现。
    if !ctx.plan.scene_writes.authoritative {
        let declared = ctx
            .scene_document
            .as_ref()
            .map(|document| super::scene::SceneWrites::parse(&document.writes))
            .unwrap_or_default();
        let effective = ctx.plan.scene_writes.clone().merge(declared);
        ctx.plan.scene_writes = if effective.is_empty() {
            ctx.scene_document
                .as_ref()
                .map(super::scene::SceneWrites::from_document)
                .unwrap_or_default()
        } else {
            effective
        };
    }
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
        "referenceLotMaster": master_report,
        "groundSource": ctx.ground_source.as_ref().map(|(resources, source_lot)| serde_json::json!({
            "sourceLot": source_lot.to_string(),
            "snapshotResources": resources.len(),
            "fingerprint": connector::fingerprint(resources),
        })),
        "pool": {"count": current.len(), "bytes": current.values().sum::<usize>(), "drift": pool_drift},
    }))
}

/// donor lot 主体契约：只接受带全地面列的主体变体，拒绝靠继承链补数据的
/// 派生变体；继承链（00B2CCCB 逐跳）写入台账日志。
///
/// 主体三要件 = 游戏渲染 lot 地面的最小本地列集：
/// - `00F9EFBA` BoundingBox（放置边界；缺它 menu-contract 只能告警）
/// - `0CCB7FC8` LotSize（地块尺寸向量）
/// - `0CCB7FD5` LotMask（地面掩码 raster 键）
///
/// 返回进台账的日志值；主体要件缺失 = Err（fail-closed，错误里指认"换主体
/// 变体"而非"调参数"）。
fn reference_lot_master_contract(
    lot_id: &ResourceId,
    bytes: &[u8],
    resources: &BuildResources,
) -> Result<serde_json::Value, String> {
    if lot_id.group != 0x40E1_C000 {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": format!("lot group {:08X} is not an asset-dictionary lot; master contract not applied", lot_id.group),
        }));
    }
    let file = sc_properties::PropertyFile::parse(bytes)
        .map_err(|error| format!("reference lot {:08X} parse: {error}", lot_id.instance))?;
    let column_count = file.values.len();
    let required: [(&str, u32); 3] = [
        ("00F9EFBA BoundingBox", 0x00F9_EFBA),
        ("0CCB7FC8 LotSize", 0x0CCB_7FC8),
        ("0CCB7FD5 LotMask", 0x0CCB_7FD5),
    ];
    let missing: Vec<String> = required
        .iter()
        .filter(|(_, hash)| file.get(*hash).is_none())
        .map(|(label, _)| label.to_string())
        .collect();
    // 继承链逐跳日志：主体判定证据（列数随跳递增 = 当前是派生变体）。
    let mut chain: Vec<serde_json::Value> = Vec::new();
    let mut cursor = file
        .get(0x00B2_CCCB)
        .and_then(|property| match &property.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) => Some(key.instance),
            _ => None,
        });
    for _ in 0..4 {
        let Some(instance) = cursor else { break };
        let parent_id = ResourceId {
            type_id: lot_id.type_id,
            group: lot_id.group,
            instance,
        };
        let (parent_columns, next) = match resources.get(&parent_id) {
            Some(bytes) => match sc_properties::PropertyFile::parse(bytes) {
                Ok(parent) => {
                    let next = parent
                        .get(0x00B2_CCCB)
                        .and_then(|property| match &property.kind {
                            sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) => {
                                Some(key.instance)
                            }
                            _ => None,
                        });
                    (parent.values.len() as i64, next)
                }
                Err(_) => (0, None),
            },
            None => (-1, None),
        };
        chain.push(serde_json::json!({
            "instance": format!("{instance:08X}"),
            "columns": parent_columns,
            "inPackage": parent_columns >= 0,
        }));
        cursor = next;
    }
    if !missing.is_empty() {
        let chain_text = chain
            .iter()
            .map(|hop| {
                format!(
                    "{}({}列{})",
                    hop["instance"].as_str().unwrap_or("?"),
                    hop["columns"],
                    if hop["inPackage"].as_bool().unwrap_or(false) {
                        ""
                    } else {
                        ",不在包内"
                    }
                )
            })
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(format!(
            "[load-inputs] reference lot 40E1C000:{:08X} is a DERIVED variant ({} columns, missing {}); \
             cloning it yields a white-slab lot. Use the MASTER variant (carries the full ground column set) — \
             scan candidates with `scan_big_lots` and pick one with BoundingBox+LotSize+LotMask. \
             继承链: {chain_text}",
            lot_id.instance,
            column_count,
            missing.join(", ")
        ));
    }
    Ok(serde_json::json!({
        "status": "master-ok",
        "columns": column_count,
        "parentChain": chain,
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

/// Legacy workbench LOTMs need a half-turn relative to the lot-unit frame.
/// Building40 follows the reference packager instead: its geometry and every
/// lot/path/prop/light column use the same authored rotation.
fn workbench_model_rotation(scene_rotation: u32) -> Result<u32, String> {
    match scene_rotation {
        0 => Ok(180),
        90 => Ok(270),
        180 => Ok(0),
        270 => Ok(90),
        other => Err(format!("朝向仅支持 0、90、180、270 度（收到 {other}）")),
    }
}

fn model_rotation(scene_rotation: u32, building40: bool) -> Result<u32, String> {
    if building40 {
        workbench_model_rotation(scene_rotation).map(|_| scene_rotation)
    } else {
        workbench_model_rotation(scene_rotation)
    }
}

/// 场景模型(LOTM/GLB)→ RW4 编译 + LOD id 分配(缺模型跳过)。
fn insert_model(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let scene_rotation = ctx
        .plan
        .menu
        .as_ref()
        .and_then(|menu| menu.rotation_degrees)
        // Author scene orientation (backend settings.rotationDegrees,
        // default 180). native_profile applies this value to lot columns.
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
    let document = model_bytes.as_deref().and_then(|bytes| {
        super::lotm::is_container(bytes)
            .then(|| super::lotm::parse(bytes).ok())
            .flatten()
    });
    let route = document.as_ref().and_then(|doc| {
        super::geometry_route::derive(ctx.plan.intermediate.scene.model_shape.as_deref(), doc).ok()
    });
    let rotation = model_rotation(
        scene_rotation,
        route.as_ref().is_some_and(|route| route.name == "building40"),
    )?;
    let material_contract = document
        .as_ref()
        .map(|doc| {
            let diagnostics = serde_json::from_str::<serde_json::Value>(&doc.diagnostics).ok();
            let diagnostic = |key: &str| {
                diagnostics
                    .as_ref()
                    .and_then(|value| value.get(key))
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
            };
            serde_json::json!({
                "materialGroups": doc.materials.len(),
                "parameterColumns": doc.materials.iter().map(|m| m.param_cols).collect::<Vec<_>>(),
                "meshCount": doc.meshes.len(),
                "source": diagnostic("source"),
                "nativeControlProfile": diagnostic("nativeControlProfile"),
                "windows": diagnostic("windows"),
                "decorPanels": diagnostic("decorPanels"),
                "nativeMaterials": diagnostic("nativeMaterials"),
            })
        })
        .unwrap_or(serde_json::Value::Null);
    // G1 材质指纹闸门：源模型 hash + 材质组/参数列/网格 + 六槽实例与载荷
    // + 材质资源数。与 .build/material-baseline.json 对拍；未声明的漂移在
    // 本处理器即失败（s10 贴图回归的构建期拦截点）。
    let gate = match (&model_bytes, ctx.plan.intermediate.scene.model.as_deref()) {
        (Some(bytes), Some(model_key)) => {
            let contract = super::lotm::parse(bytes).ok();
            let fingerprint = super::material_gate::MaterialFingerprint {
                model_sha256: digest(bytes),
                model_shape: route.as_ref().map(|route| route.name.to_string()),
                material_groups: contract.as_ref().map_or(0, |doc| doc.materials.len()),
                parameter_columns: contract
                    .as_ref()
                    .map(|doc| doc.materials.iter().map(|m| m.param_cols).collect())
                    .unwrap_or_default(),
                mesh_count: contract.as_ref().map_or(0, |doc| doc.meshes.len()),
                texture_slot_instances: ctx.texture_instances,
                slot_payload_sha256: slot_payload_digest(
                    &ctx.root,
                    &ctx.plan.intermediate.scene.slots,
                ),
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
        route.as_ref().map(|route| route.name),
        gate.as_ref().map(|(key, fingerprint)| {
            (
                key.clone(),
                fingerprint.model_sha256.clone(),
                fingerprint.slot_payload_sha256.clone(),
            )
        }),
    )?;
    ctx.model_ids = model_ids;
    ctx.author_building40 = route
        .as_ref()
        .is_some_and(|route| route.name == "building40");
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
    // Building40 production contract (2026-11-09): single compiled model
    // family + six authored material payloads. The six slots come from the
    // LOTM's material table (`author_materials`), not the scene's external
    // texture nodes; `ctx.texture_instances` may legitimately stay zero for a
    // native LOTM. Window completeness is enforced by the author material
    // compiler and the material-binding-contract (empty raster) gate below.
    let mut building40_contract = serde_json::Value::Null;
    if ctx.author_building40 {
        if ctx.model_ids.is_none() {
            return Err(
                "[insert-model] Building40 route compiled no LOD model ids — refusing empty geometry"
                    .into(),
            );
        }
        if material_resource_count < 6 {
            return Err(format!(
                "[insert-model] Building40 material resource count {material_resource_count} < 6"
            ));
        }
        if let Some(doc) = document.as_ref() {
            let groups = doc.materials.len();
            if groups == 0 {
                return Err(
                    "[insert-model] Building40 LOTM has zero material groups".into(),
                );
            }
        }
        building40_contract = serde_json::json!({
            "lodModels": true,
            "textureInstances": ctx.texture_instances,
            "materialResources": material_resource_count,
            "slot5Interior": ctx.texture_instances[5],
        });
    }
    // Record the interpreter decision separately from the author-facing field.
    let shape_declared = ctx.plan.intermediate.scene.model_shape.clone();
    let warnings: Vec<String> = Vec::new();
    Ok(serde_json::json!({
        "materialResources": material_resource_count,
        "materialContract": material_contract,
        "materialGate": gate_json,
        "materialCache": cache_state,
        "building40Contract": building40_contract,
        "sceneRotationDegrees": scene_rotation,
        "modelRotationDegrees": rotation,
        "modelShapeDeclared": shape_declared,
        "geometryRoute": route.as_ref().map(|route| serde_json::json!({
            "name": route.name,
            "reason": route.reason,
            "source": if shape_declared.is_some() { "schema" } else { "inferred" },
        })),
        "warnings": warnings,
    }))
}

/// G3b 材质缓存：指纹命中时跳过模型编译/图集编码，直接恢复材质与 RW4
/// 资源入池（alloc_free_instance 内容寻址 ⇒ 实例号与重编译一致）。
/// 返回 (model_ids, material_resource_count, "hit"|"miss"|"off")。
fn insert_model_with_cache(
    ctx: &mut BuildContext,
    rotation: u32,
    model_bytes: Option<&[u8]>,
    effective_shape: Option<&str>,
    gate_key: Option<(String, String, String)>,
) -> Result<(Option<super::package::LodModelIds>, usize, &'static str), String> {
    let transform = super::package::scene_model_transform(ctx.scene_document.as_ref());
    let transform_repr = format!("{transform:?};geometry-route={effective_shape:?}");
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
        effective_shape.or(ctx.plan.intermediate.scene.model_shape.as_deref()),
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
    Ok((
        model_ids,
        material_resource_count,
        if fingerprint_key.is_some() {
            "miss"
        } else {
            "off"
        },
    ))
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
    ctx.source_hash = match (&ctx.identity_resources, &ctx.presentation_resources) {
        (Some(identity), Some(presentation)) => format!(
            "{}:{}:{}",
            connector::fingerprint(&ctx.resources),
            connector::fingerprint(identity),
            connector::fingerprint(presentation)
        ),
        (Some(identity), None) => format!(
            "{}:{}",
            connector::fingerprint(&ctx.resources),
            connector::fingerprint(identity)
        ),
        (None, Some(presentation)) => format!(
            "{}:{}",
            connector::fingerprint(&ctx.resources),
            connector::fingerprint(presentation)
        ),
        (None, None) => connector::fingerprint(&ctx.resources),
    };
    if let Some((ground, source_lot)) = &ctx.ground_source {
        ctx.source_hash = format!(
            "{}:ground:{}:{}",
            ctx.source_hash,
            source_lot,
            connector::fingerprint(ground)
        );
    }
    let plan_bytes = serde_json::to_vec_pretty(&ctx.plan).map_err(|e| e.to_string())?;
    ctx.build_hash = digest(
        &serde_json::to_vec(&(
            ctx.plan.processor_version,
            &ctx.source_hash,
            &plan_bytes,
            &ctx.scene_document_bytes_hash,
        ))
        .map_err(|e| e.to_string())?,
    );
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
            &ctx.plan.scene_writes,
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
            &ctx.plan.scene_writes,
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
                &ctx.plan.scene_writes,
                ctx.model_ids,
                ctx.ground_source
                    .as_ref()
                    .map(|(resources, source_lot)| (resources, *source_lot)),
                ctx.identity_resources.as_ref(),
                &ctx.root,
                ctx.author_building40,
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

/// layout-contract（s25/s26/s27 事故后新增）：属性文件布局契约闸门。
///
/// 游戏加载器信任属性数组声明的 item_size 步长（2026-10-19 Text=8 实证菜单
/// ***；2026-11-07 Transform=0 实证整个 lot 拒载）。delta-audit/列 diff 都
/// 工作在解析值层，看不见声明元数据——本阶段在编码产物上 fail-closed 校验
/// 每个数组列的声明步长 ∈ vanilla 全量标定合法表（scan_strides.rs：2385
/// 文件、每类型恰好一个合法步长、零例外），空数组（vanilla 从不存在）同拒。
///
/// /36：游戏基类模板条目（vanilla-shadow-strip 将剥离的 shadow 组）跳过——
/// 它们是游戏自有字节（原版 9EC474FF 模板就带着 count=0 的 0CABB2FC 数组），
/// 不归本契约管；/35 把 strip 移到 delta-audit 之后后，本闸门先于 strip 运行，
/// 不跳过则每个部署构建都会被 vanilla 数据击落。
fn layout_contract(ctx: &BuildContext) -> Result<serde_json::Value, String> {
    let mut checked = 0usize;
    let mut arrays = 0usize;
    let mut skipped_shadows = 0usize;
    let mut violations: Vec<String> = Vec::new();
    let vanilla_shadows = vanilla_shadow_instances();
    for (id, bytes) in ctx.resources.iter() {
        if id.type_id != 0x00B1_B104 {
            continue;
        }
        // The reference snapshot is intentionally readable here for closure
        // checks, but its vanilla property layout is not our encoded output.
        // Validate only resources this build actually rewrote/created.
        if !ctx.resources.is_touched(id) {
            continue;
        }
        if vanilla_shadows.contains(&(id.group, id.instance)) {
            skipped_shadows += 1;
            continue;
        }
        let file = match sc_properties::PropertyFile::parse(bytes) {
            Ok(file) => file,
            Err(error) => {
                violations.push(format!(
                    "{:08X}:{:08X}:{:08X} parse: {error}",
                    id.type_id, id.group, id.instance
                ));
                continue;
            }
        };
        checked += 1;
        if let Err(error) = file.validate_declared_strides() {
            violations.push(format!(
                "{:08X}:{:08X}:{:08X} {error}",
                id.type_id, id.group, id.instance
            ));
        }
        arrays += file
            .values
            .iter()
            .filter(|property| matches!(property.kind, sc_properties::Kind::Array(_)))
            .count();
    }
    if !violations.is_empty() {
        return Err(format!(
            "[layout-contract] property layout violations ({}): {}",
            violations.len(),
            violations.join("; ")
        ));
    }
    Ok(serde_json::json!({
        "status": "ok",
        "property_files": checked,
        "array_columns": arrays,
        "vanilla_shadows_skipped": skipped_shadows,
    }))
}

/// unit/lot/template 组：instance 已存在于游戏包的资源 = 游戏自行提供，
/// 部署包不得携带（遮蔽即事故）。
const SHADOW_GROUPS: [u32; 7] = [
    0x40E0_C000,
    0x40E1_C000,
    0x40E0_C400,
    0x40E1_C400,
    0x48E0_C000,
    0x41E0_C000,
    0x41E1_C000,
];

/// vanilla-shadow-strip 与 layout-contract 共用的 shadow 组表与游戏侧
/// instance 集（元数据枚举，不解压内容）。OPENSCP_GAME_DATA 缺省（测试
/// 构建）= 空集（非部署构建，不跳过、不剥离）。
fn vanilla_shadow_instances() -> std::collections::HashSet<(u32, u32)> {
    let mut vanilla = std::collections::HashSet::new();
    let Ok(game_data) = std::env::var("OPENSCP_GAME_DATA") else {
        return vanilla;
    };
    for path in crate::game_assets::package_paths(std::path::Path::new(&game_data)) {
        let Ok(package) = dbpf::Package::open(&path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id == 0x00B1_B104 && SHADOW_GROUPS.contains(&entry.id.group) {
                vanilla.insert((entry.id.group, entry.id.instance));
            }
        }
    }
    vanilla
}

/// vanilla-shadow-strip（2026-11-08 HopeTower 事故后新增）：部署包
/// 基类遮蔽自动剥离 + 复核。
///
/// 事故：golden fixture（E2E 测试夹具）被用作部署参照，其携带的
/// 游戏基类模板链（9EC474FF Unit 基类等 40E1C000 组 6 件）随包出货，
/// 遮蔽游戏原版基类 → 全城建筑 transform 错乱（东倒西歪）+ 存档污染。
///
/// 机制：unit/lot/template 组（40E0C000/40E1C000/48E0C000/41E0C000/
/// 41E1C000/40E0C400/40E1C400）中，instance 已存在于游戏包（游戏会
/// 自行提供）的资源一律从部署包剥离；剥离后仍有残留 = 构建失败。
/// 游戏数据路径缺省（测试构建）时记录 warning 跳过。
fn vanilla_shadow_strip(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    // 游戏数据路径由应用启动时注入（lib.rs setup → OPENSCP_GAME_DATA）；
    // 测试构建未注入 = 非部署构建，跳过影子检查。
    if std::env::var("OPENSCP_GAME_DATA").is_err() {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "OPENSCP_GAME_DATA unset (non-deployment build); shadow check skipped",
        }));
    }
    let vanilla = vanilla_shadow_instances();
    let dropped = ctx
        .resources
        .strip_vanilla_shadows(&SHADOW_GROUPS, &vanilla);
    // 复核：剥离后不允许残留"与 vanilla 撞车"的条目（防御未来回归）。
    // 非撞车的新身份（我们自己的建筑）不受影响。
    let mut remaining: Vec<String> = Vec::new();
    for (id, _bytes) in ctx.resources.iter() {
        if id.type_id == 0x00B1_B104
            && SHADOW_GROUPS.contains(&id.group)
            && vanilla.contains(&(id.group, id.instance))
        {
            remaining.push(format!(
                "{:08X}:{:08X}:{:08X}",
                id.type_id, id.group, id.instance
            ));
        }
    }
    if !remaining.is_empty() {
        return Err(format!(
            "[vanilla-shadow-strip] residual vanilla-shadowing entries after strip: {}",
            remaining.join("; ")
        ));
    }
    Ok(serde_json::json!({
        "status": "ok",
        "stripped": dropped.len(),
        "dropped": dropped,
    }))
}

/// material-binding-contract（2026-11-08 白模事故后新增）：材质绑定字节层
/// 闸门。模拟游戏读取路径——产物池里每个 RW4 模型的材质纹理行（slot 0..=4）
/// 必须指向池内存在且非空的 raster（>64B；32B = 仅头部的空壳，tint/palette/
/// shader 无像素 → 白模）。§7.1 事故：多材质 LOTM 缺 `scene.modelShape` 落进
/// nh-partitioned 路线，塔 4 LOD 的 slot1/2/3 全指向 32B 空 raster，离线门禁
/// （资源计数/引用可解析）全绿、实机全白——绑定验证必须做在编码产物层。
/// fail-closed：报错携带修复指引（Building40 六槽），而非"构建通过、实机返工"。
fn material_binding_contract(ctx: &BuildContext) -> Result<serde_json::Value, String> {
    const RASTER_TYPE: u32 = 0x2F4E_681C;
    const MODEL_TYPE: u32 = 0x2F4E_681B;
    let empty_rasters: std::collections::HashSet<u32> = ctx
        .resources
        .iter()
        .filter(|(id, bytes)| id.type_id == RASTER_TYPE && bytes.len() <= 64)
        .map(|(id, _)| id.instance)
        .collect();
    let mut models = 0usize;
    let mut texture_rows = 0usize;
    let mut violations: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for (id, bytes) in ctx.resources.iter() {
        if id.type_id != MODEL_TYPE {
            continue;
        }
        let file = match rw4::Rw4File::parse(bytes) {
            Ok(file) => file,
            Err(error) => {
                // 夹具/不透明资源可能不是 RW4（与 menu-contract 的容忍一致）；
                // 真实模型都出自本编译器，解析失败不拦构建只记录。
                warnings.push(format!(
                    "{:08X}:{:08X}:{:08X} rw4 parse skipped: {error}",
                    id.type_id, id.group, id.instance
                ));
                continue;
            }
        };
        models += 1;
        // Native plugin exports intentionally preserve one MATERIAL section
        // per facade recipe. Their source contract may include a tiny raster
        // placeholder for an unused slot; that is not the same failure as an
        // NH/atlas model pointing at an empty raster for its active surface.
        let native_material_graph = file.sections_of_type(rw4::SectionType::MATERIAL).count() > 4;
        for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
            let material = match file.decode_material(bytes, section.number) {
                Ok(material) => material,
                Err(_) => continue,
            };
            for row in material.texture_refs() {
                let slot = row.slot & 0xFF;
                if slot == 0x2D {
                    continue; // shader-def 引用引擎侧资源，不是 raster
                }
                texture_rows += 1;
                let missing = !ctx.resources.iter().any(|(rid, _)| {
                    rid.type_id == RASTER_TYPE && rid.instance == row.texture_instance
                });
                let empty = empty_rasters.contains(&row.texture_instance);
                if empty && !native_material_graph {
                    // 确证白模：raster 在包内但只有头部无像素（§7.1 事故本体）。
                    violations.push(format!(
                        "model {:08X} material #{} slot{} -> raster {:08X} is an EMPTY raster (no pixels; renders white). \
                         Multi-material LOTM must use the Rust Building40 route (omit modelShape or set \"building40\") so six-slot textures compile",
                        id.instance, section.number, slot, row.texture_instance
                    ));
                } else if missing {
                    // 缺失 raster = 原版自带容忍现象（donor 装饰模型的悬空引用，
                    // 如 263E0569 链的 EDD00202，全安装无此文件而原版渲染正常）
                    // → 告警不拦截，保持可见。
                    warnings.push(format!(
                        "model {:08X} material #{} slot{} -> raster {:08X} not in package (vanilla-tolerated dangling ref)",
                        id.instance, section.number, slot, row.texture_instance
                    ));
                }
            }
        }
    }
    if !violations.is_empty() {
        return Err(format!(
            "[material-binding-contract] untextured material bindings ({}): {}{}",
            violations.len(),
            violations.join("; "),
            if warnings.is_empty() {
                String::new()
            } else {
                format!("; warnings({}): {}", warnings.len(), warnings.join("; "))
            }
        ));
    }
    Ok(serde_json::json!({
        "status": "ok",
        "models": models,
        "texture_rows": texture_rows,
        "empty_rasters": empty_rasters.len(),
        "dangling_refs": warnings.len(),
    }))
}

/// golden-shape（2026-11-08，用户裁决：与 s31 的差距必须在构建期可见）：
/// ①包内全部 property 实例 = 身份实例（s31：4 个 property 全是 be18da2d）；
/// ②Asset 引用的 LOD RW4 恰 1 个 mesh（s31：12 sections/1 Mesh；游戏 census
/// ≤4，孤儿形状 2026-10-29 已记录）；③raster ≤ 6（s31：3 atlas 页 + 1 mask）。
/// 任何一条不满足 = 构建失败，报错列出违规资源。
fn golden_shape(ctx: &BuildContext) -> Result<serde_json::Value, String> {
    let Some(instance) = ctx.menu_instance else {
        return Ok(serde_json::json!({"status":"skipped","reason":"no menu identity"}));
    };
    const PROPERTY: u32 = 0x00B1_B104;
    const MODEL: u32 = 0x2F4E_681B;
    const RASTER: u32 = 0x2F4E_681C;
    let mut violations: Vec<String> = Vec::new();
    let mut property_files = 0usize;
    // A custom PathEntry is a declared scene dependency, not a foreign donor
    // property. Exempt only entries referenced by this document and only in
    // C400 groups; every other non-identity Property remains a hard failure.
    let declared_path_entries: std::collections::HashSet<u32> = ctx
        .scene_document
        .as_ref()
        .into_iter()
        .flat_map(|document| document.paths.iter())
        .filter_map(|path| {
            super::path_catalog::resolve(
                path.kind.as_deref(),
                path.entry.as_deref(),
                path.scope_local,
                path.road_connect,
            )
            .ok()
        })
        .map(|recipe| recipe.entry_instance)
        .collect();
    for (id, _) in ctx.resources.iter() {
        if id.type_id == PROPERTY {
            property_files += 1;
            let declared_path_entry =
                (id.group & 0xFFFF) == 0xC400 && declared_path_entries.contains(&id.instance);
            if id.instance != instance && !declared_path_entry {
                violations.push(format!(
                    "foreign property {:08X}:{:08X}:{:08X} (identity is {instance:08X})",
                    id.type_id, id.group, id.instance
                ));
            }
        }
    }
    let rasters: Vec<ResourceId> = ctx
        .resources
        .iter()
        .filter(|(id, _)| id.type_id == RASTER)
        .map(|(id, _)| *id)
        .collect();
    if rasters.len() > 6 {
        violations.push(format!(
            "{} rasters in package (s31 GOLDEN carries 4: atlas pages + lot mask) —              textures must bake into atlas pages, not ship per-material rasters: {:08X?}",
            rasters.len(),
            rasters.iter().take(8).map(|id| id.instance).collect::<Vec<_>>()
        ));
    }
    // LOD 引用 mesh 数 == 1
    let asset_id = ResourceId {
        type_id: PROPERTY,
        group: 0x40E1_C000,
        instance,
    };
    if let Some(bytes) = ctx.resources.get(&asset_id) {
        if let Ok(file) = sc_properties::PropertyFile::parse(bytes) {
            for hash in [0x00F9_EFBBu32, 0x00F9_EFBC, 0x00F9_EFBD, 0x00F9_EFBE] {
                if let Some(property) = file.get(hash) {
                    if let sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) =
                        &property.kind
                    {
                        let model_id = ResourceId {
                            type_id: MODEL,
                            group: 0,
                            instance: key.instance,
                        };
                        if let Some(model_bytes) = ctx.resources.get(&model_id) {
                            match rw4::Rw4File::parse(model_bytes) {
                                Ok(model) => {
                                    let meshes =
                                        model.sections_of_type(rw4::SectionType::MESH).count();
                                    if meshes != 1 {
                                        violations.push(format!(
                                            "LOD model {:08X} has {meshes} meshes — GOLDEN shape is exactly 1 mesh per RW4                                              (game census ≤4; dozens of meshes = orphan shape, 2026-10-29)",
                                            key.instance
                                        ));
                                    }
                                }
                                Err(error) => violations.push(format!(
                                    "LOD model {:08X} rw4 parse: {error}",
                                    key.instance
                                )),
                            }
                        }
                    }
                }
            }
        }
    }
    if !violations.is_empty() {
        return Err(format!(
            "[golden-shape] package deviates from the s31 GOLDEN shape ({}): {}",
            violations.len(),
            violations.join("; ")
        ));
    }
    Ok(serde_json::json!({
        "status": "ok",
        "identity": format!("{instance:08X}"),
        "property_files": property_files,
        "rasters": rasters.len(),
    }))
}

/// identity-closure（2026-11-08，用户验收标准：包里只允许"这一栋楼"）：
/// 从身份根可达性保留资源，其余（donor lot、模板链、fixture 夹具、孤儿
/// raster）全部丢弃。产出 = s31 形状的包：property 文件只有身份自己的。
/// 丢弃清单进台账——每一件被丢的东西都可见、可追溯。
fn identity_closure(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    let Some(instance) = ctx.menu_instance else {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "no menu identity (not a menu asset)",
        }));
    };
    let dropped = ctx.resources.retain_identity_closure(instance);
    let preview: Vec<String> = dropped.iter().take(16).cloned().collect();
    Ok(serde_json::json!({
        "status": "ok",
        "identity": format!("{instance:08X}"),
        "dropped": dropped.len(),
        "dropped_preview": preview,
    }))
}

/// menu-contract（s25 事故后新增）：菜单/渲染契约闸门。
///
/// 覆盖四类历史事故：菜单条目出现但只有锚点（公园容器/模型引用/自映射）、
/// 标题 ***/乱码（locale 表/行）、菜单不加载（menuUnit/身份链/类别）、
/// lot/模型丢失（BoundingBox/LotMask/模型引用）。fail-closed，staging 之前。
fn menu_contract(ctx: &BuildContext) -> Result<serde_json::Value, String> {
    let Some(lot) = ctx.plan.intermediate.references.lot else {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "no reference lot (not a building asset)",
        }));
    };
    let instance = ctx.menu_instance.unwrap_or(lot.instance);
    let donor = ctx.menu_instance.and_then(|_| {
        ctx.identity_resources.as_ref().map(|resources| {
            let donor_instance = ctx
                .plan
                .menu
                .as_ref()
                .and_then(|menu| menu.identity_instance)
                .unwrap_or(super::menu::DALIAN_IDENTITY_INSTANCE);
            (resources, donor_instance)
        })
    });
    let report = if ctx.author_building40 {
        super::menu_contract::validate_building40(
            &ctx.resources,
            instance,
            super::menu::lot_group(),
            donor.map(|(resources, donor_instance)| (resources, donor_instance)),
            ctx.menu_instance.is_some(),
        )?
    } else {
        super::menu_contract::validate(
            &ctx.resources,
            instance,
            // 产物 lot 叶子固定在 Asset 字典组（40E1C000）——参照可能是 lot-config
            // （58F0C000，大学路线），二者不是同一层。2026-11-08：把参照组当产物组
            // 会让 menu-contract 在错误的组里找 lot/模型引用。
            super::menu::lot_group(),
            donor.map(|(resources, donor_instance)| (resources, donor_instance)),
            ctx.menu_instance.is_some(),
        )?
    };
    Ok(report.to_json())
}

/// delta-audit（r10 事故后新增）：最终 Lot vs 参照 Lot 逐列对拍。
///
/// 允许变化的列 = `scene.writes` 声明族的列（与写回共用
/// `scene::declared_column_hashes` 同一张表）+ 文档显式 lot 字段 +
/// LOD 模型引用列 + 显式 property patch 列 + behavior 声明的路网/看板列。
/// 其它任何 changed/added/removed = 阶段失败，**发生在 staging 之前**，
/// 因此上一次成功产物不会被覆盖（风险隔离）。
fn delta_audit(ctx: &mut BuildContext) -> Result<serde_json::Value, String> {
    use sc_properties::{
        LOD_MODEL_HASHES, LOT_OVERLAY_OFFSET_HASH, LOT_PLACEMENT_HASH, LOT_SIZE_HASH,
    };

    let writes = &ctx.plan.scene_writes;
    let declared = writes.describe();
    if writes.authoritative {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "authoritative scene writes (from-zero build)",
            "declared": declared,
        }));
    }
    let Some((lot_target, reference_bytes)) = ctx.reference_lot.clone() else {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "no reference lot",
            "declared": declared,
        }));
    };
    let Some(document) = ctx.scene_document.as_ref() else {
        return Ok(serde_json::json!({
            "status": "skipped",
            "reason": "no scene document",
            "declared": declared,
        }));
    };
    let Some(built_bytes) = ctx.resources.get(&lot_target) else {
        return Err(format!(
            "lot property {}:{}:{} missing from build pool",
            lot_target.type_id, lot_target.group, lot_target.instance
        ));
    };
    let reference = sc_properties::PropertyFile::parse(&reference_bytes)
        .map_err(|error| format!("reference lot parse: {error}"))?;
    let built = sc_properties::PropertyFile::parse(built_bytes)
        .map_err(|error| format!("built lot parse: {error}"))?;

    // 声明族列（与写回同表）+ 显式输入列。
    let mut allowed = super::scene::declared_column_hashes(writes);
    if document.lot_size.is_some() {
        allowed.push(LOT_SIZE_HASH);
    }
    if document.lot_offset.is_some() {
        allowed.push(LOT_OVERLAY_OFFSET_HASH);
    }
    if document
        .placement
        .as_ref()
        .is_some_and(|matrix| matrix.len() == 12)
    {
        allowed.push(LOT_PLACEMENT_HASH);
    }
    allowed.extend(
        super::scene::ground_values(document)
            .into_iter()
            .map(|(hash, _)| hash),
    );
    if document.lot_mask.is_some() {
        allowed.push(sc_properties::LOT_MASK_HASH);
    }
    // LOD 引用按确定性身份重定向；模型存在性由 insert-model 不变量兜底。
    allowed.extend(LOD_MODEL_HASHES);
    // 编译模型的内景控制器（package.rs 按材质契约写 0F169C46）是构建器
    // 自有字段，不是作者场景声明；否则每次带内景材质的构建都会被 delta
    // 审计误判为未声明漂移。
    allowed.push(0x0F16_9C46);
    if ctx.plan.behavior.is_some() {
        // legacy behavior 原语显式声明路网重写与看板族。
        allowed.extend([
            0x0CAA_680D,
            0x0CB0_0ED8,
            0x0CAA_6832,
            0x0CAA_6841,
            0x0CC8_FE7A,
            0x0D9A_4E54,
            0x0D9A_A2BB,
            0x0D79_70ED,
        ]);
    }
    if let Some(program) = &ctx.plan.behavior_program {
        // 逻辑编排：允许列从节点声明派生（不再"有行为就放行全部路网列"）。
        allowed.extend(program.declared_lot_columns.iter().copied());
    }
    for patch in &ctx.plan.typed_patches {
        if patch.target == lot_target {
            allowed.push(patch.hash);
        }
    }

    let mut detail = match super::delta_audit::audit(
        &reference,
        &built,
        lot_target.instance,
        lot_target.instance,
        &allowed,
        &declared,
    )? {
        None => serde_json::json!({
            "status": "pass",
            "declared": declared,
            "referenceColumns": reference.values.len(),
            "builtColumns": built.values.len(),
        }),
        Some(report) => serde_json::json!({
            "status": "declared",
            "declared": declared,
            "report": report.to_json(),
        }),
    };

    // 菜单身份路线（新 Unit/Lot/Menu2）逐列对拍：对照面是 **identity donor
    // lot**（不是参照 lot——新身份按设计从 identity donor 克隆）。未声明列
    // 必须与 donor 一致；r10 的"新身份整体重写"事故在这里会被点名。
    // 父无 donor 的 from-zero 路线没有可比基线，记录 skipped。
    if let Some(instance) = ctx.menu_instance {
        let donor_instance = ctx
            .plan
            .menu
            .as_ref()
            .and_then(|menu| menu.identity_instance)
            .unwrap_or(super::menu::DALIAN_IDENTITY_INSTANCE);
        let identity_lot = ResourceId {
            type_id: lot_target.type_id,
            group: lot_target.group,
            instance,
        };
        let donor_lot = ResourceId {
            type_id: lot_target.type_id,
            group: lot_target.group,
            instance: donor_instance,
        };
        let donor_bytes = ctx
            .identity_resources
            .as_ref()
            .and_then(|identity| identity.get(&donor_lot))
            .cloned();
        let Some(donor_bytes) = donor_bytes else {
            detail["menuIdentity"] = serde_json::json!({
                "status": "skipped",
                "reason": "no identity donor lot (from-zero menu route)",
                "instance": format!("{instance:08X}"),
            });
            return Ok(detail);
        };
        let Some(bytes) = ctx.resources.get(&identity_lot) else {
            return Err(format!(
                "menu identity lot {identity_lot} missing from build pool"
            ));
        };
        let identity_file = sc_properties::PropertyFile::parse(bytes)
            .map_err(|error| format!("menu identity lot parse: {error}"))?;
        let donor_file = sc_properties::PropertyFile::parse(&donor_bytes)
            .map_err(|error| format!("identity donor lot parse: {error}"))?;
        // 身份重定向列（clone_donor_identity 的固定副作用）：自映射表与
        // 标题链接必须指向新身份/新 locale，不能要求与 donor 字节一致。
        let mut identity_allowed = allowed.clone();
        identity_allowed.extend([0x0CA5_CF4A, 0x09FB_78CB]);
        let report = super::delta_audit::audit(
            &donor_file,
            &identity_file,
            donor_instance,
            instance,
            &identity_allowed,
            &declared,
        )
        .map_err(|error| {
            format!("menu identity {instance:08X} vs donor {donor_instance:08X}: {error}")
        })?;
        detail["menuIdentity"] = match report {
            None => serde_json::json!({
                "status": "pass",
                "instance": format!("{instance:08X}"),
                "donor": format!("{donor_instance:08X}"),
            }),
            Some(report) => serde_json::json!({
                "status": "declared",
                "instance": format!("{instance:08X}"),
                "donor": format!("{donor_instance:08X}"),
                "report": report.to_json(),
            }),
        };
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
    let staged_package = staging
        .path
        .join(&super::package::package_relative_name(&ctx.plan));
    staging.write(&super::package::package_relative_name(&ctx.plan), &output)?;
    super::connector::verify(&staged_package, &written)?;
    // Validate the actual DBPF read back from disk, not the intermediate pool.
    // A failed contract aborts before staging.publish(), so a bad package can
    // never replace the last accepted deliverable.
    let final_resources = connector::read_source(&staged_package)?;
    let donor = ctx.menu_instance.and_then(|_| {
        ctx.identity_resources.as_ref().map(|resources| {
            let donor_instance = ctx
                .plan
                .menu
                .as_ref()
                .and_then(|menu| menu.identity_instance)
                .unwrap_or(super::menu::DALIAN_IDENTITY_INSTANCE);
            (resources, donor_instance)
        })
    });
    let final_contract = super::written_contract::validate(
        &final_resources,
        super::written_contract::WrittenContractSpec {
            menu_instance: ctx.menu_instance,
            reference_lot: ctx.reference_lot.as_ref().map(|(id, _)| *id),
            document: ctx.scene_document.as_ref(),
            writes: &ctx.plan.scene_writes,
            model_ids: ctx.model_ids,
            author_building40: ctx.author_building40,
            donor,
            ground_source: ctx
                .ground_source
                .as_ref()
                .map(|(resources, source_lot)| (resources, *source_lot)),
            rotation_degrees: ctx
                .plan
                .menu
                .as_ref()
                .and_then(|menu| menu.rotation_degrees)
                .unwrap_or(180),
        },
    )?;
    staging.write(
        "reports/written-contract.json",
        &serde_json::to_vec_pretty(&final_contract.detail).map_err(|error| error.to_string())?,
    )?;
    ctx.lowered_units = final_contract.lowered_units;
    ctx.written_count = final_resources.len();
    let result = M1BuildResult {
        path: staging
            .destination
            .join(&super::package::package_relative_name(&ctx.plan)),
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
    std::fs::create_dir_all(&ledger_dir).map_err(|error| format!("ledger directory: {error}"))?;
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
        "writtenContract": final_contract.detail,
        "poolDrift": ctx.pool_drift,
    }))
}

// ---- G2 处理器级测试（设计文档 4.6：单处理器毫秒级驱动，不拉全链） ----
#[cfg(test)]
mod stage_tests {
    use super::*;
    use crate::asset_build_m1::plan::plan_from_schema;
    use sc_properties::{
        Kind, PropType, Property, PropertyEncoding, PropertyFile, Value as PropValue,
    };

    /// 构造一个最小 lot property 文件（可选拾取列）。
    fn lot_property_file(master: bool) -> Vec<u8> {
        let mut values: Vec<Property> = vec![Property {
            hash: 0x00B2_CCCB,
            prop_type: PropType::Key,
            kind: Kind::Scalar(PropValue::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: 0xAAAA_0001,
            })),
            encoding: PropertyEncoding::default(),
        }];
        if master {
            for hash in [0x00F9_EFBAu32, 0x0CCB_7FC8, 0x0CCB_7FD5] {
                values.push(Property {
                    hash,
                    prop_type: PropType::Float,
                    kind: Kind::Scalar(PropValue::Float(1.0)),
                    encoding: PropertyEncoding::default(),
                });
            }
        }
        PropertyFile {
            claimed_count: values.len() as u32,
            values,
        }
        .encode_canonical()
        .unwrap()
    }

    #[test]
    fn ground_source_selection_preserves_menu_donor_and_s31_reference_fallback() {
        let reference = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xBE18_DA2D,
        };
        let imported = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x58F0_C000,
            instance: 0x114A_0EDA,
        };
        let resources = connector::Resources::from([(reference, vec![1])]);
        let document = SceneDocument {
            lot_mask: Some(super::super::scene::SceneLotMaskKind::Source),
            ground_source: Some(super::super::scene::SceneGroundSource {
                lot: imported,
                package_path: None,
            }),
            ..SceneDocument::default()
        };
        assert_eq!(
            select_ground_source_lot(&document, &resources, Some(reference), false).unwrap(),
            reference,
            "s31 full same-Lot route falls back to its present reference Lot"
        );
        assert_eq!(
            select_ground_source_lot(&document, &resources, Some(reference), true).unwrap(),
            imported,
            "menu route never silently substitutes the schema donor"
        );
    }

    #[test]
    fn reference_lot_master_contract_accepts_master_and_logs_chain() {
        let mut resources = BuildResources::empty();
        let master_id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0x263E_0569,
        };
        resources.insert(master_id, lot_property_file(true));
        let report = reference_lot_master_contract(
            &master_id,
            &resources.get(&master_id).unwrap(),
            &resources,
        )
        .unwrap();
        assert_eq!(report["status"], "master-ok", "{report}");
        assert_eq!(report["columns"], 4, "{report}");
        assert!(report["parentChain"].is_array(), "chain logged: {report}");
    }

    #[test]
    fn reference_lot_master_contract_rejects_derived_variant() {
        // 派生变体：两列（parent 指向主体 + 一个数据列），三要件缺失。
        let derived_id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0x8D00_7745,
        };
        let bytes = PropertyFile {
            claimed_count: 2,
            values: vec![
                Property {
                    hash: 0x00B2_CCCB,
                    prop_type: PropType::Key,
                    kind: Kind::Scalar(PropValue::Key(sc_properties::Key {
                        type_id: 0x00B1_B104,
                        group: 0x40E1_C000,
                        instance: 0x263E_0569,
                    })),
                    encoding: PropertyEncoding::default(),
                },
                Property {
                    hash: 0x0BBB_CDED,
                    prop_type: PropType::Int32,
                    kind: Kind::Scalar(PropValue::Int32(219)),
                    encoding: PropertyEncoding::default(),
                },
            ],
        }
        .encode_canonical()
        .unwrap();
        let mut resources = BuildResources::empty();
        resources.insert(
            ResourceId {
                type_id: 0x00B1_B104,
                group: 0x40E1_C000,
                instance: 0x263E_0569,
            },
            lot_property_file(true),
        );
        let error = reference_lot_master_contract(&derived_id, &bytes, &resources).unwrap_err();
        assert!(error.contains("DERIVED variant"), "{error}");
        assert!(error.contains("00F9EFBA"), "{error}");
        assert!(error.contains("scan_big_lots"), "{error}");
        // 继承链进日志：第一跳 = 主体 263E0569（4 列，在包内）。
        assert!(error.contains("263E0569"), "{error}");
    }

    #[test]
    fn reference_lot_master_contract_skips_non_asset_lots() {
        let resources = BuildResources::empty();
        let config_id = ResourceId {
            type_id: 0x00B1_B104,
            group: 0x58F0_C000,
            instance: 0x2063_CCDD,
        };
        let report =
            reference_lot_master_contract(&config_id, &lot_property_file(false), &resources)
                .unwrap();
        assert_eq!(report["status"], "skipped", "{report}");
    }

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

    #[test]
    fn building40_preserves_author_rotation_while_legacy_routes_keep_the_half_turn() {
        for scene in [0, 90, 180, 270] {
            assert_eq!(model_rotation(scene, true).unwrap(), scene);
        }
        assert_eq!(model_rotation(0, false).unwrap(), 180);
        assert_eq!(model_rotation(90, false).unwrap(), 270);
        assert_eq!(model_rotation(180, false).unwrap(), 0);
        assert_eq!(model_rotation(270, false).unwrap(), 90);
        assert!(model_rotation(45, true).is_err());
        assert!(model_rotation(45, false).is_err());

        let matrix = super::super::native_model::rotate_xyz_matrix(model_rotation(180, true).unwrap())
            .unwrap();
        assert_eq!(
            super::super::native_model::rotate_point(&matrix, [2.0, 3.0, 4.0]),
            [-2.0, -3.0, 4.0]
        );
    }

    /// insert-model 独立驱动：不经过 load-inputs/闭包/发布，毫秒级钉住
    /// "模型编译 ⇒ 模型身份 + 材质资源 + 闸门事件"这一保护区契约。
    #[test]
    fn insert_model_compiles_in_isolation() {
        let root = stage_root("insert-model");
        std::fs::write(
            root.join("model.lotm"),
            super::super::e2e_tests::workbench_lotm_with_meshes(2),
        )
        .unwrap();
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
        assert_eq!(detail["sceneRotationDegrees"], 180);
        assert_eq!(detail["modelRotationDegrees"], 180);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invariant_material_non_empty_fires_on_zero_resources() {
        let root = stage_root("invariant-material");
        let plan = plan_from_schema(&model_schema()).unwrap();
        let mut ctx = BuildContext::new(root.clone(), plan);
        ctx.model_ids = Some(
            [dbpf::ResourceId {
                type_id: 0x2F4E_681B,
                group: 0,
                instance: 1,
            }; 4],
        );
        ctx.material_resource_count = 0;
        let error = check_invariants("insert-model", &ctx).unwrap_err();
        assert!(error.contains("MATERIAL_NON_EMPTY_WHEN_MODEL"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invariant_slots_count_fires_on_missing_slots() {
        let root = stage_root("invariant-slots");
        let mut schema: serde_json::Value = serde_json::from_str(&model_schema()).unwrap();
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
