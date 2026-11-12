use crate::region_write::REGION_FIELD_PX;
use serde::Deserialize;

const SAMPLE_METERS: f64 = 8.0;
const HALF_REGION: f64 = REGION_FIELD_PX as f64 * SAMPLE_METERS / 2.0;
const RAW_PER_METER: f64 = 32.0;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BrushMode {
    Raise,
    Level,
    Lower,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerrainStroke {
    pub mode: BrushMode,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub strength: f64,
}

/// Offline circular stamp inspired by TerrainBrushAdd/Subtract/Level.
/// The smooth radial falloff is a workbench kernel, not a recovered Maxis shader.
pub fn apply_strokes(original: &[u16], strokes: &[TerrainStroke]) -> Result<Vec<u16>, String> {
    if original.len() != REGION_FIELD_PX * REGION_FIELD_PX || strokes.len() > 256 {
        return Err("invalid terrain edit size".into());
    }
    let mut edited = original.to_vec();
    let index = |v: f64| ((v + HALF_REGION) / SAMPLE_METERS).floor()
        .clamp(0.0, (REGION_FIELD_PX - 1) as f64) as usize;
    for stroke in strokes {
        if !(-HALF_REGION..HALF_REGION).contains(&stroke.x)
            || !(-HALF_REGION..HALF_REGION).contains(&stroke.y)
            || !(32.0..=2048.0).contains(&stroke.radius)
            || !(1.0..=512.0).contains(&stroke.strength)
        {
            return Err("invalid terrain stroke".into());
        }
        let target = f64::from(edited[index(stroke.y) * REGION_FIELD_PX + index(stroke.x)]);
        for y in index(stroke.y - stroke.radius)..=index(stroke.y + stroke.radius) {
            for x in index(stroke.x - stroke.radius)..=index(stroke.x + stroke.radius) {
                let dx = x as f64 * SAMPLE_METERS - HALF_REGION - stroke.x;
                let dy = y as f64 * SAMPLE_METERS - HALF_REGION - stroke.y;
                let weight = (1.0 - dx.hypot(dy) / stroke.radius).max(0.0);
                let weight = weight * weight * (3.0 - 2.0 * weight);
                let cell = &mut edited[y * REGION_FIELD_PX + x];
                let value = f64::from(*cell);
                let step = stroke.strength * RAW_PER_METER;
                let delta = match stroke.mode {
                    BrushMode::Raise => step,
                    BrushMode::Lower => -step,
                    BrushMode::Level => (target - value).clamp(-step, step),
                };
                *cell = (value + delta * weight).round().clamp(0.0, f64::from(u16::MAX)) as u16;
            }
        }
    }
    Ok(edited)
}
