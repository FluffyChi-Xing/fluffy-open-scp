//! Replace a mutated template entry in a reference package with the clean
//! vanilla-form bytes from a source package.
//! Usage: ... -- <clean-source.package> <hex-instance> <reference-in> <reference-out>
use dbpf::{OverlayEntry, Package, ResourceId};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let want = u32::from_str_radix(args[2].trim_start_matches("0x"), 16).unwrap();
    let out_path = args[4].clone();

    // Windows mmap：Package 打开会映射文件；读写同一文件会撞 os 1224。
    // 因此先释放两个 Package 的映射，再写临时文件改名收尾。
    let source_path = std::path::PathBuf::from(&args[1]);
    let reference_path = std::path::PathBuf::from(&args[3]);
    let clean_bytes = {
        let source = Package::open(&source_path).expect("open source");
        let clean = source
            .entries()
            .iter()
            .find(|e| e.id.instance == want)
            .expect("clean entry not found in source");
        source.read(clean).expect("read clean").clone()
    };

    let mut entries = Vec::new();
    let mut replaced = false;
    {
        let reference = Package::open(&reference_path).expect("open reference");
        for entry in reference.entries() {
            let bytes = reference.read(entry).expect("read entry");
            if entry.id.instance == want && entry.id.type_id == 0x00B1_B104 {
                replaced = true;
                println!(
                    "replace {:08X}:{:08X}:{:08X} {}B -> {}B",
                    entry.id.type_id,
                    entry.id.group,
                    entry.id.instance,
                    bytes.len(),
                    clean_bytes.len()
                );
                entries.push(OverlayEntry::new(entry.id, clean_bytes.clone()));
            } else {
                entries.push(OverlayEntry::new(entry.id, bytes));
            }
        }
    }

    let payload = dbpf::write_uncompressed_overlay(&entries).expect("write overlay");
    let temp_path = format!("{out_path}.tmp");
    std::fs::write(&temp_path, payload).expect("write temp");
    println!("wrote {temp_path}");
}
