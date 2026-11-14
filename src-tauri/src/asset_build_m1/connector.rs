//! DBPF adapter. Unchanged resources remain opaque, including RW4/Raster.
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::Path};

pub type Resources = HashMap<ResourceId, Vec<u8>>;

pub fn fingerprint(resources: &Resources) -> String {
    let mut sorted: Vec<_> = resources.iter().collect();
    sorted.sort_by_key(|(id, _)| (id.type_id, id.group, id.instance));
    let mut hash = Sha256::new();
    for (id, bytes) in sorted {
        for word in [id.type_id, id.group, id.instance] {
            hash.update(word.to_be_bytes());
        }
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(bytes);
    }
    format!("{hash:x}", hash = hash.finalize())
}

pub fn read_source(path: &Path) -> Result<Resources, String> {
    let package = Package::open(path).map_err(|e| e.to_string())?;
    let mut resources = Resources::new();
    for entry in package.entries() {
        let bytes = package.read(entry).map_err(|e| e.to_string())?;
        if resources.insert(entry.id, bytes).is_some() {
            return Err(format!("duplicate source TGI {}", entry.id));
        }
    }
    Ok(resources)
}

pub fn link(resources: &Resources) -> Result<Vec<u8>, String> {
    let entries: Vec<_> = resources
        .iter()
        .map(|(id, bytes)| OverlayEntry::new(*id, bytes.clone()))
        .collect();
    write_uncompressed_overlay(&entries).map_err(|e| e.to_string())
}

pub fn verify(path: &Path, expected: &Resources) -> Result<(), String> {
    let actual = read_source(path)?;
    if &actual != expected {
        return Err("DBPF resource identity/payload readback mismatch".into());
    }
    Ok(())
}
