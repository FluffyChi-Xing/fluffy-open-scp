//! Prepare an offline DBPF delta for the first in-game reload experiment.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sc_properties::{Kind, PROPERTY_RESOURCE_TYPE, PropertyFile, Value};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

const MENU_GROUP: u32 = 0x0987_8A01;
const TOOL_TITLE: u32 = 0x0A09_F5FA;
const TOOL_DESCRIPTION: u32 = 0x0A09_F5FB;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn package_sha256(path: &Path) -> Result<String> {
    let mut hash = Sha256::new();
    io::copy(&mut File::open(path)?, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn keys(package: &Package) -> Result<HashSet<ResourceId>> {
    let mut ids = HashSet::new();
    for entry in package.entries() {
        if !ids.insert(entry.id) {
            return Err(
                format!("Duplicate TGI {} in {}", entry.id, package.path().display()).into(),
            );
        }
    }
    Ok(ids)
}

fn inspect_menu(data: &[u8]) -> Result<PropertyFile> {
    let props = PropertyFile::parse(data)?;
    if props.values.len() != props.claimed_count as usize || props.encode_canonical()? != data {
        return Err("Menu2 is not a complete canonical property list".into());
    }
    let mut hashes = HashSet::new();
    if props.values.iter().any(|prop| !hashes.insert(prop.hash)) {
        return Err("Menu2 has duplicate properties".into());
    }
    Ok(props)
}

fn menu_change(before: &[u8], after: &[u8]) -> Result<u32> {
    let before = inspect_menu(before)?;
    let mut after = inspect_menu(after)?;
    let mut changed = None;
    for (old, new) in before.values.iter().zip(&mut after.values) {
        if old == new {
            continue;
        }
        if changed.is_some()
            || old.hash != new.hash
            || !matches!(new.hash, TOOL_TITLE | TOOL_DESCRIPTION)
            || old.prop_type != new.prop_type
            || old.encoding != new.encoding
            || !matches!((&old.kind, &new.kind), (
                Kind::Array(a), Kind::Array(b)
            ) if matches!((&a[..], &b[..]), ([Value::Text(_)], [Value::Text(_)])))
        {
            return Err(
                "Expected one existing Menu2 title or description reference to change".into(),
            );
        }
        changed = Some(new.hash);
        *new = old.clone();
    }
    if after.encode_canonical()? != before.encode_canonical()? {
        return Err("Other Menu2 data changed".into());
    }
    changed.ok_or_else(|| "Menu2 reference did not change".into())
}

fn prepare(baseline: &Path, candidate: &Path, instance: u32, output: &Path) -> Result<Json> {
    let baseline_hash = package_sha256(baseline)?;
    let candidate_hash = package_sha256(candidate)?;
    let baseline = Package::open(baseline)?;
    let candidate = Package::open(candidate)?;
    if keys(&baseline)? != keys(&candidate)? {
        return Err("Package TGI set changed".into());
    }
    let target = ResourceId {
        type_id: PROPERTY_RESOURCE_TYPE,
        group: MENU_GROUP,
        instance,
    };
    if baseline.entry(target).is_none() {
        return Err(format!("Menu2 TGI {target} is missing from the baseline").into());
    }

    let mut delta = None;
    for entry in baseline.entries() {
        let old_bytes = baseline.read(entry)?;
        let next_entry = candidate.entry(entry.id).ok_or("Candidate index changed")?;
        let new_bytes = candidate.read(next_entry)?;
        if old_bytes == new_bytes {
            continue;
        }
        if entry.id != target {
            return Err(
                format!("Changed resource {} is outside the target Menu2", entry.id).into(),
            );
        }
        let hash = menu_change(&old_bytes, &new_bytes)?;
        delta = Some((hash, new_bytes));
    }

    let (property, bytes) = delta.ok_or("Package has no changed resource payloads")?;
    let overlay = write_uncompressed_overlay(&[OverlayEntry::new(target, bytes)])?;
    if package_sha256(baseline.path())? != baseline_hash
        || package_sha256(candidate.path())? != candidate_hash
    {
        return Err("Input package changed while preparing the delta".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&overlay)?;
    file.sync_all()?;

    Ok(json!({
        "status": "prepared",
        "baselineSha256": baseline_hash,
        "candidateSha256": candidate_hash,
        "overlaySha256": format!("{:x}", Sha256::digest(&overlay)),
        "overlay": output,
        "tgi": target.to_string(),
        "property": format!("{property:08X}"),
    }))
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [baseline, candidate, instance, output] = args.as_slice() else {
        return Err(
            "Usage: package_reload_prepare BASELINE CANDIDATE MENU_INSTANCE_HEX OUTPUT".into(),
        );
    };
    let instance = instance.to_str().ok_or("Invalid menu instance")?;
    let instance = u32::from_str_radix(instance.trim_start_matches("0x"), 16)?;
    let result = prepare(
        Path::new(baseline),
        Path::new(candidate),
        instance,
        Path::new(output),
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sc_properties::{PropType, Property, PropertyEncoding, Text};

    fn menu(title: u32, description: u32) -> Vec<u8> {
        PropertyFile {
            values: [(TOOL_TITLE, title), (TOOL_DESCRIPTION, description)]
                .into_iter()
                .map(|(hash, instance_id)| Property {
                    hash,
                    prop_type: PropType::Text,
                    kind: Kind::Array(vec![Value::Text(Text {
                        table_id: 0x1122_3344,
                        instance_id,
                    })]),
                    encoding: PropertyEncoding {
                        flags: 0x801C,
                        array_item_size: Some(8),
                    },
                })
                .collect(),
            claimed_count: 2,
        }
        .encode_canonical()
        .unwrap()
    }

    #[test]
    fn prepares_round_trip_and_rejects_a_second_resource_change() {
        let folder = std::env::temp_dir().join(format!(
            "openscp-reload-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&folder).unwrap();
        let a = folder.join("a.package");
        let b = folder.join("b.package");
        let rejected = folder.join("rejected.package");
        let unrelated = ResourceId {
            type_id: 0x0A98_EAF0,
            group: 0x02FA_BF01,
            instance: 0x1122_3344,
        };
        let target = ResourceId {
            type_id: PROPERTY_RESOURCE_TYPE,
            group: MENU_GROUP,
            instance: 0x1122_3344,
        };
        let write_package = |path: &Path, title, other: &[u8]| {
            let bytes = write_uncompressed_overlay(&[
                OverlayEntry::new(target, menu(title, 2)),
                OverlayEntry::new(unrelated, other.to_vec()),
            ])
            .unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        write_package(&a, 1, b"same");
        write_package(&b, 3, b"same");
        for (before, after, title) in [(&a, &b, 3), (&b, &a, 1)] {
            let output = folder.join(format!("delta-{title}.package"));
            let manifest = prepare(before, after, target.instance, &output).unwrap();
            assert_eq!(manifest["property"], "0A09F5FA");
            assert_eq!(manifest["overlaySha256"], package_sha256(&output).unwrap());
            let delta = Package::open(&output).unwrap();
            assert_eq!(delta.entries().len(), 1);
            assert_eq!(
                delta.read(delta.entry(target).unwrap()).unwrap(),
                menu(title, 2)
            );
            assert!(prepare(before, after, target.instance, &output).is_err());
        }
        write_package(&b, 3, b"changed");
        assert!(prepare(&a, &b, target.instance, &rejected).is_err());
        assert!(!rejected.exists());
        for name in [
            "a.package",
            "b.package",
            "delta-1.package",
            "delta-3.package",
        ] {
            std::fs::remove_file(folder.join(name)).unwrap();
        }
        std::fs::remove_dir(folder).unwrap();
    }
}
