//! uv_kind 放宽风险普查（一次性诊断，不长期保留）：
//! 对包内全部原生模型统计 mesh_uv_kind 现行推导 vs 放宽推导
//! （all-FLOAT4 且带 D3DCOLOR → 视为 facade）的翻转量。

use rw4::{Rw4File, SectionType};
use std::sync::Arc;

fn relaxed_kind(mesh: &rw4::DecodedMesh) -> u8 {
    let mut kind = 0u8;
    for vertex in &mesh.vertices {
        for (element, value) in &vertex.components {
            if element.usage != rw4::DeclarationUsage::TexCoord {
                continue;
            }
            match value {
                rw4::ComponentValue::Float2(_) => kind = kind.max(1),
                rw4::ComponentValue::Float4(f) => {
                    if f[0].abs() <= 8.0 && f[1].abs() <= 8.0 {
                        kind = kind.max(1);
                    } else {
                        kind = kind.max(2);
                    }
                }
                _ => {}
            }
        }
    }
    let any_color = mesh.vertices.iter().any(|v| v.d3d_color_g().is_some());
    let has_facade = !mesh.vertices.is_empty()
        && mesh.vertices.iter().all(|v| facade_uv4(v).is_some())
        && (mesh
            .vertices
            .iter()
            .any(|v| facade_uv4(v).map(|f| f.iter().any(|x| x.abs() > 8.0)).unwrap_or(false))
            || any_color);
    if has_facade {
        return 2;
    }
    kind.min(1)
}

fn facade_uv4(vertex: &rw4::DecodedVertex) -> Option<[f32; 4]> {
    vertex
        .components
        .iter()
        .filter_map(|(element, value)| {
            if element.usage != rw4::DeclarationUsage::TexCoord {
                return None;
            }
            match value {
                rw4::ComponentValue::Float4(f) => {
                    let score = f.iter().fold(0.0_f32, |m, v| m.max(v.abs()));
                    Some((score, *f))
                }
                _ => None,
            }
        })
        .max_by(|(a, _), (b, _)| a.total_cmp(b))
        .map(|(_, f)| f)
}

#[test]
#[ignore = "census over a local game package; set SC_UV_CENSUS_PACKAGE"]
fn uv_kind_relaxation_census() {
    let path = std::path::PathBuf::from(
        std::env::var("SC_UV_CENSUS_PACKAGE").expect("set SC_UV_CENSUS_PACKAGE"),
    );
    let package = dbpf::Package::open(&path).unwrap();
    let mut current = [0usize; 3];
    let mut relaxed = [0usize; 3];
    let mut flips = 0usize;
    let mut mesh_total = 0usize;
    let manager = crate::package_service::PackageManager::new();
    let _ = manager;
    for entry in package.entries().to_vec() {
        if entry.id.type_id != 0x2F4E_681B {
            continue;
        }
        let Ok(bytes) = package.read(&entry) else { continue };
        let Ok(file) = Rw4File::parse(&bytes) else { continue };
        for section in file.sections_of_type(SectionType::MESH) {
            let Ok(decoded) = file.decode_mesh(&bytes, section.number) else {
                continue;
            };
            if !decoded.is_exportable() {
                continue;
            }
            mesh_total += 1;
            let a = crate::package_service::mesh_uv_kind(&decoded) as usize;
            let b = relaxed_kind(&decoded) as usize;
            current[a] += 1;
            relaxed[b] += 1;
            if a != b {
                flips += 1;
            }
        }
    }
    let _ = Arc::new(());
    println!(
        "meshes={mesh_total} current kind0/1/2={current:?} relaxed={relaxed:?} flips={flips}"
    );
}
