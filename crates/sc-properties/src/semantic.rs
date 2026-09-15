//! property（0x00B1B104）的语义子类型 tag。
//!
//! 判据（两级，见 docs/overview/analyze/05 的普查证据）：
//! 1. **group 低 16 位**（原 SCP InstanceType，`Views/.../InstanceTypeIconConverter.cs`）——
//!    适用于游戏 UI/装配侧家族（Unit/Agent/Menu…，group 形如 0x48E1C500）；
//! 2. **结构特征**（兜底）——GCT 模板资源（地形/生态画笔、水、地图模板）以
//!    **全随机 32 位 group** 复制多份入索引（同一实例可出现 30+ 个不同 group），
//!    低 16 位对它们无意义，必须按特征键判定
//!    （heightmap 名 string8 `0x0DBA3A9C` + 分辨率 u32 `0x0DC097E3` 同时在场）。
use serde::Serialize;

use crate::PropertyFile;

/// 特征键：画笔/地图模板的 heightmap 名称（string8，如 "heightmap"/"forestheightmap"）。
pub const BRUSH_HEIGHTMAP_NAME: u32 = 0x0DBA_3A9C;
/// 特征键：画笔/地图模板的高度图分辨率（u32，128/256）。
pub const BRUSH_RESOLUTION: u32 = 0x0DC0_97E3;

/// property 资源的语义子类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PropertySemantic {
    /// 0xC000 — lot/建筑单元（灯光/效果/贴花/道具/路径/生成器同住一个 property）。
    Unit,
    /// 0xC600 — 载具/小人定义（Coal Truck / Computer Truck / 小人；含车灯列族）。
    Agent,
    /// 0x2D00 — Agent 派生的载具/人物模型条目（Vehicle Models + GI_Simm_* 顾问/市民）。
    AgentVehicleModel,
    /// 0xC100 — kResourceID* 模拟资源定义（石油/合金/学生 Token…，Resource Name）。
    ResourceDef,
    /// 0xE800 — kResourceID*Stop 等 站点/载具资源装载条目（多路 int32 装载数组）。
    ResourceEntry,
    /// 0xC500 — GlassBox 动作/任务对象（Action Title / Action Failed Title）。
    SimAction,
    /// 0xC400 — 道路/网络定义（Dirt Road…，Appearance 引用 Path）。
    Network,
    /// 0x8B7E — 路径外观定义（桥梁/车道断面参数，被 Network 引用）。
    Path,
    /// 0xE000 — 数据图层/区域地图定义（kRegionDataLayer_* · kCategoryID* · *Light）。
    MapLayer,
    /// 0x44F2 — 城市警报定义（AlertNoWater…，图标 + 文本 + 时长）。
    Alert,
    /// 0xC300 — RCI 分区颜色（Residential/Commercial/Industrial + colorRGB）。
    ZoneColor,
    /// 0x8A01 — 菜单条目（工具/建筑入口）。
    Menu,
    /// 0xC900 — 菜单条目（EP1/DLC 分卷，Parent 指回 0x09878A01 基类）。
    Menu2,
    /// 0xBA03 — 菜单分类（kCategoryID*，group 0x09A4BA03）。
    MenuCategory,
    /// 0xEB00 — 工具本体（Radius Marker · ecoGameToolAction · Cursor）。
    ToolBody,
    /// 0xE900 — 市政管线绘制样式（watermain/powermain，多组 RGBA + 宽度）。
    UtilityLine,
    /// GCT 地形/生态画笔与地图模板（结构判定；同一实例以 30+ 随机 group 复制入索引）。
    TerrainBrushTemplate,
    /// 0x2043 — 描述符（纹理/模型/载具杂项描述）。
    Descriptor,
    /// 0xB185 / 0x1651 / 0x1652 — 贴花字典三册。
    DecalAtlas,
    /// 0x0000 — 模型包装（LOD1 + Model Bounding Box + Texture 引用，多为无名）。
    ModelWrapper,
    /// 其余低频/未定簇。
    Other,
}

impl PropertySemantic {
    /// 中文短标签（资源树/预览徽章用）。
    pub fn label_zh(&self) -> &'static str {
        match self {
            PropertySemantic::Unit => "单元",
            PropertySemantic::Agent => "载具/小人",
            PropertySemantic::AgentVehicleModel => "载具模型",
            PropertySemantic::ResourceDef => "模拟资源",
            PropertySemantic::ResourceEntry => "站点资源",
            PropertySemantic::SimAction => "动作/任务",
            PropertySemantic::Network => "道路网络",
            PropertySemantic::Path => "路径外观",
            PropertySemantic::MapLayer => "数据图层",
            PropertySemantic::Alert => "警报",
            PropertySemantic::ZoneColor => "分区颜色",
            PropertySemantic::Menu => "菜单条目",
            PropertySemantic::Menu2 => "菜单条目",
            PropertySemantic::MenuCategory => "菜单分类",
            PropertySemantic::ToolBody => "工具本体",
            PropertySemantic::UtilityLine => "管线样式",
            PropertySemantic::TerrainBrushTemplate => "地形画笔模板",
            PropertySemantic::Descriptor => "描述符",
            PropertySemantic::DecalAtlas => "贴花字典",
            PropertySemantic::ModelWrapper => "模型包装",
            PropertySemantic::Other => "其他",
        }
    }

    /// 英文短标签（与原 SCP InstanceTypeIconConverter 对齐）。
    pub fn label_en(&self) -> &'static str {
        match self {
            PropertySemantic::Unit => "Unit",
            PropertySemantic::Agent => "Agent",
            PropertySemantic::AgentVehicleModel => "Agent Vehicle Model",
            PropertySemantic::ResourceDef => "Resource Definition",
            PropertySemantic::ResourceEntry => "Resource Entry",
            PropertySemantic::SimAction => "Sim Action",
            PropertySemantic::Network => "Network",
            PropertySemantic::Path => "Path",
            PropertySemantic::MapLayer => "Map Layer",
            PropertySemantic::Alert => "Alert",
            PropertySemantic::ZoneColor => "Zone Color",
            PropertySemantic::Menu | PropertySemantic::Menu2 => "Menu",
            PropertySemantic::MenuCategory => "Menu Category",
            PropertySemantic::ToolBody => "Tool Body",
            PropertySemantic::UtilityLine => "Utility Line",
            PropertySemantic::TerrainBrushTemplate => "Terrain Brush Template",
            PropertySemantic::Descriptor => "Descriptor",
            PropertySemantic::DecalAtlas => "Decal Atlas",
            PropertySemantic::ModelWrapper => "Model Wrapper",
            PropertySemantic::Other => "Other",
        }
    }
}

/// 按 group 低 16 位判别（原 SCP InstanceType 规则）。
///
/// 注意：GCT 模板资源（画笔/地图模板）以随机 group 多重复制入索引，
/// 低 16 位对它们不稳定——完整判定请用 [`property_semantic`]（结构判据优先）。
pub fn property_semantic_by_group(group: u32) -> PropertySemantic {
    match group & 0xFFFF {
        0xC000 => PropertySemantic::Unit,
        0xC600 => PropertySemantic::Agent,
        0x2D00 => PropertySemantic::AgentVehicleModel,
        0xC100 => PropertySemantic::ResourceDef,
        0xE800 => PropertySemantic::ResourceEntry,
        0xC500 => PropertySemantic::SimAction,
        0xC400 => PropertySemantic::Network,
        0x8B7E => PropertySemantic::Path,
        0xE000 => PropertySemantic::MapLayer,
        0x44F2 => PropertySemantic::Alert,
        0xC300 => PropertySemantic::ZoneColor,
        0x8A01 => PropertySemantic::Menu,
        0xC900 => PropertySemantic::Menu2,
        0xBA03 => PropertySemantic::MenuCategory,
        0xEB00 => PropertySemantic::ToolBody,
        0xE900 => PropertySemantic::UtilityLine,
        0x2043 => PropertySemantic::Descriptor,
        0xB185 | 0x1651 | 0x1652 => PropertySemantic::DecalAtlas,
        0x0000 => PropertySemantic::ModelWrapper,
        _ => PropertySemantic::Other,
    }
}

/// 结构判据：heightmap 名称 + 分辨率两个特征键同时在场 → 地形画笔模板。
fn is_terrain_brush_template(file: &PropertyFile) -> bool {
    let mut has_name = false;
    let mut has_resolution = false;
    for p in &file.values {
        has_name |= p.hash == BRUSH_HEIGHTMAP_NAME;
        has_resolution |= p.hash == BRUSH_RESOLUTION;
        if has_name && has_resolution {
            return true;
        }
    }
    false
}

/// 完整判定：结构判据优先（GCT 模板资源的 group 低 16 位不稳定），
/// 否则回退 group 低 16 位表。
pub fn property_semantic(file: &PropertyFile, group: u32) -> PropertySemantic {
    if is_terrain_brush_template(file) {
        return PropertySemantic::TerrainBrushTemplate;
    }
    property_semantic_by_group(group)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Kind, PropType, Property, Value};

    fn prop(hash: u32, prop_type: PropType, kind: Kind) -> Property {
        Property {
            hash,
            prop_type,
            kind,
            encoding: crate::model::PropertyEncoding::default(),
        }
    }

    #[test]
    fn group_low16_matches_known_families() {
        assert_eq!(property_semantic_by_group(0x48E1_C000), PropertySemantic::Unit);
        assert_eq!(property_semantic_by_group(0x48E1_C600), PropertySemantic::Agent);
        assert_eq!(property_semantic_by_group(0x40E0_2D00), PropertySemantic::AgentVehicleModel);
        assert_eq!(property_semantic_by_group(0x48E1_C500), PropertySemantic::SimAction);
        assert_eq!(property_semantic_by_group(0x48E1_E800), PropertySemantic::ResourceEntry);
        assert_eq!(property_semantic_by_group(0x0000_C100), PropertySemantic::ResourceDef);
        assert_eq!(property_semantic_by_group(0x0955_8B7E), PropertySemantic::Path);
        assert_eq!(property_semantic_by_group(0x09A4_BA03), PropertySemantic::MenuCategory);
        assert_eq!(property_semantic_by_group(0x40E0_E900), PropertySemantic::UtilityLine);
        assert_eq!(property_semantic_by_group(0xB185_1651), PropertySemantic::DecalAtlas);
        assert_eq!(property_semantic_by_group(0x1234_5678), PropertySemantic::Other);
    }

    #[test]
    fn brush_template_detected_by_structure_under_arbitrary_group() {
        // 画笔模板在真实数据里挂在随机 group 下（如 0xB2150776 / 0xC0E1023A）。
        let file = PropertyFile {
            claimed_count: 0,
            values: vec![
                prop(BRUSH_HEIGHTMAP_NAME, PropType::String8, Kind::Scalar(Value::String8("heightmap".into()))),
                prop(BRUSH_RESOLUTION, PropType::UInt32, Kind::Scalar(Value::UInt32(256))),
            ],
        };
        assert_eq!(
            property_semantic(&file, 0xB215_0776),
            PropertySemantic::TerrainBrushTemplate
        );
    }

    #[test]
    fn single_brush_key_is_not_enough() {
        let file = PropertyFile {
            claimed_count: 0,
            values: vec![prop(
                BRUSH_HEIGHTMAP_NAME,
                PropType::String8,
                Kind::Scalar(Value::String8("heightmap".into())),
            )],
        };
        assert_eq!(property_semantic(&file, 0x48E1_C600), PropertySemantic::Agent);
    }

    #[test]
    fn unit_family_beats_unknown_group() {
        let file = PropertyFile { claimed_count: 0, values: vec![] };
        assert_eq!(property_semantic(&file, 0x42E1_C000), PropertySemantic::Unit);
        assert_eq!(property_semantic(&file, 0x0000_0000), PropertySemantic::ModelWrapper);
    }
}
