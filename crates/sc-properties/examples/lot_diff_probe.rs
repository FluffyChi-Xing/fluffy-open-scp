//! 对比两个 lot property 的全部属性。仅开发用。
fn main() {
    let path = std::env::args().nth(1).expect("usage: lot_diff_probe <package> <instA> <instB>");
    let a = u32::from_str_radix(std::env::args().nth(2).unwrap().trim_start_matches("0x"), 16).unwrap();
    let b = u32::from_str_radix(std::env::args().nth(3).unwrap().trim_start_matches("0x"), 16).unwrap();
    let package = dbpf::Package::open(&path).expect("open package");
    for inst in [a, b] {
        let entry = package.entries().iter()
            .find(|e| e.id.type_id == 0x00B1_B104 && e.id.instance == inst)
            .expect("property not found").clone();
        let data = package.read(&entry).unwrap();
        let file = sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default()).unwrap();
        println!("=== lot 0x{inst:08X} group=0x{:08X} props={}", entry.id.group, file.values.len());
        for p in &file.values {
            let brief = match &p.kind {
                sc_properties::Kind::Scalar(v) => format!("{v:?}").chars().take(60).collect::<String>(),
                sc_properties::Kind::Array(vs) => format!("[{} items] e0={:?}", vs.len(), vs.first().map(|v| format!("{v:?}").chars().take(40).collect::<String>())),
                sc_properties::Kind::Empty => "empty".into(),
            };
            println!("  0x{:08X} {:?} {}", p.hash, p.prop_type, brief);
        }
    }
}
