//! 单模型快速盘点：mesh 数/顶点/三角/属性/材质节。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let inst = u32::from_str_radix(args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open");
    let entry = package.entries().iter().find(|e| e.id.instance == inst).expect("not found").clone();
    let data = package.read(&entry).expect("read");
    let file = rw4::Rw4File::parse(&data).expect("parse");
    let mut mats = 0;
    for s in file.sections_of_type(rw4::SectionType::MATERIAL) {
        mats += 1;
        if let Ok(rw4::MaterialSection::Decoded(m)) = file.decode_material(&data, s.number) {
            let slots: Vec<String> = (0..6).filter_map(|i| m.slot_texture(i).map(|t| format!("s{i}=0x{t:08X}"))).collect();
            println!("material #{}: {}", s.number, slots.join(" "));
        } else {
            println!("material #{}: raw", s.number);
        }
    }
    println!("materials={mats}");
    for s in file.sections_of_type(rw4::SectionType::TEXTURE) {
        if let Ok(tex) = file.decode_texture(&data, s.number) {
            println!("texture #{}: {}x{} fmt={:?}", s.number, tex.width, tex.height, tex.format());
        }
    }
    for s in file.sections_of_type(rw4::SectionType::MESH) {
        if let Ok(mesh) = file.decode_mesh(&data, s.number) {
            let attrs: Vec<String> = mesh.vertices.first().map(|v| v.components.iter().map(|(e, val)| {
                let k = match val {
                    rw4::ComponentValue::Float2(_) => "F2",
                    rw4::ComponentValue::Float3(_) => "F3",
                    rw4::ComponentValue::Float4(_) => "F4",
                    rw4::ComponentValue::UByte4(_) => "UB4",
                    rw4::ComponentValue::D3DColor { .. } => "CLR",
                    _ => "?",
                };
                format!("{:?}{}", e.usage, k)
            }).collect()).unwrap_or_default();
            println!("mesh #{}: {} verts {} tris [{}] exportable={}", s.number, mesh.vertices.len(), mesh.triangles.len(), attrs.join(","), mesh.is_exportable());
        }
    }
}
// texture sections appended below in main via second pass? keep simple: separate loop already? add:
