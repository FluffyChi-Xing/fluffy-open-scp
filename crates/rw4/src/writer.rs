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
    SectionSpec {
        type_code,
        alignment: 0x10,
        payload,
        fixups: Vec::new(),
    }
}

pub(crate) fn align_up(v: u32, a: u32) -> u32 {
    if a <= 1 { v } else { (v + (a - 1)) & !(a - 1) }
}

/// 构造一个字节级合法的 RW4 文件（小端），布局与 C# `RW4Header.Write` 一致。

/// 模板片段：原样间隙（零填充）或 section。
pub enum TemplatePiece {
    /// N 个零字节（原版文件的对齐/预留间隙，如 house 模型的 43.8KB）。
    Gap(u32),
    Section(SectionSpec),
}

/// 模板模式构建：按调用方给出的 pieces 顺序/间隙/类型表/特殊块值重建文件，
/// 用于与原版模型逐字节对拍。与 `build` 的差异：section 位置含间隙、
/// 类型表由调用方给全量有序表、w25 可指定、0x10007 块照抄模板值。
pub fn build_template(
    file_type_code: u32,
    pieces: &[TemplatePiece],
    type_table: &[u32],
    w25: u32,
    block10007: [u32; 6],
) -> Vec<u8> {
    let type_constant: u32 = if file_type_code == 1 { 16 } else { 4 };
    let type_count = type_table.len() as u32;
    let count = pieces
        .iter()
        .filter(|p| matches!(p, TemplatePiece::Section(_)))
        .count() as u32;

    // ---- 头部 0x98（字段与 build() 相同，w25 换成模板值）----
    let mut out = Vec::with_capacity(0x400);
    out.extend_from_slice(&crate::header::MAGIC);
    out.extend(file_type_code.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend(type_constant.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    let sib_slot = out.len();
    out.extend(0u32.to_le_bytes());
    out.extend(0x98u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    let sie_slot = out.len();
    out.extend(0u32.to_le_bytes());
    out.extend(type_constant.to_le_bytes());
    let blob_slot = out.len();
    out.extend(0u32.to_le_bytes());
    for v in [4u32, 0, 1, 0, 1] {
        out.extend(v.to_le_bytes());
    }
    out.extend(w25.to_le_bytes());
    for v in [4u32, 0, 1, 0, 1] {
        out.extend(v.to_le_bytes());
    }
    for v in [0u32, 1, 0, 0, 0, 0, 0] {
        out.extend(v.to_le_bytes());
    }
    assert_eq!(out.len(), 0x98);

    // ---- 0x10004..0x10008 块 ----
    let blocks_begin = out.len() as u32;
    out.extend(0x10_004u32.to_le_bytes());
    let after_offsets = 28u32;
    for v in [4u32, 12, after_offsets, 0, 0, 0] {
        out.extend(v.to_le_bytes());
    }
    let b10005 = out.len();
    out.extend(0x10_005u32.to_le_bytes());
    out.extend(type_count.to_le_bytes());
    out.extend(12u32.to_le_bytes());
    for v in type_table {
        out.extend(v.to_le_bytes());
    }
    let after_10005 = (out.len() - blocks_begin as usize) as u32;
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
    let after_10006 = (out.len() - blocks_begin as usize) as u32;
    out.extend(0x10_007u32.to_le_bytes());
    for v in block10007 {
        out.extend(v.to_le_bytes());
    }
    let after_10007 = (out.len() - blocks_begin as usize) as u32;
    out.extend(0x10_008u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    out.extend(0u32.to_le_bytes());
    let header_end = out.len() as u32;
    // 回填 0x10004 的后三个偏移（相对块起始，语义同 build()）；
    // 偏移槽 i 位于 blocks_begin + 4 + 4*i，故 offsets[3] 在 +16。
    for (slot, value) in [(16, after_10005), (20, after_10006), (24, after_10007)] {
        let at = blocks_begin as usize + slot;
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    let _ = (b10005, after_10007);

    // ---- pieces 排放（Gap 前进；Section 对齐排放）----
    let mut cursor = header_end;
    let mut positions = Vec::with_capacity(count as usize);
    for piece in pieces {
        match piece {
            TemplatePiece::Gap(n) => cursor += n,
            TemplatePiece::Section(s) if s.type_code == SectionType::BLOB => {
                positions.push(0); // blob 数据在文件末的 blob 区，主布局不占位
            }
            TemplatePiece::Section(s) => {
                let aligned = align_up(cursor, s.alignment.max(1));
                positions.push(aligned);
                cursor = aligned + s.payload.len() as u32;
            }
        }
    }
    let section_index_begin = align_up(cursor, 4);
    let fixup_total: usize = pieces
        .iter()
        .filter_map(|p| match p {
            TemplatePiece::Section(s) => Some(s.fixups.len()),
            _ => None,
        })
        .sum();
    let section_index_end = section_index_begin + 24 * count + 8 * fixup_total as u32;

    // ---- 非 blob section 载荷 ----
    let mut idx = 0usize;
    for piece in pieces {
        match piece {
            TemplatePiece::Gap(n) => out.resize(out.len() + *n as usize, 0),
            TemplatePiece::Section(s) if s.type_code == SectionType::BLOB => {
                idx += 1; // blob 数据在文件末 blob 区统一排放
            }
            TemplatePiece::Section(s) => {
                let start = positions[idx] as usize;
                out.resize(start, 0);
                out.extend_from_slice(&s.payload);
                idx += 1;
            }
        }
    }
    out.resize(section_index_begin as usize, 0);

    // ---- section index + blob 区布局 ----
    // blob_positions 按 section 序号存放相对 section_index_end 的偏移。
    let mut blob_layout: Vec<(usize, u32)> = Vec::new();
    let mut blob_cursor = section_index_end;
    let mut ordinal = 0usize;
    for piece in pieces {
        if let TemplatePiece::Section(s) = piece {
            if s.type_code == SectionType::BLOB {
                let p = align_up(blob_cursor, s.alignment.max(1));
                blob_layout.push((ordinal, p - section_index_end));
                blob_cursor = p + s.payload.len() as u32;
            }
            ordinal += 1;
        }
    }
    let mut blob_positions = vec![0u32; count as usize];
    for (ord, rel) in &blob_layout {
        blob_positions[*ord] = *rel;
    }
    let mut k = 0usize;
    for piece in pieces {
        if let TemplatePiece::Section(s) = piece {
            let raw_pos = if s.type_code == SectionType::BLOB {
                blob_positions[k]
            } else {
                positions[k]
            };
            k += 1;
            out.extend(raw_pos.to_le_bytes());
            out.extend(0u32.to_le_bytes());
            out.extend((s.payload.len() as u32).to_le_bytes());
            out.extend(s.alignment.to_le_bytes());
            let ind = type_table
                .iter()
                .position(|t| *t == s.type_code)
                .expect("type must be in table") as u32;
            out.extend(ind.to_le_bytes());
            out.extend(s.type_code.to_le_bytes());
        }
    }
    for piece in pieces {
        if let TemplatePiece::Section(s) = piece {
            for &(target, offset) in &s.fixups {
                out.extend(target.to_le_bytes());
                out.extend(offset.to_le_bytes());
            }
        }
    }
    out.resize(section_index_end as usize, 0);

    // ---- blob 区（绝对位置 = section_index_end + 相对偏移）----
    let sections_in_order: Vec<&SectionSpec> = pieces
        .iter()
        .filter_map(|p| match p {
            TemplatePiece::Section(s) => Some(s),
            TemplatePiece::Gap(_) => None,
        })
        .collect();
    let mut end_cursor = section_index_end;
    for (ord, rel) in &blob_layout {
        let s = sections_in_order[*ord];
        let start = (section_index_end + *rel) as usize;
        out.resize(start, 0);
        out.extend_from_slice(&s.payload);
        end_cursor = (start + s.payload.len()) as u32;
    }
    let _ = end_cursor;

    // ---- 回填头 ----
    out[sib_slot..sib_slot + 4].copy_from_slice(&section_index_begin.to_le_bytes());
    out[sie_slot..sie_slot + 4].copy_from_slice(&section_index_end.to_le_bytes());
    let blob_total = out.len() as u32 - section_index_end;
    out[blob_slot..blob_slot + 4].copy_from_slice(&blob_total.to_le_bytes());
    out
}

/// Rebuild an existing native RW4 while preserving its container skeleton.
/// `replacements` is keyed by section number; all unmodified sections,
/// alignment gaps, type tables, header flags and the 0x10007 block are copied
/// from the source file. This is the safe path for modern GLB/LOTM geometry:
/// only payloads are translated, the twenty-year-old RenderWare envelope is
/// never re-invented.
pub fn replace_template_sections(
    source: &[u8],
    replacements: &std::collections::HashMap<u32, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let file = crate::Rw4File::parse(source).map_err(|e| e.to_string())?;
    let mut pieces = Vec::new();
    let mut type_table = vec![0, 0x10030, 0x10031, 0x10032, 0x10010];
    let mut prev_end = 0x138u32;
    for section in file.sections() {
        if section.type_code == SectionType::VERTEX_FORMAT {
            let gap = section.pos.saturating_sub(prev_end);
            if gap > 0 { pieces.push(TemplatePiece::Gap(gap)); }
        }
        prev_end = section.pos + section.size;
        let payload = replacements.get(&section.number).cloned().unwrap_or_else(|| {
            file.payload(source, section.number).unwrap_or_default().to_vec()
        });
        pieces.push(TemplatePiece::Section(SectionSpec {
            type_code: section.type_code,
            alignment: if section.alignment == 0 { 4 } else { section.alignment },
            payload,
            fixups: Vec::new(),
        }));
        if !type_table.contains(&section.type_code) { type_table.push(section.type_code); }
    }
    let mut extras = type_table[5..].to_vec();
    extras.sort_unstable();
    type_table.truncate(5);
    type_table.extend(extras);
    let w25 = u32::from_le_bytes(source[0x64..0x68].try_into().map_err(|_| "RW4 header truncated")?);
    let block = find_10007(source).ok_or("RW4 0x10007 block missing")?;
    let block10007 = read_words(source, block + 4, 6).ok_or("RW4 0x10007 block truncated")?;
    Ok(build_template(1, &pieces, &type_table, w25, block10007))
}

fn read_words(bytes: &[u8], offset: usize, count: usize) -> Option<[u32; 6]> {
    if count != 6 || offset.checked_add(24)? > bytes.len() { return None; }
    let mut out = [0u32; 6];
    for (i, word) in out.iter_mut().enumerate() {
        *word = u32::from_le_bytes(bytes[offset + i * 4..offset + i * 4 + 4].try_into().ok()?);
    }
    Some(out)
}

fn find_10007(bytes: &[u8]) -> Option<usize> {
    let mut off = 0x98usize;
    while off + 4 <= bytes.len() {
        let id = u32::from_le_bytes(bytes[off..off + 4].try_into().ok()?);
        if id == 0x10007 { return Some(off); }
        off += match id {
            0x10004 => 28,
            0x10005 => 12 + 4 * u32::from_le_bytes(bytes.get(off + 4..off + 8)?.try_into().ok()?) as usize,
            0x10006 => 36,
            0x10007 => 28,
            0x10008 => 12,
            _ => return None,
        };
    }
    None
}

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
