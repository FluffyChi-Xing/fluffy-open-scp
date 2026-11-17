//! In-place layout editing for the map records of a decompressed EcoGame snapshot.
use std::collections::BTreeMap;

use super::{map_reader, read_state};

/// Absolute native values. Eraser is represented by zero. A later edit wins
/// when two strokes touch the same cell.
#[derive(Debug, Clone, Copy)]
pub struct CellEdit {
    pub x: usize,
    pub y: usize,
    pub value: i32,
}

/// Rasterize a round brush using cell centers in the selected city's local frame.
/// Bounds are [min_x, min_y, max_x, max_y], in meters.
pub fn round_stamp(
    side: usize,
    bounds: [f64; 4],
    center: [f64; 2],
    radius: f64,
    value: i32,
) -> Result<Vec<CellEdit>, String> {
    let [min_x, min_y, max_x, max_y] = bounds;
    let [x, y] = center;
    if !(1..=2048).contains(&side)
        || !bounds
            .into_iter()
            .chain(center)
            .chain([radius])
            .all(f64::is_finite)
        || min_x >= max_x
        || min_y >= max_y
        || radius <= 0.0
        || !(min_x..max_x).contains(&x)
        || !(min_y..max_y).contains(&y)
    {
        return Err("invalid brush or center outside selected city".into());
    }
    let step_x = (max_x - min_x) / side as f64;
    let step_y = (max_y - min_y) / side as f64;
    if !step_x.is_finite() || !step_y.is_finite() || step_x <= 0.0 || step_y <= 0.0 {
        return Err("invalid city grid spacing".into());
    }
    let limits = |axis: f64, start: f64, step: f64| {
        let cell = |coord: f64| (((coord - start) / step).floor() as usize).min(side - 1);
        cell(axis - radius)..=cell(axis + radius)
    };
    let mut edits = Vec::new();
    for row in limits(y, min_y, step_y) {
        let dy = min_y + (row as f64 + 0.5) * step_y - y;
        for col in limits(x, min_x, step_x) {
            let dx = min_x + (col as f64 + 0.5) * step_x - x;
            if dx.hypot(dy) <= radius {
                edits.push(CellEdit {
                    x: col,
                    y: row,
                    value,
                });
            }
        }
    }
    Ok(edits)
}

/// Create a deposit with a peak at the pointer and a quadratic falloff
/// in squared distance. Zero-weight edge cells leave the existing map intact.
pub fn feathered_stamp(
    side: usize,
    bounds: [f64; 4],
    center: [f64; 2],
    radius: f64,
    peak: i32,
) -> Result<Vec<CellEdit>, String> {
    let step_x = (bounds[2] - bounds[0]) / side as f64;
    let step_y = (bounds[3] - bounds[1]) / side as f64;
    let mut cells = round_stamp(side, bounds, center, radius, peak)?;
    for cell in &mut cells {
        let dx = bounds[0] + (cell.x as f64 + 0.5) * step_x - center[0];
        let dy = bounds[1] + (cell.y as f64 + 0.5) * step_y - center[1];
        let weight = (1.0 - (dx * dx + dy * dy) / (radius * radius)).max(0.0);
        cell.value = (f64::from(peak) * weight * weight).round() as i32;
    }
    cells.retain(|cell| cell.value > 0);
    Ok(cells)
}

/// Amend selected maps and their checksums, preserving the source state layout.
/// Callers resolve writable layers and brush coordinates before calling this function.
pub fn patch_maps(
    source: &[u8],
    expected_environment: u32,
    edits: &BTreeMap<u32, Vec<CellEdit>>,
) -> Result<Vec<u8>, String> {
    let state = read_state(source)?;
    if state.environment != expected_environment {
        return Err(format!(
            "unexpected city environment {:08X}",
            state.environment
        ));
    }
    let (mut reader, _, _, count, version) = map_reader(source)?;
    let mut output = source.to_vec();
    let mut remaining = edits.len();
    for map in state.maps.iter().take(count) {
        let id = reader.u()?;
        let cells = reader.count(4_194_304)?;
        let values_start = reader.pos;
        reader.take(cells * 4)?;
        let sum_start = reader.pos;
        reader.take(if version == 3 { 4 } else { 8 })?;
        let Some(changes) = edits.get(&id) else {
            continue;
        };
        if state.maps.iter().filter(|other| other.id == id).count() != 1 {
            return Err(format!("ambiguous map {id:08X}"));
        }
        remaining -= 1;
        let mut sum: i64 = map.values.iter().map(|&value| i64::from(value)).sum();
        for &CellEdit { x, y, value } in changes {
            if x >= map.size || y >= map.size {
                return Err(format!("cell ({x}, {y}) outside map {id:08X}"));
            }
            let offset = values_start + (y * map.size + x) * 4;
            let field = &mut output[offset..offset + 4];
            let previous = i32::from_be_bytes(field.try_into().unwrap());
            sum += i64::from(value) - i64::from(previous);
            field.copy_from_slice(&value.to_be_bytes());
        }
        if version == 3 {
            let sum =
                i32::try_from(sum).map_err(|_| format!("v3 sum overflow for map {id:08X}"))?;
            output[sum_start..sum_start + 4].copy_from_slice(&sum.to_be_bytes());
        } else {
            output[sum_start..sum_start + 8].copy_from_slice(&sum.to_be_bytes());
        }
    }
    if remaining != 0 {
        return Err("edit references absent state maps".into());
    }
    read_state(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::region_state::{RegionState, StateMap, write_state};

    #[test]
    fn resource_paint_and_erase_preserve_other_records_and_tail() {
        let original = write_state(&RegionState {
            version: 20,
            environment: 1,
            maps: vec![
                StateMap {
                    id: 2,
                    size: 2,
                    values: vec![0, 1, 2, 3],
                },
                StateMap {
                    id: 3,
                    size: 2,
                    values: vec![4, 5, 6, 7],
                },
            ],
            curves: vec![],
            units: vec![],
        })
        .unwrap();
        let mut original = original;
        original.extend_from_slice(b"state data beyond the decoded subset");
        let bounds = [0., 0., 32., 32.];
        let edits = BTreeMap::from([
            (2, round_stamp(2, bounds, [8., 24.], 9., 100).unwrap()),
            (3, round_stamp(2, bounds, [24., 8.], 9., 0).unwrap()),
        ]);
        let edited = patch_maps(&original, 1, &edits).unwrap();
        let maps = read_state(&edited).unwrap().maps;
        assert_eq!(maps[0].values, [0, 1, 100, 3]);
        assert_eq!(maps[1].values, [4, 0, 6, 7]);
        let undo = BTreeMap::from([
            (2, round_stamp(2, bounds, [8., 24.], 9., 2).unwrap()),
            (3, round_stamp(2, bounds, [24., 8.], 9., 5).unwrap()),
        ]);
        assert_eq!(patch_maps(&edited, 1, &undo).unwrap(), original);
        assert!(round_stamp(2, bounds, [32., 8.], 9., 0).is_err());
    }
}
