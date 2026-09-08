//! glTF 2.0 GLB 导出 —— 对齐 C# `GltfConverter`（几何 + 皮肤 + 动画）。
//!
//! 管线（`ExtractSkin` + `Export` + `BuildJson` + `WriteGlb` 语义）：
//! 1. BIN：POSITION（原始 Z-up，min/max 记录）→ NORMAL → TEXCOORD_0
//!    （FLOAT2 恒可；FLOAT4 仅当 max|uv| ≤ 8，否则 0,0；V 取负）→
//!    UINT32 索引（跳过任两点相等的退化三角形）
//!    另：FLOAT4 TexCoord 存在时导出 TEXCOORD_2/3 = facade 世界投影 UV
//!    （xy=Base 层、zw=Top 层，原值直出不取反——tint 着色器 frac(uv) 与
//!    后端 bake 同坐标系）
//! 2. 皮肤：IBM = 存储 bind 矩阵修正第 4 列（[3]=[7]=[11]=0,[15]=1）；
//!    bind-local TRS = `parent<0 ? IBM⁻¹ : IBM[parent]·IBM⁻¹` 再分解；
//!    JOINTS_0 = BLENDINDICES ÷3 + 关节数钳位；WEIGHTS_0 归一（缺失 → [1,0,0,0]）
//! 3. shell-rig 回退（无 BLENDINDICES 且有动画且关节数 ≥2）：TEXCOORD1
//!    UBYTE4 标记 `(127,127,127,0)`=静态 → 关节 0；运动分组质心 → 最近
//!    非根骨骼 pivot（IBM⁻¹ 平移量），刚性绑定权重 1
//! 4. 动画：通道按关节名 FNV 映射到关节（缺失按序号，越界跳过）；
//!    每轨 Times/T/R/(S) 采样器，LINEAR 插值
//! 5. 节点图：根节点带 Z-up→Y-up 的 -90° X 旋转 `[-√½,0,0,√½]`；
//!    蒙皮时根挂 mesh 节点 + 根关节，关节各自带 bind-local TRS
//!
//! JSON 用 serde_json 生成（非移植 C# 手写 chunk），GLB 容器字节布局与
//! C# `WriteGlb` 一致（JSON 块 0x20 填充、BIN 块 0x00 填充）。
//! 可选 baseColor/normal PNG 作为 bufferView 嵌入 GLB，并写入 glTF 材质引用。

use rw4::{DecodedAnim, DecodedMesh, DecodedSkeleton, mat4_decompose_trs, mat4_inverse, mat4_mul};

const GLB_MAGIC: u32 = 0x4654_6C67; // "glTF"
const GLB_VERSION: u32 = 2;
const CHUNK_JSON: u32 = 0x4E4F_534A; // "JSON"
const CHUNK_BIN: u32 = 0x004E_4942; // "BIN\0"

const COMP_FLOAT: u32 = 5126;
const COMP_UINT: u32 = 5125;
const COMP_USHORT: u32 = 5123;
const TARGET_ARRAY: u32 = 34962;
const TARGET_ELEMENT: u32 = 34963;
const MODE_TRIANGLES: u32 = 4;

const SHELL_NEUTRAL_MARKER: u32 = (127 << 24) | (127 << 16) | (127 << 8);

/// 一条采样后的动画轨道（对应 C# `AnimTrack`）。
#[derive(Debug, Clone)]
pub struct AnimTrack {
    pub joint: usize,
    pub times: Vec<f32>,
    pub t: Vec<[f32; 3]>,
    pub r: Vec<[f32; 4]>,
    pub s: Vec<[f32; 3]>,
    pub has_scale: bool,
    pub times_offset: usize,
    pub time_min: f32,
    pub time_max: f32,
    pub t_offset: usize,
    pub r_offset: usize,
    pub s_offset: usize,
}

/// 一段动画剪辑（对应 C# `AnimClip`）。
#[derive(Debug, Clone, Default)]
pub struct AnimClip {
    pub name: String,
    pub tracks: Vec<AnimTrack>,
}

/// 皮肤数据（对应 C# `SkinData`）。
#[derive(Debug, Clone)]
pub struct SkinData {
    pub joint_count: usize,
    pub parent: Vec<i32>,
    pub inverse_bind: Vec<f32>,
    pub node_t: Vec<[f32; 3]>,
    pub node_r: Vec<[f32; 4]>,
    pub node_s: Vec<[f32; 3]>,
    pub v_joints: Vec<u16>,
    pub v_weights: Vec<f32>,
    pub clips: Vec<AnimClip>,
}

/// 从骨骼 + 网格蒙皮分量 + 动画构建皮肤数据；不可解时返回 `None`
///（C# `ExtractSkin` catch → null）。
pub fn extract_skin(
    mesh: &DecodedMesh,
    skeleton: &DecodedSkeleton,
    anims: &[DecodedAnim],
) -> Option<SkinData> {
    let items = &skeleton.hierarchy.joints;
    let n = items.len();
    if n == 0 || skeleton.bind_matrices.len() < n {
        return None;
    }

    // IBM：存储矩阵修正第 4 列
    let mut ibm: Vec<rw4::Mat4> = Vec::with_capacity(n);
    let mut parent = Vec::with_capacity(n);
    for (i, src) in skeleton.bind_matrices.iter().enumerate().take(n) {
        parent.push(items[i].parent);
        let mut ib = *src;
        ib[3] = 0.0;
        ib[7] = 0.0;
        ib[11] = 0.0;
        ib[15] = 1.0;
        ibm.push(ib);
    }

    // bind-local TRS
    let mut node_t = Vec::with_capacity(n);
    let mut node_r = Vec::with_capacity(n);
    let mut node_s = Vec::with_capacity(n);
    for i in 0..n {
        let bind_pose = mat4_inverse(&ibm[i]);
        let local = if parent[i] < 0 {
            bind_pose
        } else {
            mat4_mul(&ibm[parent[i] as usize], &bind_pose)
        };
        let (t, r, s) = mat4_decompose_trs(&local);
        node_t.push(t);
        node_r.push(r);
        node_s.push(s);
    }

    // 逐顶点 joints/weights
    let v_count = mesh.vertices.len();
    let mut v_joints = vec![0u16; v_count * 4];
    let mut v_weights = vec![0f32; v_count * 4];
    let mut mesh_has_blend = false;
    for (v, vertex) in mesh.vertices.iter().enumerate() {
        let mut idx = [0u16; 4];
        let mut wt = [0f32; 4];
        let mut got_idx = false;
        let mut got_wt = false;
        for (element, value) in &vertex.components {
            use rw4::DeclarationUsage::*;
            match element.usage {
                BlendIndices => {
                    if let Some(raw) = value.as_quad_u() {
                        for k in 0..4 {
                            let mut j = (raw[k] / 3) as u16;
                            if u32::from(j) >= n as u32 {
                                j = 0;
                            }
                            idx[k] = j;
                        }
                        got_idx = true;
                    }
                }
                BlendWeight => {
                    if let Some(w) = value.as_quad_f() {
                        wt = w;
                        got_wt = true;
                    }
                }
                _ => {}
            }
        }
        if got_idx {
            mesh_has_blend = true;
        }
        if !got_idx {
            idx = [0, 0, 0, 0];
        }
        if !got_wt {
            wt = [1.0, 0.0, 0.0, 0.0];
        }
        let sum = wt[0] + wt[1] + wt[2] + wt[3];
        let (wt0, sum) = if sum <= 0.0001 {
            (1.0, 1.0)
        } else {
            (wt[0], sum)
        };
        wt[0] = wt0;
        for k in 0..4 {
            v_joints[v * 4 + k] = idx[k];
            v_weights[v * 4 + k] = wt[k] / sum;
        }
    }

    // 无标准蒙皮 → shell-rig 刚体回退（需有动画）
    if !mesh_has_blend {
        let has_anim = anims.iter().any(|a| !a.channels.is_empty());
        let recovered = if has_anim {
            try_build_rigid_shell_skin(mesh, &ibm, n)
        } else {
            None
        };
        let (sj, sw) = recovered?;
        v_joints = sj;
        v_weights = sw;
    }

    // 动画轨道
    let mut by_fnv = std::collections::HashMap::new();
    for (i, item) in items.iter().enumerate() {
        by_fnv.entry(item.name_fnv).or_insert(i);
    }
    let mut clips = Vec::new();
    for (clip_no, anim) in anims.iter().enumerate() {
        let mut clip = AnimClip {
            name: format!("anim{clip_no}"),
            tracks: Vec::new(),
        };
        for (ci, ch) in anim.channels.iter().enumerate() {
            if ch.keys.is_empty() {
                continue;
            }
            let joint = match by_fnv.get(&ch.id) {
                Some(&j) => j,
                None if ci < n => ci,
                _ => continue,
            };
            let kc = ch.keys.len();
            let mut track = AnimTrack {
                joint,
                times: Vec::with_capacity(kc),
                t: Vec::with_capacity(kc),
                r: Vec::with_capacity(kc),
                s: Vec::with_capacity(kc),
                has_scale: ch.components == rw4::COMPONENTS_LOC_ROT_SCALE,
                times_offset: 0,
                time_min: 0.0,
                time_max: 0.0,
                t_offset: 0,
                r_offset: 0,
                s_offset: 0,
            };
            for key in &ch.keys {
                track.times.push(key.time);
                track.t.push([key.tx, key.ty, key.tz]);
                track.r.push([key.qx, key.qy, key.qz, key.qw]);
                track.s.push([key.sx, key.sy, key.sz]);
            }
            clip.tracks.push(track);
        }
        if !clip.tracks.is_empty() {
            clips.push(clip);
        }
    }

    Some(SkinData {
        joint_count: n,
        parent,
        inverse_bind: ibm.iter().flat_map(|m| m.iter().copied()).collect(),
        node_t,
        node_r,
        node_s,
        v_joints,
        v_weights,
        clips,
    })
}

/// shell-rig 刚体蒙皮恢复（C# `TryBuildRigidShellSkin`）。
fn try_build_rigid_shell_skin(
    mesh: &DecodedMesh,
    ibm: &[rw4::Mat4],
    n: usize,
) -> Option<(Vec<u16>, Vec<f32>)> {
    if n < 2 {
        return None; // 至少要有一个非根骨骼
    }
    // 非根骨骼的全局 bind pivot = IBM⁻¹ 的平移量
    let pivot: Vec<[f32; 3]> = (0..n)
        .map(|i| {
            let bp = mat4_inverse(&ibm[i]);
            [bp[12], bp[13], bp[14]]
        })
        .collect();

    let v_count = mesh.vertices.len();
    let mut marker_key = vec![SHELL_NEUTRAL_MARKER; v_count];
    let mut pos = vec![[0f32; 3]; v_count];
    let mut has_marker = false;
    for (v, vertex) in mesh.vertices.iter().enumerate() {
        let mut key = SHELL_NEUTRAL_MARKER;
        for (element, value) in &vertex.components {
            use rw4::DeclarationUsage::*;
            match element.usage {
                Position => {
                    if let rw4::ComponentValue::Float3(p) = value {
                        pos[v] = *p;
                    }
                }
                TexCoord => {
                    if let rw4::ComponentValue::UByte4(b) = value {
                        key = (u32::from(b[0]) << 24)
                            | (u32::from(b[1]) << 16)
                            | (u32::from(b[2]) << 8)
                            | u32::from(b[3]);
                        has_marker = true;
                    }
                }
                _ => {}
            }
        }
        marker_key[v] = key;
    }
    if !has_marker {
        return None; // 无标记通道
    }

    // 运动分组质心
    let mut sum: std::collections::HashMap<u32, [f64; 4]> = std::collections::HashMap::new();
    for (v, key) in marker_key.iter().enumerate() {
        if *key == SHELL_NEUTRAL_MARKER {
            continue;
        }
        let a = sum.entry(*key).or_insert([0.0; 4]);
        a[0] += f64::from(pos[v][0]);
        a[1] += f64::from(pos[v][1]);
        a[2] += f64::from(pos[v][2]);
        a[3] += 1.0;
    }
    if sum.is_empty() {
        return None;
    }

    // 每个运动组 → 最近非根骨骼 pivot
    let mut group_bone: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    for (key, a) in &sum {
        let (cx, cy, cz) = (a[0] / a[3], a[1] / a[3], a[2] / a[3]);
        let mut best = 1usize;
        let mut best_d = f64::MAX;
        for (j, p) in pivot.iter().enumerate().skip(1) {
            let (dx, dy, dz) = (
                f64::from(p[0]) - cx,
                f64::from(p[1]) - cy,
                f64::from(p[2]) - cz,
            );
            let d = dx * dx + dy * dy + dz * dz;
            if d < best_d {
                best_d = d;
                best = j;
            }
        }
        group_bone.insert(*key, best);
    }

    // 逐顶点：静态 → 根关节 0；运动 → 组骨骼，权重 1
    let mut v_joints = vec![0u16; v_count * 4];
    let mut v_weights = vec![0f32; v_count * 4];
    for v in 0..v_count {
        let j = if marker_key[v] != SHELL_NEUTRAL_MARKER {
            group_bone.get(&marker_key[v]).copied().unwrap_or(0)
        } else {
            0
        };
        v_joints[v * 4] = j as u16;
        v_weights[v * 4] = 1.0;
    }
    Some((v_joints, v_weights))
}

/// GLB 导出结果。
pub struct GlbOutput {
    pub bytes: Vec<u8>,
}

/// Export a mesh as GLB using the legacy no-texture behavior.
pub fn export_glb(
    mesh: &DecodedMesh,
    skeleton: Option<&DecodedSkeleton>,
    anims: &[DecodedAnim],
) -> GlbOutput {
    export_glb_with_colors(mesh, skeleton, anims, EmbeddedTextures::default(), None, None)
}

/// Optional PNG images embedded in the GLB BIN chunk.
#[derive(Debug, Clone, Copy, Default)]
pub struct EmbeddedTextures<'a> {
    pub base_color_png: Option<&'a [u8]>,
    pub normal_png: Option<&'a [u8]>,
}

/// Export a mesh as GLB, optionally embedding PNG base-color and normal maps.
pub fn export_glb_with_textures(
    mesh: &DecodedMesh,
    skeleton: Option<&DecodedSkeleton>,
    anims: &[DecodedAnim],
    textures: EmbeddedTextures<'_>,
) -> GlbOutput {
    export_glb_with_colors(mesh, skeleton, anims, textures, None, None)
}

/// Export a mesh as GLB with embedded textures and/or per-vertex linear RGB
///（COLOR_0，three.js GLTFLoader 映射为 `color` 顶点属性）。
///
/// `mat_indices`：逐顶点材质索引（D3DCOLOR.G），写入 TEXCOORD_1.x（/255 归一），
/// 供前端 tint 着色器按顶点选 regionXform；TEXCOORD_1.y = 假内景随机种子
/// （D3DCOLOR.B = 游戏 A，/255 归一，5b）。
pub fn export_glb_with_colors(
    mesh: &DecodedMesh,
    skeleton: Option<&DecodedSkeleton>,
    anims: &[DecodedAnim],
    textures: EmbeddedTextures<'_>,
    colors: Option<&[[f32; 3]]>,
    mat_indices: Option<&[f32]>,
) -> GlbOutput {
    let v_count = mesh.vertices.len();

    // UV 可用性：FLOAT2 恒可；FLOAT4 需 max|uv| ≤ 8（facade 世界投影剔除）
    let mut has_uv = false;
    if v_count > 0 && mesh.vertices[0].uv().is_some() {
        let has_float2 = mesh.vertices[0].components.iter().any(|(e, v)| {
            e.usage == rw4::DeclarationUsage::TexCoord
                && matches!(v, rw4::ComponentValue::Float2(_))
        });
        if has_float2 {
            has_uv = true;
        } else {
            let mut mx = 0f32;
            for v in &mesh.vertices {
                if let Some([u, vv]) = v.uv() {
                    mx = mx.max(u.abs().max(vv.abs()));
                }
            }
            has_uv = mx <= 8.0;
        }
    }

    let mut skin = skeleton.and_then(|s| extract_skin(mesh, s, anims));

    // ---- BIN chunk ----
    let mut bin: Vec<u8> = Vec::new();
    let put = |bin: &mut Vec<u8>, bytes: &[u8]| bin.extend_from_slice(bytes);
    let pad4 = |bin: &mut Vec<u8>| {
        while !bin.len().is_multiple_of(4) {
            bin.push(0);
        }
    };

    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for v in &mesh.vertices {
        let p = v.position().unwrap_or([0.0; 3]);
        put_f32s(&mut bin, &p);
        for k in 0..3 {
            min[k] = min[k].min(p[k]);
            max[k] = max[k].max(p[k]);
        }
    }
    let pos_offset = 0usize;
    let pos_len = v_count * 12;

    let norm_offset = bin.len();
    for v in &mesh.vertices {
        let n = v.normal().unwrap_or([0.0; 3]);
        put_f32s(&mut bin, &n);
    }
    let norm_len = v_count * 12;

    let uv_offset = bin.len();
    for v in &mesh.vertices {
        // RW4 的 V 相对 glTF 左上原点取反
        let uv = if has_uv {
            v.uv().unwrap_or([0.0, 0.0])
        } else {
            [0.0, 0.0]
        };
        put_f32s(&mut bin, &[uv[0], -uv[1]]);
    }
    let uv_len = v_count * 8;

    let color_offset = colors.map(|colors| {
        pad4(&mut bin);
        let offset = bin.len();
        for v in 0..v_count {
            let rgb = colors.get(v).copied().unwrap_or([1.0, 1.0, 1.0]);
            put_f32s(&mut bin, &rgb);
        }
        offset
    });
    let color_len = v_count * 12;

    // TEXCOORD_1 = (materialIndex/255, interiorSeed/255, 0, 0)：前端 tint
    // 着色器的逐顶点材质索引 + 假内景随机种子（D3DCOLOR.B = 游戏 A，5b）
    let texcoord1_offset = mat_indices.map(|indices| {
        pad4(&mut bin);
        let offset = bin.len();
        for v in 0..v_count {
            let m = indices.get(v).copied().unwrap_or(0.0) / 255.0;
            let seed = mesh.vertices[v]
                .components
                .iter()
                .find_map(|(_, val)| match val {
                    rw4::ComponentValue::D3DColor { b, .. } => Some(f32::from(*b) / 255.0),
                    _ => None,
                })
                .unwrap_or(0.0);
            put_f32s(&mut bin, &[m, seed, 0.0, 0.0]);
        }
        offset
    });
    let texcoord1_len = v_count * 16;

    // TEXCOORD_2/3 = facade 世界投影 UV（FLOAT4 TexCoord：xy=Base 层、
    // zw=Top 层）。原值直出不取反：tint 着色器按游戏公式 frac(uv)*regionXform
    // 采样，与后端 bake 同坐标系。此前被 0,0 占位（2026-09-08 修复）。
    let has_facade_uv = v_count > 0
        && mesh.vertices.iter().any(|v| {
            v.components.iter().any(|(e, val)| {
                e.usage == rw4::DeclarationUsage::TexCoord
                    && matches!(val, rw4::ComponentValue::Float4(_))
            })
        });
    let (facade_base_offset, facade_top_offset) = if has_facade_uv {
        let mut facade = |component: usize| -> usize {
            pad4(&mut bin);
            let offset = bin.len();
            for v in &mesh.vertices {
                let f = v
                    .components
                    .iter()
                    .find_map(|(e, val)| {
                        (e.usage == rw4::DeclarationUsage::TexCoord)
                            .then_some(())
                            .and_then(|_| match val {
                                rw4::ComponentValue::Float4(f) => Some(*f),
                                _ => None,
                            })
                    })
                    .unwrap_or([0.0; 4]);
                put_f32s(&mut bin, &[f[component], f[component + 1]]);
            }
            offset
        };
        let base = facade(0);
        let top = facade(2);
        (Some(base), Some(top))
    } else {
        (None, None)
    };
    let facade_base_len = facade_base_offset.map(|_| v_count * 8).unwrap_or(0);
    let facade_top_len = facade_top_offset.map(|_| v_count * 8).unwrap_or(0);

    let idx_offset = bin.len();
    let mut idx_count = 0usize;
    for t in &mesh.triangles {
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        for idx in t {
            put(&mut bin, &(*idx as u32).to_le_bytes());
        }
        idx_count += 3;
    }
    let idx_len = idx_count * 4;

    // skin 缓冲 + 动画关键帧缓冲
    let mut joints_offset = None;
    let mut weights_offset = None;
    let mut ibm_offset = None;
    if let Some(skin) = skin.as_mut() {
        pad4(&mut bin);
        joints_offset = Some(bin.len());
        for j in &skin.v_joints {
            put(&mut bin, &j.to_le_bytes());
        }
        pad4(&mut bin);
        weights_offset = Some(bin.len());
        for w in &skin.v_weights {
            put(&mut bin, &w.to_le_bytes());
        }
        pad4(&mut bin);
        ibm_offset = Some(bin.len());
        put(&mut bin, bytemuck_cast_f32(&skin.inverse_bind));

        for clip in &mut skin.clips {
            for tr in &mut clip.tracks {
                pad4(&mut bin);
                tr.times_offset = bin.len();
                let mut tmin = f32::MAX;
                let mut tmax = f32::MIN;
                for t in &tr.times {
                    put(&mut bin, &t.to_le_bytes());
                    tmin = tmin.min(*t);
                    tmax = tmax.max(*t);
                }
                tr.time_min = tmin;
                tr.time_max = tmax;
                pad4(&mut bin);
                tr.t_offset = bin.len();
                for p in &tr.t {
                    put_f32s(&mut bin, p);
                }
                pad4(&mut bin);
                tr.r_offset = bin.len();
                for q in &tr.r {
                    put_f32s(&mut bin, q);
                }
                if tr.has_scale {
                    pad4(&mut bin);
                    tr.s_offset = bin.len();
                    for s in &tr.s {
                        put_f32s(&mut bin, s);
                    }
                }
            }
        }
    }

    let base_color_offset = textures.base_color_png.map(|png| {
        pad4(&mut bin);
        let offset = bin.len();
        bin.extend_from_slice(png);
        offset
    });
    let normal_offset = textures.normal_png.map(|png| {
        pad4(&mut bin);
        let offset = bin.len();
        bin.extend_from_slice(png);
        offset
    });
    let bin_len = bin.len();
    let json = build_json(
        base_color_offset,
        textures.base_color_png.map_or(0, |png| png.len()),
        normal_offset,
        textures.normal_png.map_or(0, |png| png.len()),
        v_count,
        idx_count,
        pos_offset,
        pos_len,
        norm_offset,
        norm_len,
        uv_offset,
        uv_len,
        color_offset,
        color_len,
        texcoord1_offset,
        texcoord1_len,
        facade_base_offset,
        facade_base_len,
        facade_top_offset,
        facade_top_len,
        idx_offset,
        idx_len,
        bin_len,
        min,
        max,
        skin.as_ref(),
        joints_offset,
        weights_offset,
        ibm_offset,
    );

    GlbOutput {
        bytes: write_glb(json.as_bytes(), &bin),
    }
}

fn bytemuck_cast_f32(v: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast::<u8>(), v.len() * 4) }
}

/// 追加一组 f32（LE）。
fn put_f32s(bin: &mut Vec<u8>, values: &[f32]) {
    for v in values {
        bin.extend_from_slice(&v.to_le_bytes());
    }
}

/// 组装 glTF JSON（serde_json 生成，结构与 C# `BuildJson` 一致）。
#[allow(clippy::too_many_arguments)]
fn build_json(
    base_color_offset: Option<usize>,
    base_color_len: usize,
    normal_offset: Option<usize>,
    normal_len: usize,
    v_count: usize,
    idx_count: usize,
    pos_offset: usize,
    pos_len: usize,
    norm_offset: usize,
    norm_len: usize,
    uv_offset: usize,
    uv_len: usize,
    color_offset: Option<usize>,
    color_len: usize,
    texcoord1_offset: Option<usize>,
    texcoord1_len: usize,
    facade_base_offset: Option<usize>,
    facade_base_len: usize,
    facade_top_offset: Option<usize>,
    facade_top_len: usize,
    idx_offset: usize,
    idx_len: usize,
    buffer_length: usize,
    min: [f32; 3],
    max: [f32; 3],
    skin: Option<&SkinData>,
    joints_offset: Option<usize>,
    weights_offset: Option<usize>,
    ibm_offset: Option<usize>,
) -> String {
    use serde_json::json;

    let mut buffer_views = vec![
        json!({"buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": TARGET_ARRAY}),
        json!({"buffer": 0, "byteOffset": norm_offset, "byteLength": norm_len, "target": TARGET_ARRAY}),
        json!({"buffer": 0, "byteOffset": uv_offset, "byteLength": uv_len, "target": TARGET_ARRAY}),
        json!({"buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": TARGET_ELEMENT}),
    ];
    let mut accessors = vec![
        json!({"bufferView": 0, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC3",
               "min": [min[0], min[1], min[2]], "max": [max[0], max[1], max[2]]}),
        json!({"bufferView": 1, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC3"}),
        json!({"bufferView": 2, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC2"}),
        json!({"bufferView": 3, "componentType": COMP_UINT, "count": idx_count, "type": "SCALAR"}),
    ];

    let mut primitive_attrs = json!({"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2});

    let mut texcoord1_acc = None;
    if let Some(offset) = texcoord1_offset {
        let bv = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": texcoord1_len, "target": TARGET_ARRAY}));
        let acc = accessors.len();
        accessors.push(json!({"bufferView": bv, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC4"}));
        primitive_attrs["TEXCOORD_1"] = json!(acc);
        texcoord1_acc = Some(acc);
    }

    if let Some(offset) = facade_base_offset {
        let bv = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": facade_base_len, "target": TARGET_ARRAY}));
        let acc = accessors.len();
        accessors.push(json!({"bufferView": bv, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC2"}));
        primitive_attrs["TEXCOORD_2"] = json!(acc);
    }
    if let Some(offset) = facade_top_offset {
        let bv = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": facade_top_len, "target": TARGET_ARRAY}));
        let acc = accessors.len();
        accessors.push(json!({"bufferView": bv, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC2"}));
        primitive_attrs["TEXCOORD_3"] = json!(acc);
    }

    if let Some(offset) = color_offset {
        let bv = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": color_len, "target": TARGET_ARRAY}));
        let acc = accessors.len();
        accessors.push(json!({"bufferView": bv, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC3"}));
        primitive_attrs["COLOR_0"] = json!(acc);
    }

    let has_skin = skin.is_some();
    let mut gltf_skin = None;
    let mut nodes: Vec<serde_json::Value> = Vec::new();
    let up_rot = [
        -std::f64::consts::FRAC_1_SQRT_2,
        0.0,
        0.0,
        std::f64::consts::FRAC_1_SQRT_2,
    ];
    if let Some(skin) = skin {
        let bv_j = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": joints_offset, "byteLength": v_count * 8, "target": TARGET_ARRAY}));
        let bv_w = buffer_views.len();
        buffer_views.push(json!({"buffer": 0, "byteOffset": weights_offset, "byteLength": v_count * 16, "target": TARGET_ARRAY}));
        let bv_ibm = buffer_views.len();
        buffer_views.push(
            json!({"buffer": 0, "byteOffset": ibm_offset, "byteLength": skin.joint_count * 64}),
        );
        let joints_acc = accessors.len();
        accessors.push(json!({"bufferView": bv_j, "componentType": COMP_USHORT, "count": v_count, "type": "VEC4"}));
        let weights_acc = accessors.len();
        accessors.push(json!({"bufferView": bv_w, "componentType": COMP_FLOAT, "count": v_count, "type": "VEC4"}));
        let ibm_acc = accessors.len();
        accessors.push(json!({"bufferView": bv_ibm, "componentType": COMP_FLOAT, "count": skin.joint_count, "type": "MAT4"}));
        primitive_attrs["JOINTS_0"] = json!(joints_acc);
        primitive_attrs["WEIGHTS_0"] = json!(weights_acc);

        // 节点图：0 根（Z-up 矫正）、1 mesh、2.. 关节
        let joint_base = 2usize;
        let mut root_children = vec![1usize];
        for (i, p) in skin.parent.iter().enumerate() {
            if *p < 0 {
                root_children.push(joint_base + i);
            }
        }
        nodes = vec![
            json!({"rotation": up_rot, "children": root_children}),
            json!({"mesh": 0, "skin": 0}),
        ];
        for i in 0..skin.joint_count {
            let mut jn = json!({
                "translation": skin.node_t[i],
                "rotation": skin.node_r[i],
                "scale": skin.node_s[i],
                "name": format!("joint{i}"),
            });
            let kids: Vec<usize> = (0..skin.joint_count)
                .filter(|&c| skin.parent[c] == i as i32)
                .map(|c| joint_base + c)
                .collect();
            if !kids.is_empty() {
                jn["children"] = json!(kids);
            }
            nodes.push(jn);
        }
        let joints_list: Vec<usize> = (0..skin.joint_count).map(|i| joint_base + i).collect();
        let first_root = skin.parent.iter().position(|&p| p < 0).unwrap_or(0);
        gltf_skin = Some(json!([{
            "inverseBindMatrices": ibm_acc,
            "joints": joints_list,
            "skeleton": joint_base + first_root,
        }]));
    }

    // 动画：每剪辑一个 glTF animation，采样器喂关节节点的 T/R/(S)
    let mut animations = None;
    if let Some(skin) = skin
        && !skin.clips.is_empty()
    {
        let joint_base = 2usize;
        let mut anim_list = Vec::new();
        for (clip_no, clip) in skin.clips.iter().enumerate() {
            let mut samplers = Vec::new();
            let mut channels = Vec::new();
            for tr in &clip.tracks {
                let node = joint_base + tr.joint;
                let kc = tr.times.len();
                let bv_t = buffer_views.len();
                buffer_views.push(
                    json!({"buffer": 0, "byteOffset": tr.times_offset, "byteLength": kc * 4}),
                );
                let in_acc = accessors.len();
                accessors.push(json!({"bufferView": bv_t, "componentType": COMP_FLOAT, "count": kc, "type": "SCALAR",
                                          "min": [tr.time_min], "max": [tr.time_max]}));
                let mut add_channel = |off: usize, comps: usize, ty: &str, path: &str| {
                    let bv = buffer_views.len();
                    buffer_views.push(
                        json!({"buffer": 0, "byteOffset": off, "byteLength": kc * comps * 4}),
                    );
                    let out_acc = accessors.len();
                    accessors.push(json!({"bufferView": bv, "componentType": COMP_FLOAT, "count": kc, "type": ty}));
                    let si = samplers.len();
                    samplers.push(
                        json!({"input": in_acc, "output": out_acc, "interpolation": "LINEAR"}),
                    );
                    channels.push(json!({"sampler": si, "target": {"node": node, "path": path}}));
                };
                add_channel(tr.t_offset, 3, "VEC3", "translation");
                add_channel(tr.r_offset, 4, "VEC4", "rotation");
                if tr.has_scale {
                    add_channel(tr.s_offset, 3, "VEC3", "scale");
                }
            }
            anim_list.push(json!({
                "name": format!("anim{clip_no}"),
                "samplers": samplers,
                "channels": channels,
            }));
        }
        animations = Some(anim_list);
    }

    let texture_bv_start = buffer_views.len();
    if base_color_offset.is_some() || normal_offset.is_some() {
        if let Some(offset) = base_color_offset {
            buffer_views
                .push(json!({"buffer": 0, "byteOffset": offset, "byteLength": base_color_len}));
        }
        if let Some(offset) = normal_offset {
            buffer_views.push(json!({"buffer": 0, "byteOffset": offset, "byteLength": normal_len}));
        }
    }
    let mut gltf = json!({
        "asset": {"version": "2.0", "generator": "OpenSCP glTF exporter"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "meshes": [{"primitives": [{
            "attributes": primitive_attrs,
            "indices": 3,
            "mode": MODE_TRIANGLES,
        }]}],
        "buffers": [{"byteLength": buffer_length}],
        "bufferViews": buffer_views,
        "accessors": accessors,
    });
    if base_color_offset.is_some() || normal_offset.is_some() {
        let mut images = Vec::new();
        let mut textures = Vec::new();
        let mut material = serde_json::json!({"pbrMetallicRoughness": {}});
        if base_color_offset.is_some() {
            images.push(json!({"bufferView": texture_bv_start, "mimeType": "image/png"}));
            textures.push(json!({"source": images.len() - 1}));
            material["pbrMetallicRoughness"]["baseColorTexture"] =
                json!({"index": textures.len() - 1});
        }
        if normal_offset.is_some() {
            let view = texture_bv_start + if base_color_offset.is_some() { 1 } else { 0 };
            images.push(json!({"bufferView": view, "mimeType": "image/png"}));
            textures.push(json!({"source": images.len() - 1}));
            material["normalTexture"] = json!({"index": textures.len() - 1});
        }
        gltf["images"] = json!(images);
        gltf["textures"] = json!(textures);
        gltf["materials"] = json!([material]);
        gltf["meshes"][0]["primitives"][0]["material"] = json!(0);
    }
    if let Some(nodes) = gltf_skin {
        gltf["skins"] = nodes;
    }
    if has_skin {
        gltf["nodes"] = json!(nodes);
    } else {
        gltf["nodes"] = json!([{"mesh": 0, "rotation": up_rot}]);
    }
    if let Some(anim_list) = animations {
        gltf["animations"] = json!(anim_list);
    }

    serde_json::to_string(&gltf).expect("glTF JSON serialize")
}

/// GLB 容器（C# `WriteGlb` 字节布局）。
fn write_glb(json: &[u8], bin: &[u8]) -> Vec<u8> {
    let json_pad = (4 - json.len() % 4) % 4;
    let bin_pad = (4 - bin.len() % 4) % 4;
    let total = 12 + 8 + json.len() + json_pad + 8 + bin.len() + bin_pad;

    let mut out = Vec::with_capacity(total);
    out.extend(GLB_MAGIC.to_le_bytes());
    out.extend(GLB_VERSION.to_le_bytes());
    out.extend((total as u32).to_le_bytes());
    out.extend(((json.len() + json_pad) as u32).to_le_bytes());
    out.extend(CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(json);
    out.extend(std::iter::repeat_n(0x20, json_pad));
    out.extend(((bin.len() + bin_pad) as u32).to_le_bytes());
    out.extend(CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(bin);
    out.extend(std::iter::repeat_n(0u8, bin_pad));
    out
}
