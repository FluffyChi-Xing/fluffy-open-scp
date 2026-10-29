//! RW4 model container writer.

use crate::header::FIXED_SECTION_TYPES;
use crate::section::SectionType;

pub struct SectionSpec {
    pub type_code: u32,
    pub alignment: u32,
    pub payload: Vec<u8>,
    pub fixups: Vec<(u32, u32)>,
}

pub fn section(type_code: u32, payload: Vec<u8>) -> SectionSpec {
    SectionSpec { type_code, alignment: 0x10, payload, fixups: Vec::new() }
}

pub(crate) fn align_up(v: u32, a: u32) -> u32 {
    if a <= 1 { v } else { (v + (a - 1)) & !(a - 1) }
}

/// 构造一个字节级合法的 RW4 文件（小端），布局与 C# `RW4Header.Write` 一致。
pub fn build(file_type_code: u32, specs: &[SectionSpec], extra_types: &[u32]) -> Vec<u8> {
    let type_constant: u32 = if file_type_code == 1 { 16 } else { 4 };

    // 类型表：固定 5 项 + 各 section 用到的类型
    let mut table: Vec<u32> = FIXED_SECTION_TYPES.to_vec();
    let mut indirects = Vec::with_capacity(specs.len());
    for s in specs {
        let index = table
            .iter()
            .position(|t| *t == s.type_code)
            .unwrap_or_else(|| {
                table.push(s.type_code);
                table.len() - 1
            });
        indirects.push(index as u32);
    }
    table.extend_from_slice(extra_types);
    let type_count = table.len() as u32;

    // 布局
    let header_end: u32 = 0x98 + 116 + 4 * type_count;
    let mut cursor = header_end;
    let mut positions = Vec::with_capacity(specs.len());
    for s in specs {
        let p = align_up(cursor, s.alignment.max(1));
        positions.push(p);
        cursor = p + s.payload.len() as u32;
    }
    let section_index_begin = cursor;
    let fixup_total: usize = specs.iter().map(|s| s.fixups.len()).sum();
    let section_index_end = section_index_begin + 24 * specs.len() as u32 + 8 * fixup_total as u32;
    let mut blob_cursor = section_index_end;
    let mut blob_positions = vec![0u32; specs.len()];
    for (i, s) in specs.iter().enumerate() {
        if s.type_code == SectionType::BLOB {
            let p = align_up(blob_cursor, s.alignment.max(1));
            blob_positions[i] = p;
            blob_cursor = p + s.payload.len() as u32;
        }
    }
    let total_len = blob_cursor.max(section_index_end);

    // ---- 头部 ----
    let mut out = Vec::with_capacity(total_len as usize);
    out.extend_from_slice(&crate::header::MAGIC);
    out.extend(file_type_code.to_le_bytes());
    let count = specs.len() as u32;
    out.extend(count.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(type_constant.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(section_index_begin.to_le_bytes());
    out.extend(0x98u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(section_index_end.to_le_bytes());
    out.extend(type_constant.to_le_bytes());
    out.extend((total_len - section_index_end).to_le_bytes());
    for v in [4u32, 0, 1, 0, 1] {
        out.extend(v.to_le_bytes());
    }
    out.extend(0x40u32.to_le_bytes());
    for v in [4u32, 0, 1, 0, 1] {
        out.extend(v.to_le_bytes());
    }
    for v in [0u32, 1, 0, 0, 0, 0, 0] {
        out.extend(v.to_le_bytes());
    }
    assert_eq!(out.len(), 0x98);

    // ---- 0x10004..0x10008 头部 section 块 ----
    out.extend(0x10_004u32.to_le_bytes());
    let after_offsets = 28u32;
    let after_types = after_offsets + 12 + 4 * type_count;
    let after_10006 = after_types + 36;
    let after_10007 = after_10006 + 28;
    for v in [
        4u32,
        12,
        after_offsets,
        after_types,
        after_10006,
        after_10007,
    ] {
        out.extend(v.to_le_bytes());
    }
    out.extend(0x10_005u32.to_le_bytes());
    out.extend(type_count.to_le_bytes());
    out.extend(12u32.to_le_bytes());
    for v in &table {
        out.extend(v.to_le_bytes());
    }
    out.extend(0x10_006u32.to_le_bytes());
    for v in [
        3u32,
        0x18,
        file_type_code,
        0xFFB0_0000,
        file_type_code,
        0,
        0,
        0,
    ] {
        out.extend(v.to_le_bytes());
    }
    out.extend(0x10_007u32.to_le_bytes());
    out.extend((fixup_total as u32).to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend((section_index_begin + 24 * count + 8 * fixup_total as u32).to_le_bytes());
    out.extend((section_index_begin + 24 * count).to_le_bytes());
    out.extend((fixup_total as u32).to_le_bytes());
    out.extend(0x10_008u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    assert_eq!(out.len() as u32, header_end);

    // ---- 非 Blob payload ----
    for (i, s) in specs.iter().enumerate() {
        if s.type_code == SectionType::BLOB {
            continue;
        }
        let start = positions[i] as usize;
        out.resize(start, 0);
        out.extend_from_slice(&s.payload);
    }
    out.resize(section_index_begin as usize, 0);

    // ---- section index ----
    for (i, s) in specs.iter().enumerate() {
        let raw_pos = if s.type_code == SectionType::BLOB {
            blob_positions[i] - section_index_end
        } else {
            positions[i]
        };
        out.extend(raw_pos.to_le_bytes());
        out.extend(0u32.to_le_bytes());
        out.extend((s.payload.len() as u32).to_le_bytes());
        out.extend(s.alignment.to_le_bytes());
        out.extend(indirects[i].to_le_bytes());
        out.extend(s.type_code.to_le_bytes());
    }
    for s in specs {
        for &(target, offset) in &s.fixups {
            out.extend(target.to_le_bytes());
            out.extend(offset.to_le_bytes());
        }
    }
    out.resize(section_index_end as usize, 0);

    // ---- Blob payload（位于 index 之后）----
    for (i, s) in specs.iter().enumerate() {
        if s.type_code != SectionType::BLOB {
            continue;
        }
        let start = blob_positions[i] as usize;
        out.resize(start, 0);
        out.extend_from_slice(&s.payload);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_parseable_blob_container() {
        let bytes = build(1, &[section(SectionType::BLOB, vec![1, 2, 3, 4])], &[]);
        let file = crate::Rw4File::parse(&bytes).unwrap();
        let blob = file.sections_of_type(SectionType::BLOB).next().unwrap();
        assert_eq!(file.payload(&bytes, blob.number).unwrap(), &[1, 2, 3, 4]);
    }
}

