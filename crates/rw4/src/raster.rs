//! Raster（独立资源类型 `0x2F4E681C`）解析：D3D8/9 风格的纹理容器。
//!
//! 对齐 C# `Views/viewHelpers/RasterImage.cs` 与 HANDOFF §4：
//! - 头部 6 × u32（大端）：rasterType、width、height、mipCount、pixelSize、
//!   pixelFormat；
//! - 随后逐 mip：`[u32 blockSize][载荷]`，宽高逐级减半；
//! - `pixelFormat == 21` = D3DFMT_A8R8G8B8：未压缩 32bit 纹理，D3D9 内存
//!   布局为 B,G,R,A，解码时重排为 R,G,B,A（2026-09-08 修正，与 RW4 内嵌
//!   raw 纹理一致；游戏渲染器源码证实 raster 经 D3D 管线按 BGRA 采样）；
//!   DXT 压缩变体（pixFmt != 21）C# CLI 同样未实现，仅保留元数据。

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
    /// pixFmt 21 = D3DFMT_A8R8G8B8：D3D9 内存布局为 B,G,R,A（与 RW4 内嵌
    /// raw 纹理一致），这里重排为 R,G,B,A。2026-09-08 修正：此前按顺序
    /// R,G,B,A 直读，导致全仓 raster 颜色 R/B 反（法线图平坦区呈粉色、
    /// tint 亮度/U 权重通道互换）。旧对拍只验证了 alpha（两种解读同在
    /// byte3），不能区分 R/B。
    pub fn decode_top_mip_rgba(&self) -> Result<Vec<u8>> {
        if !self.is_raw_rgba() {
            return Err(Error::UnsupportedRasterPixelFormat(self.pixel_format));
        }
        let mip = self.top_mip_bytes()?;
        let mut out = mip.to_vec();
        for px in out.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
        }
        Ok(out)
    }

    /// LotMask 四层量化（复刻 SCP `ViewLotEditor` 的 `RasterChannel.Preview`）：
    /// 每像素 RGBA 各对应一层，阈值 ≥128 选中该层颜色并输出不透明像素，
    /// 全部未选中输出全透明。通道→颜色：R→LotColor1、G→LotColor2、
    /// B→LotColor3、A→LotColor4（优先级 A > R > G > B，与旧交叉映射逐字节
    /// 等价——旧版在 BGRA 原始字节上交叉，等价于重排后直读）。
    pub fn decode_lot_mask_rgba(&self, colors: &[[u8; 4]; 4]) -> Result<Vec<u8>> {
        let rgba = self.decode_top_mip_rgba()?;
        const CHANNEL_TO_COLOR: [(usize, usize); 4] = [(3, 3), (0, 0), (1, 1), (2, 2)];
        let mut out = Vec::with_capacity(rgba.len());
        for px in rgba.as_chunks::<4>().0 {
            let chosen = CHANNEL_TO_COLOR
                .iter()
                .find(|(channel, _)| px[*channel] >= 128)
                .map(|(_, color_index)| colors[*color_index]);
            match chosen {
                Some([r, g, b, _]) => out.extend_from_slice(&[r, g, b, 255]),
                None => out.extend_from_slice(&[0, 0, 0, 0]),
            }
        }
        Ok(out)
    }
}

// SimCity 法线图编码说明（2026-09-08 源码实证）：slot2 法线为标准切线
// 空间 RGB（B = 沿顶点法线轴，平坦 ≈ 128,128,255），游戏侧用法为
// `rgb * 2 - 0.9985`（building4DefaultPS `ApplyNormalMap`），alpha 含
// specular。此前存在 `unswizzle_simcity_normal`（R↔B 对调）以补偿 raster
// 的错误直读——decode_top_mip_rgba 恢复 BGRA 重排后不再需要，已删除。
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
    fn parses_header_and_reorders_bgra() {
        // pixFmt21 = D3DFMT_A8R8G8B8：内存 B,G,R,A → 重排为 R,G,B,A。
        let data = raster(21, 2, 1, &[10, 20, 30, 40, 50, 60, 70, 80]);
        let image = RasterImage::parse(&data).unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.mip_count, 1);
        assert_eq!(image.pixel_format, 21);
        assert!(image.is_raw_rgba());
        assert_eq!(
            image.decode_top_mip_rgba().unwrap(),
            vec![30, 20, 10, 40, 70, 60, 50, 80]
        );
    }

    #[test]
    fn normal_map_flat_region_decodes_blue_up() {
        // 法线图平坦区原始字节呈粉色（内存 BGRA：B=255 在前）→ 解码后为
        // 标准蓝 up (128,128,255)，与游戏 ApplyNormalMap 的 B=沿法线轴一致。
        let data = raster(21, 1, 1, &[255, 128, 128, 249]);
        let image = RasterImage::parse(&data).unwrap();
        assert_eq!(
            image.decode_top_mip_rgba().unwrap(),
            vec![128, 128, 255, 249]
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
        let colors = [[1, 1, 1, 0], [2, 2, 2, 0], [3, 3, 3, 0], [4, 4, 4, 0]];
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
        assert!(image.decode_lot_mask_rgba(&[[0; 4]; 4]).is_err());
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
