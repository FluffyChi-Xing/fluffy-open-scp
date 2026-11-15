//! Build an offline v17 coal-only control from a shipped city bootstrap.
//! Usage: region_coal_control <EP1.package> <group> <instance> <environment> <cell-x> <cell-y> <out.package>
use dbpf::{OverlayEntry, Package, ResourceId};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sc_properties::region_state::read_state;
use std::{
    error::Error,
    fs::OpenOptions,
    io::{Read, Write},
};

const COAL: u32 = 0xD779_C976;
const SIDE: usize = 128;
const CELLS: usize = SIDE * SIDE;
const RADIUS: usize = 4;
const SIGNAL: i32 = 32_768;

fn unpack(gzip: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
    const LIMIT: u64 = 128 * 1024 * 1024;
    let mut payload = Vec::new();
    GzDecoder::new(gzip)
        .take(LIMIT + 1)
        .read_to_end(&mut payload)?;
    if payload.len() as u64 > LIMIT {
        return Err("bootstrap exceeds decompression limit".into());
    }
    Ok(payload)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [source, group, instance, environment, x, y, output] = args.as_slice() else {
        return Err(
            "expected package, group, instance, environment, cell-x, cell-y, output".into(),
        );
    };
    let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16);
    let id = ResourceId {
        type_id: 0x0806_8AED,
        group: hex(group)?,
        instance: hex(instance)?,
    };
    let (x, y): (usize, usize) = (x.parse()?, y.parse()?);
    if !(RADIUS..SIDE - RADIUS).contains(&x) || !(RADIUS..SIDE - RADIUS).contains(&y) {
        return Err("coal patch falls outside the original grid".into());
    }
    let source = Package::open(source)?;
    let packed = source.read(source.entry(id).ok_or("bootstrap TGI missing")?)?;
    let baseline = unpack(&packed)?;
    let state = read_state(&baseline)?;
    let coal = state
        .maps
        .iter()
        .find(|m| m.id == COAL)
        .ok_or("coal map missing")?;
    if state.version != 17
        || state.environment != hex(environment)?
        || coal.size != SIDE
        || coal.values.iter().any(|&value| value != 0)
    {
        return Err("expected the selected v17 environment and an empty 128² coal baseline".into());
    }

    // cEcoMap::Read consumes count, i32 values and i32 sum. The v17
    // bootstrap precedes this record with the typed map ID.
    let mut record = Vec::with_capacity(12 + CELLS * 4);
    record.extend(COAL.to_be_bytes());
    record.extend((CELLS as u32).to_be_bytes());
    record.resize(12 + CELLS * 4, 0);
    let hits: Vec<_> = baseline
        .windows(record.len())
        .enumerate()
        .filter_map(|(offset, bytes)| (bytes == record).then_some(offset))
        .collect();
    let &[offset] = hits.as_slice() else {
        return Err("expected a unique v17 zero coal record".into());
    };
    let end = offset + record.len();
    let mut payload = baseline.clone();
    let mut sum = 0_i32;
    for row in y - RADIUS..=y + RADIUS {
        for col in x - RADIUS..=x + RADIUS {
            let cell = offset + 8 + (row * SIDE + col) * 4;
            payload[cell..cell + 4].copy_from_slice(&SIGNAL.to_be_bytes());
            sum += SIGNAL;
        }
    }
    payload[end - 4..end].copy_from_slice(&sum.to_be_bytes());
    let mut restored = payload.clone();
    restored[offset..end].copy_from_slice(&record);
    assert_eq!(restored, baseline);

    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&payload)?;
    let overlay = dbpf::write_uncompressed_overlay(&[OverlayEntry::new(id, gzip.finish()?)])?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&overlay)?;
    drop(file);
    let check = Package::open(output)?;
    assert_eq!(check.entries().len(), 1);
    let reread = unpack(&check.read(check.entry(id).ok_or("output TGI missing")?)?)?;
    assert_eq!(reread, payload);
    let parsed = read_state(&reread)?;
    let actual = &parsed
        .maps
        .iter()
        .find(|m| m.id == COAL)
        .ok_or("output coal absent")?
        .values;
    for (i, &value) in actual.iter().enumerate() {
        let inside = (i % SIDE).abs_diff(x) <= RADIUS && (i / SIDE).abs_diff(y) <= RADIUS;
        assert_eq!(value, if inside { SIGNAL } else { 0 });
    }
    println!(
        "TGI={id} env={:08X} coal center=({x},{y}) cells=81 sum={sum}",
        parsed.environment
    );
    Ok(())
}
