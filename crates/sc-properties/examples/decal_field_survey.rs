//! 只读取证：lot decal 单元各字段在真实包中的存在率与取值范围。
//!
//! 起因：脱壳 exe 的 `SC::cGraphicsUnitDecals` 属性注册（FUN_0081d680）里出现了
//! **第 7 个字段 `0x0D1090B0`（Float）**，而原 SCP C# 只列了 6 个。本探针回答：
//! 0x0B0 在真实数据里是否存在、取值如何；以及 Depth(`0x0D109070+cat`) 的实际量级。
//!
//! 用法：cargo run -p sc-properties --release --example decal_field_survey -- <package...>

use dbpf::Package;

const PROPERTY_TYPE: u32 = 0x00B1_B104;

/// (hash, 名称)——来自 exe 注册函数 FUN_0081d680 的 7 个声明。
const FIELDS: [(u32, &str); 7] = [
    (0x0D10_9050, "ID(Key)"),
    (0x0D10_9060, "Transform(Transform)"),
    (0x0D10_9070, "Depth(Float)"),
    (0x0D10_9080, "MaterialData(Vector3)"),
    (0x0D10_9090, "RenderGroup(Key)"),
    (0x0D10_90A0, "MachineSpec(Int32)"),
    (0x0D10_90B0, "??? (Float)"),
];

#[derive(Default, Clone, Copy)]
struct Stat {
    lots: usize,
    values: usize,
    min: f32,
    max: f32,
    sum: f64,
}

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut stats: Vec<Stat> = vec![Stat::default(); FIELDS.len()];
    let mut lots_with_decals = 0usize;
    let verbose = std::env::var("DECAL_DEBUG").is_ok();
    let mut debug_budget = 6usize;

    for path in &paths {
        let Ok(package) = Package::open(path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != PROPERTY_TYPE {
                continue;
            }
            let Ok(data) = package.read(entry) else { continue };
            let Ok(file) =
                sc_properties::PropertyFile::parse_with_limits(&data, sc_properties::ParseLimits::default())
            else {
                continue;
            };
            if file.get(0x0D10_9050).is_none() {
                continue; // 没有 decal 单元的 lot
            }
            lots_with_decals += 1;
            for (slot, (base, _)) in FIELDS.iter().enumerate() {
                // 3 个并行槽位：任一存在即计入
                let mut present = false;
                for cat in 0..3u32 {
                    let Some(property) = file.get(base + cat) else {
                        continue;
                    };
                    if !matches!(property.kind, sc_properties::Kind::Array(_)) {
                        if verbose && debug_budget > 0 {
                            println!(
                                "  [debug] lot 0x{:08X} hash 0x{:08X} kind={:?} type={}",
                                entry.id.instance,
                                base + cat,
                                property.kind,
                                property.prop_type.name()
                            );
                            debug_budget -= 1;
                        }
                        continue;
                    }
                    let Some(values) = property.array() else {
                        continue;
                    };
                    present = true;
                    for value in values {
                        if let sc_properties::Value::Float(v) = value {
                            let stat = &mut stats[slot];
                            if stat.values == 0 {
                                stat.min = *v;
                                stat.max = *v;
                            }
                            stat.min = stat.min.min(*v);
                            stat.max = stat.max.max(*v);
                            stat.sum += f64::from(*v);
                            stat.values += 1;
                        }
                    }
                }
                if present {
                    stats[slot].lots += 1;
                }
            }
        }
    }

    println!("== 含 decal 单元的 lot property 数：{lots_with_decals}");
    println!(
        "{:<26} {:>8} {:>10} {:>10} {:>10} {:>10}",
        "字段", "有该数组的lot", "浮点值个数", "min", "max", "mean"
    );
    for (slot, (hash, name)) in FIELDS.iter().enumerate() {
        let s = stats[slot];
        // 注意：min/max/mean 仅对 Float 字段有意义，非 Float 字段恒为 "-"；
        // 「有该数组的 lot」独立统计，不受此影响。
        let (min, max, mean) = if s.values > 0 {
            (
                format!("{:.3}", s.min),
                format!("{:.3}", s.max),
                format!("{:.3}", s.sum / s.values as f64),
            )
        } else {
            ("-".into(), "-".into(), "-".into())
        };
        println!(
            "0x{hash:08X} {name:<14} {:>8} {:>10} {min:>10} {max:>10} {mean:>10}",
            s.lots, s.values
        );
    }
    let _ = &stats;
}
