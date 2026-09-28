//! 冒烟工具：导出模型 RW4 的全部可导出 mesh 为 GLB。
//! 用法：cargo run -p sc-exporter --release --example dump_model_glb -- <package> <model_instance_hex> <out_dir>
use dbpf::Package;

const MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("hex");
    let out_dir = &args[2];
    std::fs::create_dir_all(out_dir).expect("mkdir");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && e.id.type_id == MODEL_TYPE)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read");
    let file = rw4::Rw4File::parse(&data).expect("parse");
    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else { continue };
        if !mesh.is_exportable() { continue; }
        let out = sc_exporter::export_glb(&mesh, None, &[]);
        let path = format!("{out_dir}/mesh_{}.glb", section.number);
        std::fs::write(&path, &out.bytes).expect("write");
        println!("mesh #{} verts={} → {}", section.number, mesh.vertices.len(), path);
    }
}
