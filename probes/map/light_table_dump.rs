//! 灯光表列结构转储：确认 `0x0CAA8F10` 家族各列的类型与步长。
//!
//! 用法：
//! `cargo run -p probes --release --example light_table_dump -- <package> <group> <instance>`
//!
//! 依据 `SC::FillLightInfoMap`（RVA 0x7D24D0）：类型列 `0x0CAA8F10` 的值哈希决定
//! 创建 Line/Spot/Point 灯；颜色、半径、内半径、Kd、Ks、裁剪距离、变换等列由
//! `SP::GetPropertiesAsTable` 的列描述给出。

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("package path");
    let group = u32::from_str_radix(args.next().expect("group").trim_start_matches("0x"), 16)
        .expect("group hex");
    let instance = u32::from_str_radix(args.next().expect("instance").trim_start_matches("0x"), 16)
        .expect("instance hex");
    let pkg = dbpf::Package::open(&path).expect("open package");
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.group == group && e.id.instance == instance)
        .expect("missing entry");
    println!(
        "TGI {:08X}:{:08X}:{:08X} compressed={}",
        entry.id.type_id, entry.id.group, entry.id.instance, entry.compressed_size
    );
    let props = sc_properties::PropertyFile::parse(&pkg.read(entry).unwrap()).expect("parse");
    let mut rows: Vec<_> = props.values.iter().collect();
    rows.sort_by_key(|p| p.hash);
    for prop in rows {
        let kind = match &prop.kind {
            sc_properties::Kind::Scalar(sc_properties::Value::Float(v)) => format!("f32 = {v}"),
            sc_properties::Kind::Scalar(v) => format!("scalar = {v}"),
            sc_properties::Kind::Array(values) => {
                let head: Vec<String> = values.iter().take(6).map(|v| v.to_string()).collect();
                format!(
                    "array[{}] item_size={:?} head=[{}]",
                    values.len(),
                    prop.encoding.array_item_size,
                    head.join(", ")
                )
            }
            sc_properties::Kind::Empty => "empty".into(),
        };
        println!(
            "0x{:08X}  {:<12} flags=0x{:04X}  {kind}",
            prop.hash,
            prop.prop_type.name(),
            prop.encoding.flags
        );
    }
}
