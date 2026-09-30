//! 冒烟工具：decal 聚类 material 资源（RW4）的贴图槽清单（含 0x2D shader-def）。
//! 用法：cargo run -p sc-exporter --release --example decal_material_slots -- <material_instance_hex> <pkg> [...]
use dbpf::Package;

const RW4_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let material = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let packages: Vec<Package> = args
        .iter()
        .skip(1)
        .map(|p| Package::open(p).unwrap_or_else(|e| panic!("open {p}: {e}")))
        .collect();
    for package in &packages {
        let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.type_id == RW4_TYPE && e.id.instance == material)
            .cloned()
        else { continue };
        let data = package.read(&entry).expect("read");
        let file = rw4::Rw4File::parse(&data).expect("parse");
        println!("material {material:08X} in {}", args.iter().skip(1).find(|_| true).unwrap());
        for section in file.sections_of_type(rw4::SectionType::MATERIAL) {
            match file.decode_material(&data, section.number) {
                Ok(rw4::MaterialSection::Decoded(m)) => {
                    for r in &m.texture_refs {
                        println!(
                            "  slot {:#04x} -> instance {:#010x}",
                            r.slot, r.texture_instance
                        );
                    }
                }
                Ok(rw4::MaterialSection::Raw(_)) => println!("  (raw material section)"),
                Err(e) => println!("  decode err {e}"),
            }
        }
        return;
    }
    println!("material {material:08X} not found");
}
