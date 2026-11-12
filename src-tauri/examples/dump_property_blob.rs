//! 把 DBPF 里取出的资源 blob（可能是 RefPack 压缩）解压后按 PropertyFile 逐列打印。
//!
//! 用法：`cargo run -p fluffy-open-scp --example dump_property_blob -- <blob>`
//!
//! 存在原因：EcoGame scripts 包里的 bundle（组 40E0C700）是压缩块，`dump_unit`
//! 只认未压缩的 PropertyFile。2026-11-11 用它对拍 HTzhiyao 的 bundle 声明。

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_property_blob <file>");
    let raw = std::fs::read(&path).expect("read");
    let data = match dbpf::refpack_stream_header(&raw) {
        Ok((size, _)) => dbpf::refpack_decompress(&raw, size as usize).expect("decompress"),
        Err(_) => raw,
    };
    let file = sc_properties::PropertyFile::parse(&data).expect("parse property file");
    for property in &file.values {
        println!(
            "{:08X}\t{:?}\t{:?}",
            property.hash, property.prop_type, property.kind
        );
    }
}
