//! 隔离梯 R4：R3 的 tiny 模型，Material section 换成 NH house 的真实
//! CompiledState（441B 原文，纹理槽引用 NH 包内资源）。
//! 用法：cargo run -p rw4 --release --example nh_mat_swap -- <nh_pkg> <tiny_pkg> <out_pkg>
use dbpf::{OverlayEntry, Package, ResourceId, write_uncompressed_overlay_to_path};
use rw4::{Rw4File, SectionType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (nh_path, tiny_path, out_path) = (&args[0], &args[1], &args[2]);
    let nh = Package::open(nh_path)?;
    let tiny = Package::open(tiny_path)?;

    // NH house 的材质 payload
    let nh_model_entry = nh
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C)
        .expect("NH model");
    let nh_model = nh.read(nh_model_entry).unwrap();
    let nh_file = Rw4File::parse(&nh_model).unwrap();
    let mat_section = nh_file
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .expect("NH material");
    let nh_material = nh_file.payload(&nh_model, mat_section.number)?.to_vec();
    println!("NH material payload: {} bytes", nh_material.len());

    // tiny 模型：替换 MATERIAL section payload，其余原样
    // 精确选中 tiny 模型（R3 输出里还混着 NH 的原版模型）
    let tiny_entry = tiny
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0xCEC3_ACAA)
        .expect("tiny model (CEC3ACAA)");
    let tiny_model = tiny.read(tiny_entry).unwrap();
    let tiny_file = Rw4File::parse(&tiny_model).unwrap();
    let mat_section = tiny_file
        .sections_of_type(SectionType::MATERIAL)
        .next()
        .expect("tiny material");
    let mut patched = tiny_model.clone();
    let pos = mat_section.pos as usize;
    let size = mat_section.size as usize;
    patched[pos..pos + size].copy_from_slice(&nh_material[..size.min(nh_material.len())]);
    if nh_material.len() < size {
        // NH 材质更短：截断 section（改 index size 字段——这里直接重建）
        panic!("NH material smaller than tiny material section — need rebuild");
    }

    let mut entries: Vec<OverlayEntry> = Vec::new();
    for e in tiny.entries() {
        let payload = if e.id.type_id == 0x2F4E_681B {
            patched.clone()
        } else {
            tiny.read(e).unwrap()
        };
        entries.push(OverlayEntry::new(e.id, payload));
    }
    write_uncompressed_overlay_to_path(out_path, &entries)?;
    println!("R4 written {} entries -> {}", entries.len(), out_path);
    Ok(())
}
