//! 菜单录入（`semantics.menu`）：把构建好的建筑以全新 instance 注册为
//! 城市发展>文化>地标 下的独立菜单条目，不覆盖原建筑。
//!
//! 依据（全部实证）：
//! - 字段语义：`docs/re/menu-item-relations-20261018.md`、
//!   `docs/re/willis-tower-landmark-20261027.json`、
//!   `docs/re/landmark-asset-metadata-checklist-20261027.md`；
//! - 写入形状：`src/lib/game-ui/menu-export.ts`（实机闭环验证过的编码）；
//! - 最小可用先例：`SC5_NightHouse_Test_0_5_0.package` 解剖（2026-10-30，
//!   17 条资源、0 覆盖、三 property 同 instance + 00B2CCCB 模板继承）。
//!
//! 资源组约定（组不可换，游戏按组寻址）：
//! - Menu2      `00B1B104:09878A01:<X>`（工具条目组）
//! - Unit 叶子  `00B1B104:40E0C000:<X>`（cUnit 叶子，parent → 地标模板
//!   `48E0C000:4E01E311`，运行时继承解锁/限额/经济科目）
//! - Lot 叶子   `00B1B104:40E1C000:<X>`（克隆参照 lot property 并重定向
//!   模型/单元，parent → `40E1C000:4E01E311`）
//! - 图标 PNG   `2F7D0004:40E02400:<X>`、rollover JPG `3F8662EA:0:<X>`
//! - locale 表  `0A98EAF0:02FABF01:BF82F7CE`（自有表，键 `"0x%08x"`）
use super::{
    connector, materials::raster_type, package::LodModelIds, scene::SceneDocument,
};
use dbpf::ResourceId;
use sc_properties::{
    Kind, LOD_MODEL_HASHES, LOT_MASK_HASH, LOT_SIZE_HASH, PropType, Property, PropertyEncoding,
    PropertyFile, Text, Value,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// 工具条目组（Menu2 所在组；地标批在 SimCity_Graphics 包）。
pub const MENU_GROUP: u32 = 0x09878_A01;
/// cUnit 叶子组 / Lot 资产配置组 / 模板组（Unit 模板在 48E0C000）。
pub const UNIT_GROUP: u32 = 0x40E0_C000;
pub fn lot_group() -> u32 {
    std::env::var("OPENSCP_LOT_GROUP")
        .ok()
        .and_then(|value| u32::from_str_radix(value.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0x40E1_C000)
}
pub const UNIT_TEMPLATE_GROUP: u32 = 0x48E0_C000;
/// 地标 cUnit 模板对（TemplateToyBase_landmark；plan.rs 保护表同源）。
pub const LANDMARK_UNIT_TEMPLATE: u32 = 0x565B_72B9;
pub const LANDMARK_LOT_TEMPLATE: u32 = 0x34CE_7E14;
/// 文化>城市发展 顶层 plop 菜单根（parent 固定写法）。
pub const PLOP_MENU_PARENT: u32 = 0xAF042_E9A;
/// 「文化>地标」子分类容器 instance（uiCategories 归属边）。
pub const LANDMARK_CATEGORY: u32 = 0x3F43_E038;
/// 图标 PNG 组 / rollover JPG 类型。
pub const ICON_GROUP: u32 = 0x40E0_2400;
pub const ICON_TYPE: u32 = 0x2F7D_0004;
pub const MARQUEE_TYPE: u32 = 0x3F86_62EA;
/// OpenSCP 自有 locale 表（FNV-1("openscp/strings")，与原版 356 张表无冲突）。
pub const LOCALE_TABLE: u32 = 0xBF82_F7CE;
pub const LOCALE_TYPE: u32 = 0x0A98_EAF0;
pub const LOCALE_GROUP: u32 = 0x02FA_BF01;

// Menu2 property 哈希（menu-export.ts HASH 表同源）。
const HASH_PARENT: u32 = 0x00B2_CCCB;
/// Dalian’s proven full identity instance (Menu2/Unit/Lot share it).
pub const DALIAN_IDENTITY_INSTANCE: u32 = 0xF922_1FCF;
/// Dalian’s locale table id (existing game table, safe to override).
pub const DALIAN_LOCALE_TABLE: u32 = 0x284C_6727;
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_TOOL_ICON: u32 = 0x0977_AA8F;
const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const HASH_MARQUEE: u32 = 0x0DDE_EE56;
const HASH_UNLOCK_TARGET: u32 = 0x0DE8_4DC9;
const HASH_UNLOCK_AMOUNT: u32 = 0x0DE8_4DD3;
const HASH_UNLOCK_STRING: u32 = 0x0DE8_4DDC;
const HASH_EFFECT_TITLE: u32 = 0x0EB1_FC05;
const HASH_EFFECT_PARAM: u32 = 0x0EB1_FC22;
const HASH_PARK_FLAG: u32 = 0x0EC4_80DF;
/// kResourceIDHQ_CityHall_IsOn（市政厅启用；NightHouse/威利斯同款解锁位）。
const UNLOCK_CITY_HALL: u32 = 0xC93F_D4B5;
// Unit 叶子哈希。
const HASH_UNIT_LOT_LINK: u32 = 0x08E1_7ED7;
const HASH_UNIT_EFFECT: u32 = 0x0AFE_27CA;
const HASH_UNIT_UNLOCK_BIN: u32 = 0x0BF5_D2E3;
const HASH_UNIT_UNLOCK_AMOUNT: u32 = 0x0BF5_D2E4;
const HASH_CONSTRUCTION_COST: u32 = 0x0A8A_41DA;
const HASH_MAINTENANCE_COST: u32 = 0x09AE_19D7;

/// `semantics.menu` 文档：新建筑身份 + 菜单归属。字段全部可选除标题。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct MenuRegistrationSpec {
    /// 菜单显示名（写入自有 locale 表）。
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    /// uiCategories 目标子分类 instance；默认 `0x3F43E038`（文化>地标）。
    /// 公园面板用 `0x07049250`（原版 tools.json 公园工具同款）。
    #[serde(default, deserialize_with = "de_hash")]
    pub category_instance: Option<u32>,
    /// 面板内排序（原版地标 ~11500）。
    #[serde(default)]
    pub ui_position: Option<i32>,
    /// 建造成本（SCUnit 0x0A8A41DA）；缺省继承模板。
    #[serde(default)]
    pub construction_cost: Option<i32>,
    /// 维护成本（SCUnit 0x09AE19D7）；缺省继承模板。
    #[serde(default)]
    pub maintenance_cost: Option<i32>,
    /// 工具图标 PNG 的项目内路径；缺省自动生成 128×128 占位图。
    #[serde(default)]
    pub icon_png: Option<String>,
    /// Optional proven identity donor package (relative to project root).
    /// When present, Menu2/Unit/Lot are cloned from this package instead of
    /// being rebuilt from the NightHouse minimal shape.
    #[serde(default)]
    pub identity_package: Option<String>,
    /// Donor instance shared by the donor Menu2/Unit/Lot (Dalian: F9221FCF).
    #[serde(default, deserialize_with = "de_hash")]
    pub identity_instance: Option<u32>,
    /// Locale table id used by the donor title/description Text values.
    #[serde(default, deserialize_with = "de_hash")]
    pub locale_table: Option<u32>,
    /// Menu2 继承边（00B2CCCB）的目标 instance；默认 plop 根 `AF042E9A`。
    /// 公园类条目用公园子根（如 `F970A822`，原版 tools.json 同款）。
    #[serde(default, deserialize_with = "de_hash")]
    pub menu_parent_instance: Option<u32>,
    /// Unit 叶子的 cUnit 模板 instance（组 48E0C000）；默认地标模板
    /// `4E01E311`。公园测试可用 NightHouse 实证对 `565B72B9`。
    #[serde(default, deserialize_with = "de_hash")]
    pub unit_template_instance: Option<u32>,
    /// Lot 叶子的 Lot 模板 instance（组 40E1C000）；默认地标模板
    /// `4E01E311`。公园测试可用 NightHouse 实证对 `34CE7E14`。
    #[serde(default, deserialize_with = "de_hash")]
    pub lot_template_instance: Option<u32>,
    /// 0.8.0 parent-free 身份路线：Asset 字典 = 通用链 40E1C000:9EC474FF
    /// 物化 + 地面 Lot 数字字段 + prop/light 列；Unit = tested-unit.bin
    /// 克隆；不再克隆参照 lot。`reference.lot` 此时充当 ground 来源。
    #[serde(default)]
    pub parent_free: bool,
    /// 作者朝向（asset_transforms.rotation_degrees，默认 180）。parent-free
    /// 路线同时作用于几何旋转、地面 raster/盒与 prop/light 矩阵。
    #[serde(default)]
    pub rotation_degrees: Option<u32>,
}

/// instance 字段同时接受 JSON 数字与 `"0xXXXXXXXX"` 十六进制字符串。
fn de_hash<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let value = <Option<serde_json::Value> as Deserialize>::deserialize(deserializer)?;
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(number)) => number
            .as_u64()
            .and_then(|v| u32::try_from(v).ok())
            .map(Some)
            .ok_or_else(|| serde::de::Error::custom("instance out of u32 range")),
        Some(serde_json::Value::String(text)) => u32::from_str_radix(
            text.trim()
                .trim_start_matches("0x")
                .trim_start_matches("0X"),
            16,
        )
        .map(Some)
        .map_err(|_| serde::de::Error::custom(format!("invalid hex instance {text:?}"))),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected u32 or hex string, got {other}"
        ))),
    }
}

impl MenuRegistrationSpec {
    /// 从 `SemanticsIr.menu`（字符串键 JSON 表）解析并校验。
    pub fn from_values(
        values: &std::collections::BTreeMap<String, serde_json::Value>,
    ) -> Result<Self, String> {
        let value =
            serde_json::Value::Object(values.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let spec: MenuRegistrationSpec = serde_json::from_value(value)
            .map_err(|error| format!("invalid menu registration: {error}"))?;
        if spec.title.trim().is_empty() {
            return Err("menu registration requires a non-empty title".into());
        }
        Ok(spec)
    }
}

/// 菜单录入产出的全部资源（delta 写包时全部进入输出）。
pub struct MenuResources {
    /// Menu2/Unit/Lot 共用的新建筑 instance。
    pub instance: u32,
    pub entries: Vec<(ResourceId, Vec<u8>)>,
}

/// 由参照 lot property 克隆出新建筑身份并生成菜单/单元/图标/locale 资源。
///
/// `lot_source` 必须已存在于 `resources`（参照包克隆）；新 lot 叶子继承其
/// 全部字段后改挂地标模板，再走与同 TGI 覆盖完全相同的场景 lowering。
pub fn generate(
    spec: &MenuRegistrationSpec,
    resources: &connector::Resources,
    lot_source: ResourceId,
    scene_document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
    identity: Option<&connector::Resources>,
) -> Result<MenuResources, String> {
    let instance = alloc_menu_instance(spec, resources);
    // 0.8.0 parent-free route: generic Asset dictionary + tested-unit.bin
    // cUnit; props/lights land in the Asset dictionary (author semantics).
    if spec.parent_free {
        return generate_parent_free(
            spec,
            resources,
            lot_source,
            instance,
            scene_document,
            model_ids,
        );
    }
    let lot_bytes = resources.get(&lot_source).ok_or_else(|| {
        format!("menu registration: reference lot {lot_source} missing from source package")
    })?;
    let (mask_entry, mask_key) = clone_lot_mask(resources, lot_bytes, instance)?;
    let (menu2, unit_leaf, lot_leaf, locale_table) = if let Some(identity) = identity {
        clone_donor_identity(
            spec,
            identity,
            lot_bytes,
            instance,
            mask_key,
            model_ids,
            scene_document,
        )?
    } else {
        let lot_leaf = build_lot_leaf(
            spec,
            lot_bytes,
            instance,
            scene_document,
            model_ids,
            mask_key,
        )?;
        (
            build_menu2(spec, instance)?,
            build_unit_leaf(spec, instance)?,
            lot_leaf,
            LOCALE_TABLE,
        )
    };
    let locale = if identity.is_some() {
        build_donor_locale(spec, instance, locale_table)
    } else {
        build_locale(spec, instance)
    };
    let icon_png = build_icon_png(spec, instance);
    let marquee_jpg = build_marquee_jpg(instance);

    let mut entries = vec![
        (
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: MENU_GROUP,
                instance,
            },
            menu2,
        ),
        (
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: UNIT_GROUP,
                instance,
            },
            unit_leaf,
        ),
        (
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: lot_group(),
                instance,
            },
            lot_leaf,
        ),
        (
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance: locale_table,
            },
            locale,
        ),
        (
            ResourceId {
                type_id: ICON_TYPE,
                group: ICON_GROUP,
                instance,
            },
            icon_png,
        ),
        (
            ResourceId {
                type_id: MARQUEE_TYPE,
                group: 0,
                instance,
            },
            marquee_jpg,
        ),
    ];
    if let Some(entry) = mask_entry {
        entries.push(entry);
    }
    Ok(MenuResources { instance, entries })
}

/// 0.8.0 parent-free identity route: Menu2/icon/locale from the verified
/// minimal shape, Unit from `tested-unit.bin`, Asset dictionary materialized
/// from the generic chain (`native_profile::build`).
fn generate_parent_free(
    spec: &MenuRegistrationSpec,
    resources: &connector::Resources,
    ground_lot: ResourceId,
    instance: u32,
    scene_document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
) -> Result<MenuResources, String> {
    let rotation = spec.rotation_degrees.unwrap_or(180);
    // Menu2 = the FULLY materialized generic menu chain (author parent-free
    // shape; the reference package carries F970A822 + AF042E9A layers — the
    // walk must cover both or the tool entry loses its presentation fields
    // and the game drops it from the menu).
    let menu2 = match super::native_profile::flat_chain(resources, super::native_profile::GENERIC_MENU) {
        Ok((mut file, _)) => {
            apply_menu_overrides(&mut file, spec, instance)?;
            encode(&file)?
        }
        Err(_) => build_menu2(spec, instance)?,
    };
    let native = super::native_profile::build(
        spec,
        resources,
        ground_lot,
        instance,
        scene_document,
        model_ids.as_ref(),
        rotation,
    )?;
    let asset_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: super::native_profile::ASSET_GROUP,
        instance,
    };
    let unit_id = ResourceId {
        type_id: PROPERTY_TYPE,
        group: UNIT_GROUP,
        instance,
    };
    let locale = build_locale(spec, instance);
    let icon_png = build_icon_png(spec, instance);
    let marquee_jpg = build_marquee_jpg(instance);
    let mut entries = vec![
        (
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: MENU_GROUP,
                instance,
            },
            menu2,
        ),
        (unit_id, encode(&native.unit)?),
        (asset_id, encode(&native.asset)?),
        (
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance: LOCALE_TABLE,
            },
            locale,
        ),
        (
            ResourceId {
                type_id: ICON_TYPE,
                group: ICON_GROUP,
                instance,
            },
            icon_png,
        ),
        (
            ResourceId {
                type_id: MARQUEE_TYPE,
                group: 0,
                instance,
            },
            marquee_jpg,
        ),
    ];
    if let Some((mask_id, mask_bytes)) = native.mask {
        entries.push((mask_id, mask_bytes));
    }
    Ok(MenuResources { instance, entries })
}

/// Clone a proven identity (Dalian) and only replace the per-asset links.
fn clone_donor_identity(
    spec: &MenuRegistrationSpec,
    identity: &connector::Resources,
    reference_lot: &[u8],
    instance: u32,
    mask_key: Option<ResourceId>,
    model_ids: Option<LodModelIds>,
    scene_document: Option<&SceneDocument>,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>, u32), String> {
    let donor_instance = spec.identity_instance.unwrap_or(DALIAN_IDENTITY_INSTANCE);
    let locale_table = spec.locale_table.unwrap_or(DALIAN_LOCALE_TABLE);
    let donor_property = |group: u32| -> Result<&[u8], String> {
        identity
            .get(&ResourceId {
                type_id: PROPERTY_TYPE,
                group,
                instance: donor_instance,
            })
            .map(Vec::as_slice)
            .ok_or_else(|| {
                format!(
                    "identity donor is missing Property {PROPERTY_TYPE:08X}:{group:08X}:{donor_instance:08X}"
                )
            })
    };
    let mut menu = PropertyFile::parse(donor_property(MENU_GROUP)?)
        .map_err(|error| format!("identity donor Menu2 parse: {error}"))?;
    let mut unit = PropertyFile::parse(donor_property(UNIT_GROUP)?)
        .map_err(|error| format!("identity donor Unit parse: {error}"))?;
    let mut lot = PropertyFile::parse(donor_property(lot_group())?)
        .map_err(|error| format!("identity donor Lot parse: {error}"))?;

    patch_instance_refs(&mut menu, donor_instance, instance);
    patch_instance_refs(&mut unit, donor_instance, instance);
    patch_instance_refs(&mut lot, donor_instance, instance);

    // The external SC5 Asset Packager uses the cUnit -> Asset edge here.
    // Dalian's donor is a legacy identity whose field happens to point at
    // the Unit group; retaining that value leaves the menu placeable but
    // gives the renderer no Asset/LOD chain to follow.
    set_scalar(
        &mut unit,
        HASH_UNIT_LOT_LINK,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: lot_group(),
            instance,
        }),
    );

    // Park category: the donor is education; the caller asked for a park.
    set_array(
        &mut menu,
        HASH_UI_CATEGORY,
        PropType::Key,
        vec![Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: spec.category_instance.unwrap_or(0x0704_9250),
        })],
    );
    set_text_table(&mut menu, HASH_TITLE, locale_table, 1);
    set_text_table(&mut menu, HASH_DESC, locale_table, 2);

    if let Some(cost) = spec.construction_cost {
        set_scalar(
            &mut unit,
            HASH_CONSTRUCTION_COST,
            PropType::Float,
            Value::Float(cost as f32),
        );
    }
    if let Some(cost) = spec.maintenance_cost {
        set_scalar(
            &mut unit,
            HASH_MAINTENANCE_COST,
            PropType::Float,
            Value::Float(cost as f32),
        );
    }

    if let Some(mask) = mask_key {
        set_scalar(
            &mut lot,
            LOT_MASK_HASH,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: mask.type_id,
                group: mask.group,
                instance: mask.instance,
            }),
        );
    }
    if let Some(ids) = model_ids {
        for (index, hash) in LOD_MODEL_HASHES.into_iter().enumerate() {
            set_scalar(
                &mut lot,
                hash,
                PropType::Key,
                Value::Key(sc_properties::Key {
                    type_id: ids[index].type_id,
                    group: ids[index].group,
                    instance: ids[index].instance,
                }),
            );
        }
    }
    // Keep ground size and bounding box from the reference asset.
    if let Ok(reference) = PropertyFile::parse(reference_lot) {
        if let Some(value) = reference.get(LOT_SIZE_HASH).and_then(|p| p.scalar()) {
            set_scalar(&mut lot, LOT_SIZE_HASH, PropType::Vector2, value.clone());
        }
        for hash in [
            0x00F9_EFBAu32,
            0x0FE0_1484,
            0x0BF7_6A29,
            0x0BF7_6A2A,
            0x0CCB_7FC8,
            0x0CCB_7FC9,
            0x0CCB_7FD0,
            0x0CCB_7FD2,
            0x0CCB_7FD3,
            0x0CCB_7FD4,
            0x0CCB_7FD5,
            0x0CCB_7FD6,
        ] {
            if let Some(source) = reference.get(hash) {
                if let Some(target) = lot.values.iter_mut().find(|p| p.hash == hash) {
                    *target = source.clone();
                } else {
                    lot.values.push(source.clone());
                }
            }
        }
    }
    if let Some(document) = scene_document {
        super::scene::apply_units(&mut lot, document)?;
    }
    // Do not synthesize parent/self-map fields here.  The external packager
    // preserves those fields from the Asset/Lot donor; Dalian and the known
    // working reference both omit them.  Inventing them changes the runtime
    // inheritance graph and can leave only a placement marker visible.
    set_array(
        &mut lot,
        0x0CA5_CF4A,
        PropType::Key,
        vec![Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: lot_group(),
            instance,
        })],
    );
    if let Some([width, height]) = scene_document.and_then(|document| document.lot_size) {
        set_scalar(
            &mut lot,
            LOT_SIZE_HASH,
            PropType::Vector2,
            Value::Vector2([width, height]),
        );
    }
    lot.claimed_count = lot.values.len() as u32;

    Ok((
        encode(&menu)?,
        encode(&unit)?,
        encode(&lot)?,
        locale_table,
    ))
}

fn patch_instance_refs(file: &mut PropertyFile, old: u32, new: u32) {
    for property in &mut file.values {
        match &mut property.kind {
            Kind::Scalar(Value::Key(key)) => patch_key(key, old, new),
            Kind::Array(values) => {
                for value in values {
                    if let Value::Key(key) = value {
                        patch_key(key, old, new);
                    }
                }
            }
            _ => {}
        }
    }
}

fn patch_key(key: &mut sc_properties::Key, old: u32, new: u32) {
    if key.instance == old
        && (key.type_id == 0
            || key.type_id == PROPERTY_TYPE
            || key.type_id == ICON_TYPE
            || key.type_id == MARQUEE_TYPE)
    {
        key.instance = new;
    }
}

fn set_text_table(file: &mut PropertyFile, hash: u32, table: u32, instance: u32) {
    let values = vec![Value::Text(Text {
        table_id: table,
        instance_id: instance,
    })];
    set_array_with_flags(file, hash, PropType::Text, values, 0x801C);
}

fn build_donor_locale(
    spec: &MenuRegistrationSpec,
    instance: u32,
    table: u32,
) -> Vec<u8> {
    let _ = (instance, table);
    let mut lines = vec![format!(
        "    \"0x00000001\" : {}",
        serde_json::to_string(spec.title.trim()).unwrap_or_default()
    )];
    if let Some(description) = &spec.description {
        lines.push(format!(
            "    \"0x00000002\" : {}",
            serde_json::to_string(description.trim()).unwrap_or_default()
        ));
    }
    lines.push("    \"0xffffffff\" : \"0xffffffff\"".to_string());
    let json = format!("{{\r\n{}\r\n}}", lines.join(",\r\n"));
    let mut bytes = Vec::with_capacity(json.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(json.as_bytes());
    bytes
}

/// Copy the reference lot's mask under a fresh raster instance and return
/// both the new resource and its Key.  A new menu identity must not reuse the
/// reference lot's mask TGI: that would overwrite the original building's
/// mask (or collide with an official DLC entry).
fn clone_lot_mask(
    resources: &connector::Resources,
    lot_bytes: &[u8],
    menu_instance: u32,
) -> Result<(Option<(ResourceId, Vec<u8>)>, Option<ResourceId>), String> {
    let file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("menu registration: reference lot is not a property: {error}"))?;
    let Some(property) = file.get(LOT_MASK_HASH) else {
        return Ok((None, None));
    };
    let Kind::Scalar(Value::Key(key)) = &property.kind else {
        return Err("menu registration: lot mask is not a Key".into());
    };
    let candidates = [
        ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: raster_type(),
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: raster_type(),
            group: 0,
            instance: key.instance,
        },
    ];
    let (_, bytes) = candidates
        .iter()
        .find_map(|id| resources.get_key_value(id))
        .ok_or_else(|| {
            format!(
                "menu registration: reference lot mask 0x{:08X} is not in the source package",
                key.instance
            )
        })?;
    let instance = alloc_raster_instance(resources, menu_instance, bytes);
    let id = ResourceId {
        type_id: raster_type(),
        group: 0,
        instance,
    };
    Ok((Some((id, bytes.clone())), Some(id)))
}

fn alloc_raster_instance(
    resources: &connector::Resources,
    menu_instance: u32,
    bytes: &[u8],
) -> u32 {
    let mut hash = Sha256::new();
    hash.update(b"openscp/menu-lot-mask");
    hash.update(menu_instance.to_be_bytes());
    hash.update(bytes);
    let mut instance = u32::from_be_bytes(hash.finalize()[..4].try_into().unwrap());
    while resources.contains_key(&ResourceId {
        type_id: raster_type(),
        group: 0,
        instance,
    }) {
        instance = instance.wrapping_add(1);
    }
    instance
}

/// 新 instance：标题内容哈希，对已存在 property TGI 线性探测。与构建器
/// 其它实例分配同契约（内容确定、冲突避让）。
fn alloc_menu_instance(spec: &MenuRegistrationSpec, resources: &connector::Resources) -> u32 {
    let mut instance = u32::from_be_bytes(
        Sha256::digest(spec.title.as_bytes())[..4]
            .try_into()
            .unwrap(),
    );
    while resources
        .keys()
        .any(|id| id.type_id == PROPERTY_TYPE && id.instance == instance)
    {
        instance = instance.wrapping_add(1);
    }
    instance
}

/// Lot 叶子 = 参照 lot property 克隆 + parent 改挂地标模板 + 与同 TGI
/// 覆盖一致的场景 lowering（单元列/LotSize/Placement/LOD 重定向）。
fn build_lot_leaf(
    spec: &MenuRegistrationSpec,
    lot_bytes: &[u8],
    instance: u32,
    scene_document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
    mask_key: Option<ResourceId>,
) -> Result<Vec<u8>, String> {
    let _ = instance;
    let mut file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("menu registration: reference lot is not a property: {error}"))?;
    // Keep the proven clean landmark Lot skeleton (R11/Blender comparison).
    // Reference lots may carry dozens of building-specific prop/path fields;
    // carrying those into a new menu leaf causes save-load crashes.
    const SAFE_LOT_HASHES: &[u32] = &[
        HASH_PARENT, 0x00F9EFBA, 0x00F9EFBB, 0x00F9EFBC, 0x00F9EFBD, 0x00F9EFBE,
        0x02E33A81, 0x09FB78CB, 0x0AFE2FEB, 0x0BBBCDED, 0x0BBBCE2A,
        0x0BBD5875, 0x0BF76A2A,
        0x0CCB7FC8, 0x0CCB7FC9, 0x0CCB7FD0, 0x0CCB7FD2, 0x0CCB7FD3,
        0x0CCB7FD4, 0x0CCB7FD5, 0x0CCB7FD6, 0x0D0082F1,
        0x0D02D586, 0x0D02D587, 0x0D02D588, 0x0D02D589, 0x0D02D58A, 0x0D02D58B,
        0x0D7AF042, 0x0D7AF043, 0x0D7AF044, 0x0D7AF045, 0x0D7AF046, 0x0D7AF047,
        0x0D7AF048, 0x0D7AF049,
    ];
    file.values.retain(|property| SAFE_LOT_HASHES.contains(&property.hash));
    set_scalar(
        &mut file,
        HASH_PARENT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: lot_group(),
            instance: spec.lot_template_instance.unwrap_or(LANDMARK_LOT_TEMPLATE),
        }),
    );
    if let Some(mask) = mask_key {
        set_scalar(
            &mut file,
            LOT_MASK_HASH,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: mask.type_id,
                group: mask.group,
                instance: mask.instance,
            }),
        );
    }
    // 保留作者编排的灯光与 prop 单元。prop 的 id 列由引擎脚本资源表解析，
    // 因此必须和新 Lot 一起写入；省略它会让树木等资产静默消失。
    let lights = scene_document
        .map(|document| SceneDocument {
            units: document
                .units
                .iter()
                .filter(|unit| matches!(
                    unit,
                    super::scene::SceneUnit::Light { .. }
                        | super::scene::SceneUnit::Prop { .. }
                ))
                .cloned()
                .collect(),
            ..SceneDocument::default()
        })
        .unwrap_or_default();
    super::scene::apply_units(&mut file, &lights)?;
    // LOD 重定向到编译模型；场景 LotSize/Placement 不降入（引用 lot 原值）。
    let lowered = super::package::apply_scene_lot_semantics(&encode(&file)?, None, model_ids)?;
    match lowered {
        Some((encoded, _)) => Ok(encoded),
        None => encode(&file),
    }
}

/// Unit 叶子 = 地标 cUnit 模板的最小覆盖叶（NightHouse 同形）：
/// parent 指模板、lot 链接指本 instance，其余字段运行时继承。
fn build_unit_leaf(spec: &MenuRegistrationSpec, instance: u32) -> Result<Vec<u8>, String> {
    let mut file = PropertyFile::default();
    set_scalar(
        &mut file,
        HASH_PARENT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: UNIT_TEMPLATE_GROUP,
            instance: spec
                .unit_template_instance
                .unwrap_or(LANDMARK_UNIT_TEMPLATE),
        }),
    );
    set_scalar(
        &mut file,
        HASH_UNIT_LOT_LINK,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: lot_group(),
            instance,
        }),
    );
    // NightHouse-capable cUnit leaf: effect/unlock chain.  The menu tooltip
    // and the simulation entry expect these alongside the lot link.
    set_scalar(
        &mut file,
        HASH_UNIT_EFFECT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: 0x437A_B00F,
        }),
    );
    set_scalar(
        &mut file,
        HASH_UNIT_UNLOCK_BIN,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: UNLOCK_CITY_HALL,
        }),
    );
    set_scalar(
        &mut file,
        HASH_UNIT_UNLOCK_AMOUNT,
        PropType::Float,
        Value::Float(1.0),
    );
    if let Some(cost) = spec.construction_cost {
        set_scalar(
            &mut file,
            HASH_CONSTRUCTION_COST,
            PropType::Int32,
            Value::Int32(cost),
        );
    }
    if let Some(cost) = spec.maintenance_cost {
        set_scalar(
            &mut file,
            HASH_MAINTENANCE_COST,
            PropType::Int32,
            Value::Int32(cost),
        );
    }
    encode(&file)
}

/// Menu2 = plop 根下新条目：归属边 uiCategories → 文化>地标，文案指向
/// 自有 locale 表，图标指向本 instance 的 PNG/JPG，menuUnit 指向 Unit。
fn build_menu2(spec: &MenuRegistrationSpec, instance: u32) -> Result<Vec<u8>, String> {
    let mut file = PropertyFile::default();
    set_scalar(
        &mut file,
        HASH_PARENT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: 0x48E1_EB00,
            instance: spec.menu_parent_instance.unwrap_or(PLOP_MENU_PARENT),
        }),
    );
    set_array(
        &mut file,
        HASH_UI_CATEGORY,
        PropType::Key,
        vec![Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: spec.category_instance.unwrap_or(LANDMARK_CATEGORY),
        })],
    );
    set_array(
        &mut file,
        HASH_UI_POSITION,
        PropType::Int32,
        vec![Value::Int32(spec.ui_position.unwrap_or(11500))],
    );
    set_array(
        &mut file,
        HASH_TITLE,
        PropType::Text,
        vec![Value::Text(Text {
            table_id: LOCALE_TABLE,
            instance_id: instance,
        })],
    );
    if spec.description.is_some() {
        set_array(
            &mut file,
            HASH_DESC,
            PropType::Text,
            vec![Value::Text(Text {
                table_id: LOCALE_TABLE,
                instance_id: description_id(instance),
            })],
        );
    }
    let icon_key = Value::Key(sc_properties::Key {
        type_id: ICON_TYPE,
        group: ICON_GROUP,
        instance,
    });
    set_scalar(&mut file, HASH_TOOL_ICON, PropType::Key, icon_key.clone());
    // NightHouse 的六态图标从 09756955 开始；09756950 不是它的字段。
    // 六态图标（NightHouse 实测槽位 09756955..5A）+ 背景图标 0x0C65D729
    //（NightHouse 与已验证 overlay 都带，缺它按钮背景/悬停缺资源）。
    for hash in [
        0x0975_6955u32,
        0x0975_6956,
        0x0975_6957,
        0x0975_6958,
        0x0975_6959,
        0x0975_695A,
        0x0C65_D729,
    ] {
        set_scalar(&mut file, hash, PropType::Key, icon_key.clone());
    }
    // 解锁条件（NightHouse 同款：市政厅 1 级；文案只引用原版表条目，
    // 不重定义原版表）。
    set_array(
        &mut file,
        HASH_UNLOCK_TARGET,
        PropType::Key,
        vec![Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: UNLOCK_CITY_HALL,
        })],
    );
    set_array(
        &mut file,
        HASH_UNLOCK_AMOUNT,
        PropType::Int32,
        vec![Value::Int32(1)],
    );
    set_array_with_flags(
        &mut file,
        HASH_UNLOCK_STRING,
        PropType::Text,
        vec![Value::Text(Text {
            table_id: 0x4B54_417A,
            instance_id: 0x0DF1_9F95,
        })],
        0x809C,
    );
    // NightHouse-shaped statistics row and park flag.  These are not cosmetic:
    // the tooltip builder reads both fields while constructing the tool entry.
    set_array_with_flags(
        &mut file,
        HASH_EFFECT_TITLE,
        PropType::Text,
        vec![Value::Text(Text {
            table_id: 0x50AA_0BEA,
            instance_id: 0x0EBC_E29A,
        })],
        0x809C,
    );
    set_array(
        &mut file,
        HASH_EFFECT_PARAM,
        PropType::UInt32,
        vec![Value::UInt32(0x0DA3_EE15)],
    );
    set_scalar(
        &mut file,
        HASH_PARK_FLAG,
        PropType::Bool,
        Value::Bool(false),
    );
    set_scalar(
        &mut file,
        HASH_MARQUEE,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: MARQUEE_TYPE,
            group: 0,
            instance,
        }),
    );
    // plop 经济链入口：cToolPlop::FillFromProps 按它求和造价（T/G=0 合法）。
    set_scalar(
        &mut file,
        HASH_MENU_UNIT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance,
        }),
    );
    encode(&file)
}

/// locale 表资源：BOM + 宽松排版 JSON（实机验证：紧凑单行 JSON 会被整表丢弃）。
fn build_locale(spec: &MenuRegistrationSpec, instance: u32) -> Vec<u8> {
    // Author convention: the table carries fixed rows 0x00000001 (title) and
    // 0x00000002 (description) and every text edge points at those ids.
    // Keying rows by the menu instance (old workbench convention) makes the
    // game's lookup miss and render `***` placeholders.
    let _ = instance;
    let mut lines = vec![format!(
        "    \"0x00000001\" : {}",
        serde_json::to_string(spec.title.trim()).unwrap_or_default()
    )];
    if let Some(description) = &spec.description {
        lines.push(format!(
            "    \"0x00000002\" : {}",
            serde_json::to_string(description.trim()).unwrap_or_default()
        ));
    }
    let json = format!("{{\r\n{}\r\n}}", lines.join(",\r\n"));
    let mut bytes = Vec::with_capacity(json.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(json.as_bytes());
    bytes
}

/// Overwrite the materialized generic menu with this asset's identity:
/// category/position (schema), texts (own locale table), icons/marquee
/// (own instances) and the type-less menuUnit edge — the same fields the
/// author's `menu_record` + `build_candidate` overwrite after materializing.
fn apply_menu_overrides(
    file: &mut PropertyFile,
    spec: &MenuRegistrationSpec,
    instance: u32,
) -> Result<(), String> {
    set_array(
        file,
        HASH_UI_CATEGORY,
        PropType::Key,
        vec![Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: spec.category_instance.unwrap_or(LANDMARK_CATEGORY),
        })],
    );
    set_array(
        file,
        HASH_UI_POSITION,
        PropType::Int32,
        vec![Value::Int32(spec.ui_position.unwrap_or(11500))],
    );
    set_text_table(file, HASH_TITLE, LOCALE_TABLE, 1);
    set_text_table(file, HASH_DESC, LOCALE_TABLE, 2);
    // menuUnit: type-less key to the new Unit leaf (author `key(0x0975695f,(0,0,main_id))`).
    set_scalar(
        file,
        HASH_MENU_UNIT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance,
        }),
    );
    let icon_key = Value::Key(sc_properties::Key {
        type_id: ICON_TYPE,
        group: ICON_GROUP,
        instance,
    });
    set_scalar(file, HASH_TOOL_ICON, PropType::Key, icon_key.clone());
    for hash in [
        0x0975_6955u32,
        0x0975_6956,
        0x0975_6957,
        0x0975_6958,
        0x0975_6959,
        0x0975_695A,
        0x0C65_D729,
    ] {
        set_scalar(file, hash, PropType::Key, icon_key.clone());
    }
    set_scalar(
        file,
        HASH_MARQUEE,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: MARQUEE_TYPE,
            group: 0,
            instance,
        }),
    );
    // Keep every other materialized chain field verbatim — the park flag
    // (0EC480DF) and presentation rows are part of the proven-visible
    // golden menu shape; stripping them makes the entry invisible.
    Ok(())
}

fn description_id(instance: u32) -> u32 {
    instance.wrapping_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clone_lot_mask_allocates_a_new_raster_identity() {
        let old = 0x4E23_544D;
        let lot = PropertyFile {
            values: vec![Property {
                hash: LOT_MASK_HASH,
                prop_type: PropType::Key,
                kind: Kind::Scalar(Value::Key(sc_properties::Key {
                    type_id: 0,
                    group: 0,
                    instance: old,
                })),
                encoding: PropertyEncoding::default(),
            }],
            claimed_count: 1,
        }
        .encode_canonical()
        .unwrap();
        let mut resources = connector::Resources::new();
        resources.insert(
            ResourceId {
                type_id: raster_type(),
                group: 0,
                instance: old,
            },
            vec![1, 2, 3, 4],
        );
        let (entry, key) = clone_lot_mask(&resources, &lot, 0x1234_5678).unwrap();
        let (id, bytes) = entry.expect("mask entry");
        let key = key.expect("mask key");
        assert_ne!(key.instance, old, "new identity must not reuse the source mask TGI");
        assert_eq!(key, id);
        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }
}

/// 128×128 工具图标 PNG：instance 决定的双色棋盘（测试用占位，可换
/// `iconPng` 真图）。
fn build_icon_png(spec: &MenuRegistrationSpec, instance: u32) -> Vec<u8> {
    if let Some(path) = &spec.icon_png {
        if let Ok(bytes) = std::fs::read(path) {
            return bytes;
        }
    }
    let (a, b) = palette(instance);
    let mut rgba = Vec::with_capacity(128 * 128 * 4);
    for y in 0..128u32 {
        for x in 0..128u32 {
            let base = if (x / 16 + y / 16) % 2 == 0 { a } else { b };
            let border = x < 2 || y < 2 || x >= 126 || y >= 126;
            let px = if border { [20, 20, 24, 255] } else { base };
            rgba.extend_from_slice(&px);
        }
    }
    let mut png = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut png));
    image::ImageEncoder::write_image(encoder, &rgba, 128, 128, image::ExtendedColorType::Rgba8)
        .expect("icon png encode");
    png
}

/// 454×263 rollover 大图 JPG：同色系横幅（测试占位）。
fn build_marquee_jpg(instance: u32) -> Vec<u8> {
    let (a, b) = palette(instance);
    let mut rgb = Vec::with_capacity(454 * 263 * 3);
    for y in 0..263u32 {
        for x in 0..454u32 {
            let t = (x * 3 + y) as f32 / (454.0 * 3.0 + 263.0);
            rgb.extend_from_slice(&[
                (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
                (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
                (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
            ]);
        }
    }
    let mut jpg = Vec::new();
    let encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::Cursor::new(&mut jpg), 88);
    image::ImageEncoder::write_image(encoder, &rgb, 454, 263, image::ExtendedColorType::Rgb8)
        .expect("marquee jpg encode");
    jpg
}

fn palette(seed: u32) -> ([u8; 4], [u8; 4]) {
    let hash = Sha256::digest(seed.to_be_bytes());
    let mix = |i: usize, base: u8| hash[i].saturating_add(base) / 2 + 40;
    (
        [mix(0, 60), mix(1, 90), mix(2, 140), 255],
        [mix(3, 20), mix(4, 30), mix(5, 50), 255],
    )
}

fn set_scalar(file: &mut PropertyFile, hash: u32, prop_type: PropType, value: Value) {
    match file
        .values
        .iter_mut()
        .find(|property| property.hash == hash)
    {
        Some(property) => {
            property.kind = Kind::Scalar(value);
            property.prop_type = prop_type;
        }
        None => file.values.push(Property {
            hash,
            prop_type,
            kind: Kind::Scalar(value),
            encoding: PropertyEncoding::default(),
        }),
    }
}

fn set_array(file: &mut PropertyFile, hash: u32, prop_type: PropType, values: Vec<Value>) {
    let item_size = match prop_type {
        PropType::Key => Some(12),
        PropType::Text => Some(sc_properties::TEXT_ARRAY_ITEM_SIZE),
        PropType::Int32 | PropType::UInt32 | PropType::Float => Some(4),
        _ => None,
    };
    match file
        .values
        .iter_mut()
        .find(|property| property.hash == hash)
    {
        Some(property) => {
            property.kind = Kind::Array(values);
            property.prop_type = prop_type;
            property.encoding.array_item_size = item_size;
        }
        None => file.values.push(Property {
            hash,
            prop_type,
            kind: Kind::Array(values),
            encoding: PropertyEncoding {
                flags: 0,
                array_item_size: item_size,
            },
        }),
    }
}

fn set_array_with_flags(
    file: &mut PropertyFile,
    hash: u32,
    prop_type: PropType,
    values: Vec<Value>,
    flags: u16,
) {
    set_array(file, hash, prop_type, values);
    if let Some(property) = file.values.iter_mut().find(|property| property.hash == hash) {
        property.encoding.flags = flags;
    }
}

fn encode(file: &PropertyFile) -> Result<Vec<u8>, String> {
    // The game rejects a property list containing the same hash twice.  The
    // workbench can legitimately set a field that the donor already carried
    // (for example 0DC1E3E0), so keep the last write per hash.
    let mut values: Vec<Property> = Vec::with_capacity(file.values.len());
    for property in &file.values {
        if let Some(existing) = values.iter_mut().find(|item| item.hash == property.hash) {
            *existing = property.clone();
        } else {
            values.push(property.clone());
        }
    }
    let mut deduped = file.clone();
    deduped.values = values;
    deduped
        .encode_canonical()
        .map_err(|error| format!("menu property encode: {error}"))
}
