//! 离线对拍：NH house 模型 → 解码顶点 → 用 NH 布局重编码顶点 blob →
//! 与原版 blob 逐字节比对。完全离线，无游戏参与。
//! 用法：cargo run -p rw4 --release --example nh_vertex_roundtrip -- <nh_pkg>
use dbpf::Package;
use rw4::{Rw4File, SectionType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C)
        .expect("NH LOD1 model");
    let model = pkg.read(entry)?;
    let file = Rw4File::parse(&model).unwrap();

    // 1) 原始 blob：VA 的数据引用
    let va_sec = file
        .sections_of_type(SectionType::VERTEX_ARRAY)
        .next()
        .unwrap();
    let va = file.payload(&model, va_sec.number).unwrap();
    let va_words: Vec<u32> = va
        .chunks(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    println!("VA payload: {:?}", va_words);
    let blob_id = va_words[6];
    let blob_sec = file
        .sections()
        .iter()
        .find(|s| s.number == blob_id)
        .expect("vertex blob");
    let original_blob = file.payload(&model, blob_sec.number)?.to_vec();
    println!(
        "vertex blob: {} bytes (section {})",
        original_blob.len(),
        blob_sec.number
    );

    // 2) 解码
    let mesh_sec = file.sections_of_type(SectionType::MESH).next().unwrap();
    let mesh = file.decode_mesh(&model, mesh_sec.number).unwrap();
    println!(
        "decoded: {} verts, {} tris",
        mesh.vertices.len(),
        mesh.triangles.len()
    );

    // 3) 逐顶点打印解码分量（前 2 个顶点的完整分量）
    for (vi, v) in mesh.vertices.iter().enumerate().take(2) {
        println!("vertex[{vi}]:");
        for (element, value) in &v.components {
            println!(
                "  {:?}/{:?}[{}] = {:?}",
                element.usage, element.decl_type, element.index, value
            );
        }
    }

    // 4) 重编码（NH 布局 stride 40：pos F3 + normal U4 + tangent U4 + color 4 + uv F4）
    let mut reencoded = Vec::with_capacity(mesh.vertices.len() * 40);
    for v in &mesh.vertices {
        let pos = v.position().unwrap_or([0.0; 3]);
        reencoded.extend_from_slice(
            &pos.iter()
                .flat_map(|f| f.to_le_bytes())
                .collect::<Vec<u8>>(),
        );
        // 从解码分量提取原始 UBYTE4 字节（不做浮点往返，直接找 UBYTE4 分量）
        let mut wrote_normal = false;
        let mut wrote_tangent = false;
        let mut wrote_color = false;
        for (element, value) in &v.components {
            match (element.usage, element.decl_type) {
                (rw4::DeclarationUsage::Normal, rw4::DeclarationType::UByte4) => {
                    if let rw4::ComponentValue::UByte4(b) = value {
                        reencoded.extend_from_slice(b);
                        wrote_normal = true;
                    }
                }
                (rw4::DeclarationUsage::Tangent, rw4::DeclarationType::UByte4) => {
                    if let rw4::ComponentValue::UByte4(b) = value {
                        reencoded.extend_from_slice(b);
                        wrote_tangent = true;
                    }
                }
                (rw4::DeclarationUsage::Color, rw4::DeclarationType::D3DColor) => {
                    if let rw4::ComponentValue::D3DColor { a, r, g, b } = value {
                        // 内存序对齐原版：首字节 = b（seed 通道）
                        reencoded.extend_from_slice(&[*a, *r, *g, *b]);
                        wrote_color = true;
                    }
                }
                _ => {}
            }
        }
        // NH 的 TEXCOORD 是 FLOAT4（facade base.xy+top.zw）
        for (element, value) in &v.components {
            if element.usage == rw4::DeclarationUsage::TexCoord {
                if let rw4::ComponentValue::Float4(f4) = value {
                    reencoded.extend_from_slice(
                        &f4.iter().flat_map(|f| f.to_le_bytes()).collect::<Vec<u8>>(),
                    );
                }
                break;
            }
        }
        let _ = (wrote_normal, wrote_tangent, wrote_color);
    }

    // 5) 比对
    if reencoded.len() != original_blob.len() {
        println!(
            "LENGTH MISMATCH: re-encoded {} vs original {}",
            reencoded.len(),
            original_blob.len()
        );
    }
    let n = reencoded.len().min(original_blob.len());
    let mut diffs = 0usize;
    let mut first = Vec::new();
    for i in 0..n {
        if reencoded[i] != original_blob[i] {
            diffs += 1;
            if first.len() < 3 {
                let s = i.saturating_sub(8);
                let e = (i + 8).min(n);
                first.push(format!(
                    "@{i}: orig={:02x?} re={:02x?}",
                    &original_blob[s..e],
                    &reencoded[s..e]
                ));
            }
        }
    }
    println!("differing bytes: {diffs}/{n}");
    for f in &first {
        println!("  {f}");
    }
    std::fs::write("tmp/nh_vertex_blob_reencoded.bin", &reencoded)?;
    std::fs::write("tmp/nh_vertex_blob_original.bin", &original_blob)?;
    println!("blobs written to tmp/ for full diff");
    Ok(())
}
