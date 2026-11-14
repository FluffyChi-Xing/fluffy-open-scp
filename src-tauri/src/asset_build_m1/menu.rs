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
use super::property_edit::{encode, set_array, set_array_with_flags, set_scalar};
use super::{connector, materials::raster_type, package::LodModelIds, scene::SceneDocument};
use dbpf::ResourceId;
use sc_properties::{
    Key, Kind, PropType, Property, PropertyEncoding, PropertyFile, Text, Value,
    LOD_MODEL_HASHES, LOT_MASK_HASH, LOT_SIZE_HASH,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROPERTY_TYPE: u32 = 0x00B1_B104;
/// 图形单元组：建筑的"非模拟属性"载体（模型引用 / BoundingBox / UnitLocation /
/// 亮灯开关 0x0E0BE186 lightswhenactive 等）。引擎 cGraphicsUnitInteriors 经
/// FindUnitNonSimProps 在该组按图形状态 ID 检索——缺失 = 夜景永不亮。
pub const GRAPHICS_UNIT_GROUP: u32 = 0x42E1_C000;
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
/// 遗留共享 locale 表（FNV-1("openscp/strings")）。2026-11-08 起各路由一律
/// 改写身份自有表（instance=身份实例）；该常量仅描述已部署旧包（如
/// Commercial_v7）占用的表，新代码不得再写入。
pub const LOCALE_TABLE: u32 = 0xBF82_F7CE;
pub const LOCALE_TYPE: u32 = 0x0A98_EAF0;
pub const LOCALE_GROUP: u32 = 0x02FA_BF01;

// Menu2 property 哈希（menu-export.ts HASH 表同源）。
pub(crate) const HASH_PARENT: u32 = 0x00B2_CCCB;
/// Dalian’s proven full identity instance (Menu2/Unit/Lot share it).
pub const DALIAN_IDENTITY_INSTANCE: u32 = 0xF922_1FCF;
/// Dalian’s locale table id (existing game table, safe to override).
pub const DALIAN_LOCALE_TABLE: u32 = 0x284C_6727;
pub(crate) const HASH_TITLE: u32 = 0x0A09_F5FA;
const HASH_DESC: u32 = 0x0A09_F5FB;
pub(crate) const HASH_MENU_UNIT: u32 = 0x0975_695F;
/// Menu2 工具对象类别（原版 plop 建筑统一 `C710B6E9` kCategoryIDBuilding；
/// 见 docs/re/building-eco-chain-and-s17-diagnosis-20261106.md §5.3 R1）。
pub(super) const HASH_BUILDING_CATEGORY: u32 = 0x0975_695E;
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
pub(crate) const EDUCATION_BEHAVIOR_CATEGORY: u32 = 0x95A9_D6FD;
const PARK_BEHAVIOR_CATEGORY: u32 = 0x0704_9250;
pub(crate) const ELEMENTARY_TEMPLATE: u32 = 0x7E4B_99E5;
/// kResourceIDHQ_CityHall_IsOn（市政厅启用；NightHouse/威利斯同款解锁位）。
const UNLOCK_CITY_HALL: u32 = 0xC93F_D4B5;
// Unit 叶子哈希。
const HASH_UNIT_LOT_LINK: u32 = 0x08E1_7ED7;
const HASH_UNIT_EFFECT: u32 = 0x0AFE_27CA;
const HASH_UNIT_UNLOCK_BIN: u32 = 0x0BF5_D2E3;
const HASH_UNIT_UNLOCK_AMOUNT: u32 = 0x0BF5_D2E4;
const HASH_CONSTRUCTION_COST: u32 = 0x0A8A_41DA;
const HASH_MAINTENANCE_COST: u32 = 0x09AE_19D7;
pub(crate) const HASH_UNIT_CATEGORY: u32 = 0x0AFE_27B1;
pub(crate) const HASH_STUDENT_CAPACITY: u32 = 0x0AD7_FAC9;

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
    /// uiCategories 多容器数组（2026-11-06 菜单编辑节点组）：同一建筑身份
    /// 挂到多个菜单容器。0x0DB9FC63 本就是 Key 数组，非空时整体替代
    /// `category_instance`；placement 与建筑功能（behavior_category_instance）
    /// 保持正交。
    #[serde(default, deserialize_with = "de_hash_vec")]
    pub category_instances: Vec<u32>,
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
    /// 菜单背景图（rollover 454×263 JPG）的项目内路径；缺省自动生成占位图。
    #[serde(default)]
    pub marquee_png: Option<String>,
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
    /// Menu2 工具对象类别（0x0975695E）。原版一切 plop 建筑工具都带
    /// `C710B6E9`（kCategoryIDBuilding）；缺省不写（与旧行为兼容）。s17 诊断
    /// 的 R1 判别实验要求公园菜单 + plop 家族时显式携带该字段。
    #[serde(default, deserialize_with = "de_hash")]
    pub building_category_instance: Option<u32>,
    /// Lot 城市看板绑定（Asset 0x0D7970ED，Key）。游戏在 UnitCreate 时读取
    /// 该值把单元槽位挂进 `mGameUI->mCachedUnitSlotHashMap`（UnitDestroy 对称
    /// 注销）——人口地图/面板聚合按看板查询。原版小学 = 教育容器
    /// `6A1785B4`；交互链恢复会把公园残留值（D0621548）覆盖上来，教育类
    /// 建筑必须显式声明。缺省不写（与旧行为兼容）。
    #[serde(default, deserialize_with = "de_hash")]
    pub lot_board_instance: Option<u32>,
    /// Unit 叶子的 cUnit 模板 instance（组 48E0C000）；默认地标模板
    /// `4E01E311`。公园测试可用 NightHouse 实证对 `565B72B9`。
    #[serde(default, deserialize_with = "de_hash")]
    pub unit_template_instance: Option<u32>,
    /// Optional runtime behavior template parent. This is independent from
    /// menu category: e.g. a diagnostic item may live under the park menu
    /// while inheriting the elementary-school cUnit behavior chain.
    #[serde(default, deserialize_with = "de_hash")]
    pub simulation_template_instance: Option<u32>,
    /// Explicit runtime behavior category written to the Unit leaf. This is
    /// deliberately independent from `category_instance`, which only controls
    /// the visible Menu2 category. When omitted, the native profile derives a
    /// safe default from the selected Unit donor.
    #[serde(default, deserialize_with = "de_hash")]
    pub behavior_category_instance: Option<u32>,
    /// Built-in Unit property donor. The donor preserves the complete
    /// simulation/education field chain while the builder rewrites identity
    /// links. Supported values are `elementary-school`, `generic-park`, and
    /// `complete-building`. Prefer letting `behavior_family` fill this from
    /// workbench leaves instead of hard-coding Building40 → university.
    #[serde(default)]
    pub unit_donor: Option<String>,
    /// Menu2 骨架：`generic`（扁平通用）或 `complete-building`（s31 大学菜单）。
    /// 由行为家族适配器写入；缺省时 Building40 仍回落 complete-building（兼容旧 schema）。
    #[serde(default)]
    pub menu_skeleton: Option<String>,
    /// Optional elementary-school desk/student capacity override. This maps to
    /// the verified `0x0AD7FAC9` Unit field; it is not a worker/job count.
    #[serde(default)]
    pub student_capacity: Option<i32>,
    /// Whether to activate the shared dynamic tree-bin bundle. Static scene
    /// tree props already occupy the Lot BinDraw columns, so this must be
    /// false when the asset intentionally owns a fixed tree count.
    #[serde(default = "default_tree_decoration")]
    pub tree_decoration: bool,
    /// Lot 叶子的 Lot 模板 instance（组 40E1C000）；默认地标模板
    /// `4E01E311`。公园测试可用 NightHouse 实证对 `34CE7E14`。
    #[serde(default, deserialize_with = "de_hash")]
    pub lot_template_instance: Option<u32>,
    /// 0.8.0 parent-free 身份路线：Asset 字典 = 通用链 40E1C000:9EC474FF
    /// 物化 + 地面 Lot 数字字段 + prop/light 列；Unit = tested-unit.bin
    /// 克隆；不再克隆参照 lot。`reference.lot` 此时充当 ground 来源。
    #[serde(default)]
    pub parent_free: bool,
    /// 模型/场景几何朝向（Building40 打包旋转 mesh/地面/prop）。
    /// 与临街吸附边正交——地块 tab 不得写入本字段。
    #[serde(default)]
    pub rotation_degrees: Option<u32>,
    /// 临街吸附边（lot 本地角：0=+Y / 90=−X / 180=−Y / 270=+X）。
    /// 只表达「哪条边临街」，**不**驱动几何旋转。
    #[serde(default)]
    pub street_facing_degrees: Option<u32>,
}

fn default_tree_decoration() -> bool {
    true
}

/// instance 字段同时接受 JSON 数字与 `"0xXXXXXXXX"` 十六进制字符串。
pub(crate) fn de_hash<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
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

/// instance 字段数组版：接受 JSON 数字与 `"0x…"` 十六进制字符串的混排。
pub(crate) fn de_hash_vec<'de, D>(deserializer: D) -> Result<Vec<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    let values = <Vec<serde_json::Value> as Deserialize>::deserialize(deserializer)?;
    values
        .into_iter()
        .map(|value| match value {
            serde_json::Value::Number(number) => number
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| serde::de::Error::custom("instance out of u32 range")),
            serde_json::Value::String(text) => u32::from_str_radix(
                text.trim()
                    .trim_start_matches("0x")
                    .trim_start_matches("0X"),
                16,
            )
            .map_err(|_| serde::de::Error::custom(format!("invalid hex instance {text:?}"))),
            other => Err(serde::de::Error::custom(format!(
                "expected u32 or hex string, got {other}"
            ))),
        })
        .collect()
}

/// uiCategories Key 数组：`category_instances` 非空时整体展开（多容器挂载），
/// 否则回退单 `category_instance`（再回退 `default_instance`）。
fn ui_category_values(spec: &MenuRegistrationSpec, default_instance: u32) -> Vec<Value> {
    let default = [default_instance];
    spec.ui_categories()
        .unwrap_or(&default)
        .iter()
        .copied()
        .map(|instance| {
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance,
            })
        })
        .collect()
}

impl MenuRegistrationSpec {
    pub(super) fn ui_categories(&self) -> Option<&[u32]> {
        if self.category_instances.is_empty() {
            self.category_instance.as_ref().map(std::slice::from_ref)
        } else {
            Some(&self.category_instances)
        }
    }

    /// 从 `SemanticsIr.menu`（字符串键 JSON 表）解析并校验。
    pub fn from_values(
        values: &std::collections::BTreeMap<String, serde_json::Value>,
    ) -> Result<Self, String> {
        let value =
            serde_json::Value::Object(values.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let mut spec: MenuRegistrationSpec = serde_json::from_value(value)
            .map_err(|error| format!("invalid menu registration: {error}"))?;
        if spec.title.trim().is_empty() {
            return Err("menu registration requires a non-empty title".into());
        }
        super::street_placement::compensation(spec.street_facing_degrees)?;
        // The editor's Parks root resolves to Basic Parks in the Packager's menu_locations.py.
        let canonical = |id| if id == 0x899B_EA70 { 0x0704_9250 } else { id };
        spec.category_instance = spec.category_instance.map(canonical);
        let mut seen = std::collections::HashSet::new();
        for id in &mut spec.category_instances {
            *id = canonical(*id);
        }
        spec.category_instances.retain(|id| seen.insert(*id));
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
    scene_writes: &super::scene::SceneWrites,
    model_ids: Option<LodModelIds>,
    ground_source: Option<(&connector::Resources, ResourceId)>,
    identity: Option<&connector::Resources>,
    root: &std::path::Path,
    author_building40: bool,
) -> Result<MenuResources, String> {
    let instance = alloc_menu_instance(spec, resources);
    // Building40 always uses the complete parent-free property recipe. The old
    // minimal clone route dropped the panel and utility fields even though its
    // identity-link checks passed. Other geometry routes retain their bytes.
    if spec.parent_free || author_building40 {
        if author_building40 && identity.is_some() {
            return Err(
                "Building40 does not accept a legacy identity donor; use the complete parent-free property recipe"
                    .into(),
            );
        }
        return generate_parent_free(
            spec,
            resources,
            lot_source,
            instance,
            scene_document,
            model_ids,
            ground_source,
            root,
            author_building40,
        );
    }
    if super::street_placement::compensation(spec.street_facing_degrees)? != 0 {
        return Err("streetFacingDegrees requires the parent-free building route".into());
    }
    let lot_bytes = resources.get(&lot_source).ok_or_else(|| {
        format!("menu registration: reference lot {lot_source} missing from source package")
    })?;
    let (mask_entry, mask_key) = lot_mask_for_scene(
        resources,
        lot_bytes,
        scene_document,
        instance,
        ground_source,
    )?;
    let (menu2, unit_leaf, lot_leaf, locale_table) = if let Some(identity) = identity {
        clone_donor_identity(
            spec,
            identity,
            lot_bytes,
            instance,
            mask_key,
            model_ids,
            scene_document,
            scene_writes,
        )?
    } else {
        let lot_leaf = build_lot_leaf(
            spec,
            lot_bytes,
            instance,
            scene_document,
            scene_writes,
            model_ids,
            mask_key,
            model_bounds(resources, model_ids),
        )?;
        (
            build_menu2(spec, instance)?,
            build_unit_leaf(spec, instance)?,
            lot_leaf,
            // Locale resource rides the identity instance (per-identity
            // table, 19a1b3a) so two packages can never shadow each other's
            // title/description via one shared table TGI.
            instance,
        )
    };
    let locale = if identity.is_some() {
        build_donor_locale(spec, instance, locale_table)
    } else {
        build_locale(spec, instance)
    };
    let icon_png = build_icon_png(spec, instance, root)?;
    let marquee_jpg = build_marquee_jpg(spec, instance, root)?;

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
    // A donor identity may live in a package that the user intentionally
    // removes after deployment. Carry its graphical closure into the new
    // package so the Lot remains renderable without the donor package.
    if let Some(identity) = identity {
        for (id, bytes) in identity {
            if id.type_id == 0x2F4E_681B
                || (id.type_id == 0x2F4E_681C && id.instance == 0x0186_1E39)
                    && !entries.iter().any(|(entry_id, _)| entry_id == id)
            {
                entries.push((*id, bytes.clone()));
            }
        }
    }
    Ok(MenuResources { instance, entries })
}

/// Parent-free identity route. Building40 uses the complete dictionaries from
/// the reference candidate (package SHA-256
/// 50ec8c71b3647ebe3ef8409f3259664c731127641293cbff317bd2ce52398530),
/// then overlays this asset's identity, tourism/park semantics and scene.
fn generate_parent_free(
    spec: &MenuRegistrationSpec,
    resources: &connector::Resources,
    ground_lot: ResourceId,
    instance: u32,
    scene_document: Option<&SceneDocument>,
    model_ids: Option<LodModelIds>,
    ground_source: Option<(&connector::Resources, ResourceId)>,
    root: &std::path::Path,
    author_building40: bool,
) -> Result<MenuResources, String> {
    // Building40 workbench scenes use the game lot frame directly. The old
    // 180° fallback is retained for legacy routes only; applying it here
    // flips authored paths and makes a +Y street face appear at -Y in game.
    let rotation = spec
        .rotation_degrees
        .unwrap_or(if author_building40 { 0 } else { 180 });
    // 家族适配器优先；未声明时 Building40 旧默认仍为 complete-building。
    let skeleton = spec
        .menu_skeleton
        .as_deref()
        .unwrap_or(if author_building40 {
            "complete-building"
        } else {
            "generic"
        });
    let menu2 = if skeleton == "complete-building" {
        let mut file = PropertyFile::parse(include_bytes!(
            "../../resources/sc5/complete-building-menu.bin"
        ))
        .map_err(|error| format!("complete-building-menu.bin parse: {error}"))?;
        patch_instance_refs(&mut file, 0x4C0E_8889, instance);
        apply_menu_overrides(&mut file, spec, instance)?;
        encode(&file)?
    } else {
        match super::native_profile::flat_chain(resources, super::native_profile::GENERIC_MENU) {
            Ok((mut file, _)) => {
                apply_menu_overrides(&mut file, spec, instance)?;
                encode(&file)?
            }
            Err(_) => build_menu2(spec, instance)?,
        }
    };
    let mut native = super::native_profile::build(
        spec,
        resources,
        ground_lot,
        ground_source,
        instance,
        scene_document,
        model_ids.as_ref(),
        rotation,
        author_building40,
    )?;
    let placement = super::street_placement::apply(
        &mut native.asset,
        super::street_placement::compensation(spec.street_facing_degrees)?,
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
    let icon_png = build_icon_png(spec, instance, root)?;
    let marquee_jpg = build_marquee_jpg(spec, instance, root)?;
    // 图形单元资源（组 42E1C000）：**所有 parent-free 建筑默认携带**。
    // 引擎 cGraphicsUnitInteriors::FillFromProps（0x7D4690）从该资源的非模拟
    // 属性读亮灯开关 0x0E0BE186 lightswhenactive——缺失 = 夜景永不亮
    // （2026-11-11 希望大厦夜景全黑的根因；19/19 原版地标图形单元都带 true）。
    // 内容对齐原版形态：parent→自身 Lot、BoundingBox、模型引用 ×4、开关。
    let graphics_unit = {
        let mut file = PropertyFile::default();
        if let Some(placement) = &placement {
            super::street_placement::write(&mut file, placement);
        }
        set_scalar(
            &mut file,
            0x00B2_CCCB,
            PropType::Key,
            Value::Key(Key {
                type_id: PROPERTY_TYPE,
                group: lot_group(),
                instance,
            }),
        );
        if model_ids.is_some() {
            write_graphics_bounds(&mut file, &native.asset)?;
        }
        for (offset, lod) in model_ids.iter().flatten().enumerate() {
            set_scalar(
                &mut file,
                0x00F9EFBB + offset as u32,
                PropType::Key,
                Value::Key(sc_properties::Key {
                    type_id: lod.type_id,
                    group: lod.group,
                    instance: lod.instance,
                }),
            );
        }
        super::interior_lighting::write_graphics_defaults(&mut file);
        let encoded = encode(&file)?;
        // 构建期硬门槛（防第 N 次回归）：图形单元必须同时声明两个内景开关。
        // 缺失任一条 = 窗灯必黑（SC5 报告《第九阶段》0x0081D020/0x0081CD60），
        // 因此在这里直接 fail，而不是等进游戏肉眼发现。
        verify_interior_controller_contract(&encoded)?;
        if model_ids.is_some() {
            verify_graphics_bounds(&encoded, &native.asset)?;
        }
        encoded
    };
    let encoded_asset = encode(&native.asset)?;
    super::native_profile::verify_parking_slots(&encoded_asset, scene_document)?;
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
        (asset_id, encoded_asset),
        (
            ResourceId {
                type_id: PROPERTY_TYPE,
                group: GRAPHICS_UNIT_GROUP,
                instance,
            },
            graphics_unit,
        ),
        (
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance,
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
    scene_writes: &super::scene::SceneWrites,
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

    // A static-tree asset must not inherit the shared dynamic tree-bin bundle
    // from a previous donor. The scene document already owns its fixed tree
    // prop instances; retaining this bundle would draw a second population.
    if !spec.tree_decoration {
        // Donors use both the legacy 0x0A0209A5 bundle list and the
        // repeated 0x0A0209A2 form. Remove the dynamic tree bundle from
        // either encoding; the authored lot owns its fixed tree count.
        for property in unit
            .values
            .iter_mut()
            .filter(|p| matches!(p.hash, 0x0A02_09A2 | 0x0A02_09A5))
        {
            if let Kind::Array(values) = &mut property.kind {
                values.retain(
                    |value| !matches!(value, Value::Key(key) if key.instance == 0xE769_0450),
                );
            }
        }
    }

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
        ui_category_values(spec, 0x0704_9250),
    );
    // 身份路线的菜单嵌套/工具家族：显式声明时覆盖 donor 的 parent 与
    // 工具对象类别（s12/s13 公园入口配方：F970A822 + C710B6E9）。
    if let Some(parent) = spec.menu_parent_instance {
        set_scalar(
            &mut menu,
            HASH_PARENT,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: PROPERTY_TYPE,
                group: 0x48E1_EB00,
                instance: parent,
            }),
        );
    }
    if let Some(building_category) = spec.building_category_instance {
        set_scalar(
            &mut menu,
            HASH_BUILDING_CATEGORY,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: building_category,
            }),
        );
    }
    set_text_table(&mut menu, HASH_TITLE, locale_table, 1);
    set_text_table(&mut menu, HASH_DESC, locale_table, 2);

    // The donor menu is park-shaped even though the Unit already carries the
    // education category. Rewrite the presentation family on the new Menu2;
    // otherwise the game displays the item and its detail panel as a park.
    if spec.behavior_category_instance == Some(EDUCATION_BEHAVIOR_CATEGORY)
        || spec.unit_donor.as_deref() == Some("elementary-school")
        || spec.simulation_template_instance == Some(ELEMENTARY_TEMPLATE)
    {
        set_scalar(
            &mut menu,
            HASH_PARK_FLAG,
            PropType::Bool,
            Value::Bool(false),
        );
        // 统计行必须与 s31 GOLDEN 同形（3 文本 + 3 参数）：公园模板的 2 行
        // 含需参数替换的格式串行（0EBCE055），缺参数时游戏原样打印
        // `~population_amount:number~`（2026-11-08 菜单乱码事故）。
        set_effect_stat_rows(&mut menu);
    }
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

    // Behavior binding is an explicit overlay on the frozen donor identity.
    // Keep menu placement and all scene resources untouched; only the Unit
    // runtime selector/template/capacity are allowed to change here.
    let behavior_category = spec.behavior_category_instance.or_else(|| {
        (spec.unit_donor.as_deref() == Some("elementary-school"))
            .then_some(EDUCATION_BEHAVIOR_CATEGORY)
    });
    if let Some(category) = behavior_category {
        set_scalar(
            &mut unit,
            HASH_UNIT_CATEGORY,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: category,
            }),
        );
    }
    if let Some(template) = spec.simulation_template_instance {
        set_scalar(
            &mut unit,
            HASH_PARENT,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: PROPERTY_TYPE,
                group: UNIT_TEMPLATE_GROUP,
                instance: template,
            }),
        );
    }
    if let Some(capacity) = spec.student_capacity {
        if capacity < 0 {
            return Err(format!(
                "student capacity must be finite and non-negative, got {capacity}"
            ));
        }
        set_scalar(
            &mut unit,
            HASH_STUDENT_CAPACITY,
            PropType::Float,
            Value::Float(capacity as f32),
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
        // 菜单身份与普通场景共用 delta 语义：只写 scene.writes 声明的列族，
        // 未声明族从 donor 身份原样保留。r10 的"整体重写"曾把锚点/灯光附属
        // 全部清空——菜单路线同样不能用 authoritative。
        super::scene::apply_units(&mut lot, document, scene_writes)?;
    }
    // Do not synthesize parent/self-map fields here.  The external packager
    // preserves those fields from the Asset/Lot donor; Dalian and the known
    // working reference both omit them.  Inventing them changes the runtime
    // inheritance graph and can leave only a placement marker visible.
    if let Some([width, height]) = scene_document.and_then(|document| document.lot_size) {
        set_scalar(
            &mut lot,
            LOT_SIZE_HASH,
            PropType::Vector2,
            Value::Vector2([width, height]),
        );
    }
    lot.claimed_count = lot.values.len() as u32;

    Ok((encode(&menu)?, encode(&unit)?, encode(&lot)?, locale_table))
}

fn patch_instance_refs(file: &mut PropertyFile, old: u32, new: u32) {
    for property in &mut file.values {
        match &mut property.kind {
            Kind::Scalar(Value::Key(key)) => patch_key(key, old, new),
            Kind::Scalar(Value::Text(text)) => {
                if text.instance_id == old {
                    text.instance_id = new;
                }
            }
            Kind::Array(values) => {
                for value in values {
                    match value {
                        Value::Key(key) => patch_key(key, old, new),
                        Value::Text(text) if text.instance_id == old => text.instance_id = new,
                        _ => {}
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

fn build_donor_locale(spec: &MenuRegistrationSpec, instance: u32, table: u32) -> Vec<u8> {
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

/// 编译后模型的真实包围盒（LOD0 顶点位置并集）。失败 = None（保留 donor
/// 盒子并保持构建继续；menu-contract 仍会校验 BoundingBox 是否存在）。
/// 图形单元契约：内景控制器创建/驱动所需的两个布尔开关必须同时为 true。
///
/// 依据 SC5 零售引擎研究（`D:/ea-games/simcity_offline/entity/001/第九阶段_内景控制器与昼夜驱动.md`）：
/// - `0x0081D020` 创建：无居民/工人来源时，只有图形资产 `0F169C46=true` 才建控制器；
/// - `0x0081CD60` 初始化：读 `0F169C46`（模式）与 `0E0BE186`；
/// - 控制器为空 → `0x00783BC1` 使用固定阈值 `(1,1,1)` → shader 永远选内景第 0 列，
///   而本仓库材质把夜景发光编在第 1 列 → 窗格永久全黑。
///
/// auto-wiring 顺序（s31 / 原版地标同构）：
/// 1. 先尝试从宿主单位的居民/工人 bin 创建控制器；
/// 2. 宿主无对应来源时（例如新建资产工人不足），**图形资源里 0F169C46=true 解除限制**；
/// 3. 控制器创建成功后按 GetInfo `[q*d*s, q*s, s]` 计算阈值（夜 q=0.85, d=0），
///    选中内景图集第 1 列（夜景发光帧），白天回第 0 列。
///
/// 注意 flags：原版 bool 用标量 flags，在编码层保持默认（本仓库 `set_scalar` 已对齐）；
/// 资源本身也不属于 `42E1C000` 的整包属性形态。

fn verify_interior_controller_contract(bytes: &[u8]) -> Result<(), String> {
    super::interior_lighting::verify_graphics_switches(bytes).map(|_| ())
}

const GRAPHICS_BOUNDS: u32 = 0x00F9_EFBA;

fn write_graphics_bounds(graphics: &mut PropertyFile, asset: &PropertyFile) -> Result<(), String> {
    let bounds = asset
        .get(GRAPHICS_BOUNDS)
        .and_then(|property| match &property.kind {
            Kind::Array(values) => match values.as_slice() {
                [value @ Value::BoundingBox { .. }] => Some(value.clone()),
                _ => None,
            },
            _ => None,
        })
        .ok_or("graphics unit contract: Asset requires one BoundingBox in 00F9EFBA")?;
    // Retail 42E1C000 uses the same array as the Asset dictionary
    // (SC5 Packager build_candidate.py:112). EA::Variant selects separate
    // storage branches for scalar and array bounding boxes.
    set_array_with_flags(
        graphics,
        GRAPHICS_BOUNDS,
        PropType::BoundingBox,
        vec![bounds],
        0x801C,
    );
    Ok(())
}

fn verify_graphics_bounds(bytes: &[u8], asset: &PropertyFile) -> Result<(), String> {
    let graphics = PropertyFile::parse(bytes)
        .map_err(|error| format!("graphics unit contract: parse: {error}"))?;
    let expected = asset.get(GRAPHICS_BOUNDS).map(|property| &property.kind);
    if graphics.get(GRAPHICS_BOUNDS).is_some_and(|property| {
        property.prop_type == PropType::BoundingBox
            && property.encoding.flags == 0x801C
            && property.encoding.array_item_size == Some(24)
            && Some(&property.kind) == expected
    }) {
        return Ok(());
    }
    Err("graphics unit contract: 00F9EFBA must match the Asset's native BoundingBox array".into())
}

fn model_bounds(
    resources: &connector::Resources,
    model_ids: Option<LodModelIds>,
) -> Option<([f32; 3], [f32; 3])> {
    let model_ids = model_ids?;
    let bytes = resources.get(&model_ids[0])?;
    let file = rw4::Rw4File::parse(bytes).ok()?;
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut found = false;
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(bytes, section.number) else {
            continue;
        };
        for vertex in &mesh.vertices {
            let Some(p) = vertex.position() else { continue };
            found = true;
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
    }
    found.then_some((min, max))
}

/// 按工作台文档的地块尺寸生成纯色满覆盖遮罩（正方形 80×80 就是 80×80）。
/// 2026-11-08 事故：克隆 donor 遮罩（88×120 长方形）再拉成 80×80，地面
/// 既不是自己的形状也不是自己的纹样。地面以用户编排为准。
fn synthetic_lot_mask(
    resources: &connector::Resources,
    menu_instance: u32,
    size: [f32; 2],
) -> Result<(ResourceId, Vec<u8>), String> {
    let width = size[0].round().max(1.0) as u32;
    let height = size[1].round().max(1.0) as u32;
    let pixels = vec![[255u8, 255, 255, 255]; (width as usize) * (height as usize)];
    let bytes = super::materials::encode_rgba_raster(width, height, &pixels)?;
    let instance = alloc_raster_instance(resources, menu_instance, &bytes);
    Ok((
        ResourceId {
            type_id: raster_type(),
            group: 0,
            instance,
        },
        bytes,
    ))
}

fn lot_mask_for_scene(
    resources: &connector::Resources,
    donor_lot: &[u8],
    document: Option<&SceneDocument>,
    menu_instance: u32,
    ground_source: Option<(&connector::Resources, ResourceId)>,
) -> Result<(Option<(ResourceId, Vec<u8>)>, Option<ResourceId>), String> {
    match document.and_then(|document| document.lot_mask) {
        Some(super::scene::SceneLotMaskKind::Authored) => {
            // legacy 克隆路线不做 donor-free 合成：authored 地面必须走
            // parent-free (Building40) 路线（stages 菜单阶段已挡，此处
            // 防御性断言——合成 mask 属于 ground_source 快照，不属于这里）。
            return Err(
                "authored ground requires the parent-free identity route; the legacy \
                 lot-clone route is not supported"
                    .into(),
            );
        }
        Some(super::scene::SceneLotMaskKind::Source) => {
            let (source_resources, source_lot) = ground_source
                .ok_or("lot.mask.kind=source requires a resolved source Lot and raster snapshot")?;
            let source_lot_bytes = source_resources.get(&source_lot).ok_or_else(|| {
                format!("ground source Lot {source_lot} is missing from its snapshot")
            })?;
            clone_lot_mask(
                source_resources,
                resources,
                source_lot_bytes,
                menu_instance,
                true,
            )
        }
        Some(super::scene::SceneLotMaskKind::Synthetic) => {
            let size = document
                .and_then(|document| document.lot_size)
                .ok_or("lot.mask.kind=synthetic requires lot.size")?;
            let (id, bytes) = synthetic_lot_mask(resources, menu_instance, size)?;
            Ok((Some((id, bytes)), Some(id)))
        }
        // Legacy documents have no mask metadata. Preserve their old behavior;
        // new explicit source/synthetic documents never take this branch.
        None => match document.and_then(|document| document.lot_size) {
            Some(size) => {
                let (id, bytes) = synthetic_lot_mask(resources, menu_instance, size)?;
                Ok((Some((id, bytes)), Some(id)))
            }
            None => clone_lot_mask(resources, resources, donor_lot, menu_instance, false),
        },
    }
}

fn mask_key(file: &PropertyFile) -> Result<Option<sc_properties::Key>, String> {
    let Some(property) = file.get(LOT_MASK_HASH) else {
        return Ok(None);
    };
    let Kind::Scalar(Value::Key(key)) = &property.kind else {
        return Err("menu registration: lot mask is not a Key".into());
    };
    Ok(Some(*key))
}

pub(crate) fn find_lot_mask(
    resources: &connector::Resources,
    key: sc_properties::Key,
) -> Option<(ResourceId, &[u8])> {
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
    candidates.into_iter().find_map(|id| {
        (id.type_id == raster_type())
            .then(|| resources.get(&id).map(|bytes| (id, bytes.as_slice())))
            .flatten()
    })
}

/// Keep only the independent ground Lot and its mask. The source package may
/// be hundreds of megabytes; none of its unrelated resources enter the output.
pub(crate) fn ground_source_snapshot(
    resources: &connector::Resources,
    lot_id: ResourceId,
) -> Result<connector::Resources, String> {
    let lot_bytes = resources
        .get(&lot_id)
        .ok_or_else(|| format!("ground source Lot {lot_id} is missing"))?;
    let file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("ground source Lot {lot_id} is not a property: {error}"))?;
    let key =
        mask_key(&file)?.ok_or_else(|| format!("ground source Lot {lot_id} has no LotMask"))?;
    let (mask_id, mask_bytes) = find_lot_mask(resources, key).ok_or_else(|| {
        format!(
            "ground source LotMask {:08X}:{:08X}:{:08X} raster is missing",
            key.type_id, key.group, key.instance
        )
    })?;
    Ok(connector::Resources::from([
        (lot_id, lot_bytes.clone()),
        (mask_id, mask_bytes.to_vec()),
    ]))
}

fn clone_lot_mask(
    source_resources: &connector::Resources,
    target_resources: &connector::Resources,
    lot_bytes: &[u8],
    menu_instance: u32,
    required: bool,
) -> Result<(Option<(ResourceId, Vec<u8>)>, Option<ResourceId>), String> {
    let file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("menu registration: reference lot is not a property: {error}"))?;
    let Some(key) = mask_key(&file)? else {
        if required {
            return Err("lot.mask.kind=source: source Lot has no LotMask".into());
        }
        return Ok((None, None));
    };
    let Some((_, bytes)) = find_lot_mask(source_resources, key) else {
        if required {
            return Err(format!(
                "lot.mask.kind=source: source LotMask {:08X}:{:08X}:{:08X} raster is missing",
                key.type_id, key.group, key.instance
            ));
        }
        return Ok((None, None));
    };
    let instance = alloc_raster_instance(target_resources, menu_instance, bytes);
    let id = ResourceId {
        type_id: raster_type(),
        group: 0,
        instance,
    };
    Ok((Some((id, bytes.to_vec())), Some(id)))
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
    // 游戏原生 unit/lot 实例集（部署安全：新身份不得撞游戏既有 TGI，
    // 否则我们的内容会顶替真实游戏单位——2026-11-08 HopeTower 事故）。
    let mut vanilla: std::collections::HashSet<(u32, u32)> = Default::default();
    if let Ok(game_data) = std::env::var("OPENSCP_GAME_DATA") {
        for path in crate::game_assets::package_paths(std::path::Path::new(&game_data)) {
            let Ok(package) = dbpf::Package::open(&path) else {
                continue;
            };
            for entry in package.entries() {
                if entry.id.type_id != 0x00B1_B104 {
                    continue;
                }
                if matches!(
                    entry.id.group,
                    0x40E0_C000 | 0x40E1_C000 | 0x48E0_C000 | 0x41E0_C000 | 0x41E1_C000
                ) {
                    vanilla.insert((entry.id.group, entry.id.instance));
                }
            }
        }
    }
    loop {
        let taken = resources
            .keys()
            .any(|id| id.type_id == PROPERTY_TYPE && id.instance == instance)
            || vanilla.contains(&(0x40E0_C000, instance))
            || vanilla.contains(&(0x40E1_C000, instance))
            || vanilla.contains(&(0x48E0_C000, instance));
        if !taken {
            return instance;
        }
        instance = instance.wrapping_add(1);
    }
}

/// 观光/默认资产的统计行（实证组合）：text 0EBCE29A ↔ param 0x0DA3EE15。
fn set_scalar_kind_single_row(file: &mut PropertyFile) {
    set_array_with_flags(
        file,
        HASH_EFFECT_TITLE,
        PropType::Text,
        vec![Value::Text(Text {
            table_id: 0x50AA_0BEA,
            instance_id: 0x0EBC_E29A,
        })],
        0x809C,
    );
    set_array(
        file,
        HASH_EFFECT_PARAM,
        PropType::UInt32,
        vec![Value::UInt32(0x0DA3_EE15)],
    );
}

/// Tooltip 统计行：s31 GOLDEN 实证的三对 text↔param（必须成对对齐）。
/// 2026-11-08 菜单乱码事故：公园模板只给两行且错位，`0EBCE055` 是需参数
/// 替换的格式串行，缺对应参数时游戏原样打印 `~population_amount:number~`。
fn set_effect_stat_rows(file: &mut PropertyFile) {
    set_array_with_flags(
        file,
        HASH_EFFECT_TITLE,
        PropType::Text,
        vec![
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
        ],
        0x809C,
    );
    set_array(
        file,
        HASH_EFFECT_PARAM,
        PropType::UInt32,
        vec![
            Value::UInt32(0x0AD9_3A77),
            Value::UInt32(0x0C3E_9F99),
            Value::UInt32(0x0E58_5AE0),
        ],
    );
}

/// Lot 叶子 = 参照 lot property 克隆 + parent 改挂地标模板 + 与同 TGI
/// 覆盖一致的场景 lowering（单元列/LotSize/Placement/LOD 重定向）。
fn build_lot_leaf(
    spec: &MenuRegistrationSpec,
    lot_bytes: &[u8],
    instance: u32,
    scene_document: Option<&SceneDocument>,
    scene_writes: &super::scene::SceneWrites,
    model_ids: Option<LodModelIds>,
    mask_key: Option<ResourceId>,
    bounds: Option<([f32; 3], [f32; 3])>,
) -> Result<Vec<u8>, String> {
    let _ = instance;
    let mut file = PropertyFile::parse(lot_bytes)
        .map_err(|error| format!("menu registration: reference lot is not a property: {error}"))?;
    // Keep the proven clean landmark Lot skeleton (R11/Blender comparison).
    // Reference lots may carry dozens of building-specific prop/path fields;
    // carrying those into a new menu leaf causes save-load crashes.
    // 2026-11-08 事故：`09FB78CB` 是 donor 的**名字文本边**
    //（Text → locale 表），克隆后建筑在游戏里显示 donor 的名字
    //（实机：大连希望大厦显示「高级煤矿坑」）。身份/文本列一律不得随
    // 地面克隆——lot 只负责地面与数值。
    const SAFE_LOT_HASHES: &[u32] = &[
        HASH_PARENT,
        0x00F9EFBA,
        0x00F9EFBB,
        0x00F9EFBC,
        0x00F9EFBD,
        0x00F9EFBE,
        0x02E33A81,
        0x0AFE2FEB,
        0x0BBBCDED,
        0x0BBBCE2A,
        0x0BBD5875,
        0x0BF76A2A,
        0x0CCB7FC8,
        0x0CCB7FC9,
        0x0CCB7FD0,
        0x0CCB7FD2,
        0x0CCB7FD3,
        0x0CCB7FD4,
        0x0CCB7FD5,
        0x0CCB7FD6,
        0x0D0082F1,
        0x0D02D586,
        0x0D02D587,
        0x0D02D588,
        0x0D02D589,
        0x0D02D58A,
        0x0D02D58B,
        0x0D7AF042,
        0x0D7AF043,
        0x0D7AF044,
        0x0D7AF045,
        0x0D7AF046,
        0x0D7AF047,
        0x0D7AF048,
        0x0D7AF049,
    ];
    file.values
        .retain(|property| SAFE_LOT_HASHES.contains(&property.hash));
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
    // BoundingBox 以**编译模型的真实包围盒**为准：克隆自 donor 的盒子是
    // donor 建筑的尺寸（煤矿坑 88×120），与实际塔楼不符（2026-11-08 事故）。
    if let Some(bounds) = bounds {
        set_array_with_flags(
            &mut file,
            0x00F9_EFBA,
            PropType::BoundingBox,
            vec![Value::BoundingBox {
                min: bounds.0,
                max: bounds.1,
            }],
            0x801C,
        );
    }
    // LOD 重定向与场景写回必须共享同一份完整文档和真实 writes 声明。
    // 旧实现先过滤成 Light/Prop，又把两次写入都硬编码为 lights-only，
    // 导致 paths、effects 和声明的 props 通过源文档校验后仍在最终包消失。
    // 未声明族仍由 delta 语义保留 donor 原值，不能在这里改成全量覆盖。
    let lowered = super::package::apply_scene_lot_semantics(
        &encode(&file)?,
        scene_document,
        model_ids,
        scene_writes,
    )?;
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
    // The normal menu route emits a fresh Unit leaf instead of passing through
    // native_profile.  Carry the runtime placement family here as well;
    // without it the menu identity is visible but has no ploppable behavior.
    set_scalar(
        &mut file,
        HASH_UNIT_CATEGORY,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance: spec
                .behavior_category_instance
                .unwrap_or(PARK_BEHAVIOR_CATEGORY),
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
    // 原版 SCUnit 两项成本均为 Float32（s31/v7/vanilla 实测一致）。Int32 的
    // 位型会被游戏按 f32 读出 ~0 —— 建造面板显示「免费」（2026-11-08
    // HopeTower 事故）。
    if let Some(cost) = spec.construction_cost {
        set_scalar(
            &mut file,
            HASH_CONSTRUCTION_COST,
            PropType::Float,
            Value::Float(cost as f32),
        );
    }
    if let Some(cost) = spec.maintenance_cost {
        set_scalar(
            &mut file,
            HASH_MAINTENANCE_COST,
            PropType::Float,
            Value::Float(cost as f32),
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
        ui_category_values(spec, LANDMARK_CATEGORY),
    );
    // 与 parent-free 路径同款：可选工具对象类别（0x0975695E）。
    if let Some(building_category) = spec.building_category_instance {
        set_scalar(
            &mut file,
            HASH_BUILDING_CATEGORY,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: building_category,
            }),
        );
    }
    set_array(
        &mut file,
        HASH_UI_POSITION,
        PropType::Int32,
        vec![Value::Int32(spec.ui_position.unwrap_or(11500))],
    );
    // The companion locale resource uses fixed rows 1 (title) and 2
    // (description), written at the identity instance (19a1b3a).  Pointing at
    // the shared LOCALE_TABLE constant here made two packages shadow each
    // other's title/description (2026-11-08: v7 "OpenSCP Commercial Tower"
    // masked 大连希望大厦) — and in the fallback path the shared table was
    // never written at all, leaving a dangling Text edge.
    set_text_table(&mut file, HASH_TITLE, instance, 1);
    if spec.description.is_some() {
        set_text_table(&mut file, HASH_DESC, instance, 2);
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
    // 统计行：0EBCE29A（Visitors per day）+ 0x0DA3EE15 是 a42cf63e 实机
    // 验证过的组合（显示"每日旅客量 51"）。text 与 param 必须成对，换行
    // 不换参数 = 打印未替换格式串。
    set_scalar_kind_single_row(&mut file);
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
    // The flattened generic menu loses its parent edge. Restore it so the
    // tool remains a real ploppable entry and can participate in bulldoze/
    // lifecycle cleanup. The parent controls menu nesting only.
    set_scalar(
        file,
        HASH_PARENT,
        PropType::Key,
        Value::Key(sc_properties::Key {
            type_id: PROPERTY_TYPE,
            group: 0x48E1_EB00,
            instance: spec.menu_parent_instance.unwrap_or(PLOP_MENU_PARENT),
        }),
    );
    set_array(
        file,
        HASH_UI_CATEGORY,
        PropType::Key,
        ui_category_values(spec, LANDMARK_CATEGORY),
    );
    // R1（s17 诊断）：plop 家族工具对象类别。原版小学 Menu2 带
    // 0x0975695E=C710B6E9；公园子根 parent 的 s17 缺失该字段。
    if let Some(building_category) = spec.building_category_instance {
        set_scalar(
            file,
            HASH_BUILDING_CATEGORY,
            PropType::Key,
            Value::Key(sc_properties::Key {
                type_id: 0,
                group: 0,
                instance: building_category,
            }),
        );
    }
    set_array(
        file,
        HASH_UI_POSITION,
        PropType::Int32,
        vec![Value::Int32(spec.ui_position.unwrap_or(11500))],
    );
    set_text_table(file, HASH_TITLE, instance, 1);
    if spec.description.as_ref().is_some_and(|text| !text.trim().is_empty()) {
        set_text_table(file, HASH_DESC, instance, 2);
    } else {
        file.values.retain(|property| property.hash != HASH_DESC);
    }
    let education_behavior = spec.behavior_category_instance == Some(EDUCATION_BEHAVIOR_CATEGORY)
        || spec.unit_donor.as_deref() == Some("elementary-school")
        || spec.simulation_template_instance == Some(ELEMENTARY_TEMPLATE);
    if education_behavior {
        // These are the school tooltip/effect rows. Leaving the generic park
        // rows in place produces literal `~population_amount~` and
        // `~maintenance_cost~` placeholders even though the title resolves.
        set_effect_stat_rows(file);
        // Menu placement remains the park category above; this flag controls
        // the runtime presentation family and must not remain the park value.
        set_scalar(file, HASH_PARK_FLAG, PropType::Bool, Value::Bool(false));
    } else {
        // 观光/非教育资产：**实证过的单行**（a42cf63e 实机截图：
        // 每日旅客量 51 + -1,500/小时）。0EBCE29A 是 "Visitors per day:
        // ~amount:number~"，必须配 0x0DA3EE15 才能替换；配错参数就会原样
        // 打印 `~amount:number~`（2026-11-08 乱码事故的真实原因）。
        set_scalar_kind_single_row(file);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_unit_contract_requires_both_interior_switches() {
        // 回归防护（夜灯反复十几次的类别）：图形单元缺任一开关 → 构建期直接失败，
        // 不再等到进游戏才发现窗格全黑。
        let mut complete = PropertyFile::default();
        set_scalar(&mut complete, 0x0F16_9C46, PropType::Bool, Value::Bool(true));
        set_scalar(&mut complete, 0x0E0B_E186, PropType::Bool, Value::Bool(true));
        let encoded = encode(&complete).unwrap();
        assert!(verify_interior_controller_contract(&encoded).is_ok());

        // 缺 0F169C46（控制器创建开关）：控制器为空 → 固定阈值 (1,1,1) → 永远第 0 列。
        let mut no_controller = PropertyFile::default();
        set_scalar(&mut no_controller, 0x0E0B_E186, PropType::Bool, Value::Bool(true));
        let error = verify_interior_controller_contract(&encode(&no_controller).unwrap())
            .unwrap_err();
        assert!(error.contains("0F169C46"), "{error}");

        // 缺 0E0BE186（运营提亮）：夜里 q=0 时窗灯仍不会按 active 提亮。
        let mut no_light_switch = PropertyFile::default();
        set_scalar(&mut no_light_switch, 0x0F16_9C46, PropType::Bool, Value::Bool(true));
        let error = verify_interior_controller_contract(&encode(&no_light_switch).unwrap())
            .unwrap_err();
        assert!(error.contains("0E0BE186"), "{error}");

        // 值写成 false 同样拒绝（存在但关闭 = 同样黑）。
        let mut disabled = PropertyFile::default();
        set_scalar(&mut disabled, 0x0F16_9C46, PropType::Bool, Value::Bool(false));
        set_scalar(&mut disabled, 0x0E0B_E186, PropType::Bool, Value::Bool(true));
        assert!(verify_interior_controller_contract(&encode(&disabled).unwrap()).is_err());
    }

    #[test]
    fn graphics_bounds_use_the_native_asset_array_encoding() {
        let bounds = Value::BoundingBox {
            min: [-35.441772, -39.548546, -0.37917364],
            max: [35.44178, 43.228867, 270.3182],
        };
        let mut asset = PropertyFile::default();
        set_array_with_flags(
            &mut asset,
            GRAPHICS_BOUNDS,
            PropType::BoundingBox,
            vec![bounds.clone()],
            0x801C,
        );
        let mut graphics = PropertyFile::default();
        set_scalar(&mut graphics, GRAPHICS_BOUNDS, PropType::BoundingBox, bounds);
        assert!(verify_graphics_bounds(&encode(&graphics).unwrap(), &asset).is_err());
        write_graphics_bounds(&mut graphics, &asset).unwrap();
        verify_graphics_bounds(&encode(&graphics).unwrap(), &asset).unwrap();
    }

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
        let (entry, key) = clone_lot_mask(&resources, &resources, &lot, 0x1234_5678, true).unwrap();
        let (id, bytes) = entry.expect("mask entry");
        let key = key.expect("mask key");
        assert_ne!(
            key.instance, old,
            "new identity must not reuse the source mask TGI"
        );
        assert_eq!(key, id);
        assert_eq!(bytes, vec![1, 2, 3, 4]);
    }

    #[test]
    fn explicit_source_mask_survives_lot_size_pixel_for_pixel() {
        let source_lot = ResourceId {
            type_id: PROPERTY_TYPE,
            group: 0x41F0_C000,
            instance: 0x0483_239E,
        };
        let source_mask = ResourceId {
            type_id: raster_type(),
            group: 0,
            instance: 0xDEDC_4095,
        };
        let lot = PropertyFile {
            values: vec![Property {
                hash: LOT_MASK_HASH,
                prop_type: PropType::Key,
                kind: Kind::Scalar(Value::Key(sc_properties::Key {
                    type_id: 0,
                    group: 0,
                    instance: source_mask.instance,
                })),
                encoding: PropertyEncoding::default(),
            }],
            claimed_count: 1,
        }
        .encode_canonical()
        .unwrap();
        let pixels = (0..12)
            .map(|index| [index as u8, 255 - index as u8, index as u8 * 3, 0])
            .collect::<Vec<_>>();
        let raster = super::super::materials::encode_rgba_raster(4, 3, &pixels).unwrap();
        let source = connector::Resources::from([(source_lot, lot), (source_mask, raster.clone())]);
        let snapshot = ground_source_snapshot(&source, source_lot).unwrap();
        assert_eq!(snapshot.len(), 2, "only the Lot and mask are retained");
        let document = super::super::scene::parse_document(&serde_json::json!({
            "lot": {"size": [80, 80], "mask": {"kind": "source"}}
        }))
        .unwrap();
        let target = connector::Resources::new();
        let (entry, _) = lot_mask_for_scene(
            &target,
            &source[&source_lot],
            Some(&document),
            0x1234_5678,
            Some((&snapshot, source_lot)),
        )
        .unwrap();
        let (new_id, bytes) = entry.unwrap();
        assert_ne!(new_id, source_mask);
        assert_eq!(bytes, raster);
        let decoded = rw4::RasterImage::parse(&bytes)
            .unwrap()
            .decode_top_mip_rgba()
            .unwrap();
        let expected = pixels
            .iter()
            .flat_map(|pixel| pixel.into_iter().copied())
            .collect::<Vec<_>>();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn source_snapshot_fails_closed_without_its_raster() {
        let source_lot = ResourceId {
            type_id: PROPERTY_TYPE,
            group: 0x41F0_C000,
            instance: 0x0483_239E,
        };
        let lot = PropertyFile {
            values: vec![Property {
                hash: LOT_MASK_HASH,
                prop_type: PropType::Key,
                kind: Kind::Scalar(Value::Key(sc_properties::Key {
                    type_id: 0,
                    group: 0,
                    instance: 0xDEDC_4095,
                })),
                encoding: PropertyEncoding::default(),
            }],
            claimed_count: 1,
        }
        .encode_canonical()
        .unwrap();
        let source = connector::Resources::from([(source_lot, lot)]);
        let error = ground_source_snapshot(&source, source_lot).unwrap_err();
        assert!(error.contains("raster is missing"), "{error}");
    }

    #[test]
    fn menu_category_and_behavior_category_are_independent() {
        let mut values = std::collections::BTreeMap::new();
        values.insert("title".into(), serde_json::json!("school"));
        values.insert("categoryInstance".into(), serde_json::json!("0x07049250"));
        values.insert(
            "behaviorCategoryInstance".into(),
            serde_json::json!("0x95A9D6FD"),
        );
        let spec = MenuRegistrationSpec::from_values(&values).unwrap();
        assert_eq!(spec.category_instance, Some(0x0704_9250));
        assert_eq!(spec.behavior_category_instance, Some(0x95A9_D6FD));
        assert_ne!(spec.category_instance, spec.behavior_category_instance);
    }

    #[test]
    fn menu_building_category_writes_the_plop_object_class() {
        // R1 判别实验（s17 诊断 §5.3）：Menu2 0x0975695E = C710B6E9
        // kCategoryIDBuilding；原版小学 Menu2 携带，s17 缺失。
        let mut values = std::collections::BTreeMap::new();
        values.insert("title".into(), serde_json::json!("school"));
        values.insert(
            "buildingCategoryInstance".into(),
            serde_json::json!("0xC710B6E9"),
        );
        let spec = MenuRegistrationSpec::from_values(&values).unwrap();
        assert_eq!(spec.building_category_instance, Some(0xC710_B6E9));
        let bytes = build_menu2(&spec, 0x1234_5678).unwrap();
        let file = PropertyFile::parse(&bytes).unwrap();
        let category = file
            .values
            .iter()
            .find(|property| property.hash == HASH_BUILDING_CATEGORY)
            .expect("building category entry");
        match &category.kind {
            Kind::Scalar(Value::Key(key)) => assert_eq!(key.instance, 0xC710_B6E9),
            other => panic!("unexpected building category value {other:?}"),
        }

        // 缺省不写：与旧行为兼容。
        let mut minimal = std::collections::BTreeMap::new();
        minimal.insert("title".into(), serde_json::json!("school"));
        let spec = MenuRegistrationSpec::from_values(&minimal).unwrap();
        let bytes = build_menu2(&spec, 0x1234_5678).unwrap();
        let file = PropertyFile::parse(&bytes).unwrap();
        assert!(
            file.values
                .iter()
                .all(|property| property.hash != HASH_BUILDING_CATEGORY),
            "building category must be absent by default"
        );
    }

    #[test]
    fn menu_category_instances_expand_the_ui_category_array() {
        // 多容器挂载：categoryInstances 非空时整体写进 0x0DB9FC63 Key 数组
        // （菜单编辑节点组 = 同一建筑身份的多个目标容器）。
        let mut values = std::collections::BTreeMap::new();
        values.insert("title".into(), serde_json::json!("campus"));
        values.insert(
            "categoryInstances".into(),
            serde_json::json!(["0x899BEA70", "94E412ED", "07049250"]),
        );
        let spec = MenuRegistrationSpec::from_values(&values).unwrap();
        assert_eq!(spec.category_instances, vec![0x0704_9250, 0x94E4_12ED]);
        let bytes = build_menu2(&spec, 0x1234_5678).unwrap();
        let file = PropertyFile::parse(&bytes).unwrap();
        let category = file
            .values
            .iter()
            .find(|property| property.hash == HASH_UI_CATEGORY)
            .expect("uiCategories entry");
        match &category.kind {
            Kind::Array(items) => {
                let instances: Vec<u32> = items
                    .iter()
                    .map(|value| match value {
                        Value::Key(key) => key.instance,
                        other => panic!("unexpected uiCategories value {other:?}"),
                    })
                    .collect();
                assert_eq!(instances, vec![0x0704_9250, 0x94E4_12ED]);
            }
            other => panic!("uiCategories must be an array, got {other:?}"),
        }

        // 单字段路径保持兼容：categoryInstances 缺省为空，回退 categoryInstance。
        let mut single = std::collections::BTreeMap::new();
        single.insert("title".into(), serde_json::json!("school"));
        single.insert("categoryInstance".into(), serde_json::json!("0x07049250"));
        let spec = MenuRegistrationSpec::from_values(&single).unwrap();
        assert!(spec.category_instances.is_empty());
        let bytes = build_menu2(&spec, 0x1234_5678).unwrap();
        let file = PropertyFile::parse(&bytes).unwrap();
        let category = file
            .values
            .iter()
            .find(|property| property.hash == HASH_UI_CATEGORY)
            .expect("uiCategories entry");
        match &category.kind {
            Kind::Array(items) => assert_eq!(items.len(), 1),
            other => panic!("uiCategories must be an array, got {other:?}"),
        }
    }
}

/// 128×128 工具图标 PNG：`iconPng` 指向项目内真实 PNG 时原样使用；
/// 缺失/非法文件 fail-closed（不再静默回退占位图——r10 类"图没换也没人
/// 知道"的静默失败必须消失）。空串 = 生成占位棋盘。
fn build_icon_png(
    spec: &MenuRegistrationSpec,
    instance: u32,
    root: &std::path::Path,
) -> Result<Vec<u8>, String> {
    if let Some(path) = spec
        .icon_png
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let file = root.join(path);
        let bytes =
            std::fs::read(&file).map_err(|error| format!("menu.iconPng: {path}: {error}"))?;
        // 图标契约（照抄 SC5 Packager `menu_icons.py:6-10` + `verify_package.py:46-49`）：
        // 无论输入是 PNG 还是被误标成 `.png` 的 JPEG，一律解码 → RGBA →
        // 等比 `fit` 到 128×128（Lanczos）→ 重编码 PNG。基准侧 verify 对
        // 尺寸不符直接判失败；直通会把 240×160 之类的图打进包导致 UI 拉伸/裁切。
        let image = image::load_from_memory(&bytes)
            .map_err(|_| format!("menu.iconPng: {path} is not a decodable image"))?
            .to_rgba8();
        let fitted = image::imageops::resize(
            &image,
            128,
            128,
            image::imageops::FilterType::Lanczos3,
        );
        let mut normalized = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(fitted)
            .write_to(&mut normalized, image::ImageFormat::Png)
            .map_err(|error| format!("menu.iconPng: {path} image conversion failed: {error}"))?;
        return Ok(normalized.into_inner());
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
    Ok(png)
}

/// 454×263 rollover 大图 JPG：`marqueePng` 指向项目内真实 JPG 时原样使用；
/// 缺失/非 JPG 时 fail-closed。空串 = 生成占位横幅。
fn build_marquee_jpg(
    spec: &MenuRegistrationSpec,
    instance: u32,
    root: &std::path::Path,
) -> Result<Vec<u8>, String> {
    if let Some(path) = spec
        .marquee_png
        .as_deref()
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let lower = path.to_ascii_lowercase();
        if !(lower.ends_with(".jpg") || lower.ends_with(".jpeg")) {
            return Err(format!(
                "menu.marqueePng: {path} must be a .jpg/.jpeg rollover image"
            ));
        }
        let bytes = std::fs::read(root.join(path))
            .map_err(|error| format!("menu.marqueePng: {path}: {error}"))?;
        if !bytes.starts_with(&[0xFF, 0xD8]) {
            return Err(format!("menu.marqueePng: {path} is not a JPEG file"));
        }
        return Ok(bytes);
    }
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
    Ok(jpg)
}

fn palette(seed: u32) -> ([u8; 4], [u8; 4]) {
    let hash = Sha256::digest(seed.to_be_bytes());
    let mix = |i: usize, base: u8| hash[i].saturating_add(base) / 2 + 40;
    (
        [mix(0, 60), mix(1, 90), mix(2, 140), 255],
        [mix(3, 20), mix(4, 30), mix(5, 50), 255],
    )
}
