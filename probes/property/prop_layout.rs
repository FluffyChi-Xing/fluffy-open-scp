//! 探针：打印 property 资源每个条目的文件偏移/hash/type/flags/载荷 hex，
//! 并可选与另一资源对拍 flags。用于定位写回路径丢失的编码标志位。仅开发用。
//!
//! 用法：cargo run -p probes --release --example prop_layout --
//!   <package> <instance_hex> [group_hex]
use dbpf::Package;

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
    let d = p.read(&e).unwrap();
    let be32 = |b: &[u8], o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let be16 = |b: &[u8], o: usize| u16::from_be_bytes([b[o], b[o + 1]]);
    let count = be32(&d, 0) as usize;
    println!("count={count} len={}B", d.len());
    let mut o = 4usize;
    for _ in 0..count {
        let hash = be32(&d, o);
        let ftype = be16(&d, o + 4);
        let flags = be16(&d, o + 6);
        let mut size = 0usize;
        let mut note = String::new();
        let scalar = |ft: u16| -> usize {
            match ft {
                1 => 1,
                9 | 10 | 13 => 4,
                18 | 19 => 0, // 变长，另行处理
                32 => 12,
                34 => 8,
                48 => 8,
                49 | 50 => 12,
                51 | 52 => 16,
                56 => 50, // 近似，不用于本 dump
                57 => 24,
                _ => 0,
            }
        };
        if flags & 0x40 != 0 {
            let n = be32(&d, o + 8) as usize;
            let isz = be32(&d, o + 12) as usize;
            size = 8 + n * isz;
            if ftype == 34 && isz == 16 {
                let mut parts = Vec::new();
                for k in 0..n {
                    let b = o + 16 + k * 16;
                    parts.push(format!(
                        "{:08X}:{:08X}(b={:#06x})",
                        be32(&d, b + 8),
                        be32(&d, b + 12),
                        be16(&d, b + 6)
                    ));
                }
                note = format!("array n={n} itemSize={isz} texts [{}]", parts.join(", "));
            } else {
                note = format!("array n={n} itemSize={isz}");
            }
        } else if ftype == 18 || ftype == 19 {
            if flags & 0x100 != 0 && ftype == 18 {
                let n = d[o + 8] as usize;
                size = 1 + n;
                note = format!("string8 len={n}");
            } else {
                let n = be16(&d, o + 8) as usize;
                size = 2 + n;
                note = format!("string len={n}");
            }
        } else if ftype == 34 {
            // 标量 Text：[00 00 00 01][u16 a][u16 b][table][instance]
            let a = be16(&d, o + 12);
            let b = be16(&d, o + 14);
            let table = be32(&d, o + 16);
            let inst2 = be32(&d, o + 20);
            size = 16;
            note = format!(
                "text a={a:#06x} b={b:#06x} -> {table:08X}:{inst2:08X}"
            );
        } else {
            size = scalar(ftype);
        }
        let payload_hex: String = d[o + 8..(o + 8 + size).min(d.len())]
            .iter()
            .take(24)
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "off {:#06x} {:08X} type={:<2} flags={:#06x} {} [{}]",
            o, hash, ftype, flags, note, payload_hex
        );
        o += 8 + size;
    }
    println!("end at {:#06x} (len {})", o, d.len());
}
