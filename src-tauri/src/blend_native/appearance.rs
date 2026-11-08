//! Deterministic per-window occupancy (port of `appearance.face_seed`/`choices`).

use sha2::{Digest, Sha256};

pub const COLORS: [[f32; 3]; 4] = [
    [1.0, 0.66, 0.32],
    [1.0, 0.86, 0.64],
    [0.9, 0.95, 1.0],
    [0.58, 0.76, 1.0],
];

const HASH_MODULUS: i64 = 2039;

pub fn face_seed(object_name: &str, face_index: u32) -> i32 {
    let digest = Sha256::digest(object_name.as_bytes());
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&digest[..4]);
    let base = i64::from(u32::from_le_bytes(bytes)) % HASH_MODULUS;
    ((base + i64::from(face_index) * 131) % HASH_MODULUS) as i32
}

/// Returns `(lit, hue_index)` for one window cell.
pub fn choices(face: i32, row: i32, column: i32, seed: i32, ratio: f64) -> (bool, usize) {
    let face = i64::from(face);
    let row = i64::from(row);
    let column = i64::from(column);
    let seed = i64::from(seed);
    let t = (column * 37 + row * 101 + face * 17 + seed * 53).rem_euclid(HASH_MODULUS);
    let h = (t * t + 19 * t + 23).rem_euclid(HASH_MODULUS);
    let lit = (h as f64) / (HASH_MODULUS as f64) < ratio;
    let hue =
        (((h * 109 + 1021).rem_euclid(HASH_MODULUS)) as f64 * 4.0 / HASH_MODULUS as f64) as usize;
    (lit, hue.min(3))
}

pub fn room_choices(
    face: i32,
    row: i32,
    group: i32,
    column: i32,
    columns: i32,
    mode: &str,
    seed: i32,
    ratio: f64,
) -> (bool, usize) {
    let column = if mode == "GROUP" {
        group
    } else {
        group * columns + column
    };
    choices(face, row, column, seed, ratio)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_seed_matches_python() {
        assert_eq!(face_seed("051", 0), 1712);
        assert_eq!(face_seed("051", 5), 328);
        assert_eq!(face_seed("Box001", 0), 1450);
    }

    #[test]
    fn choices_match_python() {
        assert_eq!(choices(123, 2, 3, 1, 0.45), (false, 2));
    }
}
