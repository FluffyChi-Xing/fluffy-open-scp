//! Parser for the workbench LOTM envelope.
//!
//! LOTM is a small envelope around a standard GLB. Keeping this parser
//! independent from the RW4 compiler lets the compiler validate and stage the
//! exact model bytes before it starts producing native resources.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LotmDocument {
    pub version: u32,
    pub metadata: u32,
    pub glb: Vec<u8>,
}

pub fn parse(bytes: &[u8]) -> Result<LotmDocument, String> {
    if bytes.len() < 28 || &bytes[..4] != b"LOTM" {
        return Err("invalid LOTM header".into());
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let metadata = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let glb_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    if glb_len < 12 || 16usize.checked_add(glb_len).is_none_or(|end| end > bytes.len()) {
        return Err("LOTM GLB payload is truncated".into());
    }
    let glb = bytes[16..16 + glb_len].to_vec();
    if &glb[..4] != b"glTF" || u32::from_le_bytes(glb[4..8].try_into().unwrap()) != 2 {
        return Err("LOTM payload is not a GLB v2 document".into());
    }
    let declared = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if declared != glb.len() {
        return Err(format!("GLB length mismatch: header {declared}, actual {}", glb.len()));
    }
    Ok(LotmDocument { version, metadata, glb })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_validates_embedded_glb() {
        let mut bytes = b"LOTM".to_vec();
        bytes.extend_from_slice(&9u32.to_le_bytes());
        bytes.extend_from_slice(&0x4Au32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&12u32.to_le_bytes());
        let doc = parse(&bytes).unwrap();
        assert_eq!(doc.version, 9);
        assert_eq!(doc.glb, &bytes[16..]);
    }
}
