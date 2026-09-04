//! RW4 文件头与 section 索引解析 —— 严格对齐 C# `RW4Header.Read` /
//! `RW4Section.LoadHeader`（`Simcitypak-v2/SimCityPak/RenderWare4/`）。
//!
//! 布局（小端）：
//! ```text
//! 28B magic | file_type_code | section_count(repeat) | 16|4 常量 | 0
//! section_index_begin | 0x98(first header section) | 0,0,0 | section_index_end
//! 16|4 | file_size(-index_end) | 4,0,1,0,1 | unknown | 4,0,1,0,1 | 0,1,0,0,0,0,0
//! 0x98: 0x10004 + 6×相对偏移 | 0x10005 + 类型表 | 0x10006 固定块
//!       | 0x10007 fixup 元信息 | 0x10008 | 0,0
//! section index: count×24B entries + fixupCount×8B (target, offset) + padding
//! ```
//!
//! Blob section 的 `pos` 以 `section_index_end` 为基准存储，读取时重定位。

use crate::error::{Error, Result};
use crate::model::{FileType, Rw4File};
use crate::reader::Reader;
use crate::section::{Section, SectionType};

/// `"\x89RW4w32\x00\r\n\x1a\n\x00 \x04\x00454\x00000\x00\x00\x00\x00\x00"`
pub(crate) const MAGIC: [u8; 28] = [
    137, 82, 87, 52, 119, 51, 50, 0, 13, 10, 26, 10, 0, 32, 4, 0, 52, 53, 52, 0, 48, 48, 48, 0, 0,
    0, 0, 0,
];

/// 类型表前 5 项固定（C# `fixed_section_types`）。
pub(crate) const FIXED_SECTION_TYPES: [u32; 5] = [0, 0x10_030, 0x10_031, 0x10_032, 0x10_010];

const FILE_TYPE_MODEL: u32 = 1;
const FILE_TYPE_TEXTURE: u32 = 0x0400_0000;

pub(crate) fn parse(data: &[u8]) -> Result<Rw4File> {
    if data.len() < MAGIC.len() {
        return Err(Error::Truncated {
            needed: MAGIC.len(),
            at: 0,
            size: data.len(),
        });
    }
    if data[..MAGIC.len()] != MAGIC {
        return Err(Error::BadMagic);
    }

    let mut r = Reader::new(data);
    r.seek(MAGIC.len())?;

    let file_type_code = r.u32("file_type")?;
    let file_type = match file_type_code {
        FILE_TYPE_MODEL => FileType::Model,
        FILE_TYPE_TEXTURE => FileType::Texture,
        other => return Err(Error::UnknownFileType(other)),
    };
    let type_constant = u64::from(if file_type == FileType::Model {
        16u32
    } else {
        4u32
    });

    let section_count = r.u32("H001_count")?;
    r.expect_u64(u64::from(section_count), "H001")?;
    r.expect_u64(type_constant, "H002")?;
    r.expect_u32(0, "H003")?;

    let section_index_begin = r.u32("section_index_begin")?;
    let first_header_section_begin = r.u32("first_header_section_begin")?;
    r.expect_slice(&[0, 0, 0], "H010")?;
    let section_index_end = r.u32("section_index_end")?;
    r.expect_u64(type_constant, "H011")?;
    // C# 读取 file_size 并加上 index_end，但 H012 校验被注释禁用；此处仅读取。
    let _file_size_field = r.u32("file_size_field")?;

    r.expect_slice(&[4, 0, 1, 0, 1], "H020")?;
    let unknown = r.u32("header_unknown")?;
    r.expect_slice(&[4, 0, 1, 0, 1], "H040")?;
    r.expect_slice(&[0, 1, 0, 0, 0, 0, 0], "H041")?;

    if r.pos() != first_header_section_begin as usize {
        return Err(Error::UnexpectedValue {
            check: "H099",
            expected: first_header_section_begin as u64,
            actual: r.pos() as u64,
        });
    }

    r.expect_u32(0x10_004, "H140")?;
    let mut offsets = [0u32; 6];
    for slot in &mut offsets {
        *slot = r
            .u32("H140_offset")?
            .wrapping_add(first_header_section_begin);
    }

    expect_pos(&r, offsets[2], "H145")?;
    r.expect_u32(0x10_005, "H150")?;
    let section_type_count = r.u32("H150_count")?;
    r.expect_u32(12, "H151")?;
    let mut section_types = Vec::with_capacity(section_type_count as usize);
    for _ in 0..section_type_count {
        section_types.push(r.u32("section_type")?);
    }

    expect_pos(&r, offsets[3], "H146")?;
    r.expect_u32(0x10_006, "H160")?;
    r.expect_slice(
        &[
            3,
            0x18,
            file_type_code,
            0xFFB0_0000,
            file_type_code,
            0,
            0,
            0,
        ],
        "H161",
    )?;

    expect_pos(&r, offsets[4], "H147")?;
    r.expect_u32(0x10_007, "H170")?;
    let fixup_count = r.u32("H170_fixup_count")?;
    r.expect_u32(0, "H171")?;
    r.expect_u32(0, "H172")?;
    let index_and_fixups = offset_of("H173", section_index_begin, section_count, fixup_count)?;
    r.expect_u64(index_and_fixups, "H173")?;
    let index_only = offset_of("H174", section_index_begin, section_count, 0)?;
    r.expect_u64(index_only, "H174")?;
    r.expect_u64(u64::from(fixup_count), "H176")?;

    expect_pos(&r, offsets[5], "H148")?;
    r.expect_u32(0x10_008, "H180")?;
    r.expect_slice(&[0, 0], "H181")?;
    let header_end = r.pos() as u32;

    // ---- section index ----
    r.seek(section_index_begin as usize)?;
    let mut sections = Vec::with_capacity(section_count as usize);
    for number in 0..section_count {
        let pos = r.u32("H201_pos")?;
        r.expect_u32(0, "H201")?;
        let size = r.u32("section_size")?;
        let alignment = r.u32("section_alignment")?;
        let type_code_indirect = r.u32("section_indirect")?;
        let type_code = r.u32("section_type_code")?;
        // Blob 的 pos 以 section index 结束处为基准（C# `LoadHeader`）。
        let pos = if type_code == SectionType::BLOB {
            pos.wrapping_add(section_index_end)
        } else {
            pos
        };
        sections.push(Section {
            number,
            pos,
            size,
            alignment,
            type_code_indirect,
            type_code,
            fixups: Vec::new(),
        });
    }
    for _ in 0..fixup_count {
        let target = r.u32("fixup_section")? as usize;
        let offset = r.u32("fixup_offset")?;
        let section = sections
            .get_mut(target)
            .ok_or(Error::FixupTargetOutOfRange {
                index: target,
                count: section_count,
            })?;
        section.fixups.push(offset);
    }
    // padding 至 section_index_end（内容无语义，校验未越界即可）
    if r.pos() as u64 > u64::from(section_index_end) {
        return Err(Error::Truncated {
            needed: section_index_end as usize,
            at: section_index_begin as usize,
            size: data.len(),
        });
    }

    // ---- indirect 类型表一致性（C# H300/H301/H302）----
    let mut used = vec![false; section_types.len()];
    for section in &sections {
        let index = section.type_code_indirect as usize;
        let expected = *section_types
            .get(index)
            .ok_or(Error::IndirectTypeOutOfRange {
                index: section.type_code_indirect,
                count: section_type_count,
            })?;
        if expected != section.type_code {
            return Err(Error::IndirectTypeMismatch {
                check: "H300",
                index: section.type_code_indirect,
                expected,
                actual: section.type_code,
            });
        }
        used[index] = true;
    }
    for (i, fixed) in FIXED_SECTION_TYPES.iter().enumerate() {
        let actual = *section_types.get(i).ok_or(Error::IndirectTypeOutOfRange {
            index: i as u32,
            count: section_type_count,
        })?;
        if actual != *fixed {
            return Err(Error::FixedSectionTypeMismatch {
                check: "H301",
                index: i as u32,
                expected: *fixed,
                actual,
            });
        }
    }
    for (i, used_flag) in used.iter().enumerate().skip(FIXED_SECTION_TYPES.len()) {
        if !used_flag {
            return Err(Error::UnusedSectionType {
                code: section_types[i],
            });
        }
    }

    Ok(Rw4File::from_parts(
        file_type,
        unknown,
        section_index_begin,
        section_index_end,
        header_end,
        sections,
    ))
}

fn expect_pos(r: &Reader<'_>, expected: u32, check: &'static str) -> Result<()> {
    if r.pos() as u64 != u64::from(expected) {
        return Err(Error::UnexpectedValue {
            check,
            expected: expected as u64,
            actual: r.pos() as u64,
        });
    }
    Ok(())
}

fn offset_of(check: &'static str, base: u32, section_count: u32, fixup_count: u32) -> Result<u64> {
    let count = u64::from(section_count);
    let fixups = u64::from(fixup_count);
    u64::from(base)
        .checked_add(count * 24)
        .and_then(|v| v.checked_add(fixups * 8))
        .ok_or(Error::Overflow { check })
}
