//! 探针：城市 egb 中找非零 u16 常量跑（水位候选）+ 地图块结构盘点。仅开发用。
fn main() {
    let path = std::env::args().nth(1).expect("usage: egb_water_hunt <file>");
    let d = std::fs::read(&path).unwrap();
    println!("{path}: {} B", d.len());
    let n16 = d.len() / 2;
    let mut i = 0usize;
    let mut runs: Vec<(u64, usize, usize)> = Vec::new();
    while i < n16 {
        let v = u16::from_le_bytes([d[i * 2], d[i * 2 + 1]]);
        let mut j = i + 1;
        while j < n16 && u16::from_le_bytes([d[j * 2], d[j * 2 + 1]]) == v {
            j += 1;
        }
        if v != 0 && j - i >= 32 {
            runs.push((u64::from(v), i, j - i));
        }
        i = j;
    }
    runs.sort_by(|a, b| b.2.cmp(&a.2));
    println!("-- 非零 u16 长跑 top40 --");
    for (v, s, l) in runs.iter().take(40) {
        println!("  {v} ({v:#06x}) @ {s:#x} x{l}");
    }
    // 32768 元素块的清单（推测地图数组）：找所有 0x8014=32772 或类似步长
    // 直接看前 0x100 字节的头部与 0x10000 附近
    println!("-- 头 64 B --");
    for row in 0..4 {
        let off = row * 16;
        let hex: Vec<String> = d[off..off + 16].iter().map(|b| format!("{b:02x}")).collect();
        println!("  {off:08x}: {}", hex.join(" "));
    }
}
