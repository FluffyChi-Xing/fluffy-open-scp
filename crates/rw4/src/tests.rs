use super::*;
use crate::header::FIXED_SECTION_TYPES;

/// 一个待写入 section index 的 section 规格。
struct Spec {
    type_code: u32,
    alignment: u32,
    payload: Vec<u8>,
    /// (目标 section 序号, fixup 偏移)
    fixups: Vec<(u32, u32)>,
}

fn spec(type_code: u32, payload: Vec<u8>) -> Spec {
    let alignment = 0x10;
    Spec {
        type_code,
        alignment,
        payload,
        fixups: Vec::new(),
    }
}

fn align_up(v: u32, a: u32) -> u32 {
    if a <= 1 { v } else { (v + (a - 1)) & !(a - 1) }
}

/// 构造一个字节级合法的 RW4 文件（小端），布局与 C# `RW4Header.Write` 一致。
fn build(file_type_code: u32, specs: &[Spec], extra_types: &[u32]) -> Vec<u8> {
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

fn patch_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn mesh_stack() -> Vec<Spec> {
    vec![
        spec(SectionType::VERTEX_FORMAT, vec![0xAA; 8]),
        spec(SectionType::VERTEX_ARRAY, vec![0xBB; 16]),
        spec(SectionType::TRIANGLE_ARRAY, vec![0xCC; 12]),
        spec(SectionType::MESH, vec![0xDD; 40]),
        Spec {
            // fixup 记录 (target_section, offset)：指向自身(4)以验证挂载
            fixups: vec![(4, 0x10)],
            ..spec(SectionType::BLOB, vec![0xEE; 64])
        },
    ]
}

#[test]
fn parses_model_with_mesh_stack_and_blob() {
    let data = build(1, &mesh_stack(), &[]);
    let file = Rw4File::parse(&data).unwrap();

    assert_eq!(file.file_type(), FileType::Model);
    assert_eq!(file.sections().len(), 5);
    assert_eq!(file.unknown(), 0x40);
    // 类型表 = 固定 5 项 + 4 个 mesh 栈类型 = 9 项
    assert_eq!(file.header_end() as usize, 0x98 + 116 + 4 * 9);

    let mesh = file.sections_of_type(SectionType::MESH).next().unwrap();
    assert_eq!(mesh.number, 3);
    assert_eq!(mesh.type_name(), Some("Mesh"));
    assert_eq!(file.payload(&data, mesh.number).unwrap(), &[0xDD; 40]);

    let blob = file.section(4).unwrap();
    assert!(blob.is_blob());
    assert!(
        blob.pos >= file.section_index_end(),
        "blob pos rebased past index"
    );
    assert_eq!(blob.fixups, vec![0x10], "fixup offset attached");
    assert_eq!(file.payload(&data, 4).unwrap(), &[0xEE; 64]);

    assert_eq!(file.payload(&data, 0).unwrap(), &[0xAA; 8]);
    assert!(file.section(0).unwrap().fixups.is_empty());

    // 索引边界合理
    assert!(file.section_index_begin() >= file.header_end());
    assert!(file.section_index_end() > file.section_index_begin());
}

#[test]
fn parses_texture_file_type() {
    let specs = vec![spec(SectionType::TEXTURE, vec![0x11; 32])];
    let data = build(0x0400_0000, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    assert_eq!(file.file_type(), FileType::Texture);
    assert_eq!(file.sections()[0].type_name(), Some("Texture"));
    assert_eq!(file.payload(&data, 0).unwrap(), &[0x11; 32]);
}

#[test]
fn unknown_type_codes_are_preserved() {
    let specs = vec![spec(0xAB_CD, vec![0x22; 4])];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    assert_eq!(file.sections()[0].type_code, 0xAB_CD);
    assert_eq!(file.sections()[0].type_name(), None);
}

#[test]
fn section_number_lookup_and_out_of_range() {
    let data = build(1, &mesh_stack(), &[]);
    let file = Rw4File::parse(&data).unwrap();
    assert!(file.section(4).is_some());
    assert!(file.section(5).is_none());
    assert!(matches!(
        file.payload(&data, 5),
        Err(Error::SectionNumberOutOfRange {
            number: 5,
            count: 5
        })
    ));
}

#[test]
fn payload_out_of_range_detected() {
    let mut data = build(1, &mesh_stack(), &[]);
    let file = Rw4File::parse(&data).unwrap();
    let size_field = file.section_index_begin() as usize + 8;
    patch_u32(&mut data, size_field, 0xFFFF_FFF0);
    let file = Rw4File::parse(&data).unwrap();
    assert!(matches!(
        file.payload(&data, 0),
        Err(Error::PayloadOutOfRange { .. })
    ));
}

// ---- 损坏用例 ----

#[test]
fn rejects_bad_magic() {
    let mut data = build(1, &mesh_stack(), &[]);
    data[0] = 0x00;
    assert!(matches!(Rw4File::parse(&data), Err(Error::BadMagic)));
}

#[test]
fn rejects_unknown_file_type() {
    let mut data = build(1, &mesh_stack(), &[]);
    patch_u32(&mut data, 28, 0x1234);
    assert!(matches!(
        Rw4File::parse(&data),
        Err(Error::UnknownFileType(0x1234))
    ));
}

#[test]
fn rejects_wrong_section_count_repeat() {
    let mut data = build(1, &mesh_stack(), &[]);
    patch_u32(&mut data, 36, 99);
    match Rw4File::parse(&data) {
        Err(Error::UnexpectedValue { check: "H001", .. }) => {}
        other => panic!("expected H001, got {other:?}"),
    }
}

#[test]
fn rejects_wrong_header_constant() {
    let mut data = build(1, &mesh_stack(), &[]);
    patch_u32(&mut data, 40, 0);
    match Rw4File::parse(&data) {
        Err(Error::UnexpectedValue { check: "H002", .. }) => {}
        other => panic!("expected H002, got {other:?}"),
    }
}

#[test]
fn rejects_wrong_header_section_base() {
    let mut data = build(1, &mesh_stack(), &[]);
    patch_u32(&mut data, 52, 0x90);
    match Rw4File::parse(&data) {
        Err(Error::UnexpectedValue { check: "H099", .. }) => {}
        other => panic!("expected H099, got {other:?}"),
    }
}

#[test]
fn rejects_fixed_type_table_mismatch() {
    let mut data = build(1, &mesh_stack(), &[]);
    // 类型表位于 0x98 + 40；补丁 table[2]（固定 0x10031，mesh 栈未使用）。
    // 注意 C# 先跑 H300：若补丁被引用的固定槽（如 table[1]=Blob）会先报 H300。
    let table2 = 0x98 + 40 + 8;
    patch_u32(&mut data, table2, 0xDEAD);
    match Rw4File::parse(&data) {
        Err(Error::FixedSectionTypeMismatch {
            check: "H301",
            index: 2,
            ..
        }) => {}
        other => panic!("expected H301, got {other:?}"),
    }
}

#[test]
fn rejects_unused_extra_type() {
    let data = build(1, &mesh_stack(), &[0x4242]);
    match Rw4File::parse(&data) {
        Err(Error::UnusedSectionType { code: 0x4242 }) => {}
        other => panic!("expected H302, got {other:?}"),
    }
}

#[test]
fn rejects_fixup_target_out_of_range() {
    let mut specs = mesh_stack();
    specs[0].fixups.push((9, 0));
    let data = build(1, &specs, &[]);
    match Rw4File::parse(&data) {
        Err(Error::FixupTargetOutOfRange { index: 9, count: 5 }) => {}
        other => panic!("expected fixup range error, got {other:?}"),
    }
}

#[test]
fn rejects_indirect_type_mismatch() {
    let mut data = build(1, &mesh_stack(), &[]);
    // entry 1 (VertexArray, indirect 6) 改指向 indirect 5（VertexFormat 槽）
    let entry1_indirect = {
        let file = Rw4File::parse(&data).unwrap();
        (file.section_index_begin() + 24 + 16) as usize
    };
    patch_u32(&mut data, entry1_indirect, 5);
    match Rw4File::parse(&data) {
        Err(Error::IndirectTypeMismatch { check: "H300", .. }) => {}
        other => panic!("expected H300, got {other:?}"),
    }
}

#[test]
fn truncated_inputs_do_not_panic() {
    let data = build(1, &mesh_stack(), &[]);
    for cut in [
        0,
        5,
        27,
        40,
        100,
        0x97,
        0x99,
        data.len() / 2,
        data.len() - 1,
    ] {
        let _ = Rw4File::parse(&data[..cut]);
    }
    // 截断到 index 中段：必须返回 Err 而不是 panic
    let file = Rw4File::parse(&data).unwrap();
    let mid_index = (file.section_index_begin() + file.section_index_end()) as usize / 2;
    assert!(Rw4File::parse(&data[..mid_index]).is_err());
}

#[test]
fn deterministic_reparse() {
    let data = build(1, &mesh_stack(), &[]);
    let a = Rw4File::parse(&data).unwrap();
    let b = Rw4File::parse(&data).unwrap();
    assert_eq!(a.sections(), b.sections());
    assert_eq!(a.file_type(), b.file_type());
}

// ---- M3 网格解码 fixture ----

use crate::vertex::{ComponentValue, DeclarationType, DeclarationUsage};

/// 声明布局：POS@0 FLOAT3 | NORMAL@12 UBYTE4 | TEXCOORD0@16 FLOAT2 |
/// BLENDINDICES@24 UBYTE4 | BLENDWEIGHT@28 FLOAT4，stride 44
fn vertex_format_payload() -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(0u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(5u16.to_le_bytes()); // 5 个组件
    p.extend(44u16.to_be_bytes()); // stride（BE）
    p.extend(0u32.to_be_bytes());
    p.extend(0u32.to_be_bytes());
    let element = |offset: u16, ty: u16, usage: u16, index: u8| {
        let mut e = Vec::new();
        e.extend(0u8.to_le_bytes());
        e.extend(offset.to_be_bytes());
        e.extend(ty.to_be_bytes());
        e.extend(usage.to_be_bytes());
        e.extend(index.to_le_bytes());
        e.extend(0u8.to_le_bytes());
        e.extend(0u16.to_be_bytes());
        e.extend(0u8.to_le_bytes());
        e
    };
    p.extend(element(0, 2, 0, 0)); // FLOAT3 POSITION
    p.extend(element(12, 5, 3, 0)); // UBYTE4 NORMAL
    p.extend(element(16, 1, 5, 0)); // FLOAT2 TEXCOORD0
    p.extend(element(24, 5, 2, 0)); // UBYTE4 BLENDINDICES
    p.extend(element(28, 3, 1, 0)); // FLOAT4 BLENDWEIGHT
    p
}

fn vertex_bytes(
    position: [f32; 3],
    normal: [u8; 4],
    uv: [f32; 2],
    indices: [u8; 4],
    weights: [f32; 4],
) -> Vec<u8> {
    let mut v = Vec::with_capacity(44);
    for f in position {
        v.extend(f.to_le_bytes());
    }
    v.extend(normal);
    for f in uv {
        v.extend(f.to_le_bytes());
    }
    v.extend(indices);
    for f in weights {
        v.extend(f.to_le_bytes());
    }
    assert_eq!(v.len(), 44);
    v
}

fn triangle_data_payload(tris: &[[u16; 3]]) -> Vec<u8> {
    let mut p = Vec::new();
    for t in tris {
        for idx in t {
            p.extend(idx.to_le_bytes());
        }
    }
    p
}

fn triangle_array_payload(data_section: u32, index_count: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(0u32.to_le_bytes()); // unk1
    p.extend(0u32.to_le_bytes()); // expect 0
    p.extend(index_count.to_le_bytes());
    p.extend(8u32.to_le_bytes());
    p.extend(101u32.to_le_bytes());
    p.extend(4u32.to_le_bytes());
    p.extend((data_section as i32).to_le_bytes());
    p
}

fn vertex_array_payload(
    fmt_section: u32,
    data_section: u32,
    vertex_count: u32,
    vertex_size: u32,
) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend((fmt_section as i32).to_le_bytes());
    p.extend(0u32.to_le_bytes()); // unk2
    p.extend(0u32.to_le_bytes()); // expect 0
    p.extend(vertex_count.to_le_bytes());
    p.extend(8u32.to_le_bytes());
    p.extend(vertex_size.to_le_bytes());
    p.extend((data_section as i32).to_le_bytes());
    p
}

fn mesh_payload(tri_section: u32, tri_count: u32, vert_count: u32, vert_section: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(40u32.to_le_bytes());
    p.extend(4u32.to_le_bytes());
    p.extend((tri_section as i32).to_le_bytes());
    p.extend(tri_count.to_le_bytes());
    p.extend(1u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend((tri_count * 3).to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(vert_count.to_le_bytes());
    p.extend((vert_section as i32).to_le_bytes());
    p
}

#[test]
fn decodes_mesh_with_vertices_and_blend_indices() {
    let verts = vec![
        vertex_bytes(
            [1.0, 2.0, 3.0],
            [255, 128, 0, 1],
            [0.25, 0.75],
            [0, 3, 9, 255],
            [1.0, 0.0, 0.0, 0.0],
        ),
        vertex_bytes(
            [-1.5, 0.0, 4.5],
            [128, 128, 128, 255],
            [0.5, 0.5],
            [6, 12, 0, 3],
            [0.5, 0.5, 0.0, 0.0],
        ),
        vertex_bytes(
            [0.0, 0.0, 0.0],
            [0, 0, 0, 0],
            [1.0, 0.0],
            [0, 0, 0, 0],
            [0.0, 0.0, 0.0, 1.0],
        ),
    ];
    let mut vertex_blob = Vec::new();
    for v in &verts {
        vertex_blob.extend(v.iter().copied());
    }

    let specs = vec![
        spec(SectionType::VERTEX_FORMAT, vertex_format_payload()),
        spec(SectionType::VERTEX_ARRAY, vertex_array_payload(0, 2, 3, 44)),
        spec(SectionType::BLOB, vertex_blob),
        Spec {
            fixups: vec![(3, 0)],
            ..spec(
                SectionType::BLOB,
                triangle_data_payload(&[[0, 1, 2], [2, 1, 0]]),
            )
        },
        spec(SectionType::TRIANGLE_ARRAY, triangle_array_payload(3, 6)),
        spec(SectionType::MESH, mesh_payload(4, 2, 3, 1)),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    assert_eq!(file.sections().len(), 6);

    let decoded = file.decode_mesh(&data, 5).unwrap();
    assert_eq!(decoded.triangles, vec![[0, 1, 2], [2, 1, 0]]);
    assert_eq!(decoded.vertices.len(), 3);

    let v0 = &decoded.vertices[0];
    assert_eq!(v0.position(), Some([1.0, 2.0, 3.0]));
    let n = v0.normal().unwrap();
    assert!((n[0] - 1.0).abs() < 1e-6, "255 -> ~1.0, got {n:?}");
    assert!((n[1] - 0.0).abs() < 5e-3, "128 -> ~0, got {n:?}");
    assert_eq!(v0.uv(), Some([0.25, 0.75]));
    // 原始 [0,3,9,255] ÷3 = [0,1,3,85]，joint_count=4 → 85 钳到 0
    assert_eq!(v0.blend_indices_raw(), Some([0, 3, 9, 255]));
    assert_eq!(v0.blend_indices(4), Some([0, 1, 3, 0]));
    assert_eq!(v0.blend_weights(), Some([1.0, 0.0, 0.0, 0.0]));
}

#[test]
fn decoded_vertex_uv_prefers_float2_over_float4() {
    // TEXCOORD0 FLOAT4（facade 大坐标）+ TEXCOORD1 FLOAT2（真实 UV）
    let mut fmt = vertex_format_payload();
    fmt[12..14].copy_from_slice(&6u16.to_le_bytes()); // 组件数 5 -> 6
    // 元素区起始于 24（12 零 + count2 + stride2 + unk2_4 + unk3_4）；
    // 元素2(TEXCOORD0) 类型字段在 24+2*12+3
    fmt[51..53].copy_from_slice(&3u16.to_be_bytes()); // FLOAT2 -> FLOAT4
    {
        let mut e = Vec::new();
        e.extend(0u8.to_le_bytes());
        e.extend(44u16.to_be_bytes()); // offset 44
        e.extend(1u16.to_be_bytes()); // FLOAT2
        e.extend(5u16.to_be_bytes()); // TEXCOORD
        e.extend(1u8.to_le_bytes()); // index 1
        e.extend(0u8.to_le_bytes());
        e.extend(0u16.to_be_bytes());
        e.extend(0u8.to_le_bytes());
        fmt.extend(e);
    }
    fmt[14..16].copy_from_slice(&52u16.to_be_bytes()); // stride 44 -> 52

    let mut v = vertex_bytes([0.0; 3], [127; 4], [999.0, 999.0], [0; 4], [0.0; 4]);
    v.extend(0.125f32.to_le_bytes());
    v.extend(0.875f32.to_le_bytes());
    let vb = v; // 单顶点

    let specs = vec![
        spec(SectionType::VERTEX_FORMAT, fmt),
        spec(SectionType::VERTEX_ARRAY, vertex_array_payload(0, 2, 1, 52)),
        spec(SectionType::BLOB, vb),
        Spec {
            fixups: vec![(3, 0)],
            ..spec(SectionType::BLOB, triangle_data_payload(&[[0, 0, 0]]))
        },
        spec(SectionType::TRIANGLE_ARRAY, triangle_array_payload(3, 3)),
        spec(SectionType::MESH, mesh_payload(4, 1, 1, 1)),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    let decoded = file.decode_mesh(&data, 5).unwrap();

    assert_eq!(decoded.vertices.len(), 1);
    assert_eq!(
        decoded.vertices[0].uv(),
        Some([0.125, 0.875]),
        "FLOAT2 preferred"
    );
}

#[test]
fn mesh_without_vertex_section_decodes_triangles_only() {
    let specs = vec![
        Spec {
            fixups: vec![(1, 0)],
            ..spec(SectionType::BLOB, triangle_data_payload(&[[0, 1, 2]]))
        },
        spec(SectionType::TRIANGLE_ARRAY, triangle_array_payload(0, 3)),
        spec(
            SectionType::MESH,
            mesh_payload(1, 1, 0, crate::mesh::NO_VERTEX_SECTION),
        ),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    let decoded = file.decode_mesh(&data, 2).unwrap();
    assert_eq!(decoded.triangles, vec![[0, 1, 2]]);
    assert!(
        decoded.vertices.is_empty(),
        "blend-shape mesh has no vertices"
    );
    assert!(!decoded.header.has_vertex_data());
}

#[test]
fn decode_mesh_rejects_wrong_section_types() {
    // mesh 引用的 tri_section 指向 VertexFormat → TA000
    let specs = vec![
        spec(SectionType::VERTEX_FORMAT, vertex_format_payload()),
        spec(
            SectionType::MESH,
            mesh_payload(0, 1, 0, crate::mesh::NO_VERTEX_SECTION),
        ),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    match file.decode_mesh(&data, 1) {
        Err(Error::BadSectionType { check: "TA000", .. }) => {}
        other => panic!("expected TA000, got {other:?}"),
    }
}

#[test]
fn decode_mesh_rejects_triangle_count_mismatch() {
    let specs = vec![
        Spec {
            fixups: vec![(1, 0)],
            ..spec(SectionType::BLOB, triangle_data_payload(&[[0, 1, 2]]))
        },
        spec(SectionType::TRIANGLE_ARRAY, triangle_array_payload(0, 3)),
        spec(
            SectionType::MESH,
            mesh_payload(1, 5, 0, crate::mesh::NO_VERTEX_SECTION),
        ),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    match file.decode_mesh(&data, 2) {
        Err(Error::UnexpectedValue {
            check: "ME100",
            expected: 5,
            actual: 1,
        }) => {}
        other => panic!("expected ME100, got {other:?}"),
    }
}

#[test]
fn declaration_metadata_is_complete() {
    assert_eq!(DeclarationType::from_u16(5), Some(DeclarationType::UByte4));
    assert_eq!(DeclarationType::UByte4.size(), 4);
    assert_eq!(DeclarationType::Float3.size(), 12);
    assert_eq!(DeclarationType::Float4.size(), 16);
    assert_eq!(DeclarationType::Float4.name(), "FLOAT4");
    assert_eq!(DeclarationType::from_u16(18), None);
    assert_eq!(
        DeclarationUsage::from_u16(2),
        Some(DeclarationUsage::BlendIndices)
    );
    assert_eq!(DeclarationUsage::BlendIndices.name(), "BLENDINDICES");
    assert_eq!(DeclarationUsage::from_u16(14), None);
    let _ = ComponentValue::Float1(0.0);
}
