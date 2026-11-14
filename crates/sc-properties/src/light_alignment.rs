//! Unit 附属灯光（路灯 / 建筑夜灯）的坐标契约与“盒外”对齐审计。
//!
//! 依据 dev 样本反编译：
//!
//! - `SC::FillLightInfoMap`（RVA 0x7D24D0）按表属性生成 `cLightInfo`；
//!   `cSPTransform::ConvertToMatrix4` 取每盏灯的**局部变换**。
//! - `SC::cUnitModel::FillFromProps` / `OnMove` 调
//!   `cGraphicsLights::SetTransform(&unit->mWorldTransform)`；
//! - `SC::cGraphicsLights::SetTransform`（RVA 0x7D1750）→
//!   `SC::cLightInfo::SetTransform`（RVA 0x7D0E70）→ `cLightRenderer::SetPos`。
//!
//! 因此灯的最终世界位置 = `unitWorldTransform × lightLocalTranslation`，
//! **链上没有任何 `PlayArea` 夹取**。盒外灯错位只可能来自：
//!
//! 1. 传给灯的是区域坐标而网格/模型用城市局部坐标（或反之，差一个
//!    `mPosInRegion`）；
//! 2. 灯位置本身用 eco 采样（`WorldToEco` 在 `|p| > flatScale` 时夹到格边）；
//! 3. 单位变换未更新（盒外单位被跳过）导致灯停在旧位置。
//!
//! 本模块把这三类都变成可判定的量：给出一组灯的局部变换、单位变换和城市
//! 原点，就能算出每盏灯的期望世界位置、是否落在 `PlayArea`/eco 定义域内，
//! 并在候选坐标系之间做假设检验，输出需要的平移修正。
//!
//! 灯表列（`0x00B1B104` 属性内，实测 ≥1810 处）：
//!
//! | 列 | 含义 |
//! |---|---|
//! | `0x0CAA8F10` | 灯类（每模型常量，如 `BC76E6BA`） |
//! | `0x0CAA8F11` | 灯型：`0x75D4C8CD` 点 / `0x2F0FF9FD` 聚光 / `0x0820ABAF` 线 |
//! | `0x0CAA8F12` | 局部变换（56 B，12 float） |
//! | `0x0CAA8F13` | 颜色 RGB |
//! | `0x0CAA8F14` / `0x0CAA8F15` / `0x0CAA8F16` | 半径 / 内半径 / 第三参数 |
//! | `0x0CAA8F17` | 通道名（`overhang`、`F_door`、`F_spot`…） |

use crate::map_frames::{EcoGrid, EcoSample};
use crate::play_area::PlayArea;
use crate::{Kind, PropertyFile, Transform, Value};
use serde::Serialize;

/// 灯类列。
pub const LIGHT_CLASS: u32 = 0x0CAA_8F10;
/// 灯型列（值为 `LIGHT_TYPE_*`）。
pub const LIGHT_TYPE: u32 = 0x0CAA_8F11;
/// 局部变换列。
pub const LIGHT_TRANSFORM: u32 = 0x0CAA_8F12;
pub const LIGHT_COLOR: u32 = 0x0CAA_8F13;
pub const LIGHT_RADIUS: u32 = 0x0CAA_8F14;
pub const LIGHT_INNER_RADIUS: u32 = 0x0CAA_8F15;
pub const LIGHT_PARAM: u32 = 0x0CAA_8F16;
pub const LIGHT_CHANNEL: u32 = 0x0CAA_8F17;

/// 线光源（`CreateLineLight`）。
pub const LIGHT_TYPE_LINE: u32 = 0x0820_ABAF;
/// 聚光源（`CreateSpotLight`）。
pub const LIGHT_TYPE_SPOT: u32 = 0x2F0F_F9FD;
/// 点光源（`CreatePointLight`）。
pub const LIGHT_TYPE_POINT: u32 = 0x75D4_C8CD;

/// `cSPTransform` 的前三行 + 平移（反编译 `ConvertToMatrix4` 的输入）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Transform34 {
    /// 三个基向量（行主序，与 12 float 存储一致）。
    pub axes: [[f32; 3]; 3],
    pub translation: [f32; 3],
}

impl Transform34 {
    pub fn identity() -> Self {
        Self {
            axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            translation: [0.0; 3],
        }
    }

    pub fn from_translation(t: [f32; 3]) -> Self {
        Self {
            translation: t,
            ..Self::identity()
        }
    }

    /// 12 float → 3×3 + 平移。矩阵元素不足 12 个时返回 `None`。
    pub fn from_transform(value: &Transform) -> Option<Self> {
        if value.matrix.len() < 12 {
            return None;
        }
        let m = &value.matrix;
        Some(Self {
            axes: [[m[0], m[1], m[2]], [m[3], m[4], m[5]], [m[6], m[7], m[8]]],
            translation: [m[9], m[10], m[11]],
        })
    }

    /// 点变换：`p' = xAxis·p.x + yAxis·p.y + zAxis·p.z + t`
    /// （与 `cLightInfo::SetTransform` 对 `wAxis` 的乘法一致）。
    pub fn transform_point(&self, p: [f32; 3]) -> [f32; 3] {
        let mut out = self.translation;
        for (axis, comp) in self.axes.iter().zip(p) {
            for k in 0..3 {
                out[k] += axis[k] * comp;
            }
        }
        out
    }

    /// `self ∘ child`：先用 `child` 再用 `self`。
    pub fn compose(&self, child: &Self) -> Self {
        let mut axes = [[0.0f32; 3]; 3];
        for (row, axis) in axes.iter_mut().enumerate() {
            for (slot, column) in axis.iter_mut().enumerate() {
                *column = (0..3)
                    .map(|j| self.axes[j][slot] * child.axes[row][j])
                    .sum::<f32>();
            }
        }
        Self {
            axes,
            translation: self.transform_point(child.translation),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LightKind {
    Line,
    Spot,
    Point,
    /// 未识别的灯型哈希（原值保留，供人工判定）。
    Other(u32),
}

impl LightKind {
    pub fn from_hash(hash: u32) -> Self {
        match hash {
            LIGHT_TYPE_LINE => Self::Line,
            LIGHT_TYPE_SPOT => Self::Spot,
            LIGHT_TYPE_POINT => Self::Point,
            other => Self::Other(other),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightDef {
    pub index: usize,
    pub kind: LightKind,
    pub class: u32,
    /// 灯在单位（模型）空间中的变换。
    pub local: Transform34,
    pub color: [f32; 3],
    pub radius: f32,
    pub inner_radius: f32,
    pub param: f32,
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightTable {
    pub lights: Vec<LightDef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LightTableError {
    /// 没有灯型列或变换列：该属性不是灯光表。
    NotALightTable,
    /// 列长度与灯型列不一致。
    ColumnLengthMismatch {
        hash: u32,
        expected: usize,
        actual: usize,
    },
    /// 变换列元素不是 12 float。
    UnsupportedTransform { index: usize },
}

/// 从属性文件解析灯光表。缺失的可选列用中性值填充。
pub fn parse_light_table(props: &PropertyFile) -> Result<LightTable, LightTableError> {
    let type_column = props
        .get(LIGHT_TYPE)
        .or_else(|| props.get(LIGHT_CLASS))
        .ok_or(LightTableError::NotALightTable)?;
    let Kind::Array(types) = &type_column.kind else {
        return Err(LightTableError::NotALightTable);
    };
    let Some(transform_prop) = props.get(LIGHT_TRANSFORM) else {
        return Err(LightTableError::NotALightTable);
    };
    let Kind::Array(transforms) = &transform_prop.kind else {
        return Err(LightTableError::NotALightTable);
    };
    if transforms.len() != types.len() {
        return Err(LightTableError::ColumnLengthMismatch {
            hash: LIGHT_TRANSFORM,
            expected: types.len(),
            actual: transforms.len(),
        });
    }
    let column = |hash: u32| -> Result<Option<&Vec<Value>>, LightTableError> {
        match props.get(hash).map(|p| &p.kind) {
            Some(Kind::Array(values)) => {
                if values.len() != types.len() {
                    return Err(LightTableError::ColumnLengthMismatch {
                        hash,
                        expected: types.len(),
                        actual: values.len(),
                    });
                }
                Ok(Some(values))
            }
            _ => Ok(None),
        }
    };
    let classes = column(LIGHT_CLASS)?;
    let colors = column(LIGHT_COLOR)?;
    let radii = column(LIGHT_RADIUS)?;
    let inner = column(LIGHT_INNER_RADIUS)?;
    let params = column(LIGHT_PARAM)?;
    let channels = column(LIGHT_CHANNEL)?;

    let mut lights = Vec::with_capacity(types.len());
    for (index, (type_value, transform_value)) in types.iter().zip(transforms).enumerate() {
        let Value::Transform(raw) = transform_value else {
            return Err(LightTableError::UnsupportedTransform { index });
        };
        let Some(local) = Transform34::from_transform(raw) else {
            return Err(LightTableError::UnsupportedTransform { index });
        };
        let type_hash = match type_value {
            Value::Key(k) => k.instance,
            Value::UInt32(v) => *v,
            _ => return Err(LightTableError::NotALightTable),
        };
        let class = match classes.map(|c| &c[index]) {
            Some(Value::Key(k)) => k.instance,
            _ => 0,
        };
        let color = match colors.map(|c| &c[index]) {
            Some(Value::ColorRgb { r, g, b }) => [*r, *g, *b],
            _ => [1.0, 1.0, 1.0],
        };
        let float_at = |values: Option<&Vec<Value>>, default: f32| match values.map(|v| &v[index]) {
            Some(Value::Float(v)) => *v,
            Some(Value::Int32(v)) => *v as f32,
            _ => default,
        };
        lights.push(LightDef {
            index,
            kind: LightKind::from_hash(type_hash),
            class,
            local,
            color,
            radius: float_at(radii, 0.0),
            inner_radius: float_at(inner, 0.0),
            param: float_at(params, 0.0),
            channel: match channels.map(|c| &c[index]) {
                Some(Value::String8(s)) | Some(Value::String16(s)) => s.clone(),
                _ => String::new(),
            },
        });
    }
    Ok(LightTable { lights })
}

/// 单位变换下的灯世界位置（城市局部帧，除非单位变换本身是区域帧）。
pub fn light_world_positions(unit: &Transform34, table: &LightTable) -> Vec<[f32; 3]> {
    table
        .lights
        .iter()
        .map(|l| unit.transform_point(l.local.translation))
        .collect()
}

/// 候选坐标系。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LightFrame {
    /// 算出来的位置已经是城市局部坐标。
    CityLocal,
    /// 位置是区域坐标，需要减去城市原点。
    Region,
    /// 位置被重复叠加了城市原点。
    DoubleOffset,
}

/// 坐标系假设检验：三个假设里哪个让**全部**灯落进 `PlayArea`。
///
/// 三个假设对应 `offset` 分别为 `0`、`-city_origin`（源数据是区域坐标）与
/// `-2×city_origin`（源数据被重复叠加了一次城市原点）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameResolution {
    pub frame: LightFrame,
    /// 需要施加到位置上的平移修正（米）。
    pub offset: [f32; 2],
    pub inside_before: usize,
    pub inside_after: usize,
    pub total: usize,
    /// 三个假设的命中数（`[cityLocal, region, doubleOffset]`）。
    pub hypothesis_hits: [usize; 3],
}

/// 对一组世界位置做坐标系假设检验。`city_origin` = 该城市在区域中的原点
/// （`mPosInRegion`）。
pub fn resolve_frame(
    positions: &[[f32; 3]],
    play_area: &PlayArea,
    city_origin: [f32; 2],
) -> FrameResolution {
    let count_inside = |dx: f32, dy: f32| {
        positions
            .iter()
            .filter(|p| play_area.contains_xy([p[0] + dx, p[1] + dy]))
            .count()
    };
    let hits = [
        count_inside(0.0, 0.0),
        count_inside(-city_origin[0], -city_origin[1]),
        count_inside(-2.0 * city_origin[0], -2.0 * city_origin[1]),
    ];
    let total = positions.len();
    let inside_before = hits[0];
    // 选择命中数最多的假设；平手时优先 CityLocal（改动最小）。
    let (index, offset) = if hits[0] >= hits[1] && hits[0] >= hits[2] {
        (0, [0.0, 0.0])
    } else if hits[1] >= hits[2] {
        (1, [-city_origin[0], -city_origin[1]])
    } else {
        (2, [-2.0 * city_origin[0], -2.0 * city_origin[1]])
    };
    let frame = match index {
        0 => LightFrame::CityLocal,
        1 => LightFrame::Region,
        _ => LightFrame::DoubleOffset,
    };
    FrameResolution {
        frame,
        offset,
        inside_before,
        inside_after: hits[index],
        total,
        hypothesis_hits: hits,
    }
}

/// 单盏灯的审计问题。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LightIssue {
    /// 灯在 `PlayArea` 之外。
    OutsidePlayArea { overshoot: [f32; 2] },
    /// eco 采样被夹取（`|x|` 或 `|y|` 越过 `flatScale`）。
    EcoClamped { sample: EcoSample },
    /// 灯位置比地面低（使用 `PlayArea.z` 下限；z 未知时不判定）。
    BelowPlayAreaZ { z: f32, min_z: f32 },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightReport {
    pub index: usize,
    pub kind: LightKind,
    pub channel: String,
    pub local: [f32; 3],
    pub world: [f32; 3],
    /// 叠加城市原点后的区域坐标。
    pub region: Option<[f32; 3]>,
    pub inside_play_area: bool,
    pub eco: EcoSample,
    pub issues: Vec<LightIssue>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LightAudit {
    pub lights: Vec<LightReport>,
    pub outside_play_area: usize,
    pub eco_clamped: usize,
    pub frame: Option<FrameResolution>,
}

/// 审计一张灯表在某单位变换下的世界布局。
pub fn audit(
    table: &LightTable,
    unit: &Transform34,
    play_area: &PlayArea,
    grid: EcoGrid,
    city_origin: Option<[f32; 2]>,
) -> LightAudit {
    let positions = light_world_positions(unit, table);
    let frame = city_origin.map(|o| resolve_frame(&positions, play_area, o));
    let mut lights = Vec::with_capacity(table.lights.len());
    let (mut outside, mut clamped) = (0usize, 0usize);
    for (def, world) in table.lights.iter().zip(&positions) {
        let inside_play_area = play_area.contains_xy([world[0], world[1]]);
        let eco = grid.sample([world[0], world[1]]);
        let mut issues = Vec::new();
        if !inside_play_area {
            outside += 1;
            let overshoot = [
                (play_area.min_xy[0] - world[0]).max(world[0] - play_area.max_xy[0]),
                (play_area.min_xy[1] - world[1]).max(world[1] - play_area.max_xy[1]),
            ];
            issues.push(LightIssue::OutsidePlayArea { overshoot });
        }
        if !eco.in_grid() {
            clamped += 1;
            issues.push(LightIssue::EcoClamped { sample: eco });
        }
        match play_area.z {
            Some([min_z, _]) if world[2] < min_z - 1e-3 => {
                issues.push(LightIssue::BelowPlayAreaZ { z: world[2], min_z });
            }
            _ => {}
        }
        lights.push(LightReport {
            index: def.index,
            kind: def.kind,
            channel: def.channel.clone(),
            local: def.local.translation,
            world: *world,
            region: city_origin.map(|o| [world[0] + o[0], world[1] + o[1], world[2]]),
            inside_play_area,
            eco,
            issues,
        });
    }
    LightAudit {
        lights,
        outside_play_area: outside,
        eco_clamped: clamped,
        frame,
    }
}

/// 给一组灯位置施加平移修正，返回 `(修正后位置, 最大位移)`。
pub fn realign(positions: &[[f32; 3]], offset: [f32; 2]) -> (Vec<[f32; 3]>, f32) {
    let mut max_shift = 0.0f32;
    let shifted = positions
        .iter()
        .map(|p| {
            let out = [p[0] + offset[0], p[1] + offset[1], p[2]];
            max_shift = max_shift.max(((out[0] - p[0]).powi(2) + (out[1] - p[1]).powi(2)).sqrt());
            out
        })
        .collect();
    (shifted, max_shift)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Key, PropType, Property};

    fn prop(hash: u32, prop_type: PropType, kind: Kind) -> Property {
        Property {
            hash,
            kind,
            prop_type,
            encoding: Default::default(),
        }
    }

    fn key(instance: u32) -> Value {
        Value::Key(Key {
            instance,
            type_id: 0,
            group: 0,
        })
    }

    fn transform(x: f32, y: f32, z: f32) -> Value {
        Value::Transform(Transform {
            flags: 0x000e,
            unknown: None,
            matrix: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, y, z],
        })
    }

    fn table() -> LightTable {
        let types = vec![key(LIGHT_TYPE_POINT), key(LIGHT_TYPE_LINE)];
        let props = PropertyFile {
            values: vec![
                prop(LIGHT_TYPE, PropType::Key, Kind::Array(types)),
                prop(
                    LIGHT_TRANSFORM,
                    PropType::Transform,
                    Kind::Array(vec![transform(10.0, 0.0, 5.0), transform(-10.0, 0.0, 5.0)]),
                ),
                prop(
                    LIGHT_CLASS,
                    PropType::Key,
                    Kind::Array(vec![key(0xBC76_E6BA), key(0xBC76_E6BA)]),
                ),
                prop(
                    LIGHT_CHANNEL,
                    PropType::String8,
                    Kind::Array(vec![
                        Value::String8("F_door".into()),
                        Value::String8("overhang".into()),
                    ]),
                ),
                prop(
                    LIGHT_RADIUS,
                    PropType::Float,
                    Kind::Array(vec![Value::Float(4.0), Value::Float(12.0)]),
                ),
            ],
            claimed_count: 5,
        };
        parse_light_table(&props).unwrap()
    }

    #[test]
    fn parses_retail_light_columns() {
        let table = table();
        assert_eq!(table.lights.len(), 2);
        assert_eq!(table.lights[0].kind, LightKind::Point);
        assert_eq!(table.lights[1].kind, LightKind::Line);
        assert_eq!(table.lights[0].channel, "F_door");
        assert_eq!(table.lights[1].radius, 12.0);
        assert_eq!(table.lights[0].class, 0xBC76_E6BA);
        // 默认颜色在缺列时是中性的，不代表零售取值。
        assert_eq!(table.lights[0].color, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn rejects_non_light_tables_and_mismatched_columns() {
        let empty = PropertyFile::default();
        assert_eq!(
            parse_light_table(&empty).unwrap_err(),
            LightTableError::NotALightTable
        );
        let props = PropertyFile {
            values: vec![
                prop(
                    LIGHT_TYPE,
                    PropType::Key,
                    Kind::Array(vec![key(LIGHT_TYPE_POINT)]),
                ),
                prop(
                    LIGHT_TRANSFORM,
                    PropType::Transform,
                    Kind::Array(vec![transform(0.0, 0.0, 0.0); 2]),
                ),
            ],
            claimed_count: 2,
        };
        assert!(matches!(
            parse_light_table(&props).unwrap_err(),
            LightTableError::ColumnLengthMismatch { .. }
        ));
    }

    #[test]
    fn unit_transform_places_lights_in_world() {
        let table = table();
        let unit = Transform34::from_translation([100.0, 200.0, 0.0]);
        let world = light_world_positions(&unit, &table);
        assert_eq!(world[0], [110.0, 200.0, 5.0]);
        assert_eq!(world[1], [90.0, 200.0, 5.0]);
    }

    #[test]
    fn compose_matches_sequential_application() {
        let unit = Transform34::from_translation([50.0, -20.0, 3.0]);
        let local = Transform34::from_translation([2.0, 4.0, 1.0]);
        let combined = unit.compose(&local);
        let naive = unit.transform_point(local.transform_point([0.0; 3]));
        assert_eq!(combined.translation, naive);
    }

    #[test]
    fn audit_flags_lights_outside_the_city_box() {
        let table = table();
        let area = PlayArea::retail_city([0.0, 0.0]);
        let grid = EcoGrid::retail([128, 128]);
        // 单位被推到盒外（1600, 0）：两盏灯都在 PlayArea 外且 eco 被夹取。
        let unit = Transform34::from_translation([1600.0, 0.0, 0.0]);
        let audit = audit(&table, &unit, &area, grid, None);
        assert_eq!(audit.outside_play_area, 2);
        assert_eq!(audit.eco_clamped, 2);
        let report = &audit.lights[0];
        assert!(!report.inside_play_area);
        assert!(report.issues.iter().any(|i| matches!(
            i,
            LightIssue::OutsidePlayArea { overshoot } if overshoot[0] > 500.0
        )));
    }

    #[test]
    fn audit_keeps_inside_lights_clean() {
        let table = table();
        let area = PlayArea::retail_city([0.0, 0.0]);
        let grid = EcoGrid::retail([128, 128]);
        let unit = Transform34::from_translation([500.0, -300.0, 0.0]);
        let audit = audit(&table, &unit, &area, grid, Some([4096.0, -4096.0]));
        assert_eq!(audit.outside_play_area, 0);
        assert_eq!(audit.eco_clamped, 0);
        let report = &audit.lights[0];
        assert!(report.inside_play_area);
        assert!(report.issues.is_empty());
        assert_eq!(report.region.unwrap(), [4606.0, -4396.0, 5.0]);
    }

    #[test]
    fn resolves_region_frame_with_single_origin_offset() {
        let table = table();
        let area = PlayArea::retail_city([0.0, 0.0]);
        // 灯被放在区域坐标（城市原点 1200, -800），直接用会全部落在盒外。
        let origin = [1200.0, -800.0];
        let unit = Transform34::from_translation([origin[0] + 10.0, origin[1], 0.0]);
        let positions = light_world_positions(&unit, &table);
        let resolution = resolve_frame(&positions, &area, origin);
        assert_eq!(resolution.frame, LightFrame::Region);
        assert_eq!(resolution.offset, [-1200.0, 800.0]);
        assert_eq!(resolution.inside_after, 2);
        let (fixed, max_shift) = realign(&positions, resolution.offset);
        assert!(fixed.iter().all(|p| area.contains_xy([p[0], p[1]])));
        assert!((max_shift - (1200.0f32.powi(2) + 800.0f32.powi(2)).sqrt()).abs() < 1e-2);
    }

    #[test]
    fn resolves_city_local_without_touching_already_correct_lights() {
        let table = table();
        let area = PlayArea::retail_city([0.0, 0.0]);
        let origin = [1200.0, -800.0];
        let unit = Transform34::from_translation([10.0, 0.0, 0.0]);
        let positions = light_world_positions(&unit, &table);
        let resolution = resolve_frame(&positions, &area, origin);
        assert_eq!(resolution.frame, LightFrame::CityLocal);
        assert_eq!(resolution.offset, [0.0, 0.0]);
        assert_eq!(resolution.inside_before, 2);
    }

    #[test]
    fn double_offset_is_detectable() {
        let table = table();
        let area = PlayArea::retail_city([0.0, 0.0]);
        // 城市原点必须大于盒半边长，否则“减一次原点”也会落进盒内，
        // 与“减两次”无法区分（假设检验会退化为平手）。
        let origin = [2000.0, 0.0];
        // 城市局部灯位被重复叠加了两次原点：直接读全部落在盒外，
        // 只有减去 2×origin 才回到正确位置。
        let unit = Transform34::from_translation([origin[0] * 2.0 + 10.0, 0.0, 0.0]);
        let positions = light_world_positions(&unit, &table);
        let resolution = resolve_frame(&positions, &area, origin);
        assert_eq!(resolution.frame, LightFrame::DoubleOffset);
        assert_eq!(resolution.offset, [-2.0 * origin[0], 0.0]);
        assert_eq!(resolution.hypothesis_hits, [0, 0, 2]);
    }
}
