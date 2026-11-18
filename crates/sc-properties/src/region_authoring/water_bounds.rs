use crate::{
    model::Value,
    region_map::SHARED_TILE_GRID,
    region_write::{PYRAMID_LEVELS, TILE_BYTES, TILES_PER_REGION},
};

pub(super) const INSTANCE: u32 = 0x6e741c4c;
pub(super) const HEIGHT_MIN_MAX: u32 = 0xc7394cf1;

/// Mirrors cTerrainExporter::ComputeMinMaxValues over the encoded F0
/// tiles. cTerrainSVT::MipXYToIndex orders the table by finest mip first,
/// then image row and column within each mip.
pub(super) fn from_f0<T: AsRef<[u8]>>(
    mut read_tile: impl FnMut(u32) -> Result<T, String>,
) -> Result<Vec<Value>, String> {
    let mut bounds = Vec::with_capacity(TILES_PER_REGION);
    for mip in 0..PYRAMID_LEVELS {
        let side = SHARED_TILE_GRID.len() >> mip;
        for row in &SHARED_TILE_GRID[..side] {
            for &tile_id in &row[..side] {
                let instance = tile_id ^ mip as u32;
                let data = read_tile(instance)?;
                let tile = data.as_ref();
                if tile.len() != TILE_BYTES {
                    return Err(format!("Invalid F0 tile length for {instance:08X}"));
                }
                let (min, max) =
                    tile[20..]
                        .chunks_exact(2)
                        .fold((u16::MAX, u16::MIN), |(min, max), raw| {
                            let value = u16::from_le_bytes([raw[0], raw[1]]);
                            (min.min(value), max.max(value))
                        });
                bounds.push(Value::Vector2([
                    f32::from(min) / f32::from(u16::MAX),
                    f32::from(max) / f32::from(u16::MAX),
                ]));
            }
        }
    }
    Ok(bounds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finest_water_tile_is_classified_at_its_native_lut_index() {
        let submerged = SHARED_TILE_GRID[7][7];
        let table = from_f0(|instance| {
            let mut tile = vec![0; TILE_BYTES];
            let height = if instance == submerged { 2400u16 } else { 8000 };
            for raw in tile[20..].chunks_exact_mut(2) {
                raw.copy_from_slice(&height.to_le_bytes());
            }
            Ok(tile)
        })
        .unwrap();
        assert_eq!(table.len(), TILES_PER_REGION);
        let normalized = 2400.0 / 65535.0;
        assert_eq!(table[7 + 16 * 7], Value::Vector2([normalized; 2]));
        assert_eq!(table.last(), Some(&Value::Vector2([8000.0 / 65535.0; 2])));
        assert!(normalized * 2048.0 - 1024.0 < super::super::SEA as f32);
    }
}
