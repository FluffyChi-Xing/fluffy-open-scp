//! 地图三套坐标契约：`WorldFrame` / `EcoGridFrame` / `RegionTileFrame`。
//!
//! 依据 dev 样本反编译与零售包实测（见
//! `docs/re/map-coordinate-contract-20261102.md`）：
//!
//! - `SC::cTerrainMapScalars::SetFlatScale`（RVA 0x42A790）设置 `mFlatScale`；
//!   `SC::cTerrainGfx::FillFromProps` 从 config 属性 `0xB8F6B4CF` 读取。
//!   零售包实测 `flatScale = 1024`（`SimCity_Game.package`
//!   `00B1B104:40232000:A2E2DF52`，同资源给出 heightScale 2048 / heightBias -1024）。
//! - `SC::cTerrainMapSet::WorldToEco`（RVA 0x42A910）把连续世界坐标映射到
//!   `[-flatScale, +flatScale]²` 的 eco 网格，只在高侧夹取索引。
//! - 341 高度 tile 金字塔（`crate::region_write`）：4096² 场、1 px = 8 m、
//!   tile 256² px = 2048 m，区域全幅 32768 m。
//!
//! 因此“城市盒外”不是另一套世界原点，而是 eco 采样的定义域边界：`flatScale`
//! 恰好等于城市盒半边长 1024 m，城市盒以外的坐标会先被夹到格边再采样。

use serde::Serialize;

/// 零售 `mFlatScale`（米）。eco 定义域为 `[-flatScale, +flatScale]²`。
pub const DEFAULT_FLAT_SCALE: f32 = 1024.0;
/// 零售 heightScale：`raw = (z + 1024) × 32`。
pub const DEFAULT_HEIGHT_SCALE: f32 = 2048.0;
/// 零售 heightBias（米）。
pub const DEFAULT_HEIGHT_BIAS: f32 = -1024.0;
/// 城市地块半边长（米）：地块表以 `中心 ± 1024` 绑定。
pub const CITY_HALF_EXTENT: f32 = 1024.0;
/// 区域高度场每像素米数（`region_write::REGION_FIELD_PX` = 4096 px → 32768 m）。
pub const REGION_METERS_PER_PIXEL: f32 = 8.0;
/// 单张 F0 tile 边长（像素，= 2048 m）。
pub const REGION_TILE_PX: u32 = 256;

/// 一个 eco 采样结果。`clamped` 表示索引被引擎夹到 `size-1`；
/// `underflow` 表示世界坐标落在网格下界之外（引擎不夹取，`s/t` 会小于 0）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoSample {
    pub x: i32,
    pub y: i32,
    pub s: f32,
    pub t: f32,
    pub clamped: bool,
    pub underflow: bool,
}

impl EcoSample {
    /// 采样点是否落在网格内（未被夹取且没有下溢）。
    pub fn in_grid(&self) -> bool {
        !self.clamped && !self.underflow
    }
}

/// `EcoGridFrame`：`WorldToEco` 的定长整数格。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EcoGrid {
    pub size: u32,
    pub flat_scale: f32,
}

impl EcoGrid {
    pub fn new(size: u32, flat_scale: f32) -> Self {
        Self { size, flat_scale }
    }

    /// 零售默认：`size = mMapBounds`（InitGame 用 `mMapBounds[0..2]` 求 `mPlayCell`），
    /// `flatScale = 1024`。
    pub fn retail(map_bounds: [i32; 2]) -> Self {
        Self::new(map_bounds[0].max(1) as u32, DEFAULT_FLAT_SCALE)
    }

    /// 场宽（米）：`2 × flatScale`。
    pub fn span(&self) -> f32 {
        self.flat_scale * 2.0
    }

    /// 单格边长（米）。
    pub fn cell(&self) -> f32 {
        self.span() / self.size as f32
    }

    /// 网格左下角世界坐标。
    pub fn origin(&self) -> [f32; 2] {
        [-self.flat_scale, -self.flat_scale]
    }

    /// `SC::cTerrainMapSet::WorldToEco` 的逐位等价实现。
    ///
    /// 注意：坐标恰为 `+flatScale` 时 `index == size`，会被夹到 `size-1`
    /// 并置 `clamped`——零售 `flatScale` 正好等于城市盒半边长，所以城市盒
    /// 的正方向边界点本身就会饱和到最后一格。
    pub fn sample(&self, p: [f32; 2]) -> EcoSample {
        let flat = self.flat_scale;
        let v6 = p[0] + flat;
        let v7 = p[1] + flat;
        let inv = 1.0 / self.span();
        let size = self.size as f32;
        let mut x = (size * (inv * v6)) as i32;
        let mut y = (size * (inv * v7)) as i32;
        let last = self.size as i32 - 1;
        let mut clamped = false;
        if x > last {
            x = last;
            clamped = true;
        }
        if y > last {
            y = last;
            clamped = true;
        }
        let cell = self.cell();
        let underflow = v6 < 0.0 || v7 < 0.0;
        EcoSample {
            x,
            y,
            s: (1.0 / cell) * (v6 - x as f32 * cell),
            t: (1.0 / cell) * (v7 - y as f32 * cell),
            clamped,
            underflow,
        }
    }

    /// `EcoSample` → 世界坐标（`sample` 的逆）。
    pub fn world(&self, s: &EcoSample) -> [f32; 2] {
        let cell = self.cell();
        [
            s.x as f32 * cell + s.s * cell - self.flat_scale,
            s.y as f32 * cell + s.t * cell - self.flat_scale,
        ]
    }

    /// 点是否在 eco 定义域内（盒外采样会被夹取）。
    pub fn covers(&self, p: [f32; 2]) -> bool {
        p[0].abs() <= self.flat_scale && p[1].abs() <= self.flat_scale
    }
}

/// `RegionFrame`：区域可见范围（`region_3d` 同源：2048² px × 16 m/px）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionFrame {
    /// 区域左下角世界坐标（零售 = `(-16384, -16384)`）。
    pub origin: [f32; 2],
    /// 区域边长（米，零售 = 32768）。
    pub side: f32,
}

impl RegionFrame {
    pub fn retail() -> Self {
        Self {
            origin: [-16384.0, -16384.0],
            side: 32768.0,
        }
    }

    pub fn half(&self) -> f32 {
        self.side * 0.5
    }

    pub fn center(&self) -> [f32; 2] {
        [self.origin[0] + self.half(), self.origin[1] + self.half()]
    }

    /// 区域可见范围内判定（含边界）。
    pub fn contains(&self, p: [f32; 2]) -> bool {
        let [ox, oy] = self.origin;
        let [x, y] = p;
        x >= ox && y >= oy && x <= ox + self.side && y <= oy + self.side
    }
}

/// `RegionTileFrame`：341 高度 tile 金字塔的局部坐标。
///
/// 每张 tile 为 `REGION_TILE_PX`² 像素、`REGION_METERS_PER_PIXEL` 米/像素。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionTileFrame {
    pub origin: [f32; 2],
    pub meters_per_pixel: f32,
    pub tile_px: u32,
}

impl RegionTileFrame {
    pub fn retail() -> Self {
        Self {
            origin: [-16384.0, -16384.0],
            meters_per_pixel: REGION_METERS_PER_PIXEL,
            tile_px: REGION_TILE_PX,
        }
    }

    /// 单 tile 边长（米）。
    pub fn tile_units(&self) -> f32 {
        self.tile_px as f32 * self.meters_per_pixel
    }

    /// 米坐标 → `(tileX, tileY, 局部米坐标)`。tile 内只做局部索引，
    /// **不得**再次叠加 `mPosInRegion`（见 `map-editor-pdb-breakthrough-20261030.md`）。
    pub fn locate(&self, p: [f32; 2]) -> ([i32; 2], [f32; 2]) {
        let tile = self.tile_units();
        let dx = p[0] - self.origin[0];
        let dy = p[1] - self.origin[1];
        let tx = (dx / tile).floor() as i32;
        let ty = (dy / tile).floor() as i32;
        ([tx, ty], [dx - tx as f32 * tile, dy - ty as f32 * tile])
    }

    /// tile 资源名（零售命名 `xNN_yNN_mipN`）。
    pub fn tile_name(&self, tile: [i32; 2], mip: u32) -> String {
        format!("x{:02}_y{:02}_mip{mip}", tile[0], tile[1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_to_eco_matches_decompiled_formula() {
        let grid = EcoGrid::new(128, 1024.0);
        assert_eq!(grid.cell(), 16.0);
        // 世界原点落在 64 号格的**起点**（格内 s=t=0，不是格心）。
        let mid = grid.sample([0.0, 0.0]);
        assert_eq!((mid.x, mid.y), (64, 64));
        assert!((mid.s).abs() < 1e-6 && (mid.t).abs() < 1e-6);
        assert!(mid.in_grid());
        // 左上角。
        let corner = grid.sample([-1024.0, -1024.0]);
        assert_eq!((corner.x, corner.y), (0, 0));
        assert!((corner.s).abs() < 1e-6 && (corner.t).abs() < 1e-6);
        // 右边界：1024 恰好等于 span 末端 → 索引被夹到 127。
        let edge = grid.sample([1023.9, 1023.9]);
        assert_eq!((edge.x, edge.y), (127, 127));
        assert!(edge.in_grid());
    }

    #[test]
    fn outside_city_box_is_flagged_as_clamped_or_underflow() {
        let grid = EcoGrid::new(128, 1024.0);
        let outside_hi = grid.sample([1500.0, -300.0]);
        assert!(outside_hi.clamped, "盒外高侧必须标记夹取");
        assert!(!grid.covers([1500.0, -300.0]));
        let outside_lo = grid.sample([-1500.0, 0.0]);
        assert!(outside_lo.underflow, "盒外低侧引擎不夹取但 s<0");
        assert!(!outside_lo.in_grid());
        // 坐标反算是精确的（s/t 保留溢出），但索引饱和：1500 与 5000
        // 落在同一个 eco 格——采样会读到同一纹素，这才是“盒外”的损失。
        let far = grid.sample([5000.0, -300.0]);
        assert_eq!((outside_hi.x, outside_hi.y), (far.x, far.y));
        let back = grid.world(&outside_hi);
        assert!((back[0] - 1500.0).abs() < 1e-2, "{back:?}");
    }

    #[test]
    fn eco_roundtrip_inside_grid_is_sub_cell() {
        let grid = EcoGrid::new(128, 1024.0);
        for p in [
            [0.0, 0.0],
            [-1000.0, 900.0],
            [512.5, -512.5],
            [1023.0, -1023.0],
        ] {
            let s = grid.sample(p);
            assert!(s.in_grid());
            let back = grid.world(&s);
            let cell = grid.cell();
            assert!(
                (back[0] - p[0]).abs() < cell && (back[1] - p[1]).abs() < cell,
                "{p:?} → {back:?} 超出单格 {cell}"
            );
        }
    }

    #[test]
    fn tile_frame_uses_verified_2048m_tiles() {
        let frame = RegionTileFrame::retail();
        assert_eq!(frame.tile_units(), 2048.0);
        assert_eq!(frame.locate([-16384.0, -16384.0]).0, [0, 0]);
        assert_eq!(frame.locate([0.0, 0.0]).0, [8, 8]);
        assert_eq!(frame.locate([16383.0, -16384.0]).0, [15, 0]);
        let (tile, local) = frame.locate([-16384.0 + 2048.0 + 8.0, -16384.0]);
        assert_eq!(tile, [1, 0]);
        assert!((local[0] - 8.0).abs() < 1e-4);
        assert_eq!(frame.tile_name([3, 12], 4), "x03_y12_mip4");
    }

    #[test]
    fn region_frame_matches_preview_extent() {
        let region = RegionFrame::retail();
        assert_eq!(region.half(), 16384.0);
        assert_eq!(region.center(), [0.0, 0.0]);
        assert!(region.contains([-16384.0, -16384.0]));
        assert!(!region.contains([16385.0, 0.0]));
    }
}
