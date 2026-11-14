//! 行为家族适配器：前端简单叶子 IR → 后端补齐游戏可识别语义。
//!
//! 工作台只表达意图（park / tourism / education / landvalue / labor）。
//! 本模块按叶子组合选择家族适配器，补齐 Unit donor、Menu2 骨架、学校残留
//! 剥离、岗位接线策略等——未来加「可住地标」等复合类型时只注册新适配器，
//! 不改前端配置面。

use super::behavior_loops::{self, BehaviorLoop};
use super::behavior_nodes::ResolvedBehavior;
use super::menu::{ELEMENTARY_TEMPLATE, MenuRegistrationSpec};
use serde::Serialize;

/// 公园行为类别（菜单容器与 Unit 模拟类别在公园族上重合）。
const PARK_BEHAVIOR_CATEGORY: u32 = 0x0704_9250;
/// 地标 Unit 父模板（文化>地标实证）。
const LANDMARK_TEMPLATE: u32 = 0x4E01_E311;
/// 教育行为类别。
const EDUCATION_CATEGORY: u32 = 0x95A9_D6FD;
/// 教育看板容器（s31 Menu2 uiCategories 中间项；关闭设施按钮所在面板族）。
const EDUCATION_BOARD: u32 = 0x6A17_85B5;

/// 小学 / 社区大学教育签名 bundle（complete-building = s31 大学叶）。
const EDUCATION_BUNDLE_PRIMARY: u32 = 0x6C04_2367;
const EDUCATION_BUNDLE_COLLEGE: u32 = 0x6C04_2366;
/// 学校样本岗位 bundle（labor 自动接线曾无条件灌入）。
const JOBS_BUNDLE_MAIN: u32 = 0xD07C_5A04;
const JOBS_BUNDLE_AUX: u32 = 0xF236_BB93;

/// complete-building / 学校 donor 可能带来的学校侧列。剥离时扣除活跃闭环
/// 拥有的列（behavior_loops::owned_fields）：其中 `0AD6A725/0E2660C6/
/// 0C0F2290/0E3F65A9/B3/B4` 分别被观光游客容量、纪念品容量、货运规则、
/// 劳工数量绑定消费，只有在对应闭环不活跃时才算残留。
const SCHOOL_RESIDUE_FIELDS: &[u32] = &[
    0x0AD7_FAC9, // student capacity
    0x0AD6_A725,
    0x0C0F_2290,
    0x0E26_60C6,
    0x0E3F_65A9,
    0x0E3F_65B3,
    0x0E3F_65B4,
    0x0DC1_DB39, // grade distribution
    0x0DC1_DB43,
    0x0DC1_DB45,
    0x0DC1_DB5B,
    0x0DC1_DB5C,
    0x0DC1_DB5D,
];

/// 地标模板唯一性闸门 bundle（slot-0 规则 7D3B8867：HasLandmark / 每城一座）。
/// 闭环嫁接不得带入身份闸门。
const LANDMARK_GATE_BUNDLE: u32 = 0x8A60_B969;

const SCHOOL_RESIDUE_BUNDLES: &[u32] = &[
    EDUCATION_BUNDLE_PRIMARY,
    EDUCATION_BUNDLE_COLLEGE,
    JOBS_BUNDLE_MAIN,
    JOBS_BUNDLE_AUX,
];

/// 前端叶子意图（纯数据，无副作用）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LeafIntent {
    pub education: bool,
    pub tourism: bool,
    pub park: bool,
    pub landvalue: bool,
    pub labor: bool,
    pub residence: bool,
}

/// 已注册的行为家族（可扩展；新复合类型加变体 + 适配器即可）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BehaviorFamily {
    /// 教育闭环（小学/大学叶子）。
    Education,
    /// 公园菜单身份 + 地标观光插件（希望大厦目标形）。
    ParkTourism,
    /// 纯公园。
    Park,
    /// 纯观光/地标（无公园叶子）。
    Tourism,
    /// 未匹配已知组合时的保守回落（公园 donor，避免大学骨架）。
    Generic,
}

/// Menu2 骨架选择（与 Unit donor 正交）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MenuSkeleton {
    /// s31 大学 Menu2（complete-building-menu.bin）——仅教育过渡期可能使用。
    CompleteBuilding,
    /// 通用扁平 Menu2（非大学文案族）；公园/观光默认。
    Generic,
}

/// 岗位自动接线策略。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LaborProfile {
    /// 学校样本：jobs bundle + 学历分布 + 招工常量（教育族）。
    SchoolSample,
    /// 只保留作者 W1/W2/W3/minWorkers/排班，不灌学校样本。
    AuthorOnly,
    /// 无岗位。
    #[default]
    None,
}

/// 适配器输出：构建器各阶段消费的补齐语义。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FamilySemantics {
    pub family: BehaviorFamily,
    pub unit_donor: &'static str,
    pub menu_skeleton: MenuSkeleton,
    pub labor_profile: LaborProfile,
    pub strip_school_residue: bool,
    pub behavior_category: Option<u32>,
    pub simulation_template: Option<u32>,
    /// 与身份正交、由叶子意图激活的闭环（labor 另需岗位 > 0 才落地）。
    pub loops: Vec<BehaviorLoop>,
    pub note: String,
}

/// 从已解析行为节点类型收集意图。
pub fn leaf_intent_from_nodes(nodes: &[serde_json::Value], has_labor: bool) -> LeafIntent {
    let mut intent = LeafIntent {
        labor: has_labor,
        ..LeafIntent::default()
    };
    for node in nodes {
        let kind = node
            .get("type")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        match kind {
            "education" => intent.education = true,
            "tourism" => intent.tourism = true,
            "park" => intent.park = true,
            "landvalue" => intent.landvalue = true,
            "residence" => intent.residence = true,
            _ => {}
        }
    }
    intent
}

/// 解析家族：按优先级匹配适配器（教育 > 公园+观光 > 观光 > 公园 > 通用）。
pub fn resolve(intent: LeafIntent) -> FamilySemantics {
    let mut semantics = if intent.education {
        education_adapter(intent)
    } else if intent.park && intent.tourism {
        park_tourism_adapter(intent)
    } else if intent.tourism {
        tourism_adapter(intent)
    } else if intent.park || intent.landvalue {
        park_adapter(intent)
    } else {
        generic_adapter(intent)
    };
    semantics.loops = active_loops(intent);
    semantics
}

/// 叶子意图 → 闭环集合（与家族/身份无关）。教育族的招工由 SchoolSample
/// 样本接线承载，不再叠加 labor 闭环。
fn active_loops(intent: LeafIntent) -> Vec<BehaviorLoop> {
    let mut loops = Vec::new();
    if intent.labor && !intent.education {
        loops.push(BehaviorLoop::Labor);
    }
    if intent.tourism {
        loops.push(BehaviorLoop::Tourism);
    }
    if intent.park || intent.landvalue || intent.tourism {
        loops.push(BehaviorLoop::LandValue);
    }
    if intent.residence {
        loops.push(BehaviorLoop::Residence);
    }
    loops
}

fn education_adapter(intent: LeafIntent) -> FamilySemantics {
    FamilySemantics {
        family: BehaviorFamily::Education,
        unit_donor: "elementary-school",
        menu_skeleton: MenuSkeleton::Generic,
        labor_profile: if intent.labor {
            LaborProfile::SchoolSample
        } else {
            LaborProfile::None
        },
        strip_school_residue: false,
        behavior_category: Some(EDUCATION_CATEGORY),
        simulation_template: Some(ELEMENTARY_TEMPLATE),
        loops: Vec::new(),
        note: "family=education: elementary-school donor; school labor sample when jobs present"
            .into(),
    }
}

fn park_tourism_adapter(intent: LeafIntent) -> FamilySemantics {
    FamilySemantics {
        family: BehaviorFamily::ParkTourism,
        unit_donor: "generic-park",
        menu_skeleton: MenuSkeleton::Generic,
        labor_profile: if intent.labor {
            LaborProfile::AuthorOnly
        } else {
            LaborProfile::None
        },
        strip_school_residue: true,
        behavior_category: Some(PARK_BEHAVIOR_CATEGORY),
        // 公园身份 = parent-free 公园 donor；观光只作为闭环嫁接，不继承地标模板
        // （地标模板带每城唯一闸门与地标经济，属于身份层）。
        simulation_template: None,
        loops: Vec::new(),
        note: "family=park-tourism: park identity (no parent) + grafted tourism loop; strip school residue"
            .into(),
    }
}

fn tourism_adapter(intent: LeafIntent) -> FamilySemantics {
    FamilySemantics {
        family: BehaviorFamily::Tourism,
        unit_donor: "generic-park",
        menu_skeleton: MenuSkeleton::Generic,
        labor_profile: if intent.labor {
            LaborProfile::AuthorOnly
        } else {
            LaborProfile::None
        },
        strip_school_residue: true,
        behavior_category: Some(PARK_BEHAVIOR_CATEGORY),
        simulation_template: Some(LANDMARK_TEMPLATE),
        loops: Vec::new(),
        note: "family=tourism: park-shaped unit donor + landmark parent; strip school residue"
            .into(),
    }
}

fn park_adapter(intent: LeafIntent) -> FamilySemantics {
    FamilySemantics {
        family: BehaviorFamily::Park,
        unit_donor: "generic-park",
        menu_skeleton: MenuSkeleton::Generic,
        labor_profile: if intent.labor {
            LaborProfile::AuthorOnly
        } else {
            LaborProfile::None
        },
        strip_school_residue: true,
        behavior_category: Some(PARK_BEHAVIOR_CATEGORY),
        simulation_template: None,
        loops: Vec::new(),
        note: "family=park: generic-park donor; strip school residue".into(),
    }
}

fn generic_adapter(intent: LeafIntent) -> FamilySemantics {
    FamilySemantics {
        family: BehaviorFamily::Generic,
        unit_donor: "generic-park",
        menu_skeleton: MenuSkeleton::Generic,
        labor_profile: if intent.labor {
            LaborProfile::AuthorOnly
        } else {
            LaborProfile::None
        },
        strip_school_residue: true,
        behavior_category: None,
        simulation_template: None,
        loops: Vec::new(),
        note: "family=generic: refuse complete-building default; park donor fallback".into(),
    }
}

/// 把家族语义落到 ResolvedBehavior（strip / 类别缺省 / 台账 note）。
pub fn apply_to_behavior(resolved: &mut ResolvedBehavior, semantics: &FamilySemantics) {
    resolved.family = Some(semantics.family);
    resolved.unit_donor = Some(semantics.unit_donor.to_string());
    resolved.menu_skeleton = Some(match semantics.menu_skeleton {
        MenuSkeleton::CompleteBuilding => "complete-building".into(),
        MenuSkeleton::Generic => "generic".into(),
    });
    resolved.labor_profile = semantics.labor_profile;

    let loops = landed_loops(resolved, semantics);
    let owned_fields = behavior_loops::owned_fields(&loops);
    let owned_bundles = behavior_loops::owned_bundles(&loops);

    if semantics.strip_school_residue {
        for bundle in SCHOOL_RESIDUE_BUNDLES {
            if owned_bundles.contains(bundle) {
                continue;
            }
            resolved.ensure_bundles.retain(|candidate| candidate != bundle);
            if !resolved.remove_bundles.contains(bundle) {
                resolved.remove_bundles.push(*bundle);
            }
        }
        for field in SCHOOL_RESIDUE_FIELDS {
            if owned_fields.contains(field) {
                continue;
            }
            resolved.set_float_fields.remove(field);
            resolved.set_int_fields.remove(field);
            if !resolved.remove_fields.contains(field) {
                resolved.remove_fields.push(*field);
            }
        }
    }

    for id in &loops {
        behavior_loops::apply(resolved, *id);
    }

    if semantics.simulation_template != Some(LANDMARK_TEMPLATE) {
        resolved.ensure_bundles.retain(|bundle| *bundle != LANDMARK_GATE_BUNDLE);
    }

    // 作者已写回的列不得再留在 remove_fields：超宽 tourism.removeFields
    // 曾把岗位/排班一并列入，导致 AuthorOnly labor 写了又被剥（菜单乱码）。
    resolved.remove_fields.retain(|field| {
        !resolved.set_float_fields.contains_key(field) && !resolved.set_int_fields.contains_key(field)
    });

    if resolved.behavior_category.is_none() {
        if let Some(category) = semantics.behavior_category {
            resolved.behavior_category = Some(category);
            if !resolved.declared_unit_columns.contains(&0x0AFE_27B1) {
                resolved.declared_unit_columns.push(0x0AFE_27B1);
            }
        }
    }
    // 劳工到岗 / 夜灯 / 观光三者对 Unit 模板的依赖（2026-11-11 逐列对拍定案）：
    // - 夜灯开关（lightswhenactive / interiorsdrivenbyactiveflag）经 parent 模板链
    //   的 non-sim props 继承（FindUnitMasterNonSimProps）。实测：旧版 125ed5c7
    //   parent=7E4B99E5 时夜景亮灯；换到地标模板 4E01E311 后夜景全黑
    //   （Lot 仅差 180° 镜像、模型仅差 80B，其余 183 列完全相同）。
    // - 旅游地标族沿用教育模板既是历史配方也是唯一被实测证明亮灯的形态；
    //   观光 guest 链路另由叶子 bundle 解决（不靠 parent 换血）。
    if loops.contains(&BehaviorLoop::Labor) {
        resolved.behavior_category = Some(EDUCATION_CATEGORY);
        if !resolved.declared_unit_columns.contains(&0x0AFE_27B1) {
            resolved.declared_unit_columns.push(0x0AFE_27B1);
        }
        resolved.parent_template = Some(ELEMENTARY_TEMPLATE);
        if !resolved.declared_unit_columns.contains(&0x00B2_CCCB) {
            resolved.declared_unit_columns.push(0x00B2_CCCB);
        }
        resolved.notes.push(
            "labor loop: category 95A9D6FD + parent 7E4B99E5 (proven lit + hires; park menu kept)"
                .into(),
        );
    }
    if resolved.parent_template.is_none() {
        if let Some(template) = semantics.simulation_template {
            resolved.parent_template = Some(template);
            if !resolved.declared_unit_columns.contains(&0x00B2_CCCB) {
                resolved.declared_unit_columns.push(0x00B2_CCCB);
            }
        }
    }
    resolved.notes.push(semantics.note.clone());
}

/// 实际落地的闭环：labor 只有在作者写了岗位（W1/W2/W3 任一 > 0）时才挂，
/// 否则 jobs bundle 只会让建筑停在「招聘中」。
fn landed_loops(resolved: &ResolvedBehavior, semantics: &FamilySemantics) -> Vec<BehaviorLoop> {
    let has_jobs = [0x0A79_17C5u32, 0x0A79_17C6, 0x0A79_17C7]
        .iter()
        .any(|hash| resolved.set_float_fields.get(hash).is_some_and(|value| *value > 0.0));
    semantics
        .loops
        .iter()
        .copied()
        .filter(|id| *id != BehaviorLoop::Labor || has_jobs)
        .collect()
}

/// 把家族语义写入 MenuRegistrationSpec（作者显式字段优先）。
pub fn enrich_menu(spec: &mut MenuRegistrationSpec, semantics: &FamilySemantics) {
    if spec.unit_donor.is_none() {
        spec.unit_donor = Some(semantics.unit_donor.to_string());
    }
    if spec.menu_skeleton.is_none() {
        spec.menu_skeleton = Some(match semantics.menu_skeleton {
            MenuSkeleton::CompleteBuilding => "complete-building".into(),
            MenuSkeleton::Generic => "generic".into(),
        });
    }
    if spec.behavior_category_instance.is_none() {
        spec.behavior_category_instance = semantics.behavior_category;
    }
    if spec.simulation_template_instance.is_none() {
        spec.simulation_template_instance = semantics.simulation_template;
    }
    if semantics.loops.contains(&BehaviorLoop::Labor) {
        // Preserve the legacy presentation default for recipes with no menu selection.
        if spec.ui_categories().is_none() {
            let mut categories = vec![PARK_BEHAVIOR_CATEGORY];
            for category in [
                semantics.behavior_category,
                Some(EDUCATION_BOARD),
                Some(EDUCATION_CATEGORY),
            ]
            .into_iter()
            .flatten()
            {
                if !categories.contains(&category) {
                    categories.push(category);
                }
            }
            spec.category_instances = categories;
        }
        spec.behavior_category_instance = Some(EDUCATION_CATEGORY);
        spec.simulation_template_instance = Some(ELEMENTARY_TEMPLATE);
    }
    // Building40 公园/观光不再走大学 Menu2；education 也用 generic + 教育统计行覆盖。
    if semantics.menu_skeleton == MenuSkeleton::Generic {
        spec.menu_skeleton = Some("generic".into());
    }
}

/// 从已编译程序回读家族语义（plan 阶段 enrich menu 用）。
pub fn semantics_from_program(program: &ResolvedBehavior) -> Option<FamilySemantics> {
    let family = program.family?;
    let intent = LeafIntent {
        education: matches!(family, BehaviorFamily::Education),
        tourism: matches!(
            family,
            BehaviorFamily::Tourism | BehaviorFamily::ParkTourism
        ),
        park: matches!(family, BehaviorFamily::Park | BehaviorFamily::ParkTourism),
        landvalue: false,
        labor: !matches!(program.labor_profile, LaborProfile::None),
        residence: false,
    };
    Some(resolve(intent))
}

#[cfg(test)]
mod tests {
    use super::*;

    const VISITORS: u32 = 0x0DA3_EE15;
    const SHOPPERS: u32 = 0x0AD6_A725;
    const INCOME_W1: u32 = 0x0EE2_A50D;
    const INCOME_W2: u32 = 0x0EE2_A50E;
    const SOUVENIRS: u32 = 0x0E26_60C6;
    const FREIGHT_RATE: u32 = 0x0C0F_2290;
    const WORKER_AMOUNT: u32 = 0x0E3F_65A9;
    const LAND_VALUE_A: u32 = 0x0E58_5AE0;
    const LAND_WEALTH_B: u32 = 0x0E58_5B01;
    const POPULATION: u32 = 0x0A79_17CB;

    fn park_tourism(labor: bool) -> FamilySemantics {
        resolve(LeafIntent {
            park: true,
            tourism: true,
            labor,
            ..LeafIntent::default()
        })
    }

    #[test]
    fn park_plus_tourism_keeps_park_identity_without_landmark_parent() {
        let semantics = park_tourism(true);
        assert_eq!(semantics.family, BehaviorFamily::ParkTourism);
        assert_eq!(semantics.unit_donor, "generic-park");
        assert_eq!(semantics.menu_skeleton, MenuSkeleton::Generic);
        assert!(semantics.strip_school_residue);
        assert_eq!(semantics.labor_profile, LaborProfile::AuthorOnly);
        assert_eq!(semantics.simulation_template, None);
        assert_eq!(
            semantics.loops,
            vec![BehaviorLoop::Labor, BehaviorLoop::Tourism, BehaviorLoop::LandValue]
        );
    }

    #[test]
    fn labor_preserves_a_parks_only_menu_selection() {
        let mut spec: MenuRegistrationSpec = serde_json::from_value(serde_json::json!({
            "title": "Park with jobs",
            "categoryInstances": ["07049250"]
        }))
        .unwrap();
        enrich_menu(&mut spec, &park_tourism(true));
        assert_eq!(spec.category_instances, vec![PARK_BEHAVIOR_CATEGORY]);
        assert_eq!(spec.behavior_category_instance, Some(EDUCATION_CATEGORY));
        assert_eq!(spec.simulation_template_instance, Some(ELEMENTARY_TEMPLATE));
    }

    #[test]
    fn education_keeps_school_donor_and_sample_labor() {
        let semantics = resolve(LeafIntent {
            education: true,
            labor: true,
            ..LeafIntent::default()
        });
        assert_eq!(semantics.family, BehaviorFamily::Education);
        assert_eq!(semantics.unit_donor, "elementary-school");
        assert!(!semantics.strip_school_residue);
        assert_eq!(semantics.labor_profile, LaborProfile::SchoolSample);
        assert!(!semantics.loops.contains(&BehaviorLoop::Labor));
    }

    #[test]
    fn empty_intent_refuses_complete_building_default() {
        let semantics = resolve(LeafIntent::default());
        assert_eq!(semantics.family, BehaviorFamily::Generic);
        assert_eq!(semantics.unit_donor, "generic-park");
        assert!(semantics.loops.is_empty());
    }

    #[test]
    fn apply_strips_school_bundles_for_park_tourism_without_labor() {
        let mut resolved = ResolvedBehavior::default();
        resolved.ensure_bundles.push(EDUCATION_BUNDLE_COLLEGE);
        resolved.ensure_bundles.push(JOBS_BUNDLE_MAIN);
        apply_to_behavior(&mut resolved, &park_tourism(false));
        assert!(resolved.remove_bundles.contains(&EDUCATION_BUNDLE_COLLEGE));
        assert!(resolved.remove_bundles.contains(&JOBS_BUNDLE_MAIN));
        assert!(!resolved.ensure_bundles.contains(&EDUCATION_BUNDLE_COLLEGE));
        assert_eq!(resolved.set_float_fields.get(&VISITORS), Some(&51.0));
        assert_eq!(resolved.parent_template, None);
        // 无岗位 → 不解封经济 bundle、不挂劳工数量列。
        assert!(!resolved.remove_excluded_bundles.contains(&0x3064_5625));
        assert!(resolved.remove_fields.contains(&WORKER_AMOUNT));
    }

    #[test]
    fn labor_loop_lands_only_with_authored_jobs() {
        let mut idle = ResolvedBehavior::default();
        apply_to_behavior(&mut idle, &park_tourism(true));
        assert!(!idle.ensure_bundles.contains(&JOBS_BUNDLE_MAIN));

        let mut hiring = ResolvedBehavior::default();
        hiring.set_float_fields.insert(0x0A79_17C5, 30.0);
        apply_to_behavior(&mut hiring, &park_tourism(true));
        assert!(hiring.ensure_bundles.contains(&JOBS_BUNDLE_MAIN));
        assert!(hiring.ensure_bundles.contains(&JOBS_BUNDLE_AUX));
        assert!(hiring.remove_excluded_bundles.contains(&0x3064_5625));
        assert!(hiring.ensure_internal_path_ref);
        assert_eq!(hiring.set_float_fields.get(&WORKER_AMOUNT), Some(&10.0));
        assert!(!hiring.remove_fields.contains(&WORKER_AMOUNT));
        // 公园身份菜单保留；Unit 行为类别切到可招工的教育类别。
        // 2026-11-11 地标普查：旅游地标族回到地标模板（同时带游客与劳工 bin），
        // 不再嫁接教育模板/类别。
        // 实测：parent=7E4B99E5（教育）时夜景亮灯+可招工；地标模板会熄灯。
        assert_eq!(hiring.behavior_category, Some(EDUCATION_CATEGORY));
        assert_eq!(hiring.parent_template, Some(ELEMENTARY_TEMPLATE));
        assert_eq!(idle.behavior_category, Some(PARK_BEHAVIOR_CATEGORY));
        assert_eq!(idle.parent_template, None);
    }

    #[test]
    fn tourism_loop_owned_columns_survive_school_residue_strip() {
        let mut resolved = ResolvedBehavior::default();
        resolved.remove_fields.push(SHOPPERS);
        resolved.remove_fields.push(INCOME_W2);
        apply_to_behavior(
            &mut resolved,
            &resolve(LeafIntent {
                tourism: true,
                ..LeafIntent::default()
            }),
        );
        for (hash, value) in [
            (SHOPPERS, 390.0),
            (INCOME_W1, 11.0),
            (INCOME_W2, 11.0),
            (SOUVENIRS, 3300.0),
            (FREIGHT_RATE, 48.0),
        ] {
            assert_eq!(resolved.set_float_fields.get(&hash), Some(&value), "{hash:08X}");
            assert!(!resolved.remove_fields.contains(&hash), "{hash:08X}");
            assert!(resolved.declared_unit_columns.contains(&hash), "{hash:08X}");
        }
        for bundle in [0xCF11_6B9C, 0x2B5A_2BAF, 0x3620_87BD] {
            assert!(resolved.ensure_bundles.contains(&bundle));
        }
    }

    #[test]
    fn labor_wired_education_distribution_survives_school_residue_strip() {
        // 希望大厦 /48 回归（2026-11-11）：auto-wiring 注入的学历分布发生在
        // strip 之前，LABOR 闭环未收编 0DC1DB* 时"剥了没人补"→ 全链缺失、
        // 永远「需要劳工」。收编后必须完整存活。
        let mut resolved = ResolvedBehavior::default();
        resolved.labor_profile = LaborProfile::AuthorOnly;
        resolved.set_float_fields.insert(0x0A79_17C5, 90.0); // JobsW1
        crate::asset_build_m1::behavior_nodes::apply_labor_auto_wiring(&mut resolved);
        apply_to_behavior(&mut resolved, &park_tourism(true));
        for (hash, value) in [
            (0x0DC1_DB39, 45.0),
            (0x0DC1_DB43, 0.0),
            (0x0DC1_DB45, 0.0),
            (0x0DC1_DB5B, 45.0),
            (0x0DC1_DB5C, 0.0),
            (0x0DC1_DB5D, 0.0),
        ] {
            assert_eq!(
                resolved.set_float_fields.get(&hash),
                Some(&value),
                "{hash:08X}"
            );
            assert!(!resolved.remove_fields.contains(&hash), "{hash:08X}");
            assert!(resolved.declared_unit_columns.contains(&hash), "{hash:08X}");
        }
    }

    #[test]
    fn tourism_no_longer_implies_residence_or_landmark_gate() {
        let mut resolved = ResolvedBehavior::default();
        resolved.ensure_bundles.push(LANDMARK_GATE_BUNDLE);
        apply_to_behavior(&mut resolved, &park_tourism(false));
        assert!(!resolved.ensure_bundles.contains(&LANDMARK_GATE_BUNDLE));
        assert!(!resolved.ensure_bundles.contains(&0x5D5E_F1FF));
        assert!(!resolved.set_float_fields.contains_key(&POPULATION));
    }

    #[test]
    fn residence_leaf_grafts_copan_stack() {
        let mut resolved = ResolvedBehavior::default();
        apply_to_behavior(
            &mut resolved,
            &resolve(LeafIntent {
                park: true,
                residence: true,
                ..LeafIntent::default()
            }),
        );
        assert!(resolved.ensure_bundles.contains(&0x5D5E_F1FF));
        assert!(resolved.ensure_bundles.contains(&0xEE66_978E));
        assert_eq!(resolved.set_float_fields.get(&POPULATION), Some(&1100.0));
    }

    #[test]
    fn landvalue_pairs_fill_for_park_tourism() {
        let mut resolved = ResolvedBehavior::default();
        apply_to_behavior(&mut resolved, &park_tourism(false));
        assert_eq!(resolved.set_float_fields.get(&LAND_VALUE_A), Some(&500.0));
        assert_eq!(resolved.set_float_fields.get(&LAND_WEALTH_B), Some(&2.0));
    }

    #[test]
    fn authored_fields_are_exempted_from_remove_list() {
        let mut resolved = ResolvedBehavior::default();
        resolved.set_float_fields.insert(0x0A79_17C5, 30.0);
        resolved.set_int_fields.insert(0x0D00_853A, 6);
        resolved.remove_fields.push(0x0A79_17C5);
        resolved.remove_fields.push(0x0D00_853A);
        resolved.remove_fields.push(0x0AD7_FAC9);
        let semantics = resolve(LeafIntent {
            tourism: true,
            labor: true,
            ..LeafIntent::default()
        });
        apply_to_behavior(&mut resolved, &semantics);
        assert!(!resolved.remove_fields.contains(&0x0A79_17C5));
        assert!(!resolved.remove_fields.contains(&0x0D00_853A));
        assert!(resolved.remove_fields.contains(&0x0AD7_FAC9));
        assert_eq!(resolved.set_int_fields.get(&0x0D00_853A), Some(&6));
    }

    #[test]
    fn authored_tourism_params_override_baselines() {
        let mut resolved = ResolvedBehavior::default();
        resolved.set_float_fields.insert(INCOME_W1, 9.0);
        resolved.set_float_fields.insert(SHOPPERS, 250.0);
        apply_to_behavior(
            &mut resolved,
            &resolve(LeafIntent {
                tourism: true,
                ..LeafIntent::default()
            }),
        );
        assert_eq!(resolved.set_float_fields.get(&INCOME_W1), Some(&9.0));
        assert_eq!(resolved.set_float_fields.get(&SHOPPERS), Some(&250.0));
        assert_eq!(resolved.set_float_fields.get(&INCOME_W2), Some(&11.0));
    }
}
