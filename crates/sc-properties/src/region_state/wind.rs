//! WindDirection lives in the city's global bin set, after paths and zones.
use std::collections::HashSet;

use super::read_state_prefix;

pub const WIND_DIRECTION_ID: u32 = 0x3136055A;

fn direction_field(bytes: &[u8]) -> Result<Option<(usize, i32)>, String> {
    let (_, mut r) = read_state_prefix(bytes)?;
    let zones = r.count(1_000_000)?;
    let max_zones = r.count(1_000_000)?;
    let version = r.u()?;
    if zones > max_zones || version > 3 {
        return Err("unsupported EcoGame zone table".into());
    }
    let mut used = HashSet::new();
    for _ in 0..zones {
        let slot = r.count(max_zones)?;
        if slot >= max_zones || !used.insert(slot) {
            return Err("invalid EcoGame zone slot".into());
        }
        r.u()?; // zone entry ID
        let cell_spans = r.count(1_000_000)?;
        r.take(cell_spans * 12)?;
        let path_slots = r.count(1_000_000)?;
        r.take(path_slots * 8)?; // path slots and first span indices
        let (path_spans, _) = r.slots()?;
        r.take(path_spans * 12)?; // cEcoTSpan
        if version > 1 {
            r.u()?; // creation tick
        }
        if version == 3 {
            let bins = r.count(16383)?;
            r.take(bins * 8 + 8)?; // bin pairs and control flags
        }
    }
    let bins = r.count(16383)?;
    let mut direction = None;
    for _ in 0..bins {
        let id = r.u()?;
        let offset = r.pos;
        let amount = r.u()? as i32;
        if id == WIND_DIRECTION_ID && direction.replace((offset, amount)).is_some() {
            return Err("duplicate global WindDirection bin".into());
        }
    }
    r.take(8)?; // global control flags
    Ok(direction)
}

pub fn read_wind_direction(bytes: &[u8]) -> Result<Option<i32>, String> {
    Ok(direction_field(bytes)?.map(|(_, angle)| angle.rem_euclid(360)))
}

pub fn set_wind_direction(bytes: &mut [u8], angle: i32) -> Result<(), String> {
    if !(0..360).contains(&angle) {
        return Err("wind direction must be an angle from 0 through 359".into());
    }
    let (offset, _) = direction_field(bytes)?.ok_or("city has no global WindDirection bin")?;
    bytes[offset..offset + 4].copy_from_slice(&angle.to_be_bytes());
    Ok(())
}
