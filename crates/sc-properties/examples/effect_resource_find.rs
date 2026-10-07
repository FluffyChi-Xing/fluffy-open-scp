//! 手动取证（effect Swarm 预览）：按 instance 反查 effect 资源实体——
//! 类型/所在包/大小/格式嗅探（前 256 字节）。只读。
//! cargo run -p sc-properties --release --example effect_resource_find -- <pkg> [...]
use dbpf::Package;

const TARGETS: [u32; 8] = [
    0xC970_1842, 0xA617_CD2A, 0xF038_64CC, 0x309C_FE3F,
    0x59A5_69A8, 0x59A5_69AF, 0x660B_C390, 0xB97A_D80A,
];

fn sniff(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(96)];
    let printable = head
        .iter()
        .filter(|b| (32..=127).contains(*b) || matches!(b, b'\n' | b'\r' | b'\t'))
        .count();
    if printable * 4 >= head.len() * 3 {
        format!("TEXT: {}", String::from_utf8_lossy(head).replace('\n', " ⏎ "))
    } else {
        format!("BIN: {:02X?}", &head[..head.len().min(24)])
    }
}

fn main() {
    let mut found = 0usize;
    // 支持目录参数：递归扫目录内全部 .package（effect 资源可能不在常用五包）
    let mut paths: Vec<String> = Vec::new();
    for arg in std::env::args().skip(1) {
        if std::fs::metadata(&arg)
            .map(|m| m.is_dir())
            .unwrap_or(false)
        {
            let mut stack = vec![arg.clone()];
            while let Some(dir) = stack.pop() {
                let Ok(read_dir) = std::fs::read_dir(&dir) else { continue };
                for child in read_dir.flatten() {
                    let child_path = child.path();
                    if child_path.is_dir() {
                        stack.push(child_path.to_string_lossy().into_owned());
                    } else if child_path
                        .extension()
                        .map(|e| e.eq_ignore_ascii_case("package"))
                        .unwrap_or(false)
                    {
                        paths.push(child_path.to_string_lossy().into_owned());
                    }
                }
            }
        } else {
            paths.push(arg);
        }
    }
    println!("scanning {} packages", paths.len());
    for path in &paths {
        let Ok(package) = Package::open(&path) else { continue };
        let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
        for entry in package.entries() {
            if TARGETS.contains(&entry.id.instance) {
                let Ok(data) = package.read(entry) else { continue };
                println!(
                    "{name} T {:08X} G {:08X} I {:08X} len={}",
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    data.len()
                );
                println!("  {}", sniff(&data));
                found += 1;
                if found >= 16 {
                    println!("(截断，>=16)");
                    return;
                }
            }
        }
    }
    println!("found {found}");
}
