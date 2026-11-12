//! 直读 DBPF 包内指定 TGI 的资源：解压 + 按 PropertyFile 逐列打印（失败则打印原始头）。
//! 用法：`dump_tgi <package> <type_hex> <group_hex> <instance_hex>`
use dbpf::ResourceId;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (path, t, g, i) = (
        args[1].clone(),
        u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap(),
        u32::from_str_radix(args[3].trim_start_matches("0x"), 16).unwrap(),
        u32::from_str_radix(args[4].trim_start_matches("0x"), 16).unwrap(),
    );
    let package = dbpf::Package::open(&path).expect("open package");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id == ResourceId { type_id: t, group: g, instance: i })
        .unwrap_or_else(|| panic!("TGI {t:08X}:{g:08X}:{i:08X} 不在包内"));
    let data = package.read(entry).expect("read entry");
    println!("// {} bytes", data.len());
    match sc_properties::PropertyFile::parse(&data) {
        Ok(file) => {
            for property in &file.values {
                println!(
                    "{:08X}\t{:?}\t{:?}",
                    property.hash, property.prop_type, property.kind
                );
            }
        }
        Err(error) => {
            println!("// 非属性资源: {error:?}");
            println!("// 头 64 字节: {}", hex(&data[..data.len().min(64)]));
        }
    }
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}
