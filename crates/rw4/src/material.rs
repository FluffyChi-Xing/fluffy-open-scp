//! RW4Material（0x2000B）解析：材质头 + 槽位纹理引用。
//!
//! 对齐 C# `RW4Material.Read`：`Size` u32 → 28B Header →（若模型含
//! VertexFormat section）顶点格式副本 → 扫描 `0x2D` shader-def 标记
//! （绝不越过 section 末尾，C# 防挂起补丁）→ AdditionalData → 固定
//! 6×24B `MaterialTextureReference`（slot/unk2/**TextureInstanceId**/unk3/unk4/unk5）
//! → 剩余 Data。
//!
//! 槽位语义（HANDOFF §4）：引用 `slot == 0x2D` 是 shader-def 槽；
//! `0..=4` 为纹理槽（0 调色板 / 1 区域遮罩 / 2 法线 / 3 副遮罩）。
//! 纹理本体在**独立资源**中（instance id → 0x2F4E681B RW4 包裹 /
//! 0x2F4E681C 光栅），不追 SimCity 未用的 0x2001a 链路。
//!
//! 布局无法解析（找不到 0x2D 标记等）时与 C# 一致回退 `Raw`：
//! 保留整个 section 原样，让模型其余部分继续可导出。

use crate::error::Result;
use crate::model::Rw4File;
use crate::reader::Reader;
use crate::section::SectionType;
use crate::vertex::VertexFormat;

/// shader-def 槽位标记（`slot == 0x2D`）。
pub const SHADER_DEF_MARKER: u32 = 0x2D;

/// 一条材质纹理引用（24 字节）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureSlotRef {
    /// 槽位原始值：低字节为槽编号（0..=4 纹理槽，0x2D shader-def），
    /// 高位含标志（真实数据中出现 0x100/0x400/0x3F800000 等）。
    pub slot: u32,
    pub unknown2: u32,
    /// 纹理资源 instance id（跨资源引用）。
    pub texture_instance: u32,
    pub unknown3: u32,
    pub unknown4: u32,
    pub unknown5: u32,
}

impl TextureSlotRef {
    /// 槽编号（`slot` 低字节，HANDOFF §4 语义）。
    pub fn slot_byte(&self) -> u8 {
        (self.slot & 0xFF) as u8
    }
}

/// 已解析的材质 section。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMaterial {
    /// section 声明的总大小（== section size）。
    pub size: u32,
    /// 28 字节材质头（原样保留）。
    pub header: [u8; 28],
    /// 材质内嵌的顶点格式副本（模型含 VertexFormat section 时存在）。
    pub vertex_format_data: Vec<u8>,
    /// `0x2D` 标记之前的附加数据。
    pub additional_data: Vec<u8>,
    /// 固定 6 条纹理引用。
    pub texture_refs: [TextureSlotRef; 6],
    /// 引用表之后的剩余数据。
    pub data: Vec<u8>,
}

impl DecodedMaterial {
    /// 迭代纹理槽引用（跳过 shader-def 槽）。
    pub fn texture_slots(&self) -> impl Iterator<Item = &TextureSlotRef> {
        self.texture_refs
            .iter()
            .filter(|r| r.slot != SHADER_DEF_MARKER)
    }

    /// 取指定槽位的纹理 instance id（0 调色板 / 1 遮罩 / 2 法线 / 3 副遮罩）。
    pub fn slot_texture(&self, slot: u32) -> Option<u32> {
        self.texture_slots()
            .find(|r| r.slot == slot)
            .map(|r| r.texture_instance)
    }
}

/// 材质 section 解析结果：可解析，或整体回退为原样字节（C# `_rawSection`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaterialSection {
    Decoded(Box<DecodedMaterial>),
    /// 布局不认识的材质：整段保留，纹理解析对其返回空引用（C# 回退行为）。
    Raw(Vec<u8>),
}

impl MaterialSection {
    /// 已解析材质的纹理槽引用；Raw 材质返回空。
    pub fn texture_refs(&self) -> &[TextureSlotRef] {
        match self {
            MaterialSection::Decoded(m) => &m.texture_refs,
            MaterialSection::Raw(_) => &[],
        }
    }
}

impl Rw4File {
    /// 解码一个材质 section。
    pub fn decode_material(&self, data: &[u8], number: u32) -> Result<MaterialSection> {
        let section = self.section_for(number, "MA000", SectionType::MATERIAL)?;
        let payload = self.payload(data, section.number)?;
        // C#：仅当模型存在 VertexFormat section 时读取其 ComputeSize()
        //（= 24 + 12×组件数）大小的副本
        let vertex_format_size = self
            .sections_of_type(SectionType::VERTEX_FORMAT)
            .next()
            .and_then(|vf| self.payload(data, vf.number).ok())
            .and_then(|payload| VertexFormat::parse(payload).ok())
            .map(|f| 24 + 12 * f.elements.len());
        Ok(parse_material(payload, vertex_format_size))
    }
}

pub(crate) fn parse_material(payload: &[u8], vertex_format_size: Option<usize>) -> MaterialSection {
    let size = payload
        .get(0..4)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map(u32::from_le_bytes)
        .unwrap_or(0);
    match decode_material_inner(payload, size, vertex_format_size) {
        Ok(material) => MaterialSection::Decoded(Box::new(material)),
        Err(()) => MaterialSection::Raw(payload.to_vec()),
    }
}

type RawResult<T> = std::result::Result<T, ()>;

fn decode_material_inner(
    payload: &[u8],
    size: u32,
    vertex_format_size: Option<usize>,
) -> RawResult<DecodedMaterial> {
    let mut r = Reader::new(payload);
    let _ = r.u32("MA_size").map_err(drop)?;
    let mut header = [0u8; 28];
    for byte in &mut header {
        *byte = r.u8("MA_header").map_err(drop)?;
    }
    let vertex_format_data = match vertex_format_size {
        Some(n) => r.take(n, "MA_vf").map_err(drop)?.to_vec(),
        None => Vec::new(),
    };

    // 扫描 0x2D 标记，绝不越过 section 末尾（C# 防挂起补丁）
    let scan_start = r.pos();
    let mut found = false;
    while r.pos() + 4 <= payload.len() {
        if r.u32("MA_scan").map_err(drop)? == SHADER_DEF_MARKER {
            found = true;
            break;
        }
    }
    if !found {
        return Err(());
    }
    let additional_data = payload[scan_start..r.pos() - 4].to_vec();

    let mut texture_refs = [const {
        TextureSlotRef {
            slot: 0,
            unknown2: 0,
            texture_instance: 0,
            unknown3: 0,
            unknown4: 0,
            unknown5: 0,
        }
    }; 6];
    for reference in &mut texture_refs {
        reference.slot = r.u32("MA_slot").map_err(drop)?;
        reference.unknown2 = r.u32("MA_unk2").map_err(drop)?;
        reference.texture_instance = r.u32("MA_instance").map_err(drop)?;
        reference.unknown3 = r.u32("MA_unk3").map_err(drop)?;
        reference.unknown4 = r.u32("MA_unk4").map_err(drop)?;
        reference.unknown5 = r.u32("MA_unk5").map_err(drop)?;
    }
    let data = payload[r.pos().min(payload.len())..].to_vec();
    Ok(DecodedMaterial {
        size,
        header,
        vertex_format_data,
        additional_data,
        texture_refs,
        data,
    })
}
