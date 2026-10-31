//! Workbench LOTM/GLB to a small native RW4 static model.
//!
//! The workbench stores editable geometry as a LOTM envelope containing a
//! standard glTF 2.0 binary per mesh.  The lowering mirrors the game's own
//! building vertex contract so the Property Viewer renders compiled models
//! through the same facade/tint chain as native ones:
//!
//! - every mesh GLB in the container becomes one RW4 mesh (the v1 compiler
//!   took only the first one — user report 2026-10-30);
//! - node transforms are applied when flattening each GLB scene (matching the
//!   viewport's `normalizeLotModelScene`: undeclared roots lose their ∓90°X
//!   strip rotation, declared `y`-up files gain the +90°X correction);
//! - the vertex format is POSITION FLOAT3 + NORMAL FLOAT3 + COLOR D3DCOLOR
//!   (G = tint parameter column) + TEXCOORD0 FLOAT2 (base UV) + TEXCOORD1
//!   FLOAT4 (facade world projection: base.xy + top.zw);
//! - each unique container material becomes one MATERIAL section bound to its
//!   meshes via MeshMaterialAssignment, with texture instance refs supplied by
//!   the caller (rasters are allocated by the package orchestrator).

use super::lotm::LotmDocument;
use rw4::{Rw4File, SectionType, build_file, writer_section};
use std::collections::HashMap;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct GlbPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Base UV (TEXCOORD_0; falls back to a VEC2 TEXCOORD_1).
    pub uvs: Vec<[f32; 2]>,
    /// Facade projection UV per vertex: (base.xy from TEXCOORD_2,
    /// top.zw from TEXCOORD_3); zeros when the streams are absent.
    pub facade_uvs: Vec<[f32; 4]>,
    /// Tint parameter column per vertex (TEXCOORD_1.x × 255; 0 when absent).
    pub tint_column: Vec<u8>,
    /// Interior room seed per vertex (TEXCOORD_1.y × 255; lands in
    /// D3DCOLOR.B, which the GLB re-export maps to TEXCOORD_1.y).
    pub seeds: Vec<u8>,
    /// Tangent per vertex (source TANGENT xyz, node-transformed; the GLB
    /// re-export maps it to TANGENT:VEC4 with w=1 — native models carry
    /// TANGENT0 on 2267/2267 meshes and the tint shader's window interior /
    /// relief chain builds its frame from it).
    pub tangents: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

/// Placement baked into the compiled vertices: `origin` reproduces the
/// viewport's deterministic base translation (horizontal center + base on the
/// ground plane), `transform` is the workbench pseudo-unit WPF row-major
/// matrix applied on top (row-vector convention: p' = p·M + t).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModelPlacement {
    pub origin: [f32; 3],
    pub transform: Option<[f32; 12]>,
}

impl ModelPlacement {
    pub fn apply(&self, point: [f32; 3]) -> [f32; 3] {
        let mut out = [
            point[0] + self.origin[0],
            point[1] + self.origin[1],
            point[2] + self.origin[2],
        ];
        if let Some(m) = &self.transform {
            out = [
                out[0] * m[0] + out[1] * m[3] + out[2] * m[6] + m[9],
                out[0] * m[1] + out[1] * m[4] + out[2] * m[7] + m[10],
                out[0] * m[2] + out[1] * m[5] + out[2] * m[8] + m[11],
            ];
        }
        out
    }
}

/// Bounding-box placement origin: horizontally centered, base on z=0.
/// Mirrors the frontend `measureFootprint` so compiled geometry lands where
/// the viewport showed it.
pub fn placement_from_bounds<'a>(
    primitives: impl IntoIterator<Item = &'a GlbPrimitive>,
) -> ModelPlacement {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let mut found = false;
    for primitive in primitives {
        for p in &primitive.positions {
            found = true;
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
    }
    if !found {
        return ModelPlacement::default();
    }
    ModelPlacement {
        origin: [-(min[0] + max[0]) / 2.0, -(min[1] + max[1]) / 2.0, -min[2]],
        transform: None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledNativeModel {
    pub rw4: Vec<u8>,
    pub primitive_count: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub material_count: usize,
}

/// Compile a workbench LOTM document (all meshes, all materials) into one RW4
/// static model.  `mesh_material[m]` is mesh m's unique material index (the
/// deduped plan from `materials::resolve_materials`) and
/// `unique_texture_instances[i]` carries the six raster instance ids for
/// unique material `i` (slot0..slot5; 0 = unbound).  The viewport's
/// deterministic base translation (horizontal center + base on the ground
/// plane) is derived from the geometry; `transform` is the workbench
/// pseudo-unit WPF row-major matrix baked on top.

/// 2×2 占位 Raster section（NightHouse 0x2F4E681B:06C5470C 内嵌逐字节副本；
/// 原版 10960/11348 个模型文件都带一个 0x20003 Raster，模型加载器的
/// TextureManager 收编路径预期它的存在）。
const PLACEHOLDER_RASTER: [u8; 32] = [
    0x15, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xc0, 0xff, 0x38, 0x01, 0x02, 0x00, 0x02, 0x00,
    0x20, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00,
];

// Native NH collision topology. The game accepts this CollisionMesh section
// as part of the RW4 container contract; omitting it produces a model that
// parses offline but crashes during lot loading.
const NH_COLLISION_MESH: &[u8] = include_bytes!("nh_collision.bin");

/// 头部字节 0x64（word 25）的 NightHouse 实测值。语义未破译（原版 mesh 类
/// 文件呈 0xC0xxxx 形态，非 mesh 类为 0x1B8；加载路径 ReadResourceArena_1 /
/// FixupStart 均不读取）——形状克隆，随同构 section 布局一起照抄。
pub const MERGED_HEADER_W25: u32 = 0xC003_F8;

/// 原版单形状编译：全部源网格合并为最少 mesh（每 mesh ≤65535 顶点，u16
/// 索引约束），只绑定材质 0 的六槽；附 BBox（32B = 2×vec4，引擎缺省回退
/// ±1 盒）与占位 Raster；头 word25 照抄 NightHouse。
/// 普查依据（2026-10-30，11348 个原版模型）：mesh 数 {0:3333,1:6132,2:1877,
/// 3:2,4:4}，材质与 mesh 严格 1:1，p99 尺寸 991KiB / 上限 5.3MB——逐 GLB
/// mesh 一 RW4 mesh 的 v3 形状（商业 105 mesh / 14MB）是孤儿形状，实测
/// 令加载存档崩溃。
pub fn compile_lotm_merged(
    doc: &LotmDocument,
    mesh_material: &[u32],
    unique_texture_instances: &[[u32; 6]],
    transform: Option<[f32; 12]>,
) -> Result<CompiledNativeModel, String> {
    let mut per_mesh_primitives = Vec::with_capacity(doc.meshes.len());
    for glb in &doc.meshes {
        per_mesh_primitives.push(parse_glb_primitives(glb)?);
    }
    if mesh_material.len() < doc.meshes.len() || unique_texture_instances.is_empty() {
        return Err("material plan / slots empty for merged compile".into());
    }
    let head_slots = unique_texture_instances[mesh_material[0] as usize];
    compile_merged_primitives(per_mesh_primitives, head_slots, transform)
}

/// [`compile_lotm_merged`] 的 primitives 层入口（可测试）。
pub fn compile_merged_primitives(
    per_mesh_primitives: Vec<Vec<GlbPrimitive>>,
    head_slots: [u32; 6],
    transform: Option<[f32; 12]>,
) -> Result<CompiledNativeModel, String> {
    let mut placement = placement_from_bounds(per_mesh_primitives.iter().flatten());
    placement.transform = transform;

    // 合并分区：贪心装箱，源网格不拆分；单网格超限才报错。
    let mut chunks: Vec<Vec<&GlbPrimitive>> = vec![Vec::new()];
    let mut chunk_verts = 0usize;
    for prims in &per_mesh_primitives {
        for prim in prims {
            let n = prim.positions.len();
            if chunk_verts + n > u16::MAX as usize && !chunks.last().unwrap().is_empty() {
                chunks.push(Vec::new());
                chunk_verts = 0;
            }
            chunk_verts += n;
            chunks.last_mut().unwrap().push(prim);
        }
    }
    let chunk_material = vec![0u32; chunks.len()];
    let mut compiled = compile_primitives_partitioned(
        &chunks,
        &chunk_material,
        std::slice::from_ref(&head_slots),
        placement,
    )?;
    append_native_shape_sections(&mut compiled.rw4, placement, &chunks)?;
    compiled.material_count = 1;
    Ok(compiled)
}

/// 在已编译 RW4 上追加 BBox 与占位 Raster section，并写头 word25。
/// 直接在字节层补 section 会破坏偏移——这里经 writer 重建：解析现有
/// section（payload 抽出），追加 BBox/Raster spec 后整体重排。
fn append_native_shape_sections(
    rw4: &mut Vec<u8>,
    placement: ModelPlacement,
    chunks: &[Vec<&GlbPrimitive>],
) -> Result<(), String> {
    let file = Rw4File::parse(rw4).map_err(|e| format!("merged RW4 parse failed: {e}"))?;
    let mut specs: Vec<rw4::writer::SectionSpec> = Vec::new();
    let mut numbers: Vec<u32> = Vec::new();
    for section in file.sections() {
        let payload = file
            .payload(rw4, section.number)
            .map_err(|e| format!("merged RW4 payload: {e}"))?
            .to_vec();
        specs.push(rw4::writer::SectionSpec {
            type_code: section.type_code,
            alignment: section.alignment,
            payload,
            fixups: Vec::new(),
        });
        numbers.push(section.number);
    }
    // BBox：placement 后几何的全局 min/max（2×vec4，32B）。
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for chunk in chunks {
        for prim in chunk.iter() {
            for p in &prim.positions {
                let v = placement.apply(*p);
                for c in 0..3 {
                    min[c] = min[c].min(v[c]);
                    max[c] = max[c].max(v[c]);
                }
            }
        }
    }
    let mut bbox = Vec::with_capacity(32);
    for value in [min[0], min[1], min[2], 0.0, max[0], max[1], max[2], 0.0] {
        bbox.extend_from_slice(&value.to_le_bytes());
    }
    let mut bbox_spec = rw4::writer::section(SectionType::BBOX, bbox);
    bbox_spec.alignment = 16;
    // 只追加、不重排：mesh/VA/TA/MMA payload 内部按 section 编号互引，
    // 前插会让全部编号移位（引擎 IdToObject 与 payload 引用双双失配）。
    let mut all = specs;
    all.push(bbox_spec);
    all.push(rw4::writer::section(
        SectionType::TEXTURE,
        PLACEHOLDER_RASTER.to_vec(),
    ));
    *rw4 = rw4::writer::build(1, &all, &[]);
    // writer 的 BLOB pos 以 sie 为基准；0x20003 占位 Raster 非 BLOB，
    // 走绝对 pos——与 NightHouse 的非 blob Raster 同位。
    let w25 = MERGED_HEADER_W25.to_le_bytes();
    rw4[0x64..0x68].copy_from_slice(&w25);
    Ok(())
}

/// NH（NightHouse/原版主位）顶点布局：stride 40。
/// POSITION F3(12) + NORMAL UBYTE4(4) + TANGENT UBYTE4(4) + COLOR D3DCOLOR(4)
/// + TEXCOORD0 FLOAT4(16, facade base.xy+top.zw)。与原版模型 VF 逐字段同构
/// （压缩法线/切线解包 = (b-127.5)/127.5；census: 2037/2267 原生模型此布局）。
const NH_VERTEX_STRIDE: usize = 40;
const NH_VERTEX_ELEMENTS: usize = 5;

fn nh_vertex_payload(
    positions: &[[f32; 3]],
    normals: &[[f32; 3]],
    tangents: &[[f32; 3]],
    columns: &[u8],
    seeds: &[u8],
    facade: &[[f32; 4]],
    placement: &ModelPlacement,
) -> Vec<u8> {
    let n = positions.len();
    let mut v = Vec::with_capacity(n * NH_VERTEX_STRIDE);
    let compress = |f: f32| -> u8 { (f * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8 };
    for i in 0..n {
        for value in placement.apply(positions[i]) {
            v.extend_from_slice(&value.to_le_bytes());
        }
        let normal = normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
        for c in normal {
            v.push(compress(c));
        }
        v.push(255);
        let tangent = tangents.get(i).copied().unwrap_or([1.0, 0.0, 0.0]);
        for c in tangent {
            v.push(compress(c));
        }
        v.push(255);
        // D3DCOLOR 内存序 (b,g,r,a)：B=seed、G=tint 列（NH 同位）
        v.push(seeds.get(i).copied().unwrap_or(0));
        v.push(0);
        v.push(columns.get(i).copied().unwrap_or(0));
        v.push(0);
        for value in facade.get(i).copied().unwrap_or([0.0; 4]) {
            v.extend_from_slice(&value.to_le_bytes());
        }
    }
    v
}

fn nh_vertex_format_payload() -> Vec<u8> {
    let mut p = vec![0; 12];
    p.extend_from_slice(&(NH_VERTEX_ELEMENTS as u16).to_le_bytes());
    p.extend_from_slice(&(NH_VERTEX_STRIDE as u16).to_be_bytes());
    p.extend_from_slice(&[0; 8]);
    for (offset, ty, usage, index) in [
        (0u16, 2u16, 0u16, 0u8), // POSITION FLOAT3
        (12, 5, 3, 0),           // NORMAL UBYTE4
        (16, 5, 6, 0),           // TANGENT UBYTE4
        (20, 4, 10, 0),          // COLOR D3DCOLOR
        (24, 3, 5, 0),           // TEXCOORD0 FLOAT4 (facade)
    ] {
        p.push(0);
        p.extend_from_slice(&offset.to_be_bytes());
        p.extend_from_slice(&ty.to_be_bytes());
        p.extend_from_slice(&usage.to_be_bytes());
        p.push(index);
        p.extend_from_slice(&[0]);
        p.extend_from_slice(&2u16.to_le_bytes());
        p.push(0);
    }
    p
}

/// NH 布局 + NH 材质原文的合并编译：几何合并 ≤65535 顶点/mesh，
/// 顶点用原生主位格式（压缩法线/切线 + facade F4 单 UV 流），
/// 材质 section 使用调用方提供的原文字节（例如 NH 的真实 CompiledState）。
pub fn compile_merged_primitives_nh(
    per_mesh_primitives: Vec<Vec<GlbPrimitive>>,
    material_payload: Vec<u8>,
    transform: Option<[f32; 12]>,
) -> Result<CompiledNativeModel, String> {
    let mut placement = placement_from_bounds(per_mesh_primitives.iter().flatten());
    placement.transform = transform;

    let mut chunks: Vec<Vec<&GlbPrimitive>> = vec![Vec::new()];
    let mut chunk_verts = 0usize;
    for prims in &per_mesh_primitives {
        for prim in prims {
            let n = prim.positions.len();
            if chunk_verts + n > u16::MAX as usize && !chunks.last().unwrap().is_empty() {
                chunks.push(Vec::new());
                chunk_verts = 0;
            }
            chunk_verts += n;
            chunks.last_mut().unwrap().push(prim);
        }
    }

    let fmt = nh_vertex_format_payload();
    let mut specs = vec![writer_section(SectionType::VERTEX_FORMAT, fmt.clone())];
    let mut mesh_numbers = Vec::new();
    let mut vertex_count = 0usize;
    let mut index_count = 0usize;
    for primitives in chunks.iter().map(Vec::as_slice) {
        let total_vertices: usize = primitives.iter().map(|p| p.positions.len()).sum();
        let total_indices: usize = primitives.iter().map(|p| p.indices.len()).sum();
        let mut vertices = Vec::with_capacity(total_vertices * NH_VERTEX_STRIDE);
        for primitive in primitives {
            for i in 0..primitive.positions.len() {
                for value in placement.apply(primitive.positions[i]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
                let normal = primitive.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                for c in normal {
                    vertices.push((c * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8);
                }
                vertices.push(255); // UBYTE4 第 4 字节填充（NH: 255）
                let tangent = primitive
                    .tangents
                    .get(i)
                    .copied()
                    .unwrap_or([1.0, 0.0, 0.0]);
                for c in tangent {
                    vertices.push((c * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8);
                }
                vertices.push(255);
                // D3DCOLOR 内存序 (b,g,r,a)：B=seed、G=tint 列
                vertices.push(primitive.seeds.get(i).copied().unwrap_or(0));
                vertices.push(0);
                vertices.push(primitive.tint_column.get(i).copied().unwrap_or(0));
                vertices.push(0);
                for value in primitive.facade_uvs.get(i).copied().unwrap_or([0.0; 4]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        let mut indices = Vec::with_capacity(total_indices * 2);
        let mut base: u32 = 0;
        for primitive in primitives {
            for &index in &primitive.indices {
                let value = u16::try_from(index + base).map_err(|_| "NH vertex range overflow")?;
                indices.extend_from_slice(&value.to_le_bytes());
            }
            base += primitive.positions.len() as u32;
        }
        let va = specs.len() as u32;
        specs.push(writer_section(
            SectionType::VERTEX_ARRAY,
            vertex_array_payload(0, va + 1, total_vertices as u32, NH_VERTEX_STRIDE as u32),
        ));
        specs.push(writer_section(SectionType::BLOB, vertices));
        let index_blob = specs.len() as u32;
        specs.push(writer_section(SectionType::BLOB, indices));
        let ta = specs.len() as u32;
        specs.push(writer_section(
            SectionType::TRIANGLE_ARRAY,
            triangle_array_payload(index_blob, total_indices as u32, 101),
        ));
        let mesh = specs.len() as u32;
        specs.push(writer_section(
            SectionType::MESH,
            mesh_payload(ta, (total_indices / 3) as u32, total_vertices as u32, va),
        ));
        mesh_numbers.push(mesh);
        vertex_count += total_vertices;
        index_count += total_indices;
    }
    // 材质 section：调用方原文（NH 的真实 CompiledState）
    let material_base = specs.len() as u32;
    specs.push(rw4::writer::SectionSpec {
        type_code: SectionType::MATERIAL,
        alignment: 16,
        payload: material_payload,
        fixups: Vec::new(),
    });
    for mesh in mesh_numbers.iter().copied() {
        specs.push(writer_section(
            SectionType::MESH_MATERIAL_ASSIGNMENT,
            assignment_payload(mesh, material_base),
        ));
    }
    // BBox + 占位 Raster（R2/R3 同款追加语义）
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for prims in per_mesh_primitives.iter().flatten() {
        for i in 0..prims.positions.len() {
            let v = placement.apply(prims.positions[i]);
            for c in 0..3 {
                min[c] = min[c].min(v[c]);
                max[c] = max[c].max(v[c]);
            }
        }
    }
    let mut bbox = Vec::with_capacity(32);
    for value in [min[0], min[1], min[2], 0.0, max[0], max[1], max[2], 0.0] {
        bbox.extend_from_slice(&value.to_le_bytes());
    }
    let mut bbox_spec = rw4::writer::section(SectionType::BBOX, bbox);
    bbox_spec.alignment = 16;
    specs.push(rw4::writer::section(
        SectionType::COLLISION_MESH,
        NH_COLLISION_MESH.to_vec(),
    ));
    specs.push(bbox_spec);
    specs.push(rw4::writer::section(
        SectionType::TEXTURE,
        PLACEHOLDER_RASTER.to_vec(),
    ));

    let rw4 = rw4::writer::build(1, &specs, &[]);
    let file = Rw4File::parse(&rw4).map_err(|e| format!("NH RW4 parse failed: {e}"))?;
    let mut dv = 0usize;
    let mut di = 0usize;
    for mesh in mesh_numbers.iter().copied() {
        let decoded = file
            .decode_mesh(&rw4, mesh)
            .map_err(|e| format!("NH mesh decode: {e}"))?;
        dv += decoded.vertices.len();
        di += decoded.triangles.len() * 3;
    }
    if dv != vertex_count || di != index_count {
        return Err("NH round-trip count mismatch".into());
    }
    Ok(CompiledNativeModel {
        rw4,
        primitive_count: per_mesh_primitives.iter().map(|m| m.len()).sum(),
        vertex_count,
        index_count,
        material_count: 1,
    })
}

/// 分材质分区的 NH 布局编译：每个唯一材质一组 mesh（组内按 65535 顶点帽
/// 拆分），每组一个 MATERIAL section（NH 顶点布局 stride 40：压缩法线/
/// 切线 + facade F4 单 UV 流）。R7 真实内容包用。
pub fn compile_lotm_nh_partitioned(
    doc: &LotmDocument,
    mesh_material: &[u32],
    unique_instances: &[[u32; 6]],
    transform: Option<[f32; 12]>,
) -> Result<CompiledNativeModel, String> {
    let mut per_mesh = Vec::with_capacity(doc.meshes.len());
    for glb in &doc.meshes {
        per_mesh.push(parse_glb_primitives(glb)?);
    }
    if mesh_material.len() < doc.meshes.len() {
        return Err(format!(
            "material plan covers {} of {} meshes",
            mesh_material.len(),
            doc.meshes.len()
        ));
    }
    let mut placement = placement_from_bounds(per_mesh.iter().flatten());
    placement.transform = transform;

    // 按唯一材质分组（保持首现顺序）。组顶点数 ≤65536 时按 u16 帽拆 chunk
    // （TA 标志 101）；超出则整组单 chunk + u32 索引（TA 标志 102，原版
    // Dalian 塔 105414 顶点先例，2026-10-31 R9 上游戏已验证可渲染）。
    let mut group_totals = vec![0usize; unique_instances.len()];
    for (mesh_index, prims) in per_mesh.iter().enumerate() {
        let m = mesh_material.get(mesh_index).copied().unwrap_or(0) as usize;
        if m >= unique_instances.len() {
            return Err(format!("mesh {mesh_index} material {m} out of range"));
        }
        group_totals[m] += prims.iter().map(|p| p.positions.len()).sum::<usize>();
    }
    let mut groups: Vec<Vec<Vec<&GlbPrimitive>>> = vec![Vec::new(); unique_instances.len()];
    let mut order: Vec<usize> = Vec::new();
    for (mesh_index, prims) in per_mesh.iter().enumerate() {
        let m = mesh_material.get(mesh_index).copied().unwrap_or(0) as usize;
        if groups[m].is_empty() {
            order.push(m);
        }
        let wide = group_totals[m] > u16::MAX as usize + 1;
        for prim in prims {
            let group = &mut groups[m];
            let last_verts: usize = group
                .last()
                .map(|c| c.iter().map(|p| p.positions.len()).sum())
                .unwrap_or(0);
            let cap = if wide {
                u32::MAX as usize
            } else {
                u16::MAX as usize
            };
            if group.is_empty() || last_verts + prim.positions.len() > cap {
                group.push(Vec::new());
            }
            group.last_mut().unwrap().push(prim);
        }
    }

    let fmt = nh_vertex_format_payload();
    let mut specs = vec![writer_section(SectionType::VERTEX_FORMAT, fmt.clone())];
    let mut mesh_material_pairs: Vec<(u32, u32)> = Vec::new();
    let mut vertex_count = 0usize;
    let mut index_count = 0usize;
    for m in order {
        for chunk in groups[m].clone() {
            let total_vertices: usize = chunk.iter().map(|p| p.positions.len()).sum();
            let total_indices: usize = chunk.iter().map(|p| p.indices.len()).sum();
            let mut vertices = Vec::with_capacity(total_vertices * NH_VERTEX_STRIDE);
            for primitive in chunk.iter() {
                for i in 0..primitive.positions.len() {
                    for value in placement.apply(primitive.positions[i]) {
                        vertices.extend_from_slice(&value.to_le_bytes());
                    }
                    let normal = primitive.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]);
                    for c in normal {
                        vertices.push((c * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8);
                    }
                    vertices.push(255);
                    let tangent = primitive
                        .tangents
                        .get(i)
                        .copied()
                        .unwrap_or([1.0, 0.0, 0.0]);
                    for c in tangent {
                        vertices.push((c * 127.5 + 127.5).round().clamp(0.0, 255.0) as u8);
                    }
                    vertices.push(255);
                    vertices.push(primitive.seeds.get(i).copied().unwrap_or(0));
                    vertices.push(0);
                    vertices.push(primitive.tint_column.get(i).copied().unwrap_or(0));
                    vertices.push(0);
                    for value in primitive.facade_uvs.get(i).copied().unwrap_or([0.0; 4]) {
                        vertices.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
            let wide = group_totals[m] > u16::MAX as usize + 1;
            let index_format: u32 = if wide { 102 } else { 101 };
            let index_bytes = if wide { 4 } else { 2 };
            let mut indices = Vec::with_capacity(total_indices * index_bytes);
            let mut base: u32 = 0;
            for primitive in chunk.iter() {
                for &index in &primitive.indices {
                    let value = index + base;
                    if wide {
                        indices.extend_from_slice(&value.to_le_bytes());
                    } else {
                        let value = u16::try_from(value).map_err(|_| "NH vertex range overflow")?;
                        indices.extend_from_slice(&value.to_le_bytes());
                    }
                }
                base += primitive.positions.len() as u32;
            }
            let va = specs.len() as u32;
            specs.push(writer_section(
                SectionType::VERTEX_ARRAY,
                vertex_array_payload(0, va + 1, total_vertices as u32, NH_VERTEX_STRIDE as u32),
            ));
            specs.push(writer_section(SectionType::BLOB, vertices));
            let index_blob = specs.len() as u32;
            specs.push(writer_section(SectionType::BLOB, indices));
            let ta = specs.len() as u32;
            specs.push(writer_section(
                SectionType::TRIANGLE_ARRAY,
                triangle_array_payload(index_blob, total_indices as u32, index_format),
            ));
            let mesh = specs.len() as u32;
            specs.push(writer_section(
                SectionType::MESH,
                mesh_payload(ta, (total_indices / 3) as u32, total_vertices as u32, va),
            ));
            mesh_material_pairs.push((mesh, m as u32));
            vertex_count += total_vertices;
            index_count += total_indices;
        }
    }
    let material_base = specs.len() as u32;
    for (m, instances) in unique_instances.iter().enumerate() {
        specs.push(writer_section(
            SectionType::MATERIAL,
            material_payload(&fmt, instances),
        ));
        let _ = m;
    }
    for (mesh, m) in mesh_material_pairs.iter().copied() {
        specs.push(writer_section(
            SectionType::MESH_MATERIAL_ASSIGNMENT,
            assignment_payload(mesh, material_base + m as u32),
        ));
    }
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for prims in per_mesh.iter().flatten() {
        for i in 0..prims.positions.len() {
            let v = placement.apply(prims.positions[i]);
            for c in 0..3 {
                min[c] = min[c].min(v[c]);
                max[c] = max[c].max(v[c]);
            }
        }
    }
    let mut bbox = Vec::with_capacity(32);
    for value in [min[0], min[1], min[2], 0.0, max[0], max[1], max[2], 0.0] {
        bbox.extend_from_slice(&value.to_le_bytes());
    }
    let mut bbox_spec = rw4::writer::section(SectionType::BBOX, bbox);
    bbox_spec.alignment = 16;
    specs.push(rw4::writer::section(
        SectionType::COLLISION_MESH,
        NH_COLLISION_MESH.to_vec(),
    ));
    specs.push(bbox_spec);
    specs.push(rw4::writer::section(
        SectionType::TEXTURE,
        PLACEHOLDER_RASTER.to_vec(),
    ));

    let rw4 = rw4::writer::build(1, &specs, &[]);
    let file = Rw4File::parse(&rw4).map_err(|e| format!("NH-partitioned parse failed: {e}"))?;
    let mut dv = 0usize;
    let mut di = 0usize;
    for (mesh, _) in mesh_material_pairs.iter().copied() {
        let decoded = file
            .decode_mesh(&rw4, mesh)
            .map_err(|e| format!("NH-partitioned mesh decode: {e}"))?;
        dv += decoded.vertices.len();
        di += decoded.triangles.len() * 3;
    }
    if dv != vertex_count || di != index_count {
        return Err("NH-partitioned round-trip count mismatch".into());
    }
    Ok(CompiledNativeModel {
        rw4,
        primitive_count: per_mesh.iter().map(|m| m.len()).sum(),
        vertex_count,
        index_count,
        material_count: unique_instances.len(),
    })
}

/// Compile LOTM geometry into the section payloads of an already valid native
/// RW4. The template keeps its header, type table, material, collision mesh,
/// embedded raster and all unknown RenderWare fields; only geometry payloads
/// are replaced. This is the production path for game-facing assets.
pub fn compile_lotm_nh_template(
    doc: &LotmDocument,
    mesh_material: &[u32],
    unique_instances: &[[u32; 6]],
    transform: Option<[f32; 12]>,
    template: &[u8],
) -> Result<CompiledNativeModel, String> {
    let generic = compile_lotm_nh_partitioned(
        doc,
        mesh_material,
        unique_instances,
        transform,
    )?;
    let source = Rw4File::parse(&generic.rw4).map_err(|e| format!("generated RW4 parse: {e}"))?;
    let target = Rw4File::parse(template).map_err(|e| format!("template RW4 parse: {e}"))?;
    let mut by_type: HashMap<u32, Vec<Vec<u8>>> = HashMap::new();
    for section in source.sections() {
        if matches!(section.type_code, SectionType::BLOB | SectionType::VERTEX_ARRAY | SectionType::TRIANGLE_ARRAY | SectionType::MESH | SectionType::BBOX) {
            by_type.entry(section.type_code).or_default().push(
                source.payload(&generic.rw4, section.number).map_err(|e| e.to_string())?.to_vec(),
            );
        }
    }
    let mut replacements = HashMap::new();
    let mut source_to_target = HashMap::new();
    let mut source_by_type = HashMap::<u32, Vec<u32>>::new();
    for section in source.sections() {
        source_by_type.entry(section.type_code).or_default().push(section.number);
    }
    let mut target_by_type = HashMap::<u32, Vec<u32>>::new();
    for section in target.sections() {
        target_by_type.entry(section.type_code).or_default().push(section.number);
    }
    let mut source_ordinals = HashMap::<u32, usize>::new();
    for section in source.sections() {
        let ordinal = source_ordinals.entry(section.type_code).or_insert(0);
        if let Some(target_number) = target_by_type.get(&section.type_code).and_then(|v| v.get(*ordinal)) {
            source_to_target.insert(section.number, *target_number);
        }
        *ordinal += 1;
    }
    let mut ordinal = HashMap::<u32, usize>::new();
    for section in target.sections() {
        if !by_type.contains_key(&section.type_code) { continue; }
        let index = ordinal.entry(section.type_code).or_insert(0);
        let Some(payload) = by_type.get(&section.type_code).and_then(|v| v.get(*index)) else { continue; };
        let mut payload = payload.clone();
        if source_by_type.get(&section.type_code).and_then(|v| v.get(*index)).is_some() {
            remap_geometry_refs(section.type_code, &mut payload, &source_to_target);
            replacements.insert(section.number, payload);
        }
        *index += 1;
    }
    let rw4 = rw4::writer::replace_template_sections(template, &replacements)?;
    let file = Rw4File::parse(&rw4).map_err(|e| format!("template replacement parse: {e}"))?;
    let mesh_count = file.sections_of_type(SectionType::MESH).count();
    if mesh_count == 0 || mesh_count > 4 {
        return Err(format!("template replacement produced {mesh_count} meshes"));
    }
    Ok(CompiledNativeModel { rw4, ..generic })
}

fn remap_geometry_refs(type_code: u32, payload: &mut [u8], map: &HashMap<u32, u32>) {
    let offsets: &[usize] = match type_code {
        SectionType::VERTEX_ARRAY => &[0, 6],
        SectionType::TRIANGLE_ARRAY => &[6],
        SectionType::MESH => &[2, 9],
        SectionType::MESH_MATERIAL_ASSIGNMENT => &[0, 2],
        _ => &[],
    };
    for word in offsets {
        let at = *word * 4;
        if at + 4 > payload.len() { continue; }
        let old = u32::from_le_bytes(payload[at..at + 4].try_into().unwrap());
        if let Some(new) = map.get(&old) { payload[at..at + 4].copy_from_slice(&new.to_le_bytes()); }
    }
}

pub fn compile_lotm(
    doc: &LotmDocument,
    mesh_material: &[u32],
    unique_texture_instances: &[[u32; 6]],
    transform: Option<[f32; 12]>,
) -> Result<CompiledNativeModel, String> {
    let mut per_mesh_primitives = Vec::with_capacity(doc.meshes.len());
    for glb in &doc.meshes {
        per_mesh_primitives.push(parse_glb_primitives(glb)?);
    }
    if mesh_material.len() < doc.meshes.len() {
        return Err(format!(
            "material plan covers {} of {} meshes",
            mesh_material.len(),
            doc.meshes.len()
        ));
    }
    let unique_count = unique_texture_instances.len();
    for (index, material) in mesh_material.iter().enumerate().take(doc.meshes.len()) {
        if (*material as usize) >= unique_count {
            return Err(format!(
                "mesh {index} references material {material} outside the {unique_count} unique materials"
            ));
        }
    }
    let mut placement = placement_from_bounds(per_mesh_primitives.iter().flatten());
    placement.transform = transform;
    compile_primitives_partitioned(
        &per_mesh_primitives
            .iter()
            .map(|prims| prims.iter().collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        &mesh_material[..doc.meshes.len()],
        unique_texture_instances,
        placement,
    )
}

/// Vertex stride: POSITION(12) + NORMAL(12) + COLOR(4) + TEXCOORD0(8) +
/// TEXCOORD1(16) + TANGENT(12).
const VERTEX_STRIDE: usize = 64;
const VERTEX_ELEMENTS: usize = 6;

fn vertex_format_payload() -> Vec<u8> {
    let mut p = vec![0; 12];
    p.extend_from_slice(&(VERTEX_ELEMENTS as u16).to_le_bytes());
    p.extend_from_slice(&(VERTEX_STRIDE as u16).to_be_bytes());
    p.extend_from_slice(&[0; 8]);
    for (offset, ty, usage, index) in [
        (0u16, 2u16, 0u16, 0u8), // POSITION FLOAT3
        (12, 2, 3, 0),           // NORMAL FLOAT3
        (24, 4, 10, 0),          // COLOR D3DCOLOR
        (28, 1, 5, 0),           // TEXCOORD0 FLOAT2
        (36, 3, 5, 1),           // TEXCOORD1 FLOAT4 (facade)
        (52, 2, 6, 0),           // TANGENT FLOAT3
    ] {
        p.push(0);
        p.extend_from_slice(&offset.to_be_bytes());
        p.extend_from_slice(&ty.to_be_bytes());
        p.extend_from_slice(&usage.to_be_bytes());
        p.push(index);
        p.extend_from_slice(&[0]);
        p.extend_from_slice(&0u16.to_be_bytes());
        p.push(0);
    }
    p
}

#[allow(clippy::too_many_arguments)]
fn vertex_array_payload(fmt: u32, data: u32, count: u32, size: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&(fmt as i32).to_le_bytes());
    p.extend_from_slice(&[0; 8]);
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&8u32.to_le_bytes());
    p.extend_from_slice(&size.to_le_bytes());
    p.extend_from_slice(&(data as i32).to_le_bytes());
    p
}
fn triangle_array_payload(data: u32, count: u32, index_format: u32) -> Vec<u8> {
    let mut p = vec![0; 8];
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&8u32.to_le_bytes());
    // 索引宽度标志（原版 Dalian 塔 105414 顶点先例）：101 = u16，102 = u32。
    p.extend_from_slice(&index_format.to_le_bytes());
    p.extend_from_slice(&4u32.to_le_bytes());
    p.extend_from_slice(&(data as i32).to_le_bytes());
    p
}
fn mesh_payload(tri: u32, count: u32, verts: u32, vertex: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&40u32.to_le_bytes());
    p.extend_from_slice(&4u32.to_le_bytes());
    p.extend_from_slice(&(tri as i32).to_le_bytes());
    p.extend_from_slice(&count.to_le_bytes());
    p.extend_from_slice(&1u32.to_le_bytes());
    p.extend_from_slice(&0u32.to_le_bytes());
    p.extend_from_slice(&(count * 3).to_le_bytes());
    p.extend_from_slice(&0u32.to_le_bytes());
    p.extend_from_slice(&verts.to_le_bytes());
    p.extend_from_slice(&(vertex as i32).to_le_bytes());
    p
}
fn material_payload(fmt: &[u8], texture_instances: &[u32; 6]) -> Vec<u8> {
    let size = 4 + 28 + 24 + fmt.len() + 7 * 24;
    let mut p = Vec::new();
    p.extend_from_slice(&(size as u32).to_le_bytes());
    p.extend_from_slice(&[0; 28]);
    p.extend_from_slice(fmt);
    p.extend_from_slice(&[0; 24]);
    for (slot, texture) in [
        (0x2d, 0),
        (0, texture_instances[0]),
        (1, texture_instances[1]),
        (2, texture_instances[2]),
        (3, texture_instances[3]),
        (4, texture_instances[4]),
        (5, texture_instances[5]),
    ] {
        p.extend_from_slice(&(slot as u32).to_le_bytes());
        p.extend_from_slice(&[0; 4]);
        p.extend_from_slice(&texture.to_le_bytes());
        p.extend_from_slice(&[0; 12]);
    }
    p
}
fn assignment_payload(mesh: u32, material: u32) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&mesh.to_le_bytes());
    p.extend_from_slice(&1u32.to_le_bytes());
    p.extend_from_slice(&material.to_le_bytes());
    p
}

/// Compile partitioned primitives: `per_mesh[i]` belongs to RW4 mesh i, whose
/// material is `mesh_material[i].material`.  One MATERIAL section is emitted
/// per unique material index.
fn compile_primitives_partitioned(
    per_mesh: &[Vec<&GlbPrimitive>],
    mesh_material: &[u32],
    unique_texture_instances: &[[u32; 6]],
    placement: ModelPlacement,
) -> Result<CompiledNativeModel, String> {
    let total_primitives: usize = per_mesh.iter().map(Vec::len).sum();
    if total_primitives == 0 {
        return Err("no primitive".into());
    }
    let fmt = vertex_format_payload();
    let mut specs = vec![writer_section(SectionType::VERTEX_FORMAT, fmt.clone())];
    let mut mesh_numbers = Vec::new();
    let mut vertex_count = 0usize;
    let mut index_count = 0usize;
    for (_mesh_index, primitives) in per_mesh.iter().enumerate() {
        if primitives.is_empty() {
            continue;
        }
        let total_vertices: usize = primitives.iter().map(|p| p.positions.len()).sum();
        let total_indices: usize = primitives.iter().map(|p| p.indices.len()).sum();
        if total_vertices > u16::MAX as usize {
            return Err("RW4 static mesh supports at most 65535 vertices".into());
        }
        let mut vertices = Vec::with_capacity(total_vertices * VERTEX_STRIDE);
        for primitive in primitives {
            for i in 0..primitive.positions.len() {
                for value in placement.apply(primitive.positions[i]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
                for value in primitive.normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
                // D3DCOLOR memory order is A,R,G,B; the viewer reads G as the
                // tint parameter column and B as the interior seed (the GLB
                // re-export maps them to TEXCOORD_1.xy, matching native
                // building4 meshes).
                let column = primitive.tint_column.get(i).copied().unwrap_or(0);
                let seed = primitive.seeds.get(i).copied().unwrap_or(0);
                vertices.extend_from_slice(&[255, 0, column, seed]);
                for value in primitive.uvs.get(i).copied().unwrap_or([0.0, 0.0]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
                for value in primitive.facade_uvs.get(i).copied().unwrap_or([0.0; 4]) {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
                for value in primitive
                    .tangents
                    .get(i)
                    .copied()
                    .unwrap_or([1.0, 0.0, 0.0])
                {
                    vertices.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        let mut indices = Vec::with_capacity(total_indices * 2);
        let mut base: u32 = 0;
        for primitive in primitives {
            for &index in &primitive.indices {
                let value = u16::try_from(index + base)
                    .map_err(|_| "RW4 static mesh vertex range overflow")?;
                indices.extend_from_slice(&value.to_le_bytes());
            }
            base += primitive.positions.len() as u32;
        }
        let va = specs.len() as u32;
        specs.push(writer_section(
            SectionType::VERTEX_ARRAY,
            vertex_array_payload(0, va + 1, total_vertices as u32, VERTEX_STRIDE as u32),
        ));
        specs.push(writer_section(SectionType::BLOB, vertices));
        let index_blob = specs.len() as u32;
        specs.push(writer_section(SectionType::BLOB, indices));
        let ta = specs.len() as u32;
        specs.push(writer_section(
            SectionType::TRIANGLE_ARRAY,
            triangle_array_payload(index_blob, total_indices as u32, 101),
        ));
        let mesh = specs.len() as u32;
        specs.push(writer_section(
            SectionType::MESH,
            mesh_payload(ta, (total_indices / 3) as u32, total_vertices as u32, va),
        ));
        mesh_numbers.push(mesh);
        vertex_count += total_vertices;
        index_count += total_indices;
    }
    // One MATERIAL section per unique container material, shared by every
    // mesh bound to it (the viewer resolves each section once).
    let material_base = specs.len() as u32;
    for instances in unique_texture_instances {
        specs.push(writer_section(
            SectionType::MATERIAL,
            material_payload(&fmt, instances),
        ));
    }
    for (mesh_index, mesh) in mesh_numbers.iter().copied().enumerate() {
        specs.push(writer_section(
            SectionType::MESH_MATERIAL_ASSIGNMENT,
            assignment_payload(mesh, material_base + mesh_material[mesh_index]),
        ));
    }
    let rw4 = build_file(1, &specs, &[]);
    verify_round_trip(
        &rw4,
        &mesh_numbers,
        vertex_count,
        index_count,
        unique_texture_instances.len(),
    )?;
    Ok(CompiledNativeModel {
        rw4,
        primitive_count: total_primitives,
        vertex_count,
        index_count,
        material_count: unique_texture_instances.len(),
    })
}

fn verify_round_trip(
    rw4: &[u8],
    mesh_numbers: &[u32],
    vertex_count: usize,
    index_count: usize,
    material_count: usize,
) -> Result<(), String> {
    let file = Rw4File::parse(rw4).map_err(|e| format!("compiled RW4 parse failed: {e}"))?;
    let mut decoded_vertices = 0;
    let mut decoded_indices = 0;
    for mesh in mesh_numbers.iter().copied() {
        let decoded = file
            .decode_mesh(rw4, mesh)
            .map_err(|e| format!("compiled RW4 mesh failed: {e}"))?;
        decoded_vertices += decoded.vertices.len();
        decoded_indices += decoded.triangles.len() * 3;
    }
    if decoded_vertices != vertex_count || decoded_indices != index_count {
        return Err("compiled RW4 round-trip count mismatch".into());
    }
    let materials = file
        .sections_of_type(SectionType::MATERIAL)
        .filter(|section| {
            file.decode_material(rw4, section.number)
                .is_ok_and(|m| m.texture_refs().len() == 7)
        })
        .count();
    if materials != material_count {
        return Err(format!(
            "compiled RW4 material count mismatch: {materials} != {material_count}"
        ));
    }
    Ok(())
}

/// Flatten every primitive of one GLB into lot-local Z-up space.
///
/// Frame rules mirror the shared viewport contract (`normalizeLotModelScene`):
/// files that declare `asset.extras.openscp.upAxis` are trusted (declared
/// `y` gains the +90°X correction, declared `z` is kept); undeclared files
/// lose the strip rotation on direct scene children (game gltf.rs exports
/// carry a redundant ∓90°X root rotation on otherwise Z-up data).
pub fn parse_glb_primitives(glb: &[u8]) -> Result<Vec<GlbPrimitive>, String> {
    let (document, bin) = parse_glb_container(glb)?;
    let declared = document
        .pointer("/asset/extras/openscp/upAxis")
        .and_then(Value::as_str)
        .map(|axis| axis == "y");
    let scene_nodes = default_scene_roots(&document)?;
    let mut out = Vec::new();
    for root in scene_nodes {
        // Undeclared files: drop the direct child's rotation (viewport parity);
        // strip only the rotation, keep translation/scale.
        let strip_rotation = declared.is_none();
        flatten_node(
            &document,
            &bin,
            root,
            &LinXform::IDENTITY,
            strip_rotation,
            declared == Some(true),
            &mut out,
        )?;
    }
    if out.is_empty() {
        return Err("GLB has no mesh primitives".into());
    }
    Ok(out)
}

/// 3×3 linear part + translation; column-vector convention with row-major
/// storage (`p' = lin·p + t`).
#[derive(Debug, Clone, Copy)]
struct LinXform {
    lin: [[f32; 3]; 3],
    trans: [f32; 3],
}

impl LinXform {
    const IDENTITY: LinXform = LinXform {
        lin: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        trans: [0.0; 3],
    };

    fn compose(&self, local: &LinXform) -> LinXform {
        let mut lin = [[0f32; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                lin[r][c] = (0..3).map(|k| self.lin[r][k] * local.lin[k][c]).sum();
            }
        }
        let trans = [
            (0..3).map(|k| self.lin[0][k] * local.trans[k]).sum::<f32>() + self.trans[0],
            (0..3).map(|k| self.lin[1][k] * local.trans[k]).sum::<f32>() + self.trans[1],
            (0..3).map(|k| self.lin[2][k] * local.trans[k]).sum::<f32>() + self.trans[2],
        ];
        LinXform { lin, trans }
    }

    fn apply_point(&self, p: [f32; 3]) -> [f32; 3] {
        [
            (0..3).map(|k| self.lin[0][k] * p[k]).sum::<f32>() + self.trans[0],
            (0..3).map(|k| self.lin[1][k] * p[k]).sum::<f32>() + self.trans[1],
            (0..3).map(|k| self.lin[2][k] * p[k]).sum::<f32>() + self.trans[2],
        ]
    }

    fn apply_direction(&self, n: [f32; 3]) -> [f32; 3] {
        let out = [
            (0..3).map(|k| self.lin[0][k] * n[k]).sum::<f32>(),
            (0..3).map(|k| self.lin[1][k] * n[k]).sum::<f32>(),
            (0..3).map(|k| self.lin[2][k] * n[k]).sum::<f32>(),
        ];
        let len = (out[0] * out[0] + out[1] * out[1] + out[2] * out[2]).sqrt();
        if len > f32::EPSILON {
            [out[0] / len, out[1] / len, out[2] / len]
        } else {
            n
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn flatten_node(
    document: &Value,
    bin: &[u8],
    node_index: usize,
    parent: &LinXform,
    strip_rotation: bool,
    y_up_declared: bool,
    out: &mut Vec<GlbPrimitive>,
) -> Result<(), String> {
    let nodes = document
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("GLB nodes missing")?;
    let node = nodes.get(node_index).ok_or("GLB node out of range")?;
    let mut local = node_xform(node, strip_rotation)?;
    if y_up_declared && parent_translates_only(parent) {
        // Declared Y-up contract: rotate the whole file +90°X once, at the
        // scene roots (same point where the viewport applies its correction).
        local = ROT_X_90.compose(&local);
    }
    let world = parent.compose(&local);
    if let Some(mesh_index) = node.get("mesh").and_then(Value::as_u64) {
        let meshes = document
            .get("meshes")
            .and_then(Value::as_array)
            .ok_or("GLB meshes missing")?;
        let mesh = meshes
            .get(mesh_index as usize)
            .ok_or("GLB mesh out of range")?;
        out.extend(read_mesh_primitives(document, mesh, bin, &world)?);
    }
    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            let index = child.as_u64().ok_or("GLB child node index invalid")? as usize;
            flatten_node(document, bin, index, &world, false, y_up_declared, out)?;
        }
    }
    Ok(())
}

/// The +90°X correction applies once per root; roots compose from IDENTITY so
/// a pure-translation parent is the identity sentinel.
fn parent_translates_only(parent: &LinXform) -> bool {
    parent.lin == LinXform::IDENTITY.lin
}

const ROT_X_90: LinXform = LinXform {
    // +90°X: (x, y, z) → (x, -z, y)
    lin: [[1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]],
    trans: [0.0; 3],
};

fn node_xform(node: &Value, strip_rotation: bool) -> Result<LinXform, String> {
    if let Some(matrix) = node.get("matrix").and_then(Value::as_array) {
        let values: Vec<f32> = matrix
            .iter()
            .map(|v| v.as_f64().map(|x| x as f32).unwrap_or_default())
            .collect();
        if values.len() != 16 {
            return Err("GLB node matrix must have 16 floats".into());
        }
        // glTF matrices are column-major 4×4.
        let lin = [
            [values[0], values[4], values[8]],
            [values[1], values[5], values[9]],
            [values[2], values[6], values[10]],
        ];
        let trans = [values[12], values[13], values[14]];
        return Ok(LinXform { lin, trans });
    }
    let scale = read_vec3_field(node, "scale").unwrap_or([1.0; 3]);
    let translation = read_vec3_field(node, "translation").unwrap_or([0.0; 3]);
    let rotation = if strip_rotation {
        None
    } else {
        node.get("rotation").and_then(Value::as_array).map(|q| {
            let v: Vec<f32> = q
                .iter()
                .map(|x| x.as_f64().map(|f| f as f32).unwrap_or_default())
                .collect();
            quaternion_matrix(&v)
        })
    };
    let rotation = rotation.unwrap_or(LinXform::IDENTITY.lin);
    let lin = [
        [
            rotation[0][0] * scale[0],
            rotation[0][1] * scale[1],
            rotation[0][2] * scale[2],
        ],
        [
            rotation[1][0] * scale[0],
            rotation[1][1] * scale[1],
            rotation[1][2] * scale[2],
        ],
        [
            rotation[2][0] * scale[0],
            rotation[2][1] * scale[1],
            rotation[2][2] * scale[2],
        ],
    ];
    Ok(LinXform {
        lin,
        trans: translation,
    })
}

fn read_vec3_field(node: &Value, key: &str) -> Option<[f32; 3]> {
    let values = node.get(key)?.as_array()?;
    Some([
        values.first()?.as_f64()? as f32,
        values.get(1)?.as_f64()? as f32,
        values.get(2)?.as_f64()? as f32,
    ])
}

/// Quaternion (x, y, z, w) → 3×3 rotation matrix.
fn quaternion_matrix(q: &[f32]) -> [[f32; 3]; 3] {
    let [x, y, z, w] = [
        q.first().copied().unwrap_or(0.0),
        q.get(1).copied().unwrap_or(0.0),
        q.get(2).copied().unwrap_or(0.0),
        q.get(3).copied().unwrap_or(1.0),
    ];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    [
        [1.0 - 2.0 * (yy + zz), 2.0 * (xy - wz), 2.0 * (xz + wy)],
        [2.0 * (xy + wz), 1.0 - 2.0 * (xx + zz), 2.0 * (yz - wx)],
        [2.0 * (xz - wy), 2.0 * (yz + wx), 1.0 - 2.0 * (xx + yy)],
    ]
}

fn default_scene_roots(document: &Value) -> Result<Vec<usize>, String> {
    let scenes = document
        .get("scenes")
        .and_then(Value::as_array)
        .ok_or("GLB scenes missing")?;
    let scene_index = document.get("scene").and_then(Value::as_u64).unwrap_or(0) as usize;
    let scene = scenes.get(scene_index).ok_or("GLB default scene missing")?;
    let roots = scene
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("GLB scene nodes missing")?;
    roots
        .iter()
        .map(|v| {
            v.as_u64()
                .map(|i| i as usize)
                .ok_or_else(|| "GLB scene node invalid".to_string())
        })
        .collect()
}

fn parse_glb_container(glb: &[u8]) -> Result<(Value, Vec<u8>), String> {
    if glb.len() < 12 || &glb[..4] != b"glTF" {
        return Err("invalid GLB header".into());
    }
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported GLB version {version}"));
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total > glb.len() || total < 20 {
        return Err("GLB length is truncated".into());
    }
    let mut cursor = 12usize;
    let mut json = None;
    let mut bin = None;
    while cursor + 8 <= total {
        let len = u32::from_le_bytes(glb[cursor..cursor + 4].try_into().unwrap()) as usize;
        let kind = u32::from_le_bytes(glb[cursor + 4..cursor + 8].try_into().unwrap());
        cursor = cursor
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .ok_or("GLB chunk overflow")?;
        if cursor > total {
            return Err("GLB chunk is truncated".into());
        }
        let start = cursor - len;
        match kind {
            0x4E4F534A => json = Some(&glb[start..cursor]),
            0x004E4942 => bin = Some(&glb[start..cursor]),
            _ => {}
        }
    }
    let document: Value = serde_json::from_slice(json.ok_or("GLB JSON chunk missing")?)
        .map_err(|e| format!("invalid GLB JSON: {e}"))?;
    Ok((document, bin.ok_or("GLB BIN chunk missing")?.to_vec()))
}

fn read_mesh_primitives(
    document: &Value,
    mesh: &Value,
    bin: &[u8],
    world: &LinXform,
) -> Result<Vec<GlbPrimitive>, String> {
    let accessors = document
        .get("accessors")
        .and_then(Value::as_array)
        .ok_or("GLB accessors missing")?;
    let views = document
        .get("bufferViews")
        .and_then(Value::as_array)
        .ok_or("GLB bufferViews missing")?;
    let primitives = mesh
        .get("primitives")
        .and_then(Value::as_array)
        .ok_or("mesh primitives missing")?;
    let mut out = Vec::new();
    for primitive in primitives {
        if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
            return Err("only triangle GLB primitives are supported".into());
        }
        let attrs = primitive
            .get("attributes")
            .and_then(Value::as_object)
            .ok_or("primitive attributes missing")?;
        let attr = |name: &str| attrs.get(name).and_then(Value::as_u64).map(|v| v as usize);
        let position_accessor = attr("POSITION").ok_or("primitive POSITION missing")?;
        let positions: Vec<[f32; 3]> =
            read_vec(accessors, views, bin, position_accessor, "POSITION", 3)?
                .into_iter()
                .map(|v| world.apply_point([v[0], v[1], v[2]]))
                .collect();
        let mut normals: Vec<[f32; 3]> = match attr("NORMAL") {
            Some(i) => read_vec(accessors, views, bin, i, "NORMAL", 3)?
                .into_iter()
                .map(|v| world.apply_direction([v[0], v[1], v[2]]))
                .collect(),
            None => Vec::new(),
        };
        if normals.is_empty() {
            normals = vec![[0.0, 0.0, 1.0]; positions.len()];
        }
        let tangents: Vec<[f32; 3]> = match attr("TANGENT") {
            Some(i) => read_vec(accessors, views, bin, i, "TANGENT", 3)?
                .into_iter()
                .map(|v| world.apply_direction([v[0], v[1], v[2]]))
                .collect(),
            None => Vec::new(),
        };
        let texcoord = |name: &str| -> Result<Option<Vec<[f32; 4]>>, String> {
            match attr(name) {
                Some(i) => {
                    let components = accessors
                        .get(i)
                        .and_then(|a| a.get("type"))
                        .and_then(Value::as_str)
                        .unwrap_or("VEC2");
                    let count = match components {
                        "VEC2" => 2,
                        "VEC4" => 4,
                        other => return Err(format!("{name} type {other} unsupported")),
                    };
                    Ok(Some(read_vec(accessors, views, bin, i, name, count)?))
                }
                None => Ok(None),
            }
        };
        let uv0 = texcoord("TEXCOORD_0")?;
        let uv1 = texcoord("TEXCOORD_1")?;
        let uv2 = texcoord("TEXCOORD_2")?;
        let uv3 = texcoord("TEXCOORD_3")?;
        let count = positions.len();
        // Base UV: TEXCOORD_0, else a plain VEC2 TEXCOORD_1 (Blender base
        // domain); a VEC4 TEXCOORD_1 is the tint-column stream instead.
        let base_uv: Vec<[f32; 2]> = match (&uv0, &uv1) {
            (Some(v), _) if v.len() == count && count > 0 => {
                v.iter().map(|u| [u[0], u[1]]).collect()
            }
            (None, Some(v)) if v.len() == count && count > 0 && uv1_is_vec2(accessors, &attr) => {
                v.iter().map(|u| [u[0], u[1]]).collect()
            }
            _ => vec![[0.0, 0.0]; count],
        };
        // Tint column + interior seed: TEXCOORD_1.xy × 255.  Contract GLBs
        // carry a VEC4 stream; plugin-authored containers use VEC2 with the
        // same meaning once TEXCOORD_0 is the base UV.  (When TEXCOORD_0 is
        // absent the VEC2 stream IS the base UV — Blender layout — and there
        // is no column/seed payload.)
        let uv1_is_base = uv1_is_vec2(accessors, &attr) && uv0.is_none();
        let (tint_column, seeds): (Vec<u8>, Vec<u8>) = match (&uv1, uv1_is_base) {
            (Some(v), false) if v.len() == count && count > 0 => (
                v.iter()
                    .map(|u| (u[0] * 255.0).round().clamp(0.0, 255.0) as u8)
                    .collect(),
                v.iter()
                    .map(|u| (u[1] * 255.0).round().clamp(0.0, 255.0) as u8)
                    .collect(),
            ),
            _ => (vec![0; count], vec![0; count]),
        };
        // Facade FLOAT4: (TEXCOORD_2.xy, TEXCOORD_3.xy).
        let facade_uvs: Vec<[f32; 4]> = (0..count)
            .map(|i| {
                let base = uv2
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .map(|u| [u[0], u[1]])
                    .unwrap_or([0.0, 0.0]);
                let top = uv3
                    .as_ref()
                    .and_then(|v| v.get(i))
                    .map(|u| [u[0], u[1]])
                    .unwrap_or([0.0, 0.0]);
                [base[0], base[1], top[0], top[1]]
            })
            .collect();
        let indices = if let Some(i) = primitive.get("indices").and_then(Value::as_u64) {
            read_indices(accessors, views, bin, i as usize)?
        } else {
            (0..count as u32).collect()
        };
        if positions.is_empty() || indices.len() % 3 != 0 {
            return Err("primitive has no vertices or non-triangle indices".into());
        }
        if normals.len() != 0 && normals.len() != count {
            return Err("attribute count does not match POSITION".into());
        }
        out.push(GlbPrimitive {
            positions,
            normals,
            uvs: base_uv,
            facade_uvs,
            tint_column,
            seeds,
            tangents,
            indices,
        });
    }
    Ok(out)
}

/// TEXCOORD_1 accessor component count (VEC2 = base UV stream, VEC4 = tint
/// column stream).  `attr` re-reads the attribute index lazily.
fn uv1_is_vec2(accessors: &[Value], attr: &impl Fn(&str) -> Option<usize>) -> bool {
    attr("TEXCOORD_1")
        .and_then(|i| accessors.get(i))
        .and_then(|a| a.get("type"))
        .and_then(Value::as_str)
        == Some("VEC2")
}

fn accessor_slice<'a>(
    accessors: &[Value],
    views: &[Value],
    bin: &'a [u8],
    index: usize,
    label: &str,
) -> Result<(&'a [u8], usize, usize, usize), String> {
    let accessor = accessors
        .get(index)
        .ok_or_else(|| format!("{label} accessor out of range"))?;
    let view_index = accessor
        .get("bufferView")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} bufferView missing"))? as usize;
    let view = views
        .get(view_index)
        .ok_or_else(|| format!("{label} bufferView out of range"))?;
    if view.get("buffer").and_then(Value::as_u64).unwrap_or(0) != 0 {
        return Err(format!("{label} uses unsupported external buffer"));
    }
    let component = accessor
        .get("componentType")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{label} componentType missing"))? as u32;
    let ty = accessor
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{label} type missing"))?;
    let components = match ty {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return Err(format!("{label} type {ty} unsupported")),
    };
    let component_size = match component {
        5126 | 5125 => 4,
        5123 => 2,
        _ => return Err(format!("{label} component type {component} unsupported")),
    };
    let count = accessor
        .get("count")
        .and_then(Value::as_u64)
        .ok_or("accessor count missing")? as usize;
    let stride = view
        .get("byteStride")
        .and_then(Value::as_u64)
        .unwrap_or((component_size * components) as u64) as usize;
    let start = view.get("byteOffset").and_then(Value::as_u64).unwrap_or(0) as usize
        + accessor
            .get("byteOffset")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
    let need = start
        .checked_add(stride.checked_mul(count.saturating_sub(1)).unwrap_or(0))
        .and_then(|v| v.checked_add(component_size * components))
        .ok_or("accessor overflow")?;
    if need > bin.len() {
        return Err(format!("{label} data truncated"));
    }
    Ok((&bin[start..need], stride, component as usize, components))
}

fn read_vec(
    accessors: &[Value],
    views: &[Value],
    bin: &[u8],
    index: usize,
    label: &str,
    components: usize,
) -> Result<Vec<[f32; 4]>, String> {
    let (data, stride, component, declared) = accessor_slice(accessors, views, bin, index, label)?;
    if component != 5126 {
        return Err(format!("{label} must use FLOAT"));
    }
    if declared < components {
        return Err(format!("{label} component count mismatch"));
    }
    let count = accessors[index]
        .get("count")
        .and_then(Value::as_u64)
        .unwrap() as usize;
    Ok((0..count)
        .map(|i| {
            let p = i * stride;
            let mut out = [0f32; 4];
            for (k, slot) in out.iter_mut().enumerate().take(components) {
                *slot = f32::from_le_bytes(data[p + k * 4..p + k * 4 + 4].try_into().unwrap());
            }
            out
        })
        .collect())
}

fn read_indices(
    accessors: &[Value],
    views: &[Value],
    bin: &[u8],
    index: usize,
) -> Result<Vec<u32>, String> {
    let (data, stride, component, _) = accessor_slice(accessors, views, bin, index, "indices")?;
    let count = accessors[index]
        .get("count")
        .and_then(Value::as_u64)
        .unwrap() as usize;
    (0..count)
        .map(|i| {
            let p = i * stride;
            Ok(match component {
                5123 => u16::from_le_bytes(data[p..p + 2].try_into().unwrap()) as u32,
                5125 => u32::from_le_bytes(data[p..p + 4].try_into().unwrap()),
                _ => return Err("indices must use UNSIGNED_SHORT or UNSIGNED_INT".into()),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Single-material compile used by the unit tests below.
    fn compile_primitives_with_textures(
        primitives: &[GlbPrimitive],
        texture_instances: &[u32; 6],
    ) -> Result<CompiledNativeModel, String> {
        compile_primitives_partitioned(
            &[primitives.iter().collect()],
            &[0],
            std::slice::from_ref(texture_instances),
            ModelPlacement::default(),
        )
    }

    fn triangle_primitive() -> GlbPrimitive {
        GlbPrimitive {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
            facade_uvs: vec![[0.0; 4]; 3],
            tint_column: vec![0, 0, 0],
            seeds: vec![0, 0, 0],
            tangents: vec![[1.0, 0.0, 0.0]; 3],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn merged_compile_produces_native_single_shape() {
        let prims = vec![
            vec![triangle_primitive(), triangle_primitive()],
            vec![triangle_primitive()],
            vec![triangle_primitive()],
        ];
        let model = compile_merged_primitives(prims, [11, 22, 33, 44, 55, 66], None).unwrap();
        // 9 顶点 ≤ 65535 → 单 mesh；材质 1；BBox + 占位 Raster 齐备。
        assert_eq!(model.vertex_count, 12);
        assert_eq!(model.material_count, 1);
        let file = Rw4File::parse(&model.rw4).unwrap();
        let count = |code: u32| file.sections_of_type(code).count();
        assert_eq!(count(SectionType::MESH), 1, "single merged mesh");
        assert_eq!(count(SectionType::MATERIAL), 1, "single material");
        assert_eq!(count(SectionType::MESH_MATERIAL_ASSIGNMENT), 1);
        assert_eq!(
            count(SectionType::BBOX),
            1,
            "BBox present (engine ±1 fallback otherwise)"
        );
        assert_eq!(
            count(SectionType::TEXTURE),
            1,
            "2x2 placeholder raster present"
        );
        // 头 word25（字节 0x64）= NightHouse 形状克隆值。
        assert_eq!(
            &model.rw4[0x64..0x68],
            &MERGED_HEADER_W25.to_le_bytes(),
            "header word25 shape-cloned"
        );
        let types: Vec<String> = file
            .sections()
            .iter()
            .map(|s| format!("{:#x}", s.type_code))
            .collect();
        println!("merged sections: {:?}", types);
        let mesh = file.sections_of_type(SectionType::MESH).next().unwrap();
        let decoded = file.decode_mesh(&model.rw4, mesh.number).unwrap();
        assert_eq!(decoded.vertices.len(), 12);
        assert_eq!(decoded.triangles.len(), 4);
        // BBox 32B = 2×vec4，覆盖 placement 后几何（z 0..1）。
        let bbox = file.sections_of_type(SectionType::BBOX).next().unwrap();
        let payload = file.payload(&model.rw4, bbox.number).unwrap();
        assert_eq!(payload.len(), 32);
        let values: Vec<f32> = payload
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| f32::from_le_bytes(*c))
            .collect();
        assert_eq!(values[0], -0.5);
        assert_eq!(values[4], 0.5);
    }

    #[test]
    fn merged_compile_splits_at_u16_vertex_cap() {
        // 单 primitive 顶点数超 65535 → 拆两个 chunk（两个 mesh，同一材质）。
        let mut big = triangle_primitive();
        big.positions = vec![[0.0, 0.0, 0.0]; 70000];
        big.normals = vec![[0.0, 0.0, 1.0]; 70000];
        big.uvs = vec![[0.0, 0.0]; 70000];
        big.facade_uvs = vec![[0.0; 4]; 70000];
        big.tint_column = vec![0; 70000];
        big.seeds = vec![0; 70000];
        big.tangents = vec![[1.0, 0.0, 0.0]; 70000];
        big.indices = vec![0, 1, 2];
        let model = compile_merged_primitives(vec![vec![big]], [11, 22, 33, 44, 55, 66], None);
        // 单 primitive 无法在 primitive 边界拆分 → 当前策略报错而非静默碎裂。
        assert!(
            model.is_err(),
            "single oversized primitive must fail loudly"
        );
    }

    #[test]
    fn compiles_static_triangle_to_parseable_rw4() {
        let model = compile_primitives_with_textures(
            &[triangle_primitive(), triangle_primitive()],
            &[11, 22, 33, 44, 55, 66],
        )
        .unwrap();
        assert_eq!(model.material_count, 1);
        let file = Rw4File::parse(&model.rw4).unwrap();
        let meshes: Vec<u32> = file
            .sections_of_type(SectionType::MESH)
            .map(|s| s.number)
            .collect();
        assert_eq!(meshes.len(), 1, "one RW4 mesh merges both primitives");
        let decoded = file.decode_mesh(&model.rw4, meshes[0]).unwrap();
        assert_eq!(decoded.triangles, vec![[0, 1, 2], [3, 4, 5]]);
        assert_eq!(decoded.vertices.len(), 6);
        let material = file
            .sections_of_type(SectionType::MATERIAL)
            .next()
            .unwrap()
            .number;
        let material = file.decode_material(&model.rw4, material).unwrap();
        assert_eq!(material.texture_refs()[2].texture_instance, 22);
    }

    #[test]
    fn vertex_format_decodes_native_facade_layout() {
        let model = compile_primitives_with_textures(&[triangle_primitive()], &[0; 6]).unwrap();
        let file = Rw4File::parse(&model.rw4).unwrap();
        let mesh = file
            .sections_of_type(SectionType::MESH)
            .next()
            .unwrap()
            .number;
        let decoded = file.decode_mesh(&model.rw4, mesh).unwrap();
        let vertex = &decoded.vertices[0];
        assert_eq!(vertex.position(), Some([0.0, 0.0, 0.0]));
        assert_eq!(vertex.normal(), Some([0.0, 0.0, 1.0]));
        assert_eq!(vertex.d3d_color_g(), Some(0));
        assert_eq!(vertex.uv(), Some([0.0, 0.0]));
        assert!(vertex.components.iter().any(|(e, v)| {
            e.usage == rw4::DeclarationUsage::TexCoord
                && matches!(v, rw4::ComponentValue::Float4(_))
        }));
    }

    #[test]
    fn tint_column_lands_in_color_g_channel() {
        let mut primitive = triangle_primitive();
        primitive.tint_column = vec![7, 7, 7];
        let model = compile_primitives_with_textures(&[primitive], &[0; 6]).unwrap();
        let file = Rw4File::parse(&model.rw4).unwrap();
        let mesh = file
            .sections_of_type(SectionType::MESH)
            .next()
            .unwrap()
            .number;
        let decoded = file.decode_mesh(&model.rw4, mesh).unwrap();
        assert!(decoded.vertices.iter().all(|v| v.d3d_color_g() == Some(7)));
    }

    #[test]
    fn placement_bakes_origin_and_wpf_transform() {
        let mut primitive = triangle_primitive();
        primitive.positions = vec![[10.0, 0.0, 4.0], [11.0, 0.0, 4.0], [10.0, 1.0, 4.0]];
        let placement = ModelPlacement {
            origin: [-10.0, 0.0, -4.0],
            transform: Some([2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 5.0, 0.0, 0.0]),
        };
        let compiled =
            compile_primitives_partitioned(&[vec![&primitive]], &[0], &[[0; 6]], placement)
                .unwrap();
        let file = Rw4File::parse(&compiled.rw4).unwrap();
        let mesh = file
            .sections_of_type(SectionType::MESH)
            .next()
            .unwrap()
            .number;
        let decoded = file.decode_mesh(&compiled.rw4, mesh).unwrap();
        // ((10,0,4) + (-10,0,-4)) = origin, then scaled ×2 on x and shifted +5.
        assert_eq!(decoded.vertices[0].position(), Some([5.0, 0.0, 0.0]));
    }

    /// Plugin-authored containers carry TEXCOORD_1 as a VEC2 (column, seed)
    /// aux stream next to a TEXCOORD_0 base domain; both must survive the
    /// compile into D3DCOLOR.G/B (the GLB re-export maps them back to
    /// TEXCOORD_1.xy for the tint shader).
    #[test]
    fn vec2_texcoord1_column_and_seed_survive() {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let uv0: [f32; 6] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0];
        // column 0.2 → 51, seed 0.75 → 191
        let uv1: [f32; 6] = [0.2, 0.75, 0.2, 0.75, 0.2, 0.75];
        let indices: [u16; 3] = [0, 1, 2];
        let mut bin = Vec::new();
        let mut view = |bytes: &[u8], bin: &mut Vec<u8>| -> (u32, u32) {
            let offset = bin.len() as u32;
            bin.extend_from_slice(bytes);
            (offset, bytes.len() as u32)
        };
        let f32_bytes =
            |values: &[f32]| -> Vec<u8> { values.iter().flat_map(|v| v.to_le_bytes()).collect() };
        let (pos_off, pos_len) = view(&f32_bytes(&positions), &mut bin);
        let (uv0_off, uv0_len) = view(&f32_bytes(&uv0), &mut bin);
        let (uv1_off, uv1_len) = view(&f32_bytes(&uv1), &mut bin);
        let idx_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
        let (idx_off, idx_len) = view(&idx_bytes, &mut bin);
        let json = serde_json::json!({
            "asset": {"version": "2.0"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"mesh": 0}],
            "meshes": [{"primitives": [{
                "attributes": {"POSITION": 0, "TEXCOORD_0": 1, "TEXCOORD_1": 2},
                "indices": 3
            }]}],
            "accessors": [
                {"componentType": 5126, "count": 3, "type": "VEC3", "bufferView": 0},
                {"componentType": 5126, "count": 3, "type": "VEC2", "bufferView": 1},
                {"componentType": 5126, "count": 3, "type": "VEC2", "bufferView": 2},
                {"componentType": 5123, "count": 3, "type": "SCALAR", "bufferView": 3}
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": pos_off, "byteLength": pos_len},
                {"buffer": 0, "byteOffset": uv0_off, "byteLength": uv0_len},
                {"buffer": 0, "byteOffset": uv1_off, "byteLength": uv1_len},
                {"buffer": 0, "byteOffset": idx_off, "byteLength": idx_len}
            ],
            "buffers": [{"byteLength": bin.len() as u64}]
        });
        let json = serde_json::to_vec(&json).unwrap();
        let total = (12 + 8 + json.len() + 8 + bin.len()) as u32;
        let mut glb = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&total.to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004E4942u32.to_le_bytes());
        glb.extend_from_slice(&bin);
        let primitives = parse_glb_primitives(&glb).unwrap();
        assert_eq!(
            primitives[0].tint_column,
            vec![51, 51, 51],
            "TEXCOORD_1.x carries the tint column"
        );
        assert_eq!(
            primitives[0].seeds,
            vec![191, 191, 191],
            "TEXCOORD_1.y carries the interior seed"
        );
        assert_eq!(primitives[0].uvs[1], [1.0, 0.0], "base UV stays TEXCOORD_0");
        let compiled = compile_primitives_with_textures(&primitives, &[0; 6]).unwrap();
        let file = Rw4File::parse(&compiled.rw4).unwrap();
        let mesh = file
            .sections_of_type(SectionType::MESH)
            .next()
            .unwrap()
            .number;
        let decoded = file.decode_mesh(&compiled.rw4, mesh).unwrap();
        let vertex = &decoded.vertices[0];
        let color = vertex
            .components
            .iter()
            .find_map(|(_, v)| match v {
                rw4::ComponentValue::D3DColor { a, r, g, b } => Some((*a, *r, *g, *b)),
                _ => None,
            })
            .expect("D3DCOLOR present");
        assert_eq!(color, (255, 0, 51, 191), "column+seed baked into COLOR");
    }

    /// A GLT node with translation must move the compiled geometry (the v1
    /// compiler dropped node transforms entirely).
    #[test]
    fn node_transforms_are_applied() {
        let glb = minimal_glb(
            &serde_json::json!({
                "asset": {"version": "2.0"},
                "scene": 0,
                "scenes": [{"nodes": [0]}],
                "nodes": [{"mesh": 0, "translation": [4.0, 5.0, 6.0]}],
                "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
                "accessors": [
                    {"componentType": 5126, "count": 3, "type": "VEC3",
                     "bufferView": 0, "min": [0,0,0], "max": [1,1,0]},
                    {"componentType": 5123, "count": 3, "type": "SCALAR",
                     "bufferView": 1}
                ],
                "bufferViews": [
                    {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                    {"buffer": 0, "byteOffset": 36, "byteLength": 6}
                ],
                "buffers": [{"byteLength": 42}]
            }),
            42,
        );
        let primitives = parse_glb_primitives(&glb).unwrap();
        assert_eq!(primitives[0].positions[0], [4.0, 5.0, 6.0]);
    }

    fn minimal_glb(json: &serde_json::Value, bin_len: usize) -> Vec<u8> {
        let json = serde_json::to_vec(json).unwrap();
        let bin = vec![0u8; bin_len];
        let mut out = b"glTF".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        let total = 12 + 8 + json.len() + 8 + bin.len();
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004E4942u32.to_le_bytes());
        out.extend_from_slice(&bin);
        out
    }
}
