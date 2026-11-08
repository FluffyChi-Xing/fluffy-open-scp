//! 诊断探针：扫包内全部 0x00B1B104 属性资源，输出 unit 六族列的存在矩阵
//! 与 light 列的完整值（对拍 SC5 Asset Packager light_contract 契约）。
//! 用法：cargo run -p probes --release --example light_columns_dump -- <pkg> [pkg2…]
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const PROP_TYPE: u32 = 0x00B1_B104;

const LIGHT_COLS: &[(u32, &str)] = &[
    (0x0CAA_8F10, "ids"),
    (0x0CAA_8F11, "types"),
    (0x0CAA_8F12, "transforms"),
    (0x0CAA_8F13, "colors"),
    (0x0CAA_8F14, "radii"),
    (0x0CAA_8F15, "inner_radii"),
    (0x0CAA_8F16, "diffuse"),
    (0x0CAA_8F17, "names"),
    (0x0D4C_96B4, "specular"),
    (0x0D4C_AD23, "lengths"),
    (0x0E01_200F, "cull"),
    (0x0E4D_F244, "falloff"),
    (0x0E98_D445, "volumetric"),
    (0x0EC4_567E, "vol_strength"),
];
const TIME_COLS: &[(u32, &str)] = &[
    (0x0CE7_1B10, "time_ids"),
    (0x0CE7_1B11, "time_periods"),
    (0x0CE7_1B12, "time_ranges"),
    (0x0CE7_1B13, "time_vary_ranges"),
];
const OTHER_COLS: &[(u32, &str)] = &[
    (0x0E1B_AC61, "spawner_ids"),
    (0x0E1B_AC62, "spawner_transforms"),
    (0x0CAA_6841, "path_pairs"),
    (0x0CAA_680D, "path_points"),
    (0x0CB0_0ED8, "path_tangents"),
    (0x0CAA_6832, "path_indices"),
    (0x0CC8_FE7A, "path_entries"),
    (0x0F16_9C46, "interior_ctrl"),
    (0x0D9A_4E54, "path_scope"),
    (0x0D9A_A2BB, "path_road_connect"),
];

fn vals(kind: &Kind) -> &[Value] {
    match kind {
        Kind::Array(vs) => vs,
        Kind::Scalar(v) => std::slice::from_ref(v),
        Kind::Empty => &[],
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        println!("=== {path} ===");
        let pkg = Package::open(&path).expect("open");
        let mut found = 0usize;
        for e in pkg.entries() {
            if e.id.type_id != PROP_TYPE {
                continue;
            }
            let data = match pkg.read(&e) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let pf = match PropertyFile::parse(&data) {
                Ok(pf) => pf,
                Err(_) => continue,
            };
            let light_count = pf
                .values
                .iter()
                .filter(|p| p.hash == 0x0CAA_8F10)
                .map(|p| vals(&p.kind).len())
                .sum::<usize>();
            let has_any_unit = pf.values.iter().any(|p| {
                LIGHT_COLS
                    .iter()
                    .chain(TIME_COLS.iter())
                    .chain(OTHER_COLS.iter())
                    .any(|(h, _)| *h == p.hash)
            });
            if !has_any_unit {
                continue;
            }
            found += 1;
            println!(
                "--- TGI {:08X}:{:08X}:{:08X} (props={}) ---",
                e.id.type_id,
                e.id.group,
                e.id.instance,
                pf.values.len()
            );
            for (hash, label) in LIGHT_COLS
                .iter()
                .chain(TIME_COLS.iter())
                .chain(OTHER_COLS.iter())
            {
                let props: Vec<_> = pf.values.iter().filter(|p| p.hash == *hash).collect();
                if props.is_empty() {
                    continue;
                }
                for p in props {
                    let vs = vals(&p.kind);
                    let show: Vec<String> = vs.iter().take(6).map(|v| v.to_string()).collect();
                    let light_marker = if *hash == 0x0CAA_8F10 {
                        "  <<LIGHTS"
                    } else {
                        ""
                    };
                    println!(
                        "  {label:<16} {:08X} type={} n={} [{}]{}",
                        p.hash,
                        p.prop_type as u16,
                        vs.len(),
                        show.join(" | "),
                        light_marker
                    );
                }
            }
            let _ = light_count;
        }
        if found == 0 {
            println!("  (no 0x00B1B104 resource carries unit columns)");
        }
    }
}
