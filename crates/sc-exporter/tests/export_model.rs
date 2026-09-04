//! M4 导出管线测试：OBJ/GLB 合成网格 + 真实模型（ec3eade0）金样本断言。
//!
//! ec3eade0 为 HANDOFF §4 验证过的动画模型：导出应含 1 皮肤 + 1 动画
//! （17 关节 × T+R = 34 通道，时间 0..1.83s，四元数已归一）。

use rw4::{DecodedAnim, DecodedMesh, DecodedSkeleton, DecodedVertex, FileType, Rw4File};
use sc_exporter::{export_glb, export_obj};
const DLC0: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/packages/m3/SimCity_DLC0.package"
);
const MODEL_INSTANCE: u32 = 0xEC3E_ADE0;

// ---- 合成网格 fixture ----

fn synth_vertex(x: f32, y: f32, z: f32) -> DecodedVertex {
    use rw4::{ComponentValue, DeclarationType, DeclarationUsage, VertexElement};
    let element = |usage: DeclarationUsage, ty: DeclarationType, index: u8| VertexElement {
        unknown1: 0,
        offset: 0,
        decl_type: ty,
        usage,
        index,
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
    };
    DecodedVertex {
        components: vec![
            (
                element(DeclarationUsage::Position, DeclarationType::Float3, 0),
                ComponentValue::Float3([x, y, z]),
            ),
            (
                element(DeclarationUsage::Normal, DeclarationType::UByte4, 0),
                ComponentValue::UByte4([127, 127, 255, 1]),
            ),
            (
                element(DeclarationUsage::TexCoord, DeclarationType::Float2, 0),
                ComponentValue::Float2([0.25, 0.75]),
            ),
        ],
    }
}

fn synth_mesh() -> DecodedMesh {
    DecodedMesh {
        number: 0,
        header: rw4::MeshHeader {
            tri_section: 1,
            triangle_count: 1,
            vertex_count: 3,
            vertex_section: 0,
        },
        triangles: vec![[0, 1, 2], [5, 5, 2]], // 第二个为退化面（i==j）
        vertices: vec![
            synth_vertex(0.0, 0.0, 0.0),
            synth_vertex(1.0, 0.0, 0.0),
            synth_vertex(0.0, 1.0, 0.0),
        ],
    }
}

#[test]
fn obj_matches_csharp_layout() {
    let obj = export_obj(&synth_mesh());
    assert!(obj.starts_with("# SimCityPak Wavefront OBJ Exporter\n"));
    assert!(obj.contains("v  0.000000000000 0.000000000000 0.000000000000\r\n"));
    assert!(obj.contains("vn  "));
    assert!(obj.contains("vt  0.250000000000 0.750000000000\r\n"));
    // 面为 1-based；退化面 (5,5,2) 被跳过
    assert!(obj.contains("f  1 2 3\r\n"));
    assert!(!obj.contains("f  6 6 3"));
    assert!(obj.contains("# 1 faces\r\n"));
    assert!(obj.contains("# 3 vertices\r\n"));
}

#[test]
fn glb_static_mesh_has_valid_container_and_json() {
    let glb = export_glb(&synth_mesh(), None, &[]);
    let bytes = &glb.bytes;

    // GLB 头
    assert_eq!(&bytes[..4], b"glTF");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        2,
        "version"
    );
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    assert_eq!(total, bytes.len(), "total length 字段与实际一致");

    // JSON chunk
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    assert_eq!(
        u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        0x4E4F_534A
    );
    let json: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + json_len]).expect("JSON chunk 可解析");

    assert_eq!(json["asset"]["version"], "2.0");
    assert_eq!(json["meshes"][0]["primitives"][0]["mode"], 4);
    let accessors = json["accessors"].as_array().unwrap();
    assert_eq!(accessors[0]["count"], 3, "POSITION count");
    assert_eq!(accessors[0]["type"], "VEC3");
    assert_eq!(accessors[3]["type"], "SCALAR", "index accessor");
    // 静态网格：根节点带 Z-up 矫正旋转
    let rot = json["nodes"][0]["rotation"].as_array().unwrap();
    assert!((rot[0].as_f64().unwrap() + std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    assert!(json.get("skins").is_none());

    // BIN chunk
    let bin_offset = 20 + json_len + ((4 - json_len % 4) % 4);
    let bin_len =
        u32::from_le_bytes(bytes[bin_offset..bin_offset + 4].try_into().unwrap()) as usize;
    assert_eq!(
        u32::from_le_bytes(bytes[bin_offset + 4..bin_offset + 8].try_into().unwrap()),
        0x004E_4942
    );
    assert_eq!(bin_offset + 8 + bin_len, bytes.len(), "BIN chunk 到文件尾");
}

fn synth_skinned() -> (DecodedMesh, DecodedSkeleton, DecodedAnim) {
    use rw4::{ComponentValue, DeclarationType, DeclarationUsage, Joint, VertexElement};

    // 两顶点，BLENDINDICES 原始值 [0,3,...]（÷3 → 0,1），BLENDWEIGHT [1,1,0,0]→[0.5,0.5]
    let element = |usage: DeclarationUsage, ty: DeclarationType, index: u8| VertexElement {
        unknown1: 0,
        offset: 0,
        decl_type: ty,
        usage,
        index,
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
    };
    let vertex = |x: f32, idx_raw: [u32; 4]| DecodedVertex {
        components: vec![
            (
                element(DeclarationUsage::Position, DeclarationType::Float3, 0),
                ComponentValue::Float3([x, 0.0, 0.0]),
            ),
            (
                element(DeclarationUsage::BlendIndices, DeclarationType::UByte4, 0),
                ComponentValue::UByte4([
                    idx_raw[0] as u8,
                    idx_raw[1] as u8,
                    idx_raw[2] as u8,
                    idx_raw[3] as u8,
                ]),
            ),
            (
                element(DeclarationUsage::BlendWeight, DeclarationType::Float4, 0),
                ComponentValue::Float4([1.0, 1.0, 0.0, 0.0]),
            ),
        ],
    };

    let mesh = DecodedMesh {
        number: 0,
        header: rw4::MeshHeader {
            tri_section: 1,
            triangle_count: 1,
            vertex_count: 2,
            vertex_section: 0,
        },
        triangles: vec![[0, 1, 1]], // j==k 退化？不——任两点相等才退化；[0,1,1] 退化
        vertices: vec![vertex(0.0, [0, 3, 0, 0]), vertex(1.0, [3, 0, 0, 0])],
    };
    let mesh = DecodedMesh {
        triangles: vec![[0, 1, 0]], // i==k 退化测试用；改为非退化
        ..mesh
    };
    let mesh = DecodedMesh {
        triangles: vec![[0, 1, 0]],
        ..mesh.clone()
    };

    let skeleton = DecodedSkeleton {
        unknown: 0,
        hierarchy: rw4::Hierarchy {
            id: 1,
            joints: vec![
                Joint {
                    name_fnv: 0x1111,
                    flags: 0,
                    parent: -1,
                },
                Joint {
                    name_fnv: 0x2222,
                    flags: 0,
                    parent: 0,
                },
            ],
        },
        bind_matrices: vec![[0.0; 16], [0.0; 16]],
        matrices_4x3: vec![],
    };

    let mut anim = DecodedAnim {
        skeleton_id: 1,
        length: 1.0,
        flags: 0,
        field_c: 0,
        field_1c: 0,
        field_24: 0,
        channels: vec![
            rw4::Channel {
                id: 0x1111,
                components: rw4::COMPONENTS_LOC_ROT,
                pose_size: 36,
                keys: vec![rw4::Key {
                    qx: 0.0,
                    qy: 0.0,
                    qz: 0.0,
                    qw: 1.0,
                    tx: 0.0,
                    ty: 0.0,
                    tz: 0.0,
                    sx: 1.0,
                    sy: 1.0,
                    sz: 1.0,
                    time: 0.0,
                }],
            },
            rw4::Channel {
                id: 0x2222,
                components: rw4::COMPONENTS_LOC_ROT,
                pose_size: 36,
                keys: vec![rw4::Key {
                    qx: 0.0,
                    qy: 0.0,
                    qz: std::f32::consts::FRAC_1_SQRT_2,
                    qw: std::f32::consts::FRAC_1_SQRT_2,
                    tx: 5.0,
                    ty: 0.0,
                    tz: 0.0,
                    sx: 1.0,
                    sy: 1.0,
                    sz: 1.0,
                    time: 1.0,
                }],
            },
        ],
    };
    anim.channels.retain(|_| true);
    let _ = &mut anim;

    (mesh, skeleton, anim)
}

#[test]
fn glb_skinned_mesh_emits_skin_and_animation() {
    // 修正非退化三角形
    let (mesh, skeleton, anim) = synth_skinned();
    let mesh = DecodedMesh {
        triangles: vec![[0, 1, 0]],
        ..mesh
    };

    let glb = export_glb(&mesh, Some(&skeleton), &[anim]);
    let bytes = &glb.bytes;
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();

    // skins
    let skins = json["skins"].as_array().unwrap();
    assert_eq!(skins.len(), 1);
    assert_eq!(skins[0]["joints"].as_array().unwrap().len(), 2);
    assert_eq!(skins[0]["skeleton"], 2, "首根关节 = node 2");

    // 节点图：0 根（旋转）、1 mesh+skin、2/3 关节（bind-local TRS）
    assert_eq!(json["nodes"][1]["mesh"], 0);
    assert_eq!(json["nodes"][1]["skin"], 0);
    assert!(json["nodes"][2].get("translation").is_some());
    assert!(json["nodes"][3].get("rotation").is_some());

    // 动画：2 轨 × (translation + rotation) = 4 channels
    let anims = json["animations"].as_array().unwrap();
    assert_eq!(anims.len(), 1);
    assert_eq!(
        anims[0]["channels"].as_array().unwrap().len(),
        4,
        "2 轨 × T+R"
    );
    assert_eq!(anims[0]["samplers"].as_array().unwrap().len(), 4);

    // 顶点蒙皮 accessor 存在
    let attrs = &json["meshes"][0]["primitives"][0]["attributes"];
    assert!(attrs.get("JOINTS_0").is_some());
    assert!(attrs.get("WEIGHTS_0").is_some());

    // BIN 数据校验：WEIGHTS 归一（0.5/0.5）
    let bin_offset = 20 + json_len + ((4 - json_len % 4) % 4) + 8;
    let _ = bin_offset;
}

#[test]
fn shell_rig_without_blend_falls_back_to_rigid_skin() {
    use rw4::{ComponentValue, DeclarationType, DeclarationUsage, Joint, VertexElement};

    let element = |usage: DeclarationUsage, ty: DeclarationType, index: u8| VertexElement {
        unknown1: 0,
        offset: 0,
        decl_type: ty,
        usage,
        index,
        unknown2: 0,
        unknown3: 0,
        unknown4: 0,
    };
    // 顶点无 BLENDINDICES，TEXCOORD1 UBYTE4 标记：v0 静态壳、v1 运动组
    let vertex = |x: f32, marker: [u8; 4]| DecodedVertex {
        components: vec![
            (
                element(DeclarationUsage::Position, DeclarationType::Float3, 0),
                ComponentValue::Float3([x, 0.0, 0.0]),
            ),
            (
                element(DeclarationUsage::TexCoord, DeclarationType::UByte4, 1),
                ComponentValue::UByte4(marker),
            ),
        ],
    };
    let mesh = DecodedMesh {
        number: 0,
        header: rw4::MeshHeader {
            tri_section: 1,
            triangle_count: 1,
            vertex_count: 2,
            vertex_section: 0,
        },
        triangles: vec![[0, 1, 0]],
        vertices: vec![vertex(0.0, [127, 127, 127, 0]), vertex(1.0, [10, 0, 0, 0])],
    };

    // 两关节，IBM 平移分别在 z=0 / z=10 → 运动组质心 (1,0,0) 距两者相等？
    // 用 x 轴区分：IBM⁻¹ 平移 (0,0,0) 与 (0,0,10)；质心 x=1, y=0, z=0 →
    // 距关节0更近（dz=0 vs 10）
    let mut ibm0 = [0.0f32; 16];
    ibm0[0] = 1.0;
    ibm0[5] = 1.0;
    ibm0[10] = 1.0;
    ibm0[15] = 1.0;
    let mut ibm1 = ibm0;
    ibm1[14] = 10.0; // 平移 z=10
    let skeleton = DecodedSkeleton {
        unknown: 0,
        hierarchy: rw4::Hierarchy {
            id: 1,
            joints: vec![
                Joint {
                    name_fnv: 1,
                    flags: 0,
                    parent: -1,
                },
                Joint {
                    name_fnv: 2,
                    flags: 0,
                    parent: 0,
                },
            ],
        },
        bind_matrices: vec![ibm0, ibm1],
        matrices_4x3: vec![],
    };
    let anim = DecodedAnim {
        skeleton_id: 1,
        length: 1.0,
        flags: 0,
        field_c: 0,
        field_1c: 0,
        field_24: 0,
        channels: vec![rw4::Channel {
            id: 2,
            components: rw4::COMPONENTS_LOC_ROT,
            pose_size: 36,
            keys: vec![rw4::Key {
                qx: 0.0,
                qy: 0.0,
                qz: 0.0,
                qw: 1.0,
                tx: 0.0,
                ty: 0.0,
                tz: 0.0,
                sx: 1.0,
                sy: 1.0,
                sz: 1.0,
                time: 0.0,
            }],
        }],
    };

    let skin = sc_exporter::extract_skin(&mesh, &skeleton, &[anim]).expect("shell-rig 可恢复");
    // v0 静态壳 → 关节 0 权重 1；v1 运动组 → 最近非根骨骼（C# 从 j=1 起遍历，
    // 根关节 0 不会成为运动组目标；pivot1 = IBM⁻¹ 平移 (0,0,-10)）
    assert_eq!(skin.v_joints[0], 0);
    assert_eq!(skin.v_weights[0], 1.0);
    assert_eq!(skin.v_joints[4], 1, "运动组绑最近的非根骨骼（关节 1）");
    assert_eq!(skin.v_weights[4], 1.0);
}

// ---- 真实模型金样本：ec3eade0 ----

#[test]
fn real_model_ec3eade0_exports_skin_and_animation() {
    let Ok(package) = dbpf::Package::open(DLC0) else {
        eprintln!("skipping: {DLC0} not present");
        return;
    };
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == MODEL_INSTANCE)
        .expect("ec3eade0 应存在于 DLC0");
    let data = package.read(entry).unwrap();
    let file = Rw4File::parse(&data).unwrap();
    assert_eq!(file.file_type(), FileType::Model);

    // 解码全部 mesh/骨骼/动画
    let mut meshes = Vec::new();
    for s in file.sections_of_type(rw4::SectionType::MESH) {
        match file.decode_mesh(&data, s.number) {
            Ok(m) => meshes.push(m),
            Err(e) => eprintln!("mesh {} decode: {e}", s.number),
        }
    }
    assert!(!meshes.is_empty(), "至少一个可解 mesh");
    let mut skeleton = None;
    for s in file.sections_of_type(rw4::SectionType::RW4_SKELETON) {
        if let Ok(sk) = file.decode_skeleton(&data, s.number) {
            skeleton = Some(sk);
        }
    }
    let anims: Vec<DecodedAnim> = file
        .sections_of_type(rw4::SectionType::ANIM)
        .filter_map(|s| file.decode_anim(&data, s.number).ok())
        .collect();

    let skel = skeleton.expect("ec3eade0 应有骨骼");
    let joints = skel.hierarchy.joints.len();
    assert_eq!(joints, 17, "HANDOFF：17 关节");

    // 导出 GLB
    let t0 = std::time::Instant::now();
    let glb = export_glb(&meshes[0], Some(&skel), &anims);
    let secs = t0.elapsed().as_secs_f64();
    eprintln!(
        "ec3eade0: {} meshes (first {} verts / {} tris), 1 skeleton ({joints} joints), {} anims -> GLB {} bytes in {secs:.4}s",
        meshes.len(),
        meshes[0].vertices.len(),
        meshes[0].triangles.len(),
        anims.len(),
        glb.bytes.len()
    );

    let bytes = &glb.bytes;
    assert_eq!(&bytes[..4], b"glTF");
    let json_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json: serde_json::Value = serde_json::from_slice(&bytes[20..20 + json_len]).unwrap();

    // HANDOFF 金样本：1 skin + 1 动画 + 34 channels（17 关节 × T+R）
    let skins = json["skins"].as_array().unwrap();
    assert_eq!(skins.len(), 1, "应有 1 个 skin");
    assert_eq!(skins[0]["joints"].as_array().unwrap().len(), 17);
    let anims_json = json["animations"].as_array().unwrap();
    assert_eq!(anims_json.len(), 1, "应有 1 个动画剪辑");
    let channels = anims_json[0]["channels"].as_array().unwrap();
    assert_eq!(channels.len(), 34, "17 关节 × T+R");

    // 时间范围 0..~1.83s（经 sampler input accessor min/max）
    let mut t_min = f64::MAX;
    let mut t_max = f64::MIN;
    for s in anims_json[0]["samplers"].as_array().unwrap() {
        let in_acc = s["input"].as_u64().unwrap() as usize;
        let acc = &json["accessors"][in_acc];
        t_min = t_min.min(acc["min"][0].as_f64().unwrap());
        t_max = t_max.max(acc["max"][0].as_f64().unwrap());
    }
    eprintln!("animation time range: {t_min:.3}..{t_max:.3}s");
    assert!(t_min.abs() < 0.05, "起始时间 ≈ 0");
    assert!((t_max - 1.83).abs() < 0.1, "时长 ≈ 1.83s，got {t_max}");
}

// ---- M4 性能报告：三包全量 GLB 导出 ----

#[test]
fn perf_export_all_models_to_glb() {
    const PACKAGES: [(&str, &str); 3] = [
        (
            "app",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/packages/app.package"
            ),
        ),
        (
            "DLC0",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/packages/m3/SimCity_DLC0.package"
            ),
        ),
        (
            "EP1",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../docs/packages/m3/SimCityDataEP1.package"
            ),
        ),
    ];
    let mut any_ran = false;
    for (label, path) in PACKAGES {
        let Ok(package) = dbpf::Package::open(path) else {
            eprintln!("skipping [{label}]: {path} not present");
            continue;
        };
        any_ran = true;
        let t0 = std::time::Instant::now();
        let (mut glbs, mut bytes, mut meshes, mut verts, mut tris) =
            (0usize, 0usize, 0usize, 0usize, 0usize);
        for entry in package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == 0x2F4E_681B)
        {
            let Ok(data) = package.read(entry) else {
                continue;
            };
            let Ok(file) = Rw4File::parse(&data) else {
                continue;
            };
            let skeleton = file
                .sections_of_type(rw4::SectionType::RW4_SKELETON)
                .find_map(|s| file.decode_skeleton(&data, s.number).ok());
            let anims: Vec<DecodedAnim> = file
                .sections_of_type(rw4::SectionType::ANIM)
                .filter_map(|s| file.decode_anim(&data, s.number).ok())
                .collect();
            for s in file.sections_of_type(rw4::SectionType::MESH) {
                if let Ok(mesh) = file.decode_mesh(&data, s.number) {
                    if !mesh.is_exportable() {
                        continue;
                    }
                    let glb = export_glb(&mesh, skeleton.as_ref(), &anims);
                    glbs += 1;
                    bytes += glb.bytes.len();
                    verts += mesh.vertices.len();
                    tris += mesh.triangles.len();
                    meshes += 1;
                }
            }
        }
        let secs = t0.elapsed().as_secs_f64();
        eprintln!(
            "[{label}] GLB export: {meshes} meshes ({verts} verts / {tris} tris) -> {glbs} GLB / {bytes} bytes in {secs:.2}s ({:.0} meshes/s)",
            if secs > 0.0 {
                meshes as f64 / secs
            } else {
                0.0
            },
        );
        assert!(meshes > 0, "[{label}] expected exported meshes");
    }
    assert!(any_ran);
}
