//! 并排 dump 原版/重建 RW4 的头部字段、类型表与 section 表，定位布局差异。
//! 用法：cargo run -p rw4 --release --example nh_layout_diff -- <orig.rw4> <rebuilt.rw4>
use rw4::{Rw4File, SectionType};

fn words(data: &[u8], off: usize, count: usize) -> Vec<u32> {
    (0..count)
        .map(|i| u32::from_le_bytes(data[off + i * 4..off + i * 4 + 4].try_into().unwrap()))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let orig_bytes = std::fs::read(&args[0])?;
    let new_bytes = std::fs::read(&args[1])?;
    println!("parsing orig {}...", args[0]);
    let orig = Rw4File::parse(&orig_bytes)?;
    println!("parsing rebuilt {}...", args[1]);
    let new = Rw4File::parse(&new_bytes)?;

    println!(
        "== file size: orig {} vs new {}",
        orig_bytes.len(),
        new_bytes.len()
    );

    println!("\n== header words @0x1c..0x98 ==");
    let hw_o = words(&orig_bytes, 0x1c, 30);
    let hw_n = words(&new_bytes, 0x1c, 30);
    for (i, (a, b)) in hw_o.iter().zip(&hw_n).enumerate() {
        let off = 0x1c + i * 4;
        let mark = if a == b { " " } else { "*" };
        println!("{mark}@{off:#04x}: {a:#010x}  vs  {b:#010x}");
    }

    println!("\n== 0x10004 block ==");
    let bo = words(&orig_bytes, 0x98, 7);
    let bn = words(&new_bytes, 0x98, 7);
    println!("  orig: {bo:02x?}");
    println!("  new : {bn:02x?}");

    // 0x10005 类型表（id@0xb4, count@0xb8, elem_size@0xbc, 表@0xc0）
    let cnt_o = words(&orig_bytes, 0xb8, 1)[0] as usize;
    let cnt_n = words(&new_bytes, 0xb8, 1)[0] as usize;
    println!("\n== 0x10005 type table: orig {cnt_o} vs new {cnt_n} ==");
    let to = words(&orig_bytes, 0xc0, cnt_o);
    let tn = words(&new_bytes, 0xc0, cnt_n);
    for i in 0..cnt_o.max(cnt_n) {
        let a = to
            .get(i)
            .map(|v| format!("{v:#010x}"))
            .unwrap_or_else(|| "    --    ".into());
        let b = tn
            .get(i)
            .map(|v| format!("{v:#010x}"))
            .unwrap_or_else(|| "    --    ".into());
        let mark = if to.get(i) == tn.get(i) { " " } else { "*" };
        println!("{mark}[{i:2}] {a}  vs  {b}");
    }

    // 0x10006 与 0x10007 块
    let after_types_o = 0xc0 + 4 * cnt_o;
    let after_types_n = 0xc0 + 4 * cnt_n;
    println!("\n== 0x10006 ==");
    println!("  orig: {:02x?}", words(&orig_bytes, after_types_o, 9));
    println!("  new : {:02x?}", words(&new_bytes, after_types_n, 9));
    println!("== 0x10007 ==");
    println!("  orig: {:02x?}", words(&orig_bytes, after_types_o + 36, 7));
    println!("  new : {:02x?}", words(&new_bytes, after_types_n + 36, 7));

    // section 表并排
    println!(
        "\n== sections: orig {} vs new {} ==",
        orig.sections().len(),
        new.sections().len()
    );
    println!(
        "  {:>3} {:>10} {:>8} {:>4} {:>3} {:<10} | {:>10} {:>8} {:>4} {:>3} {:<10}",
        "#", "orig.pos", "size", "algn", "idx", "type", "new.pos", "size", "algn", "idx", "type"
    );
    let so = orig.sections();
    let sn = new.sections();
    for i in 0..so.len().max(sn.len()) {
        let fmt = |s: Option<&rw4::Section>| match s {
            Some(s) => (
                format!("{:#x}", s.pos),
                format!("{}", s.size),
                format!("{}", s.alignment),
                format!("{}", s.type_code_indirect),
                SectionType::name(s.type_code).unwrap_or("?").to_string(),
            ),
            None => (
                "--".into(),
                "--".into(),
                "--".into(),
                "--".into(),
                "--".into(),
            ),
        };
        let (p1, z1, a1, i1, t1) = fmt(so.get(i));
        let (p2, z2, a2, i2, t2) = fmt(sn.get(i));
        let same = so
            .get(i)
            .zip(sn.get(i))
            .map(|(a, b)| a.pos == b.pos && a.size == b.size && a.alignment == b.alignment);
        let mark = match same {
            Some(true) => " ",
            _ => "*",
        };
        println!(
            "{mark}{i:>3} {p1:>10} {z1:>8} {a1:>4} {i1:>3} {t1:<10} | {p2:>10} {z2:>8} {a2:>4} {i2:>3} {t2:<10}"
        );
    }

    // 原版文件中的空隙地图（按原版 section 顺序）
    println!("\n== orig gap map (prev_end -> next.pos) ==");
    let mut prev_end = 0x138u32;
    for s in so {
        if s.pos > prev_end {
            println!(
                "  gap {:#x}..{:#x} ({} bytes) before [{}] {}",
                prev_end,
                s.pos,
                s.pos - prev_end,
                s.number,
                s.type_name().unwrap_or("?")
            );
        }
        prev_end = (s.pos + s.size).max(prev_end);
    }
    if prev_end < orig.section_index_begin() {
        println!(
            "  gap {:#x}..{:#x} ({} bytes) before section index",
            prev_end,
            orig.section_index_begin(),
            orig.section_index_begin() - prev_end
        );
    }
    println!(
        "\n== orig section index: begin {:#x} end {:#x}; new: begin {:#x} end {:#x} ==",
        orig.section_index_begin(),
        orig.section_index_end(),
        new.section_index_begin(),
        new.section_index_end()
    );

    // 重建文件的零段地图（>256 字节的连续零区）
    println!("\n== rebuilt zero-run map (>256B) ==");
    let mut i = 0usize;
    while i < new_bytes.len() {
        if new_bytes[i] == 0 {
            let start = i;
            while i < new_bytes.len() && new_bytes[i] == 0 {
                i += 1;
            }
            if i - start > 256 {
                println!("  {:#x}..{:#x} ({} bytes)", start, i, i - start);
            }
        } else {
            i += 1;
        }
    }

    // geometry 相关 section 的 payload 头部 word dump
    println!("\n== section payload words (first 12) ==");
    for s in so {
        let payload = orig.payload(&orig_bytes, s.number)?;
        let n = (payload.len() / 4).min(12);
        let w: Vec<String> = words(payload, 0, n)
            .iter()
            .map(|v| format!("{v:#x}"))
            .collect();
        println!(
            "  [{}] {:<10} size={:<6} {}",
            s.number,
            s.type_name().unwrap_or("?"),
            payload.len(),
            w.join(" ")
        );
    }

    Ok(())
}
