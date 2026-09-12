//! 探针：打印模型各 mesh 的 texcoord0 范围。
//!
//! 动机：`generic_lot` 用 `lerp(lotBaseUVMin, lotBaseUVMax, frac(baseUV))` 采样底图，
//! 其中 `baseUV = In.texcoord0`（VS 不缩放）。所以**图集格在地块上重复几次，完全由
//! 地面 mesh 的 UV 跨度决定**——引擎里没有"材质最小单位"常量。此探针把该跨度量出来。
//!
//! 用法：cargo run -p sc-exporter --release --example lot_uv_range -- <package> <model_instance_hex>
use dbpf::Package;
use rw4::{Rw4File, SectionType};

const RW4_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: lot_uv_range <package> <model_instance_hex>");
        std::process::exit(2);
    }
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("bad instance");
    let package = Package::open(&args[0]).unwrap_or_else(|error| panic!("open: {error}"));
    let entry = package
        .entries()
        .iter()
        .find(|entry| entry.id.type_id == RW4_TYPE && entry.id.instance == instance)
        .cloned()
        .unwrap_or_else(|| panic!("model 0x{instance:08X} not found"));
    let data = package.read(&entry).unwrap();
    let file = Rw4File::parse(&data).expect("parse rw4");

    for section in file.sections_of_type(SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else {
            continue;
        };
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        let mut count = 0usize;
        for vertex in &mesh.vertices {
            if let Some(uv) = vertex.uv() {
                for axis in 0..2 {
                    min[axis] = min[axis].min(uv[axis]);
                    max[axis] = max[axis].max(uv[axis]);
                }
                count += 1;
            }
        }
        if count == 0 {
            println!("mesh #{:<4} verts={} UV 缺失", mesh.number, mesh.vertices.len());
            continue;
        }
        println!(
            "mesh #{:<4} verts={:<5} tris={:<5} uv 范围 x [{:.3}, {:.3}] span {:.3} | y [{:.3}, {:.3}] span {:.3}",
            mesh.number,
            mesh.vertices.len(),
            mesh.triangles.len(),
            min[0],
            max[0],
            max[0] - min[0],
            min[1],
            max[1],
            max[1] - min[1],
        );
    }
}
