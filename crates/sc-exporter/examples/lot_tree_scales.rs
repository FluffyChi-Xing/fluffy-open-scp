//! 冒烟工具：实测 lot 树道具的放置缩放与推算实际树高。
//! 实际树高 = 树模型原生高度（GLB 顶点实测 51.2m，4DF43690）× 单元变换
//! 基向量长度（引擎 impostor 同构：聚合模型 × 单元 scale）。
//! 用法：cargo run -p sc-exporter --release --example lot_tree_scales -- <lot_instance_hex> <pkg> [...]
use dbpf::Package;
use sc_properties::{LotEditorDocument, PropertyFile};

fn basis_scale(matrix: &[f32]) -> f64 {
    if matrix.len() != 12 {
        return 1.0;
    }
    let l = |x: f32, y: f32, z: f32| -> f64 {
        f64::from((x * x + y * y + z * z).sqrt())
    };
    (l(matrix[0], matrix[1], matrix[2])
        + l(matrix[3], matrix[4], matrix[5])
        + l(matrix[6], matrix[7], matrix[8]))
        / 3.0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let trees: [u32; 4] = [0x1498_4C68, 0x1498_4C69, 0x1498_4C6A, 0x1498_4C6B];
    for path in &args[1..] {
        let Ok(package) = Package::open(path) else { continue };
        for entry in package
            .entries()
            .iter()
            .filter(|e| e.id.instance == instance && e.id.type_id == 0x00B1_B104)
        {
            let Ok(data) = package.read(entry) else { continue };
            let Ok(properties) = PropertyFile::parse_with_limits(
                &data,
                sc_properties::ParseLimits::default(),
            ) else {
                continue;
            };
            let document = LotEditorDocument::from_property_file(properties);
            let units = document.assemble_units();
            println!("{path}: G {:08X}", entry.id.group);
            let mut shown = 0usize;
            for unit in &units.units {
                if let sc_properties::LotUnit::Prop {
                    index,
                    resource_id: Some(rid),
                    transform,
                    scale,
                    ..
                } = unit
                {
                    if !trees.contains(rid) {
                        continue;
                    }
                    let basis = transform
                        .as_ref()
                        .map(|t| basis_scale(&t.matrix))
                        .unwrap_or(1.0);
                    let unknown = scale.as_ref().map(|v| v.to_string()).unwrap_or("-".into());
                    let m = transform
                        .as_ref()
                        .map(|t| format!("{:?}", t.matrix))
                        .unwrap_or("-".into());
                    println!(
                        "  树 idx {index} rid {rid:08X} 基缩放 {basis:.3} scale字段 {unknown}
    matrix = {m}"
                    );
                    shown += 1;
                }
            }
            if shown > 0 {
                return;
            }
        }
    }
}
