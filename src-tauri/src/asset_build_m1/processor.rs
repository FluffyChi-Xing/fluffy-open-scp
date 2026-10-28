//! Property processor: immutable bytes in, verified bytes out, no filesystem.
use super::patch::{PropertyPatch, apply_patch};
use dbpf::ResourceId;
use sc_properties::PropertyFile;

pub struct PropertyArtifact {
    pub target: ResourceId,
    pub materialize: Box<dyn FnOnce() -> Result<Vec<u8>, String> + Send>,
}

pub fn defer_property(
    target: ResourceId,
    source: Vec<u8>,
    patches: Vec<PropertyPatch>,
) -> PropertyArtifact {
    PropertyArtifact {
        target,
        materialize: Box::new(move || process_property(&source, &patches)),
    }
}

pub fn process_property(source: &[u8], patches: &[PropertyPatch]) -> Result<Vec<u8>, String> {
    let mut file = PropertyFile::parse(source).map_err(|e| e.to_string())?;
    // The general reader tolerates truncated counts/trailing data. A writer
    // must fail closed rather than silently dropping bytes it cannot represent.
    let encoded = file.encode_canonical().map_err(|e| e.to_string())?;
    if file.claimed_count as usize != file.values.len() || encoded != source {
        return Err("source Property is not fully representable; writeback refused".into());
    }
    for patch in patches {
        file = apply_patch(&file, patch)?;
    }
    let output = file.encode_canonical().map_err(|e| e.to_string())?;
    let readback = PropertyFile::parse(&output).map_err(|e| e.to_string())?;
    if readback.values.len() != file.values.len()
        || file.values.iter().any(|expected| {
            !readback.get(expected.hash).is_some_and(|actual| {
                actual.prop_type == expected.prop_type && actual.kind == expected.kind
            })
        })
    {
        return Err("Property semantic readback mismatch".into());
    }
    Ok(output)
}
