//! Prepare a 4096 m height candidate and measure its seam against the retail crop.
use dbpf::{Package, ResourceId};
use sc_properties::region_3d::assembled_height;
use std::{error::Error, fs::OpenOptions, io::Write};

const CITY: ResourceId = ResourceId {
    type_id: 0x03e421f0,
    group: 0x81ced36f,
    instance: 0x8238c400,
};
const REGION: u32 = 0xa0b60dde;
const OLD: usize = 256;
const NEW: usize = 512;
const INSET: usize = (NEW - OLD) / 2;
const HEADER: usize = 20;
const WORLD_ORIGIN: (i32, i32) = (-2496, 1472);

fn region_start(coordinate: i32) -> usize {
    let shifted = coordinate - 2048 + 16384;
    assert_eq!(shifted % 8, 0);
    usize::try_from(shifted / 8).unwrap()
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [source, output] = args.as_slice() else {
        return Err("expected SimCity_RegionTerrain0.package out.f0".into());
    };
    let source = Package::open(source)?;
    let stock = source.read(source.entry(CITY).ok_or("stock city height missing")?)?;
    assert_eq!(stock.len(), HEADER + OLD * OLD * 2);
    assert_eq!(
        &stock[..HEADER],
        &[0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 7, 0, 2, 0, 0]
    );
    let region = assembled_height(&source, REGION).ok_or("region height tiles missing")?;
    let (px, py) = (region_start(WORLD_ORIGIN.0), region_start(WORLD_ORIGIN.1));
    assert!(px + NEW <= 4096 && py + NEW <= 4096);
    let mut grid = Vec::with_capacity(NEW * NEW);
    for row in py..py + NEW {
        let start = row * 4096 + px;
        grid.extend_from_slice(&region[start..start + NEW]);
    }
    let mut maximum_center_delta = 0;
    let mut total_center_delta = 0_u64;
    for (y, row) in stock[HEADER..].chunks_exact(OLD * 2).enumerate() {
        for (x, raw) in row.chunks_exact(2).enumerate() {
            let sample = u16::from_le_bytes(raw.try_into()?);
            let index = (y + INSET) * NEW + x + INSET;
            let delta = sample.abs_diff(grid[index]);
            maximum_center_delta = maximum_center_delta.max(delta);
            total_center_delta += u64::from(delta);
            grid[index] = sample;
        }
    }
    let average_delta_m = total_center_delta as f64 / (OLD * OLD) as f64 / 32.0;
    let max_delta_m = f64::from(maximum_center_delta) / 32.0;
    let mut max_seam = 0;
    for p in INSET..INSET + OLD {
        let low = INSET;
        let high = INSET + OLD - 1;
        for (inside, outside) in [
            (p * NEW + low, p * NEW + low - 1),
            (p * NEW + high, p * NEW + high + 1),
            (low * NEW + p, (low - 1) * NEW + p),
            (high * NEW + p, (high + 1) * NEW + p),
        ] {
            max_seam = max_seam.max(grid[inside].abs_diff(grid[outside]));
        }
    }
    let mut bytes = stock[..HEADER].to_vec();
    bytes[4..8].copy_from_slice(&(NEW as u32).to_be_bytes());
    bytes[8..12].copy_from_slice(&(NEW as u32).to_be_bytes());
    bytes[16..20].copy_from_slice(&((NEW * NEW * 2) as u32).to_be_bytes());
    for sample in grid {
        bytes.extend(sample.to_le_bytes());
    }
    for (y, row) in stock[HEADER..].chunks_exact(OLD * 2).enumerate() {
        let start = HEADER + ((y + INSET) * NEW + INSET) * 2;
        assert_eq!(&bytes[start..start + OLD * 2], row);
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    println!(
        "source window ({px},{py}); stock vs region center mean {average_delta_m:.3} m max {max_delta_m:.3} m; candidate maximum seam {:.3} m",
        f64::from(max_seam) / 32.0
    );
    Ok(())
}
