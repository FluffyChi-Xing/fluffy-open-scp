//! Rewrite only the six external texture instances of a native RW4 template.
//!
//! The production compiler deliberately keeps the template's MATERIAL header,
//! shader block and unknown data.  It must not keep the template's six texture
//! instance ids: those point at the reference building and are not part of the
//! new asset's delta package.

use rw4::{Rw4File, SectionType};

/// Replace slot0..slot5 texture instance words in every MATERIAL section.
///
/// The returned bytes have the same length and section layout as the input;
/// only the six `texture_instance` fields change.  The function validates the
/// result by parsing and decoding the material again.
pub fn rewrite_texture_instances(rw4: &[u8], instances: &[u32; 6]) -> Result<Vec<u8>, String> {
    let file = Rw4File::parse(rw4).map_err(|e| format!("template RW4 parse failed: {e}"))?;
    let mut out = rw4.to_vec();
    let mut patched = 0usize;
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let payload = file
            .payload(rw4, section.number)
            .map_err(|e| format!("material #{} payload: {e}", section.number))?;
        let anchor = find_texture_table(payload)
            .ok_or_else(|| format!("material #{} has no shader-def table", section.number))?;
        if anchor + 7 * 24 > payload.len() {
            return Err(format!(
                "material #{} texture table is truncated",
                section.number
            ));
        }
        for (slot, instance) in instances.iter().enumerate() {
            let at = section.pos as usize + anchor + (slot + 1) * 24 + 8;
            let end = at
                .checked_add(4)
                .ok_or("material texture offset overflow")?;
            if end > out.len() {
                return Err(format!(
                    "material #{} slot{} texture instance is out of range",
                    section.number, slot
                ));
            }
            out[at..end].copy_from_slice(&instance.to_le_bytes());
        }
        patched += 1;
    }
    if patched == 0 {
        return Err("template RW4 has no MATERIAL section".into());
    }
    validate(&out, instances)?;
    Ok(out)
}

fn find_texture_table(payload: &[u8]) -> Option<usize> {
    // 4-byte Material size + 28-byte header + optional format copy / data.
    // The first aligned 0x2D word is the shader-def reference record.
    let mut at = 4 + 28;
    while at + 4 <= payload.len() {
        if payload[at..at + 4] == 0x2D_u32.to_le_bytes() {
            return Some(at);
        }
        at += 4;
    }
    None
}

fn validate(bytes: &[u8], instances: &[u32; 6]) -> Result<(), String> {
    let file = Rw4File::parse(bytes).map_err(|e| format!("rewritten RW4 parse failed: {e}"))?;
    let mut materials = 0usize;
    for section in file.sections_of_type(SectionType::MATERIAL) {
        let material = file
            .decode_material(bytes, section.number)
            .map_err(|e| format!("rewritten material #{} decode failed: {e}", section.number))?;
        let rw4::MaterialSection::Decoded(material) = material else {
            return Err(format!(
                "rewritten material #{} layout is unknown",
                section.number
            ));
        };
        for slot in material.texture_slots() {
            let index = slot.slot_byte() as usize;
            if index >= 6 {
                continue;
            }
            if slot.texture_instance != instances[index] {
                return Err(format!(
                    "rewritten material #{} slot{} instance mismatch",
                    section.number, index
                ));
            }
        }
        materials += 1;
    }
    if materials == 0 {
        return Err("rewritten RW4 lost its material".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rw4::writer::{build as build_file, section as writer_section};

    fn material_payload(instances: [u32; 6]) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&0u32.to_le_bytes()); // size placeholder
        p.extend_from_slice(&[0u8; 28]); // native material header
        p.extend_from_slice(&0x2D_u32.to_le_bytes());
        p.extend_from_slice(&[0u8; 20]);
        for (slot, instance) in instances.iter().enumerate() {
            p.extend_from_slice(&(slot as u32).to_le_bytes());
            p.extend_from_slice(&0u32.to_le_bytes());
            p.extend_from_slice(&instance.to_le_bytes());
            p.extend_from_slice(&[0u8; 12]);
        }
        p
    }

    #[test]
    fn rewrites_six_instances_without_changing_length() {
        let old = [1, 2, 3, 4, 5, 6];
        let body = material_payload(old);
        let file = build_file(1, &[writer_section(SectionType::MATERIAL, body)], &[]);
        let new = [11, 12, 13, 14, 15, 16];
        let out = rewrite_texture_instances(&file, &new).unwrap();
        assert_eq!(out.len(), file.len());
        validate(&out, &new).unwrap();
    }
}
