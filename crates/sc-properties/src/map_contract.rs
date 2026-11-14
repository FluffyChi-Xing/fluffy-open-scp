//! 地图契约汇总报告：一次把范围、格网、tile、相机可见幅与坐标回环算清。
//!
//! 这是给 UI / 探针使用的只读聚合层：不写包、不启动游戏。所有字段都能追到
//! [`crate::play_area`]、[`crate::map_frames`]、[`crate::camera_bounds`] 的
//! 单一实现，避免同一公式在多处复制。
//!
//! 帧约定（必须显式区分）：地块表给的是**区域坐标**；`PlayArea`、
//! `mPlayCell`、eco 格网、相机目标都使用**城市局部坐标**（城市中心为 0）。
//! 两者只差一个平移 `mPosInRegion`（`PointSampleHeightInRegion` 即
//! `regionPos = mPosInRegion + worldPos`）。因此报告同时保留
//! `city_origin`（区域坐标）和 `play_area`（城市局部坐标）。

use crate::camera_bounds::{CameraConstraintProfile, CameraUnlockPlan};
use crate::map_frames::{EcoGrid, EcoSample, RegionFrame, RegionTileFrame};
use crate::play_area::{PlayArea, PlayCell};
use serde::Serialize;

/// 一个地块（城市 / 伟工位）的契约视图。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LandmarkContract {
    pub uid: String,
    pub name: Option<String>,
    /// `"city"` | `"greatwork"`。
    pub kind: String,
    /// 区域坐标下的地块中心（= `mPosInRegion` 的来源）。
    pub city_origin: [f32; 2],
    /// 城市局部坐标下的逻辑范围（中心恒为 0）。
    pub play_area: PlayArea,
    pub cell: PlayCell,
    /// `cGraphicsGame::StartGame` 的 `SetWorldBounds` 半径。
    pub model_world_bound: f32,
    /// 相机有效可移动半幅（两层取小）。
    pub camera_reach_half: [f32; 2],
    /// 绝对层带来的额外内缩（零售 4 m）。
    pub camera_inset: f32,
    /// 把可达半幅翻倍所需的静态方案（`ready` 恒 false）。
    pub camera_plan: CameraUnlockPlan,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoundTripSample {
    pub label: String,
    pub world: [f32; 2],
    pub eco: EcoSample,
    pub back: [f32; 2],
    /// `|back - world|`（米）。
    pub error: f32,
    pub inside_play_area: bool,
    pub inside_eco_grid: bool,
}

/// 地图契约汇总。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MapContractReport {
    pub region: RegionFrame,
    pub grid: EcoGrid,
    pub tiles: RegionTileFrame,
    pub landmarks: Vec<LandmarkContract>,
    /// 以第一个城市为中心的回环样本；无地块时为空。
    pub round_trip: Vec<RoundTripSample>,
}

/// 地块输入：`(uid, 名称, 区域中心, 半边长, kind)`；伟工位常用 768 m 半边长，
/// 城市 1024 m。
pub type PlotSpec = (String, Option<String>, [f32; 2], f32, String);

/// 由地块表生成报告。
pub fn report(region: RegionFrame, grid: EcoGrid, plots: &[PlotSpec]) -> MapContractReport {
    let tiles = RegionTileFrame {
        origin: region.origin,
        ..RegionTileFrame::retail()
    };
    let landmarks: Vec<_> = plots
        .iter()
        .map(|(uid, name, origin, half, kind)| {
            // 区域坐标 → 城市局部坐标：`PlayArea` 与 eco 格网都以城市中心为 0。
            let play_area = PlayArea::city_box([0.0, 0.0], *half, None, [grid.size as i32; 2]);
            let profile = CameraConstraintProfile::retail_city(play_area);
            let reach = profile.reachable_half_extent();
            let target = (play_area.half_extent()[0] * 2.0).max(1.0);
            LandmarkContract {
                uid: uid.clone(),
                name: name.clone(),
                kind: kind.clone(),
                city_origin: *origin,
                play_area,
                cell: play_area.play_cell(),
                model_world_bound: play_area.model_world_bound(),
                camera_reach_half: reach,
                camera_inset: profile.absolute_inset(),
                camera_plan: profile.plan_unlock(target),
            }
        })
        .collect();
    let round_trip = landmarks
        .first()
        .map(|l| samples(&l.play_area, grid))
        .unwrap_or_default();
    MapContractReport {
        region,
        grid,
        tiles,
        landmarks,
        round_trip,
    }
}

/// 以城市盒为基准生成 9 个回环样本：中心、四角、四边外 1.5 倍。
pub fn samples(area: &PlayArea, grid: EcoGrid) -> Vec<RoundTripSample> {
    let [cx, cy] = area.center_xy();
    let [hx, hy] = area.half_extent();
    let mut points = vec![("center", [cx, cy])];
    for (label, x, y) in [
        ("min-min", cx - hx, cy - hy),
        ("max-min", cx + hx, cy - hy),
        ("min-max", cx - hx, cy + hy),
        ("max-max", cx + hx, cy + hy),
    ] {
        points.push((label, [x, y]));
    }
    for (label, x, y) in [
        ("outside-west", cx - hx * 1.5, cy),
        ("outside-east", cx + hx * 1.5, cy),
        ("outside-south", cx, cy - hy * 1.5),
        ("outside-north", cx, cy + hy * 1.5),
    ] {
        points.push((label, [x, y]));
    }
    points
        .into_iter()
        .map(|(label, world)| {
            let eco = grid.sample(world);
            let back = grid.world(&eco);
            RoundTripSample {
                label: label.to_string(),
                world,
                eco,
                back,
                error: ((back[0] - world[0]).powi(2) + (back[1] - world[1]).powi(2)).sqrt(),
                inside_play_area: area.contains_xy(world),
                inside_eco_grid: eco.in_grid(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plots() -> Vec<(String, Option<String>, [f32; 2], f32, String)> {
        vec![
            (
                "1026".to_string(),
                Some("CapeTrinity".to_string()),
                [3000.0, -2000.0],
                1024.0,
                "city".to_string(),
            ),
            (
                "greatwork:arcology".to_string(),
                None,
                [-4000.0, 4000.0],
                768.0,
                "greatwork".to_string(),
            ),
        ]
    }

    #[test]
    fn report_exposes_retail_camera_limits() {
        let report = report(RegionFrame::retail(), EcoGrid::retail([128, 128]), &plots());
        assert_eq!(report.landmarks.len(), 2);
        let city = &report.landmarks[0];
        assert_eq!(city.camera_reach_half, [1024.0, 1024.0]);
        assert_eq!(city.camera_inset, 4.0);
        assert_eq!(city.cell.size_xy(), [16.0, 16.0]);
        assert_eq!(city.model_world_bound, 320.0);
        assert!(!city.camera_plan.ready);
        // 计划目标 = 当前半幅 ×2：两层都必须改。
        assert_eq!(city.camera_plan.target_half, 2048.0);
        let greatwork = &report.landmarks[1];
        assert_eq!(greatwork.camera_reach_half, [768.0, 768.0]);
    }

    #[test]
    fn round_trip_is_exact_inside_and_flags_outside() {
        let report = report(RegionFrame::retail(), EcoGrid::retail([128, 128]), &plots());
        assert_eq!(report.round_trip.len(), 9);
        for sample in &report.round_trip {
            // 回环本身是精确的：夹取只饱和索引，`s/t` 保留溢出量。
            if sample.inside_play_area {
                assert!(sample.error < 1e-3, "{}: {sample:?}", sample.label);
            }
        }
        let center = report
            .round_trip
            .iter()
            .find(|s| s.label == "center")
            .unwrap();
        assert!(center.inside_eco_grid);
        // 盒边恰好等于 flatScale：正方向边界点的索引会饱和到 size-1，
        // 这是引擎行为，必须显式暴露而不是当成“在格网内”。
        let max_max = report
            .round_trip
            .iter()
            .find(|s| s.label == "max-max")
            .unwrap();
        assert!(max_max.inside_play_area);
        assert!(!max_max.inside_eco_grid && max_max.eco.clamped);
        // 盒外 1.5 倍同样越出 eco 定义域。
        let outside = report
            .round_trip
            .iter()
            .find(|s| s.label == "outside-east")
            .unwrap();
        assert!(!outside.inside_play_area);
        assert!(!outside.inside_eco_grid);
        assert!(outside.eco.clamped || outside.eco.underflow);
    }

    #[test]
    fn empty_plot_table_yields_no_samples() {
        let report = report(RegionFrame::retail(), EcoGrid::retail([128, 128]), &[]);
        assert!(report.landmarks.is_empty());
        assert!(report.round_trip.is_empty());
    }
}
