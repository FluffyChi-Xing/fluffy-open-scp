//! Read a Lot/Asset property and report its final LotMask pixels.
//! Usage: cargo run -q -p fluffy-open-scp --example ground_mask_audit -- <package> <type:group:instance> [...]

use std::collections::HashMap;
use std::error::Error;

use dbpf::{Package, ResourceId};
use rw4::RasterImage;
use sc_properties::{Kind, PropertyFile, Value};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const RASTER_TYPE: u32 = 0x2F4E_681C;
const LOT_MASK: u32 = 0x0CCB_7FD5;
const GROUND_FIELDS: [u32; 20] = [
    0x0CCB_7FC8,
    0x0CCB_7FC9,
    0x0CCB_7FD0,
    0x0CCB_7FD4,
    0x0CCB_7FD6,
    0x0D00_82F1,
    0x0D02_D586,
    0x0D02_D587,
    0x0D02_D588,
    0x0D02_D589,
    0x0D02_D58A,
    0x0D02_D58B,
    0x0D7A_F042,
    0x0D7A_F043,
    0x0D7A_F044,
    0x0D7A_F045,
    0x0D7A_F046,
    0x0D7A_F047,
    0x0D7A_F048,
    0x0D7A_F049,
];

fn parse_tgi(text: &str) -> Result<ResourceId, Box<dyn Error>> {
    let parts = text
        .split(':')
        .map(|part| u32::from_str_radix(part.trim_start_matches("0x"), 16))
        .collect::<Result<Vec<_>, _>>()?;
    if parts.len() != 3 {
        return Err(format!("invalid TGI {text:?}").into());
    }
    Ok(ResourceId {
        type_id: parts[0],
        group: parts[1],
        instance: parts[2],
    })
}

fn read(package: &Package, id: ResourceId) -> Result<Vec<u8>, Box<dyn Error>> {
    let entry = package.entry(id).ok_or_else(|| format!("missing {id}"))?;
    Ok(package.read(entry)?)
}

fn resolve_mask(
    package: &Package,
    key: sc_properties::Key,
) -> Result<(ResourceId, Vec<u8>), Box<dyn Error>> {
    for id in [
        ResourceId {
            type_id: key.type_id,
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: RASTER_TYPE,
            group: key.group,
            instance: key.instance,
        },
        ResourceId {
            type_id: RASTER_TYPE,
            group: 0,
            instance: key.instance,
        },
    ] {
        if id.type_id == RASTER_TYPE && package.entry(id).is_some() {
            return Ok((id, read(package, id)?));
        }
    }
    Err(format!(
        "LotMask raster {:08X}:{:08X}:{:08X} is not in package",
        key.type_id, key.group, key.instance
    )
    .into())
}

fn audit(package_path: &str, property_tgi: &str) -> Result<Json, Box<dyn Error>> {
    let package = Package::open(std::path::Path::new(package_path))?;
    let property_id = parse_tgi(property_tgi)?;
    if property_id.type_id != PROPERTY_TYPE {
        return Err(format!("{property_id} is not a property resource").into());
    }
    let property = PropertyFile::parse(&read(&package, property_id)?)?;
    let key = match property.get(LOT_MASK).map(|property| &property.kind) {
        Some(Kind::Scalar(Value::Key(key))) => *key,
        other => {
            return Err(format!("{property_id} LotMask is missing or not a key: {other:?}").into());
        }
    };
    let (mask_id, bytes) = resolve_mask(&package, key)?;
    let raster = RasterImage::parse(&bytes)?;
    let rgba = raster.decode_top_mip_rgba()?;
    let mut min = [u8::MAX; 4];
    let mut max = [u8::MIN; 4];
    let mut selected = [0usize; 4];
    let mut nonzero = [0usize; 4];
    let mut alpha_zero = 0usize;
    let mut alpha_255 = 0usize;
    let mut rgb_nonzero = 0usize;
    let mut any_coverage = 0usize;
    let mut colors: HashMap<[u8; 4], usize> = HashMap::new();
    for pixel in rgba.as_chunks::<4>().0 {
        let pixel = [pixel[0], pixel[1], pixel[2], pixel[3]];
        for channel in 0..4 {
            min[channel] = min[channel].min(pixel[channel]);
            max[channel] = max[channel].max(pixel[channel]);
            selected[channel] += usize::from(pixel[channel] >= 128);
            nonzero[channel] += usize::from(pixel[channel] != 0);
        }
        alpha_zero += usize::from(pixel[3] == 0);
        alpha_255 += usize::from(pixel[3] == 255);
        rgb_nonzero += usize::from(pixel[..3].iter().any(|value| *value != 0));
        any_coverage += usize::from(pixel.iter().any(|value| *value >= 128));
        *colors.entry(pixel).or_default() += 1;
    }
    let mut colors = colors.into_iter().collect::<Vec<_>>();
    colors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let top_colors = colors
        .iter()
        .take(16)
        .map(|(rgba, count)| json!({"rgba": rgba, "count": count}))
        .collect::<Vec<_>>();
    let fields = GROUND_FIELDS
        .iter()
        .filter_map(|hash| {
            property.get(*hash).map(|property| {
                (
                    format!("{hash:08X}"),
                    json!({
                        "type": property.prop_type.name(),
                        "kind": format!("{:?}", property.kind),
                    }),
                )
            })
        })
        .collect::<serde_json::Map<_, _>>();
    Ok(json!({
        "package": package_path,
        "property": property_id.to_string(),
        "maskKey": format!("{:08X}:{:08X}:{:08X}", key.type_id, key.group, key.instance),
        "maskResource": mask_id.to_string(),
        "resourceBytes": bytes.len(),
        "raster": {
            "version": raster.raster_type,
            "width": raster.width,
            "height": raster.height,
            "mipCount": raster.mip_count,
            "pixelSize": raster.pixel_size,
            "pixelFormat": raster.pixel_format,
            "topMipBytes": raster.mips.first().map(Vec::len),
            "rgbaSha256": format!("{:x}", Sha256::digest(&rgba)),
        },
        "pixels": {
            "count": rgba.len() / 4,
            "min": min,
            "max": max,
            "nonzeroByChannel": nonzero,
            "selectedAt128ByChannel": selected,
            "anyCoverageAt128": any_coverage,
            "alphaZero": alpha_zero,
            "alpha255": alpha_255,
            "rgbNonzero": rgb_nonzero,
            "uniquePixels": colors.len(),
            "topColors": top_colors,
        },
        "groundFields": fields,
    }))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.len() % 2 != 0 {
        return Err("expected <package> <property-tgi> pairs".into());
    }
    let reports = args
        .chunks_exact(2)
        .map(|pair| audit(&pair[0], &pair[1]))
        .collect::<Result<Vec<_>, _>>()?;
    println!("{}", serde_json::to_string_pretty(&reports)?);
    Ok(())
}
