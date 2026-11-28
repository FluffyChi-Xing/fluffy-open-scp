//! Extend the 4096 m EdgeWaterBay surface experiment with matching shader UVs and ED layers.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use sc_properties::{region_3d::assembled_ed, region_state::read_state};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{Read, Write},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const BOOTSTRAP: ResourceId = ResourceId {
    type_id: 0x08068aed,
    group: 0xccdff91d,
    instance: 0x021bda2f,
};
const WIDTH: usize = 4096;
const ECO_CELL: usize = 16;
const SIDE: usize = WIDTH / ECO_CELL;
const SOURCE_SIDE: usize = 2048;
const SOURCE_HALF: i32 = (SOURCE_SIDE * ECO_CELL / 2) as i32;
const CITY_ORIGIN: [i32; 2] = [-2496, 1472];
const INSET: usize = (SIDE - 128) / 2;
const LAYERS: [u32; 3] = [0x0f92dc56, 0x1d52be3e, 0x28f896a4];

const SHADER_RULES: [(&[u8], &[u8]); 7] = [
    (b"worldPos.xy / 2048.0", b"worldPos.xy / 4096.0"),
    (b"worldPosition.xy / 2048.0", b"worldPosition.xy / 4096.0"),
    (
        b"Current.position.xyxy / 1024.0 * 0.5",
        b"Current.position.xyxy / 2048.0 * 0.5",
    ),
    (
        b"(Current.position.xy + 1024.0f) / 2048.0f",
        b"(Current.position.xy + 2048.0f) / 4096.0f",
    ),
    (
        b"clip (float2(1024,1024) - abs(worldPos.xy))",
        b"clip (float2(2048,2048) - abs(worldPos.xy))",
    ),
    (
        b"In.texcoord7.xy / 1024.0 * 0.5",
        b"In.texcoord7.xy / 2048.0 * 0.5",
    ),
    (
        b"In.texcoord4.xy / 1024.0 * 0.5",
        b"In.texcoord4.xy / 2048.0 * 0.5",
    ),
];

fn open_reference(path: &str, expected: &str) -> Result<Package> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut fs::File::open(path)?, &mut hasher)?;
    if format!("{:x}", hasher.finalize()) != expected {
        return Err(format!("unexpected SHA-256 for {path}").into());
    }
    Ok(Package::open(path)?)
}

fn read(package: &Package, id: ResourceId) -> Result<Vec<u8>> {
    Ok(package.read(package.entry(id).ok_or_else(|| format!("missing {id}"))?)?)
}

fn word(payload: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(payload[offset..offset + 4].try_into().unwrap())
}

fn outer(index: usize) -> bool {
    let stock = INSET..SIDE - INSET;
    !stock.contains(&(index % SIDE)) || !stock.contains(&(index / SIDE))
}

fn region_sample(index: usize) -> usize {
    let start = CITY_ORIGIN.map(|axis| {
        let offset = axis + SOURCE_HALF - WIDTH as i32 / 2;
        assert_eq!(offset % ECO_CELL as i32, 0);
        usize::try_from(offset / ECO_CELL as i32).unwrap()
    });
    (start[1] + index / SIDE) * SOURCE_SIDE + start[0] + index % SIDE
}

fn expand_layer(channel: &[u8], index: usize) -> i32 {
    // The archived ED channels are 8-bit. Use the existing preview conversion,
    // capped at the stock city maps' observed maximum.
    (i32::from(channel[region_sample(index)]) * 257).min(65534)
}

fn fill_eco(payload: &[u8], channels: [&[u8]; 3]) -> Result<Vec<u8>> {
    let before = read_state(payload)?;
    assert_eq!((before.version, before.environment), (17, 0x81ced36f));
    assert_eq!(before.maps.len(), 34);
    let env = 16 + 8 * word(payload, 12) as usize;
    let header = env + 20 + 8 * word(payload, env + 4) as usize;
    assert_eq!((word(payload, header), word(payload, header + 4)), (34, 3));
    let mut result = Vec::with_capacity(payload.len() + 34 * 4);
    result.extend_from_slice(&payload[..header + 4]);
    // The retail reader accepts map versions 3 and 4 independently of the
    // outer snapshot version. v4 stores signed 64-bit sums for every map.
    result.extend_from_slice(&4_u32.to_be_bytes());
    let mut record = header + 8;
    let mut replaced = 0;
    for _ in 0..34 {
        let id = word(payload, record);
        assert_eq!(word(payload, record + 4) as usize, SIDE * SIDE);
        let start = record + 8;
        let end = start + SIDE * SIDE * 4;
        result.extend_from_slice(&payload[record..start]);
        if let Some(layer) = LAYERS.iter().position(|map| *map == id) {
            let mut total = 0_i64;
            for index in 0..SIDE * SIDE {
                let offset = start + index * 4;
                let stock = word(payload, offset) as i32;
                let value = if outer(index) {
                    assert_eq!(stock, 0);
                    expand_layer(channels[layer], index)
                } else {
                    stock
                };
                total += i64::from(value);
                result.extend_from_slice(&value.to_be_bytes());
            }
            result.extend_from_slice(&total.to_be_bytes());
            replaced += 1;
        } else {
            result.extend_from_slice(&payload[start..end]);
            result.extend_from_slice(&i64::from(word(payload, end) as i32).to_be_bytes());
        }
        record = end + 4;
    }
    assert_eq!(replaced, LAYERS.len());
    assert_eq!(result.len(), record + 34 * 4);
    result.extend_from_slice(&payload[record..]);
    let after = read_state(&result)?;
    for (previous, modified) in before.maps.iter().zip(&after.maps) {
        assert_eq!((previous.id, previous.size), (modified.id, modified.size));
        let channel = LAYERS.iter().position(|key| *key == modified.id);
        for (index, value) in modified.values.iter().enumerate() {
            let expected = match channel {
                Some(layer) if outer(index) => expand_layer(channels[layer], index),
                _ => previous.values[index],
            };
            assert_eq!(*value, expected);
        }
        if channel.is_some() {
            let count = modified
                .values
                .iter()
                .enumerate()
                .filter(|(i, v)| outer(*i) && **v != 0)
                .count();
            assert!(count > 0);
            println!("map {:08X}: {count} nonzero outer cells", modified.id);
        }
    }
    Ok(result)
}

fn patch_shader(mut data: Vec<u8>, group: u32, variant: u32) -> Vec<u8> {
    let expected = if (group, variant) == (0x40212012, 0) {
        [4, 1, 1, 1, 1, 0, 6]
    } else {
        [4, 1, 1, 1, 1, 4, 2]
    };
    for ((original, replacement), count) in SHADER_RULES.into_iter().zip(expected) {
        // Preserve binary string lengths and serialized offsets. Only replace
        // horizontal transforms proven to address the city surface textures.
        assert_eq!(original.len(), replacement.len());
        let positions = data
            .windows(original.len())
            .enumerate()
            .filter_map(|(offset, s)| (s == original).then_some(offset))
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), count, "shader {group:08x}/{variant}");
        for offset in positions {
            data[offset..offset + original.len()].copy_from_slice(replacement);
        }
    }
    data
}

fn main() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [candidate, region, app, output] = args.as_slice() else {
        return Err("expected SurfaceCandidate.package SimCity_RegionTerrain0.package SimCity_App.package out.package".into());
    };
    let base = open_reference(
        candidate,
        "044a90c239ca7490aeb48bf793c0f1950b6e3b2c757013298b9dba810bd767c7",
    )?;
    let region = open_reference(
        region,
        "2ff8bcac2272b83ef05fba23d3d775402c2de8b7e6d5a4383f6a22f261c6d4a6",
    )?;
    let app = open_reference(
        app,
        "a290bc0d81faf75c8a04e852ce39c859ad8651b64b10b03d6d8186393c2b4066",
    )?;
    let (water, forest, soil) =
        assembled_ed(&region, 0xa0b60dde).ok_or("missing EdgeWaterBay ED")?;
    let compressed = read(&base, BOOTSTRAP)?;
    let mut payload = Vec::new();
    GzDecoder::new(&compressed[..]).read_to_end(&mut payload)?;
    let restored = fill_eco(&payload, [&water, &forest, &soil])?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&restored)?;
    let bootstrap = encoder.finish()?;
    let mut entries = Vec::new();
    for entry in base.entries() {
        let data = if entry.id == BOOTSTRAP {
            bootstrap.clone()
        } else {
            base.read(entry)?
        };
        entries.push(OverlayEntry::new(entry.id, data));
    }
    assert_eq!(entries.len(), 8);
    for group in [0x40212002, 0x40212012] {
        for variant in 0..4 {
            let id = ResourceId {
                type_id: 0x0469a3f7,
                group,
                instance: variant,
            };
            let original = read(&app, id)?;
            let modified = patch_shader(original.clone(), group, variant);
            assert_ne!(modified, original);
            entries.push(OverlayEntry::new(id, modified));
        }
    }
    let package = write_uncompressed_overlay(&entries)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&package)?;
    let reread = Package::open(output)?;
    assert_eq!(reread.entries().len(), entries.len());
    for entry in entries {
        assert_eq!(read(&reread, entry.id)?, entry.data);
    }
    println!(
        "16 resources verified; SHA-256 {:x}",
        Sha256::digest(package)
    );
    Ok(())
}
