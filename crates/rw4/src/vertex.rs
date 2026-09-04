//! 顶点声明（VertexFormat 0x20004）与顶点组件解码。
//!
//! 对齐 C# `VertexFormat.Read` / `VertexComponentValueFactory` / `Vertex.Read`：
//! - 头部为**混合字节序**：3×LE u32 零、LE u16 组件数、**BE** u16 stride、
//!   **BE** u32 unknown2/3；
//! - 每个元素 12 字节：u8 unk1、**BE** u16 offset、**BE** u16 type、
//!   **BE** u16 usage、u8 index、u8 unk2、**BE** u16 unk3、u8 unk4；
//! - 组件值统一小端（f32/i16），D3DCOLOR 按 A,R,G,B 字节序读取；
//! - 工厂不支持的声明类型（UBYTE4N/SHORT2N/…/UNUSED）在 C# 中返回 null
//!   并被跳过，此处同样跳过。

use crate::error::{Error, Result};
use crate::reader::Reader;

/// DirectX 9 顶点声明类型（`D3DDECLTYPE`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationType {
    Float1,
    Float2,
    Float3,
    Float4,
    D3DColor,
    UByte4,
    Short2,
    Short4,
    UByte4N,
    Short2N,
    Short4N,
    UShort2N,
    UShort4N,
    UDec3,
    Dec3N,
    Float16_2,
    Float16_4,
    Unused,
}

impl DeclarationType {
    pub fn from_u16(v: u16) -> Option<DeclarationType> {
        Some(match v {
            0 => DeclarationType::Float1,
            1 => DeclarationType::Float2,
            2 => DeclarationType::Float3,
            3 => DeclarationType::Float4,
            4 => DeclarationType::D3DColor,
            5 => DeclarationType::UByte4,
            6 => DeclarationType::Short2,
            7 => DeclarationType::Short4,
            8 => DeclarationType::UByte4N,
            9 => DeclarationType::Short2N,
            10 => DeclarationType::Short4N,
            11 => DeclarationType::UShort2N,
            12 => DeclarationType::UShort4N,
            13 => DeclarationType::UDec3,
            14 => DeclarationType::Dec3N,
            15 => DeclarationType::Float16_2,
            16 => DeclarationType::Float16_4,
            17 => DeclarationType::Unused,
            _ => return None,
        })
    }

    /// 声明类型的字节宽度。
    pub fn size(self) -> usize {
        match self {
            DeclarationType::Float1
            | DeclarationType::D3DColor
            | DeclarationType::UByte4
            | DeclarationType::Short2
            | DeclarationType::UByte4N
            | DeclarationType::Short2N
            | DeclarationType::UShort2N
            | DeclarationType::UShort4N
            | DeclarationType::UDec3
            | DeclarationType::Dec3N
            | DeclarationType::Float16_2 => 4,
            DeclarationType::Float2
            | DeclarationType::Short4
            | DeclarationType::Short4N
            | DeclarationType::Float16_4 => 8,
            DeclarationType::Float3 => 12,
            DeclarationType::Float4 => 16,
            DeclarationType::Unused => 0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DeclarationType::Float1 => "FLOAT1",
            DeclarationType::Float2 => "FLOAT2",
            DeclarationType::Float3 => "FLOAT3",
            DeclarationType::Float4 => "FLOAT4",
            DeclarationType::D3DColor => "D3DCOLOR",
            DeclarationType::UByte4 => "UBYTE4",
            DeclarationType::Short2 => "SHORT2",
            DeclarationType::Short4 => "SHORT4",
            DeclarationType::UByte4N => "UBYTE4N",
            DeclarationType::Short2N => "SHORT2N",
            DeclarationType::Short4N => "SHORT4N",
            DeclarationType::UShort2N => "USHORT2N",
            DeclarationType::UShort4N => "USHORT4N",
            DeclarationType::UDec3 => "UDEC3",
            DeclarationType::Dec3N => "DEC3N",
            DeclarationType::Float16_2 => "FLOAT16_2",
            DeclarationType::Float16_4 => "FLOAT16_4",
            DeclarationType::Unused => "UNUSED",
        }
    }
}

/// DirectX 9 顶点声明用途（`D3DDECLUSAGE`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationUsage {
    Position,
    BlendWeight,
    BlendIndices,
    Normal,
    Psize,
    TexCoord,
    Tangent,
    Binormal,
    TessFactor,
    PositionT,
    Color,
    Fog,
    Depth,
    Sample,
}

impl DeclarationUsage {
    pub fn from_u16(v: u16) -> Option<DeclarationUsage> {
        Some(match v {
            0 => DeclarationUsage::Position,
            1 => DeclarationUsage::BlendWeight,
            2 => DeclarationUsage::BlendIndices,
            3 => DeclarationUsage::Normal,
            4 => DeclarationUsage::Psize,
            5 => DeclarationUsage::TexCoord,
            6 => DeclarationUsage::Tangent,
            7 => DeclarationUsage::Binormal,
            8 => DeclarationUsage::TessFactor,
            9 => DeclarationUsage::PositionT,
            10 => DeclarationUsage::Color,
            11 => DeclarationUsage::Fog,
            12 => DeclarationUsage::Depth,
            13 => DeclarationUsage::Sample,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            DeclarationUsage::Position => "POSITION",
            DeclarationUsage::BlendWeight => "BLENDWEIGHT",
            DeclarationUsage::BlendIndices => "BLENDINDICES",
            DeclarationUsage::Normal => "NORMAL",
            DeclarationUsage::Psize => "PSIZE",
            DeclarationUsage::TexCoord => "TEXCOORD",
            DeclarationUsage::Tangent => "TANGENT",
            DeclarationUsage::Binormal => "BINORMAL",
            DeclarationUsage::TessFactor => "TESSFACTOR",
            DeclarationUsage::PositionT => "POSITIONT",
            DeclarationUsage::Color => "COLOR",
            DeclarationUsage::Fog => "FOG",
            DeclarationUsage::Depth => "DEPTH",
            DeclarationUsage::Sample => "SAMPLE",
        }
    }
}

/// 一条顶点元素声明（12 字节，C# `VertexUsage`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexElement {
    pub unknown1: u8,
    pub offset: u16,
    pub decl_type: DeclarationType,
    pub usage: DeclarationUsage,
    pub index: u8,
    pub unknown2: u8,
    pub unknown3: u16,
    pub unknown4: u8,
}

/// 顶点格式（section 0x20004 的 payload）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexFormat {
    /// 顶点步长（C# `VertexSize`，来自 BE u16）。
    pub vertex_size: u16,
    pub unknown2: u32,
    pub unknown3: u32,
    pub elements: Vec<VertexElement>,
}

impl VertexFormat {
    pub(crate) fn parse(data: &[u8]) -> Result<VertexFormat> {
        let mut r = Reader::new(data);
        r.expect_u32(0, "VF001")?;
        r.expect_u32(0, "VF002")?;
        r.expect_u32(0, "VF003")?;
        let component_count = r.u16("VF_count")?;
        let vertex_size = r.u16be("VF_stride")?;
        let unknown2 = r.u32be("VF_unknown2")?;
        let unknown3 = r.u32be("VF_unknown3")?;

        let mut elements = Vec::with_capacity(component_count as usize);
        for _ in 0..component_count {
            let unknown1 = r.u8("VF_unk1")?;
            let offset = r.u16be("VF_offset")?;
            let decl_type = r.u16be("VF_type")?;
            let usage = r.u16be("VF_usage")?;
            let index = r.u8("VF_index")?;
            let unknown2 = r.u8("VF_unk2")?;
            let unknown3 = r.u16be("VF_unk3")?;
            let unknown4 = r.u8("VF_unk4")?;
            elements.push(VertexElement {
                unknown1,
                offset,
                decl_type: DeclarationType::from_u16(decl_type).ok_or(Error::UnexpectedValue {
                    check: "VF_type",
                    expected: 17,
                    actual: decl_type as u64,
                })?,
                usage: DeclarationUsage::from_u16(usage).ok_or(Error::UnexpectedValue {
                    check: "VF_usage",
                    expected: 13,
                    actual: usage as u64,
                })?,
                index,
                unknown2,
                unknown3,
                unknown4,
            });
        }
        Ok(VertexFormat {
            vertex_size,
            unknown2,
            unknown3,
            elements,
        })
    }
}

/// 已解码的顶点组件值（C# `IVertexComponentValue` 层级）。
#[derive(Debug, Clone, PartialEq)]
pub enum ComponentValue {
    Float1(f32),
    Float2([f32; 2]),
    Float3([f32; 3]),
    Float4([f32; 4]),
    UByte4([u8; 4]),
    D3DColor {
        a: u8,
        r: u8,
        g: u8,
        b: u8,
    },
    Short2([i16; 2]),
    Short4([i16; 4]),
    /// 归一化有符号短整型（原始 i16 / 32767，按 C# double 精度计算后转 f32）。
    Short4N([f32; 4]),
}

pub(crate) fn read_component(data: &[u8], decl: DeclarationType) -> Result<ComponentValue> {
    let mut r = Reader::new(data);
    Ok(match decl {
        DeclarationType::Float1 => ComponentValue::Float1(r.f32("comp")?),
        DeclarationType::Float2 => ComponentValue::Float2([r.f32("comp")?, r.f32("comp")?]),
        DeclarationType::Float3 => {
            ComponentValue::Float3([r.f32("comp")?, r.f32("comp")?, r.f32("comp")?])
        }
        DeclarationType::Float4 => ComponentValue::Float4([
            r.f32("comp")?,
            r.f32("comp")?,
            r.f32("comp")?,
            r.f32("comp")?,
        ]),
        DeclarationType::UByte4 => {
            ComponentValue::UByte4([r.u8("comp")?, r.u8("comp")?, r.u8("comp")?, r.u8("comp")?])
        }
        DeclarationType::D3DColor => ComponentValue::D3DColor {
            a: r.u8("comp")?,
            r: r.u8("comp")?,
            g: r.u8("comp")?,
            b: r.u8("comp")?,
        },
        DeclarationType::Short2 => ComponentValue::Short2([r.i16("comp")?, r.i16("comp")?]),
        DeclarationType::Short4 => ComponentValue::Short4([
            r.i16("comp")?,
            r.i16("comp")?,
            r.i16("comp")?,
            r.i16("comp")?,
        ]),
        DeclarationType::Short4N => {
            // C# 以 double 计算 i16/32767.0 后存储
            let norm = |raw: i16| ((raw as f64) / 32767.0) as f32;
            ComponentValue::Short4N([
                norm(r.i16("comp")?),
                norm(r.i16("comp")?),
                norm(r.i16("comp")?),
                norm(r.i16("comp")?),
            ])
        }
        // C# 工厂返回 null → 组件被跳过
        _ => return Err(Error::UnsupportedDeclarationType(decl as u16)),
    })
}

impl ComponentValue {
    /// UBYTE4 / SHORT4 的原始无符号四元组（关节索引未除 3）。
    pub fn as_quad_u(&self) -> Option<[u32; 4]> {
        match self {
            ComponentValue::UByte4(b) => Some([b[0] as u32, b[1] as u32, b[2] as u32, b[3] as u32]),
            ComponentValue::Short4(s) => Some([
                s[0] as u16 as u32,
                s[1] as u16 as u32,
                s[2] as u16 as u32,
                s[3] as u16 as u32,
            ]),
            _ => None,
        }
    }

    /// UBYTE4 / FLOAT4 / SHORT4N 的浮点四元组（蒙皮权重语义，C# `ReadQuadF`）。
    pub fn as_quad_f(&self) -> Option<[f32; 4]> {
        match self {
            ComponentValue::UByte4(b) => Some([
                b[0] as f32 / 255.0,
                b[1] as f32 / 255.0,
                b[2] as f32 / 255.0,
                b[3] as f32 / 255.0,
            ]),
            ComponentValue::Float4(f) => Some(*f),
            ComponentValue::Short4N(n) => Some(*n),
            _ => None,
        }
    }
}
