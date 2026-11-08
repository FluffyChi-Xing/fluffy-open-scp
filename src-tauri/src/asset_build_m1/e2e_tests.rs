use super::events::StageEventKind;
use super::{build_from_schema, connector, tests::temp_root};
use dbpf::{
    OverlayEntry, Package, ResourceId, write_uncompressed_overlay,
    write_uncompressed_overlay_to_path,
};
use sc_properties::{
    Kind, LotEditorDocument, PropType, Property, PropertyEncoding, PropertyFile, Value,
};
use sha2::Digest;
use std::{fs, path::PathBuf};

// ---- 端到端回归（用户报告 2026-10-30）----
//
// 问题1：v1 编译器把工作台 LOTM v9 容器（多 mesh GLB + 材质）误读成单 GLB
// 信封——只编译第一个 mesh，贴图从未进包。问题2/3：场景文档的手动单元
// （prop bin/路径点）从未写回 Lot property，引用 lot 的原 units 反而原样
// 留在构建产物里。本测试覆盖全链路：容器编译 → 材质/贴图资源 → 单元写回
// → LotSize/LOD 键 lowering。

/// 合成一个 v9 LOTM 容器：两个 mesh GLB（带 facade UV + tint 列）+ 一个
/// 六槽材质（png[4]=tint PNG + f32 参数表）。
fn facade_mesh_glb(offset: f32) -> Vec<u8> {
    let positions: Vec<f32> = [[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 0.0, 4.0]]
        .iter()
        .flat_map(|p| [p[0] + offset, p[1], p[2]])
        .collect();
    let normals: Vec<f32> = [[0.0, 1.0, 0.0]; 3].iter().flat_map(|p| *p).collect();
    let uv0: Vec<f32> = [[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
        .iter()
        .flat_map(|p| *p)
        .collect();
    // TEXCOORD_1.x = 0.5 → tint 参数列 128；TEXCOORD_2/3 = facade 世界投影。
    let uv1: Vec<f32> = [[0.5, 0.0, 0.0, 0.0]; 3].iter().flat_map(|p| *p).collect();
    let uv2: Vec<f32> = [[20.0, 30.0], [21.0, 30.0], [20.0, 31.0]]
        .iter()
        .flat_map(|p| *p)
        .collect();
    let uv3: Vec<f32> = [[40.0, 50.0], [41.0, 50.0], [40.0, 51.0]]
        .iter()
        .flat_map(|p| *p)
        .collect();
    let indices: Vec<u16> = vec![0, 1, 2];
    let mut bin = Vec::new();
    let mut view = |bytes: &[u8], bin: &mut Vec<u8>| -> (u32, u32) {
        let offset = bin.len() as u32;
        bin.extend_from_slice(bytes);
        (offset, bytes.len() as u32)
    };
    let (pos_off, pos_len) = view(f32_bytes(&positions), &mut bin);
    let (nor_off, nor_len) = view(f32_bytes(&normals), &mut bin);
    let (uv0_off, uv0_len) = view(f32_bytes(&uv0), &mut bin);
    let (uv1_off, uv1_len) = view(f32_bytes(&uv1), &mut bin);
    let (uv2_off, uv2_len) = view(f32_bytes(&uv2), &mut bin);
    let (uv3_off, uv3_len) = view(f32_bytes(&uv3), &mut bin);
    let idx_bytes: Vec<u8> = indices.iter().flat_map(|i| i.to_le_bytes()).collect();
    let (idx_off, idx_len) = view(&idx_bytes, &mut bin);
    let json = serde_json::json!({
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [{
            "attributes": {
                "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2,
                "TEXCOORD_1": 3, "TEXCOORD_2": 4, "TEXCOORD_3": 5
            },
            "indices": 6
        }]}],
        "accessors": [
            {"componentType": 5126, "count": 3, "type": "VEC3", "bufferView": 0},
            {"componentType": 5126, "count": 3, "type": "VEC3", "bufferView": 1},
            {"componentType": 5126, "count": 3, "type": "VEC2", "bufferView": 2},
            {"componentType": 5126, "count": 3, "type": "VEC4", "bufferView": 3},
            {"componentType": 5126, "count": 3, "type": "VEC2", "bufferView": 4},
            {"componentType": 5126, "count": 3, "type": "VEC2", "bufferView": 5},
            {"componentType": 5123, "count": 3, "type": "SCALAR", "bufferView": 6}
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": pos_off, "byteLength": pos_len},
            {"buffer": 0, "byteOffset": nor_off, "byteLength": nor_len},
            {"buffer": 0, "byteOffset": uv0_off, "byteLength": uv0_len},
            {"buffer": 0, "byteOffset": uv1_off, "byteLength": uv1_len},
            {"buffer": 0, "byteOffset": uv2_off, "byteLength": uv2_len},
            {"buffer": 0, "byteOffset": uv3_off, "byteLength": uv3_len},
            {"buffer": 0, "byteOffset": idx_off, "byteLength": idx_len}
        ],
        "buffers": [{"byteLength": bin.len() as u64}]
    });
    let json = serde_json::to_vec(&json).unwrap();
    let total = (12 + 8 + json.len() + 8 + bin.len()) as u32;
    let mut out = b"glTF".to_vec();
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x4E4F534Au32.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004E4942u32.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

fn f32_bytes(values: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}

fn tiny_png_bytes() -> Vec<u8> {
    let rgba = vec![90u8; 4 * 4 * 4];
    let image = image::RgbaImage::from_raw(4, 4, rgba).unwrap();
    let mut png = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    png
}

fn workbench_lotm() -> Vec<u8> {
    workbench_lotm_with_meshes(2)
}

/// 参数化网格数的 LOTM：G1 材质指纹闸门测试用它制造受控模型变更。
pub(crate) fn workbench_lotm_with_meshes(meshes: u32) -> Vec<u8> {
    let mut bytes = b"LOTM".to_vec();
    bytes.extend_from_slice(&9u32.to_le_bytes());
    bytes.extend_from_slice(&meshes.to_le_bytes()); // mesh_count
    for index in 0..meshes {
        let glb = facade_mesh_glb(index as f32 * 8.0);
        bytes.extend_from_slice(&(glb.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&glb);
    }
    bytes.extend_from_slice(&1u32.to_le_bytes()); // material_count
    let tint = tiny_png_bytes();
    let mut params = Vec::new();
    for row in 0..4u32 {
        for value in [row as f32, 1.0, 0.0, 2.0] {
            params.extend_from_slice(&value.to_le_bytes());
        }
    }
    // PNG 逐张 (len, payload)（8 张，index 4 = tint；与前端 readPng 一致）。
    for index in 0..8u32 {
        if index == 4 {
            bytes.extend_from_slice(&(tint.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&tint);
        } else {
            bytes.extend_from_slice(&0u32.to_le_bytes());
        }
    }
    bytes.extend_from_slice(&(params.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&params);
    bytes.extend_from_slice(&1u32.to_le_bytes()); // param_cols
    for _ in 0..meshes {
        bytes.extend_from_slice(&0u32.to_le_bytes()); // material index
        bytes.push(2); // uv kind
    }
    bytes.extend_from_slice(&0u32.to_le_bytes()); // diag
    bytes
}

fn source_lot_property() -> Vec<u8> {
    let transform = |x: f32| {
        Value::Transform(sc_properties::Transform {
            flags: 0,
            unknown: None,
            matrix: vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, x, 0.0, 0.0],
        })
    };
    let key = |instance: u32| {
        Value::Key(sc_properties::Key {
            type_id: 0,
            group: 0,
            instance,
        })
    };
    PropertyFile {
        claimed_count: 9,
        values: vec![
            // 主体三要件（2026-11-08 /37 参照 lot 主体契约）：夹具 lot 必须
            // 带 BoundingBox + LotMask，否则被 load-inputs 判为派生变体。
            Property {
                hash: 0x00F9_EFBA,
                prop_type: PropType::BoundingBox,
                kind: Kind::Scalar(Value::BoundingBox {
                    min: [-96.0, -48.0, 0.0],
                    max: [96.0, 48.0, 40.0],
                }),
                encoding: PropertyEncoding::default(),
            },
            Property {
                hash: 0x0CCB_7FD5,
                prop_type: PropType::Key,
                kind: Kind::Scalar(Value::Key(sc_properties::Key {
                    type_id: 0x2F4E_681C,
                    group: 0,
                    instance: 0x1234,
                })),
                encoding: PropertyEncoding::default(),
            },
            Property {
                hash: sc_properties::LOT_SIZE_HASH,
                prop_type: PropType::Vector2,
                kind: Kind::Scalar(Value::Vector2([192.0, 96.0])),
                encoding: PropertyEncoding::default(),
            },
            // 引用 lot 原有 units：build 后必须被场景文档的 units 完全替换。
            Property {
                hash: 0x0CAA_8F11, // LIGHT_TYPES
                prop_type: PropType::Key,
                kind: Kind::Array(vec![key(0x2F0F_F9FD)]),
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: Some(12),
                },
            },
            Property {
                hash: 0x0C12_EF20, // PROP_ID_BASE (bin 0)
                prop_type: PropType::Key,
                kind: Kind::Array(vec![key(777)]),
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: Some(12),
                },
            },
            Property {
                hash: 0x0C12_EF30, // PROP_TRANSFORM_BASE (bin 0)
                prop_type: PropType::Transform,
                kind: Kind::Array(vec![transform(5.0)]),
                // 加载器信任声明步长：Transform 数组必须声明 56（vanilla 约定），
                // 声明 0 会被 layout-contract 门禁拒绝（2026-11-07 s25/s26/s27 根因）。
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: Some(56),
                },
            },
            Property {
                hash: 0x0CAA_680D, // PATH_POINT_POINTS
                prop_type: PropType::Vector3,
                kind: Kind::Array(vec![Value::Vector3([9.0, 9.0, 9.0])]),
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: Some(12),
                },
            },
            Property {
                hash: 0x0CAA_6841, // PATH_PAIRS
                prop_type: PropType::Int32,
                kind: Kind::Array(vec![Value::Int32(9), Value::Int32(9)]),
                encoding: PropertyEncoding {
                    flags: 0,
                    array_item_size: Some(4),
                },
            },
        ],
    }
    .encode_canonical()
    .unwrap()
}

#[test]
fn workbench_lotm_builds_all_meshes_with_materials_and_replaces_reference_units() {
    let root = temp_root();
    // 参照包：lot property 带原 units（light/prop/path）。
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x8D007745,
    };
    let entries = [OverlayEntry::new(lot, source_lot_property())];
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("model.lotm"), workbench_lotm()).unwrap();
    fs::write(
        root.join("asset.lot.json"),
        serde_json::json!({
            "lot": {"size": [96, 96]},
            "units": [
                {"id": "prop:1:0", "kind": "prop", "index": 0, "bin": 1,
                 "resourceId": 3271283064u64, "slot": 0,
                 "transform": {"matrix": [1,0,0, 0,1,0, 0,0,1, 2,0,4], "flags": 0}},
                {"id": "pathPoint:0", "kind": "pathPoint", "index": 0,
                 "point": [0, 0, 0], "tangent": [1, 0, 0], "pointIndex": 0},
                {"id": "pathPoint:1", "kind": "pathPoint", "index": 1,
                 "point": [12, 0, 0], "tangent": [1, 0, 0], "pointIndex": 1}
            ],
            "editor": {"workbench": {"paths": []}}
        })
        .to_string(),
    )
    .unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "facade-tower", "kind": "building-asset",
        "reference": {
            "package": "source.package",
            "lot": "00B1B104:58F0C000:8D007745"
        },
        "scene": {"document": "asset.lot.json", "model": "model.lotm"},
        "outputs": {"package": "facade-tower.package"}
    });
    let result = build_from_schema(&root, &schema.to_string()).unwrap();
    assert!(result.scene.lowered_units >= 3, "units lowered");
    assert!(
        result.scene.material_resources >= 2,
        "tint + params inserted"
    );
    // delta-audit 闸门必须在本构建内执行，且声明来源 = 文档实际编辑族
    // （paths + props:1）——这是 r10 事故后的自动回归拦截点。
    let audit = result
        .events
        .iter()
        .find(|event| {
            event.stage == "delta-audit" && matches!(event.kind, StageEventKind::Finished)
        })
        .expect("delta-audit stage ran");
    assert_ne!(audit.detail["status"], "skipped", "{:?}", audit.detail);
    let declared = audit.detail["declared"].as_array().unwrap();
    let declared: Vec<&str> = declared.iter().filter_map(|v| v.as_str()).collect();
    assert!(declared.contains(&"paths"), "{declared:?}");
    assert!(declared.contains(&"props:1"), "{declared:?}");

    let output = Package::open(&result.path).unwrap();
    // 1) 模型：全部 mesh 进 RW4，材质引用解析到包内 raster。0x2F4E681B
    // 类型下有模型与 slot0 参数表两个 RW4 资源，按 MESH section 识别模型。
    let rw4_entries: Vec<_> = output
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x2F4E_681B)
        .cloned()
        .collect();
    // Four independent LOD model resources (NightHouse topology) + one
    // slot0 parameter-table RW4.
    assert_eq!(rw4_entries.len(), 5, "4 LOD models + slot0 params RW4");
    let mut model: Option<dbpf::IndexEntry> = None;
    for entry in &rw4_entries {
        let bytes = output.read(entry).unwrap();
        let file = rw4::Rw4File::parse(&bytes).unwrap();
        if file
            .sections_of_type(rw4::SectionType::MESH)
            .next()
            .is_some()
        {
            model = Some(entry.clone());
            break;
        }
    }
    let model = model.expect("compiled model with MESH sections");
    let model_instance = model.id.instance;
    let model_bytes = output.read(&model).unwrap();
    let file = rw4::Rw4File::parse(&model_bytes).unwrap();
    let meshes: Vec<u32> = file
        .sections_of_type(rw4::SectionType::MESH)
        .map(|s| s.number)
        .collect();
    // nh-partitioned 按材质分组合并：单材质夹具的两个容器 mesh 收敛进一个
    // RW4 mesh（几何不丢 = 顶点总数 6）。mesh 数随路线不同（generic 路线才是
    // 每 mesh 一个），断言锚定"几何全量 + 材质 survives"而不是 mesh 数。
    let total_vertices: usize = meshes
        .iter()
        .map(|mesh| {
            file.decode_mesh(&model_bytes, *mesh)
                .unwrap()
                .vertices
                .len()
        })
        .sum();
    assert_eq!(
        total_vertices, 6,
        "both container meshes compiled (geometry complete)"
    );
    for mesh in &meshes {
        let decoded = file.decode_mesh(&model_bytes, *mesh).unwrap();
        assert!(
            decoded.vertices.iter().any(|v| {
                v.components.iter().any(|(e, val)| {
                    e.usage == rw4::DeclarationUsage::TexCoord
                        && matches!(val, rw4::ComponentValue::Float4(_))
                })
            }),
            // atlas 路线会把 facade UV 重映射进图集 cell（幅值变小），这里只
            // 锚定 FLOAT4 UV 流存在；几何全量由上面的顶点总数断言覆盖。
            "facade FLOAT4 survives the compile"
        );
        let vertex = &decoded.vertices[0];
        assert_eq!(
            vertex.d3d_color_g(),
            Some(128),
            "tint column baked into COLOR.G"
        );
    }
    let material = file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .next()
        .unwrap()
        .number;
    let material = file.decode_material(&model_bytes, material).unwrap();
    let tint_instance = material.texture_refs()[2].texture_instance; // slot1
    let params_instance = material.texture_refs()[1].texture_instance; // slot0
    assert_ne!(tint_instance, 0, "tint raster bound");
    let tint_resource = ResourceId {
        type_id: 0x2F4E_681C,
        group: 0,
        instance: tint_instance,
    };
    let tint_bytes = output
        .read(output.entry(tint_resource).expect("tint raster in package"))
        .unwrap();
    let raster = rw4::RasterImage::parse(&tint_bytes).unwrap();
    assert_eq!(raster.width, 4, "PNG re-encoded as native raster");
    let params_resource = ResourceId {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: params_instance,
    };
    let params_bytes = output
        .read(
            output
                .entry(params_resource)
                .expect("params RW4 in package"),
        )
        .unwrap();
    let params_file = rw4::Rw4File::parse(&params_bytes).unwrap();
    let section = params_file
        .sections_of_type(rw4::SectionType::TEXTURE)
        .next()
        .unwrap();
    let texture = params_file
        .decode_texture(&params_bytes, section.number)
        .unwrap();
    assert_eq!(texture.texture_type, 116, "params table as paletteF32 RW4");

    // 2) 场景语义：引用 lot 的原 units 被替换，LotSize/LOD 键接新值。
    let lot_entry = output.entry(lot).unwrap();
    let lot_file = PropertyFile::parse(&output.read(lot_entry).unwrap()).unwrap();
    let lod_instances: Vec<u32> = sc_properties::LOD_MODEL_HASHES
        .iter()
        .filter_map(
            |hash| match lot_file.get(*hash).map(|property| &property.kind) {
                Some(Kind::Scalar(Value::Key(key))) => Some(key.instance),
                _ => None,
            },
        )
        .collect();
    assert_eq!(lod_instances.len(), 4, "LOD1..LOD4 are present");
    assert_eq!(
        lod_instances
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        4,
        "LOD1..LOD4 use four distinct model resources"
    );
    let assembled =
        sc_properties::LotEditorDocument::from_property_file(lot_file.clone()).assemble_units();
    let count = |predicate: &dyn Fn(&sc_properties::LotUnit) -> bool| {
        assembled.units.iter().filter(|u| predicate(u)).count()
    };
    // delta 语义（2026-11-07）：文档没有建模灯光族 → 参照 lot 的灯列
    // 原样保留。这是 r10 事故后的硬契约：未声明的族不得被清空。
    assert_eq!(
        count(&|u| matches!(u, sc_properties::LotUnit::Light { .. })),
        1,
        "undeclared light columns are preserved from the reference lot"
    );
    assert_eq!(
        count(&|u| matches!(u, sc_properties::LotUnit::Prop { bin: 0, .. })),
        1,
        "undeclared prop bin 0 is preserved from the reference lot"
    );
    assert_eq!(
        count(&|u| matches!(u, sc_properties::LotUnit::Prop { bin: 1, .. })),
        1,
        "authored prop (bin 1) written"
    );
    assert_eq!(
        count(&|u| matches!(u, sc_properties::LotUnit::PathPoint { .. })),
        2,
        "authored path points written"
    );
    assert!(
        assembled.path_pairs.is_empty(),
        "no paths declared, no pairs"
    );
    let lot_size = lot_file
        .get(sc_properties::LOT_SIZE_HASH)
        .and_then(|p| p.scalar().cloned());
    assert_eq!(
        lot_size,
        Some(Value::Vector2([96.0, 96.0])),
        "scene LotSize lowered"
    );
    let lod1 = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned());
    assert_eq!(
        lod1,
        Some(Value::Key(sc_properties::Key {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: model_instance,
        })),
        "LOD1 retargeted to the compiled model"
    );
    drop(output);
    let _ = fs::remove_dir_all(root);
}

/// G1 材质指纹闸门（s10 贴图回归的构建期拦截点，2026-11-05 用户指令）：
/// 首建记录基线 → 重复构建/纯场景变更放行 → 未声明的模型变更在
/// insert-model 即失败（错误带逐字段 old→new）→ 声明 materialChange 后
/// 放行并刷新基线。关键数字随 insert-model 事件入台账。
#[test]
fn material_gate_blocks_silent_model_changes_only() {
    let root = temp_root();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x8D007745,
    };
    let entries = [OverlayEntry::new(lot, source_lot_property())];
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("model.lotm"), workbench_lotm()).unwrap();
    fs::write(
        root.join("asset.lot.json"),
        serde_json::json!({
            "lot": {"size": [96, 96]},
            "units": [
                {"id": "pathPoint:0", "kind": "pathPoint", "index": 0,
                 "point": [0, 0, 0], "tangent": [1, 0, 0], "pointIndex": 0},
                {"id": "pathPoint:1", "kind": "pathPoint", "index": 1,
                 "point": [12, 0, 0], "tangent": [1, 0, 0], "pointIndex": 1}
            ],
            "editor": {"workbench": {"paths": []}}
        })
        .to_string(),
    )
    .unwrap();
    let schema = |material_change: bool| {
        let mut semantics = serde_json::json!({});
        if material_change {
            semantics["materialChange"] = serde_json::json!(true);
        }
        serde_json::json!({
            "$schema": "openscp.asset", "schema_version": 2,
            "id": "gate-tower", "kind": "building-asset",
            "reference": {"package": "source.package", "lot": "00B1B104:58F0C000:8D007745"},
            "scene": {"document": "asset.lot.json", "model": "model.lotm"},
            "semantics": semantics,
            "outputs": {"package": "gate-tower.package"}
        })
        .to_string()
    };

    // 1) 首建：记录基线，insert-model 事件携带指纹与关键数字。
    let first = build_from_schema(&root, &schema(false)).unwrap();
    let baseline_path = root.join(".build").join("material-baseline.json");
    assert!(baseline_path.is_file(), "baseline recorded on first build");
    let insert_model_detail = first
        .events
        .iter()
        .find(|event| {
            event.stage == "insert-model" && matches!(event.kind, StageEventKind::Finished)
        })
        .expect("insert-model finished event present");
    let gate = insert_model_detail
        .detail
        .pointer("/materialGate/fingerprint")
        .expect("material fingerprint in stage event");
    assert_eq!(
        gate.pointer("/parameterColumns"),
        Some(&serde_json::json!([1])),
        "parameter column count is logged key evidence"
    );
    assert_eq!(gate.pointer("/meshCount"), Some(&serde_json::json!(2)));

    // 2) 重复构建：指纹一致，放行。
    build_from_schema(&root, &schema(false)).unwrap();

    // 3) 纯场景变更（材质输入未动）：放行，基线不受影响。
    let scene = fs::read_to_string(root.join("asset.lot.json")).unwrap();
    let mut document: serde_json::Value = serde_json::from_str(&scene).unwrap();
    document["units"][1]["point"][0] = serde_json::json!(24);
    fs::write(root.join("asset.lot.json"), document.to_string()).unwrap();
    build_from_schema(&root, &schema(false)).unwrap();

    // 4) 模型静默变更（2 mesh -> 1 mesh）：insert-model 即失败，错误逐字段。
    fs::write(root.join("model.lotm"), workbench_lotm_with_meshes(1)).unwrap();
    let error = build_from_schema(&root, &schema(false)).unwrap_err();
    assert!(error.contains("[insert-model]"), "{error}");
    assert!(error.contains("meshCount 2 -> 1"), "{error}");

    // 5) 声明 materialChange=true：放行并刷新基线；此后不声明也放行。
    build_from_schema(&root, &schema(true)).unwrap();
    build_from_schema(&root, &schema(false)).unwrap();

    let _ = fs::remove_dir_all(root);
}

/// G3 增量构建（用户指令 2026-11-05）：
/// - 同输入二次构建 = 全链复用（事件流单条 cache-reuse，sha256 与首建一致）；
/// - 场景变更构建 = 全链重跑但 insert-model 材质缓存命中（免重编译），
///   材质资源数与首建一致。
#[test]
fn incremental_builds_reuse_run_and_material_cache() {
    let root = temp_root();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x8D007745,
    };
    let entries = [OverlayEntry::new(lot, source_lot_property())];
    fs::write(
        root.join("source.package"),
        write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    fs::write(root.join("model.lotm"), workbench_lotm()).unwrap();
    fs::write(
        root.join("asset.lot.json"),
        serde_json::json!({
            "lot": {"size": [96, 96]},
            "units": [
                {"id": "pathPoint:0", "kind": "pathPoint", "index": 0,
                 "point": [0, 0, 0], "tangent": [1, 0, 0], "pointIndex": 0},
                {"id": "pathPoint:1", "kind": "pathPoint", "index": 1,
                 "point": [12, 0, 0], "tangent": [1, 0, 0], "pointIndex": 1}
            ],
            "editor": {"workbench": {"paths": []}}
        })
        .to_string(),
    )
    .unwrap();
    let schema = serde_json::json!({
        "$schema": "openscp.asset", "schema_version": 2,
        "id": "incremental-tower", "kind": "building-asset",
        "reference": {"package": "source.package", "lot": "00B1B104:58F0C000:8D007745"},
        "scene": {"document": "asset.lot.json", "model": "model.lotm"},
        "outputs": {"package": "incremental-tower.package"}
    })
    .to_string();

    // 1) 首建：全量链，材质缓存 miss。
    let first = build_from_schema(&root, &schema).unwrap();
    let first_model_detail = first
        .events
        .iter()
        .find(|event| {
            event.stage == "insert-model" && matches!(event.kind, StageEventKind::Finished)
        })
        .unwrap();
    assert_eq!(
        first_model_detail.detail.pointer("/materialCache"),
        Some(&serde_json::json!("miss")),
        "first build compiles and stores the material cache"
    );

    // 2) 同输入二次构建：全链复用。
    let second = build_from_schema(&root, &schema).unwrap();
    assert_eq!(
        second.sha256, first.sha256,
        "reused package bytes identical"
    );
    assert_eq!(second.build_hash, first.build_hash);
    assert_eq!(
        second.events.len(),
        1,
        "single cache-reuse event: {:?}",
        second.events
    );
    assert_eq!(second.events[0].stage, "cache-reuse");

    // 3) 场景变更：全链重跑（事件为 8 阶段），材质缓存命中。
    let scene = fs::read_to_string(root.join("asset.lot.json")).unwrap();
    let mut document: serde_json::Value = serde_json::from_str(&scene).unwrap();
    document["units"][1]["point"][0] = serde_json::json!(24);
    fs::write(root.join("asset.lot.json"), document.to_string()).unwrap();
    let third = build_from_schema(&root, &schema).unwrap();
    assert_ne!(
        third.build_hash, first.build_hash,
        "scene edit is a new identity"
    );
    let finished_stages = third
        .events
        .iter()
        .filter(|event| matches!(event.kind, StageEventKind::Finished))
        .count();
    assert!(
        finished_stages >= 12,
        "full chain reruns all required stages (got {finished_stages})"
    );
    let finished_names: std::collections::HashSet<_> = third
        .events
        .iter()
        .filter(|event| matches!(event.kind, StageEventKind::Finished))
        .map(|event| event.stage.as_str())
        .collect();
    for required in [
        "load-inputs",
        "insert-model",
        "layout-contract",
        "menu-contract",
        "delta-audit",
        "material-binding-contract",
        "materialize-and-link",
    ] {
        assert!(
            finished_names.contains(required),
            "missing stage {required}: {finished_names:?}"
        );
    }
    let third_model_detail = third
        .events
        .iter()
        .find(|event| {
            event.stage == "insert-model" && matches!(event.kind, StageEventKind::Finished)
        })
        .unwrap();
    assert_eq!(
        third_model_detail.detail.pointer("/materialCache"),
        Some(&serde_json::json!("hit")),
        "material compile skipped via cache on scene-only change"
    );
    assert_eq!(
        third_model_detail.detail.pointer("/materialResources"),
        first_model_detail.detail.pointer("/materialResources"),
        "cached material resources match the compiled count"
    );

    let _ = fs::remove_dir_all(root);
}

/// Opt-in real-data smoke: builds the workbench commercial sample's 46MB
/// v9 LOTM (105 mesh GLBs, NY native pipeline) against the DLC0 reference
/// package.  Requires SC_ASSET_FIXTURE_ROOT=…/tmp/asset-build-fixtures with
/// `commercial-model/{schema.json, model.lotm, asset.lot.json,
/// SimCity_DLC0.package}`.
#[test]
#[ignore = "requires the local asset-build fixture package and the workbench sample LOTM"]
fn real_commercial_workbench_lotm_builds_full_model() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial-model");
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let started = std::time::Instant::now();
    let result = build_from_schema(&root, &raw)
        .unwrap_or_else(|error| panic!("commercial model build failed: {error}"));
    println!(
        "commercial smoke: {result:?}
build took {:?}",
        started.elapsed()
    );
    // Delta contract: the package carries only this build's own resources.
    assert!(result.delta_mode, "delta is the default output mode");
    assert!(
        result.resource_count < 500,
        "clean package must not carry the reference clone, got {} entries",
        result.resource_count
    );
    // The whole container compiles: model + slot rasters land in the package.
    assert!(result.scene.material_resources > 0, "materials inserted");
    assert!(
        result.scene.lowered_units >= 17,
        "all authored units lowered"
    );
    assert!(result.path.is_file());
    // The compiled model carries every container mesh through the same
    // decode path the Property Viewer uses.  Locate it via the built lot
    // property's LOD1 key (also proves the retarget landed).
    let output = Package::open(&result.path).unwrap();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x41F0C000,
        instance: 0x0483239E,
    };
    let lot_file = PropertyFile::parse(
        &output
            .read(output.entry(lot).expect("lot property in output"))
            .unwrap(),
    )
    .unwrap();
    let lod1 = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned());
    let Some(Value::Key(model_key)) = lod1 else {
        panic!("LOD1 key retargeted to the compiled model");
    };
    assert_eq!(model_key.type_id, 0x2F4E_681B);
    let model = output
        .entry(ResourceId {
            type_id: model_key.type_id,
            group: model_key.group,
            instance: model_key.instance,
        })
        .expect("compiled model in package");
    let bytes = output.read(model).unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();
    let mesh_count = file.sections_of_type(rw4::SectionType::MESH).count();
    let bindings = file.decode_mesh_material_bindings(&bytes).len();
    let materials = file.sections_of_type(rw4::SectionType::MATERIAL).count();
    println!("commercial model: {mesh_count} meshes, {bindings} bindings, {materials} materials");
    let merged = raw.contains("\"merged-single\"");
    if merged {
        // merged-single 形状：最少 mesh（u16 顶点帽拆分）+ 单材质。
        assert_eq!(mesh_count, 2, "merged shape: chunk split at u16 cap only");
        assert_eq!(materials, 1, "merged shape: single material");
    } else {
        let route = result
            .events
            .iter()
            .find(|event| {
                event.stage == "insert-model" && matches!(event.kind, StageEventKind::Finished)
            })
            .and_then(|event| event.detail.pointer("/geometryRoute/name"))
            .and_then(serde_json::Value::as_str);
        if let Some(route) = route {
            assert_eq!(
                route, "native-multi",
                "plugin-native LOTM route is preserved"
            );
        }
        // Empty editor containers are not emitted as invalid zero-vertex RW4
        // meshes. The render contract is one binding per emitted mesh and a
        // material table larger than one, not a blind container-count copy.
        assert!(mesh_count > 1, "native model must contain facade geometry");
        assert_eq!(
            bindings, mesh_count,
            "every emitted mesh bound to its material"
        );
        assert!(materials > 1, "native material graph survives compilation");
    }
}

/// Opt-in real export: builds the 大学-综合实验楼 workbench sample (v9 LOTM,
/// 74 mesh GLBs + 8 props + 4 path points + one road path) against a DLC0
/// copy merged with the `675B61B1` PathEntry from SimCity_Game (the entry
/// lives there, not in DLC0).  Fixture layout under
/// `$SC_ASSET_FIXTURE_ROOT/university-export/`: schema.json, model.lotm,
/// asset.lot.json, SimCity_DLC0.package; the game package path comes from
/// `SC_UNIVERSITY_GAME_PACKAGE`.  The merged copy stays in the disposable
/// fixture (re-runs re-merge idempotently); copies for manual verification
/// are made outside this test.
#[test]
#[ignore = "requires the local fixture package, SimCity_Game PathEntry and the workbench sample"]
fn real_university_workbench_export_builds_package() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let game_path = PathBuf::from(
        std::env::var("SC_UNIVERSITY_GAME_PACKAGE").expect("set SC_UNIVERSITY_GAME_PACKAGE"),
    );
    let root = fixture.join("university-export");
    // 合并 PathEntry（SimCity_Game 独有）进 DLC0 副本，路径闭包校验才成立。
    let source_path = root.join("SimCity_DLC0.package");
    let mut resources = connector::read_source(&source_path).unwrap();
    let game = Package::open(&game_path).unwrap();
    let path_entry = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C400,
        instance: 0x675B61B1,
    };
    let entry = game
        .entry(path_entry)
        .unwrap_or_else(|| panic!("PathEntry {path_entry} missing from game package"));
    resources.insert(path_entry, game.read(entry).unwrap());
    fs::write(&source_path, connector::link(&resources).unwrap()).unwrap();

    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let started = std::time::Instant::now();
    let result = build_from_schema(&root, &raw)
        .unwrap_or_else(|error| panic!("university export build failed: {error}"));
    println!(
        "university export: {result:?}
build took {:?}",
        started.elapsed()
    );
    assert!(result.delta_mode, "delta is the default output mode");
    assert!(
        result.resource_count < 500,
        "clean package must not carry the reference clone, got {} entries",
        result.resource_count
    );
    assert_eq!(result.scene.unit_count, 12, "authored units present");
    assert_eq!(result.scene.lowered_units, 12, "all units lowered");
    assert_eq!(result.scene.path_count, 1, "road path validated");
    assert!(result.scene.material_resources > 0, "materials inserted");

    // 产物自检：LOD1 → 编译模型；模型含全部容器 mesh；lot 单元只剩作者单元。
    let output = Package::open(&result.path).unwrap();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x2063CCDD,
    };
    let lot_file = PropertyFile::parse(
        &output
            .read(output.entry(lot).expect("lot property in output"))
            .unwrap(),
    )
    .unwrap();
    let Some(Value::Key(model_key)) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
    else {
        panic!("LOD1 retargeted to the compiled model");
    };
    let model_bytes = output
        .read(
            output
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .expect("compiled model in package"),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&model_bytes).unwrap();
    let mesh_count = file.sections_of_type(rw4::SectionType::MESH).count();
    let bindings = file.decode_mesh_material_bindings(&model_bytes).len();
    println!("university model: {mesh_count} meshes, {bindings} bindings");
    assert!(
        mesh_count > 1,
        "university export must retain the multi-mesh model"
    );
    assert_eq!(
        bindings, mesh_count,
        "every compiled mesh must have a material binding"
    );
    let assembled = LotEditorDocument::from_property_file(lot_file).assemble_units();
    let props = assembled
        .units
        .iter()
        .filter(|u| matches!(u, sc_properties::LotUnit::Prop { .. }))
        .count();
    let points = assembled
        .units
        .iter()
        .filter(|u| matches!(u, sc_properties::LotUnit::PathPoint { .. }))
        .count();
    assert!(props > 0, "authored props must be written");
    assert!(points >= 4, "authored path points must be retained");
    assert_eq!(assembled.path_pairs, vec![0, 3], "road path pairs written");
}

/// Hope Tower is the backend-native import sample.  It must retain the
/// plugin's multi-material LOTM semantics and use a lot-config donor rather
/// than an Asset-dictionary lot leaf.
#[test]
#[ignore = "real Hope Tower fixture build"]
fn real_hope_tower_lotm_builds_native_multi_package() {
    let fixture = PathBuf::from(
        std::env::var("SC_HOPE_FIXTURE_ROOT")
            .unwrap_or_else(|_| r"D:\rust\packages\fluffy-open-scp\tmp\hope-project".into()),
    );
    let root = fixture.join("schemas/assets");
    let raw = fs::read_to_string(root.join("m1.hope-tower.json")).unwrap();
    let result = build_from_schema(&fixture, &raw)
        .unwrap_or_else(|error| panic!("hope tower build failed: {error}"));
    assert!(
        !result.delta_mode,
        "Hope Tower sample intentionally exercises full-package output"
    );
    assert!(
        result.scene.material_resources > 0,
        "native materials inserted"
    );

    let output = Package::open(&result.path).unwrap();
    let menu_instance = result.menu_instance.expect("Hope Tower menu identity");
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x40E1C000,
        instance: menu_instance,
    };
    let lot_file = PropertyFile::parse(
        &output
            .read(output.entry(lot).expect("hope lot-config in output"))
            .unwrap(),
    )
    .unwrap();
    let Some(Value::Key(model_key)) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|property| property.scalar().cloned())
    else {
        panic!("hope lot-config must point to the compiled model");
    };
    let model_bytes = output
        .read(
            output
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .expect("compiled Hope Tower model in package"),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&model_bytes).unwrap();
    let mesh_count = file.sections_of_type(rw4::SectionType::MESH).count();
    let bindings = file.decode_mesh_material_bindings(&model_bytes).len();
    assert_eq!(
        mesh_count, 1,
        "the author Building40 route combines all source groups into one mesh"
    );
    assert_eq!(bindings, 1, "the Building40 mesh must have one binding");
    let material_sections: Vec<_> = file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .collect();
    assert_eq!(material_sections.len(), 1, "Building40 has one material");
    let rw4::MaterialSection::Decoded(material) = file
        .decode_material(&model_bytes, material_sections[0].number)
        .unwrap()
    else {
        panic!("Building40 material must use the decoded native layout");
    };
    for slot in 0..=5 {
        assert_eq!(
            material
                .texture_slots()
                .filter(|row| row.slot_byte() == slot)
                .count(),
            1,
            "Building40 slot{slot} must be present exactly once"
        );
    }

    let assembled = LotEditorDocument::from_property_file(lot_file.clone()).assemble_units();
    assert_eq!(
        assembled
            .units
            .iter()
            .filter(|unit| matches!(unit, sc_properties::LotUnit::PathPoint { .. }))
            .count(),
        12,
        "both authored paths retain their global point pool"
    );
    assert_eq!(assembled.path_pairs, vec![0, 3, 4, 11]);
    assert_eq!(
        assembled
            .units
            .iter()
            .filter(|unit| matches!(unit, sc_properties::LotUnit::Prop { .. }))
            .count(),
        2,
        "authored props survive the parent-free menu route"
    );
    for id in [
        ResourceId {
            type_id: 0x00B1_B104,
            group: 0x0987_8A01,
            instance: menu_instance,
        },
        ResourceId {
            type_id: 0x2F7D_0004,
            group: 0x40E0_2400,
            instance: menu_instance,
        },
        ResourceId {
            type_id: 0x3F86_62EA,
            group: 0,
            instance: menu_instance,
        },
        ResourceId {
            type_id: 0x0A98_EAF0,
            group: 0x02FA_BF01,
            instance: menu_instance,
        },
    ] {
        assert!(output.entry(id).is_some(), "menu resource {id} is missing");
    }
}

/// D1 diagnostic: run the Property Viewer's per-mesh export path over the
/// compiled university model and report exactly which meshes the viewer
/// would drop (is_exportable / 8MB GLB cap) versus what the container had.
#[test]
#[ignore = "diagnostic for the built university package"]
fn diag_university_compiled_model_viewer_export() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let build_dir = fixture.join("university-export/.build");
    let mut package_path = None;
    for hash in fs::read_dir(&build_dir).expect("build dir").flatten() {
        let candidate = hash.path().join("packages/university-lab.package");
        if candidate.is_file() {
            package_path = Some(candidate);
        }
    }
    let package_path = package_path.expect("built university package");
    let output = Package::open(&package_path).unwrap();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x2063CCDD,
    };
    let lot_file = PropertyFile::parse(&output.read(output.entry(lot).unwrap()).unwrap()).unwrap();
    let Value::Key(model_key) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
        .unwrap()
    else {
        panic!("LOD1 key");
    };
    let bytes = output
        .read(
            output
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .unwrap(),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();
    const CAP: usize = 8 * 1024 * 1024;
    let mut dropped_exportable = 0;
    let mut dropped_cap = 0;
    let mut total_tris = 0usize;
    let mut exported_tris = 0usize;
    println!("mesh#   verts    tris  exportable  glb_bytes  uv2/facade");
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let decoded = file.decode_mesh(&bytes, section.number).unwrap();
        let exportable = decoded.is_exportable();
        let tris = decoded.triangles.len();
        total_tris += tris;
        let facade = decoded.vertices.iter().any(|v| {
            v.components.iter().any(|(e, val)| {
                e.usage == rw4::DeclarationUsage::TexCoord
                    && matches!(val, rw4::ComponentValue::Float4(f) if f.iter().any(|x| x.abs() > 8.0))
            })
        });
        let mut glb_len = 0usize;
        let mut skipped = false;
        if exportable {
            let glb = sc_exporter::export_glb_with_colors(
                &decoded,
                None,
                &[],
                sc_exporter::EmbeddedTextures::default(),
                None,
                None,
            );
            glb_len = glb.bytes.len();
            if glb_len > CAP {
                skipped = true;
                dropped_cap += 1;
            } else {
                exported_tris += tris;
            }
        } else {
            skipped = true;
            dropped_exportable += 1;
        }
        println!(
            "#{:<5} {:>6} {:>7}  {:>9}  {:>8}  {:>6} {}",
            section.number,
            decoded.vertices.len(),
            tris,
            exportable,
            glb_len,
            facade,
            if skipped { "DROP" } else { "" }
        );
    }
    println!(
        "total tris {total_tris}, exported {exported_tris}, dropped exportable {dropped_exportable}, dropped cap {dropped_cap}"
    );
}

/// D3 diagnostic: for every mesh, compare the attribute streams of the SOURCE
/// container GLB against the viewer's re-exported GLB (ranges of each
/// TEXCOORD set + presence), to expose what the re-export loses.
#[test]
#[ignore = "diagnostic for the built university package"]
fn diag_university_uv_stream_roundtrip() {
    use sc_exporter::export_glb_with_colors;
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let source_lotm = fs::read(fixture.join("university-export/model.lotm")).unwrap();
    let doc = crate::asset_build_m1::lotm::parse(&source_lotm).unwrap();
    // compiled model out of the built package
    let build_dir = fixture.join("university-export/.build");
    let mut package_path = None;
    for hash in fs::read_dir(&build_dir).unwrap().flatten() {
        let candidate = hash.path().join("packages/university-lab.package");
        if candidate.is_file() {
            package_path = Some(candidate);
        }
    }
    let output = Package::open(&package_path.unwrap()).unwrap();
    let lot = ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x2063CCDD,
    };
    let lot_file = PropertyFile::parse(&output.read(output.entry(lot).unwrap()).unwrap()).unwrap();
    let Value::Key(model_key) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
        .unwrap()
    else {
        panic!("LOD1");
    };
    let bytes = output
        .read(
            output
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .unwrap(),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();

    fn glb_json(b: &[u8]) -> serde_json::Value {
        let total = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
        let mut c = 12usize;
        while c + 8 <= total {
            let len = u32::from_le_bytes(b[c..c + 4].try_into().unwrap()) as usize;
            let kind = u32::from_le_bytes(b[c + 4..c + 8].try_into().unwrap());
            if kind == 0x4E4F534A {
                return serde_json::from_slice(&b[c + 8..c + 8 + len]).unwrap();
            }
            c += 8 + len;
        }
        panic!("no json");
    }
    fn stream_ranges(glb: &[u8]) -> String {
        let j = glb_json(glb);
        let prim = &j["meshes"][0]["primitives"][0];
        let attrs = prim["attributes"].as_object().unwrap();
        let mut parts = Vec::new();
        let mut names: Vec<_> = attrs.keys().cloned().collect();
        names.sort();
        for name in names {
            let acc = &j["accessors"][attrs[&name].as_u64().unwrap() as usize];
            let ty = acc["type"].as_str().unwrap_or("?");
            let range = if (acc["min"].is_array() && acc["max"].is_array()) {
                let m = acc["max"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .chain(acc["min"].as_array().unwrap())
                    .filter_map(|v| v.as_f64())
                    .map(|v| v.abs())
                    .fold(0.0f64, f64::max);
                format!("max={m:.2}")
            } else {
                "n/a".to_string()
            };
            parts.push(format!("{name}:{ty}:{range}"));
        }
        let has_color = parts.iter().any(|p| p.starts_with("COLOR_0"));
        format!("attrs=[{}] color={has_color}", parts.join(" "))
    }

    for (index, glb) in doc.meshes.iter().enumerate().take(8) {
        let section = file
            .sections_of_type(rw4::SectionType::MESH)
            .nth(index)
            .unwrap()
            .number;
        let decoded = file.decode_mesh(&bytes, section).unwrap();
        let out = export_glb_with_colors(
            &decoded,
            None,
            &[],
            sc_exporter::EmbeddedTextures::default(),
            None,
            None,
        );
        println!(
            "mesh {index}
  src: {}
  out: {}",
            stream_ranges(glb),
            stream_ranges(&out.bytes)
        );
    }
}

/// 对拍诊断：原版建筑 00B1B104:52F0C000:7BEF70C3（查看器渲染正常）vs
/// 编译导出的大学模型——两者各走一遍 build_lot_model_payload_for_test，
/// 逐字段对比 payload 结构（mesh/uv_kind/材质表/GLB 属性流）。
#[test]
#[ignore = "requires local game packages; set SC_UV_CENSUS_PACKAGE to any SimCityData package"]
fn diag_native_vs_compiled_payload_diff() {
    use crate::package_service::{PackageManager, build_lot_model_payload_for_test};

    fn glb_json(glb: &[u8]) -> serde_json::Value {
        let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        let mut c = 12usize;
        while c + 8 <= total {
            let len = u32::from_le_bytes(glb[c..c + 4].try_into().unwrap()) as usize;
            let kind = u32::from_le_bytes(glb[c + 4..c + 8].try_into().unwrap());
            if kind == 0x4E4F534A {
                return serde_json::from_slice(&glb[c + 8..c + 8 + len]).unwrap();
            }
            c += 8 + len;
        }
        panic!("GLB JSON chunk missing");
    }
    fn glb_attrs(glb: &[u8]) -> String {
        let j = glb_json(glb);
        let prim = &j["meshes"][0]["primitives"][0];
        let mut names: Vec<String> = prim["attributes"]
            .as_object()
            .map(|a| a.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        names
            .iter()
            .map(|name| {
                let acc =
                    &j["accessors"][prim["attributes"][name.as_str()].as_u64().unwrap() as usize];
                format!("{}:{}", name, acc["type"].as_str().unwrap_or("?"))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
    /// v10 payload → (ver, per-mesh GLB, per-material, per-mesh binding)。
    fn split_payload(
        payload: &[u8],
    ) -> (
        u32,
        Vec<Vec<u8>>,
        Vec<(usize, usize, usize)>,
        Vec<(u32, u8)>,
    ) {
        let rd = |pos: usize| u32::from_le_bytes(payload[pos..pos + 4].try_into().unwrap());
        let ver = rd(4);
        let mut cursor = 8usize; // magic(0..4) ver(4..8) mesh_count(8..12)
        let mesh_count = rd(cursor) as usize;
        cursor += 4;
        let mut glbs = Vec::new();
        for _ in 0..mesh_count {
            let len = rd(cursor) as usize;
            cursor += 4;
            glbs.push(payload[cursor..cursor + len].to_vec());
            cursor += len;
        }
        let material_count = rd(cursor) as usize;
        cursor += 4;
        let mut materials = Vec::new();
        let png_slots = if ver >= 10 { 9usize } else { 8usize };
        for _ in 0..material_count {
            let mut present = 0usize;
            for _ in 0..png_slots {
                let len = rd(cursor) as usize;
                cursor += 4;
                if len > 0 {
                    present += 1;
                }
                cursor += len;
            }
            let params_len = rd(cursor) as usize;
            cursor += 4;
            let cols = rd(cursor + params_len) as usize;
            materials.push((present, cols, params_len));
            cursor += params_len + 4;
        }
        let mut bindings = Vec::new();
        for _ in 0..mesh_count {
            let material = rd(cursor);
            cursor += 4;
            let uv = payload[cursor];
            cursor += 1;
            bindings.push((material, uv));
        }
        (ver, glbs, materials, bindings)
    }

    let any_package =
        PathBuf::from(std::env::var("SC_UV_CENSUS_PACKAGE").expect("set SC_UV_CENSUS_PACKAGE"));
    let game_dir = any_package.parent().unwrap().to_path_buf();
    let manager = PackageManager::new();
    for name in [
        "SimCity_Game.package",
        "SimCity_Graphics.package",
        "SimCity_DLC0.package",
    ] {
        let path = game_dir.join(name);
        if path.is_file() {
            let package = dbpf::Package::open(&path).unwrap();
            manager.insert(package).unwrap();
        }
    }
    let packages = manager.all_packages_with_ids().unwrap();

    let native_lot = dbpf::ResourceId {
        type_id: 0x00B1B104,
        group: 0x41E1C000,
        instance: 0xB64CF55B,
    };
    let mut native_model: Option<(dbpf::ResourceId, u64)> = None;
    for (id, package) in &packages {
        let Some(entry) = package.entry(native_lot) else {
            continue;
        };
        let bytes = package.read(entry).unwrap();
        let lot_file = PropertyFile::parse(&bytes).unwrap();
        if let Some(Value::Key(key)) = lot_file
            .get(sc_properties::LOD1_MODEL_HASH)
            .and_then(|p| p.scalar().cloned())
        {
            native_model = Some((
                dbpf::ResourceId {
                    type_id: key.type_id,
                    group: key.group,
                    instance: key.instance,
                },
                *id,
            ));
            break;
        }
    }
    let (native_model, _) = native_model.expect("native lot LOD1 model");
    println!("native lot → model {native_model}");

    let native_bytes = packages
        .iter()
        .find_map(|(_, package)| {
            package
                .entry(native_model)
                .map(|entry| package.read(&entry).unwrap())
        })
        .expect("native model bytes");

    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let mut package_path = None;
    for hash in fs::read_dir(fixture.join("university-export/.build"))
        .unwrap()
        .flatten()
    {
        let candidate = hash.path().join("packages/university-lab.package");
        if candidate.is_file() {
            package_path = Some(candidate);
        }
    }
    let built = Package::open(&package_path.unwrap()).unwrap();
    let lot = dbpf::ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x2063CCDD,
    };
    let lot_file = PropertyFile::parse(&built.read(built.entry(lot).unwrap()).unwrap()).unwrap();
    let Value::Key(compiled_key) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
        .unwrap()
    else {
        panic!("LOD1");
    };
    let compiled_bytes = built
        .read(
            built
                .entry(ResourceId {
                    type_id: compiled_key.type_id,
                    group: compiled_key.group,
                    instance: compiled_key.instance,
                })
                .unwrap(),
        )
        .unwrap();

    for (label, model_bytes) in [("NATIVE", &native_bytes), ("COMPILED", &compiled_bytes)] {
        let file = rw4::Rw4File::parse(model_bytes).unwrap();
        let payload = build_lot_model_payload_for_test(&file, model_bytes, &built, &manager, 0);
        println!(
            "=== {label}: payload_len={} head={:02x?} ascii={}",
            payload.len(),
            &payload[..16.min(payload.len())],
            payload[..16.min(payload.len())]
                .iter()
                .map(|b| if b.is_ascii_graphic() {
                    *b as char
                } else {
                    '.'
                })
                .collect::<String>(),
        );
        let (ver, glbs, materials, bindings) = split_payload(&payload);
        println!(
            "=== {label}: ver={ver} meshes={} materials={}",
            glbs.len(),
            materials.len()
        );
        for (index, glb) in glbs.iter().enumerate().take(2) {
            println!("  glb[{index}] {}", glb_attrs(glb));
            // 解码全部 accessor 的实际数值范围。
            let j = glb_json(glb);
            let mut bin: Vec<u8> = Vec::new();
            {
                let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
                let mut c = 12usize;
                while c + 8 <= total {
                    let len = u32::from_le_bytes(glb[c..c + 4].try_into().unwrap()) as usize;
                    let kind = u32::from_le_bytes(glb[c + 4..c + 8].try_into().unwrap());
                    if kind == 0x004E4942 {
                        bin = glb[c + 8..c + 8 + len].to_vec();
                        break;
                    }
                    c += 8 + len;
                }
            }
            let mut names: Vec<String> = j["meshes"][0]["primitives"][0]["attributes"]
                .as_object()
                .map(|a| a.keys().cloned().collect())
                .unwrap_or_default();
            names.sort();
            for name in names {
                let acc_idx = j["meshes"][0]["primitives"][0]["attributes"][&name]
                    .as_u64()
                    .unwrap() as usize;
                let acc = &j["accessors"][acc_idx];
                let view = &j["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
                let base = view["byteOffset"].as_u64().unwrap_or(0) as usize
                    + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
                let comps = match acc["type"].as_str().unwrap_or("VEC3") {
                    "VEC2" => 2usize,
                    "VEC3" => 3,
                    "VEC4" => 4,
                    _ => 4,
                };
                let count = acc["count"].as_u64().unwrap_or(0) as usize;
                let mut ranges = [(f32::MAX, f32::MIN); 4];
                for i in 0..count {
                    for k in 0..comps {
                        let o = base + (i * comps + k) * 4;
                        if o + 4 > bin.len() {
                            break;
                        }
                        let v = f32::from_le_bytes(bin[o..o + 4].try_into().unwrap());
                        ranges[k].0 = ranges[k].0.min(v);
                        ranges[k].1 = ranges[k].1.max(v);
                    }
                }
                let parts: Vec<String> = (0..comps)
                    .map(|k| {
                        let (lo, hi) = ranges[k];
                        format!("[{:.2},{:.2}]", lo, hi)
                    })
                    .collect();
                println!("    {} n={count} {}", name, parts.join(" "));
            }
        }
        for (index, (present, cols, params_len)) in materials.iter().enumerate().take(4) {
            println!(
                "  mat[{index}] pngs_present={present}/9 params_len={params_len} param_cols={cols}"
            );
        }
        println!(
            "  bindings[:8] = {}",
            bindings
                .iter()
                .take(8)
                .map(|(m, uv)| format!("(mat={m},uv={uv})"))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let mut kind_hist: std::collections::BTreeMap<u8, usize> =
            std::collections::BTreeMap::new();
        for (_, uv) in &bindings {
            *kind_hist.entry(*uv).or_default() += 1;
        }
        println!("  uv_kind histogram = {kind_hist:?}");
    }
}

/// D3 终局对拍：找第一个窗户材质（params row0.z > 0），把它的源容器 GLB 与
/// 再导出 GLB 的 TEXCOORD 数值范围逐流解码对比。
#[test]
#[ignore = "requires local game packages; set SC_UV_CENSUS_PACKAGE"]
fn diag_window_mesh_uv_values() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let doc = crate::asset_build_m1::lotm::parse(
        &fs::read(fixture.join("university-export/model.lotm")).unwrap(),
    )
    .unwrap();
    let build_dir = fixture.join("university-export/.build");
    let mut package_path = None;
    for hash in fs::read_dir(&build_dir).unwrap().flatten() {
        let candidate = hash.path().join("packages/university-lab.package");
        if candidate.is_file() {
            package_path = Some(candidate);
        }
    }
    let built = Package::open(&package_path.unwrap()).unwrap();
    let lot = dbpf::ResourceId {
        type_id: 0x00B1B104,
        group: 0x58F0C000,
        instance: 0x2063CCDD,
    };
    let lot_file = PropertyFile::parse(&built.read(built.entry(lot).unwrap()).unwrap()).unwrap();
    let Value::Key(model_key) = lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
        .unwrap()
    else {
        panic!("LOD1");
    };
    let bytes = built
        .read(
            built
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .unwrap(),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();

    fn decode_accessor<'a>(
        j: &serde_json::Value,
        bin: &'a [u8],
        name: &str,
    ) -> Option<Vec<[f32; 4]>> {
        let idx = j["meshes"][0]["primitives"][0]["attributes"][name].as_u64()? as usize;
        let acc = &j["accessors"][idx];
        let view = &j["bufferViews"][acc["bufferView"].as_u64().unwrap() as usize];
        let base = view["byteOffset"].as_u64().unwrap_or(0) as usize
            + acc["byteOffset"].as_u64().unwrap_or(0) as usize;
        let comps = match acc["type"].as_str()? {
            "VEC2" => 2usize,
            "VEC3" => 3,
            "VEC4" => 4,
            _ => return None,
        };
        let count = acc["count"].as_u64()? as usize;
        Some(
            (0..count)
                .map(|i| {
                    let o = base + i * comps * 4;
                    let mut out = [0f32; 4];
                    for (k, slot) in out.iter_mut().enumerate().take(comps) {
                        *slot =
                            f32::from_le_bytes(bin[o + k * 4..o + k * 4 + 4].try_into().unwrap());
                    }
                    out
                })
                .collect(),
        )
    }
    fn report(name: &str, values: &[[f32; 4]], comps: usize) -> String {
        let mut max = 0f32;
        let mut min = f32::MAX;
        for v in values {
            for k in 0..comps {
                max = max.max(v[k].abs());
                min = min.min(v[k]);
            }
        }
        format!("{name}: n={} max_abs={max:.3} min={min:.3}", values.len())
    }

    let glb_json = |glb: &[u8]| -> serde_json::Value {
        let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        let mut c = 12usize;
        while c + 8 <= total {
            let len = u32::from_le_bytes(glb[c..c + 4].try_into().unwrap()) as usize;
            let kind = u32::from_le_bytes(glb[c + 4..c + 8].try_into().unwrap());
            if kind == 0x4E4F534A {
                return serde_json::from_slice(&glb[c + 8..c + 8 + len]).unwrap();
            }
            c += 8 + len;
        }
        panic!("no json");
    };
    let glb_bin = |glb: &[u8]| -> Vec<u8> {
        let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
        let mut c = 12usize;
        while c + 8 <= total {
            let len = u32::from_le_bytes(glb[c..c + 4].try_into().unwrap()) as usize;
            let kind = u32::from_le_bytes(glb[c + 4..c + 8].try_into().unwrap());
            if kind == 0x004E4942 {
                return glb[c + 8..c + 8 + len].to_vec();
            }
            c += 8 + len;
        }
        panic!("no bin");
    };

    // 窗户材质 = params row0.z > 0 的第一个 mesh。
    let mut window_mesh = None;
    for (mesh_index, glb) in doc.meshes.iter().enumerate() {
        let binding = doc.mesh_material.get(mesh_index).unwrap();
        let material = &doc.materials[binding.material as usize];
        let Some(params) = &material.params_f32 else {
            continue;
        };
        if params.len() < 16 {
            continue;
        }
        let interior = f32::from_le_bytes(params[8..12].try_into().unwrap());
        if interior > 0.0 {
            window_mesh = Some(mesh_index);
            break;
        }
    }
    let window_mesh = window_mesh.expect("window material mesh");
    println!("window mesh index = {window_mesh}");

    // 源容器 GLB 数值。
    let src_glb = &doc.meshes[window_mesh];
    let sj = glb_json(src_glb);
    let sbin = glb_bin(src_glb);
    for name in ["TEXCOORD_0", "TEXCOORD_1", "TEXCOORD_2", "TEXCOORD_3"] {
        if let Some(values) = decode_accessor(&sj, &sbin, name) {
            let comps = if name == "TEXCOORD_1" { 2 } else { 2 };
            println!("  src {name} {}", report(name, &values, comps));
        }
    }

    // 再导出 GLB 数值。
    let section = file
        .sections_of_type(rw4::SectionType::MESH)
        .nth(window_mesh)
        .unwrap()
        .number;
    let decoded = file.decode_mesh(&bytes, section).unwrap();
    let out = sc_exporter::export_glb_with_colors(
        &decoded,
        None,
        &[],
        sc_exporter::EmbeddedTextures::default(),
        None,
        None,
    );
    let oj = glb_json(&out.bytes);
    let obin = glb_bin(&out.bytes);
    for name in [
        "TEXCOORD_0",
        "TEXCOORD_1",
        "TEXCOORD_2",
        "TEXCOORD_3",
        "COLOR_0",
    ] {
        if let Some(values) = decode_accessor(&oj, &obin, name) {
            println!("  out {name} {}", report(name, &values, 4));
        } else {
            println!("  out {name}: MISSING");
        }
    }
}

/// Opt-in real export: registers the commercial building as a NEW identity
/// under the park panel (城市>公园, uiCategories 07049250 + 公园子根
/// F970A822 + NightHouse 实证模板对 565B72B9/34CE7E14) — 公园无放置限制，
/// 适合第一轮菜单机制实测（图标/大图/词条/描述/价格/维护费）。
/// 需要 SC_ASSET_FIXTURE_ROOT（同 commercial-model fixture）。

/// 隔离梯 R3：NH 包 + lot LOD1-4 → 极小合并模型（2 三角形，原生形状 RW4）。
/// 判读：放置 NH 后看到小三角块 → 我们的 RW4 容器/写出有效，大模型失败是
/// 规模/复杂度问题；仍是占位符 → 写出有根本缺陷 → 与 NH 原版小模型逐字节
/// 解剖。
#[test]
#[ignore = "ladder R3: NH package + tiny merged model LOD swap"]
fn real_nh_tiny_lodswap_package() {
    use super::native_model::GlbPrimitive;

    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let nh = Package::open(&nh_path).expect("open NH");

    let tri = |dx: f32| GlbPrimitive {
        positions: vec![[dx, 0.0, 0.0], [1.0 + dx, 0.0, 0.0], [dx, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        facade_uvs: vec![[0.0; 4]; 3],
        tint_column: vec![0, 0, 0],
        seeds: vec![0, 0, 0],
        tangents: vec![[1.0, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    // R5：材质六槽指向 NH 的真实 raster（NH 包已安装，引用有效）。
    // R3 的全零槽版本在放置加载时崩溃——单变量实验：槽引用是否为崩溃源。
    let slots = if std::env::var("SC_R5_REAL_SLOTS").is_ok() {
        [
            0x1DBB_C1BD,
            0x1DBB_C1BC,
            0x1DBB_C1BF,
            0x1DBB_C1BE,
            0x1DBB_C1B9,
            0x1DBB_C1B8,
        ]
    } else {
        [0, 0, 0, 0, 0, 0]
    };
    let model = crate::asset_build_m1::native_model::compile_merged_primitives(
        vec![vec![tri(0.0), tri(0.5)], vec![tri(0.25)]],
        slots,
        None,
    )
    .expect("tiny model compile");
    println!(
        "tiny model: {} bytes, {} verts, {} tris",
        model.rw4.len(),
        model.vertex_count,
        model.index_count / 3
    );
    let model_instance = u32::from_be_bytes(
        sha2::Sha256::digest(&model.rw4).as_slice()[..4]
            .try_into()
            .unwrap(),
    );
    let model_key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: model_instance,
    };

    // NH lot：只改 LOD1-4 → tiny 模型；其余（含 NH 自己的单元列）原样。
    let lot_entry = nh
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xA420_E06D,
        })
        .expect("NH lot");
    let mut lot = PropertyFile::parse(&nh.read(lot_entry).unwrap()).unwrap();
    for hash in sc_properties::LOD_MODEL_HASHES {
        if let Some(property) = lot.values.iter_mut().find(|p| p.hash == hash) {
            if let sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) = &mut property.kind
            {
                *key = model_key;
            }
        }
    }
    let lot_bytes = lot.encode_canonical().unwrap();

    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in nh.entries() {
        let payload = if e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000 {
            lot_bytes.clone()
        } else {
            nh.read(e).unwrap()
        };
        entries.push(dbpf::OverlayEntry::new(e.id, payload));
    }
    entries.push(dbpf::OverlayEntry::new(
        ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: model_instance,
        },
        model.rw4.clone(),
    ));

    let out_name = if std::env::var("SC_R5_REAL_SLOTS").is_ok() {
        "tmp/nh_r5_test.package"
    } else {
        "tmp/nh_tiny_test.package"
    };
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(out_name);
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    println!(
        "R3 NH-tiny: {} entries, model {:08X} -> {}",
        entries.len(),
        model_instance,
        out.display()
    );
}

/// 隔离梯 R6：NH 身份 + NH 材质原文 + **NH 顶点布局**的合并模型。
/// 与 R3 的唯一差异 = 顶点格式（stride 40 压缩法线/切线 + facade F4 单 UV
/// 流 + NH 材质原文 vs 我们的 stride 64 双 UV 流 + 全零槽材质）。
/// 需要 SC_NIGHTHOUSE_PACKAGE。
#[test]
#[ignore = "ladder R6: NH identity + NH material + NH vertex layout merged model"]
fn real_nh_r6_nh_layout_package() {
    use super::native_model::GlbPrimitive;
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let nh = Package::open(&nh_path).expect("open NH");

    // NH house 模型的材质 section 原文（441B 真实 CompiledState）
    let nh_model_entry = nh
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C)
        .expect("NH model");
    let nh_model = nh.read(nh_model_entry).unwrap();
    let nh_file = rw4::Rw4File::parse(&nh_model).unwrap();
    let mat_sec = nh_file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .next()
        .expect("NH material");
    let nh_material = nh_file.payload(&nh_model, mat_sec.number).unwrap().to_vec();

    // NH lot 叶子：LOD1-4 改指新模型（字节手术同 R2）
    let lot_entry = nh
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xA420_E06D,
        })
        .expect("NH lot");
    let mut lot = PropertyFile::parse(&nh.read(lot_entry).unwrap()).unwrap();
    let tri = |dx: f32| GlbPrimitive {
        positions: vec![[dx, 0.0, 0.0], [1.0 + dx, 0.0, 0.0], [dx, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]],
        facade_uvs: vec![[0.0; 4]; 3],
        tint_column: vec![0, 0, 0],
        seeds: vec![0, 0, 0],
        tangents: vec![[1.0, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
    };
    let model = crate::asset_build_m1::native_model::compile_merged_primitives_nh(
        vec![vec![tri(0.0), tri(0.5)], vec![tri(0.25)]],
        nh_material,
        None,
    )
    .expect("R6 NH-layout compile");
    let model_instance = u32::from_be_bytes(
        sha2::Sha256::digest(&model.rw4).as_slice()[..4]
            .try_into()
            .unwrap(),
    );
    let model_key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: model_instance,
    };
    println!(
        "R6 model: {} bytes, instance {model_instance:08X}",
        model.rw4.len()
    );
    for hash in sc_properties::LOD_MODEL_HASHES {
        if let Some(property) = lot.values.iter_mut().find(|p| p.hash == hash) {
            if let sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) = &mut property.kind
            {
                *key = model_key;
            }
        }
    }
    let lot_bytes = lot.encode_canonical().unwrap();

    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in nh.entries() {
        let payload = if e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000 {
            lot_bytes.clone()
        } else if e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C {
            model.rw4.clone()
        } else {
            nh.read(e).unwrap()
        };
        entries.push(dbpf::OverlayEntry::new(e.id, payload));
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tmp/nh_r6_test.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    println!(
        "R6 NH-layout: {} entries, model {model_instance:08X} -> {}",
        entries.len(),
        out.display()
    );
}

/// 隔离梯 R7：NH 身份 + **真实 DrVu HQ 几何**（NH 顶点布局分区编译：
/// 每唯一材质一组 mesh + 真实六槽材质/raster 随包）+ NH 图标/locale。
/// 判读：公园面板放置 → 塔楼带真实贴图渲染 → 全链路闭合；
/// 占位/崩溃 → 按 mesh 数（98）或其它差异继续二分。
/// 需要 SC_NIGHTHOUSE_PACKAGE + SC_ASSET_FIXTURE_ROOT。
#[test]
#[ignore = "ladder R7: NH identity + real model in NH vertex layout"]
fn real_r7_nh_layout_real_model_package() {
    let fixture = std::path::PathBuf::from(
        std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"),
    );
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let nh = Package::open(&nh_path).expect("open NH");

    // 1) 真实几何 + 真实材质
    let lotm_bytes =
        std::fs::read(fixture.join("commercial-model/model.lotm")).expect("workbench LOTM");
    let doc = super::lotm::parse(&lotm_bytes).expect("LOTM parse");
    let material_plan = super::materials::resolve_materials(&doc).expect("material plan");
    let mut scratch: std::collections::HashMap<dbpf::ResourceId, Vec<u8>> =
        std::collections::HashMap::new();
    let material_resources =
        super::materials::build_resources(&material_plan, |type_id: u32, prepared: &[u8]| {
            let mut instance = u32::from_be_bytes(
                sha2::Sha256::digest(prepared).as_slice()[..4]
                    .try_into()
                    .unwrap(),
            );
            while scratch.contains_key(&dbpf::ResourceId {
                type_id,
                group: 0,
                instance,
            }) {
                instance = instance.wrapping_add(1);
            }
            scratch.insert(
                dbpf::ResourceId {
                    type_id,
                    group: 0,
                    instance,
                },
                prepared.to_vec(),
            );
            Ok(instance)
        })
        .expect("build resources");
    let model = crate::asset_build_m1::native_model::compile_lotm_nh_partitioned(
        &doc,
        &material_plan.mesh_material,
        &material_resources.unique_instances,
        None,
    )
    .expect("R7 NH-partitioned compile");
    let model_instance = u32::from_be_bytes(
        sha2::Sha256::digest(&model.rw4).as_slice()[..4]
            .try_into()
            .unwrap(),
    );
    let model_key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: model_instance,
    };
    println!(
        "R7 model: {} bytes, {} verts, {} tris, {} materials, instance {model_instance:08X}",
        model.rw4.len(),
        model.vertex_count,
        model.index_count / 3,
        model.material_count
    );

    // 2) NH 身份：lot 叶子 LOD1-4 → 真实模型
    let lot_entry = nh
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xA420_E06D,
        })
        .expect("NH lot");
    let mut lot = PropertyFile::parse(&nh.read(lot_entry).unwrap()).unwrap();
    for hash in sc_properties::LOD_MODEL_HASHES {
        if let Some(property) = lot.values.iter_mut().find(|p| p.hash == hash) {
            if let sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) = &mut property.kind
            {
                *key = model_key;
            }
        }
    }
    let lot_bytes = lot.encode_canonical().unwrap();

    // 3) 组包：NH 全部条目（lot 换补丁版、LOD1 换真实模型）+ 真实材质/raster
    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in nh.entries() {
        let payload = if e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000 {
            lot_bytes.clone()
        } else if e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C {
            model.rw4.clone()
        } else {
            nh.read(e).unwrap()
        };
        entries.push(dbpf::OverlayEntry::new(e.id, payload));
    }
    for (id, payload) in &scratch {
        entries.push(dbpf::OverlayEntry::new(*id, payload.clone()));
    }
    println!("R7 entries: {}", entries.len());

    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tmp/r7_real_test.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    println!("R7 written -> {}", out.display());
}

/// 隔离梯 R8：NH 身份 + 真实塔楼几何（NH 顶点布局）+ **Dalian 材质原文**
/// （525B Building40 CompiledState，内部 Raster 区块引用 @377 重定位到
/// 我们的占位 Raster section）。第十六阶段文档实证：材质尾部内部引用
/// 缺失/错配 = 楼体失败 + 选择崩溃（0.8.3 同症状）。
/// 需要 SC_NIGHTHOUSE_PACKAGE + SC_ASSET_FIXTURE_ROOT + Dalian 包路径。
#[test]
#[ignore = "ladder R8: NH identity + real tower + Dalian Building40 material"]
fn real_r8_dalian_material_package() {
    use super::native_model::GlbPrimitive;
    let fixture = std::path::PathBuf::from(
        std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"),
    );
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let dalian_path = std::env::var("SC_DALIAN_PACKAGE")
        .unwrap_or_else(|_| "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityData/SC5_DalianHopeTower_Education_0_8_5.package".into());
    let nh = Package::open(&nh_path).expect("open NH");
    let dalian = Package::open(&dalian_path).expect("open Dalian");

    // Dalian 材质原文（525B Building40）
    let dalian_model_entry = dalian
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0xB9B0_8494)
        .expect("Dalian model");
    let dalian_model = dalian.read(dalian_model_entry).unwrap();
    let dalian_file = rw4::Rw4File::parse(&dalian_model).unwrap();
    let dalian_mat_sec = dalian_file
        .sections_of_type(rw4::SectionType::MATERIAL)
        .next()
        .expect("Dalian material");
    let dalian_material = dalian_file
        .payload(&dalian_model, dalian_mat_sec.number)
        .unwrap()
        .to_vec();

    // 真实塔楼几何（工作台 LOTM）NH 布局编译，材质用 Dalian 原文
    let lotm_bytes = std::fs::read(fixture.join("commercial-model/model.lotm")).expect("LOTM");
    let doc = super::lotm::parse(&lotm_bytes).expect("LOTM parse");
    let model = crate::asset_build_m1::native_model::compile_merged_primitives_nh(
        {
            let mut per_mesh = Vec::with_capacity(doc.meshes.len());
            for glb in &doc.meshes {
                per_mesh.push(
                    crate::asset_build_m1::native_model::parse_glb_primitives(glb).expect("glb"),
                );
            }
            per_mesh
        },
        dalian_material,
        None,
    )
    .expect("R8 compile");
    let model_instance = u32::from_be_bytes(
        sha2::Sha256::digest(&model.rw4).as_slice()[..4]
            .try_into()
            .unwrap(),
    );
    let model_key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: model_instance,
    };
    println!(
        "R8 model: {} bytes, instance {model_instance:08X}, material = Dalian 525B Building40",
        model.rw4.len()
    );

    // NH 身份：lot LOD1-4 → R8 模型；LOD1 资源位 → R8 模型
    let lot_entry = nh
        .entry(ResourceId {
            type_id: 0x00B1_B104,
            group: 0x40E1_C000,
            instance: 0xA420_E06D,
        })
        .expect("NH lot");
    let mut lot = PropertyFile::parse(&nh.read(lot_entry).unwrap()).unwrap();
    for hash in sc_properties::LOD_MODEL_HASHES {
        if let Some(property) = lot.values.iter_mut().find(|p| p.hash == hash) {
            if let sc_properties::Kind::Scalar(sc_properties::Value::Key(key)) = &mut property.kind
            {
                *key = model_key;
            }
        }
    }
    let lot_bytes = lot.encode_canonical().unwrap();

    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in nh.entries() {
        let payload = if e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000 {
            lot_bytes.clone()
        } else if e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C {
            model.rw4.clone()
        } else {
            nh.read(e).unwrap()
        };
        entries.push(dbpf::OverlayEntry::new(e.id, payload));
    }
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tmp/nh_r8_test.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    println!(
        "R8: {} entries, model {model_instance:08X} -> {}（放置 NH 条目，预期：房子位置出现我们的塔楼几何，贴图缺失属预期）",
        entries.len(),
        out.display()
    );
}

/// 隔离梯 R9：NH 模板**逐字节重建**（容器层 2026-10-31 已离线零差异验证：
/// build_template 对 NH 原版模型 51488/51488 字节一致）+ 真实塔楼几何注入。
/// 容器（头部/类型表/45968B 间隙/w25/0x10007/材质/Raster/CollisionMesh）
/// 与 NH 原版逐字节相同；唯一差异 = 6 个几何字段：
/// BBox、索引 Blob、TA（计数+索引宽度标志）、Mesh（画域）、顶点 Blob、VA（计数）。
/// 索引宽度按 Dalian 塔楼先例：顶点 >65535 用 u32（TA 标志 102），否则 u16（101）。
/// 判读：R9 上游戏若仍失败，则问题锁定在几何内容本身（编码/放置/碰撞语义），
/// 容器层嫌疑已被零差异验证排除。
/// 需要 SC_NIGHTHOUSE_PACKAGE + SC_ASSET_FIXTURE_ROOT。
/// R9 已上游戏验证（塔楼完整渲染）的塔楼模型构建：NH 模板逐字节布局
/// （build_template 零差异验证的布局器）+ 商业样本 LOTM 几何注入；材质/
/// CollisionMesh/纹理 blob/Texture section 全部 NH 原文，内部引用自洽。
/// 返回（模型字节, 内容哈希 instance）。
fn build_nh_template_tower_model(nh: &Package, fixture_root: &std::path::Path) -> (Vec<u8>, u32) {
    use super::lotm;
    use super::native_model::{parse_glb_primitives, placement_from_bounds};
    use rw4::writer::{SectionSpec, TemplatePiece, build_template};
    use rw4::{Rw4File, SectionType};

    // ---- 1) NH 模板提取（同 nh_template_roundtrip：间隙+保序类型表+特殊块）----
    let nh_entry = nh
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C)
        .expect("NH LOD1 model");
    let original = nh.read(nh_entry).unwrap();
    let file = Rw4File::parse(&original).unwrap();

    let words_at = |data: &[u8], off: usize, count: usize| -> Vec<u32> {
        (0..count)
            .map(|i| u32::from_le_bytes(data[off + i * 4..off + i * 4 + 4].try_into().unwrap()))
            .collect()
    };
    let patch_word = |payload: &[u8], index: usize, value: u32| -> Vec<u8> {
        let mut p = payload.to_vec();
        p[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
        p
    };

    // 类型表：固定 5 项 + 追加段按数值升序（NH 原版约定）
    let mut type_table: Vec<u32> = vec![0, 0x10030, 0x10031, 0x10032, 0x10010];
    let mut prev_end = 0x138u32;
    let mut pieces: Vec<TemplatePiece> = Vec::new(); // 三个 blob 的 section 序号 = [索引, 顶点, 纹理]（按 section 号序）
    let mut blob_numbers: Vec<u32> = Vec::new();
    for section in file.sections() {
        let payload = file.payload(&original, section.number).unwrap().to_vec();
        if section.type_code == SectionType::VERTEX_FORMAT {
            let gap = section.pos - prev_end;
            if gap > 0 {
                pieces.push(TemplatePiece::Gap(gap));
            }
        }
        prev_end = section.pos + section.size;
        let alignment = if section.alignment == 0 {
            4
        } else {
            section.alignment
        };
        pieces.push(TemplatePiece::Section(SectionSpec {
            type_code: section.type_code,
            alignment,
            payload,
            fixups: Vec::new(),
        }));
        if section.type_code == SectionType::BLOB {
            blob_numbers.push(section.number);
        }
        if !type_table.contains(&section.type_code) {
            type_table.push(section.type_code);
        }
    }
    let mut extras: Vec<u32> = type_table[5..].to_vec();
    extras.sort_unstable();
    type_table.truncate(5);
    type_table.extend_from_slice(&extras);
    assert_eq!(
        blob_numbers.len(),
        3,
        "NH template has index/vertex/tex blobs"
    );

    let w25 = words_at(&original, 0x64, 1)[0];
    // 0x10007 块：H140 第 4 个偏移（0x98+0x1c=0xb4 起）指向它
    let b10007_off = 0x98 + words_at(&original, 0x98 + 4 + 16, 1)[0] as usize;
    let block10007: [u32; 6] = words_at(&original, b10007_off + 4, 6).try_into().unwrap();
    let nh_payload = |num: u32| -> Vec<u8> { file.payload(&original, num).unwrap().to_vec() };
    let bbox_nh = nh_payload(0);
    let ta_nh = nh_payload(4);
    let mesh_nh = nh_payload(5);
    let va_nh = nh_payload(9);
    assert_eq!(ta_nh[6 * 4..7 * 4], 3u32.to_le_bytes(), "TA.data -> blob#3");
    assert_eq!(va_nh[6 * 4..7 * 4], 8u32.to_le_bytes(), "VA.data -> blob#8");

    // ---- 2) 真实塔楼几何（工作台 LOTM 全部 mesh，单材质 0 = NH 材质）----
    let lotm_bytes =
        fs::read(fixture_root.join("commercial-model/model.lotm")).expect("workbench LOTM");
    let doc = lotm::parse(&lotm_bytes).expect("LOTM parse");
    let mut per_mesh = Vec::with_capacity(doc.meshes.len());
    for glb in &doc.meshes {
        per_mesh.push(parse_glb_primitives(glb).expect("GLB primitives"));
    }
    let placement = placement_from_bounds(per_mesh.iter().flatten());
    let total_vertices: usize = per_mesh.iter().flatten().map(|p| p.positions.len()).sum();
    let total_indices: usize = per_mesh.iter().flatten().map(|p| p.indices.len()).sum();
    assert!(
        total_vertices <= u32::MAX as usize,
        "vertex count exceeds container range"
    );
    // u16 索引只能寻址 65536 顶点；超出按 Dalian 先例切 u32（TA 标志 102）
    let (index_width, ta_index_flag): (usize, u32) = if total_vertices > 65536 {
        (4, 102)
    } else {
        (2, 101)
    };

    let mut vertices = Vec::with_capacity(total_vertices * 40);
    let mut indices = Vec::with_capacity(total_indices * index_width);
    let mut base: u32 = 0;
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for primitive in per_mesh.iter().flatten() {
        for i in 0..primitive.positions.len() {
            let p = placement.apply(primitive.positions[i]);
            for value in p {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
            for axis in 0..3 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
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
        for &index in &primitive.indices {
            let value = index + base;
            if index_width == 4 {
                indices.extend_from_slice(&value.to_le_bytes());
            } else {
                indices.extend_from_slice(&(value as u16).to_le_bytes());
            }
        }
        base += primitive.positions.len() as u32;
    }
    let vertex_count = total_vertices as u32;
    let index_count = total_indices as u32;
    let tri_count = index_count / 3;
    assert_eq!(
        vertices.len(),
        vertex_count as usize * 40,
        "stride-40 vertex blob"
    );

    // ---- 3) 六个几何字段的模板 patch ----
    // BBox：word3/word7 是原版常量（0x12df858 / 0），只换 min/max
    let mut bbox = bbox_nh.clone();
    for (i, v) in [min[0], min[1], min[2], max[0], max[1], max[2]]
        .iter()
        .enumerate()
    {
        let at = if i < 3 { i * 4 } else { (i + 1) * 4 };
        bbox[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }
    // TA：word2 = 索引数，word4 = 索引宽度标志（101=u16 / 102=u32）
    let ta = patch_word(&patch_word(&ta_nh, 2, index_count), 4, ta_index_flag);
    // Mesh：word3 = 三角形数，word6 = 索引数，word8 = 顶点数（画域字段）
    let mesh = patch_word(
        &patch_word(&patch_word(&mesh_nh, 3, tri_count), 6, index_count),
        8,
        vertex_count,
    );
    // VA：word3 = 顶点数
    let va = patch_word(&va_nh, 3, vertex_count);

    // 按类型定位并替换（NH 单 mesh 模板：各类型恰好出现一次；blob 按序号序
    // = [索引, 顶点, 纹理]，前两个换成塔楼数据，第三个保持 NH 原文）
    let mut replaced = pieces;
    let mut seen = std::collections::HashSet::new();
    let mut blob_ordinal = 0usize;
    for piece in replaced.iter_mut() {
        let TemplatePiece::Section(s) = piece else {
            continue;
        };
        let dup = !seen.insert(s.type_code) && s.type_code != SectionType::BLOB;
        assert!(!dup, "unexpected duplicate {:#x}", s.type_code);
        match s.type_code {
            SectionType::BBOX => s.payload = bbox.clone(),
            SectionType::TRIANGLE_ARRAY => s.payload = ta.clone(),
            SectionType::MESH => s.payload = mesh.clone(),
            SectionType::VERTEX_ARRAY => s.payload = va.clone(),
            SectionType::BLOB => {
                if blob_ordinal == 0 {
                    s.payload = indices.clone();
                } else if blob_ordinal == 1 {
                    s.payload = vertices.clone();
                }
                blob_ordinal += 1;
            }
            _ => {}
        }
    }
    assert_eq!(blob_ordinal, 3, "three blobs in template");
    let tower = build_template(1, &replaced, &type_table, w25, block10007);
    println!(
        "R9 tower: {} bytes (NH 51488), {} verts, {} tris, index u{}/{}, instance pending",
        tower.len(),
        vertex_count,
        tri_count,
        index_width * 8,
        ta_index_flag
    );

    // ---- 4) 离线复核：解析 + 画域 roundtrip + 结构 diff ----
    let tf = Rw4File::parse(&tower).expect("R9 tower parse");
    let decoded = tf.decode_mesh(&tower, 5).expect("R9 tower mesh decode");
    assert_eq!(decoded.vertices.len() as u32, vertex_count, "decoded verts");
    assert_eq!(decoded.triangles.len() as u32, tri_count, "decoded tris");
    // 容器不变量：未替换 section 逐字节等于 NH
    let replaced_types = [
        SectionType::BBOX,
        SectionType::TRIANGLE_ARRAY,
        SectionType::MESH,
        SectionType::VERTEX_ARRAY,
        SectionType::BLOB,
    ];
    let mut blobs_seen = 0usize;
    for section in tf.sections() {
        let payload = tf.payload(&tower, section.number).unwrap();
        if replaced_types.contains(&section.type_code) {
            if section.type_code == SectionType::BLOB {
                blobs_seen += 1;
                if blobs_seen == 3 {
                    assert_eq!(
                        payload,
                        nh_payload(blob_numbers[2]),
                        "texture blob must stay NH verbatim"
                    );
                }
            }
            continue;
        }
        assert_eq!(
            payload,
            nh_payload(section.number),
            "section {} must stay NH verbatim",
            section.number
        );
    }
    assert_eq!(blobs_seen, 3, "three blobs");
    // 头部字段：除 @0x4c（blob 区大小）外全部一致
    for off in (0x1c..0x98).step_by(4) {
        if off == 0x4c {
            continue;
        }
        assert_eq!(
            words_at(&original, off, 1)[0],
            words_at(&tower, off, 1)[0],
            "header word @0x{off:x}"
        );
    }
    std::fs::write(workspace_tmp().join("nh_r9_tower.rw4"), &tower).unwrap();
    println!("R9-model VERIFY OK: container identity + geometry roundtrip");
    let model_instance = u32::from_be_bytes(
        sha2::Sha256::digest(&tower).as_slice()[..4]
            .try_into()
            .unwrap(),
    );
    (tower, model_instance)
}

#[test]
#[ignore = "ladder R9: NH template byte-exact + real tower geometry injected"]
fn real_r9_template_tower_package() {
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let nh = Package::open(&nh_path).expect("open NH");
    let (tower, model_instance) = build_nh_template_tower_model(&nh, &fixture);

    // 组包：NH 全部条目原样保留，仅四个 LOD 模型条目（06C5470C-F，
    // 原版为同一 house 的字节级拷贝，lot 的 LOD1-4 键原样指向它们）换成
    // 塔楼模型。lot/locale/图标/属性逐字节不动 = 包与 NH 原包的唯一差异
    // 是模型几何本身。（历史隔离梯：此为覆盖模式，仅存档不再部署——
    // 2026-10-31 用户指令：游戏验证一律新建菜单、禁止覆盖。）
    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in nh.entries() {
        let is_lod_model =
            e.id.type_id == 0x2F4E_681B && (0x06C5_470C..=0x06C5_470F).contains(&e.id.instance);
        let payload = if is_lod_model {
            tower.clone()
        } else {
            nh.read(e).unwrap()
        };
        entries.push(dbpf::OverlayEntry::new(e.id, payload));
    }
    let out = workspace_tmp().join("nh_r9_template_tower.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    println!(
        "R9: {} entries, tower {model_instance:08X} -> {}（NH 模板 + 塔楼几何，lot 未动，仅 4 个 LOD 模型条目换载）",
        entries.len(),
        out.display()
    );
}

/// 隔离梯 R10：自建商业样本（资产编辑器搭建的"商业地标资产开发"工作台
/// 文档）走 M1 全链路（LOTM → 材质 PNG→六槽 raster → NH 顶点布局模型 →
/// lot/菜单 lowering），模型形状 = nh-partitioned（2026-10-31 R9 上游戏
/// 验证：NH 模板 + u32 索引塔楼完整渲染）。窗户/地皮继承样本自身语义
/// （slot0 参数表 + tint map + palette；LotSize 来自 asset.lot.json）。
/// 离线判据：模型可解析、mesh 画域 roundtrip、六槽引用非零、包 delta
/// 只含本构建资源。
/// 需要 SC_ASSET_FIXTURE_ROOT=…/tmp/asset-build-fixtures。
#[test]
#[ignore = "ladder R10: workbench commercial sample full chain, nh-partitioned shape"]
fn real_r10_workbench_nh_partitioned_package() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial-model");
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    // 注入 nh-partitioned 形状 + 菜单注册（R1 实证可进公园菜单的配方：
    // 公园工具子分类 07049250 + 公园菜单子根 F970A822 + NightHouse 实证
    // 模板对 565B72B9/34CE7E14）。覆盖 DLC0 不可见资产的方式已证实无用。
    let mut schema: serde_json::Value = serde_json::from_str(&raw).unwrap();
    schema["scene"]["modelShape"] = serde_json::Value::String("nh-atlas".into());
    schema["semantics"]["menu"] = serde_json::json!({
        "title": "商业塔楼 R10",
        "categoryInstance": "0x07049250",
        "menuParentInstance": "0xF970A822",
        "unitTemplateInstance": "0x565B72B9",
        "lotTemplateInstance": "0x34CE7E14"
    });
    let raw = serde_json::to_string(&schema).unwrap();
    let started = std::time::Instant::now();
    let result =
        build_from_schema(&root, &raw).unwrap_or_else(|error| panic!("R10 build failed: {error}"));
    println!("R10 build: {result:?} took {:?}", started.elapsed());
    assert!(result.path.is_file());
    assert!(result.scene.material_resources > 0, "materials inserted");
    let menu_instance = result
        .menu_instance
        .expect("menu registration produced a building instance");
    println!("R10 menu instance: {menu_instance:08X}");

    // 模型复核：定位 lot LOD1 → 模型 → 解析 + 画域 roundtrip + u32 标志
    let output = Package::open(&result.path).unwrap();
    let mut lot_entry = None;
    for e in output.entries() {
        if e.id.type_id == 0x00B1_B104 {
            let pf = PropertyFile::parse(&output.read(e).unwrap()).unwrap();
            if pf.get(sc_properties::LOD1_MODEL_HASH).is_some() {
                lot_entry = Some((e.id, pf));
                break;
            }
        }
    }
    let (lot_id, lot_file) = lot_entry.expect("lot property with LOD1 in output");
    let model_key = match lot_file
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned())
    {
        Some(Value::Key(k)) => k,
        other => panic!("LOD1 key retargeted, got {other:?}"),
    };
    let model_bytes = output
        .read(
            output
                .entry(ResourceId {
                    type_id: model_key.type_id,
                    group: model_key.group,
                    instance: model_key.instance,
                })
                .expect("compiled model in package"),
        )
        .unwrap();
    let file = rw4::Rw4File::parse(&model_bytes).expect("R10 model parse");
    let meshes: Vec<_> = file.sections_of_type(rw4::SectionType::MESH).collect();
    let tas: Vec<_> = file
        .sections_of_type(rw4::SectionType::TRIANGLE_ARRAY)
        .collect();
    println!(
        "R10 model: {} bytes, {} meshes, instance {:08X}, lot {:08X}/{:08X}",
        model_bytes.len(),
        meshes.len(),
        model_key.instance,
        lot_id.group,
        lot_id.instance
    );
    let mut total_verts = 0usize;
    let mut total_tris = 0usize;
    for m in &meshes {
        let decoded = file
            .decode_mesh(&model_bytes, m.number)
            .expect("mesh decode");
        total_verts += decoded.vertices.len();
        total_tris += decoded.triangles.len();
    }
    println!("R10 geometry: {total_verts} verts, {total_tris} tris (decode roundtrip)");
    assert!(total_verts > 0 && total_tris > 0);
    // u32 标志：塔楼 >65536 顶点时 TA 应为 102
    let ta_payload = file.payload(&model_bytes, tas[0].number).unwrap();
    let ta_flag = u32::from_le_bytes(ta_payload[16..20].try_into().unwrap());
    println!("R10 TA index format flag: {ta_flag}");
    if total_verts > 65536 {
        assert_eq!(ta_flag, 102, "wide vertex pool needs u32 indices");
    }
    // 材质六槽引用：MMA → MATERIAL payload 的 slot0..5 实例号应非零
    let mats: Vec<_> = file.sections_of_type(rw4::SectionType::MATERIAL).collect();
    assert!(!mats.is_empty(), "model carries materials");
    let bindings = file.decode_mesh_material_bindings(&model_bytes);
    println!(
        "R10 materials: {} sections, {} bindings",
        mats.len(),
        bindings.len()
    );
    // 包内六槽 raster 资源存在（材质引用的 texture instance 都能找到）
    let mut slot_refs = 0usize;
    for mat in &mats {
        let payload = file.payload(&model_bytes, mat.number).unwrap();
        for w in payload.chunks_exact(4) {
            let v = u32::from_le_bytes(w.try_into().unwrap());
            if v != 0
                && output
                    .entries()
                    .iter()
                    .any(|e| e.id.instance == v && e.id.type_id == 0x2F4E_681C)
            {
                slot_refs += 1;
            }
        }
    }
    println!("R10 slot raster refs resolvable in package: {slot_refs}");
    assert!(
        slot_refs > 0,
        "material texture instances must land in package"
    );
    let _ = workspace_tmp().join("r10_marker");
}

/// 隔离梯 R11：菜单身份（R10 实证配方）+ **R9 已上游戏验证的塔楼模型**
/// （NH 模板逐字节布局：材质原文/CollisionMesh/纹理 blob/Texture section
/// 内部引用全部自洽）。R10 崩溃根因 = 自生成材质（无内部引用、结构非原
/// 版 CompiledState）+ Texture section 悬空引用 section 10 + 无 CollisionMesh；
/// 《新资产可逆性调研》§6 的路线即"先同拓扑替换、不动材质 section"——
/// R9 已验证该形态可渲染，R11 只把包身份从 NH 覆盖换成新建菜单。
/// 材质引用的 package 级纹理实例随 NH 纹理条目入包（其余靠基础包，与
/// R9 上游戏场景相同）。离线判据：lot LOD 键 → 包内存在的模型条目、
/// 模型解析 + 引用自洽、菜单五件套齐。
/// 需要 SC_NIGHTHOUSE_PACKAGE + SC_ASSET_FIXTURE_ROOT。
#[test]
#[ignore = "ladder R11: menu identity + R9-verified NH-template tower"]
fn real_r11_menu_identity_verified_tower() {
    // ---- 1) R10 菜单构建（同 R10：nh-partitioned + 公园菜单配方）----
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial-model");
    let raw = fs::read_to_string(root.join("schema.json")).unwrap();
    let mut schema: serde_json::Value = serde_json::from_str(&raw).unwrap();
    schema["scene"]["modelShape"] = serde_json::Value::String("nh-atlas".into());
    schema["semantics"]["menu"] = serde_json::json!({
        "title": "商业塔楼 R11",
        "categoryInstance": "0x07049250",
        "menuParentInstance": "0xF970A822",
        "unitTemplateInstance": "0x565B72B9",
        "lotTemplateInstance": "0x34CE7E14"
    });
    let result = build_from_schema(&root, &serde_json::to_string(&schema).unwrap())
        .unwrap_or_else(|error| panic!("R11 base build failed: {error}"));
    let menu_instance = result
        .menu_instance
        .expect("menu registration produced a building instance");
    println!("R11 menu instance: {menu_instance:08X}");

    // ---- 2) R9 已上游戏验证的塔楼模型（NH 模板 + 塔楼几何）----
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let nh = Package::open(&nh_path).expect("open NH");
    let (tower, tower_instance) = build_nh_template_tower_model(&nh, &fixture);
    println!("R11 tower model: instance {tower_instance:08X}");

    // ---- 3) 组包：R10 产物全部条目 + 模型换 R9 塔楼 + lot LOD 键重定向
    // + NH 纹理条目（材质引用的 package 级实例）----
    let r10 = Package::open(&result.path).expect("open R10 build output");
    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for e in r10.entries() {
        let is_scene_model =
            e.id.type_id == 0x2F4E_681B && e.id.group == 0 && r10.read(e).unwrap().len() > 4096;
        let (id, payload) = if is_scene_model {
            // 场景大模型（A2AA0912）换成 R9 塔楼：载荷与 TGI 一起换成内容
            // 哈希实例（5C786C64），lot LOD 键同批重定向；512B 占位模型
            // 保留（菜单/单元链可能引用它们）。
            (
                ResourceId {
                    type_id: 0x2F4E_681B,
                    group: 0,
                    instance: tower_instance,
                },
                tower.clone(),
            )
        } else {
            (e.id, r10.read(e).unwrap())
        };
        entries.push(dbpf::OverlayEntry::new(id, payload));
    }
    for e in nh.entries() {
        if e.id.type_id == 0x2F4E_681C {
            entries.push(dbpf::OverlayEntry::new(e.id, nh.read(e).unwrap()));
        }
    }
    // lot LOD1-4 → 塔楼模型（菜单克隆的新 lot 叶子）
    let lot_id = ResourceId {
        type_id: 0x00B1_B104,
        group: 0x40E1_C000,
        instance: menu_instance,
    };
    let lot_entry = r10.entry(lot_id).expect("menu lot leaf");
    let mut lot = PropertyFile::parse(&r10.read(lot_entry).unwrap()).unwrap();
    let tower_key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: tower_instance,
    };
    for hash in sc_properties::LOD_MODEL_HASHES {
        if let Some(property) = lot.values.iter_mut().find(|p| p.hash == hash) {
            if let Kind::Scalar(Value::Key(key)) = &mut property.kind {
                *key = tower_key;
            }
        }
    }
    let lot_bytes = lot.encode_canonical().unwrap();
    entries.retain(|e| e.id != lot_id);
    entries.push(dbpf::OverlayEntry::new(lot_id, lot_bytes));

    // ---- 4) 离线复核 ----
    let out = workspace_tmp().join("nh_r11_menu_tower.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    let check = Package::open(&out).expect("reopen R11");
    let model_entry = check
        .entry(ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: tower_instance,
        })
        .expect("tower model in package");
    let model_bytes = check.read(model_entry).unwrap();
    let mf = rw4::Rw4File::parse(&model_bytes).expect("tower parse");
    assert_eq!(
        mf.sections_of_type(rw4::SectionType::MESH).count(),
        1,
        "R9 tower has one mesh"
    );
    // lot LOD 键全部指向包内存在的模型
    let lot_check =
        PropertyFile::parse(&check.read(check.entry(lot_id).unwrap()).unwrap()).unwrap();
    for hash in sc_properties::LOD_MODEL_HASHES {
        let key = match lot_check.get(hash).and_then(|p| p.scalar().cloned()) {
            Some(Value::Key(k)) => k,
            other => panic!("LOD hash {hash:08X} missing: {other:?}"),
        };
        assert_eq!(key.instance, tower_instance, "LOD {hash:08X} retargeted");
        assert!(
            check
                .entry(ResourceId {
                    type_id: key.type_id,
                    group: key.group,
                    instance: key.instance
                })
                .is_some(),
            "LOD target must exist in package"
        );
    }
    // 菜单五件套 + NH 纹理条目在包内
    for (t, g, i) in [
        (0x00B1_B104u32, 0x0987_8A01u32, menu_instance),
        (0x00B1_B104, 0x40E0_C000, menu_instance),
        (0x2F7D_0004, 0x40E0_2400, menu_instance),
    ] {
        assert!(
            check
                .entry(ResourceId {
                    type_id: t,
                    group: g,
                    instance: i
                })
                .is_some(),
            "menu resource {t:08X}/{g:08X} missing"
        );
    }
    assert!(
        check
            .entries()
            .iter()
            .any(|e| e.id.type_id == 0x2F4E_681C && e.id.instance == 0x54F2_BB03),
        "NH lot mask raster in package"
    );
    println!(
        "R11: {} entries -> {}（菜单身份 + R9 已验证塔楼模型 + NH 纹理）",
        entries.len(),
        out.display()
    );
}

fn workspace_tmp() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("tmp")
}

/// 隔离梯 R1：NightHouse 克隆身份包——NH 的三个身份 property 逐字段保留
/// （该配方在本机离线安装实测可进公园菜单），仅替换：instance
/// （A420E06D→BA5224C6，含 menuUnit/lot-link/self-ref/icon 引用）、
/// LOD1-4 → 我们的 merged-single 模型（0x8A096154，T2 已证可加载）、
/// locale 表内容（TGI 照抄 NH 的 78BAB639，零售包无碰撞）。
/// 判读：R1 进菜单 → 崩溃源在我们手写字段，逐组二分；R1 仍崩 → 差异在
/// {模型, locale, 图标, instance}，继续换装二分。
/// 需要 SC_NIGHTHOUSE_PACKAGE（NH 包路径）。
#[test]
#[ignore = "ladder R1: byte-faithful NightHouse identity clone + our model"]
fn real_nh_clone_identity_package() {
    let nh_path = std::env::var("SC_NIGHTHOUSE_PACKAGE").unwrap_or_else(|_| {
        "D:/ea-games/SimCity/SimCityData/SC5_NightHouse_Test_0_5_0.package".into()
    });
    let workspace: PathBuf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let merged_pkg_path = workspace
        .join("tmp/asset-build-fixtures/commercial-model/.build")
        .join("fc209ccc34e1512ceb4b1b2ec22d4343f7dfd582b982c11eee65c4e455e4a7c9")
        .join("packages/commercial-menu-park.package");
    let nh = Package::open(&nh_path).expect("open NH package");
    let ours = Package::open(&merged_pkg_path).expect("open merged menu package");

    const OLD_INSTANCE: u32 = 0xA420_E06D;
    const NEW_INSTANCE: u32 = 0xBA52_24C6;
    const OUR_MODEL: sc_properties::Key = sc_properties::Key {
        type_id: 0x2F4E_681B,
        group: 0,
        instance: 0x8A09_6154,
    };

    // 1) NH 三个身份 property：实例重映射 + LOD 换装
    let groups = [0x0987_8A01u32, 0x40E0_C000, 0x40E1_C000];
    let mut entries: Vec<dbpf::OverlayEntry> = Vec::new();
    for group in groups {
        let entry = nh
            .entry(ResourceId {
                type_id: 0x00B1_B104,
                group,
                instance: OLD_INSTANCE,
            })
            .unwrap_or_else(|| panic!("NH property group {group:#x} missing"));
        let mut file = PropertyFile::parse(&nh.read(entry).unwrap()).unwrap();
        for property in file.values.iter_mut() {
            match &mut property.kind {
                sc_properties::Kind::Scalar(value) => {
                    remap_value(value, OLD_INSTANCE, NEW_INSTANCE, &OUR_MODEL)
                }
                sc_properties::Kind::Array(values) => {
                    for value in values.iter_mut() {
                        remap_value(value, OLD_INSTANCE, NEW_INSTANCE, &OUR_MODEL);
                    }
                }
                sc_properties::Kind::Empty => {}
            }
        }
        let bytes = file.encode_canonical().unwrap();
        entries.push(dbpf::OverlayEntry::new(
            ResourceId {
                type_id: 0x00B1_B104,
                group,
                instance: NEW_INSTANCE,
            },
            bytes,
        ));
    }

    // 2) NH 图标/大图：换 instance 随包携带（不依赖 NH 包安装）
    for (type_id, group) in [(0x2F7D_0004u32, 0x40E0_2400u32), (0x3F86_62EAu32, 0u32)] {
        let entry = nh
            .entry(ResourceId {
                type_id,
                group,
                instance: OLD_INSTANCE,
            })
            .unwrap_or_else(|| panic!("NH icon {type_id:08X} missing"));
        entries.push(dbpf::OverlayEntry::new(
            ResourceId {
                type_id,
                group,
                instance: NEW_INSTANCE,
            },
            nh.read(entry).unwrap(),
        ));
    }

    // 3) locale 表：NH 同 TGI（78BAB639@02FABF01），ids 1/2 = 我们的标题/描述
    let locale_text = "{
    \"0x00000001\" : \"OpenSCP Commercial Park (NH clone)\",
    \"0x00000002\" : \"R1 ladder: NightHouse identity clone + our merged model\",
    \"0xffffffff\" : \"0xffffffff\"
}";
    let mut locale_json: Vec<u8> = vec![0xEF, 0xBB, 0xBF];
    locale_json.extend_from_slice(locale_text.as_bytes());
    entries.push(dbpf::OverlayEntry::new(
        ResourceId {
            type_id: 0x0A98_EAF0,
            group: 0x02FA_BF01,
            instance: 0x78BA_B639,
        },
        locale_json.to_vec(),
    ));

    // 4) 我们的模型 + 材质资源（排除手写身份/图标/locale）
    for entry in ours.entries() {
        let id = entry.id;
        let skip = (id.type_id == 0x00B1_B104)
            || (id.type_id == 0x0A98_EAF0)
            || (id.type_id == 0x2F7D_0004)
            || (id.type_id == 0x3F86_62EA);
        if skip {
            continue;
        }
        entries.push(dbpf::OverlayEntry::new(id, ours.read(entry).unwrap()));
    }

    let out = workspace.join("tmp/nh-clone-test.package");
    write_uncompressed_overlay_to_path(&out, &entries).unwrap();
    let check = Package::open(&out).unwrap();
    assert_eq!(check.entries().len(), entries.len());
    for group in groups {
        assert!(
            check
                .entry(ResourceId {
                    type_id: 0x00B1_B104,
                    group,
                    instance: NEW_INSTANCE
                })
                .is_some()
        );
    }
    println!(
        "R1 NH-clone: {} entries -> {}（部署后：公园面板找 SC5 NIGHT HOUSE 同款条目，模型应为我们的商业塔）",
        entries.len(),
        out.display()
    );
}

fn remap_value(
    value: &mut sc_properties::Value,
    from_instance: u32,
    to_instance: u32,
    model: &sc_properties::Key,
) {
    if let sc_properties::Value::Key(key) = value {
        if key.instance == from_instance {
            *key = model.clone();
        } else if key.instance == to_instance {
            // already ours
        } else if key.instance == from_instance.wrapping_add(1) {
            key.instance = to_instance.wrapping_add(1);
        }
    }
}

#[test]
#[ignore = "requires the local asset-build fixture package and the workbench sample LOTM"]
fn real_commercial_menu_park_builds_package() {
    let fixture =
        PathBuf::from(std::env::var("SC_ASSET_FIXTURE_ROOT").expect("set SC_ASSET_FIXTURE_ROOT"));
    let root = fixture.join("commercial-model");
    let mut schema: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("schema.json")).unwrap()).unwrap();
    schema["id"] = serde_json::Value::String("commercial-menu-park".into());
    // 菜单包模型强制 merged-single：105-mesh 分区形状实测令游戏加载崩溃。
    schema["scene"]["modelShape"] = serde_json::Value::String("merged-single".into());
    schema["semantics"] = serde_json::json!({ "menu": {
        "title": "OpenSCP 商业公园",
        "description": "OpenSCP 资产工作台菜单录入测试（公园面板）",
        "menuParentInstance": "0xF970A822",
        "categoryInstance": "0x07049250",
        "unitTemplateInstance": "0x565B72B9",
        "lotTemplateInstance": "0x34CE7E14",
        "uiPosition": 900,
        "constructionCost": 8000,
        "maintenanceCost": 120
    }});
    schema["outputs"] = serde_json::json!({ "package": "commercial-menu-park.package" });
    let started = std::time::Instant::now();
    let result = build_from_schema(&root, &serde_json::to_string(&schema).unwrap())
        .unwrap_or_else(|error| panic!("commercial menu park build failed: {error}"));
    println!(
        "commercial menu park: {result:?}
build took {:?}",
        started.elapsed()
    );
    assert!(result.delta_mode);
    assert!(
        result.resource_count < 500,
        "menu package must not carry the reference clone, got {} entries",
        result.resource_count
    );
    let x = result.menu_instance.expect("new building instance");
    let output = Package::open(&result.path).unwrap();
    // 六件身份资源 + locale + 图标 + 大图
    for (group, kind) in [
        (0x09878A01, "menu2"),
        (0x40E0C000, "unit leaf"),
        (0x40E1C000, "lot leaf"),
    ] {
        assert!(
            output
                .entry(ResourceId {
                    type_id: 0x00B1B104,
                    group,
                    instance: x
                })
                .is_some(),
            "{kind} missing for instance {x:08X}"
        );
    }
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x0A98EAF0,
                group: 0x02FABF01,
                instance: 0xBF82F7CE
            })
            .is_some()
    );
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x2F7D0004,
                group: 0x40E02400,
                instance: x
            })
            .is_some()
    );
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x3F8662EA,
                group: 0,
                instance: x
            })
            .is_some()
    );
    // 参照 lot（同 TGI 覆盖）不得出现在菜单模式产物中
    assert!(
        output
            .entry(ResourceId {
                type_id: 0x00B1B104,
                group: 0x41F0C000,
                instance: 0x0483239E
            })
            .is_none(),
        "menu mode must not override the reference lot"
    );
    // 新 lot 叶子继承场景语义：作者单元写回、LOD1 指向编译模型
    let lot = PropertyFile::parse(
        &output
            .read(
                output
                    .entry(ResourceId {
                        type_id: 0x00B1B104,
                        group: 0x40E1C000,
                        instance: x,
                    })
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    // 单元列全清（NightHouse 式干净 lot）：直挂模型 id 不在建筑脚本资源
    // 表，原样携带会令加载存档崩溃（2026-10-31 实测）。
    let assembled = LotEditorDocument::from_property_file(lot.clone()).assemble_units();
    assert!(
        assembled.units.is_empty(),
        "menu lot leaf must carry no unit columns, got {}",
        assembled.units.len()
    );
    let lod1 = lot
        .get(sc_properties::LOD1_MODEL_HASH)
        .and_then(|p| p.scalar().cloned());
    let Some(Value::Key(model_key)) = lod1 else {
        panic!("new lot leaf LOD1 retargeted to the compiled model");
    };
    let model = output
        .entry(ResourceId {
            type_id: model_key.type_id,
            group: model_key.group,
            instance: model_key.instance,
        })
        .expect("compiled model in menu package");
    let bytes = output.read(model).unwrap();
    let file = rw4::Rw4File::parse(&bytes).unwrap();
    let meshes = file.sections_of_type(rw4::SectionType::MESH).count();
    println!("menu merged model: {meshes} meshes");
    assert_eq!(
        meshes, 2,
        "merged-single: chunk split at u16 cap only (105-mesh partitioned shape crashes the game)"
    );
    assert_eq!(file.sections_of_type(rw4::SectionType::BBOX).count(), 1);
    assert_eq!(file.sections_of_type(rw4::SectionType::TEXTURE).count(), 1);
    drop(output);
}
