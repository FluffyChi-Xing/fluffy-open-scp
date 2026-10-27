//! 探针：对指定 property 资源做 解析→encode_canonical→逐字节对比，
//! 验证编码往返是否无损。仅开发用。
//!
//! 用法：cargo run -p probes --release --example prop_roundtrip --
//!   <package> <instance_hex> [group_hex]
use dbpf::Package;
use sc_properties::PropertyFile;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let group = args
        .get(2)
        .map(|g| u32::from_str_radix(g.trim_start_matches("0x"), 16).unwrap())
        .unwrap_or(0x0987_8A01);
    let inst = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).unwrap();
    let p = Package::open(&args[0]).unwrap();
    let e = p
        .entries()
        .into_iter()
        .find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == inst && e.id.group == group)
        .expect("entry");
    let raw = p.read(&e).unwrap();
    let file = PropertyFile::parse(&raw).expect("parse");
    let re = file.encode_canonical().expect("encode");
    if re == raw {
        println!("{inst:08X}: 往返逐字节一致 ({}B)", raw.len());
    } else {
        let n = raw.len().max(re.len());
        let mut first = None;
        for i in 0..n {
            if raw.get(i) != re.get(i) {
                first = Some(i);
                break;
            }
        }
        println!(
            "{inst:08X}: 不一致 raw={}B re={}B 首个差异 off {:#x}",
            raw.len(),
            re.len(),
            first.unwrap_or(usize::MAX)
        );
    }
}
