//! 离线对拍 R8：NH house 模型 → 解析全部 section → build_template 重建
//! （NH 顺序 + 间隙 + w25 + 类型表保序）→ 逐字节比对。
//! 用法：cargo run -p rw4 --release --example nh_template_roundtrip -- <nh_pkg>
use dbpf::Package;
use rw4::writer::{TemplatePiece, build_template};
use rw4::{Rw4File, SectionType};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pkg = Package::open(&args[0])?;
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x2F4E_681B && e.id.instance == 0x06C5_470C)
        .expect("NH LOD1 model");
    let original = pkg.read(entry)?;
    let file = Rw4File::parse(&original).unwrap();

    // NH 的 section 顺序与载荷
    let mut pieces: Vec<TemplatePiece> = Vec::new();
    let mut type_table: Vec<u32> = vec![0, 0x10030, 0x10031, 0x10032, 0x10010];
    let mut prev_end = 0x138u32;
    for section in file.sections() {
        let payload = file.payload(&original, section.number)?.to_vec();
        // 间隙检测：Collision(0x80003) 结束到 VF(0x20004) 起点的原版空隙
        if section.type_code == 0x20004 {
            let gap = section.pos - prev_end;
            if gap > 0 {
                pieces.push(TemplatePiece::Gap(gap));
            }
        }
        prev_end = section.pos + section.size;
        let alignment = if section.alignment == 0 {
            4
        } else {
            section.alignment
        };
        pieces.push(TemplatePiece::Section(rw4::writer::SectionSpec {
            type_code: section.type_code,
            alignment,
            payload,
            fixups: Vec::new(),
        }));
        if !type_table.contains(&section.type_code) {
            type_table.push(section.type_code);
        }
    }
    // NH 的类型表追加段按数值升序
    let mut extras: Vec<u32> = type_table[5..].to_vec();
    extras.sort_unstable();
    type_table.truncate(5);
    type_table.extend_from_slice(&extras);

    // NH 头部字段：w25 与 0x10007 块
    let w25 = words_at(&original, 0x64, 1)[0];
    let b10007_off = find_block(&original, 0x10_007);
    let block10007 = words_at(&original, b10007_off + 4, 6).try_into().unwrap();

    let rebuilt = build_template(1, &pieces, &type_table, w25, block10007);
    println!(
        "original {} bytes, rebuilt {} bytes",
        original.len(),
        rebuilt.len()
    );
    std::fs::write("tmp/nh_house_original.rw4", &original)?;
    std::fs::write("tmp/nh_house_rebuilt.rw4", &rebuilt)?;

    let n = original.len().min(rebuilt.len());
    let mut diffs = 0usize;
    let mut regions: Vec<(usize, usize)> = Vec::new();
    for i in 0..n {
        if original[i] != rebuilt[i] {
            diffs += 1;
            match regions.last_mut() {
                Some((_, end)) if i == *end => *end = i + 1,
                _ => regions.push((i, i + 1)),
            }
        }
    }
    println!("{diffs} differing bytes, {} regions:", regions.len());
    for (start, end) in regions.iter().take(20) {
        let e = (*end).min(n);
        println!(
            "  @{:#06x}..{:#06x}\n    orig={:02x?}\n    new ={rev:02x?}",
            start,
            e,
            &original[*start..e.min(*start + 24)],
            rev = &rebuilt[*start..e.min(*start + 24)],
        );
    }
    Ok(())
}

fn words_at(data: &[u8], off: usize, count: usize) -> Vec<u32> {
    (0..count)
        .map(|i| u32::from_le_bytes(data[off + i * 4..off + i * 4 + 4].try_into().unwrap()))
        .collect()
}

fn find_block(data: &[u8], id: u32) -> usize {
    let mut off = 0x98usize;
    loop {
        let bid = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
        if bid == id {
            return off;
        }
        match bid {
            0x10_004 => off += 28,
            0x10_005 => {
                let cnt = u32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap());
                off += 12 + 4 * cnt as usize;
            }
            0x10_006 => off += 36,
            0x10_007 => off += 28,
            0x10_008 => off += 12,
            other => panic!("unknown block {other:#x} at {off:#x}"),
        }
    }
}
