//! CityBox 逻辑范围：`PlayArea` / `PlayCell` 与可审计的扩展计划。
//!
//! 依据 dev 样本反编译：
//!
//! - `GB::cEcoGame::InitGame`（RVA 0x60BDC0）用
//!   `SP::GetPropertyAsBoundingBox(list, 0x07E0AC05, &mPlayArea)` 取逻辑范围，
//!   再由 [`PlayArea::play_cell`] 的同一组公式求 `mPlayCell`。
//! - `SC::cGraphicsCityBoxBoundary::StartGame`（RVA 0x7CB1B0）只按 `PlayArea`
//!   生成约 11 m 的可视边界带，**不是**约束来源。
//! - `SC::cGraphicsGame::StartGame`（RVA 0x7E0D00 附近）用
//!   `mPlayCell` 宽 × 20 调 `ModelWorld::SetWorldBounds`，因此可视世界包围盒
//!   也随 `PlayArea` 变化。
//!
//! 结论：扩展入口是逻辑 `PlayArea`，边界网格是跟随项。任何只改网格的做法都会
//! 得到“画面变大、模拟仍判盒外”的假成功。

use crate::map_frames::{EcoGrid, RegionFrame};
use serde::Serialize;

/// 城市地块默认半边长（米）。
pub const CITY_HALF_EXTENT: f32 = 1024.0;
/// `cGraphicsGame::StartGame` 传给 `SetWorldBounds` 的 play cell 倍数。
pub const MODEL_WORLD_PLAY_CELL_FACTOR: f32 = 20.0;

/// 逻辑范围（`cSPBoundingBox`，`{min.xyz, max.xyz}` 连续 6 个 float）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayArea {
    pub min_xy: [f32; 2],
    pub max_xy: [f32; 2],
    /// 来自 `mPlayArea` 的属性值；静态包无法给出时保持 `None`，
    /// 任何 z 相关判定必须显式标注“未知”，不得用合成值冒充。
    pub z: Option<[f32; 2]>,
    /// `cEcoGame::mMapBounds`（eco 分格数）。
    pub map_bounds: [i32; 2],
}

impl PlayArea {
    /// 以地块中心 + 半边长构造（地块表 `中心 ± 1024` 绑定）。
    pub fn city_box(
        center: [f32; 2],
        half: f32,
        z: Option<[f32; 2]>,
        map_bounds: [i32; 2],
    ) -> Self {
        Self {
            min_xy: [center[0] - half, center[1] - half],
            max_xy: [center[0] + half, center[1] + half],
            z,
            map_bounds,
        }
    }

    /// 零售默认城市盒：中心 ±1024，eco 128²。
    pub fn retail_city(center: [f32; 2]) -> Self {
        Self::city_box(center, CITY_HALF_EXTENT, None, [128, 128])
    }

    pub fn center_xy(&self) -> [f32; 2] {
        [
            (self.min_xy[0] + self.max_xy[0]) * 0.5,
            (self.min_xy[1] + self.max_xy[1]) * 0.5,
        ]
    }

    pub fn half_extent(&self) -> [f32; 2] {
        [
            (self.max_xy[0] - self.min_xy[0]) * 0.5,
            (self.max_xy[1] - self.min_xy[1]) * 0.5,
        ]
    }

    pub fn size_xy(&self) -> [f32; 2] {
        [
            self.max_xy[0] - self.min_xy[0],
            self.max_xy[1] - self.min_xy[1],
        ]
    }

    pub fn contains_xy(&self, p: [f32; 2]) -> bool {
        p[0] >= self.min_xy[0]
            && p[0] <= self.max_xy[0]
            && p[1] >= self.min_xy[1]
            && p[1] <= self.max_xy[1]
    }

    /// `GB::cEcoGame::InitGame` 的 `mPlayCell` 公式（逐行对应反编译）。
    ///
    /// ```text
    /// min = playArea.min
    /// max.x = (max.x - min.x) / mapBounds.x + min.x
    /// max.y = (max.y - min.y) / mapBounds.y + min.y   // 反编译先把中间值写进 v16
    /// max.z = min.z + (max.z - min.z)
    /// ```
    pub fn play_cell(&self) -> PlayCell {
        let [cells_x, cells_y] = self.map_bounds;
        let width = self.size_xy()[0];
        let height = self.size_xy()[1];
        let max_x = if cells_x > 0 {
            width / cells_x as f32 + self.min_xy[0]
        } else {
            f32::NAN
        };
        let max_y = if cells_y > 0 {
            height / cells_y as f32 + self.min_xy[1]
        } else {
            f32::NAN
        };
        PlayCell {
            min_xy: self.min_xy,
            max_xy: [max_x, max_y],
            z: self.z,
            valid: cells_x > 0 && cells_y > 0 && width > 0.0 && height > 0.0,
        }
    }

    /// `cGraphicsGame::StartGame` 的 `SetWorldBounds` 范围：play cell 宽 × 20。
    pub fn model_world_bound(&self) -> f32 {
        self.play_cell().size_xy()[0] * MODEL_WORLD_PLAY_CELL_FACTOR
    }

    /// 按四边扩展量生成新范围（z 与分格数保持不变）。
    pub fn expand(&self, edges: EdgeExpansion) -> Self {
        Self {
            min_xy: [self.min_xy[0] - edges.west, self.min_xy[1] - edges.south],
            max_xy: [self.max_xy[0] + edges.east, self.max_xy[1] + edges.north],
            z: self.z,
            map_bounds: self.map_bounds,
        }
    }
}

/// `mPlayCell`：单格范围。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayCell {
    pub min_xy: [f32; 2],
    pub max_xy: [f32; 2],
    pub z: Option<[f32; 2]>,
    /// `mapBounds` 任一维 ≤ 0 时为 false（此时区间为 NaN）。
    pub valid: bool,
}

impl PlayCell {
    pub fn size_xy(&self) -> [f32; 2] {
        [
            self.max_xy[0] - self.min_xy[0],
            self.max_xy[1] - self.min_xy[1],
        ]
    }
}

/// 四边扩展量（米，均为非负）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeExpansion {
    pub west: f32,
    pub east: f32,
    pub south: f32,
    pub north: f32,
}

impl EdgeExpansion {
    /// 四边等量扩展。
    pub fn uniform(meters: f32) -> Self {
        Self {
            west: meters,
            east: meters,
            south: meters,
            north: meters,
        }
    }

    /// 目标半边长 → 需要的四边等量扩展（负值归零）。
    pub fn to_half_extent(current_half: f32, target_half: f32) -> Self {
        Self::uniform((target_half - current_half).max(0.0))
    }

    pub fn is_zero(&self) -> bool {
        self.west == 0.0 && self.east == 0.0 && self.south == 0.0 && self.north == 0.0
    }

    pub fn total(&self) -> f32 {
        self.west + self.east + self.south + self.north
    }
}

/// 计划中的一个问题。`Severity::Blocked` 表示该扩展在落地前必须解决。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlayAreaIssue {
    /// 扩展量出现负值。
    NegativeExpansion { edge: &'static str, value: f32 },
    /// `mMapBounds` 某维 ≤ 0，`mPlayCell` 会退化为 NaN。
    DegeneratePlayCell { map_bounds: [i32; 2] },
    /// 新范围超出 eco 定义域（±flatScale）。盒外采样会被夹到格边。
    ExceedsEcoGrid {
        axis: &'static str,
        limit: f32,
        requested: f32,
    },
    /// 新范围超出区域可见范围，界外地形采样退化为格边值。
    ExceedsRegion {
        axis: &'static str,
        requested: f32,
        limit: f32,
    },
    /// 扩展量不是 eco 单格边长的整数倍：道路/分区格网与新边界不同相位。
    NotCellAligned {
        edge: &'static str,
        value: f32,
        cell: f32,
    },
    /// 中心发生位移（旧范围内对象会整体平移）。
    CenterShift { dx: f32, dy: f32 },
    /// z 未知：相机与地面判定无法在离线阶段验证。
    UnknownZ,
    /// 目标半边长小于当前值（会收缩城市盒）。
    Shrinks { current_half: f32, target_half: f32 },
}

impl PlayAreaIssue {
    pub fn is_blocking(&self) -> bool {
        matches!(
            self,
            Self::NegativeExpansion { .. }
                | Self::DegeneratePlayCell { .. }
                | Self::ExceedsRegion { .. }
                | Self::Shrinks { .. }
        )
    }
}

/// 一次扩展的完整审计结果（旧值、新值、中心点、四边扩展量与问题清单）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayAreaPlan {
    pub before: PlayArea,
    pub after: PlayArea,
    pub edges: EdgeExpansion,
    pub cell_before: PlayCell,
    pub cell_after: PlayCell,
    pub center_shift: [f32; 2],
    /// 旧范围四角是否仍在新范围内（应为 true）。
    pub contains_old_range: bool,
    pub issues: Vec<PlayAreaIssue>,
}

impl PlayAreaPlan {
    /// 允许落地：无阻塞问题，且新旧范围都给出可用的 `mPlayCell`。
    pub fn is_applicable(&self) -> bool {
        self.cell_after.valid && !self.issues.iter().any(PlayAreaIssue::is_blocking)
    }

    pub fn blocking(&self) -> impl Iterator<Item = &PlayAreaIssue> {
        self.issues.iter().filter(|i| i.is_blocking())
    }
}

/// 生成扩展计划。纯函数：不读写包、不依赖运行时。
///
/// 判定顺序与验收顺序一致：先格网/区域边界，再对齐与位移，最后 z。
pub fn plan_expansion(
    area: &PlayArea,
    edges: EdgeExpansion,
    region: RegionFrame,
    grid: EcoGrid,
) -> PlayAreaPlan {
    let after = area.expand(edges);
    let cell_before = area.play_cell();
    let cell_after = after.play_cell();
    let mut issues = Vec::new();

    for (edge, value) in [
        ("west", edges.west),
        ("east", edges.east),
        ("south", edges.south),
        ("north", edges.north),
    ] {
        if value < 0.0 {
            issues.push(PlayAreaIssue::NegativeExpansion { edge, value });
        }
    }
    if area.map_bounds[0] <= 0 || area.map_bounds[1] <= 0 {
        issues.push(PlayAreaIssue::DegeneratePlayCell {
            map_bounds: area.map_bounds,
        });
    }

    // eco 定义域：超出后采样被夹到格边（flatScale = 1024 = 城市盒半边）。
    for (axis, requested) in [("x", after.max_xy[0].abs()), ("y", after.max_xy[1].abs())] {
        if requested > grid.flat_scale {
            issues.push(PlayAreaIssue::ExceedsEcoGrid {
                axis,
                limit: grid.flat_scale,
                requested,
            });
        }
    }
    // 区域可见范围：界外地形采样走 `PointSampleHeightInRegion` 的格边值。
    for (axis, point) in [
        ("x", [after.max_xy[0], after.min_xy[1]]),
        ("y", [after.min_xy[0], after.max_xy[1]]),
    ] {
        let outside = if axis == "x" {
            !region.contains([point[0], 0.0])
        } else {
            !region.contains([0.0, point[1]])
        };
        if outside {
            issues.push(PlayAreaIssue::ExceedsRegion {
                axis,
                requested: if axis == "x" { point[0] } else { point[1] },
                limit: if axis == "x" {
                    region.origin[0] + region.side
                } else {
                    region.origin[1] + region.side
                },
            });
        }
    }
    // 相位：扩展量必须是 eco 单格边长整数倍，否则新边界与格网错相。
    let cell = grid.cell();
    for (edge, value) in [
        ("west", edges.west),
        ("east", edges.east),
        ("south", edges.south),
        ("north", edges.north),
    ] {
        if value > 0.0 && ((value / cell).fract()).abs() > 1e-3 {
            issues.push(PlayAreaIssue::NotCellAligned { edge, value, cell });
        }
    }
    let center_shift = [
        after.center_xy()[0] - area.center_xy()[0],
        after.center_xy()[1] - area.center_xy()[1],
    ];
    if center_shift[0].abs() > 1e-4 || center_shift[1].abs() > 1e-4 {
        issues.push(PlayAreaIssue::CenterShift {
            dx: center_shift[0],
            dy: center_shift[1],
        });
    }
    let before_half = area.half_extent()[0];
    let after_half = after.half_extent()[0];
    if after_half + 1e-4 < before_half {
        issues.push(PlayAreaIssue::Shrinks {
            current_half: before_half,
            target_half: after_half,
        });
    }
    if after.z.is_none() {
        issues.push(PlayAreaIssue::UnknownZ);
    }

    let contains_old_range = [
        area.min_xy,
        area.max_xy,
        [area.min_xy[0], area.max_xy[1]],
        [area.max_xy[0], area.min_xy[1]],
    ]
    .iter()
    .all(|p| after.contains_xy(*p));

    PlayAreaPlan {
        before: *area,
        after,
        edges,
        cell_before,
        cell_after,
        center_shift,
        contains_old_range,
        issues,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_frames::EcoGrid;

    fn retail() -> (PlayArea, EcoGrid, RegionFrame) {
        (
            PlayArea::retail_city([0.0, 0.0]),
            EcoGrid::retail([128, 128]),
            RegionFrame::retail(),
        )
    }

    #[test]
    fn play_cell_matches_init_game_formula() {
        let area = PlayArea::city_box([0.0, 0.0], 1024.0, Some([0.0, 300.0]), [128, 128]);
        let cell = area.play_cell();
        assert!(cell.valid);
        // 2048 / 128 = 16
        assert_eq!(cell.size_xy(), [16.0, 16.0]);
        assert_eq!(cell.z, Some([0.0, 300.0]));
        // model world bound = play cell 宽 × 20
        assert_eq!(area.model_world_bound(), 320.0);
    }

    #[test]
    fn zero_map_bounds_degenerates() {
        let area = PlayArea::city_box([0.0, 0.0], 1024.0, None, [0, 128]);
        let cell = area.play_cell();
        assert!(!cell.valid);
        assert!(cell.size_xy()[0].is_nan());
    }

    #[test]
    fn uniform_expansion_keeps_center_and_reports_eco_overflow() {
        let (area, grid, region) = retail();
        let plan = plan_expansion(&area, EdgeExpansion::uniform(512.0), region, grid);
        assert_eq!(plan.after.half_extent(), [1536.0, 1536.0]);
        assert_eq!(plan.center_shift, [0.0, 0.0]);
        assert!(plan.contains_old_range);
        // 1536 > flatScale 1024：盒外采样会夹取，必须显式报出。
        assert!(
            plan.issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::ExceedsEcoGrid { .. }))
        );
        // 512 是 16 m 单格整数倍 → 不报相位问题。
        assert!(
            !plan
                .issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::NotCellAligned { .. }))
        );
        assert!(plan.is_applicable(), "eco 溢出是警告而非阻塞");
    }

    #[test]
    fn asymmetric_expansion_shifts_center() {
        let (area, grid, region) = retail();
        let plan = plan_expansion(
            &area,
            EdgeExpansion {
                west: 0.0,
                east: 256.0,
                south: 0.0,
                north: 0.0,
            },
            region,
            grid,
        );
        assert_eq!(plan.center_shift, [128.0, 0.0]);
        assert!(
            plan.issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::CenterShift { .. }))
        );
    }

    #[test]
    fn unaligned_edge_is_flagged_but_not_blocking() {
        let (area, grid, region) = retail();
        let plan = plan_expansion(&area, EdgeExpansion::uniform(10.0), region, grid);
        assert!(
            plan.issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::NotCellAligned { .. }))
        );
        assert!(plan.is_applicable());
    }

    #[test]
    fn shrinking_and_negative_edges_block() {
        let (area, grid, region) = retail();
        let shrink = plan_expansion(
            &area,
            EdgeExpansion::to_half_extent(1024.0, 512.0),
            region,
            grid,
        );
        // to_half_extent 只产生非负扩展 → 不缩小；显式 -1 才算负值。
        assert!(shrink.is_applicable());
        let negative = plan_expansion(
            &area,
            EdgeExpansion {
                west: -1.0,
                ..EdgeExpansion::default()
            },
            region,
            grid,
        );
        assert!(!negative.is_applicable());
        assert!(
            negative
                .issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::NegativeExpansion { .. }))
        );
    }

    #[test]
    fn region_overflow_blocks() {
        let area = PlayArea::city_box([16000.0, 0.0], 1024.0, None, [128, 128]);
        let plan = plan_expansion(
            &area,
            EdgeExpansion::uniform(1024.0),
            RegionFrame::retail(),
            EcoGrid::retail([128, 128]),
        );
        assert!(!plan.is_applicable());
        assert!(
            plan.issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::ExceedsRegion { .. }))
        );
    }

    #[test]
    fn to_half_extent_never_shrinks() {
        assert_eq!(EdgeExpansion::to_half_extent(1024.0, 2048.0).east, 1024.0);
        assert_eq!(EdgeExpansion::to_half_extent(2048.0, 1024.0).east, 0.0);
    }

    #[test]
    fn unknown_z_is_reported() {
        let (area, grid, region) = retail();
        let plan = plan_expansion(&area, EdgeExpansion::uniform(256.0), region, grid);
        assert!(
            plan.issues
                .iter()
                .any(|i| matches!(i, PlayAreaIssue::UnknownZ))
        );
    }
}
