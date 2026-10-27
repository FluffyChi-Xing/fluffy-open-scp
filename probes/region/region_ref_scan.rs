//! 全包扫描引用指定区域 TGI 的属性（Key 的 type/group/instance 任一匹配）。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let region = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    for e in p.entries() {
        if e.id.type_id != 0x00B1_B104 { continue; }
        let Ok(data) = p.read(e) else { continue };
        let Ok(pf) = sc_properties::PropertyFile::parse(&data) else { continue };
        let mut hits: Vec<String> = Vec::new();
        for prop in &pf.values {
            let check_key = |k: &sc_properties::Key| {
                k.group == region || k.instance == region
            };
            match &prop.kind {
                sc_properties::Kind::Scalar(sc_properties::Value::Key(k)) => if check_key(k) { hits.push(format!("{:08X}", prop.hash)); },
                sc_properties::Kind::Array(vs) => {
                    for v in vs {
                        if let sc_properties::Value::Key(k) = v {
                            if check_key(k) { hits.push(format!("{:08X}", prop.hash)); break; }
                        }
                    }
                }
                _ => {}
            }
        }
        if !hits.is_empty() {
            println!("{:08X}:{:08X}:{:08X} ({}B) keys {:?}", e.id.type_id, e.id.group, e.id.instance, data.len(), hits);
        }
    }
}
