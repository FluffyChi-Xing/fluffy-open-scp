//! 探针：按 instance dump 一个 property 资源（0x00B1B104）的全部字段，
//! 用于比对菜单/工具条目的标题、描述、分类等引用值。仅开发用。
//!
//! 用法：cargo run -p sc-properties --release --example tool_info -- <package> <instance>...

use sc_properties::{Kind, PropertyFile, Value};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("usage: tool_info <package> <instance>...");
    let wants: Vec<u32> = args[1..]
        .iter()
        .map(|a| u32::from_str_radix(a.trim_start_matches("0x"), 16).unwrap())
        .collect();
    let package = dbpf::Package::open(path).expect("open package");
    for want in wants {
        let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.instance == want && e.id.type_id == 0x00B1_B104)
        else {
            println!("0x{want:08X}: 未找到");
            continue;
        };
        let data = package.read(entry).expect("read");
        let Ok(file) = PropertyFile::parse(&data) else {
            println!("0x{want:08X}: 解析失败");
            continue;
        };
        println!(
            "===== 0x{want:08X}  group=0x{:08X}  {} 条属性  off=0x{:X} csize={} dsize={} {}",
            entry.id.group,
            file.values.len(),
            entry.offset,
            entry.compressed_size,
            entry.decompressed_size,
            if entry.compressed { "REFPOMP" } else { "stored" }
        );
        let mut props: Vec<_> = file.values.iter().collect();
        props.sort_by_key(|p| p.hash);
        for p in props {
            let body = match &p.kind {
                Kind::Scalar(v) => render(v),
                Kind::Array(vs) => format!("[{}]", vs.iter().map(render).collect::<Vec<_>>().join(", ")),
                Kind::Empty => "<empty>".to_string(),
            };
            println!("  0x{:08X}  {body}", p.hash);
        }
    }
}

fn render(v: &Value) -> String {
    match v {
        Value::Text(t) => format!("Text #{:08X}:{:08X}", t.table_id, t.instance_id),
        other => other.to_string(),
    }
}
