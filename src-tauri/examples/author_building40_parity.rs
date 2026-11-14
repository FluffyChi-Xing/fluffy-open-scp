//! Offline oracle for the author's LOTM9 → Building40 route.
//! Writes only below the repository's tmp directory; never deploys a package.

use fluffy_open_scp_lib::asset_build_m1::{author_building40, lotm, template_material};
use rw4::{Rw4File, SectionType};
use sha2::{Digest, Sha256};
use std::{env, fs};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = env::current_dir()?;
    let input = root.join("tools/sc5_asset_packager/projects/tall_test/input/tall_test.lotm");
    let expected_path = root.join("tools/sc5_asset_packager/projects/tall_test/build/run-c0814b62eacb/candidate/资源/2F4E681B_00000000_16A11E98.rw4");
    let output = root.join("tmp/author-rust-parity/tall_test.rw4");
    let document = lotm::parse(&fs::read(input)?)?;
    let template = include_bytes!("../resources/sc5/175EFB4E.research-template.rw4");
    let collision = include_bytes!("../resources/sc5/collision-source.rw4");
    let model = author_building40::compile_model(&document, 180, template, collision)?;
    let slots = [
        0x7EC2E70C, 0x7EC2E70D, 0x7EC2E70E, 0x7EC2E70F, 0x7EC2E708, 0x7EC2E709,
    ];
    let actual = template_material::rewrite_texture_instances(&model.compiled.rw4, &slots)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&output, &actual)?;
    let expected = fs::read(&expected_path)?;
    let first_difference = actual.iter().zip(expected.iter()).position(|(a, b)| a != b);
    let file = Rw4File::parse(&actual)?;
    let mesh = file
        .sections_of_type(SectionType::MESH)
        .next()
        .ok_or("mesh missing")?;
    let decoded = file.decode_mesh(&actual, mesh.number)?;
    println!("output={}", output.display());
    println!(
        "groups={} source_triangles={} removed_degenerate={}",
        model.source_groups, model.source_triangles, model.removed_degenerate_triangles
    );
    println!(
        "vertices={} triangles={} bounds={:?}",
        decoded.vertices.len(),
        decoded.triangles.len(),
        model.bounds
    );
    println!("actual_sha256={} bytes={}", sha(&actual), actual.len());
    println!(
        "expected_sha256={} bytes={}",
        sha(&expected),
        expected.len()
    );
    println!(
        "byte_identical={} first_difference={:?}",
        actual == expected,
        first_difference
    );
    if actual != expected {
        return Err("Building40 oracle mismatch".into());
    }
    Ok(())
}
