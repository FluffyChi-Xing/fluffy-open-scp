//! 冒烟探针：扫描「可贴图 + 带调色板材质」的建筑模型清单（阶段 3.1 测试样本）。
//!
//! 条件：mesh 可解码且 has_uv（FLOAT2 或 FLOAT4≤8）、材质可解且带
//! slot0 调色板 + slot1 遮罩、三角数 ≥ 下限。输出 instance/三角数/资产名。
//!
//! 用法：cargo run -p rw4 --release --example uv_model_scan -- \
//!   <registry.s3db> <min_tris> <package>...

const RW4_IMAGE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let min_tris: u32 = args
        .first()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let mut found: Vec<(u32, String, u32, u32, u32)> = Vec::new(); // instance, pkg, tris, verts, masks
    for path in &args[1..] {
        let package = dbpf::Package::open(path).expect("open package");
        let package_name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        for entry in package.entries() {
            if entry.id.type_id != RW4_IMAGE {
                continue;
            }
            let entry = entry.clone();
            let Ok(data) = package.read(&entry) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
            let mut total_tris = 0u32;
            let mut total_verts = 0u32;
            let mut has_uv = false;
            let mut has_float2 = false;
            let mut has_d3dcolor = false;
            for section in file.sections_of_type(rw4::SectionType::MESH) {
                let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
                if !mesh.is_exportable() {
                    continue;
                }
                total_tris += mesh.triangles.len() as u32;
                total_verts += mesh.vertices.len() as u32;
                if mesh_has_uv(&mesh) {
                    has_uv = true;
                    if mesh.vertices.iter().any(|v| v.has_float2_uv()) {
                        has_float2 = true;
                    }
                }
                if mesh.vertices.iter().any(|v| v.d3d_color_g().is_some()) {
                    has_d3dcolor = true;
                }
            }
            if !has_uv || total_tris < min_tris {
                continue;
            }
            let mut masks = 0u32;
            for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
                let Ok(rw4::MaterialSection::Decoded(mat)) =
                    file.decode_material(&data, section.number)
                else {
                    continue;
                };
                if mat.slot_texture(0).is_some() && mat.slot_texture(1).is_some() {
                    masks += 1;
                }
            }
            if masks == 0 {
                continue;
            }
            let d3d = if has_d3dcolor { "+G" } else { "-G" };
            let uv_type = if has_float2 {
                format!("float2{d3d}")
            } else {
                format!("f4small{d3d}")
            };
            found.push((
                entry.id.instance,
                format!("{package_name} {uv_type}"),
                total_tris,
                total_verts,
                masks,
            ));
        }
    }
    found.sort_by(|a, b| b.2.cmp(&a.2));
    println!("candidates: {} (min_tris={min_tris})", found.len());
    for (instance, package, tris, verts, masks) in found.iter().take(15) {
        println!(
            "0x{instance:08X}  {package}  {tris} tris / {verts} verts  {masks} mask-materials"
        );
    }
}

fn mesh_has_uv(mesh: &rw4::DecodedMesh) -> bool {
    let mut has_float2 = false;
    let mut has_float4 = false;
    let mut float4_max_xy = 0f32;
    for vertex in &mesh.vertices {
        for (element, value) in &vertex.components {
            if element.usage != rw4::DeclarationUsage::TexCoord {
                continue;
            }
            match value {
                rw4::ComponentValue::Float2(_) => has_float2 = true,
                rw4::ComponentValue::Float4(f) => {
                    has_float4 = true;
                    float4_max_xy = float4_max_xy.max(f[0].abs()).max(f[1].abs());
                }
                _ => {}
            }
        }
    }
    has_float2 || (has_float4 && float4_max_xy <= 8.0)
}
