//! 探针：打印一个包内全部 0x0A98EAF0 资源的完整 TGI 与内容头部。仅开发用。
use dbpf::Package;
use sc_properties::locale::LOCALE_RESOURCE_TYPE;

fn main() {
    let path = std::env::args().nth(1).expect("package path");
    let p = Package::open(&path).unwrap();
    for e in p.entries() {
        if e.id.type_id != LOCALE_RESOURCE_TYPE {
            continue;
        }
        let data = p.read(e).unwrap_or_default();
        let head: String = String::from_utf8_lossy(&data[..data.len().min(150)])
            .chars()
            .map(|c| {
                if c == '\n' || c == '\r' || c == '\t' {
                    ' '
                } else {
                    c
                }
            })
            .collect();
        println!(
            "{:08X}:{:08X}:{:08X} raw={}B head={head:?}",
            e.id.type_id,
            e.id.group,
            e.id.instance,
            data.len()
        );
    }
}
