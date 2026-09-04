use crate::error::{Error, Result};
use crate::section::Section;

/// RW4 文件种类（头部 `file_type_code`：1=Model，0x04000000=Texture）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Model,
    Texture,
}

/// 已解析的 RW4 容器：文件头元数据 + 扁平 section 索引。
///
/// 真实格式没有「section 树」：mesh/triangle 等通过 **section 编号**
/// 互相引用（见 C# `RW4Mesh.Read` 的 `m.Sections[tri_section]`）。
/// 本阶段只提供结构与原始 payload 访问，具体 section 解码由后续里程碑实现。
#[derive(Debug, Clone)]
pub struct Rw4File {
    file_type: FileType,
    unknown: u32,
    section_index_begin: u32,
    section_index_end: u32,
    header_end: u32,
    sections: Vec<Section>,
}

impl Rw4File {
    pub(crate) fn from_parts(
        file_type: FileType,
        unknown: u32,
        section_index_begin: u32,
        section_index_end: u32,
        header_end: u32,
        sections: Vec<Section>,
    ) -> Rw4File {
        Rw4File {
            file_type,
            unknown,
            section_index_begin,
            section_index_end,
            header_end,
            sections,
        }
    }

    /// 解析 RW4 容器（头部 + section 索引），与 C# `RW4Header.Read` 同样严格。
    pub fn parse(data: &[u8]) -> Result<Rw4File> {
        crate::header::parse(data)
    }

    pub fn file_type(&self) -> FileType {
        self.file_type
    }

    /// 头部 `Unknown` 字段（C# `RW4Header.Unknown`，语义未知）。
    pub fn unknown(&self) -> u32 {
        self.unknown
    }

    pub fn section_index_begin(&self) -> u32 {
        self.section_index_begin
    }

    pub fn section_index_end(&self) -> u32 {
        self.section_index_end
    }

    pub fn header_end(&self) -> u32 {
        self.header_end
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn section(&self, number: u32) -> Option<&Section> {
        self.sections.get(number as usize)
    }

    /// 按类型码遍历 section（保持索引顺序）。
    pub fn sections_of_type(&self, type_code: u32) -> impl Iterator<Item = &Section> {
        self.sections
            .iter()
            .filter(move |s| s.type_code == type_code)
    }

    /// 取某 section 的原始 payload（对 `data` 做边界检查的切片）。
    pub fn payload<'a>(&self, data: &'a [u8], number: u32) -> Result<&'a [u8]> {
        let section = self.section(number).ok_or(Error::SectionNumberOutOfRange {
            number,
            count: self.sections.len() as u32,
        })?;
        let start = section.pos as usize;
        let end = start
            .checked_add(section.size as usize)
            .ok_or(Error::PayloadOutOfRange {
                offset: section.pos,
                size: section.size,
                len: data.len(),
            })?;
        if end > data.len() {
            return Err(Error::PayloadOutOfRange {
                offset: section.pos,
                size: section.size,
                len: data.len(),
            });
        }
        Ok(&data[start..end])
    }
}
