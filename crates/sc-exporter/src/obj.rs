//! Wavefront OBJ 导出 —— 对齐 C# `WaveFrontOBJConverter.Export`：
//! `v`/`vn`/`vt` 各占一段、`{F12}` 12 位定点（en-US）、`f` 1-based 索引、
//! 跳过 `i == j` 的退化面。
//!
//! UV 语义采用修正后的 `DecodedVertex::uv()`（FLOAT2 优先，FLOAT4 回退，
//! 与 C# 修正版 `TryGetUV` 一致），而非旧版仅认 FLOAT4 的行为。

use rw4::DecodedMesh;

/// 导出一个网格为 Wavefront OBJ 文本。
pub fn export_obj(mesh: &DecodedMesh) -> String {
    let mut out = String::with_capacity(mesh.vertices.len() * 96 + mesh.triangles.len() * 32);
    out.push_str("# SimCityPak Wavefront OBJ Exporter\n");
    out.push_str("# File Created: (OpenSCP)\n\n");

    for v in &mesh.vertices {
        let p = v.position().unwrap_or([0.0; 3]);
        out.push_str(&format!("v  {:.12} {:.12} {:.12}\r\n", p[0], p[1], p[2]));
    }
    out.push_str(&format!("# {} vertices\r\n\n", mesh.vertices.len()));

    for v in &mesh.vertices {
        let n = v.normal().unwrap_or([0.0; 3]);
        out.push_str(&format!("vn  {:.12} {:.12} {:.12}\r\n", n[0], n[1], n[2]));
    }
    out.push_str(&format!("# {} vertex normals\r\n\n", mesh.vertices.len()));

    for v in &mesh.vertices {
        let uv = v.uv().unwrap_or([0.0; 2]);
        out.push_str(&format!("vt  {:.12} {:.12}\r\n", uv[0], uv[1]));
    }
    out.push_str(&format!(
        "# {} texture coordinates\r\n\n",
        mesh.vertices.len()
    ));

    let mut faces = 0usize;
    for t in &mesh.triangles {
        // C# 仅跳过 i == j；其余原样输出
        if t[0] != t[1] {
            out.push_str(&format!("f  {} {} {}\r\n", t[0] + 1, t[1] + 1, t[2] + 1));
            faces += 1;
        }
    }
    out.push_str(&format!("# {} faces\r\n\n", faces));
    out
}
