//! Deterministic coastal-region authoring, independent of the desktop and saves.
mod bootstrap;
mod preview;
mod retail;

pub use retail::{
    RegionBuildReport, RegionLocale, RetailSources, build_coastal_region, verify_coastal_region,
};

use crate::region_state::StateCurve;
use serde::{Deserialize, Serialize};

pub const SIDE_METERS: i32 = 8192;
pub const HEIGHT_SIDE: usize = SIDE_METERS as usize / 8;
pub const ECO_SIDE: usize = SIDE_METERS as usize / 16;
const HALF: i32 = SIDE_METERS / 2;
const SEA: i32 = -870;
const HIGHWAY_X: i32 = SIDE_METERS / 4;
const HIGHWAY_Z: i32 = SEA + 24;
// Stock regional highway extrusion, used by the Cape Trinity bootstrap.
const HIGHWAY_PATH_ENTRY: u32 = 0x017A_430A;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoastalRegionSpec {
    pub seed: u32,
}

impl Default for CoastalRegionSpec {
    fn default() -> Self {
        Self { seed: 523 }
    }
}

pub struct CoastalRegion {
    /// Row-major in world XY. The first row is the southern edge.
    pub heights: Vec<u16>,
    pub ground: Vec<i32>,
    pub water_depth: Vec<i32>,
    pub highway: StateCurve,
}

pub fn generate_coastal_region(spec: CoastalRegionSpec) -> CoastalRegion {
    let mut heights = Vec::with_capacity(HEIGHT_SIDE * HEIGHT_SIDE);
    for row in 0..HEIGHT_SIDE {
        let y = row as f64 * 8.0 + 4.0 - f64::from(HALF);
        for column in 0..HEIGHT_SIDE {
            let x = column as f64 * 8.0 + 4.0 - f64::from(HALF);
            heights.push(height_sample(spec.seed, x, y));
        }
    }

    let mut ground = Vec::with_capacity(ECO_SIDE * ECO_SIDE);
    let mut water_depth = Vec::with_capacity(ECO_SIDE * ECO_SIDE);
    for y in (0..HEIGHT_SIDE).step_by(2) {
        for x in (0..HEIGHT_SIDE).step_by(2) {
            let k = y * HEIGHT_SIDE + x;
            let sum: i32 = [k, k + 1, k + HEIGHT_SIDE, k + HEIGHT_SIDE + 1]
                .into_iter()
                .map(|i| i32::from(heights[i]))
                .sum();
            let raw = (sum + 2) / 4 - 1;
            ground.push(raw);
            // Retail Cape/EdgeWater stock thickness uses a 48 raw-unit offset.
            water_depth.push(((SEA + 1024) * 32 - 48 - raw).max(0));
        }
    }

    let p = |y| [HIGHWAY_X as f32, y, HIGHWAY_Z as f32];
    let edge = HALF as f32;
    let highway = StateCurve {
        entry_id: HIGHWAY_PATH_ENTRY,
        path: 0,
        controls: [p(-edge), p(-edge / 2.0), p(edge / 2.0), p(edge)],
    };
    CoastalRegion {
        heights,
        ground,
        water_depth,
        highway,
    }
}

fn height_sample(seed: u32, x: f64, y: f64) -> u16 {
    let land = x > 0.0;
    let inland = smooth((x.abs() / 192.0).min(1.0));
    let corridor = if land {
        smooth(((x - f64::from(HIGHWAY_X)).abs() / 160.0).min(1.0))
    } else {
        1.0
    };
    let noise = perlin(seed, x / 640.0, y / 640.0) * 7.0
        + perlin(seed.rotate_left(13), x / 220.0, y / 220.0) * 2.5;
    let relief = (24.0 + noise * corridor) * inland;
    let offset = if land {
        relief.max(2.0)
    } else {
        -relief.max(2.0)
    };
    ((offset + f64::from(SEA + 1024)) * 32.0).round() as u16
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn perlin(seed: u32, x: f64, y: f64) -> f64 {
    let gradient = |i: i32, j: i32| {
        let h = (i as u32).wrapping_mul(0x9e37_79b1) ^ (j as u32).wrapping_mul(0x85eb_ca77) ^ seed;
        let h = (h ^ (h >> 16)).wrapping_mul(0x7feb_352d);
        let h = h ^ (h >> 15);
        let dx = x - f64::from(i);
        let dy = y - f64::from(j);
        match h & 3 {
            0 => dx + dy,
            1 => dx - dy,
            2 => -dx + dy,
            _ => -dx - dy,
        }
    };
    let i = x.floor() as i32;
    let j = y.floor() as i32;
    let fade = |t: f64| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let u = fade(x - f64::from(i));
    let v = fade(y - f64::from(j));
    let lerp = |a, b, t| a + (b - a) * t;
    lerp(
        lerp(gradient(i, j), gradient(i + 1, j), u),
        lerp(gradient(i, j + 1), gradient(i + 1, j + 1), u),
        v,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coast_and_highway_follow_world_coordinates() {
        let spec = CoastalRegionSpec::default();
        let region = generate_coastal_region(spec);
        assert_eq!(region.heights, generate_coastal_region(spec).heights);
        let sea = ((SEA + 1024) * 32) as u16;
        for row in region.heights.chunks_exact(HEIGHT_SIDE) {
            assert!(row[..HEIGHT_SIDE / 2].iter().all(|&v| v < sea));
            assert!(row[HEIGHT_SIDE / 2..].iter().all(|&v| v > sea));
            for index in [HEIGHT_SIDE * 3 / 4 - 1, HEIGHT_SIDE * 3 / 4] {
                assert_eq!(row[index], ((HIGHWAY_Z + 1024) * 32) as u16);
            }
        }
        for row in region.water_depth.chunks_exact(ECO_SIDE) {
            assert!(row[..ECO_SIDE / 2].iter().all(|&v| v > 0));
            assert!(row[ECO_SIDE / 2..].iter().all(|&v| v == 0));
        }
        assert_eq!(region.highway.controls[0], [2048., -4096., -846.]);
        assert_eq!(region.highway.controls[3], [2048., 4096., -846.]);
    }
}
