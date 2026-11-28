//! Populate the height-producing EcoMaps in the expanded EdgeWaterBay bootstrap.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sc_properties::region_state::read_state;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    io::{Read, Write},
};

const BOOTSTRAP: ResourceId = ResourceId {
    type_id: 0x08068aed,
    group: 0xccdff91d,
    instance: 0x021bda2f,
};
const HEIGHT: ResourceId = ResourceId {
    type_id: 0x03e421f0,
    group: 0x81ced36f,
    instance: 0x8238c400,
};
const GROUND_MAP: u32 = 0x6f9f6ab3;
const WATER_MAP: u32 = 0xab4a0ea0;
const SEA_LEVEL: i32 = (-870 + 1024) * 32;
// The stock water-thickness layer fills submerged cells to this encoded height.
const WATER_SURFACE: i32 = 4880;
const SIDE: usize = 256;
const CELLS: usize = SIDE * SIDE;

fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

fn inside(x: usize, y: usize) -> bool {
    (64..192).contains(&x) && (64..192).contains(&y)
}

fn reference_height(f0: &[u8], x: usize, y: usize) -> i32 {
    let pixel = |column, row| {
        let offset = 20 + (row * 512 + column) * 2;
        i32::from(u16::from_le_bytes(
            f0[offset..offset + 2].try_into().unwrap(),
        ))
    };
    // On the original city crop, rounded 2x2 mean minus one agrees within one
    // raw unit with every stock ground EcoMap cell. Preserve the stock center.
    let x = 2 * x;
    let y = 2 * y;
    (pixel(x, y) + pixel(x + 1, y) + pixel(x, y + 1) + pixel(x + 1, y + 1) + 2) / 4 - 1
}

fn fill_surface(payload: &[u8], f0: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    let before = read_state(payload)?;
    assert_eq!((before.version, before.environment), (17, 0x81ced36f));
    assert_eq!(before.maps.len(), 34);
    assert_eq!(
        (word(f0, 4), word(f0, 8), f0.len()),
        (512, 512, 20 + 512 * 512 * 2)
    );
    let mut result = payload.to_vec();
    let mut ground = Vec::with_capacity(CELLS);
    for y in 0..SIDE {
        for x in 0..SIDE {
            ground.push(reference_height(f0, x, y));
        }
    }
    let env = 16 + 8 * word(payload, 12) as usize;
    let header = env + 20 + 8 * word(payload, env + 4) as usize;
    assert_eq!((word(payload, header), word(payload, header + 4)), (34, 3));
    let mut record = header + 8;
    let mut changed = 0;
    for _ in 0..34 {
        let id = word(payload, record);
        assert_eq!(word(payload, record + 4) as usize, CELLS);
        let start = record + 8;
        let end = start + CELLS * 4;
        if matches!(id, GROUND_MAP | WATER_MAP) {
            let mut total = 0_i32;
            for (i, &height) in ground.iter().enumerate() {
                let offset = start + i * 4;
                let stock = word(payload, offset) as i32;
                let value = if inside(i % SIDE, i / SIDE) {
                    if id == GROUND_MAP {
                        assert!(stock.abs_diff(height) <= 1);
                        ground_range(stock);
                    }
                    stock
                } else {
                    assert_eq!(stock, 0);
                    ground_range(height);
                    if id == GROUND_MAP {
                        height
                    } else {
                        (WATER_SURFACE - height).max(0)
                    }
                };
                result[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
                total = total.checked_add(value).ok_or("surface sum overflow")?;
            }
            result[end..end + 4].copy_from_slice(&total.to_be_bytes());
            changed += 1;
        }
        record = end + 4;
    }
    assert_eq!(changed, 2);
    assert_eq!(result[record..], payload[record..]);
    let after = read_state(&result)?;
    for (old, new) in before.maps.iter().zip(&after.maps) {
        assert_eq!((old.id, old.size), (new.id, new.size));
        for (i, (&old_value, &new_value)) in old.values.iter().zip(&new.values).enumerate() {
            if inside(i % SIDE, i / SIDE) || !matches!(new.id, GROUND_MAP | WATER_MAP) {
                assert_eq!(old_value, new_value);
            } else if new.id == GROUND_MAP {
                assert_eq!(new_value, ground[i]);
            } else {
                assert_eq!(new_value, (WATER_SURFACE - ground[i]).max(0));
            }
        }
    }
    let water = before.maps.iter().find(|m| m.id == WATER_MAP).unwrap();
    let stock_ground = before.maps.iter().find(|m| m.id == GROUND_MAP).unwrap();
    for (index, (&height, &depth)) in stock_ground.values.iter().zip(&water.values).enumerate() {
        if inside(index % SIDE, index / SIDE) {
            assert_eq!(depth, (WATER_SURFACE - height).max(0));
        }
    }
    assert_eq!(SEA_LEVEL - WATER_SURFACE, 48);
    println!(
        "ground range {:?}; outer water cells {}",
        ground.iter().min().zip(ground.iter().max()),
        ground
            .iter()
            .enumerate()
            .filter(|(i, h)| !inside(i % SIDE, i / SIDE) && **h < WATER_SURFACE)
            .count()
    );
    Ok(result)
}

fn ground_range(value: i32) {
    assert!((1..=65535).contains(&value));
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [candidate, output] = args.as_slice() else {
        return Err("expected 4096_DataCandidate.package out.package".into());
    };
    let sha = format!("{:x}", Sha256::digest(fs::read(candidate)?));
    assert_eq!(
        sha,
        "3caf700dda10781e5eba8f8cf81326f1ab48a085ed60ed4d63cff45bcdd624f4"
    );
    let source = Package::open(candidate)?;
    let original = source.read(source.entry(BOOTSTRAP).ok_or("bootstrap missing")?)?;
    let f0 = source.read(source.entry(HEIGHT).ok_or("height reference missing")?)?;
    let mut payload = Vec::new();
    GzDecoder::new(&original[..]).read_to_end(&mut payload)?;
    let fixed = fill_surface(&payload, &f0)?;
    let mut encoded = GzEncoder::new(Vec::new(), Compression::default());
    encoded.write_all(&fixed)?;
    let compressed = encoded.finish()?;
    let entries = source
        .entries()
        .iter()
        .map(|entry| {
            let data = if entry.id == BOOTSTRAP {
                compressed.clone()
            } else {
                source.read(entry)?
            };
            Ok(OverlayEntry::new(entry.id, data))
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let package = write_uncompressed_overlay(&entries)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&package)?;
    let readback = Package::open(output)?;
    assert_eq!(readback.entries().len(), entries.len());
    for entry in entries {
        let data = readback.read(readback.entry(entry.id).unwrap())?;
        assert_eq!(data, entry.data);
        if entry.id == BOOTSTRAP {
            let mut decoded = Vec::new();
            GzDecoder::new(&data[..]).read_to_end(&mut decoded)?;
            assert_eq!(decoded, fixed);
        }
    }
    println!(
        "8 resources verified; SHA-256 {:x}",
        Sha256::digest(package)
    );
    Ok(())
}
