//! 行为家族适配器：前端简单叶子 IR → 后端补齐游戏可识别语义。
//!
//! 工作台只表达意图（park / tourism / education / landvalue / labor）。
//! 本模块按叶子组合选择家族适配器，补齐 Unit donor、Menu2 骨架、学校残留
//! 剥离、岗位接线策略等——未来加「可住地标」等复合类型时只注册新适配器，
//! 不改前端配置面。

use super::behavior_nodes::ResolvedBehavior;
use super::menu::{ELEMENTARY_TEMPLATE, MenuRegistrationSpec};
use serde::Serialize;

/// 公园行为类别（菜单容器与 Unit 模拟类别在公园族上重合）。
const PARK_BEHAVIOR_CATEGORY: u32 = 0x0704_9250;
/// 地标 Unit 父模板（文化>地标实证）。
const LANDMARK_TEMPLATE: u32 = 0x4E01_E311;
/// 教育行为类别。
const EDUCATION_CATEGORY: u32 = 0x95A9_D6FD;

/// 小学 / 社区大学教育签名 bundle（complete-building = s31 大学叶）。
const EDUCATION_BUNDLE_PRIMARY: u32 = 0x6C04_2367;
const EDUCATION_BUNDLE_COLLEGE: u32 = 0x6C04_2366;
/// 学校样本岗位 bundle（labor 自动接线曾无条件灌入）。
const JOBS_BUNDLE_MAIN: u32 = 0xD07C_5A04;
const JOBS_BUNDLE_AUX: u32 = 0xF236_BB93;

/// 从 complete-building / 学校 donor 剥掉的**学校特有**字段。
/// 不含作者岗位 W1–W3/合计/排班——那些由 LaborProfile 决定是否保留。
/// 学校特有字段。不含 `0x0DA3EE15`：该列在观光菜单统计行是
/// 「Visitors per day」参数（menu-contract 强制要求），公园/观光族应
/// 写成旅客量而不是剥离后留下模板占位符。
const SCHOOL_RESIDUE_FIELDS: &[u32] = &[
    0x0AD7_FAC9, // student capacity
    0x0AD6_A725, // school hire constant
    0x0C0F_2290,
    0x0E26_60C6,
    0x0E3F_65A9,
    0x0E3F_65B4,
    0x0DC1_DB39, // grade distribution
    0x0DC1_DB43,
    0x0DC1_DB45,
    0x0DC1_DB5B,
    0x0DC1_DB5C,
    0x0DC1_DB5D,
];

/// 观光悬浮卡「Visitors per day」实证默认值（a42cf63e / 86e99f6e）。
const TOURISM_VISITORS_PER_DAY: f64 = 51.0;
const TOURISM_VISITORS_FIELD: u32 = 0x0DA3_EE15;

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
            _ => {}
        }
    }
    intent
}

/// 解析家族：按优先级匹配适配器（教育 > 公园+观光 > 观光 > 公园 > 通用）。
pub fn resolve(intent: LeafIntent) -> FamilySemantics {
    if intent.education {
        return education_adapter(intent);
    }
    if intent.park && intent.tourism {
        return park_tourism_adapter(intent);
    }
    if intent.tourism {
        return tourism_adapter(intent);
    }
    if intent.park || intent.landvalue {
        return park_adapter(intent);
    }
    generic_adapter(intent)
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
        // parent 模板仍由 tourism 叶子写入 ResolvedBehavior；此处给 menu 侧缺省。
        simulation_template: Some(LANDMARK_TEMPLATE),
        note: "family=park-tourism: park donor + landmark template; strip school residue; author-only labor"
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

    if semantics.strip_school_residue {
        for bundle in SCHOOL_RESIDUE_BUNDLES {
            resolved.ensure_bundles.retain(|candidate| candidate != bundle);
            if !resolved.remove_bundles.contains(bundle) {
                resolved.remove_bundles.push(*bundle);
            }
        }
        for field in SCHOOL_RESIDUE_FIELDS {
            resolved.set_float_fields.remove(field);
            resolved.set_int_fields.remove(field);
            if !resolved.remove_fields.contains(field) {
                resolved.remove_fields.push(*field);
            }
        }
    }

    // 观光/公园+观光：补齐菜单统计行参数，避免 strip 学校样本后留下占位符。
    if matches!(
        semantics.family,
        BehaviorFamily::ParkTourism | BehaviorFamily::Tourism
    ) {
        resolved.remove_fields.retain(|field| *field != TOURISM_VISITORS_FIELD);
        if !resolved.set_float_fields.contains_key(&TOURISM_VISITORS_FIELD) {
            resolved
                .set_float_fields
                .insert(TOURISM_VISITORS_FIELD, TOURISM_VISITORS_PER_DAY);
        }
        if !resolved.declared_unit_columns.contains(&TOURISM_VISITORS_FIELD) {
            resolved.declared_unit_columns.push(TOURISM_VISITORS_FIELD);
        }
    }

    if resolved.behavior_category.is_none() {
        if let Some(category) = semantics.behavior_category {
            resolved.behavior_category = Some(category);
            if !resolved.declared_unit_columns.contains(&0x0AFE_27B1) {
                resolved.declared_unit_columns.push(0x0AFE_27B1);
            }
        }
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
    };
    Some(resolve(intent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn park_plus_tourism_selects_park_tourism_family() {
        let semantics = resolve(LeafIntent {
            park: true,
            tourism: true,
            labor: true,
            ..LeafIntent::default()
        });
        assert_eq!(semantics.family, BehaviorFamily::ParkTourism);
        assert_eq!(semantics.unit_donor, "generic-park");
        assert_eq!(semantics.menu_skeleton, MenuSkeleton::Generic);
        assert!(semantics.strip_school_residue);
        assert_eq!(semantics.labor_profile, LaborProfile::AuthorOnly);
        assert_eq!(semantics.simulation_template, Some(LANDMARK_TEMPLATE));
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
    }

    #[test]
    fn empty_intent_refuses_complete_building_default() {
        let semantics = resolve(LeafIntent::default());
        assert_eq!(semantics.family, BehaviorFamily::Generic);
        assert_eq!(semantics.unit_donor, "generic-park");
        assert_ne!(semantics.unit_donor, "complete-building");
    }

    #[test]
    fn apply_strips_school_bundles_for_park_tourism() {
        let mut resolved = ResolvedBehavior::default();
        resolved.ensure_bundles.push(EDUCATION_BUNDLE_COLLEGE);
        resolved.ensure_bundles.push(JOBS_BUNDLE_MAIN);
        resolved
            .set_float_fields
            .insert(0x0DA3_EE15, 51.0);
        let semantics = resolve(LeafIntent {
            park: true,
            tourism: true,
            labor: false,
            ..LeafIntent::default()
        });
        apply_to_behavior(&mut resolved, &semantics);
        assert!(resolved.remove_bundles.contains(&EDUCATION_BUNDLE_COLLEGE));
        assert!(resolved.remove_bundles.contains(&JOBS_BUNDLE_MAIN));
        assert!(!resolved.ensure_bundles.contains(&EDUCATION_BUNDLE_COLLEGE));
        assert!(!resolved.remove_fields.contains(&TOURISM_VISITORS_FIELD));
        assert_eq!(
            resolved.set_float_fields.get(&TOURISM_VISITORS_FIELD),
            Some(&TOURISM_VISITORS_PER_DAY)
        );
        assert_eq!(resolved.unit_donor.as_deref(), Some("generic-park"));
    }
}
