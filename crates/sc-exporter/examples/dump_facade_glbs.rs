//! 手动取证（GLB Worker 贴图乱码，2026-10-08）：扫描包内带 facade 世界投影
//! UV（FLOAT4 TexCoord → 导出 TEXCOORD_2/3，EP1 建筑特征）的网格，用与
//! package_service 载荷同款调用（export_glb_with_colors）落盘 GLB，供前端
//! roundtrip 测试对比 worker 提取/主线程 GLTFLoader 原始属性。
//! cargo run -p sc-exporter --release --example dump_facade_glbs -- <out_dir> <pkg> [...]
use dbpf::Package;

const MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (out_dir, paths) = args.split_at(1);
    let out_dir = &out_dir[0];
    std::fs::create_dir_all(out_dir).expect("mkdir");
    let mut dumped = 0usize;
    let mut scanned = 0usize;
    'outer: for path in paths {
        let Ok(package) = Package::open(path) else { continue };
        let entries: Vec<_> = package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == MODEL_TYPE)
            .cloned()
            .collect();
        for entry in entries {
            let Ok(data) = package.read(&entry) else { continue };
            let Ok(file) = rw4::Rw4File::parse(&data) else { continue };
            for section in file.sections_of_type(rw4::SectionType::MESH) {
                let Ok(mesh) = file.decode_mesh(&data, section.number) else {
                    continue;
                };
                if !mesh.is_exportable() {
                    continue;
                }
                scanned += 1;
                let has_facade = mesh.vertices.iter().any(|v| {
                    v.components.iter().any(|(e, val)| {
                        e.usage == rw4::DeclarationUsage::TexCoord
                            && matches!(val, rw4::ComponentValue::Float4(_))
                    })
                });
                if !has_facade {
                    continue;
                }
                let mat_indices: Vec<f32> = mesh
                    .vertices
                    .iter()
                    .map(|v| f32::from(v.d3d_color_g().unwrap_or(0)))
                    .collect();
                let out = sc_exporter::export_glb_with_colors(
                    &mesh,
                    None,
                    &[],
                    sc_exporter::EmbeddedTextures::default(),
                    None,
                    Some(&mat_indices),
                );
                let path = format!(
                    "{out_dir}/facade_{:08X}_{}.glb",
                    entry.id.instance, section.number
                );
                std::fs::write(&path, &out.bytes).expect("write");
                println!(
                    "facade mesh {inst:08X} #{sec}: verts={verts} -> {path}",
                    inst = entry.id.instance,
                    sec = section.number,
                    verts = mesh.vertices.len(),
                );
                dumped += 1;
                if dumped >= 6 {
                    break 'outer;
                }
            }
        }
    }
    println!("dumped {dumped} facade glbs; scanned {scanned} meshes");
}
