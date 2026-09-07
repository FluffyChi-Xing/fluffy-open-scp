//! RW4Material（0x2000B）解析：材质头 + 引用记录表。
//!
//! 布局（EP1 0x63D180B9 逐字节核对 + 全包统计验证，2026-09-07）：
//! `Size` u32 → 28B Header →（若模型含 VertexFormat section）顶点格式副本
//! → AdditionalData → **7×24B 引用记录** → 剩余 Data。记录格式
//! `(slot, unk, instance, unk, unk, unk)`：第 0 条 `slot == 0x2D` 是
//! shader-def 引用，随后 slot `0..=5` 为纹理槽（0 调色板 / 1 区域遮罩 /
//! 2 法线 / 3 副遮罩 / 4、5 补充槽），`instance` 是**外部资源** instance id
//! （0x2F4E681B RW4 包裹 / 0x2F4E681C 光栅，常在其它包中）。
//!
//! C# `RW4Material.Read` 把 0x2D 记录当作要丢弃的"标记"，之后只读 6 条
//! ——整体错位一条记录（其 `TextureInstanceId` 读到的是杂讯），下游只能
//! 靠启发式猜贴图。本实现保留 0x2D 记录并连续读取 7 条；slot 序列
//! （0x2D, 0..=5）校验失败时与 C# 一致回退 `Raw`（EP1 实测 4654/4703
//! 材质符合，余者布局异常）。

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
    /// 引用表之前的附加数据。
    pub additional_data: Vec<u8>,
    /// 引用记录：第 0 条为 shader-def（slot 0x2D），随后 slot 0..=5 纹理槽。
    pub texture_refs: Vec<TextureSlotRef>,
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

    /// 解码全部 MeshMaterialAssignment（0x2001A）——mesh→material 绑定表。
    ///
    /// 12 字节 = `mesh_section 号 u32 + u32（恒 1，语义待定）+
    /// material_section 号 u32`（migration.md §21.1，EP1 0x41B1BAC0 实测：
    /// `#13: [0B,01,0C]` 绑定 mesh#11↔material#12）。C# `RW4Model.cs`
    /// 误判为 Spore 遗产而注释禁用，实际 SimCity 文件普遍存在，且
    /// mesh 数 == material 数（可解 + Raw）。损坏条目跳过。
    pub fn decode_mesh_material_bindings(&self, data: &[u8]) -> Vec<MeshMaterialBinding> {
        self.sections_of_type(SectionType::MESH_MATERIAL_ASSIGNMENT)
            .filter_map(|section| {
                let payload = self.payload(data, section.number).ok()?;
                if payload.len() < 12 {
                    return None;
                }
                let word = |offset: usize| -> Option<u32> {
                    Some(u32::from_le_bytes(
                        payload.get(offset..offset + 4)?.try_into().ok()?,
                    ))
                };
                Some(MeshMaterialBinding {
                    mesh_section: word(0)?,
                    unknown2: word(4)?,
                    material_section: word(8)?,
                })
            })
            .collect()
    }
}

/// 一条 mesh→material 绑定（MeshMaterialAssignment 0x2001A）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshMaterialBinding {
    /// 绑定的 MESH section 号。
    pub mesh_section: u32,
    /// 恒 1（语义待定，原样保留）。
    pub unknown2: u32,
    /// 绑定的 MATERIAL section 号。
    pub material_section: u32,
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
    // 0x2D 位于引用表第 0 条记录的 slot 字段上，扫描已消费它——回退后随
    // 记录表一起读取（C# 未回退，导致整表错位一条记录）。
    r.seek(r.pos() - 4).map_err(drop)?;

    // 0x2D 是第 0 条记录的 slot（shader-def 引用），不丢弃；
    // 其后固定 6 条纹理记录（slot 0..=5）。C# 从标记后读 6 条导致错位。
    let mut texture_refs = Vec::with_capacity(7);
    for _ in 0..7 {
        let record = TextureSlotRef {
            slot: r.u32("MA_slot").map_err(drop)?,
            unknown2: r.u32("MA_unk2").map_err(drop)?,
            texture_instance: r.u32("MA_instance").map_err(drop)?,
            unknown3: r.u32("MA_unk3").map_err(drop)?,
            unknown4: r.u32("MA_unk4").map_err(drop)?,
            unknown5: r.u32("MA_unk5").map_err(drop)?,
        };
        texture_refs.push(record);
    }
    if texture_refs[0].slot != SHADER_DEF_MARKER
        || texture_refs[1..]
            .iter()
            .enumerate()
            .any(|(i, r)| r.slot != i as u32)
    {
        return Err(());
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
