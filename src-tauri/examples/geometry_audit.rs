//! 几何完整性审计：对比 LOTM 源模型与被引用 RW4 模型的顶点/三角面总数。
//! 丢面 = 编译期几何丢失的直接证据（构建期闸门与离线验收共用）。
//! Usage: cargo run --release -p fluffy-open-scp --example geometry_audit -- \
//!   <model.lotm> <package> <rw4_instance_hex> [<rw4_instance_hex>...]
use rw4::{Rw4File, SectionType};

fn lotm_totals(path: &str) -> (usize, usize) {
    let bytes = std::fs::read(path).expect("read lotm");
    let doc = fluffy_open_scp_lib::asset_build_m1::lotm::parse(&bytes).expect("parse lotm");
    let mut vertices = 0usize;
    let mut triangles = 0usize;
    for glb in &doc.meshes {
        let prims = fluffy_open_scp_lib::asset_build_m1::native_model::parse_glb_primitives(glb)
            .expect("parse mesh");
        for prim in &prims {
            vertices += prim.positions.len();
            triangles += prim.indices.len() / 3;
        }
    }
    (vertices, triangles)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let lotm_path = args.next().expect("model.lotm");
    let package_path = args.next().expect("package");
    let instances: Vec<u32> = args
        .map(|hex| u32::from_str_radix(hex.trim_start_matches("0x"), 16).expect("hex"))
        .collect();
    let (src_vertices, src_triangles) = lotm_totals(&lotm_path);
    println!("LOTM source: vertices={src_vertices} triangles={src_triangles}");
    let package = dbpf::Package::open(std::path::Path::new(&package_path)).expect("open package");
    let mut model_vertices = 0usize;
    let mut model_triangles = 0usize;
    for instance in &instances {
        let id = dbpf::ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: *instance,
        };
        let entry = package
            .entry(id)
            .unwrap_or_else(|| panic!("model {instance:08X} not in package"));
        let bytes = package.read(entry).expect("read model");
        let file = Rw4File::parse(&bytes).expect("parse rw4");
        let mut vertices = 0usize;
        let mut triangles = 0usize;
        for section in file.sections_of_type(SectionType::MESH) {
            let mesh = file
                .decode_mesh(&bytes, section.number)
                .expect("decode mesh");
            vertices += mesh.vertices.len();
            triangles += mesh.triangles.len();
        }
        println!("  model {instance:08X}: vertices={vertices} triangles={triangles}");
        model_vertices += vertices;
        model_triangles += triangles;
    }
    println!("RW4 referenced: vertices={model_vertices} triangles={model_triangles}");
    let ok = src_vertices == model_vertices && src_triangles == model_triangles;
    println!(
        "verdict: {} (vertices {src_vertices} vs {model_vertices}, triangles {src_triangles} vs {model_triangles})",
        if ok { "COMPLETE" } else { "GEOMETRY LOST" }
    );
}
