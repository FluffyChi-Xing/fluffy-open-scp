//! 扫描真实包中预览缺口类型的样本：TGA/Cursor/Greyscale/TTF/0x024a0e52/ER2。
//! 每类 dump 前几个样本到 tmp/preview-gaps/ 供解码器开发。

use dbpf::Package;

const WANTED: &[(u32, &str)] = &[
    (0x2f7d_0006, "tga"),
    (0x0239_3756, "cursor"),
    (0x03e4_21ec, "grey8"),
    (0x03e4_21ed, "grey32"),
    (0x03e4_21f0, "grey16"),
    (0x276c_a4b9, "ttf"),
    (0x024a_0e52, "spore"),
    (0x0806_8aeb, "er2bin"),
    (0x0806_8aec, "er2"),
    (0x15ae_e750, "effectsdir"),
];

fn main() -> dbpf::Result<()> {
    let out_dir = std::path::Path::new("../../tmp/preview-gaps");
    std::fs::create_dir_all(out_dir).unwrap();
    for arg in std::env::args().skip(1) {
        let package = Package::open(&arg)?;
        let name = std::path::Path::new(&arg)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let mut counts: Vec<(usize, u32, &str)> = Vec::new();
        for (type_id, label) in WANTED {
            let matches: Vec<_> = package
                .entries()
                .iter()
                .filter(|e| e.id.type_id == *type_id)
                .collect();
            counts.push((matches.len(), *type_id, label));
            for (i, entry) in matches.iter().take(3).enumerate() {
                let data = package.read(entry)?;
                let file = out_dir.join(format!(
                    "{label}_{name}_{:08x}_{}.bin",
                    entry.id.instance, i
                ));
                std::fs::write(file, &data).unwrap();
            }
        }
        println!("== {name} ==");
        for (count, type_id, label) in counts {
            println!("  {label:<12} {type_id:#010x}  {count}");
        }
    }
    Ok(())
}
