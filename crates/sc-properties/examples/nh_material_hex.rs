//! dump NH 原版材质 525B 的结构（找内部引用与 slot 表）。
use dbpf::{Package, ResourceId};
use rw4::{Rw4File, SectionType};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let e = pkg
        .entry(ResourceId {
            type_id: 0x2F4E_681B,
            group: 0,
            instance: 0x06C5_470C,
        })
        .unwrap();
    let model = pkg.read(e)?;
    let f = Rw4File::parse(&model)?;
    let mat = f.sections_of_type(SectionType::MATERIAL).next().unwrap();
    let p = f.payload(&model, mat.number)?;
    println!("material payload: {} bytes", p.len());
    for (i, chunk) in p.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        println!("{:03x}  {:<47}  {}", i * 16, hex.join(" "), ascii);
    }
    // @377 处的 u32（R8 注释里的内部引用位置）
    for off in [373usize, 377, 381] {
        if off + 4 <= p.len() {
            println!(
                "@{off}: u32 = {}",
                u32::from_le_bytes(p[off..off + 4].try_into().unwrap())
            );
        }
    }
    Ok(())
}
