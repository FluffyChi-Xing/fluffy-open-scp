//! 动画招牌（decal unit materialData[1] > 0）的三元组全库分布取证。
//!
//! 回答（2026-10-05 十一轮"LED 淡到看不见"对拍）：
//! 1) 灯强 materialLightScale = x×16+0.25 的真实 x 分布——x=0 时 0.25
//!    是否常见（= LED 比静态暗十几倍的主嫌疑）；
//! 2) z（tubeFactor = z×8+1）分布；
//! 3) 对照：非动画 decal（y=0）的 x 分布。
//!
//! 用法：cargo run -p sc-properties --release --example decal_material_data -- <package...>

use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;
const MATERIAL_DATA_BASE: u32 = 0x0D10_9080;

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut animated: Vec<(u32, [f32; 3])> = Vec::new();
    let mut static_x: Vec<f32> = Vec::new();

    for path in &paths {
        let Ok(package) = Package::open(path) else {
            continue;
        };
        for entry in package.entries().iter() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) = sc_properties::PropertyFile::parse_with_limits(
                &data,
                sc_properties::ParseLimits::default(),
            ) else {
                continue;
            };
            for cat in 0..3u32 {
                let Some(property) = file.get(MATERIAL_DATA_BASE + cat) else {
                    continue;
                };
                let Some(values) = property.array() else {
                    continue;
                };
                for value in values {
                    let sc_properties::Value::Vector3(v) = value else {
                        continue;
                    };
                    if v[1] > 0.0 {
                        animated.push((entry.id.instance, *v));
                    } else {
                        static_x.push(v[0]);
                    }
                }
            }
        }
    }

    println!("== 动画招牌（materialData[1]>0）：{} 条", animated.len());
    let mut x_vals: Vec<f32> = animated.iter().map(|(_, v)| v[0]).collect();
    x_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !x_vals.is_empty() {
        let q = |p: f64| x_vals[(p * (x_vals.len() - 1) as f64) as usize];
        println!(
            "  x（灯强）min={:.3} p25={:.3} med={:.3} p75={:.3} max={:.3}",
            x_vals[0],
            q(0.25),
            q(0.5),
            q(0.75),
            x_vals[x_vals.len() - 1]
        );
        let zero = x_vals.iter().filter(|&&x| x == 0.0).count();
        println!("  x==0 的占比：{}/{}", zero, x_vals.len());
    }
    let mut z_vals: Vec<f32> = animated.iter().map(|(_, v)| v[2]).collect();
    z_vals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !z_vals.is_empty() {
        println!(
            "  z（灯管）min={:.3} med={:.3} max={:.3}",
            z_vals[0],
            z_vals[z_vals.len() / 2],
            z_vals[z_vals.len() - 1]
        );
    }
    println!("前 12 条样本：");
    for (inst, v) in animated.iter().take(12) {
        println!(
            "  lot 0x{:08X}: x={:.3} y={:.3} z={:.3}（scale={:.2} tube={:.2}）",
            inst,
            v[0],
            v[1],
            v[2],
            v[0] * 16.0 + 0.25,
            v[2] * 8.0 + 1.0
        );
    }
    println!("== 非动画 decal（y=0）：{} 条", static_x.len());
    if !static_x.is_empty() {
        static_x.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let zero = static_x.iter().filter(|&&x| x == 0.0).count();
        println!(
            "  x min={:.3} med={:.3} max={:.3}，x==0 占比 {}/{}",
            static_x[0],
            static_x[static_x.len() / 2],
            static_x[static_x.len() - 1],
            zero,
            static_x.len()
        );
    }
}
