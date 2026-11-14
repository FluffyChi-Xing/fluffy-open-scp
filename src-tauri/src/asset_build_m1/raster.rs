//! Pure raster overlay processor for the isolated M1 pipeline.
//!
//! The processor accepts one decoded game raster and returns bytes for a new
//! resource. It does not open packages or write files; collision checks are
//! performed against the caller's resource index.

use dbpf::ResourceId;
use rw4::RasterImage;

pub const RASTER_TYPE: u32 = 0x2F4E_681C;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterOverlayArtifact {
    pub target: ResourceId,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_format: u32,
}

pub fn ensure_target_free<I>(existing: I, target: ResourceId) -> Result<(), String>
where
    I: IntoIterator<Item = ResourceId>,
{
    if existing.into_iter().any(|id| id == target) {
        Err(format!("raster target already exists: {target}"))
    } else {
        Ok(())
    }
}

/// Encode a new raster resource after changing one top-level RGBA byte.
/// Keeping this operation pure makes it safe to run before staging any output.
pub fn encode_pixel_delta(
    source: &[u8],
    target: ResourceId,
    byte_index: usize,
) -> Result<RasterOverlayArtifact, String> {
    if target.type_id != RASTER_TYPE {
        return Err(format!("raster target has wrong type: {target}"));
    }
    let image = RasterImage::parse(source).map_err(|e| e.to_string())?;
    let mut rgba = image.decode_top_mip_rgba().map_err(|e| e.to_string())?;
    let byte = rgba
        .get_mut(byte_index)
        .ok_or_else(|| "pixel byte index is out of bounds".to_string())?;
    *byte = byte.wrapping_add(1);
    let rebuilt =
        RasterImage::build_raw_bgra(image.width, image.height, &rgba, image.mip_count > 1)
            .map_err(|e| e.to_string())?;
    let bytes = rebuilt.to_bytes();
    let readback = RasterImage::parse(&bytes).map_err(|e| e.to_string())?;
    if readback.decode_top_mip_rgba().map_err(|e| e.to_string())? != rgba {
        return Err("raster semantic readback mismatch".into());
    }
    Ok(RasterOverlayArtifact {
        target,
        bytes,
        width: image.width,
        height: image.height,
        pixel_format: rebuilt.pixel_format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_collision_and_wrong_type() {
        let target = ResourceId {
            type_id: RASTER_TYPE,
            group: 0,
            instance: 7,
        };
        assert!(ensure_target_free([target], target).is_err());
        assert!(ensure_target_free([], target).is_ok());
        assert!(
            encode_pixel_delta(
                &[],
                ResourceId {
                    type_id: 1,
                    group: 0,
                    instance: 1
                },
                0
            )
            .is_err()
        );
    }
}
