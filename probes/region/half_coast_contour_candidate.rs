//! Build a data-view overlay paired with the environment-gated contour writer.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io::Write, path::Path};

const APP_SHA: &str = "a290bc0d81faf75c8a04e852ce39c859ad8651b64b10b03d6d8186393c2b4066";
const STOCK_SAMPLE: &str = concat!(
    "float2 texSize = contourInfo.mTextureSize.xy;\n",
    "float2 texelSize = contourInfo.mTextureSize.zw;\n",
    "float2 tiers = float2(contourInfo.mContourInfo[0].x, contourInfo.mContourInfo[1].x);\n",
    "float2 bottomFilter = float2(contourInfo.mContourInfo[0].y, contourInfo.mContourInfo[1].y);\n",
    "float2 topFilter = float2(contourInfo.mContourInfo[0].z, contourInfo.mContourInfo[1].z);\n",
    "float3 primaryContourColor = contourInfo.mMapColor[0].rgb;\n",
    "float2 ecoMap = bicubic(contourMap, controlMapUV, texSize, texelSize);",
);
const SCOPED_SAMPLE: &str = concat!(
    "float2 texSize=contourInfo.mTextureSize.xy;\n",
    "float2 texelSize=contourInfo.mTextureSize.zw;\n",
    "float4 ci0=contourInfo.mContourInfo[0],ci1=contourInfo.mContourInfo[1];\n",
    "float2 tiers=float2(ci0.x,ci1.x);\n",
    "float2 bottomFilter=float2(ci0.y,ci1.y);\n",
    "float2 topFilter=float2(ci0.z,ci1.z);\n",
    "float3 primaryContourColor=contourInfo.mMapColor[0].rgb;\n",
    "bool big=texSize.x<0;",
    "float2 ecoMap=bicubic(contourMap,big?worldPos.xy/8192+.5:controlMapUV,abs(texSize),texelSize);",
);
const STOCK_CLIP: &str = "abs(worldPos.x) < 1024 && abs(worldPos.y) < 1024";
const SCOPED_CLIP: &str = "all(abs(worldPos.xy)<(big?4096:1024))";

fn replace(data: &mut [u8], from: &str, to: &str) -> Result<(), Box<dyn Error>> {
    let hits = data
        .windows(from.len())
        .enumerate()
        .filter_map(|(i, bytes)| (bytes == from.as_bytes()).then_some(i))
        .collect::<Vec<_>>();
    if hits.len() != 1 || to.len() > from.len() {
        return Err("shader anchor is missing, ambiguous or too short".into());
    }
    let start = hits[0];
    data[start..start + from.len()].fill(b' ');
    data[start..start + to.len()].copy_from_slice(to.as_bytes());
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let [app_path, output] = args.as_slice() else {
        return Err(
            "usage: half_coast_contour_candidate SimCity_App.package workspace-output.package"
                .into(),
        );
    };
    let workspace_tmp = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tmp")
        .canonicalize()?;
    let output = Path::new(output);
    if !output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(workspace_tmp)
    {
        return Err("stage shared shader diagnostics under workspace tmp".into());
    }

    let mut hash = Sha256::new();
    std::io::copy(&mut fs::File::open(app_path)?, &mut hash)?;
    if format!("{:x}", hash.finalize()) != APP_SHA {
        return Err("unexpected stock App package".into());
    }
    let app = Package::open(app_path)?;
    let mut entries = Vec::new();
    for group in [0x40212002, 0x40212012] {
        for instance in 0..4 {
            let id = ResourceId {
                type_id: 0x0469a3f7,
                group,
                instance,
            };
            let mut bytes = app.read(app.entry(id).ok_or("shader resource missing")?)?;
            replace(&mut bytes, STOCK_SAMPLE, SCOPED_SAMPLE)?;
            replace(&mut bytes, STOCK_CLIP, SCOPED_CLIP)?;
            entries.push(OverlayEntry::new(id, bytes));
        }
    }
    let bytes = write_uncompressed_overlay(&entries)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    let written = Package::open(output)?;
    if written.entries().len() != entries.len() {
        return Err("shader overlay resource count mismatch".into());
    }
    for entry in entries {
        if written.read(
            written
                .entry(entry.id)
                .ok_or("shader missing after write")?,
        )? != entry.data
        {
            return Err("shader overlay readback mismatch".into());
        }
    }
    println!("Data-view diagnostic SHA-256 {:x}", Sha256::digest(&bytes));
    Ok(())
}
