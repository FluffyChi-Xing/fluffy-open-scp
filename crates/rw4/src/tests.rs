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
fn decode_mesh_rejects_index_range_overflow() {
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
    // TA 只有 3 个索引；mesh 声明 5 tri（=15 索引）→ 切片越界
    match file.decode_mesh(&data, 2) {
        Err(Error::UnexpectedValue {
            check: "ME100",
            expected: 3,
            actual: 15,
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

// ---- M3 材质与贴图 fixture ----

use crate::material::{MaterialSection, SHADER_DEF_MARKER};
use crate::texture::{TEXTURE_TYPE_DXT1, decode_dxt1, decode_dxt5};

/// 材质 payload：Size + 28B 头 + 顶点格式副本 + 附加数据
/// + 7 条引用记录（shader-def 在前，slot 0..=5 在后）+ 尾数据。
fn material_payload(slot_instances: [(u32, u32); 6]) -> Vec<u8> {
    let mut p = Vec::new();
    let vf_copy = vertex_format_payload(); // 84B（5 组件 → 24+60）
    let total = 4 + 28 + vf_copy.len() + 8 + 7 * 24 + 12;
    p.extend((total as u32).to_le_bytes());
    p.extend([0x11u8; 28]); // header
    p.extend_from_slice(&vf_copy); // 顶点格式副本
    p.extend([0xAAu8; 8]); // additional data（引用表前）
    for (slot, instance) in [(SHADER_DEF_MARKER, 0xDDDD_0001)].iter().copied()
        .chain(slot_instances)
    {
        p.extend(slot.to_le_bytes());
        p.extend(0u32.to_le_bytes());
        p.extend(instance.to_le_bytes());
        p.extend(0u32.to_le_bytes());
        p.extend(0u32.to_le_bytes());
        p.extend(0u32.to_le_bytes());
    }
    p.extend([0x77u8; 12]); // 尾部数据
    p
}

#[test]
fn material_slots_and_shader_def_are_resolved() {
    let payload = material_payload([
        (0, 0xAAAA_0001), // 调色板
        (1, 0xAAAA_0002), // 区域遮罩
        (2, 0xAAAA_0003), // 法线
        (3, 0xAAAA_0004), // 副遮罩
        (4, 0xAAAA_0005),
        (5, 0xAAAA_0006),
    ]);
    let specs = vec![
        spec(SectionType::VERTEX_FORMAT, vertex_format_payload()),
        spec(SectionType::MATERIAL, payload),
        spec(
            SectionType::MESH,
            mesh_payload(0, 0, 0, crate::mesh::NO_VERTEX_SECTION),
        ),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    let material = file.decode_material(&data, 1).unwrap();
    let MaterialSection::Decoded(ref m) = material else {
        panic!("material should decode");
    };
    assert_eq!(m.header, [0x11; 28]);
    assert_eq!(
        m.vertex_format_data.len(),
        24 + 12 * 5,
        "顶点格式副本跟随模型声明"
    );
    assert_eq!(m.additional_data, vec![0xAA; 8]);
    assert_eq!(m.data, vec![0x77; 12]);
    assert_eq!(m.slot_texture(0), Some(0xAAAA_0001));
    assert_eq!(m.slot_texture(1), Some(0xAAAA_0002));
    assert_eq!(m.slot_texture(2), Some(0xAAAA_0003));
    assert_eq!(m.slot_texture(3), Some(0xAAAA_0004));
    assert_eq!(m.texture_slots().count(), 6, "shader-def 槽被跳过");
    assert_eq!(material.texture_refs().len(), 7);
    // 第 0 条记录是 shader-def 引用（C# 错位读取的根因）
    assert_eq!(material.texture_refs()[0].slot, SHADER_DEF_MARKER);
    assert_eq!(material.texture_refs()[0].texture_instance, 0xDDDD_0001);
}

#[test]
fn material_without_marker_falls_back_to_raw() {
    // 全部槽位都不含 0x2D，并把标记位抹掉 → 扫描必然失败
    let mut payload = material_payload([(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6)]);
    let marker_at = 4 + 28 + 84 + 8; // size + header + 顶点格式副本 + 附加数据
    payload[marker_at..marker_at + 4].copy_from_slice(&[0xEE, 0xEE, 0xEE, 0xEE]);
    let specs = vec![spec(SectionType::MATERIAL, payload)];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    let material = file.decode_material(&data, 0).unwrap();
    assert!(
        matches!(material, MaterialSection::Raw(_)),
        "无 0x2D 标记应回退 Raw（C# _rawSection 行为）"
    );
    assert!(material.texture_refs().is_empty());
}

/// 4×4 DXT1 块：c0=红(0xF800)、c1=蓝(0x001F)，4 色模式，逐像素索引 0..3。
fn dxt1_block() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(0xF800u16.to_le_bytes());
    b.extend(0x001Fu16.to_le_bytes());
    b.extend(0b11100100u32.to_le_bytes()); // idx0=0,1=1,2=2,3=3
    b
}

#[test]
fn dxt1_decode_matches_reference_colors() {
    let rgba = decode_dxt1(&dxt1_block(), 4, 4);
    assert_eq!(rgba.len(), 64);
    let px = |i: usize| &rgba[i * 4..i * 4 + 4];
    assert_eq!(px(0), &[255, 0, 0, 255], "纯红");
    assert_eq!(px(1), &[0, 0, 255, 255], "纯蓝");
    // 2/3 插值：红*2/3 + 蓝*1/3
    let expected = [170u8, 0, 85, 255];
    let got = px(2);
    assert!(
        got.iter()
            .zip(expected)
            .all(|(a, b)| i16::from(*a) - i16::from(b).abs() <= 1),
        "2/3 插值 {got:?}"
    );
    assert_eq!(px(3), &[85, 0, 170, 255], "1/3 插值");
}

#[test]
fn palette_f32_decodes_row_major_columns() {
    // 2×1 调色板条：两列各 16 字节（4×f32 = ColorBottom/Top/Int1/Int2）
    let mut blob = Vec::new();
    for x in 0..2u32 {
        for value in [x as f32 + 0.1, x as f32 + 0.2, 9.0, 9.0] {
            blob.extend_from_slice(&value.to_le_bytes());
        }
    }
    let texture = crate::texture::DecodedTexture {
        texture_type: crate::texture::TEXTURE_TYPE_PALETTE_F32,
        unknown1: 0,
        width: 2,
        height: 1,
        mipmap_info: 0x100,
        data_section: 0,
        blob,
    };
    let pixels = texture.decode_palette_f32().unwrap();
    assert_eq!(pixels.len(), 2);
    assert_eq!(pixels[0][0], 0.1, "列 0 row0 = ColorBottom");
    assert_eq!(pixels[1][1], 1.2, "列 1 row1 = ColorTop");
}

#[test]
fn dxt5_alpha_gradient_matches_reference() {
    // DXT5 块：a0=255, a1=0（8 值模式），alpha 索引 2..7 渐变；颜色全红
    let mut b = Vec::new();
    b.extend(255u8.to_le_bytes());
    b.extend(0u8.to_le_bytes());
    // 索引 0..16 依次 0..7（3bit，4bit 边界跨字节）
    let mut bits: u64 = 0;
    for i in 0..16u64 {
        bits |= (i % 8) << (i * 3);
    }
    b.extend(bits.to_le_bytes()[..6].to_vec());
    b.extend(0xF800u16.to_le_bytes());
    b.extend(0x0000u16.to_le_bytes());
    b.extend(0u32.to_le_bytes()); // 全部索引 0 → 纯红

    let rgba = decode_dxt5(&b, 4, 4);
    let px = |i: usize| &rgba[i * 4..i * 4 + 4];
    assert_eq!(px(0)[3], 255);
    assert_eq!(px(1)[3], 0);
    // 索引 2 → (6*255+0)/7 = 218；索引 9%8=1 → a1=0；索引 15%8=7 → (255+0)/7
    assert_eq!(px(2)[3], 218);
    assert_eq!(px(9)[3], 0, "索引 9 % 8 = 1 → a1");
    assert_eq!(px(15)[3], 36, "(1*255+6*0)/7");
    assert_eq!(px(0)[0], 255, "R 通道");
}

fn texture_payload(
    texture_type: u32,
    width: u16,
    height: u16,
    mips: u32,
    data_section: u32,
) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(texture_type.to_le_bytes());
    p.extend(8u32.to_le_bytes());
    p.extend(0u32.to_le_bytes()); // unk1
    p.extend(width.to_le_bytes());
    p.extend(height.to_le_bytes());
    p.extend((mips << 8).to_le_bytes()); // mipmapInfo
    p.extend(0u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend((data_section as i32).to_le_bytes());
    p
}

#[test]
fn texture_section_decodes_and_writes_dds() {
    let blob = dxt1_block();
    let specs = vec![
        spec(
            SectionType::TEXTURE,
            texture_payload(TEXTURE_TYPE_DXT1, 4, 4, 7, 1),
        ),
        Spec {
            fixups: vec![(1, 0)],
            ..spec(SectionType::BLOB, blob.clone())
        },
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    let texture = file.decode_texture(&data, 0).unwrap();
    assert_eq!(texture.format(), crate::texture::TextureFormat::Dxt1);
    assert_eq!(texture.mip_count(), 7, "0x708 → 7 级 mip");
    assert_eq!(texture.blob, blob);

    let rgba = texture.decode_top_mip_rgba().unwrap();
    assert_eq!(&rgba[..4], &[255, 0, 0, 255]);

    let dds = texture.write_dds().unwrap();
    assert_eq!(&dds[..4], b"DDS ");
    assert_eq!(
        u32::from_le_bytes(dds[12..16].try_into().unwrap()),
        4,
        "height"
    );
    assert_eq!(
        u32::from_le_bytes(dds[16..20].try_into().unwrap()),
        4,
        "width"
    );
    assert_eq!(dds.len(), 128 + blob.len());
}

#[test]
fn raw_texture_is_bgra_to_rgba_and_dds_is_rejected() {
    let blob: Vec<u8> = vec![10, 20, 30, 40, 50, 60, 70, 80]; // 2 px BGRA
    let specs = vec![
        spec(
            SectionType::TEXTURE,
            texture_payload(crate::texture::TEXTURE_TYPE_RAW_BGRA, 2, 1, 1, 1),
        ),
        spec(SectionType::BLOB, blob),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    let texture = file.decode_texture(&data, 0).unwrap();
    assert_eq!(
        texture.decode_top_mip_rgba().unwrap(),
        vec![30, 20, 10, 40, 70, 60, 50, 80]
    );
    assert!(matches!(
        texture.write_dds(),
        Err(Error::UnsupportedTextureType(21))
    ));
}

// ---- M3 骨骼/动画/矩阵 fixture ----

use crate::anim::COMPONENTS_LOC_ROT;
use crate::anim::COMPONENTS_LOC_ROT_SCALE;
use crate::math::{mat4_decompose_trs, mat4_inverse, mat4_mul};
use crate::skeleton::{DecodedSkeleton, Hierarchy, Joint};

fn hierarchy_payload(joints: &[(u32, u32, i32)], id: u32) -> Vec<u8> {
    let mut p = Vec::new();
    let p1 = 24u32;
    let p2 = p1 + 4 * joints.len() as u32;
    let p3 = p1 + 8 * joints.len() as u32;
    p.extend(p2.to_le_bytes());
    p.extend(p3.to_le_bytes());
    p.extend(p1.to_le_bytes());
    p.extend((joints.len() as u32).to_le_bytes());
    p.extend(id.to_le_bytes());
    p.extend((joints.len() as u32).to_le_bytes());
    for (name, _, _) in joints {
        p.extend(name.to_le_bytes());
    }
    for (_, flags, _) in joints {
        p.extend(flags.to_le_bytes());
    }
    for (_, _, parent) in joints {
        p.extend((*parent).to_le_bytes());
    }
    p
}

fn matrices_payload<const N: usize>(items: &[[f32; N]]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(16u32.to_le_bytes()); // p1
    p.extend((items.len() as u32).to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    for item in items {
        for v in item {
            p.extend(v.to_le_bytes());
        }
    }
    p
}

fn skeleton_payload(mat3_ref: u32, hierarchy_ref: u32, mat4_ref: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend(0x400000u32.to_le_bytes());
    p.extend(0x8d6da0u32.to_le_bytes()); // unk1（观察值）
    p.extend((mat3_ref as i32).to_le_bytes());
    p.extend((hierarchy_ref as i32).to_le_bytes());
    p.extend((mat4_ref as i32).to_le_bytes());
    p
}

#[test]
fn skeleton_decodes_hierarchy_and_bind_matrices() {
    let joints = [
        (0x1111u32, 0u32, -1i32), // 根
        (0x2222u32, 1u32, 0),
        (0x3333u32, 3u32, 1),
    ];
    // 缩放平移矩阵（列主序），条数需 ≥ 关节数
    let mat4: [[f32; 16]; 3] = [
        [
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 10.0, 20.0, 30.0, 1.0,
        ],
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
        ],
        [
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ],
    ];
    let mat3: [[f32; 12]; 3] = [[1.0; 12]; 3];
    let specs = vec![
        spec(SectionType::MATRICES_4X3, matrices_payload(&mat3)),
        spec(
            SectionType::HIERARCHY_INFO,
            hierarchy_payload(&joints, 0xABCD),
        ),
        spec(0x70003, matrices_payload(&mat4)),
        spec(SectionType::RW4_SKELETON, skeleton_payload(0, 1, 2)),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    // 头部指针为绝对文件偏移：按真实 section pos 回补（C# `Matrices.Read`
    // 的 `p1 == r.Position`、`HierarchyInfo` 的 p1/p2/p3 同为绝对值）
    let base_m3 = file.section(0).unwrap().pos as u32;
    let base_h = file.section(1).unwrap().pos as u32;
    let base_m4 = file.section(2).unwrap().pos as u32;
    let mut h = hierarchy_payload(&joints, 0xABCD);
    let hp1 = base_h + 24;
    h[0..4].copy_from_slice(&(hp1 + 4 * 3).to_le_bytes()); // p2
    h[4..8].copy_from_slice(&(hp1 + 8 * 3).to_le_bytes()); // p3
    h[8..12].copy_from_slice(&hp1.to_le_bytes()); // p1
    let mut m3 = matrices_payload(&mat3);
    m3[0..4].copy_from_slice(&(base_m3 + 16).to_le_bytes());
    let mut m4 = matrices_payload(&mat4);
    m4[0..4].copy_from_slice(&(base_m4 + 16).to_le_bytes());
    // 用回补后的 payload 重建文件
    let specs = vec![
        spec(SectionType::MATRICES_4X3, m3),
        spec(SectionType::HIERARCHY_INFO, h),
        spec(0x70003, m4),
        spec(SectionType::RW4_SKELETON, skeleton_payload(0, 1, 2)),
    ];
    let data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();

    let skeleton = file.decode_skeleton(&data, 3).unwrap();
    let DecodedSkeleton {
        hierarchy,
        bind_matrices,
        matrices_4x3,
        unknown,
    } = skeleton;
    assert_eq!(unknown, 0x8d6da0);
    let Hierarchy { id, joints: parsed } = hierarchy;
    assert_eq!(id, 0xABCD);
    assert_eq!(parsed.len(), 3);
    assert_eq!(
        parsed[0],
        Joint {
            name_fnv: 0x1111,
            flags: 0,
            parent: -1
        }
    );
    assert_eq!(parsed[2].parent, 1);
    assert_eq!(bind_matrices.len(), 3);
    assert_eq!(bind_matrices[0][12], 10.0);
    assert_eq!(matrices_4x3.len(), 3);
}

fn anim_header(channels: usize, skeleton_id: u32, length: f32) -> (Vec<u8>, u32, u32, u32) {
    let count = channels as u32;
    let p_names = 48u32;
    let p_info = p_names + count * 4;
    let p_data = p_info + count * 12;
    let mut p = Vec::new();
    p.extend(p_names.to_le_bytes());
    p.extend(count.to_le_bytes());
    p.extend(skeleton_id.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(p_data.to_le_bytes());
    p.extend(0u32.to_le_bytes()); // pPaddingEnd 占位
    p.extend(count.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(length.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(0u32.to_le_bytes());
    p.extend(p_info.to_le_bytes());
    (p, p_names, p_info, p_data)
}

#[test]
fn anim_decodes_locrot_channels() {
    let key_bytes = |qx: f32, tx: f32, time: f32| {
        let mut k = Vec::new();
        for v in [qx, 0.0, 0.0, 1.0] {
            k.extend(v.to_le_bytes());
        }
        for v in [tx, 0.0, 0.0] {
            k.extend(v.to_le_bytes());
        }
        k.extend(time.to_le_bytes());
        k.extend(0u32.to_le_bytes()); // stride 对齐填充（36B key）
        k
    };
    let (mut p, _p_names, p_info, p_data) = anim_header(2, 0xFEED, 1.5);
    let ch1_data = p_data + 36;
    // pPaddingEnd
    p[20..24].copy_from_slice(&(ch1_data + 36).to_le_bytes());
    p.extend(0xAAAAu32.to_le_bytes());
    p.extend(0xBBBBu32.to_le_bytes());

    for (pos, id) in [(p_data, 0xAAAAu32), (ch1_data, 0xBBBBu32)] {
        p.extend(pos.to_le_bytes());
        p.extend(36u32.to_le_bytes());
        p.extend(COMPONENTS_LOC_ROT.to_le_bytes());
        let _ = id;
    }
    p.extend(key_bytes(0.0, 5.0, 0.0));
    p.extend(key_bytes(1.0, 9.0, 0.5));

    let specs = vec![spec(SectionType::ANIM, p.clone())];
    let mut data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    // pNames/pInfo 为绝对文件偏移：按真实 section pos 回补
    let base = file.section(0).unwrap().pos as u32;
    p[0..4].copy_from_slice(&(base + _p_names).to_le_bytes());
    p[44..48].copy_from_slice(&(base + p_info).to_le_bytes());
    data[base as usize..base as usize + p.len()].copy_from_slice(&p);
    let file = Rw4File::parse(&data).unwrap();

    let anim = file.decode_anim(&data, 0).unwrap();
    assert_eq!(anim.skeleton_id, 0xFEED);
    assert_eq!(anim.length, 1.5);
    assert_eq!(anim.channels.len(), 2);
    assert_eq!(anim.channels[0].keys.len(), 1);
    let key = anim.channels[0].keys[0];
    assert_eq!(key.tx, 5.0);
    assert_eq!(key.qw, 1.0);
    assert_eq!(key.sx, 1.0, "LocRot 无缩放 → 默认 1");
    assert_eq!(anim.channels[1].keys[0].time, 0.5);
}

#[test]
fn anim_locrotscale_reads_scale_and_pad() {
    let (mut p, _, p_info, p_data) = anim_header(1, 1, 2.0);
    p[20..24].copy_from_slice(&(p_data + 48).to_le_bytes());
    p.extend(0xCCCCu32.to_le_bytes()); // 关节名
    p.extend(p_data.to_le_bytes());
    p.extend(48u32.to_le_bytes());
    p.extend(COMPONENTS_LOC_ROT_SCALE.to_le_bytes());

    let s = std::f32::consts::FRAC_1_SQRT_2;
    for v in [0.0f32, 0.0, s, s] {
        p.extend(v.to_le_bytes());
    }
    for v in [1.0f32, 2.0, 3.0] {
        p.extend(v.to_le_bytes());
    }
    for v in [2.0f32, 2.0, 2.0] {
        p.extend(v.to_le_bytes());
    }
    p.extend(0u32.to_le_bytes());
    p.extend(0.25f32.to_le_bytes());

    let specs = vec![spec(SectionType::ANIM, p.clone())];
    let mut data = build(1, &specs, &[]);
    let file = Rw4File::parse(&data).unwrap();
    let base = file.section(0).unwrap().pos as u32;
    let p_names = 48u32;
    p[0..4].copy_from_slice(&(base + p_names).to_le_bytes());
    p[44..48].copy_from_slice(&(base + p_info).to_le_bytes());
    data[base as usize..base as usize + p.len()].copy_from_slice(&p);
    let file = Rw4File::parse(&data).unwrap();
    let anim = file.decode_anim(&data, 0).unwrap();
    assert_eq!(anim.channels[0].keys.len(), 1);
    let key = anim.channels[0].keys[0];
    let s = std::f32::consts::FRAC_1_SQRT_2;
    assert_eq!((key.qz, key.qw), (s, s));
    assert_eq!(key.sz, 2.0);
    assert_eq!(key.time, 0.25);
}

#[test]
fn math_matches_csharp_reference_paths() {
    let t: crate::math::Mat4 = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 3.0, 4.0, 5.0, 1.0,
    ];
    // X 轴 90° 旋转
    let r: crate::math::Mat4 = [
        1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    let tr = mat4_mul(&t, &r);
    let inv = mat4_inverse(&tr);
    let identity = mat4_mul(&tr, &inv);
    for (i, v) in identity.iter().enumerate() {
        let expected = if i % 5 == 0 { 1.0 } else { 0.0 };
        assert!((v - expected).abs() < 1e-5, "inv*mul[{i}]={v}");
    }
    let (translation, quat, scale) = mat4_decompose_trs(&tr);
    assert_eq!(translation, [3.0, 4.0, 5.0]);
    assert!(
        (quat[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5,
        "X-90° 四元数 {quat:?}"
    );
    assert!((quat[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
    assert!(scale.iter().all(|s| (*s - 1.0).abs() < 1e-6));
}

#[test]
fn shell_rig_marker_is_decoded_from_texcoord1() {
    // 声明：... BLENDWEIGHT@28 UBYTE4 | TEXCOORD1@32 UBYTE4，stride 36
    let mut fmt = vertex_format_payload();
    fmt[12..14].copy_from_slice(&6u16.to_le_bytes()); // 6 组件
    fmt[14..16].copy_from_slice(&36u16.to_be_bytes()); // stride 36
    // 元素4（BLENDWEIGHT）FLOAT4→UBYTE4：元素4 类型字段 @ 24+4*12+3
    fmt[24 + 48 + 3..24 + 48 + 5].copy_from_slice(&5u16.to_be_bytes());
    // 新增元素5：TEXCOORD index1 UBYTE4 @32
    let mut e = Vec::new();
    e.extend(0u8.to_le_bytes());
    e.extend(32u16.to_be_bytes());
    e.extend(5u16.to_be_bytes()); // UBYTE4
    e.extend(5u16.to_be_bytes()); // TEXCOORD
    e.extend(1u8.to_le_bytes()); // index 1
    e.extend(0u8.to_le_bytes());
    e.extend(0u16.to_be_bytes());
    e.extend(0u8.to_le_bytes());
    fmt.extend(e);

    let vertex = |marker: [u8; 4]| {
        let mut v = Vec::new();
        for f in [1.0f32, 0.0, 0.0] {
            v.extend(f.to_le_bytes());
        }
        v.extend([255u8, 128, 0, 1]); // normal
        for f in [0.25f32, 0.75] {
            v.extend(f.to_le_bytes());
        } // uv
        v.extend([0u8, 3, 9, 255]); // blend indices
        v.extend([200u8, 0, 0, 0]); // blendweight(UBYTE4)
        v.extend(marker);
        assert_eq!(v.len(), 36);
        v
    };

    let build_mesh = |marker: [u8; 4]| {
        let specs = vec![
            spec(SectionType::VERTEX_FORMAT, fmt.clone()),
            spec(SectionType::VERTEX_ARRAY, vertex_array_payload(0, 2, 1, 36)),
            spec(SectionType::BLOB, vertex(marker)),
            Spec {
                fixups: vec![(3, 0)],
                ..spec(SectionType::BLOB, triangle_data_payload(&[[0, 0, 0]]))
            },
            spec(SectionType::TRIANGLE_ARRAY, triangle_array_payload(3, 3)),
            spec(SectionType::MESH, mesh_payload(4, 1, 1, 1)),
        ];
        let data = build(1, &specs, &[]);
        let file = Rw4File::parse(&data).unwrap();
        file.decode_mesh(&data, 5).unwrap()
    };

    let moving = build_mesh([64, 0, 191, 255]);
    assert_eq!(moving.vertices[0].shell_marker(), Some([64, 0, 191, 255]));
    assert!(!moving.vertices[0].is_rigid_shell_static());

    let shell = build_mesh([127, 127, 127, 0]);
    assert!(
        shell.vertices[0].is_rigid_shell_static(),
        "(127,127,127,0) = 静态外壳"
    );
}
