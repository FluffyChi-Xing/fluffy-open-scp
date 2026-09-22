//! 导出 locale 资源全部条目。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let inst = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    for e in p.entries() {
        if e.id.instance == inst {
            let data = p.read(e).unwrap();
            let text = String::from_utf8_lossy(&data);
            let out = format!("tmp/locale_{:08X}.txt", e.id.instance);
            std::fs::write(&out, text.as_bytes()).unwrap();
            println!("{} ({}B) -> {out}", e.id.type_id, data.len());
        }
    }
}
