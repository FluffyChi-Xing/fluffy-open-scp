use super::*;
use crate::region_state::{RegionState, read_bootstrap_layout, read_state};
use std::io::{Read, Write};

const MAX_BOOTSTRAP_BYTES: u64 = 128 * 1024 * 1024;
const ROAD_SEGMENT_METERS: i32 = 256;
const SEGMENTS: u32 = (SIDE_METERS / ROAD_SEGMENT_METERS) as u32;

pub(super) fn decompress(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    flate2::read::GzDecoder::new(raw)
        .take(MAX_BOOTSTRAP_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_BOOTSTRAP_BYTES {
        return Err("Bootstrap exceeds decompression limit".into());
    }
    Ok(data)
}

fn word(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("Truncated Cape Trinity bootstrap")?;
    Ok(u32::from_be_bytes(value.try_into().unwrap()))
}

fn append(out: &mut Vec<u8>, words: impl IntoIterator<Item = u32>) {
    for word in words {
        out.extend_from_slice(&word.to_be_bytes());
    }
}

fn paths(out: &mut Vec<u8>, highway: &StateCurve) {
    let points = SEGMENTS + 1;
    let invalid = u32::MAX;
    let [x, first_y, z] = highway.controls[0];
    // Retail EcoPathSet v6, cEcoPathSet::Read and AddPathInternal.
    append(out, [6, points, 0]);
    for point in 0..points {
        let y = first_y + (point as i32 * ROAD_SEGMENT_METERS) as f32;
        append(out, [x, y, z].map(f32::to_bits));
    }
    append(out, [SEGMENTS * 2, 0]);
    for _ in 0..SEGMENTS * 2 {
        append(out, [0., ROAD_SEGMENT_METERS as f32, 0.].map(f32::to_bits));
    }
    append(out, [SEGMENTS, 0]);
    for segment in 0..SEGMENTS {
        let previous = if segment == 0 { 0 } else { 2 * segment - 1 };
        let next = if segment + 1 == SEGMENTS {
            2 * segment + 1
        } else {
            2 * (segment + 1)
        };
        append(
            out,
            [
                segment,
                segment + 1,
                2 * segment,
                2 * segment + 1,
                previous,
                next,
                0,
            ],
        );
    }
    append(out, [points]);
    append(out, (0..SEGMENTS).map(|segment| 2 * segment));
    append(out, [2 * SEGMENTS - 1]);
    for _ in 0..points {
        append(out, [0, invalid, invalid]); // point entry, group and next group point
    }
    append(
        out,
        [
            1,
            0,
            highway.entry_id,
            0,
            SEGMENTS - 1,
            SEGMENTS,
            invalid,
            invalid,
            0,
            0,
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_highway_point_has_a_closed_native_segment_ring() {
        let region = generate_coastal_region(CoastalRegionSpec::default());
        let mut bytes = Vec::new();
        paths(&mut bytes, &region.highway);
        let point_count = SEGMENTS + 1;
        let segments_at = (3 + 3 * point_count + 2 + 6 * SEGMENTS + 2) as usize * 4;
        let heads_at = segments_at + (7 * SEGMENTS + 1) as usize * 4;
        for point in 0..point_count {
            let head = word(&bytes, heads_at + point as usize * 4).unwrap();
            let mut cursor = head;
            let expected = if point == 0 || point == SEGMENTS {
                1
            } else {
                2
            };
            for visit in 0..expected {
                assert!(cursor < 2 * SEGMENTS);
                let segment = (cursor / 2) as usize;
                let direction = (cursor % 2) as usize;
                let offset = segments_at + segment * 28;
                assert_eq!(word(&bytes, offset + direction * 4).unwrap(), point);
                cursor = word(&bytes, offset + (4 + direction) * 4).unwrap();
                assert_eq!(cursor == head, visit + 1 == expected);
            }
        }
    }
}

fn zones(out: &mut Vec<u8>, stock: &[u8], side: usize) -> Result<(), String> {
    // The first Cape Trinity zone is a full-width row span for each map row.
    // Preserve the remaining zone/bin/history records from the template envelope.
    let prefix = [4, 4, 3, 0, 0x79d35ec4, 128];
    for (index, expected) in prefix.into_iter().enumerate() {
        if word(stock, index * 4)? != expected {
            return Err("Unsupported Cape Trinity whole-city zone".into());
        }
    }
    let first_span = prefix.len() * 4;
    for row in 0..128 {
        let at = first_span + row * 12;
        if [word(stock, at)?, word(stock, at + 4)?, word(stock, at + 8)?] != [row as u32, 0, 128] {
            return Err("Cape Trinity city zone does not cover its original grid".into());
        }
    }
    append(out, prefix[..prefix.len() - 1].iter().copied());
    append(out, [side as u32]);
    for row in 0..side {
        append(out, [row as u32, 0, side as u32]);
    }
    out.extend_from_slice(&stock[first_span + 128 * 12..]);
    Ok(())
}

pub(super) fn encode(
    source: &[u8],
    region: &CoastalRegion,
    state: &RegionState,
) -> Result<Vec<u8>, String> {
    let (donor, records) = read_bootstrap_layout(source)?;
    if donor.version != 17
        || donor.maps.iter().any(|map| map.size != 128)
        || !donor
            .curves
            .iter()
            .any(|curve| curve.entry_id == region.highway.entry_id)
    {
        return Err("Unexpected Cape Trinity bootstrap format or highway entry".into());
    }
    let environment_at = 16 + 8 * word(source, 12)? as usize;
    let mut payload = source[..records.start].to_vec();
    payload[environment_at..environment_at + 4].copy_from_slice(&state.environment.to_be_bytes());
    // Write fresh tick and UID counter. The intervening skill definitions stay native.
    payload[records.start - 12..records.start - 4].copy_from_slice(&[0; 8]);
    append(&mut payload, [state.maps.len() as u32, 4]);
    for map in &state.maps {
        append(&mut payload, [map.id, map.values.len() as u32]);
        append(&mut payload, map.values.iter().map(|&value| value as u32));
        let total = map
            .values
            .iter()
            .map(|&value| i64::from(value))
            .sum::<i64>();
        payload.extend_from_slice(&total.to_be_bytes());
    }
    append(&mut payload, [0, 0, 13]); // empty unit table
    paths(&mut payload, &region.highway);
    let side = state.maps.first().ok_or("Empty EcoMap table")?.size;
    zones(&mut payload, &source[records.end..], side)?;

    let check = read_state(&payload)?;
    if check.version != 17
        || check.environment != state.environment
        || !check.units.is_empty()
        || check.maps.len() != state.maps.len()
        || check
            .maps
            .iter()
            .zip(&state.maps)
            .any(|(a, b)| a.id != b.id || a.size != b.size || a.values != b.values)
        || !has_highway(&check, &region.highway)
    {
        return Err("Generated bootstrap failed readback".into());
    }
    let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gzip.write_all(&payload).map_err(|e| e.to_string())?;
    gzip.finish().map_err(|e| e.to_string())
}

pub(super) fn has_highway(state: &RegionState, highway: &StateCurve) -> bool {
    state.curves.len() == SEGMENTS as usize
        && state.curves.iter().enumerate().all(|(i, segment)| {
            let [x, first_y, z] = highway.controls[0];
            let start = first_y + (i as i32 * ROAD_SEGMENT_METERS) as f32;
            let end = start + ROAD_SEGMENT_METERS as f32;
            segment.entry_id == highway.entry_id
                && segment.path == 0
                && segment.controls[0] == [x, start, z]
                && segment.controls[3] == [x, end, z]
                && segment
                    .controls
                    .iter()
                    .all(|p| p[0] == x && p[2] == z && (start..=end).contains(&p[1]))
        })
}
