//! 冒烟工具：为前端 dbpf-writer 测试生成 write_uncompressed_overlay 十六进制 fixture。
use dbpf::ResourceId;
use dbpf::OverlayEntry;

fn main() {
    let entries = vec![
        OverlayEntry::new(
            ResourceId { type_id: 0x0A98EAF0, group: 0x02FABF01, instance: 0x00000001 },
            b"AB".to_vec(),
        ),
        OverlayEntry::new(
            ResourceId { type_id: 0x00B1B104, group: 0x09878A01, instance: 0x64088034 },
            vec![1, 2, 3],
        ),
    ];
    let bytes = dbpf::write_uncompressed_overlay(&entries).unwrap();
    println!("{}", bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>());
}
