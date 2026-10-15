//! 手动取证（effect Swarm 预览）：扫描包内 Swarm 效果集合资源
//! （type 0xEA5118B0），打印 T/G/I/大小 + 格式嗅探。只读。
//! cargo run -p sc-properties --release --example effect_collections -- <pkg> [...]
use dbpf::Package;

fn sniff(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(160)];
    let printable = head
        .iter()
        .filter(|b| (32..=127).contains(*b) || matches!(b, b'\n' | b'\r' | b'\t'))
        .count();
    if printable * 4 >= head.len() * 3 {
        format!("TEXT: {}", String::from_utf8_lossy(head).replace('\n', " ⏎ "))
    } else {
        format!("BIN: {:02X?}", &head[..head.len().min(32)])
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(package) = Package::open(&path) else { continue };
        let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
        for entry in package.entries() {
            if entry.id.type_id != 0xEA51_18B0 {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            println!(
                "{name} G {:08X} I {:08X} len={}",
                entry.id.group,
                entry.id.instance,
                data.len()
            );
            println!("  {}", sniff(&data));
        }
    }
}
