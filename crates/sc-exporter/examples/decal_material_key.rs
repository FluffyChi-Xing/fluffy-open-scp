//! 冒烟工具：列出 decal 字典的 material Key（T/G/I）。
//! 用法：cargo run -p sc-exporter --release --example decal_material_key -- <atlas_instance_hex> <pkg> [...]
use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let atlas = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    for path in &args[1..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter().filter(|e| e.id.type_id == PROPERTY_TYPE) {
            if e.id.instance != atlas { continue; }
            let data = package.read(e).unwrap_or_default();
            let Ok(dict) = sc_properties::DecalDictionary::parse(&data) else { continue };
            let m = dict.material.map(|k| format!("T {:08X} G {:08X} I {:08X}", k.type_id, k.group, k.instance));
            println!("{path}: atlas G {:08X} I {atlas:08X} material = {}", e.id.group, m.unwrap_or_else(|| "-".into()));
        }
    }
}
