//! Mesh（0x20009）/ TriangleArray（0x20007）/ VertexArray（0x20005）解析
//! 与跨 section 的网格解码。
//!
//! 对齐 C# `RW4Mesh.Read`（ME001–ME200）、`RW4TriangleArray.Read`（TA000–TA010）、
//! `RW4VertexArray.Read`（VA000–VA101）、`Buffer<Triangle>`/`VertexBuffer`（Blob
//! 0x10030）与 `Vertex.Read`（按 offset 排序组件、stride 取自 VertexFormat）。
//!
//! 特例：mesh 的 `vertex_section == 0x400000` 表示无顶点缓冲
//! （blend-shape 网格），C# 跳过顶点解析，此处 `DecodedMesh.vertices` 为空。

use crate::error::{Error, Result};
use crate::model::Rw4File;
use crate::reader::Reader;
use crate::section::{Section, SectionType};
use crate::vertex::{
    ComponentValue, DeclarationUsage, VertexElement, VertexFormat, read_component,
};

/// mesh 头部中「无顶点缓冲」的哨兵 section 编号。
pub const NO_VERTEX_SECTION: u32 = 0x40_0000;

/// Mesh section 的 10×u32 头。
///
/// 2026-09-07 重解读（migration.md §23，EP1 0x41B1BAC0 双变体逐字节取证）：
/// 变体网格**共享** TriangleArray/VertexArray 池，用 `[start_index, count)`
/// 切片；旧解析把 `[5]` 当恒 0、把 `[6][7]` 当 u64，仅在"独占池"的静态
/// 网格（start=0、[7]=0）上碰巧成立——C# 同样如此（其 ME004 期望 0）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshHeader {
    pub tri_section: u32,
    /// 本 mesh 的三角形数（= index_count / 3）。
    pub triangle_count: u32,
    /// 在共享 TA 索引 blob 中的起始索引（u16 个）。
    pub start_index: u32,
    /// 本 mesh 的索引总数（= triangle_count × 3）。
    pub index_count: u32,
    /// 顶点池中本 mesh 用到的最小索引（信息性，游戏 draw-range 优化）。
    pub min_vertex_index: u32,
    pub vertex_count: u32,
    pub vertex_section: u32,
}

impl MeshHeader {
    /// blend-shape 网格没有静态顶点缓冲。
    pub fn has_vertex_data(&self) -> bool {
        self.vertex_section != NO_VERTEX_SECTION
    }
}

/// TriangleArray section 头（7×u32，有效字段 3 个）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriangleArrayHeader {
    pub unknown1: u32,
    pub index_count: u32,
    pub data_section: u32,
}

/// VertexArray section 头（7×u32，有效字段 5 个）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexArrayHeader {
    pub format_section: u32,
    pub unknown2: u32,
    pub vertex_count: u32,
    pub vertex_size: u32,
    pub data_section: u32,
}

/// 一个已解码顶点：按声明 offset 排序的（元素，值）对。
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedVertex {
    pub components: Vec<(VertexElement, ComponentValue)>,
}

impl DecodedVertex {
    /// POSITION FLOAT3。
    pub fn position(&self) -> Option<[f32; 3]> {
        match self.find_usage(DeclarationUsage::Position)? {
            ComponentValue::Float3(p) => Some(*p),
            _ => None,
        }
    }

    /// NORMAL：UBYTE4 按 C# 语义解包 `(b - 127.5) / 127.5`。
    pub fn normal(&self) -> Option<[f32; 3]> {
        match self.find_usage(DeclarationUsage::Normal)? {
            ComponentValue::UByte4(b) => {
                let n = |x: u8| (f32::from(x) - 127.5) / 127.5;
                Some([n(b[0]), n(b[1]), n(b[2])])
            }
            _ => None,
        }
    }

    /// 导出用 UV：第一个 FLOAT2 TEXCOORD（常规模型），否则第一个 FLOAT4（facade）。
    /// 与 C# `Vertex.TryGetUV` 一致，不区分 usage index。
    pub fn uv(&self) -> Option<[f32; 2]> {
        let mut float4 = None;
        for (element, value) in &self.components {
            if element.usage != DeclarationUsage::TexCoord {
                continue;
            }
            match value {
                ComponentValue::Float2(uv) => return Some(*uv),
                ComponentValue::Float4(f) if float4.is_none() => {
                    float4 = Some([f[0], f[1]]);
                }
                _ => {}
            }
        }
        float4
    }

    /// BLENDINDICES 原始值（未除 3；UBYTE4 或 SHORT4）。
    pub fn blend_indices_raw(&self) -> Option<[u32; 4]> {
        self.find_usage(DeclarationUsage::BlendIndices)?.as_quad_u()
    }

    /// 关节索引：原始值 ÷3，超出 `joint_count` 的钳到 0（C# `ReadQuadU`）。
    pub fn blend_indices(&self, joint_count: u32) -> Option<[u16; 4]> {
        let raw = self.blend_indices_raw()?;
        let mut out = [0u16; 4];
        for (k, v) in raw.into_iter().enumerate() {
            let mut j = (v / 3) as u16;
            if u32::from(j) >= joint_count {
                j = 0;
            }
            out[k] = j;
        }
        Some(out)
    }

    /// BLENDWEIGHT（UBYTE4 /255、FLOAT4 或 SHORT4N，C# `ReadQuadF`）。
    pub fn blend_weights(&self) -> Option<[f32; 4]> {
        self.find_usage(DeclarationUsage::BlendWeight)?.as_quad_f()
    }

    /// shell-rig 刚体蒙皮标记：TEXCOORD **index 1** 的 UBYTE4 值
    ///（HANDOFF §4：`(127,127,127,0)` = 静态外壳，其余为运动顶点分组键）。
    pub fn shell_marker(&self) -> Option<[u8; 4]> {
        self.components
            .iter()
            .find(|(e, _)| e.usage == DeclarationUsage::TexCoord && e.index == 1)
            .and_then(|(_, v)| match v {
                ComponentValue::UByte4(b) => Some(*b),
                _ => None,
            })
    }

    /// 是否带 FLOAT2 纹理坐标（真 UV；FLOAT4 世界投影坐标不算）。
    pub fn has_float2_uv(&self) -> bool {
        self.components
            .iter()
            .any(|(e, v)| e.usage == DeclarationUsage::TexCoord && matches!(v, ComponentValue::Float2(_)))
    }

    /// D3DCOLOR 的 G 通道（调色板列号，C# AdvancedCollada 协议）。
    pub fn d3d_color_g(&self) -> Option<u8> {
        self.components.iter().find_map(|(_, v)| match v {
            ComponentValue::D3DColor { g, .. } => Some(*g),
            _ => None,
        })
    }

    /// 是否为 shell-rig 静态外壳顶点（标记 `(127,127,127,0)`）。
    pub fn is_rigid_shell_static(&self) -> bool {
        self.shell_marker() == Some([127, 127, 127, 0])
    }

    fn find_usage(&self, usage: DeclarationUsage) -> Option<&ComponentValue> {
        self.components
            .iter()
            .find(|(e, _)| e.usage == usage)
            .map(|(_, v)| v)
    }
}

/// 已解码的完整网格（mesh 头 + 三角形 + 顶点）。
#[derive(Debug, Clone)]
pub struct DecodedMesh {
    pub number: u32,
    pub header: MeshHeader,
    pub triangles: Vec<[u16; 3]>,
    pub vertices: Vec<DecodedVertex>,
}

impl DecodedMesh {
    /// NRE 防御对齐（C# `ExportRw4Bytes` 过滤 `vertices == null ||
    /// triangles == null` 的 mesh）：有三角形且有顶点数据才可导出。
    /// blend-shape mesh（`0x400000` 哨兵）在此返回 `false`。
    pub fn is_exportable(&self) -> bool {
        !self.triangles.is_empty() && !self.vertices.is_empty()
    }
}

impl Rw4File {
    /// 解析并解码一个 mesh 及其引用的 triangle/vertex/vertex-format/blob sections。
    ///
    /// `data` 是整份 RW4 文件字节。无顶点缓冲的 mesh（blend-shape）返回空顶点。
    pub fn decode_mesh(&self, data: &[u8], mesh_number: u32) -> Result<DecodedMesh> {
        let _mesh_section = self.section_for(mesh_number, "ME000", SectionType::MESH)?;
        let payload = self.payload(data, mesh_number)?;
        let header = parse_mesh_header(payload)?;

        let mut triangles = self.decode_triangles(
            data,
            header.tri_section,
            header.start_index,
            header.index_count,
        )?;
        let vertices = if header.has_vertex_data() {
            // 变体网格与其它 mesh 共享顶点池：解码整池后按本 mesh 索引重映射
            let pool = self.decode_vertices(data, header.vertex_section)?;
            let mut remap = vec![u32::MAX; pool.len()];
            let mut vertices: Vec<DecodedVertex> = Vec::new();
            let mut remapped = triangles.clone();
            for tri in &mut remapped {
                for k in 0..3 {
                    let pool_index = tri[k] as usize;
                    let Some(entry) = pool.get(pool_index) else {
                        return Err(Error::UnexpectedValue {
                            check: "ME300",
                            expected: u64::from(header.vertex_count),
                            actual: u64::from(tri[k]),
                        });
                    };
                    if remap[pool_index] == u32::MAX {
                        remap[pool_index] = vertices.len() as u32;
                        vertices.push(entry.clone());
                    }
                    tri[k] = remap[pool_index] as u16;
                }
            }
            triangles = remapped;
            vertices
        } else {
            Vec::new()
        };

        Ok(DecodedMesh {
            number: mesh_number,
            header,
            triangles,
            vertices,
        })
    }

    fn decode_triangles(
        &self,
        data: &[u8],
        tri_section: u32,
        start_index: u32,
        index_count: u32,
    ) -> Result<Vec<[u16; 3]>> {
        self.section_for(tri_section, "TA000", SectionType::TRIANGLE_ARRAY)?;
        let header = parse_triangle_array_header(self.payload(data, tri_section)?)?;

        if header.index_count % 3 != 0 {
            return Err(Error::UnexpectedValue {
                check: "TA010",
                expected: 0,
                actual: u64::from(header.index_count % 3),
            });
        }
        // 变体网格共享 TA：本 mesh 只取 [start_index, start_index+count) 切片
        if start_index + index_count > header.index_count {
            return Err(Error::UnexpectedValue {
                check: "ME100",
                expected: u64::from(header.index_count),
                actual: u64::from(start_index + index_count),
            });
        }

        let blob = self.blob_payload(data, header.data_section, "TA100")?;
        let byte_from = start_index as usize * 2;
        let needed = index_count as usize * 2;
        if blob.len() < byte_from + needed {
            return Err(Error::InsufficientPayload {
                check: "TA101",
                needed: byte_from + needed,
                actual: blob.len(),
            });
        }
        let mut triangles = Vec::with_capacity(index_count as usize / 3);
        for tri in blob[byte_from..byte_from + needed].as_chunks::<6>().0 {
            triangles.push([
                u16::from_le_bytes([tri[0], tri[1]]),
                u16::from_le_bytes([tri[2], tri[3]]),
                u16::from_le_bytes([tri[4], tri[5]]),
            ]);
        }
        Ok(triangles)
    }

    fn decode_vertices(&self, data: &[u8], vertex_section: u32) -> Result<Vec<DecodedVertex>> {
        self.section_for(vertex_section, "VA000", SectionType::VERTEX_ARRAY)?;
        let header = parse_vertex_array_header(self.payload(data, vertex_section)?)?;

        let fmt_section =
            self.section_for(header.format_section, "VA101", SectionType::VERTEX_FORMAT)?;
        let format = VertexFormat::parse(self.payload(data, fmt_section.number)?)?;

        let blob = self.blob_payload(data, header.data_section, "VA100")?;
        let stride = format.vertex_size as usize;
        let needed = header.vertex_count as usize * stride;
        if blob.len() < needed {
            return Err(Error::InsufficientPayload {
                check: "VA102",
                needed,
                actual: blob.len(),
            });
        }

        // 组件按 offset 排序（C# OrderBy(Offset)）；不支持的声明类型跳过
        let mut elements: Vec<&VertexElement> = format.elements.iter().collect();
        elements.sort_by_key(|e| e.offset);

        let mut vertices = Vec::with_capacity(header.vertex_count as usize);
        for i in 0..header.vertex_count as usize {
            let vertex_bytes = &blob[i * stride..(i + 1) * stride];
            let mut components = Vec::with_capacity(elements.len());
            for element in &elements {
                let end = element.offset as usize + element.decl_type.size();
                if end > stride {
                    return Err(Error::InsufficientPayload {
                        check: "VF_offset",
                        needed: end,
                        actual: stride,
                    });
                }
                match read_component(
                    &vertex_bytes[element.offset as usize..end],
                    element.decl_type,
                ) {
                    Ok(value) => components.push((**element, value)),
                    // C# 工厂返回 null → 组件跳过
                    Err(Error::UnsupportedDeclarationType(_)) => {}
                    Err(e) => return Err(e),
                }
            }
            vertices.push(DecodedVertex { components });
        }
        Ok(vertices)
    }

    fn blob_payload<'a>(
        &self,
        data: &'a [u8],
        number: u32,
        check: &'static str,
    ) -> Result<&'a [u8]> {
        let section = self.section_for(number, check, SectionType::BLOB)?;
        self.payload(data, section.number)
    }

    pub(crate) fn section_for(
        &self,
        number: u32,
        check: &'static str,
        expected: u32,
    ) -> Result<&Section> {
        let section = self.section(number).ok_or(Error::SectionNumberOutOfRange {
            number,
            count: self.sections().len() as u32,
        })?;
        if section.type_code != expected {
            return Err(Error::BadSectionType {
                check,
                number,
                expected,
                actual: section.type_code,
            });
        }
        Ok(section)
    }
}

/// C# `r.ReadS32()` 的 section 引用：负数映射为 u32::MAX，自然越界报错。
fn read_section_ref(r: &mut Reader<'_>, check: &'static str) -> Result<u32> {
    Ok(u32::try_from(r.i32(check)?).unwrap_or(u32::MAX))
}

pub(crate) fn parse_mesh_header(payload: &[u8]) -> Result<MeshHeader> {
    let mut r = Reader::new(payload);
    r.expect_u32(40, "ME001")?;
    r.expect_u32(4, "ME002")?;
    let tri_section = read_section_ref(&mut r, "ME_tri_section")?;
    let triangle_count = r.u32("ME_tri_count")?;
    r.expect_u32(1, "ME003")?;
    let start_index = r.u32("ME_start_index")?;
    let index_count = r.u32("ME_index_count")?;
    let min_vertex_index = r.u32("ME_min_vertex")?;
    let vertex_count = r.u32("ME_vertex_count")?;
    let vertex_section = read_section_ref(&mut r, "ME_vertex_section")?;
    if index_count != triangle_count * 3 {
        return Err(Error::UnexpectedValue {
            check: "ME005",
            expected: u64::from(triangle_count * 3),
            actual: u64::from(index_count),
        });
    }
    Ok(MeshHeader {
        tri_section,
        triangle_count,
        start_index,
        index_count,
        min_vertex_index,
        vertex_count,
        vertex_section,
    })
}

pub(crate) fn parse_triangle_array_header(payload: &[u8]) -> Result<TriangleArrayHeader> {
    let mut r = Reader::new(payload);
    let unknown1 = r.u32("TA_unk1")?;
    r.expect_u32(0, "TA001")?;
    let index_count = r.u32("TA_index_count")?;
    r.expect_u32(8, "TA002")?;
    r.expect_u32(101, "TA003")?;
    r.expect_u32(4, "TA004")?;
    let data_section = read_section_ref(&mut r, "TA_section")?;
    Ok(TriangleArrayHeader {
        unknown1,
        index_count,
        data_section,
    })
}

pub(crate) fn parse_vertex_array_header(payload: &[u8]) -> Result<VertexArrayHeader> {
    let mut r = Reader::new(payload);
    let format_section = read_section_ref(&mut r, "VA_fmt_section")?;
    let unknown2 = r.u32("VA_unk2")?;
    r.expect_u32(0, "VA001")?;
    let vertex_count = r.u32("VA_vertex_count")?;
    r.expect_u32(8, "VA002")?;
    let vertex_size = r.u32("VA_vertex_size")?;
    let data_section = read_section_ref(&mut r, "VA_data_section")?;
    Ok(VertexArrayHeader {
        format_section,
        unknown2,
        vertex_count,
        vertex_size,
        data_section,
    })
}
