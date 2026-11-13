//! Pure validation and lowering. No package reads or artifact evaluation.
use super::patch::{PropertyPatch, PropertyPatchSpec, parse_patch};
use crate::asset_build_m0::{IntermediateModel, lower_schema, parse_schema};
use dbpf::ResourceId;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Serialize, Debug)]
pub struct PropertyBuildPlan {
    pub processor_version: &'static str,
    pub scope: &'static str,
    pub intermediate: IntermediateModel,
    pub patches: Vec<PropertyPatchSpec>,
    pub order: [&'static str; 3],
    /// `semantics.menu`: register the built building as a NEW menu entry
    /// (new property identity) instead of overriding the reference lot.
    pub menu: Option<super::menu::MenuRegistrationSpec>,
    #[serde(skip)]
    pub(crate) selected_menu_categories: Option<Vec<u32>>,
    /// `semantics.behavior`: legacy single-spec behavior (education-minimal).
    /// Kept for byte-compatible r7-r10 projects.
    pub behavior: Option<super::behavior::BehaviorSpec>,
    /// 逻辑编排图编译后的行为指令集（`semantics.labor` + `semantics.behaviors`）。
    /// 与 legacy `behavior` 互斥；menu + behaviors 允许组合（行为绑定到新菜单身份）。
    pub behavior_program: Option<super::behavior_nodes::ResolvedBehavior>,
    /// `scene.writes`（delta 模式声明）：只有列出的列族会被场景写回覆盖。
    pub scene_writes: super::scene::SceneWrites,
    #[serde(skip)]
    pub(crate) typed_patches: Vec<PropertyPatch>,
    #[serde(skip)]
    pub(crate) protected: HashSet<ResourceId>,
}

pub fn plan_from_schema(raw: &str) -> Result<PropertyBuildPlan, String> {
    let parsed = parse_schema(raw).map_err(|error| {
        serde_json::to_string(&error.diagnostics).unwrap_or_else(|_| error.to_string())
    })?;
    let im = lower_schema(&parsed)?;
    if im.references.package.is_none() {
        return Err("M1 requires reference.package".into());
    }
    if !im.semantics.unit_overrides.is_empty() || !im.semantics.lot_bindings.is_empty() {
        return Err("M1 supports native model lowering, raster slots, scene unit lowering and menu registration; semantics.unitOverrides/lotBindings are not implemented".into());
    }
    let mut menu = if im.semantics.menu.is_empty() {
        None
    } else {
        let spec = super::menu::MenuRegistrationSpec::from_values(&im.semantics.menu)?;
        // 自绘地面无 donor（scene.authoredGround）：lot 身份由构建器从
        // 项目内合成（native_lot 契约），菜单注册不再克隆 reference.lot。
        // 参考包仍服务菜单/行为骨架（unit donor 链），那部分不能省。
        if im.references.lot.is_none() && !im.scene.authored_ground {
            return Err(
                "semantics.menu requires reference.lot to clone the lot identity from"
                    .into(),
            );
        }
        Some(spec)
    };
    let selected_menu_categories = menu
        .as_ref()
        .and_then(|spec| spec.ui_categories())
        .map(<[u32]>::to_vec);
    let scene_writes = super::scene::SceneWrites::parse(&im.scene.writes);
    // 声明位拼写错误必须 fail-closed：`"path"` 会被 parse 静默忽略，构建
    // 看起来"成功"但什么都没写——这正是 r10 类事故的温床。
    for family in &im.scene.writes {
        let valid = matches!(
            family.as_str(),
            "paths" | "lights" | "spawners" | "effects" | "decals"
        ) || family
            .strip_prefix("props:")
            .and_then(|bin| bin.parse::<usize>().ok())
            .is_some_and(|bin| bin < sc_properties::lot_unit::PROP_BINS);
        if !valid {
            return Err(format!(
                "scene.writes: unknown column family {family:?}; expected \
                 \"paths\", \"lights\", \"spawners\", \"effects\", \"decals\" or \
                 \"props:<0..{}>\"",
                sc_properties::lot_unit::PROP_BINS.saturating_sub(1)
            ));
        }
    }
    let has_composed = im.semantics.labor.is_some() || !im.semantics.behaviors.is_empty();
    if has_composed && !im.semantics.behavior.is_empty() {
        return Err(
            "semantics.behavior and semantics.behaviors/labor are mutually exclusive; \
             migrate the legacy binding to logic nodes"
                .into(),
        );
    }
    if has_composed && im.references.lot.is_none() && !im.scene.authored_ground {
        return Err(
            "semantics.behaviors/labor require reference.lot to bind the donor identity".into(),
        );
    }
    // 行为声明必须搭配 full 输出：delta 包依赖 donor 在场， donor 下架
    // 即重演 s24 菜单 ***/模型/树丢失。full 模式把参考快照整体重打包，
    // 保证包独立可工作。
    if (has_composed || !im.semantics.behavior.is_empty())
        && im.output.mode != crate::asset_build_m0::OutputMode::Full
    {
        return Err(
            "behavior bindings require outputs.mode \"full\" (a standalone package must not \
             depend on the donor at runtime)"
                .into(),
        );
    }
    let behavior = if im.semantics.behavior.is_empty() {
        None
    } else {
        Some(super::behavior::BehaviorSpec::from_values(
            &im.semantics.behavior,
        )?)
    };
    let behavior_program = if has_composed {
        let mut labor = super::behavior_nodes::parse_labor(im.semantics.labor.as_ref())?;
        let nodes = super::behavior_nodes::parse_nodes(&im.semantics.behaviors)?;
        if menu.is_none() && nodes.iter().any(|node| node["type"] == "hotel") {
            return Err("HOTEL_RESOURCE".into());
        }
        let author_tourism = menu.is_some() && super::author_tourism::selected(&nodes);
        if author_tourism
            && im.semantics.labor.as_ref().is_some_and(|value| value["minWorkers"].is_null())
            && let Some(labor) = labor.as_mut()
        {
            labor.min_workers = 0;
        }
        Some(super::behavior_nodes::compile_with_profile(labor, &nodes, author_tourism)?)
    } else {
        None
    };
    // IR 叶子 → 家族适配器补齐 menu donor / Menu2 骨架（作者显式字段优先）。
    if let (Some(spec), Some(program)) = (menu.as_mut(), behavior_program.as_ref()) {
        if let Some(mut semantics) = super::behavior_family::semantics_from_program(program) {
            if program.author_tourism {
                // The author Unit carries its own labor rules. Do not select
                // education menu rows or inherit a Unit template for its jobs.
                semantics.loops.retain(|kind| *kind != super::behavior_loops::BehaviorLoop::Labor);
                semantics.simulation_template = None;
            }
            super::behavior_family::enrich_menu(spec, &semantics);
        }
    }
    // Portable output names: avoid Windows devices, streams and trailing-dot aliases.
    let name = &im.output.package;
    let stem = name.trim_end_matches(".package");
    if stem.is_empty()
        || name.len() > 128
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
        || name.starts_with('.')
        || [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ]
        .contains(
            &stem
                .split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str(),
        )
    {
        return Err("M1 output must be a portable package filename".into());
    }
    let specs: Vec<PropertyPatchSpec> = im
        .semantics
        .properties
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|error| format!("semantics.properties: {error}"))?;
    let patches = specs
        .iter()
        .cloned()
        .map(parse_patch)
        .collect::<Result<Vec<_>, _>>()?;
    let mut protected = HashSet::new();
    for tgi in [im.references.unit_template, im.references.lot_template]
        .into_iter()
        .flatten()
    {
        protected.insert(ResourceId {
            type_id: tgi.type_id,
            group: tgi.group,
            instance: tgi.instance,
        });
    }
    // Known landmark template pair stays protected even if omitted from reference.
    for group in [0x48E0C000, 0x40E1C000] {
        protected.insert(ResourceId {
            type_id: 0x00B1B104,
            group,
            instance: 0x4E01E311,
        });
    }
    for patch in &patches {
        if !im.semantics.allow_shared_template
            && (protected.contains(&patch.target) || patch.target.group == 0x48E0C000)
        {
            return Err(format!(
                "refusing to patch shared template {}",
                patch.target
            ));
        }
    }
    Ok(PropertyBuildPlan {
        selected_menu_categories,
        // /25：新增 menu-contract 闸门（锚点/乱码/不加载/模型丢失四类前置校验），
        // 且菜单身份不再合成 0x0CA5CF4A 自映射。
        // /36：成本 f32 编码收口（i32 位型实机显示「免费」）+ 遗留共享 locale
        // 表禁写（v7 同表遮蔽大连希望大厦）+ menu-contract 第 7/8 号检查。
        // /37：material-binding-contract 闸门（RW4 纹理行 → 非空 raster 闭包，
        // 白模的构建期拦截）+ 参照 lot 主体契约（三要件+继承链日志）+
        // modelShape 漏写关键日志。
        // /44：生产路径钉死 Rust Building40；schema 拒绝 nh-atlas/merged-single
        // 与外部 SC5 Packager（.pyc / backend.command）。工作台只编排场景/逻辑，
        // 建筑编译不经过 Python packager。
        // /42：默认 LOTM9 → 作者 Building40 + 六槽材质直编译；禁止新模型
        // 静默落回 NH atlas/拆 mesh，并使旧模型缓存全部失效。场景属性仍由
        // 自有 menu/scene/path/behavior 处理器负责，不经过作者 build_candidate。
        // /40：identity-closure 阶段——部署包只保留身份可达资源（s31 形状）。
        // /38：lot 身份文本列剔除（09FB78CB donor 名字边）+ 地块尺寸/遮罩以
        // 工作台文档为准（80×80 方地）+ BoundingBox=编译模型真实包围盒 +
        // 菜单统计行统一 s31 GOLDEN 三对 text↔param。
        // /53–55: ordinary vehicle paths use Tokyo Tower's 03106C2E entry.
        // Bump whenever game-side lowering/geometry contracts change. The
        // run cache must never replay a package built by an older interpreter.
        processor_version: if behavior_program
            .as_ref()
            .is_some_and(|program| !program.templates.is_empty())
        {
            "openscp.property.m1/55"
        } else if behavior_program.as_ref().is_some_and(|program| program.author_tourism) {
            "openscp.property.m1/54"
        } else {
            "openscp.property.m1/53"
        },
        scope: "property+scene-model",
        intermediate: im,
        patches: specs,
        typed_patches: patches,
        protected,
        menu,
        behavior,
        behavior_program,
        scene_writes,
        order: ["property", "dbpf", "readback"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_ground_menu_does_not_require_reference_lot() {
        // donor-free 自绘地面：菜单注册不再克隆 reference.lot（地面由
        // 构建器从项目内合成）；参考包仍必须提供（菜单/行为骨架）。
        let schema = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "authored", "kind": "building-asset",
            "reference": {"package": "source.package"},
            "scene": {"document": "asset.lot.json", "authoredGround": true},
            "semantics": {"menu": {"title": "T"}},
            "outputs": {"package": "000_authored.package", "mode": "full"}
        });
        let plan = plan_from_schema(&schema.to_string()).unwrap();
        assert!(plan.menu.is_some());
        assert!(plan.intermediate.references.lot.is_none());
        assert!(plan.intermediate.scene.authored_ground);
    }

    #[test]
    fn menu_without_reference_lot_still_requires_a_donor_outside_authored_ground() {
        let schema = serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "legacy", "kind": "building-asset",
            "reference": {"package": "source.package"},
            "scene": {"document": "asset.lot.json"},
            "semantics": {"menu": {"title": "T"}},
            "outputs": {"package": "000_legacy.package", "mode": "full"}
        });
        let error = plan_from_schema(&schema.to_string()).unwrap_err();
        assert!(error.contains("reference.lot"), "{error}");
    }
}
