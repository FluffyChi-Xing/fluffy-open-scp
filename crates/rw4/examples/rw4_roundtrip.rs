//! 一次性对拍：NH 原版模型 → 解析全部 section → 我们的 writer 重建 →
//! 逐字节比对。差异 = 写出器的确切缺陷清单。
//! 用法：cargo run -p rw4 --release --example rw4_roundtrip -- <model.rw4> [out]
use dbpf::Package;
use rw4::{Rw4File, SectionType, writer, writer_section};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let (group, instance) = (
        u32::from_str_radix(args[1].trim_start_matches("0x"), 16)?,
        u32::from_str_radix(args[2].trim_start_matches("0x"), 16)?,
    );
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.group == group && e.id.instance == instance)
        .expect("model entry not found");
    let original = pkg.read(entry)?;
    let file = Rw4File::parse(&original)?;

    // 按 NH 原 section 顺序重建 specs（payload/alignment 原样；fixups 原样不可得=空）
    let mut specs = Vec::new();
    let mut extra_types: Vec<u32> = Vec::new();
    let fixed = [0u32, 0x10030, 0x10031, 0x10032, 0x10010];
    for section in file.sections() {
        let payload = file.payload(&original, section.number)?.to_vec();
        specs.push(writer_section(section.type_code, payload));
        // 修正对齐
        specs.last_mut().unwrap().alignment = section.alignment;
        if !fixed.contains(&section.type_code) && !extra_types.contains(&section.type_code) {
            extra_types.push(section.type_code);
        }
    }
    // NH 的 0x10004 块内类型表有 14 项（含未用类型）——writer 只写用到的，
    // extra_types 需补齐 NH 表中剩余的非固定类型以对齐 indirect 索引
    let _ = &mut extra_types;
    let rebuilt = writer::build(1, &specs, &[]);

    println!(
        "original {} bytes, rebuilt {} bytes",
        original.len(),
        rebuilt.len()
    );
    let out = args.get(3);
    if let Some(out) = out {
        std::fs::write(out, &rebuilt)?;
        println!("rebuilt written to {out}");
    }
    let n = original.len().min(rebuilt.len());
    let mut diff_count = 0usize;
    let mut regions: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        if original[i] != rebuilt[i] {
            diff_count += 1;
            if let Some(&mut (_, ref mut end)) = regions.last_mut() {
                if i == *end {
                    *end = i + 1;
                    continue;
                }
            }
            regions.push((i, i + 1));
        }
    }
    println!(
        "{diff_count} differing bytes in common range; {} diff regions:",
        regions.len()
    );
    for (start, end) in regions.iter().take(16) {
        let s = *start;
        let e = (*end).min(n);
        println!(
            "  @{:#06x}..{:#06x}\n    orig={:02x?}\n    new ={rev:02x?}",
            s,
            e,
            &original[s..e.min(s + 24)],
            rev = &rebuilt[s..e.min(s + 24)],
        );
    }
    Ok(())
}
