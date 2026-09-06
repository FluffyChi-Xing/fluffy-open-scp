//! Texture（0x20003）解析：贴图头 + Blob mip 链 + DXT1/DXT5 纯 CPU 解码。
//!
//! 对齐 C# `Texture.Read`（T000–T003）与 HANDOFF §4：
//! - 头部 32 字节：textureType、expect 8、unk1、width u16、height u16、
//!   mipmapInfo（高字节 = mip 数：0x708→7 级、0x808→8 级）、expect 0、
//!   expect 0、数据 section 编号；
//! - `textureType`：`0x31545844`('DXT1') / `0x35545844`('DXT5') 块压缩；
//!   `21` 为 raw BGRA 位图（调色板条等，无每 mip 尺寸前缀）；
//!   `116` = A32B32G32R32F 调色板条（4×f32/像素，游戏内合成用）；
//! - Blob 为 mip 链（DXT 每块 4×4 像素；顶层在前）。
//!
//! DXT 解码为标准 CPU 实现（参照 DDSLib 语义，无需 GPU / XNA）。

use crate::error::{Error, Result};
use crate::model::Rw4File;
use crate::reader::Reader;
use crate::section::SectionType;

/// 'DXT1' fourcc（LE 读取值）。
pub const TEXTURE_TYPE_DXT1: u32 = 0x3154_5844;
/// 'DXT5' fourcc（LE 读取值）。
pub const TEXTURE_TYPE_DXT5: u32 = 0x3554_5844;
/// raw BGRA 位图。
pub const TEXTURE_TYPE_RAW_BGRA: u32 = 21;
/// A32B32G32R32F 调色板条（4×f32/像素）。
pub const TEXTURE_TYPE_PALETTE_F32: u32 = 116;

/// 已解析的贴图 section（Blob 字节保留，顶层 mip 可随时解码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedTexture {
    pub texture_type: u32,
    pub unknown1: u32,
    pub width: u16,
    pub height: u16,
    /// 高字节为 mip 级数（C# 语义：`mipmapInfo / 0x100`）。
    pub mipmap_info: u32,
    pub data_section: u32,
    /// 数据 section（Blob）的完整 mip 链字节。
    pub blob: Vec<u8>,
}

impl DecodedTexture {
    /// mip 级数（`mipmap_info >> 8`）。
    pub fn mip_count(&self) -> u32 {
        self.mipmap_info >> 8
    }

    /// 贴图数据格式分类。
    pub fn format(&self) -> TextureFormat {
        match self.texture_type {
            TEXTURE_TYPE_DXT1 => TextureFormat::Dxt1,
            TEXTURE_TYPE_DXT5 => TextureFormat::Dxt5,
            TEXTURE_TYPE_RAW_BGRA | TEXTURE_TYPE_PALETTE_F32 => TextureFormat::Raw,
            _ => TextureFormat::Unknown(self.texture_type),
        }
    }

    /// 顶层 mip 的像素字节数。
    pub fn top_mip_bytes(&self) -> Option<usize> {
        let (w, h) = (u32::from(self.width), u32::from(self.height));
        match self.format() {
            TextureFormat::Dxt1 => Some((w.div_ceil(4) * h.div_ceil(4) * 8) as usize),
            TextureFormat::Dxt5 => Some((w.div_ceil(4) * h.div_ceil(4) * 16) as usize),
            TextureFormat::Raw => Some(w as usize * h as usize * 4),
            TextureFormat::Unknown(_) => None,
        }
    }

    /// 解码顶层 mip 为 RGBA8（raw 类型为 BGRA→RGBA 重排）。
    pub fn decode_top_mip_rgba(&self) -> Result<Vec<u8>> {
        let needed = self
            .top_mip_bytes()
            .ok_or(Error::UnsupportedTextureType(self.texture_type))?;
        if self.blob.len() < needed {
            return Err(Error::InsufficientPayload {
                check: "TX100",
                needed,
                actual: self.blob.len(),
            });
        }
        let (w, h) = (self.width, self.height);
        Ok(match self.format() {
            TextureFormat::Dxt1 => decode_dxt1(&self.blob[..needed], w, h),
            TextureFormat::Dxt5 => decode_dxt5(&self.blob[..needed], w, h),
            TextureFormat::Raw => {
                let mut rgba = Vec::with_capacity(needed);
                for px in self.blob[..needed].as_chunks::<4>().0 {
                    // C# ToImage 读取顺序为 b,g,r,a
                    rgba.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
                rgba
            }
            TextureFormat::Unknown(t) => return Err(Error::UnsupportedTextureType(t)),
        })
    }

    /// 解码 A32B32G32R32F 调色板条（textureType 116）为逐像素 4×f32，
    /// 按像素顺序（行主序：`index = y * width + x`）。列 = 材质元素，
    /// 行 = C# SCP 协议语义（row0 ColorBottom / row1 ColorTop / row2-3 UV 域）。
    pub fn decode_palette_f32(&self) -> Result<Vec<[f32; 4]>> {
        if self.texture_type != TEXTURE_TYPE_PALETTE_F32 {
            return Err(Error::UnsupportedTextureType(self.texture_type));
        }
        let pixels = u32::from(self.width) as usize * u32::from(self.height) as usize;
        let needed = pixels * 16;
        if self.blob.len() < needed {
            return Err(Error::InsufficientPayload {
                check: "TX110",
                needed,
                actual: self.blob.len(),
            });
        }
        Ok(self.blob[..needed]
            .as_chunks::<16>()
            .0
            .iter()
            .map(|px| {
                let f = |i: usize| f32::from_le_bytes(px[i * 4..i * 4 + 4].try_into().unwrap());
                [f(0), f(1), f(2), f(3)]
            })
            .collect())
    }

    /// 写出标准 DDS（magic + 128B 头 + 原始块压缩数据；C# `SaveDds` 同构）。
    /// raw 位图（textureType 21/116）不支持。
    pub fn write_dds(&self) -> Result<Vec<u8>> {        if self.format() == TextureFormat::Raw || matches!(self.format(), TextureFormat::Unknown(_))
        {
            return Err(Error::UnsupportedTextureType(self.texture_type));
        }
        let mut out = Vec::with_capacity(128 + self.blob.len());
        out.extend(0x2053_4444u32.to_le_bytes()); // 'DDS '
        out.extend(124u32.to_le_bytes()); // header size
        out.extend(0x000A_1007u32.to_le_bytes()); // flags
        out.extend(u32::from(self.height).to_le_bytes());
        out.extend(u32::from(self.width).to_le_bytes());
        out.extend((u32::from(self.height) * u32::from(self.width)).to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend(self.mip_count().to_le_bytes());
        out.extend([0u8; 44]); // 11 × u32 reserved
        // pixel format
        out.extend(32u32.to_le_bytes());
        out.extend(4u32.to_le_bytes()); // DDPF_FOURCC
        out.extend(self.texture_type.to_le_bytes());
        out.extend(32u32.to_le_bytes());
        out.extend(0x00ff_0000u32.to_le_bytes());
        out.extend(0x0000_ff00u32.to_le_bytes());
        out.extend(0x0000_00ffu32.to_le_bytes());
        out.extend(0xff00_0000u32.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend([0u8; 16]); // 4 × u32 reserved
        // C# `SaveDds` 到此为止（不写 caps），保持字节级一致
        out.extend_from_slice(&self.blob);
        Ok(out)
    }
}

/// 贴图数据格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat {
    Dxt1,
    Dxt5,
    Raw,
    Unknown(u32),
}

impl Rw4File {
    /// 解析一个贴图 section（含数据 section 的 Blob 字节）。
    pub fn decode_texture(&self, data: &[u8], number: u32) -> Result<DecodedTexture> {
        let section = self.section_for(number, "T000", SectionType::TEXTURE)?;
        let payload = self.payload(data, section.number)?;

        let mut r = Reader::new(payload);
        let texture_type = r.u32("T_type")?;
        r.expect_u32(8, "T001")?;
        let unknown1 = r.u32("T_unk1")?;
        let width = r.u16("T_width")?;
        let height = r.u16("T_height")?;
        let mipmap_info = r.u32("T_mipmap")?;
        r.expect_u32(0, "T002")?;
        r.expect_u32(0, "T003")?;
        let data_section = u32::try_from(r.i32("T_data_section")?).unwrap_or(u32::MAX);

        let blob_section = self
            .section(data_section)
            .ok_or(Error::SectionNumberOutOfRange {
                number: data_section,
                count: self.sections().len() as u32,
            })?;
        if blob_section.type_code != SectionType::BLOB {
            return Err(Error::BadSectionType {
                check: "TX000",
                number: data_section,
                expected: SectionType::BLOB,
                actual: blob_section.type_code,
            });
        }
        let blob = self.payload(data, data_section)?.to_vec();

        Ok(DecodedTexture {
            texture_type,
            unknown1,
            width,
            height,
            mipmap_info,
            data_section,
            blob,
        })
    }
}

/// 解码 DXT1 块压缩数据为 RGBA8。
pub fn decode_dxt1(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    decode_dxt(data, width, height, false)
}

/// 解码 DXT5 块压缩数据为 RGBA8。
pub fn decode_dxt5(data: &[u8], width: u16, height: u16) -> Vec<u8> {
    decode_dxt(data, width, height, true)
}

fn decode_dxt(data: &[u8], width: u16, height: u16, dxt5: bool) -> Vec<u8> {
    let (w, h) = (u32::from(width) as usize, u32::from(height) as usize);
    let bw = w.div_ceil(4);
    let bh = h.div_ceil(4);
    let block_size = if dxt5 { 16 } else { 8 };
    let mut out = vec![0u8; w * h * 4];

    for by in 0..bh {
        for bx in 0..bw {
            let base = (by * bw + bx) * block_size;
            if base + block_size > data.len() {
                break;
            }
            let block = &data[base..base + block_size];

            // 颜色端点（DXT5 时颜色块在后 8 字节）
            let color = if dxt5 { &block[8..16] } else { block };
            let c0 = u16::from_le_bytes([color[0], color[1]]);
            let c1 = u16::from_le_bytes([color[2], color[3]]);
            let palette = color_palette(c0, c1, !dxt5 && c0 <= c1);
            let indices = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);

            // alpha
            let alphas: [u8; 16] = if dxt5 {
                alpha_palette(block[0], block[1], &block[2..8])
            } else {
                [255; 16]
            };

            for py in 0..4 {
                for px in 0..4 {
                    let x = bx * 4 + px;
                    let y = by * 4 + py;
                    if x >= w || y >= h {
                        continue;
                    }
                    let idx = py * 4 + px;
                    let ci = ((indices >> (idx * 2)) & 0b11) as usize;
                    let o = (y * w + x) * 4;
                    out[o..o + 4].copy_from_slice(&[
                        palette[ci][0],
                        palette[ci][1],
                        palette[ci][2],
                        alphas[idx],
                    ]);
                }
            }
        }
    }
    out
}

/// DXT 颜色端点插值表（`punchthrough` = DXT1 三色 + 透明模式）。
fn color_palette(c0: u16, c1: u16, punchthrough: bool) -> [[u8; 3]; 4] {
    let expand = |c: u16| -> (u8, u8, u8) {
        let r5 = (c >> 11) & 0x1f;
        let g6 = (c >> 5) & 0x3f;
        let b5 = c & 0x1f;
        (
            ((r5 * 255 + 15) / 31) as u8,
            ((g6 * 255 + 31) / 63) as u8,
            ((b5 * 255 + 15) / 31) as u8,
        )
    };
    let (r0, g0, b0) = expand(c0);
    let (r1, g1, b1) = expand(c1);
    if punchthrough {
        let avg = |a: u8, b: u8| ((u16::from(a) + u16::from(b)) / 2) as u8;
        [
            [r0, g0, b0],
            [r1, g1, b1],
            [avg(r0, r1), avg(g0, g1), avg(b0, b1)],
            [0, 0, 0],
        ]
    } else {
        let two_thirds = |a: u8, b: u8| ((2 * u16::from(a) + u16::from(b)) / 3) as u8;
        [
            [r0, g0, b0],
            [r1, g1, b1],
            [two_thirds(r0, r1), two_thirds(g0, g1), two_thirds(b0, b1)],
            [two_thirds(r1, r0), two_thirds(g1, g0), two_thirds(b1, b0)],
        ]
    }
}

/// DXT5 alpha 插值表（8 值 / 6 值模式 + 3bit 索引）。
fn alpha_palette(a0: u8, a1: u8, indices6: &[u8]) -> [u8; 16] {
    let mut table = [0u8; 8];
    table[0] = a0;
    table[1] = a1;
    if a0 > a1 {
        let (a0, a1) = (u16::from(a0), u16::from(a1));
        table[2] = ((6 * a0 + a1) / 7) as u8;
        table[3] = ((5 * a0 + 2 * a1) / 7) as u8;
        table[4] = ((4 * a0 + 3 * a1) / 7) as u8;
        table[5] = ((3 * a0 + 4 * a1) / 7) as u8;
        table[6] = ((2 * a0 + 5 * a1) / 7) as u8;
        table[7] = ((a0 + 6 * a1) / 7) as u8;
    } else {
        let (a0, a1) = (u16::from(a0), u16::from(a1));
        table[2] = ((4 * a0 + a1) / 5) as u8;
        table[3] = ((3 * a0 + 2 * a1) / 5) as u8;
        table[4] = ((2 * a0 + 3 * a1) / 5) as u8;
        table[5] = ((a0 + 4 * a1) / 5) as u8;
        table[6] = 0;
        table[7] = 255;
    }
    let bits = u64::from_le_bytes([
        indices6[0],
        indices6[1],
        indices6[2],
        indices6[3],
        indices6[4],
        indices6[5],
        0,
        0,
    ]);
    let mut out = [0u8; 16];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = table[((bits >> (i * 3)) & 0b111) as usize];
    }
    out
}
