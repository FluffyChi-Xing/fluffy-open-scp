//! 菜单/渲染契约闸门（2026-11-07，s25 事故后定案）。
//!
//! 症状 → 根因层 → 构建期拦截。四类历史事故：
//!
//! | 症状 | 根因 | 本模块检查 |
//! |---|---|---|
//! | 菜单条目出现但放置只有锚点 | 建筑 Asset 挂在公园容器（07049250），公园工具只建公园锚点；或 Asset 缺模型引用；或合成了 donor 没有的自映射 0CA5CF4A | 放置家族、模型引用、自映射 |
//! | 菜单/面板标题 ***/乱码 | Menu2 的 Text 指向的 locale 表/行不存在或为空 | locale 行解析 |
//! | 菜单不加载/条目消失 | Menu2 无 menuUnit、Unit→Asset 断链、类别为空、资源缺失 | 身份链与类别 |
//! | lot/模型丢失 | Asset 无 BoundingBox/模型/LotMask；整 lot 重写丢列（由 delta-audit 负责） | 渲染字段 |
//!
//! 只读纯函数：输入资源池 + 身份实例，输出契约报告或 fail-closed 错误。
//! 在 behavior-bind 之后、delta-audit 之前执行——任何一类问题都在 staging
//! 之前拒绝，上一次成功产物不被覆盖。

use super::connector::Resources;
use dbpf::ResourceId;
use sc_properties::{Kind, PropType, PropertyFile, Value};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MENU_GROUP: u32 = 0x0987_8A01;
const UNIT_GROUP: u32 = 0x40E0_C000;
const LOCALE_TYPE: u32 = 0x0A98_EAF0;
const LOCALE_GROUP: u32 = 0x02FA_BF01;
const MODEL_TYPE: u32 = 0x2F4E_681B;
const RASTER_TYPE: u32 = 0x2F4E_681C;

const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_UNIT_ASSET_LINK: u32 = 0x08E1_7ED7;
const HASH_UNIT_CATEGORY: u32 = 0x0AFE_27B1;
const HASH_CONSTRUCTION_COST: u32 = 0x0A8A_41DA;
const HASH_MAINTENANCE_COST: u32 = 0x09AE_19D7;
/// 遗留共享 locale 表（menu.rs LOCALE_TABLE；已部署旧包如 Commercial_v7
/// 占用）。新构建一律不得再写入。
const LEGACY_SHARED_LOCALE_TABLE: u32 = 0xBF82_F7CE;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_BOUNDING_BOX: u32 = 0x00F9_EFBA;
const HASH_LOT_MASK: u32 = 0x0CCB_7FD5;
const HASH_SELF_MAP: u32 = 0x0CA5_CF4A;
const MODEL_REF_HASHES: [u32; 5] = [
    0x00F9_EFBB,
    0x00F9_EFBC,
    0x00F9_EFBD,
    0x00F9_EFBE,
    0x0D00_82F1,
];
const PARK_CATEGORY: u32 = 0x0704_9250;

/// 身份/donor 的 lot 组（Asset 字典）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuContractReport {
    pub instance: u32,
    pub categories: Vec<u32>,
    pub model_refs: usize,
    pub checks: Vec<String>,
}

fn parse_at(resources: &Resources, group: u32, instance: u32) -> Option<PropertyFile> {
    let bytes = resources.get(&ResourceId {
        type_id: PROPERTY_TYPE,
        group,
        instance,
    })?;
    PropertyFile::parse(bytes).ok()
}

fn scalar_key(file: &PropertyFile, hash: u32) -> Option<sc_properties::Key> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Key(key))) => Some(*key),
        _ => None,
    }
}

fn scalar_text(file: &PropertyFile, hash: u32) -> Option<sc_properties::Text> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Text(text))) => Some(*text),
        // 原版/生成器两种编码都存在：单值或单元素 Text 数组。
        Some(Kind::Array(values)) => values.iter().find_map(|value| match value {
            Value::Text(text) => Some(*text),
            _ => None,
        }),
        _ => None,
    }
}

/// BoundingBox 在两种编码里出现：单值或单元素数组。
fn has_bounding_box(file: &PropertyFile, hash: u32) -> bool {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::BoundingBox { .. })) => true,
        Some(Kind::Array(values)) => values
            .iter()
            .any(|value| matches!(value, Value::BoundingBox { .. })),
        _ => false,
    }
}

fn array_keys(file: &PropertyFile, hash: u32) -> Vec<sc_properties::Key> {
    match file.get(hash).map(|property| &property.kind) {
        Some(Kind::Array(values)) => values
            .iter()
            .filter_map(|value| match value {
                Value::Key(key) => Some(*key),
                _ => None,
            })
            .collect(),
        Some(Kind::Scalar(Value::Key(key))) => vec![*key],
        _ => Vec::new(),
    }
}

fn has_resource(resources: &Resources, type_id: u32, instance: u32) -> bool {
    resources
        .keys()
        .any(|id| id.type_id == type_id && id.instance == instance)
}

/// locale 行查询：`Ok(None)` = 表不在包内（基础游戏全局表，跳过内容校验）；
/// `Ok(Some(text))` = 行存在；`Err` = 表在包内但格式坏/行缺失/为空。
fn locale_row(resources: &Resources, table_id: u32, row: u32) -> Result<Option<String>, String> {
    let Some(bytes) = resources.get(&ResourceId {
        type_id: LOCALE_TYPE,
        group: LOCALE_GROUP,
        instance: table_id,
    }) else {
        return Ok(None);
    };
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("locale table {table_id:08X} is not valid JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| format!("locale table {table_id:08X} is not a JSON object"))?;
    let text = object
        .get(&format!("0x{row:08x}"))
        .or_else(|| object.get(&format!("0x{row:x}")))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "locale table {table_id:08X} has no row 0x{row:08X} (title would render as ***)"
            )
        })?;
    if text.trim().is_empty() {
        return Err(format!(
            "locale table {table_id:08X} row 0x{row:08X} is empty (title would render as ***)"
        ));
    }
    Ok(Some(text.to_string()))
}

/// 渲染/菜单契约校验。`donor` = identity donor 池与实例（菜单新身份时提供）。
pub fn validate(
    resources: &Resources,
    instance: u32,
    lot_group: u32,
    donor: Option<(&Resources, u32)>,
    strict: bool,
) -> Result<MenuContractReport, String> {
    validate_profile(resources, instance, lot_group, donor, strict, false)
}

pub fn validate_building40(
    resources: &Resources,
    instance: u32,
    lot_group: u32,
    donor: Option<(&Resources, u32)>,
    strict: bool,
) -> Result<MenuContractReport, String> {
    validate_profile(resources, instance, lot_group, donor, strict, true)
}

fn validate_profile(
    resources: &Resources,
    instance: u32,
    lot_group: u32,
    donor: Option<(&Resources, u32)>,
    strict: bool,
    building40: bool,
) -> Result<MenuContractReport, String> {
    let Some(menu) = parse_at(resources, MENU_GROUP, instance) else {
        return Ok(MenuContractReport {
            instance,
            categories: Vec::new(),
            model_refs: 0,
            checks: vec!["no Menu2 in output (not a menu asset)".into()],
        });
    };
    let unit = parse_at(resources, UNIT_GROUP, instance)
        .ok_or_else(|| format!("menu identity {instance:08X}: Unit resource is missing"))?;
    let asset = parse_at(resources, lot_group, instance);

    let mut checks = Vec::new();

    // 1) 菜单类别 + menuUnit。frozen/legacy 身份可能只有最小 Menu2（无
    //    menuUnit），此时不是一条可点击的菜单项，跳过菜单层检查。
    let categories: Vec<u32> = array_keys(&menu, HASH_UI_CATEGORY)
        .into_iter()
        .map(|key| key.instance)
        .collect();
    let menu_unit = scalar_key(&menu, HASH_MENU_UNIT);
    if let Some(key) = menu_unit {
        if key.instance != instance {
            return Err(format!(
                "menu {instance:08X}: menuUnit points at 0x{:08X}, not the identity — the game cannot spawn this entry",
                key.instance
            ));
        }
        if categories.is_empty() {
            return Err(format!(
                "menu {instance:08X}: uiCategories is empty — the entry will not appear in any menu panel"
            ));
        }
        checks.push("menuUnit→Unit".into());
    } else {
        checks.push("menu-level skipped (no menuUnit; frozen/minimal identity)".into());
    }

    // 2) Unit→Asset 链。严格模式（新菜单身份）缺链即失败；frozen 身份只警告
    //    （legacy Dalian 约定可能指向 Unit 组）。
    match scalar_key(&unit, HASH_UNIT_ASSET_LINK) {
        Some(key) if key.instance != instance => {
            if strict {
                return Err(format!(
                    "unit {instance:08X}: 08E17ED7 points at 0x{:08X} instead of the identity — lot/model cannot resolve",
                    key.instance
                ));
            }
            checks.push("warning: Unit→Asset link points elsewhere (legacy)".into());
        }
        Some(key) => {
            if key.type_id == PROPERTY_TYPE && key.group == lot_group && asset.is_some() {
                checks.push("Unit→Asset".into());
            } else if asset.is_some() {
                checks.push("warning: legacy Unit→Asset link (group/type)".into());
            } else if strict {
                return Err(format!(
                    "unit {instance:08X}: Asset resource {PROPERTY_TYPE:08X}:{lot_group:08X}:{instance:08X} \
                     is missing — lot/model cannot resolve"
                ));
            } else {
                checks.push("warning: Asset resource missing (legacy identity)".into());
            }
        }
        None if strict => {
            return Err(format!(
                "unit {instance:08X}: 08E17ED7 (Unit→Asset) is missing — the building would render as a marker only"
            ));
        }
        None => checks.push("warning: 08E17ED7 missing (legacy identity)".into()),
    }

    // 3) 渲染字段（Asset 存在时）：BoundingBox / 模型引用 / LotMask。
    let mut model_refs = 0usize;
    if let Some(asset) = &asset {
        let has_bbox = has_bounding_box(asset, HASH_BOUNDING_BOX);
        if has_bbox {
            checks.push("BoundingBox".into());
        } else {
            if strict {
                return Err(format!(
                    "asset {instance:08X}: 00F9EFBA BoundingBox is missing — strict menu identities need a concrete lot bound for placement and rendering"
                ));
            }
            checks.push("warning: 00F9EFBA BoundingBox missing (lot bounds)".into());
        }
        for hash in MODEL_REF_HASHES {
            if let Some(key) = scalar_key(asset, hash) {
                if has_resource(resources, MODEL_TYPE, key.instance) {
                    model_refs += 1;
                }
            }
        }
        // 只有"本次构建确实编译了模型"时，缺模型引用才是致命错误；纯属性
        // 夹具/无模型构建（单测、行为验证）降级为告警，不误杀。
        let any_model = resources.keys().any(|id| id.type_id == MODEL_TYPE);
        if model_refs == 0 {
            if strict && has_bbox && any_model {
                return Err(format!(
                    "asset {instance:08X}: no resolvable RW4 model reference (00F9EFBB..BE / 0D0082F1) — \
                     the building would render as a placement marker only"
                ));
            }
            checks.push("warning: no resolvable model refs (legacy/frozen asset)".into());
        }
        checks.push(format!("modelRefs={model_refs}"));
        match scalar_key(asset, HASH_LOT_MASK) {
            Some(key) if has_resource(resources, RASTER_TYPE, key.instance) => {
                checks.push("LotMask".into())
            }
            Some(key) => {
                if strict {
                    return Err(format!(
                        "asset {instance:08X}: LotMask raster 0x{:08X} is not resolvable — strict menu identities need a concrete ground mask",
                        key.instance
                    ));
                }
                checks.push(format!(
                    "warning: LotMask raster 0x{:08X} not in package (ground/lot resolution)",
                    key.instance
                ));
            }
            None => {
                if strict {
                    return Err(format!(
                        "asset {instance:08X}: 0CCB7FD5 LotMask is missing — strict menu identities need a ground mask"
                    ));
                }
                checks.push("warning: 0CCB7FD5 LotMask missing".into());
            }
        }
    } else {
        checks.push("warning: Asset resource missing (render fields unchecked)".into());
    }

    // 4) 菜单标题（仅真实菜单项）：Text → locale 表/行必须可解析；
    //    包内表缺行/空串 = *** 占位。
    if menu_unit.is_some() {
        let title = scalar_text(&menu, HASH_TITLE).ok_or_else(|| {
            format!(
                "menu {instance:08X}: title Text 0x0A09F5FA is missing — the entry renders as ***"
            )
        })?;
        if let Some(text) = locale_row(resources, title.table_id, title.instance_id)? {
            checks.push(format!("title={text}"));
        } else {
            checks.push("title=external-locale".into());
        }
        if let Some(desc) = scalar_text(&menu, HASH_DESC) {
            if locale_row(resources, desc.table_id, desc.instance_id)?.is_none() {
                checks.push("desc=external-locale".into());
            }
        }

        // 5) 放置家族：公园容器 + 非公园 Unit = 公园工具只会放锚点。
        if categories.contains(&PARK_CATEGORY) {
            match scalar_key(&unit, HASH_UNIT_CATEGORY).map(|key| key.instance) {
                Some(value) if value != PARK_CATEGORY => {
                    // 公园入口 + 教育行为是刻意验证的"菜单/行为解耦"配方（s24
                    // 实证可放置）；这里只提示，不拦截。
                    checks.push(format!(
                        "warning: park panel entry with non-park Unit category 0x{value:08X} (decoupled menu/behavior recipe; verify in-game)"
                    ));
                }
                Some(_) => {}
                None if strict => {
                    return Err(format!(
                        "unit {instance:08X}: behavior category is missing — menu identity has no placement family"
                    ));
                }
                None => checks.push(
                    "warning: Unit behavior category missing; park placement unverified".into(),
                ),
            }
        }
        checks.push("placement-family".into());
    }

    // 6) 不合成 donor 没有的自映射字段（0CA5CF4A）。
    if let Some((donor_resources, donor_instance)) = donor {
        if let Some(donor_asset) = parse_at(donor_resources, lot_group, donor_instance) {
            if let Some(asset) = &asset {
                let donor_has = donor_asset.get(HASH_SELF_MAP).is_some();
                let built_has = asset.get(HASH_SELF_MAP).is_some();
                if !donor_has && built_has {
                    return Err(format!(
                        "asset {instance:08X}: 0CA5CF4A self-map was synthesized although the donor \
                         ({donor_instance:08X}) has none — invented self-map fields change the runtime \
                         inheritance graph and leave only a placement marker visible"
                    ));
                }
            }
        }
    }

    // 7) 成本类型：原版 SCUnit 的建造/维护成本全部 Float32。Int32 位型会被
    //    游戏按 f32 读出 ~0 —— 建造面板显示「免费」（2026-11-08 HopeTower
    //    事故：schema 50000 写成 i32，实机 免费+维护费正常 的双面症状）。
    for (hash, label) in [
        (HASH_CONSTRUCTION_COST, "construction cost"),
        (HASH_MAINTENANCE_COST, "maintenance cost"),
    ] {
        if let Some(property) = unit.get(hash) {
            if property.prop_type != PropType::Float {
                return Err(format!(
                    "unit {instance:08X}: {label} 0x{hash:08X} is encoded as {:?} — \
                     the game reads SCUnit costs as Float32; an Int32 bit pattern renders as ~0 \
                     (placement shows the building as free)",
                    property.prop_type
                ));
            }
            checks.push(format!("{label}=f32"));
        }
    }

    // 8) 遗留共享 locale 表：所有构建路由已改写身份自有表（19a1b3a / 本次
    //    收口）。标题/描述仍指向 0xBF82F7CE 意味着回归到了共享表路线——
    //    任何两个这样的包会同 TGI 互踩（v7 "OpenSCP Commercial Tower" 遮蔽
    //    大连希望大厦的 2026-11-08 事故）。donor 覆写原版真表（如 Dalian
    //    284C6727）不受影响。
    if menu_unit.is_some() {
        if let Some(title) = scalar_text(&menu, HASH_TITLE) {
            if title.table_id == LEGACY_SHARED_LOCALE_TABLE {
                return Err(format!(
                    "menu {instance:08X}: title Text targets the legacy shared locale table \
                     0xBF82F7CE — every package built on the shared table shadows every other \
                     one's title/description; write the per-identity table instead"
                ));
            }
        }
    }

    if building40 {
        checks.extend(validate_complete_building(&menu, &unit, instance)?);
    }

    Ok(MenuContractReport {
        instance,
        categories,
        model_refs,
        checks,
    })
}

fn validate_complete_building(
    menu: &PropertyFile,
    unit: &PropertyFile,
    instance: u32,
) -> Result<Vec<String>, String> {
    // These are presentation fields present in both the s31 GOLDEN menu and
    // the complete reference candidate, but absent from Hope's 22-field leaf.
    for hash in [
        0x08F6_72C7u32,
        0x09E6_83AC,
        0x09E6_83AD,
        0x09E9_1B9C,
        0x0C7F_4336,
    ] {
        if menu.get(hash).is_none() {
            return Err(format!(
                "Building40 menu {instance:08X}: required panel field {hash:08X} is missing; the minimal Menu2 leaf is forbidden"
            ));
        }
    }
    if menu.values.len() < 40 {
        return Err(format!(
            "Building40 menu {instance:08X}: only {} property fields; the complete panel donor has 48",
            menu.values.len()
        ));
    }
    // The stock elementary-school leaf is intentionally short and obtains
    // common utility bundles from the game's typed Unit template chain.
    let native_school = scalar_key(unit, 0x00B2_CCCB).is_some_and(|parent| {
        parent.type_id == PROPERTY_TYPE
            && parent.group == super::menu::UNIT_TEMPLATE_GROUP
            && parent.instance == super::menu::ELEMENTARY_TEMPLATE
    }) && scalar_key(unit, HASH_UNIT_CATEGORY).is_some_and(|category| {
        category.instance == super::menu::EDUCATION_BEHAVIOR_CATEGORY
    }) && matches!(
        unit.get(super::menu::HASH_STUDENT_CAPACITY).and_then(|property| property.scalar()),
        Some(Value::Float(capacity)) if *capacity > 0.0
    );
    if unit.values.len() < 150 && !native_school {
        return Err(format!(
            "Building40 unit {instance:08X}: only {} property fields; the complete simulation/utility donor is required",
            unit.values.len()
        ));
    }

    let texts = match menu.get(0x0EB1_FC05).map(|property| &property.kind) {
        Some(Kind::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::Text(text) => Ok(*text),
                _ => Err(format!(
                    "Building40 menu {instance:08X}: 0EB1FC05 contains a non-Text statistic row"
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(format!(
                "Building40 menu {instance:08X}: 0EB1FC05 statistic Text array is missing"
            ))
        }
    };
    let params = match menu.get(0x0EB1_FC22).map(|property| &property.kind) {
        Some(Kind::Array(values)) => values
            .iter()
            .map(|value| match value {
                Value::UInt32(hash) => Ok(*hash),
                _ => Err(format!(
                    "Building40 menu {instance:08X}: 0EB1FC22 contains a non-UInt32 parameter row"
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(format!(
                "Building40 menu {instance:08X}: 0EB1FC22 statistic parameter array is missing"
            ))
        }
    };
    if texts.is_empty() || texts.len() != params.len() {
        return Err(format!(
            "Building40 menu {instance:08X}: statistic text/parameter rows are empty or misaligned ({} vs {})",
            texts.len(),
            params.len()
        ));
    }
    // 2026-11-08/09 菜单乱码：公园 donor 的 2 行统计（含需参数的 0EBCE055）
    // 缺参时原样打印 ~population_amount:number~。Building40 只接受观光 1 行
    // 或 s31 GOLDEN 教育 3 行，禁止公园 2 行错位。
    if texts.len() == 2 {
        return Err(format!(
            "Building40 menu {instance:08X}: park-style 2-row statistic panel is forbidden \
             (causes ~population_amount~ / Title/Left1 placeholders); use tourism 1-row or s31 3-row text↔param"
        ));
    }
    if texts.len() == 1 {
        if texts[0].table_id != 0x50AA_0BEA
            || texts[0].instance_id != 0x0EBC_E29A
            || params[0] != 0x0DA3_EE15
        {
            return Err(format!(
                "Building40 menu {instance:08X}: tourism statistic must be 50AA0BEA:0EBCE29A paired with 0DA3EE15"
            ));
        }
        let numeric = unit.get(0x0DA3_EE15).is_some_and(|property| {
            matches!(
                property.kind,
                Kind::Scalar(Value::Float(_) | Value::Int32(_) | Value::UInt32(_))
            )
        });
        if !numeric {
            return Err(format!(
                "Building40 unit {instance:08X}: tourism parameter 0DA3EE15 has no numeric Unit value; the panel would print a template placeholder"
            ));
        }
    }
    if texts.len() == 3
        && texts
            .iter()
            .any(|text| text.table_id == 0x50AA_0BEA && text.instance_id == 0x0EBC_E055)
    {
        // Education / s31 GOLDEN shape: three text↔param pairs must match exactly.
        let expected = [
            (0x0D9A_B016u32, 0x0AD9_3A77u32),
            (0x0D9A_B027, 0x0C3E_9F99),
            (0x0EBC_E055, 0x0E58_5AE0),
        ];
        for (index, (text_instance, param)) in expected.into_iter().enumerate() {
            if texts[index].table_id != 0x50AA_0BEA
                || texts[index].instance_id != text_instance
                || params[index] != param
            {
                return Err(format!(
                    "Building40 menu {instance:08X}: s31 statistic row {index} must be \
                     50AA0BEA:{text_instance:08X} paired with {param:08X}"
                ));
            }
        }
    }

    let power_bundle = unit.values.iter().any(|property| {
        matches!(property.hash, 0x0A02_09A2 | 0x0A02_09A3 | 0x0A02_09A4 | 0x0A02_09A5)
            && match &property.kind {
                Kind::Array(values) => values
                    .iter()
                    .any(|value| matches!(value, Value::Key(key) if key.instance == 0x4893_9D96)),
                Kind::Scalar(Value::Key(key)) => key.instance == 0x4893_9D96,
                _ => false,
            }
    });
    let power_excluded = unit.get(0x0A02_09B2).is_some_and(|property| {
        matches!(&property.kind, Kind::Array(values) if values.iter()
            .any(|value| matches!(value, Value::Key(key) if key.instance == 0x4893_9D96)))
    });
    if !(power_bundle || native_school) || power_excluded {
        return Err(format!(
            "Building40 unit {instance:08X}: power bundle 48939D96 is absent from A2/A3/A4/A5; refusing an unpowered building"
        ));
    }
    Ok(vec![
        format!("Building40 panelFields={}", menu.values.len()),
        format!("Building40 unitFields={}", unit.values.len()),
        format!("Building40 statisticRows={}", texts.len()),
        if power_bundle { "Building40 powerBundle=48939D96" }
        else { "Building40 powerBundle=48939D96 (native school template)" }.into(),
    ])
}

impl MenuContractReport {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "instance": format!("{:08X}", self.instance),
            "categories": self.categories.iter().map(|id| format!("{id:08X}")).collect::<Vec<_>>(),
            "modelRefs": self.model_refs,
            "checks": self.checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{Key, PropType, Property, PropertyEncoding, Text};

    const LOT_GROUP: u32 = 0x40E1_C000;
    const INSTANCE: u32 = 0x0000_0001;
    const MODEL: u32 = 0x0000_00AA;
    const MASK: u32 = 0x0000_00BB;
    const LOCALE: u32 = 0x0000_0010;

    fn prop(hash: u32, prop_type: PropType, kind: Kind) -> Property {
        let array_item_size = match prop_type {
            PropType::Key | PropType::Vector3 | PropType::ColorRgb => Some(12),
            _ => None,
        };
        Property {
            hash,
            prop_type,
            kind,
            encoding: PropertyEncoding {
                flags: 0,
                array_item_size,
            },
        }
    }
    fn key(instance: u32) -> Value {
        Value::Key(Key {
            type_id: 0,
            group: 0,
            instance,
        })
    }
    fn encoded(values: Vec<Property>) -> Vec<u8> {
        PropertyFile {
            claimed_count: values.len() as u32,
            values,
        }
        .encode_canonical()
        .unwrap()
    }
    fn base_resources(categories: &[u32], unit_category: u32) -> Resources {
        let mut resources = Resources::new();
        let menu = encoded(vec![
            prop(
                HASH_UI_CATEGORY,
                PropType::Key,
                Kind::Array(categories.iter().map(|id| key(*id)).collect()),
            ),
            prop(HASH_MENU_UNIT, PropType::Key, Kind::Scalar(key(INSTANCE))),
            prop(
                HASH_TITLE,
                PropType::Text,
                Kind::Scalar(Value::Text(Text {
                    table_id: LOCALE,
                    instance_id: 1,
                })),
            ),
        ]);
        let unit = encoded(vec![
            prop(
                HASH_UNIT_ASSET_LINK,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: PROPERTY_TYPE,
                    group: LOT_GROUP,
                    instance: INSTANCE,
                })),
            ),
            prop(
                HASH_UNIT_CATEGORY,
                PropType::Key,
                Kind::Scalar(key(unit_category)),
            ),
        ]);
        let asset = encoded(vec![
            prop(
                HASH_BOUNDING_BOX,
                PropType::BoundingBox,
                Kind::Scalar(Value::BoundingBox {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                }),
            ),
            prop(
                0x00F9_EFBB,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: MODEL_TYPE,
                    group: 0,
                    instance: MODEL,
                })),
            ),
            prop(
                HASH_LOT_MASK,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: RASTER_TYPE,
                    group: 0,
                    instance: MASK,
                })),
            ),
        ]);
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: MENU_GROUP,
                instance: INSTANCE,
            },
            menu,
        );
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: UNIT_GROUP,
                instance: INSTANCE,
            },
            unit,
        );
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: LOT_GROUP,
                instance: INSTANCE,
            },
            asset,
        );
        resources.insert(
            ResourceId {
                type_id: MODEL_TYPE,
                group: 0,
                instance: MODEL,
            },
            vec![0u8; 1],
        );
        resources.insert(
            ResourceId {
                type_id: RASTER_TYPE,
                group: 0,
                instance: MASK,
            },
            vec![0u8; 1],
        );
        let mut locale = vec![0xEF, 0xBB, 0xBF];
        locale.extend_from_slice(br#"{"0x00000001":"Test building"}"#);
        resources.insert(
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance: LOCALE,
            },
            locale,
        );
        resources
    }

    const EDUCATION: u32 = 0x95A9_D6FD;

    #[test]
    fn accepts_a_complete_menu_identity() {
        let resources = base_resources(&[0x6A17_85B5, EDUCATION], EDUCATION);
        let report = validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap();
        assert_eq!(report.model_refs, 1);
        assert_eq!(report.categories, vec![0x6A17_85B5, EDUCATION]);
        assert!(report.checks.iter().any(|c| c.starts_with("title=Test")));
    }

    #[test]
    fn rejects_int32_cost_bit_patterns() {
        // 2026-11-08 HopeTower：schema 50000 走 Int32，实机按 f32 读 ~0 →
        // 建造面板「免费」。门禁必须在字节编码层钉住 Float32。
        let mut resources = base_resources(&[EDUCATION], EDUCATION);
        let unit = encoded(vec![
            prop(
                HASH_UNIT_ASSET_LINK,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: PROPERTY_TYPE,
                    group: LOT_GROUP,
                    instance: INSTANCE,
                })),
            ),
            prop(
                HASH_UNIT_CATEGORY,
                PropType::Key,
                Kind::Scalar(key(EDUCATION)),
            ),
            prop(
                HASH_CONSTRUCTION_COST,
                PropType::Int32,
                Kind::Scalar(Value::Int32(50_000)),
            ),
            prop(
                HASH_MAINTENANCE_COST,
                PropType::Float,
                Kind::Scalar(Value::Float(1500.0)),
            ),
        ]);
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: UNIT_GROUP,
                instance: INSTANCE,
            },
            unit,
        );
        let error = validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap_err();
        assert!(error.contains("Float32"), "{error}");
    }

    #[test]
    fn rejects_legacy_shared_locale_table() {
        // 2026-11-08 HopeTower：v7 与本包同写 0xBF82F7CE，v7 的
        // "OpenSCP Commercial Tower" 遮蔽「大连希望大厦」。共享表禁写。
        let mut resources = base_resources(&[EDUCATION], EDUCATION);
        let menu = encoded(vec![
            prop(
                HASH_UI_CATEGORY,
                PropType::Key,
                Kind::Array(vec![key(EDUCATION)]),
            ),
            prop(HASH_MENU_UNIT, PropType::Key, Kind::Scalar(key(INSTANCE))),
            prop(
                HASH_TITLE,
                PropType::Text,
                Kind::Scalar(Value::Text(Text {
                    table_id: 0xBF82_F7CE,
                    instance_id: 1,
                })),
            ),
        ]);
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: MENU_GROUP,
                instance: INSTANCE,
            },
            menu,
        );
        let error = validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap_err();
        assert!(error.contains("legacy shared locale table"), "{error}");
    }

    #[test]
    fn park_container_on_a_building_unit_is_a_warning() {
        // 公园入口 + 教育行为是 s24 验证过的"菜单/行为解耦"配方，只提示不拦截。
        let resources = base_resources(&[PARK_CATEGORY], EDUCATION);
        let report = validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap();
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.contains("park panel entry with non-park Unit"))
        );
    }

    #[test]
    fn allows_park_container_on_a_park_unit() {
        let resources = base_resources(&[PARK_CATEGORY], PARK_CATEGORY);
        validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap();
    }

    #[test]
    fn rejects_missing_locale_row() {
        let mut resources = base_resources(&[EDUCATION], EDUCATION);
        let mut locale = vec![0xEF, 0xBB, 0xBF];
        locale.extend_from_slice(br#"{"0x00000002":"desc only"}"#);
        resources.insert(
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance: LOCALE,
            },
            locale,
        );
        let error = validate(&resources, INSTANCE, LOT_GROUP, None, true).unwrap_err();
        assert!(error.contains("***"), "{error}");
    }

    #[test]
    fn rejects_missing_model_refs() {
        let mut resources = base_resources(&[EDUCATION], EDUCATION);
        let asset = encoded(vec![
            prop(
                HASH_BOUNDING_BOX,
                PropType::BoundingBox,
                Kind::Scalar(Value::BoundingBox {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                }),
            ),
            prop(
                HASH_LOT_MASK,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: RASTER_TYPE,
                    group: 0,
                    instance: MASK,
                })),
            ),
        ]);
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: LOT_GROUP,
                instance: INSTANCE,
            },
            asset,
        );
        let report = validate(&resources, INSTANCE, LOT_GROUP, None, false).unwrap();
        assert!(
            report
                .checks
                .iter()
                .any(|check| check.contains("no resolvable model refs"))
        );
    }

    #[test]
    fn rejects_synthesized_self_map() {
        let mut resources = base_resources(&[EDUCATION], EDUCATION);
        let asset = encoded(vec![
            prop(
                HASH_BOUNDING_BOX,
                PropType::BoundingBox,
                Kind::Scalar(Value::BoundingBox {
                    min: [0.0, 0.0, 0.0],
                    max: [1.0, 1.0, 1.0],
                }),
            ),
            prop(
                0x00F9_EFBB,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: MODEL_TYPE,
                    group: 0,
                    instance: MODEL,
                })),
            ),
            prop(
                HASH_LOT_MASK,
                PropType::Key,
                Kind::Scalar(Value::Key(Key {
                    type_id: RASTER_TYPE,
                    group: 0,
                    instance: MASK,
                })),
            ),
            prop(
                HASH_SELF_MAP,
                PropType::Key,
                Kind::Array(vec![Value::Key(Key {
                    type_id: PROPERTY_TYPE,
                    group: LOT_GROUP,
                    instance: INSTANCE,
                })]),
            ),
        ]);
        resources.insert(
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: LOT_GROUP,
                instance: INSTANCE,
            },
            asset,
        );
        let donor = base_resources(&[EDUCATION], EDUCATION);
        let error = validate(
            &resources,
            INSTANCE,
            LOT_GROUP,
            Some((&donor, INSTANCE)),
            true,
        )
        .unwrap_err();
        assert!(error.contains("self-map"), "{error}");
    }

    fn complete_donors() -> (PropertyFile, PropertyFile) {
        let menu = PropertyFile::parse(include_bytes!(
            "../../resources/sc5/complete-building-menu.bin"
        ))
        .unwrap();
        let unit = PropertyFile::parse(include_bytes!(
            "../../resources/sc5/complete-building-unit.bin"
        ))
        .unwrap();
        assert_eq!(menu.values.len(), 48);
        assert_eq!(unit.values.len(), 175);
        (menu, unit)
    }

    #[test]
    fn building40_accepts_complete_panel_statistics_and_power_closure() {
        let (menu, unit) = complete_donors();
        let checks = validate_complete_building(&menu, &unit, INSTANCE).unwrap();
        assert!(checks.iter().any(|check| check.contains("panelFields=48")));
        assert!(checks.iter().any(|check| check.contains("powerBundle")));
    }

    #[test]
    fn building40_rejects_minimal_menu_and_missing_power_bundle() {
        let (menu, unit) = complete_donors();
        let error = validate_complete_building(
            &PropertyFile::default(),
            &PropertyFile::default(),
            INSTANCE,
        )
        .unwrap_err();
        assert!(error.contains("required panel field"), "{error}");

        let mut without_power = unit;
        without_power.values.retain(|property| property.hash != 0x0A02_09A3);
        let error = validate_complete_building(&menu, &without_power, INSTANCE).unwrap_err();
        assert!(error.contains("power bundle 48939D96"), "{error}");
    }

    #[test]
    fn building40_tourism_row_requires_its_numeric_unit_parameter() {
        let (mut menu, unit) = complete_donors();
        for property in &mut menu.values {
            match property.hash {
                0x0EB1_FC05 => {
                    property.kind = Kind::Array(vec![Value::Text(Text {
                        table_id: 0x50AA_0BEA,
                        instance_id: 0x0EBC_E29A,
                    })]);
                }
                0x0EB1_FC22 => property.kind = Kind::Array(vec![Value::UInt32(0x0DA3_EE15)]),
                _ => {}
            }
        }
        validate_complete_building(&menu, &unit, INSTANCE).unwrap();
        let mut broken = unit;
        broken.values.retain(|property| property.hash != 0x0DA3_EE15);
        let error = validate_complete_building(&menu, &broken, INSTANCE).unwrap_err();
        assert!(error.contains("template placeholder"), "{error}");
    }

    #[test]
    fn building40_rejects_park_style_two_row_statistics() {
        let (mut menu, unit) = complete_donors();
        for property in &mut menu.values {
            match property.hash {
                0x0EB1_FC05 => {
                    property.kind = Kind::Array(vec![
                        Value::Text(Text {
                            table_id: 0x50AA_0BEA,
                            instance_id: 0x0D9A_B016,
                        }),
                        Value::Text(Text {
                            table_id: 0x50AA_0BEA,
                            instance_id: 0x0EBC_E055,
                        }),
                    ]);
                }
                0x0EB1_FC22 => {
                    property.kind = Kind::Array(vec![Value::UInt32(0x0AD9_3A77), Value::UInt32(0)])
                }
                _ => {}
            }
        }
        let error = validate_complete_building(&menu, &unit, INSTANCE).unwrap_err();
        assert!(error.contains("park-style 2-row"), "{error}");
    }

    #[test]
    fn building40_accepts_s31_three_row_statistics() {
        let (mut menu, unit) = complete_donors();
        for property in &mut menu.values {
            match property.hash {
                0x0EB1_FC05 => {
                    property.kind = Kind::Array(vec![
                        Value::Text(Text {
                            table_id: 0x50AA_0BEA,
                            instance_id: 0x0D9A_B016,
                        }),
                        Value::Text(Text {
                            table_id: 0x50AA_0BEA,
                            instance_id: 0x0D9A_B027,
                        }),
                        Value::Text(Text {
                            table_id: 0x50AA_0BEA,
                            instance_id: 0x0EBC_E055,
                        }),
                    ]);
                }
                0x0EB1_FC22 => {
                    property.kind = Kind::Array(vec![
                        Value::UInt32(0x0AD9_3A77),
                        Value::UInt32(0x0C3E_9F99),
                        Value::UInt32(0x0E58_5AE0),
                    ])
                }
                _ => {}
            }
        }
        let checks = validate_complete_building(&menu, &unit, INSTANCE).unwrap();
        assert!(checks.iter().any(|check| check.contains("statisticRows=3")));
    }
}
