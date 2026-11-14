//! dump 指定模型的 VF 元素表 + VA/TA 头 + mesh 头。
use dbpf::Package;
use rw4::{Rw4File, SectionType};
fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let entry = pkg
        .entries()
        .iter()
        .find(|e| {
            e.id.type_id == 0x2F4E_681B
                && e.id.instance
                    == u32::from_str_radix(&args[1].trim_start_matches("0x"), 16).unwrap()
        })
        .expect("model entry");
    let model = pkg.read(entry)?;
    let file = Rw4File::parse(&model).unwrap();
    println!("model {} bytes", model.len());
    for s in file.sections() {
        println!(
            "  sec#{:>3} type={:#x} size={:>7} align={:>3}",
            s.number, s.type_code, s.size, s.alignment
        );
    }
    if let Some(vf) = file.sections_of_type(SectionType::VERTEX_FORMAT).next() {
        let payload = file.payload(&model, vf.number)?;
        let count = u16::from_le_bytes(payload[12..14].try_into().unwrap()) as usize;
        let stride = u16::from_be_bytes(payload[14..16].try_into().unwrap());
        println!("VF: elements={count} stride={stride}");
        for k in 0..count {
            let e = &payload[24 + k * 12..24 + (k + 1) * 12];
            let stream = u16::from_le_bytes(e[0..2].try_into().unwrap());
            let offset = u16::from_le_bytes(e[2..4].try_into().unwrap());
            println!(
                "  [{k}] stream={stream} offset={offset:>3} type={:>2} usage={:>2} idx={:>2} raw={}",
                e[4],
                e[6],
                e[7],
                hex(e)
            );
        }
    }
    if let Some(va) = file.sections_of_type(SectionType::VERTEX_ARRAY).next() {
        let p = file.payload(&model, va.number)?;
        let words: Vec<u32> = p
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        println!("VA: {words:?}");
    }
    if let Some(ta) = file.sections_of_type(SectionType::TRIANGLE_ARRAY).next() {
        let p = file.payload(&model, ta.number)?;
        let words: Vec<u32> = p
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        println!("TA: {words:?}");
    }
    if let Some(m) = file.sections_of_type(SectionType::MESH).next() {
        let p = file.payload(&model, m.number)?;
        let words: Vec<u32> = p
            .chunks(4)
            .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
            .collect();
        println!("Mesh: {words:?}");
    }
    Ok(())
}
