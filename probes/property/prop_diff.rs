//! 语义级 property diff：两包同名资源按 key 对比，输出值变化。仅开发用。
//! 用法：prop_diff <vanilla> <modded> <tgi_list.txt（每行 type group instance 十六进制）> [limit]
use std::collections::BTreeMap;

fn load_prop(package: &dbpf::Package, id: dbpf::ResourceId) -> Option<sc_properties::PropertyFile> {
    let entry = package.entry(id)?;
    let data = package.read(entry).ok()?;
    sc_properties::PropertyFile::parse(&data).ok()
}

fn fmt_vals(p: &sc_properties::Property) -> String {
    match &p.kind {
        sc_properties::Kind::Empty => "(empty)".into(),
        sc_properties::Kind::Scalar(v) => format!("{v}"),
        sc_properties::Kind::Array(vs) => {
            if vs.len() <= 8 {
                format!("[{}]", vs.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", "))
            } else {
                format!("[{} x{}]", vs.first().map(|v| v.to_string()).unwrap_or_default(), vs.len())
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let van = dbpf::Package::open(&args[0]).unwrap();
    let mod_ = dbpf::Package::open(&args[1]).unwrap();
    let list = std::fs::read_to_string(&args[2]).unwrap();
    for line in list.lines() {
        let nums: Vec<u32> = line.split_whitespace()
            .filter_map(|s| u32::from_str_radix(s, 16).ok()).collect();
        if nums.len() != 3 { continue; }
        let id = dbpf::ResourceId { type_id: nums[0], group: nums[1], instance: nums[2] };
        let (Some(pa), Some(pb)) = (load_prop(&van, id), load_prop(&mod_, id)) else {
            println!("== {:08X}:{:08X}:{:08X}: parse failed / missing", id.type_id, id.group, id.instance);
            continue;
        };
        let ma: BTreeMap<u32, &sc_properties::Property> = pa.values.iter().map(|p| (p.hash, p)).collect();
        let mb: BTreeMap<u32, &sc_properties::Property> = pb.values.iter().map(|p| (p.hash, p)).collect();
        for (h, pb_prop) in &mb {
            match ma.get(h) {
                None => println!("== {:08X}:{:08X}:{:08X}  KEY-ONLY-IN-MOD {:08X}: {}", id.type_id, id.group, id.instance, h, fmt_vals(pb_prop)),
                Some(pa_prop) => if pa_prop != pb_prop {
                    println!("== {:08X}:{:08X}:{:08X}  {:08X}: {}  =>  {}", id.type_id, id.group, id.instance, h, fmt_vals(pa_prop), fmt_vals(pb_prop));
                },
            }
        }
        for (h, pa_prop) in &ma {
            if !mb.contains_key(h) {
                println!("== {:08X}:{:08X}:{:08X}  KEY-ONLY-IN-VAN {:08X}: {}", id.type_id, id.group, id.instance, h, fmt_vals(pa_prop));
            }
        }
    }
}
