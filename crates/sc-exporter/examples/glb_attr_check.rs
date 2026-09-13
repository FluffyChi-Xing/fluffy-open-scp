//! 冒烟工具：导出一个模型的 GLB 并检查顶点属性（TANGENT 是否写入、NORMAL 是否非零）。
//! 用法：cargo run -p sc-exporter --release --example glb_attr_check -- <package> <mesh_instance>
use dbpf::Package;

const MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("hex");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && e.id.type_id == MODEL_TYPE)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read");
    let file = rw4::Rw4File::parse(&data).expect("parse");

    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else {
            continue;
        };
        if !mesh.is_exportable() {
            continue;
        }
        let out = sc_exporter::export_glb(&mesh, None, &[]);
        let (json, bin_len) = split_glb(&out.bytes).expect("glb container");
        let has_tangent = json.contains("\"TANGENT\"");
        let normals_nonzero = mesh
            .vertices
            .iter()
            .filter_map(|v| v.normal())
            .any(|n| n[0].abs() > 1e-3 || n[1].abs() > 1e-3 || n[2].abs() > 1e-3);
        let tangents_present = mesh.vertices.iter().filter(|v| v.tangent().is_some()).count();
        // 切线健全性：单位长度 + 与法线正交（资产自带的 TBN 约定）。
        let mut max_len_err = 0f32;
        let mut max_dot = 0f32;
        for v in &mesh.vertices {
            if let (Some(n), Some(t)) = (v.normal(), v.tangent()) {
                let len = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
                max_len_err = max_len_err.max((len - 1.0).abs());
                max_dot = max_dot.max((n[0] * t[0] + n[1] * t[1] + n[2] * t[2]).abs());
            }
        }
        // 结构性校验：每个 bufferView 必须落在 BIN chunk 内。
        let views = count_views(&json);
        let max_end = max_view_end(&json);
        let fits = max_end <= bin_len;
        println!(
            "mesh #{:<4} verts={:<7} TANGENT_attr={:<5} normals_nonzero={:<5} tangent_verts={}/{} |t|-1 max={:.4} |n·t| max={:.4} bufferViews={} bin={}B maxEnd={} fits={}",
            section.number,
            mesh.vertices.len(),
            has_tangent,
            normals_nonzero,
            tangents_present,
            mesh.vertices.len(),
            max_len_err,
            max_dot,
            views,
            bin_len,
            max_end,
            fits
        );
    }
}

/// 拆 GLB 容器 → (JSON chunk 文本, BIN chunk 字节数)。
fn split_glb(bytes: &[u8]) -> Option<(String, usize)> {
    if bytes.len() < 20 {
        return None;
    }
    let mut off = 12usize;
    let mut json = String::new();
    let mut bin = 0usize;
    while off + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[off..off + 4].try_into().ok()?) as usize;
        let kind = u32::from_le_bytes(bytes[off + 4..off + 8].try_into().ok()?);
        let data = bytes.get(off + 8..off + 8 + len)?;
        if kind == 0x4E4F_534A {
            json = String::from_utf8_lossy(data).into_owned();
        } else if kind == 0x004E_4942 {
            bin = len;
        }
        off += 8 + len;
    }
    Some((json, bin))
}

fn count_views(json: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("bufferViews")?.as_array().map(Vec::len))
        .unwrap_or(0)
}

/// 所有 bufferView 的 byteOffset + byteLength 的最大值（必须 ≤ BIN 长度）。
fn max_view_end(json: &str) -> usize {
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("bufferViews")?.as_array().cloned())
        .map(|views| {
            views
                .iter()
                .map(|w| {
                    w.get("byteOffset").and_then(|o| o.as_u64()).unwrap_or(0) as usize
                        + w.get("byteLength").and_then(|l| l.as_u64()).unwrap_or(0) as usize
                })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}
