//! 把单个 exemplar PropertyFile 全列打印（hash/type/kind），供包间对拍。
use sc_properties::PropertyFile;

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_unit <property-file>");
    let data = std::fs::read(&path).expect("read");
    let file = PropertyFile::parse(&data).expect("parse");
    for property in &file.values {
        println!("{:08X}\t{:?}\t{:?}", property.hash, property.prop_type, property.kind);
    }
}
