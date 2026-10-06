//! 冒烟工具：sim 部件模型的材质段结构（Decoded/Raw、纹理引用、文件纹理清单），
//! 判定 LOTM 构建会走哪条 slot 提取路径。
//! 用法：cargo run -p sc-exporter --release --example simm_material_probe -- <instance_hex> <pkg> [...]
use dbpf::Package;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    'outer: for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.instance == instance && e.id.type_id == 0x2F4E_681B)
            .cloned()
        else {
            continue;
        };
        let data = package.read(&entry).expect("read");
        let file = rw4::Rw4File::parse(&data).expect("parse");
        println!("== I {instance:08X} @ {path} ({}B)", data.len());
        for mat in file.sections_of_type(rw4::SectionType::MATERIAL) {
            match file.decode_material(&data, mat.number) {
                Ok(rw4::MaterialSection::Decoded(m)) => {
                    println!("  MATERIAL #{} Decoded: {} texrefs", mat.number, m.texture_refs.len());
                    for r in &m.texture_refs {
                        println!("    slot {:#04x} -> {:#010x}", r.slot_byte(), r.texture_instance);
                    }
                }
                Ok(rw4::MaterialSection::Raw(m)) => {
                    println!("  MATERIAL #{} Raw ({}B)", mat.number, m.len());
                }
                Err(e) => println!("  MATERIAL #{} err {e}", mat.number),
            }
        }
        for (i, tex) in file.sections_of_type(rw4::SectionType::TEXTURE).enumerate() {
            match file.decode_texture(&data, tex.number) {
                Ok(t) => println!(
                    "  TEXTURE #{} {}x{} mips",
                    i, t.width, t.height
                ),
                Err(e) => println!("  TEXTURE #{} decode err {e}", i),
            }
        }
        break 'outer;
    }
}
