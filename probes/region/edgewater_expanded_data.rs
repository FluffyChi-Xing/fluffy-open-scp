//! Stage the expanded city data. The retail game needs matching native initialization.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sc_properties::region_state::read_state;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{Read, Write},
};

const CITY_BOOTSTRAP: ResourceId = ResourceId {
    type_id: 0x08068aed,
    group: 0xccdff91d,
    instance: 0x021bda2f,
};
const CITY_HEIGHT: ResourceId = ResourceId {
    type_id: 0x03e421f0,
    group: 0x81ced36f,
    instance: 0x8238c400,
};

fn checked(path: &str, expected: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let actual = format!("{:x}", Sha256::digest(&bytes));
    if actual != expected {
        return Err(format!("unexpected input SHA-256 for {path}: {actual}").into());
    }
    Ok(bytes)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [r3, grid, height, output] = args.as_slice() else {
        return Err("expected r3.package checked-grid.bin checked-height.f0 out.package".into());
    };
    checked(
        r3,
        "99271ee15929c01237877d9c50189fc69281cc41cabdbe1456914710e5d38055",
    )?;
    let grid = checked(
        grid,
        "cea24dd5e5312349bdacf11a4ebd66543b82e1f0ff28cb9183f24e78b2665487",
    )?;
    let height = checked(
        height,
        "1c8866976862760250646710205049defd52033bf5bc59afdcb7ead6b90a7f0c",
    )?;
    let state = read_state(&grid)?;
    assert_eq!((state.version, state.environment), (17, 0x81ced36f));
    assert_eq!(state.maps.len(), 34);
    assert!(state.maps.iter().all(|map| map.size == 256));
    assert_eq!((state.curves.len(), state.units.len()), (215, 53));
    assert_eq!(height.len(), 20 + 512 * 512 * 2);
    let stock = Package::open(r3)?;
    assert_eq!(stock.entries().len(), 6);
    assert!(stock.entry(CITY_BOOTSTRAP).is_none() && stock.entry(CITY_HEIGHT).is_none());
    let mut entries = stock
        .entries()
        .iter()
        .map(|entry| {
            stock
                .read(entry)
                .map(|data| OverlayEntry::new(entry.id, data))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut compressed = GzEncoder::new(Vec::new(), Compression::default());
    compressed.write_all(&grid)?;
    entries.push(OverlayEntry::new(CITY_BOOTSTRAP, compressed.finish()?));
    entries.push(OverlayEntry::new(CITY_HEIGHT, height));
    let package = write_uncompressed_overlay(&entries)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&package)?;
    let written = Package::open(output)?;
    assert_eq!(written.entries().len(), entries.len());
    for entry in entries {
        let reread = written.read(written.entry(entry.id).unwrap())?;
        assert_eq!(reread, entry.data);
        if entry.id == CITY_BOOTSTRAP {
            let mut decoded = Vec::new();
            GzDecoder::new(&reread[..])
                .take(grid.len() as u64 + 1)
                .read_to_end(&mut decoded)?;
            assert_eq!(decoded, grid);
        }
    }
    println!(
        "8 resources verified; SHA-256 {:x}; requires 1031-specific native sizing",
        Sha256::digest(package)
    );
    Ok(())
}
