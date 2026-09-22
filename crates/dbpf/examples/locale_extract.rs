//! 提取 locale 资源中包含指定子串的条目（hash key → 文本）。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let needle = args.next().unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    for e in p.entries() {
        let Ok(data) = p.read(e) else { continue };
        let text = String::from_utf8_lossy(&data);
        if text.contains(&needle) {
            println!("== {}:{:08X}:{:08X} ({}B) ==", e.id.type_id, e.id.group, e.id.instance, data.len());
            // 逐条 "0xXXXXXXXX" : "..." 扫描
            let bytes = text.as_bytes();
            let mut i = 0usize;
            let mut found = 0;
            while i < bytes.len() && found < 40 {
                if bytes[i] == b'0' && i + 10 < bytes.len() && &text[i..i+2] == "0x" {
                    if let Some(q1) = text[i..].find('"') {
                        let key_end = i + q1;
                        if text[key_end..].starts_with("\" : \"") || text[key_end..].starts_with("\":\"") {
                            let vs = key_end + if text[key_end..].starts_with("\" : \"") { 5 } else { 3 };
                            if let Some(q2) = text[vs..].find('"') {
                                let val = &text[vs..vs + q2];
                                if val.contains(&needle) {
                                    println!("  {} => {}", &text[i..key_end], val);
                                    found += 1;
                                }
                                i = vs + q2;
                                continue;
                            }
                        }
                    }
                }
                i += 1;
            }
        }
    }
}
