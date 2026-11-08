//! 冒烟工具：打印 LOTM 每个材质组的槽位载荷（有/无纹理、参数字节数）。
//! 用法：cargo run --release -p fluffy-open-scp --example lotm_slots -- <model.lotm> [...]
use fluffy_open_scp_lib::asset_build_m1::lotm;

fn main() {
    for path in std::env::args().skip(1) {
        let bytes = std::fs::read(&path).expect("read");
        let doc = lotm::parse(&bytes).expect("parse");
        println!(
            "{path}\n  meshes={} materials={}",
            doc.meshes.len(),
            doc.materials.len(),
        );
        let empty_slots = doc
            .materials
            .iter()
            .enumerate()
            .filter(|(_, m)| m.pngs.iter().all(|p| p.is_none()))
            .count();
        println!(
            "  materials with ALL texture slots empty: {empty_slots}/{}",
            doc.materials.len()
        );
        for (index, material) in doc.materials.iter().enumerate().take(8) {
            let slots: Vec<String> = material
                .pngs
                .iter()
                .map(|png| match png {
                    Some(bytes) => format!("{}B", bytes.len()),
                    None => "-".into(),
                })
                .collect();
            println!(
                "  material[{index}] pngs=[{}] params={:?} cols={}",
                slots.join(" "),
                material.params_f32.as_ref().map(|p| p.len()),
                material.param_cols,
            );
        }
    }
}
