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
use super::{connector, scene::SceneDocument};
use dbpf::ResourceId;
use sc_properties::{Kind, PropType, Property, PropertyEncoding, PropertyFile, Text, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// 工具条目组（Menu2 所在组；地标批在 SimCity_Graphics 包）。
pub const MENU_GROUP: u32 = 0x09878_A01;
/// cUnit 叶子组 / Lot 资产配置组 / 模板组（Unit 模板在 48E0C000）。
pub const UNIT_GROUP: u32 = 0x40E0_C000;
pub const LOT_GROUP: u32 = 0x40E1_C000;
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
const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
const HASH_ICON_NORMAL: u32 = 0x0975_6950;
const HASH_MENU_UNIT: u32 = 0x0975_695F;
const HASH_TOOL_ICON: u32 = 0x0977_AA8F;
const HASH_UI_CATEGORY: u32 = 0x0DB9_FC63;
const HASH_UI_POSITION: u32 = 0x0DC1_E3E0;
const HASH_MARQUEE: u32 = 0x0DDE_EE56;
const HASH_UNLOCK_TARGET: u32 = 0x0DE8_4DC9;
const HASH_UNLOCK_AMOUNT: u32 = 0x0DE8_4DD3;
const HASH_UNLOCK_STRING: u32 = 0x0DE8_4DDC;
/// kResourceIDHQ_CityHall_IsOn（市政厅启用；NightHouse/威利斯同款解锁位）。
const UNLOCK_CITY_HALL: u32 = 0xC93F_D4B5;
// Unit 叶子哈希。
const HASH_UNIT_LOT_LINK: u32 = 0x08E1_7ED7;
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
    model_id: Option<ResourceId>,
) -> Result<MenuResources, String> {
    let instance = alloc_menu_instance(spec, resources);
    let lot_bytes = resources.get(&lot_source).ok_or_else(|| {
        format!("menu registration: reference lot {lot_source} missing from source package")
    })?;
    let lot_leaf = build_lot_leaf(spec, lot_bytes, instance, scene_document, model_id)?;
    let unit_leaf = build_unit_leaf(spec, instance)?;
    let menu2 = build_menu2(spec, instance)?;
    let locale = build_locale(spec, instance);
    let icon_png = build_icon_png(spec, instance);
    let marquee_jpg = build_marquee_jpg(instance);

    let entries = vec![
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
                group: LOT_GROUP,
                instance,
            },
            lot_leaf,
        ),
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
    Ok(MenuResources { instance, entries })
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
    model_id: Option<ResourceId>,
) -> Result<Vec<u8>, String> {
    let _ = (instance, scene_document);
    let mut file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("menu registration: reference lot is not a property: {error}"))?;
    // Keep the proven clean landmark Lot skeleton (R11/Blender comparison).
    // Reference lots may carry dozens of building-specific prop/path fields;
    // carrying those into a new menu leaf causes save-load crashes.
    const SAFE_LOT_HASHES: &[u32] = &[
        HASH_PARENT, 0x00F9EFBA, 0x00F9EFBB, 0x00F9EFBC, 0x00F9EFBD, 0x00F9EFBE,
        0x02E33A81, 0x09FB78CB, 0x0AFE2FEB, 0x0BBD5875, 0x0BF76A2A,
        0x0CCB7FC8, 0x0CCB7FC9, 0x0CCB7FD0, 0x0CCB7FD2, 0x0CCB7FD3,
        0x0CCB7FD4, 0x0CCB7FD5, 0x0D0082F1,
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
            group: LOT_GROUP,
            instance: spec.lot_template_instance.unwrap_or(LANDMARK_LOT_TEMPLATE),
        }),
    );
    // 单元列全清（NightHouse 式干净 lot）：场景 prop 的直挂模型 id 在游戏
    // 里按「建筑脚本资源表条目」解析（PROP_ID 列语义，unit-props §1.1），
    // 工作台直挂 id 不在任何脚本资源表 → 原样携带会在加载存档时崩
    // （2026-10-31 实测）。装饰 prop 等 M2 脚本资源表能力后再恢复。
    super::scene::apply_units(&mut file, &super::scene::SceneDocument::default())?;
    // LOD 重定向到编译模型；场景 LotSize/Placement 不降入（引用 lot 原值）。
    let lowered = super::package::apply_scene_lot_semantics(&encode(&file)?, None, model_id)?;
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
            group: LOT_GROUP,
            instance,
        }),
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
    set_scalar(
        &mut file,
        HASH_UI_POSITION,
        PropType::Int32,
        Value::Int32(spec.ui_position.unwrap_or(11500)),
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
    set_scalar(&mut file, HASH_ICON_NORMAL, PropType::Key, icon_key.clone());
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
    set_array(
        &mut file,
        HASH_UNLOCK_STRING,
        PropType::Text,
        vec![Value::Text(Text {
            table_id: 0x4B54_417A,
            instance_id: 0x0DF1_9F95,
        })],
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
    let mut lines = vec![format!(
        "    \"0x{instance:08x}\" : {}",
        serde_json::to_string(spec.title.trim()).unwrap_or_default()
    )];
    if let Some(description) = &spec.description {
        lines.push(format!(
            "    \"0x{:08x}\" : {}",
            description_id(instance),
            serde_json::to_string(description.trim()).unwrap_or_default()
        ));
    }
    let json = format!("{{\r\n{}\r\n}}", lines.join(",\r\n"));
    let mut bytes = Vec::with_capacity(json.len() + 3);
    bytes.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
    bytes.extend_from_slice(json.as_bytes());
    bytes
}

fn description_id(instance: u32) -> u32 {
    instance.wrapping_add(1)
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

fn encode(file: &PropertyFile) -> Result<Vec<u8>, String> {
    file.encode_canonical()
        .map_err(|error| format!("menu property encode: {error}"))
}
