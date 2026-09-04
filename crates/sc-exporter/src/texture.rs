//! Texture export API for decoded RW4 textures.
use image::{DynamicImage, ImageFormat, RgbaImage};
use rw4::DecodedTexture;
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureOutputFormat {
    Png,
    Jpg,
    Tga,
    Dds,
}

impl std::str::FromStr for TextureOutputFormat {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "png" => Ok(Self::Png),
            "jpg" | "jpeg" => Ok(Self::Jpg),
            "tga" => Ok(Self::Tga),
            "dds" => Ok(Self::Dds),
            other => Err(format!("unsupported texture format: {other}")),
        }
    }
}

pub fn export_texture(
    texture: &DecodedTexture,
    format: TextureOutputFormat,
) -> crate::Result<Vec<u8>> {
    if format == TextureOutputFormat::Dds {
        return Ok(texture.write_dds()?);
    }
    let rgba = texture.decode_top_mip_rgba()?;
    match format {
        TextureOutputFormat::Png => {
            encode_image(&rgba, texture.width, texture.height, ImageFormat::Png, None)
        }
        TextureOutputFormat::Jpg => encode_image(
            &rgba,
            texture.width,
            texture.height,
            ImageFormat::Jpeg,
            Some(85),
        ),
        TextureOutputFormat::Tga => Ok(encode_tga(&rgba, texture.width, texture.height)),
        TextureOutputFormat::Dds => unreachable!(),
    }
}

fn encode_image(
    rgba: &[u8],
    width: u16,
    height: u16,
    format: ImageFormat,
    quality: Option<u8>,
) -> crate::Result<Vec<u8>> {
    let image =
        RgbaImage::from_raw(width.into(), height.into(), rgba.to_vec()).ok_or_else(|| {
            crate::Error::Rw4(rw4::Error::InsufficientPayload {
                check: "TX_IMAGE",
                needed: usize::from(width) * usize::from(height) * 4,
                actual: rgba.len(),
            })
        })?;
    let mut out = Cursor::new(Vec::new());
    let dynamic = DynamicImage::ImageRgba8(image);
    if format == ImageFormat::Jpeg {
        let rgb = dynamic.to_rgb8();
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.unwrap_or(85));
        encoder
            .encode(
                rgb.as_raw(),
                width.into(),
                height.into(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| crate::Error::Io(std::io::Error::other(e)))?;
    } else {
        dynamic
            .write_to(&mut out, format)
            .map_err(|e| crate::Error::Io(std::io::Error::other(e)))?;
    }
    Ok(out.into_inner())
}

fn encode_tga(rgba: &[u8], width: u16, height: u16) -> Vec<u8> {
    let mut out = vec![0u8; 18];
    out[2] = 2;
    out[12..14].copy_from_slice(&width.to_le_bytes());
    out[14..16].copy_from_slice(&height.to_le_bytes());
    out[16] = 32;
    out[17] = 8;
    for px in rgba.as_chunks::<4>().0 {
        out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn raw_bgra() -> DecodedTexture {
        DecodedTexture {
            texture_type: rw4::TEXTURE_TYPE_RAW_BGRA,
            unknown1: 0,
            width: 2,
            height: 1,
            mipmap_info: 1 << 8,
            data_section: 0,
            blob: vec![10, 20, 30, 255, 40, 50, 60, 128],
        }
    }

    #[test]
    fn png_export_is_decodable_and_preserves_pixels() {
        let bytes = export_texture(&raw_bgra(), TextureOutputFormat::Png).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        let image = image::load_from_memory_with_format(&bytes, ImageFormat::Png)
            .unwrap()
            .to_rgba8();
        assert_eq!(image.as_raw(), &[30, 20, 10, 255, 60, 50, 40, 128]);
    }

    #[test]
    fn jpg_export_is_decodable() {
        let bytes = export_texture(&raw_bgra(), TextureOutputFormat::Jpg).unwrap();
        assert_eq!(&bytes[..2], b"\xff\xd8");
        let image = image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg).unwrap();
        assert_eq!(image.dimensions(), (2, 1));
    }

    #[test]
    fn tga_export_has_uncompressed_bgra_layout() {
        let bytes = export_texture(&raw_bgra(), TextureOutputFormat::Tga).unwrap();
        assert_eq!(&bytes[1..3], &[0, 2]);
        assert_eq!(u16::from_le_bytes(bytes[12..14].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(bytes[14..16].try_into().unwrap()), 1);
        assert_eq!(bytes[16], 32);
        assert_eq!(&bytes[18..], &[10, 20, 30, 255, 40, 50, 60, 128]);
    }

    #[test]
    fn dds_export_has_valid_compressed_header() {
        let texture = DecodedTexture {
            texture_type: rw4::TEXTURE_TYPE_DXT1,
            unknown1: 0,
            width: 4,
            height: 4,
            mipmap_info: 1 << 8,
            data_section: 0,
            blob: vec![0; 8],
        };
        let bytes = export_texture(&texture, TextureOutputFormat::Dds).unwrap();
        assert_eq!(&bytes[..4], b"DDS ");
        assert_eq!(&bytes[84..88], b"DXT1");
        assert_eq!(bytes.len(), 136);
    }
}
