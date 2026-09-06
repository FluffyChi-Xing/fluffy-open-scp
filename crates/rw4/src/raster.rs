//! Raster（独立资源类型 `0x2F4E681C`）解析：D3D8/9 风格的纹理容器。
//!
//! 对齐 C# `Views/viewHelpers/RasterImage.cs` 与 HANDOFF §4：
//! - 头部 6 × u32（大端）：rasterType、width、height、mipCount、pixelSize、
//!   pixelFormat；
//! - 随后逐 mip：`[u32 blockSize][载荷]`，宽高逐级减半；
//! - `pixelFormat == 21` = D3DFMT_A8R8G8B8：未压缩 32bit 纹理，像素按顺序
//!   R,G,B,A 存储（对齐 C# 读取器与用户对拍；注意 RW4 内嵌 raw 纹理为
//!   B,G,R,A，两者不同）；DXT 压缩变体（pixFmt != 21）C# CLI 同样未
//!   实现，仅保留元数据。

use crate::error::{Error, Result};
use crate::reader::Reader;

/// D3DFMT_A8R8G8B8：未压缩 32bit 纹理。
pub const RASTER_PIXEL_FORMAT_A8R8G8B8: u32 = 21;

/// 已解析的 Raster 容器（mip 载荷原样保留）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterImage {
    pub raster_type: u32,
    pub width: u32,
    pub height: u32,
    pub mip_count: u32,
    pub pixel_size: u32,
    pub pixel_format: u32,
    /// 每个 mip 的像素载荷（blockSize 头之后的数据）。
    pub mips: Vec<Vec<u8>>,
}

impl RasterImage {
    pub fn parse(data: &[u8]) -> Result<RasterImage> {
        let mut r = Reader::new(data);
        let raster_type = r.u32be("RA_type")?;
        let width = r.u32be("RA_width")?;
        let height = r.u32be("RA_height")?;
        let mip_count = r.u32be("RA_mipcount")?;
        let pixel_size = r.u32be("RA_pixelsize")?;
        let pixel_format = r.u32be("RA_pixfmt")?;

        let mut mips = Vec::with_capacity(mip_count.min(64) as usize);
        for mip in 0..mip_count {
            let block_size = r.u32be("RA_blocksize")? as usize;
            let payload = r.take(block_size, "RA_pixels")?;
            mips.push(payload.to_vec());
            let _ = mip;
        }

        Ok(RasterImage {
            raster_type,
            width,
            height,
            mip_count,
            pixel_size,
            pixel_format,
            mips,
        })
    }

    /// pixFmt 21（未压缩 BGRA）可解码；DXT 压缩变体暂不支持。
    pub fn is_raw_rgba(&self) -> bool {
        self.pixel_format == RASTER_PIXEL_FORMAT_A8R8G8B8
    }

    fn top_mip_bytes(&self) -> Result<&[u8]> {
        let needed = self.width as usize * self.height as usize * 4;
        let mip = self.mips.first().ok_or(Error::InsufficientPayload {
            check: "RA000",
            needed,
            actual: 0,
        })?;
        if mip.len() < needed {
            return Err(Error::InsufficientPayload {
                check: "RA000",
                needed,
                actual: mip.len(),
            });
        }
        Ok(&mip[..needed])
    }

    /// 顶层 mip 解码为 RGBA8。
    ///
    /// 字节序为顺序 R,G,B,A（对齐 C# `RasterImage` 读取器，用户对拍确认；
    /// 注意与 [`crate::texture`] RW4 内嵌 raw 纹理的 B,G,R,A 不同）。
    pub fn decode_top_mip_rgba(&self) -> Result<Vec<u8>> {
        if !self.is_raw_rgba() {
            return Err(Error::UnsupportedRasterPixelFormat(self.pixel_format));
        }
        let mip = self.top_mip_bytes()?;
        Ok(mip.to_vec())
    }

    /// LotMask 四层量化（复刻 SCP `ViewLotEditor` 的 `RasterChannel.Preview`）：
    /// 每像素 4 字节各对应一层，阈值 ≥128 选中该层颜色并输出不透明像素，
    /// 全部未选中输出全透明。字节→颜色映射按 C# 调用序
    ///（`color4, color3, color2, color1` 传入形参 `color1..4`）：
    /// byte3→colors[3]、byte0→colors[2]、byte1→colors[1]、byte2→colors[0]，
    /// 入参 `colors` 顺序为 `[LotColor1, LotColor2, LotColor3, LotColor4]`。
    pub fn decode_lot_mask_rgba(&self, colors: &[[u8; 3]; 4]) -> Result<Vec<u8>> {
        if !self.is_raw_rgba() {
            return Err(Error::UnsupportedRasterPixelFormat(self.pixel_format));
        }
        let mip = self.top_mip_bytes()?;
        const BYTE_TO_COLOR: [(usize, usize); 4] = [(3, 3), (0, 2), (1, 1), (2, 0)];
        let mut rgba = Vec::with_capacity(mip.len());
        for px in mip.as_chunks::<4>().0 {
            let chosen = BYTE_TO_COLOR
                .iter()
                .find(|(byte_index, _)| px[*byte_index] >= 128)
                .map(|(_, color_index)| colors[*color_index]);
            match chosen {
                Some([r, g, b]) => rgba.extend_from_slice(&[r, g, b, 255]),
                None => rgba.extend_from_slice(&[0, 0, 0, 0]),
            }
        }
        Ok(rgba)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(pixfmt: u32, width: u32, height: u32, pixels: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for value in [2u32, width, height, 1, 8, pixfmt] {
            out.extend_from_slice(&value.to_be_bytes());
        }
        out.extend_from_slice(&(pixels.len() as u32).to_be_bytes());
        out.extend_from_slice(pixels);
        out
    }

    #[test]
    fn parses_header_and_keeps_sequential_rgba() {
        // 顺序 R,G,B,A 直读（不重排）。
        let data = raster(
            21,
            2,
            1,
            &[10, 20, 30, 40, 50, 60, 70, 80],
        );
        let image = RasterImage::parse(&data).unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.mip_count, 1);
        assert_eq!(image.pixel_format, 21);
        assert!(image.is_raw_rgba());
        assert_eq!(
            image.decode_top_mip_rgba().unwrap(),
            vec![10, 20, 30, 40, 50, 60, 70, 80]
        );
    }

    #[test]
    fn reads_multiple_mips_with_block_headers() {
        let mut data = Vec::new();
        for value in [2u32, 4, 4, 2, 8, 21] {
            data.extend_from_slice(&value.to_be_bytes());
        }
        data.extend_from_slice(&64u32.to_be_bytes());
        data.extend_from_slice(&[7u8; 64]);
        data.extend_from_slice(&16u32.to_be_bytes());
        data.extend_from_slice(&[9u8; 16]);
        let image = RasterImage::parse(&data).unwrap();
        assert_eq!(image.mips.len(), 2);
        assert_eq!(image.mips[1], vec![9u8; 16]);
    }

    #[test]
    fn lot_mask_threshold_picks_cross_wired_colors() {
        // 字节序 B,G,R,A；映射 byte3→LotColor4、byte0→LotColor3、
        // byte1→LotColor2、byte2→LotColor1；阈值 ≥128。
        let colors = [[1, 1, 1], [2, 2, 2], [3, 3, 3], [4, 4, 4]];
        let data = raster(
            21,
            4,
            1,
            &[
                0, 0, 200, 0, // byte2=R=200 → LotColor1
                0, 150, 0, 0, // byte1=G=150 → LotColor2
                0, 0, 0, 255, // byte3=A=255 → LotColor4
                10, 10, 10, 10, // 全部低于阈值 → 透明
            ],
        );
        let image = RasterImage::parse(&data).unwrap();
        assert_eq!(
            image.decode_lot_mask_rgba(&colors).unwrap(),
            vec![
                1, 1, 1, 255, //
                2, 2, 2, 255, //
                4, 4, 4, 255, //
                0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn compressed_pixel_format_is_not_decodable() {
        let data = raster(71, 2, 1, &[0; 16]);
        let image = RasterImage::parse(&data).unwrap();
        assert!(!image.is_raw_rgba());
        assert!(matches!(
            image.decode_top_mip_rgba(),
            Err(Error::UnsupportedRasterPixelFormat(71))
        ));
        assert!(image.decode_lot_mask_rgba(&[[0; 3]; 4]).is_err());
    }

    #[test]
    fn truncated_mip_payload_is_rejected() {
        let data = raster(21, 4, 4, &[0; 10]);
        let image = RasterImage::parse(&data).unwrap();
        assert!(matches!(
            image.decode_top_mip_rgba(),
            Err(Error::InsufficientPayload { .. })
        ));
    }
}
