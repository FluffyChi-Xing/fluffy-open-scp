//! 相机钳制的三层模型与解除计划。
//!
//! 依据 dev 样本反编译（零售 RVA 另见 `map-consumers-ui-bridge-camera.md`）：
//!
//! 1. `SC::cCameraControllerBase::ConstrainTarget`（RVA 0x391830）每帧从
//!    `Game()->PlayArea()` 取 6 个边界值，对 `mLerpSubject.mTarget` 的 x/y/z 夹取。
//! 2. `Update`（RVA 0x395B90）的 `mRelativeMove != 0` 分支先按同一 `PlayArea`
//!    夹取 x/y/z，再对 x/y 做**绝对值**判定：`>= 1024` → `1020`，
//!    `<= -1024` → `-1020`；随后做地面采样，最后仍调用 `ConstrainTarget`。
//! 3. `CameraEyeConstraints`（RVA 0x393530）按 `PlayArea` 内外选择地面采样，
//!    界外走 `PointSampleHeightInRegion`，承担防穿地，不能整段禁用。
//!
//! 关键结论：第 2 层是**绝对坐标**常量，与 `PlayArea` 大小无关。默认城市盒
//! 半边长正好 1024，所以它看起来“和边界重合”；一旦把 `PlayArea` 扩到 1024
//! 以外，镜头反而被钉在 ±1020，比原边界还短 4 m。因此“扩 citybox 顺手解锁
//! 镜头”不仅无效，还会让可移动范围变小。
//!
//! 坐标约定：`PlayArea` 必须使用**城市局部坐标**（0 为中心）；区域坐标与
//! 城市局部坐标之间只差一个 `mPosInRegion` 平移（`PointSampleHeightInRegion`
//! 即 `regionPos = mPosInRegion + worldPos`）。

use crate::play_area::PlayArea;
use serde::Serialize;

/// 零售相对移动分支的绝对 XY 上限（米）。
pub const RETAIL_ABSOLUTE_XY_LIMIT: f32 = 1024.0;
/// 越界后写入的夹取值（米）。
pub const RETAIL_RELATIVE_CLAMP: f32 = 1020.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CameraMode {
    /// 城市内常规控制器（含 WASD/拖拽/边缘滚动/跟随）。
    City,
    /// `regionedit`：`cTerrainSystem::SetRegionAuthoringActive` 切换的控制器，
    /// 只切相机并关闭 dof/motion blur，不解除 clamp。
    RegionEdit,
    /// 假定已替换 clamp 来源的自由漫游（本仓库不提供该原生补丁）。
    Free,
}

/// 命中过哪一层夹取。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ClampLayer {
    PlayAreaX,
    PlayAreaY,
    PlayAreaZ,
    /// 相对移动分支的 ±1024 → ±1020 绝对常量。
    AbsoluteX,
    AbsoluteY,
}

/// 一次 `Update` 的夹取轨迹（不含地面采样与插值，只覆盖边界层）。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraTrace {
    pub input: [f32; 3],
    /// 第 1 层（`PlayArea`）之后。
    pub after_play_area: [f32; 3],
    /// 第 2 层（绝对 ±1024/±1020）之后；非相对移动分支为 `None`。
    pub after_absolute: Option<[f32; 3]>,
    /// 最终写回 `mLerpSubject.mTarget` 的值（第 1 层在末尾会再夹一次，幂等）。
    pub final_target: [f32; 3],
    pub layers: Vec<ClampLayer>,
}

/// 相机约束剖面。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraConstraintProfile {
    pub mode: CameraMode,
    /// 城市局部坐标下的逻辑范围。
    pub play_area: PlayArea,
    /// `None` = 已替换为自由边界；零售城市控制器为 `Some(1024)`。
    pub absolute_xy_limit: Option<f32>,
    /// 越界后写入的夹取值（零售 1020）。
    pub relative_move_clamp: f32,
    /// 是否仍保留防穿地采样（应恒为 true；仅作审计记录）。
    pub preserves_ground_sample: bool,
}

impl CameraConstraintProfile {
    pub fn retail_city(play_area: PlayArea) -> Self {
        Self {
            mode: CameraMode::City,
            play_area,
            absolute_xy_limit: Some(RETAIL_ABSOLUTE_XY_LIMIT),
            relative_move_clamp: RETAIL_RELATIVE_CLAMP,
            preserves_ground_sample: true,
        }
    }

    /// `regionedit` 与城市控制器共用同一 clamp 链，仅渲染后处理不同。
    pub fn retail_region_edit(play_area: PlayArea) -> Self {
        Self {
            mode: CameraMode::RegionEdit,
            ..Self::retail_city(play_area)
        }
    }

    /// 有效可移动半幅：两层取较小值。`absolute_xy_limit` 为 `None` 时只受
    /// `PlayArea` 限制。
    pub fn reachable_half_extent(&self) -> [f32; 2] {
        let half = self.play_area.half_extent();
        match self.absolute_xy_limit {
            Some(limit) => [half[0].min(limit), half[1].min(limit)],
            None => half,
        }
    }

    /// 绝对层造成的额外内缩（`limit - clamp`，零售 4 m）。第 2 层触发时，
    /// 目标会被写到 `±clamp`，因此可用上界实际是 `limit` 而不是 `clamp`，
    /// 但“刚好到边”的目标会被拉回 `clamp`。
    pub fn absolute_inset(&self) -> f32 {
        match self.absolute_xy_limit {
            Some(limit) => (limit - self.relative_move_clamp).max(0.0),
            None => 0.0,
        }
    }

    /// 复现两层夹取。`relative_move = false` 只走 `ConstrainTarget`。
    pub fn evaluate(&self, target: [f32; 3], relative_move: bool) -> CameraTrace {
        let mut layers = Vec::new();
        let [mut x, mut y, mut z] = target;
        let min = self.play_area.min_xy;
        let max = self.play_area.max_xy;
        if x < min[0] {
            x = min[0];
            layers.push(ClampLayer::PlayAreaX);
        } else if x > max[0] {
            x = max[0];
            layers.push(ClampLayer::PlayAreaX);
        }
        if y < min[1] {
            y = min[1];
            layers.push(ClampLayer::PlayAreaY);
        } else if y > max[1] {
            y = max[1];
            layers.push(ClampLayer::PlayAreaY);
        }
        if let Some(zr) = self.play_area.z {
            if z < zr[0] {
                z = zr[0];
                layers.push(ClampLayer::PlayAreaZ);
            } else if z > zr[1] {
                z = zr[1];
                layers.push(ClampLayer::PlayAreaZ);
            }
        }
        let after_play_area = [x, y, z];
        let mut after_absolute = None;
        if let (true, Some(limit)) = (relative_move, self.absolute_xy_limit) {
            if x >= limit {
                x = self.relative_move_clamp;
                layers.push(ClampLayer::AbsoluteX);
            } else if x <= -limit {
                x = -self.relative_move_clamp;
                layers.push(ClampLayer::AbsoluteX);
            }
            if y >= limit {
                y = self.relative_move_clamp;
                layers.push(ClampLayer::AbsoluteY);
            } else if y <= -limit {
                y = -self.relative_move_clamp;
                layers.push(ClampLayer::AbsoluteY);
            }
            after_absolute = Some([x, y, z]);
        }
        CameraTrace {
            input: target,
            after_play_area,
            after_absolute,
            final_target: [x, y, z],
            layers,
        }
    }

    /// 生成“扩到 `target_half` 后仍能看到”的解除计划。
    pub fn plan_unlock(&self, target_half: f32) -> CameraUnlockPlan {
        let current = self.reachable_half_extent();
        let mut steps = Vec::new();
        let half = self.play_area.half_extent();
        if half[0].min(half[1]) + 1e-4 < target_half {
            steps.push(UnlockStep::ExpandPlayArea {
                from_half: half[0].min(half[1]),
                to_half: target_half,
            });
        }
        match self.absolute_xy_limit {
            Some(limit) if limit < target_half => {
                steps.push(UnlockStep::ReplaceAbsoluteLimit {
                    from: limit,
                    to: target_half,
                });
            }
            None => {}
            Some(_) => {}
        }
        if self.preserves_ground_sample {
            steps.push(UnlockStep::KeepGroundSample);
        }
        steps.push(UnlockStep::VerifyInputs);
        CameraUnlockPlan {
            mode: self.mode,
            current_reach_half: current[0].min(current[1]),
            target_half,
            steps,
            ready: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum UnlockStep {
    /// 扩大逻辑 `PlayArea`（会同时改变模拟域，需另行验收）。
    ExpandPlayArea { from_half: f32, to_half: f32 },
    /// 把相对移动分支的绝对常量换成与 `PlayArea` 同源的边界。
    ReplaceAbsoluteLimit { from: f32, to: f32 },
    /// 保留 `CheckForGroundFromPoint` / `CameraEyeConstraints` 防穿地。
    KeepGroundSample,
    /// 四种输入（拖拽/键盘/边缘滚动/跟随）都走同一 bounds 才算解锁。
    VerifyInputs,
}

/// 解除计划。`ready` 恒为 false：本仓库只产出静态方案，未做原生补丁与运行时验证。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraUnlockPlan {
    pub mode: CameraMode,
    pub current_reach_half: f32,
    pub target_half: f32,
    pub steps: Vec<UnlockStep>,
    pub ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(half: f32) -> PlayArea {
        PlayArea::city_box([0.0, 0.0], half, None, [128, 128])
    }

    #[test]
    fn retail_box_edge_is_pulled_back_to_1020() {
        let profile = CameraConstraintProfile::retail_city(area(1024.0));
        // 目标正好在盒边：第 1 层不动，第 2 层把它写成 1020。
        let at_edge = profile.evaluate([1024.0, 0.0, 5.0], true);
        assert_eq!(at_edge.after_play_area[0], 1024.0);
        assert_eq!(at_edge.final_target[0], 1020.0);
        assert!(at_edge.layers.contains(&ClampLayer::AbsoluteX));
        assert_eq!(profile.absolute_inset(), 4.0);
    }

    #[test]
    fn expanding_play_area_does_not_widen_camera() {
        let retail = CameraConstraintProfile::retail_city(area(1024.0));
        let expanded = CameraConstraintProfile::retail_city(area(2048.0));
        assert_eq!(retail.reachable_half_extent(), [1024.0, 1024.0]);
        // 第二层仍压到 1024：扩盒后有效半幅没有增加。
        assert_eq!(expanded.reachable_half_extent(), [1024.0, 1024.0]);
        // 更糟：盒内 1500 处（旧盒外、新盒内）会被钉到 1020。
        let deep = expanded.evaluate([1500.0, 0.0, 5.0], true);
        assert_eq!(deep.final_target[0], 1020.0);
        // 不使用相对移动时，只受 PlayArea 限制，能到 1500。
        let absolute_free = expanded.evaluate([1500.0, 0.0, 5.0], false);
        assert_eq!(absolute_free.final_target[0], 1500.0);
        assert!(absolute_free.layers.is_empty());
    }

    #[test]
    fn play_area_clamp_applies_without_relative_move() {
        let profile = CameraConstraintProfile::retail_city(area(1024.0));
        let trace = profile.evaluate([2048.0, -2048.0, 0.0], false);
        assert_eq!(trace.final_target[0], 1024.0);
        assert_eq!(trace.final_target[1], -1024.0);
        assert_eq!(
            trace.layers,
            vec![ClampLayer::PlayAreaX, ClampLayer::PlayAreaY]
        );
    }

    #[test]
    fn z_unknown_does_not_fake_a_clamp() {
        let profile = CameraConstraintProfile::retail_city(area(1024.0));
        let trace = profile.evaluate([0.0, 0.0, 1.0e6], false);
        assert_eq!(trace.final_target[2], 1.0e6);
        assert!(!trace.layers.contains(&ClampLayer::PlayAreaZ));

        let with_z = CameraConstraintProfile::retail_city(PlayArea::city_box(
            [0.0, 0.0],
            1024.0,
            Some([0.0, 300.0]),
            [128, 128],
        ));
        let clipped = with_z.evaluate([0.0, 0.0, 1.0e6], false);
        assert_eq!(clipped.final_target[2], 300.0);
        assert!(clipped.layers.contains(&ClampLayer::PlayAreaZ));
    }

    #[test]
    fn unlock_plan_requires_both_layers() {
        let profile = CameraConstraintProfile::retail_city(area(1024.0));
        let plan = profile.plan_unlock(2048.0);
        assert!(!plan.ready, "静态方案不得宣称已解锁");
        assert!(
            plan.steps
                .iter()
                .any(|s| matches!(s, UnlockStep::ExpandPlayArea { .. }))
        );
        assert!(plan.steps.iter().any(|s| matches!(
            s,
            UnlockStep::ReplaceAbsoluteLimit { from, to } if *from == 1024.0 && *to == 2048.0
        )));
        assert!(
            plan.steps
                .iter()
                .any(|s| matches!(s, UnlockStep::KeepGroundSample))
        );
        assert_eq!(plan.current_reach_half, 1024.0);
    }

    #[test]
    fn expanding_by_only_4m_still_needs_limit_change() {
        let profile = CameraConstraintProfile::retail_city(area(1028.0));
        let plan = profile.plan_unlock(1028.0);
        assert!(
            plan.steps
                .iter()
                .any(|s| matches!(s, UnlockStep::ReplaceAbsoluteLimit { .. }))
        );
        assert!(
            !plan
                .steps
                .iter()
                .any(|s| matches!(s, UnlockStep::ExpandPlayArea { .. }))
        );
    }
}
