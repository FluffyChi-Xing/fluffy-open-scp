//! Offline oracle for the author's LOTM9 six-slot material route.
//! Writes only below the repository's tmp directory; never deploys a package.

use std::{env, fs, path::Path};

use fluffy_open_scp_lib::asset_build_m1::{author_materials, lotm};
use image::RgbaImage;
use sha2::{Digest, Sha256};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn save_png(path: &Path, image: &RgbaImage) -> Result<(), Box<dyn std::error::Error>> {
    image.save(path)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::current_dir()?;
    let project = root.join("tools/sc5_asset_packager/projects/tall_test");
    let expected_root = project.join("build/run-c0814b62eacb");
    let output = root.join("tmp/author-material-parity");
    fs::create_dir_all(&output)?;
    let document = lotm::parse(&fs::read(project.join("input/tall_test.lotm"))?)?;
    let compiled = author_materials::compile_materials(&document, true)?;
    println!(
        "groups={} parameter_width={} surface_grid={} room_atlas={}",
        document.materials.len(),
        compiled.plan.width,
        compiled.plan.grid,
        compiled.plan.room_atlas
    );

    let mut identical = true;
    let expected_params = fs::read(expected_root.join("params.float32"))?;
    fs::write(output.join("params.float32"), &compiled.params_f32)?;
    println!(
        "params actual_sha256={} expected_sha256={} identical={}",
        sha(&compiled.params_f32),
        sha(&expected_params),
        compiled.params_f32 == expected_params
    );
    identical &= compiled.params_f32 == expected_params;

    let images = [
        ("tint", &compiled.atlases.tint),
        ("normal", &compiled.atlases.normal),
        ("surface", &compiled.atlases.shader),
        ("palette", &compiled.atlases.palette),
        ("interior", &compiled.atlases.interior),
    ];
    for (name, actual) in images {
        save_png(&output.join(format!("{name}.png")), actual)?;
        let expected = image::open(expected_root.join(format!("{name}.png")))?.to_rgba8();
        let same = actual == &expected;
        let first_difference = actual
            .as_raw()
            .iter()
            .zip(expected.as_raw())
            .position(|(a, b)| a != b);
        println!(
            "{name} dimensions={}x{} actual_pixel_sha256={} expected_pixel_sha256={} identical={} first_difference={:?}",
            actual.width(),
            actual.height(),
            sha(actual.as_raw()),
            sha(expected.as_raw()),
            same,
            first_difference
        );
        identical &= same;
    }

    let slot_files = [
        "2F4E681B_00000000_7EC2E70C.rw4",
        "2F4E681C_00000000_7EC2E70D.bin",
        "2F4E681C_00000000_7EC2E70E.bin",
        "2F4E681C_00000000_7EC2E70F.bin",
        "2F4E681B_00000000_7EC2E708.rw4",
        "2F4E681B_00000000_7EC2E709.rw4",
    ];
    for (slot, filename) in slot_files.iter().enumerate() {
        let actual = &compiled.slots[slot];
        let expected = fs::read(expected_root.join("candidate/资源").join(filename))?;
        fs::write(output.join(filename), actual)?;
        let same = actual == &expected;
        let first_difference = actual.iter().zip(expected.iter()).position(|(a, b)| a != b);
        println!(
            "slot{slot} actual_sha256={} expected_sha256={} actual_bytes={} expected_bytes={} identical={} first_difference={:?}",
            sha(actual),
            sha(&expected),
            actual.len(),
            expected.len(),
            same,
            first_difference
        );
        identical &= same;
    }
    println!("byte_identical={identical}");
    if !identical {
        return Err("six-slot material oracle mismatch".into());
    }
    Ok(())
}
