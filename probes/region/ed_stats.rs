//! ED u32 字段统计：各字节通道的非零分布 + 抽样值。仅开发用。
fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().unwrap();
    let group = u32::from_str_radix(&args.next().unwrap().trim_start_matches("0x"), 16).unwrap();
    let p = dbpf::Package::open(&path).unwrap();
    let mut ch_nz = [0usize; 4];
    let mut total = 0usize;
    let mut samples: Vec<u32> = Vec::new();
    for e in p.entries() {
        if e.id.type_id == 0x03E4_21ED && e.id.group == group {
            let data = p.read(e).unwrap();
            if data.len() < 20 { continue; }
            for c in data[20..].chunks_exact(4) {
                let v = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
                for k in 0..4 { if (v >> (k * 8)) & 0xFF != 0 { ch_nz[k] += 1; } }
                if samples.len() < 24 && v != 0 { samples.push(v); }
                total += 1;
            }
        }
    }
    println!("cells={total} nonzero per byte-channel: {ch_nz:?}");
    println!("samples: {:?}", samples.iter().map(|v| format!("{v:08X}")).take(24).collect::<Vec<_>>());
}
