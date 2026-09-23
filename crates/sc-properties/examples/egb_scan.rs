//! 探针：在 EcoGame 存档快照（解压后）中找 u16 常量长跑——水位面候选值。
//! 同时找 f32 常量长跑。仅开发用。
fn main() {
    let path = std::env::args().nth(1).expect("usage: egb_scan <file>");
    let d = std::fs::read(&path).unwrap();
    println!("{path}: {} B", d.len());

    // u16 LE 常量长跑（游程 ≥ 512 个元素）
    let n16 = d.len() / 2;
    let mut i = 0usize;
    let mut runs: Vec<(u64, usize, usize)> = Vec::new(); // (value, start_elem, len_elems)
    while i < n16 {
        let v = u16::from_le_bytes([d[i * 2], d[i * 2 + 1]]);
        let mut j = i + 1;
        while j < n16 && u16::from_le_bytes([d[j * 2], d[j * 2 + 1]]) == v {
            j += 1;
        }
        if j - i >= 512 {
            runs.push((u64::from(v), i, j - i));
        }
        i = j;
    }
    runs.sort_by(|a, b| b.2.cmp(&a.2));
    println!("-- u16 长跑 top20（值, 起始字节, 元素数）--");
    for (v, s, l) in runs.iter().take(20) {
        println!("  {v:#06x} ({v}) @ {s:#x} x{l}");
    }

    // f32 常量长跑
    let n32 = d.len() / 4;
    let mut i = 0usize;
    let mut runs32: Vec<(f32, usize, usize)> = Vec::new();
    while i < n32 {
        let v = f32::from_le_bytes([d[i * 4], d[i * 4 + 1], d[i * 4 + 2], d[i * 4 + 3]]);
        let mut j = i + 1;
        while j < n32 && f32::from_le_bytes([d[j * 4], d[j * 4 + 1], d[j * 4 + 2], d[j * 4 + 3]]) == v
        {
            j += 1;
        }
        if j - i >= 256 && v.is_finite() {
            runs32.push((v, i, j - i));
        }
        i = j;
    }
    runs32.sort_by(|a, b| b.2.cmp(&a.2));
    println!("-- f32 长跑 top20 --");
    for (v, s, l) in runs32.iter().take(20) {
        println!("  {v} @ {s:#x} x{l}");
    }
}
