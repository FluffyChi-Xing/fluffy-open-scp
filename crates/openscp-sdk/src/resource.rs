//! Checked conversion at the boundary between u64 manifests and u32 DBPF IDs.
use crate::{Error, ResourceId, Result};

/// Construct the existing DBPF identity without truncating a manifest instance.
/// Text parsing and manifest schemas remain owned by the caller.
pub fn checked_id(type_id: u32, group: u32, instance: u64) -> Result<ResourceId> {
    let instance = u32::try_from(instance).map_err(|_| Error::InstanceOutOfRange { instance })?;
    Ok(ResourceId {
        type_id,
        group,
        instance,
    })
}
