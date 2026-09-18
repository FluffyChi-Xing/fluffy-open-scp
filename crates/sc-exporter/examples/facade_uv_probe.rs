//! 只读取证：facade mesh 的 TEXCOORD 结构直方图。
//! 输出每 mesh 的 TC 元素格式、TC0/TC1 值域、列索引（TC1.x×255）分布——
//! 用于比对"顶点选了哪些参数表列 → 哪些 tint 区域应该渲染"。
//! 用法：cargo run -p sc-exporter --release --example facade_uv_probe -- <package> <model_instance> [--dump-verts <csv>]
use dbpf::Package;
use rw4::{ComponentValue, DeclarationUsage};
use std::collections::{BTreeMap, BTreeSet};

const MODEL_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let package = Package::open(&args[0]).expect("open package");
    let instance = u32::from_str_radix(args[1].trim_start_matches("0x"), 16).expect("hex");
    let entry = package
        .entries()
        .iter()
        .find(|e| e.id.instance == instance && e.id.type_id == MODEL_TYPE)
        .cloned()
        .expect("model not found");
    let data = package.read(&entry).expect("read");
    let file = rw4::Rw4File::parse(&data).expect("parse");
    let dump_path = args
        .iter()
        .position(|a| a == "--dump-verts")
        .and_then(|i| args.get(i + 1));

    for section in file.sections_of_type(rw4::SectionType::MESH) {
        let Ok(mesh) = file.decode_mesh(&data, section.number) else {
            continue;
        };
        if !mesh.is_exportable() {
            continue;
        }
        println!("mesh #{}: {} verts", section.number, mesh.vertices.len());
        let mut formats: BTreeSet<String> = BTreeSet::new();
        let mut tc0_min = [f32::MAX; 4];
        let mut tc0_max = [f32::MIN; 4];
        let mut tc1_min = [f32::MAX; 4];
        let mut tc1_max = [f32::MIN; 4];
        let mut col_hist: BTreeMap<u32, usize> = BTreeMap::new();
        let mut tc1_ub0: BTreeMap<u32, usize> = BTreeMap::new();
        let mut tc1_ub1: BTreeMap<u32, usize> = BTreeMap::new();
        let mut d3d_g: BTreeMap<u32, usize> = BTreeMap::new();
        let mut d3d_b_max = 0u32;
        let mut float2_min = [f32::MAX; 2];
        let mut float2_max = [f32::MIN; 2];
        for vertex in &mesh.vertices {
            for (element, value) in &vertex.components {
                // Color 通道（D3DCOLOR.G = 逐顶点材质索引，导出器写进
                // TEXCOORD_1.x）——上一版探针只扫 TexCoord 漏掉这里，
                // 得出「mesh 无材质索引」的错误结论，教训记录。
                if element.usage == DeclarationUsage::Color {
                    if let rw4::ComponentValue::D3DColor { g, b, .. } = value {
                        *d3d_g.entry(u32::from(*g)).or_insert(0) += 1;
                        d3d_b_max = d3d_b_max.max(u32::from(*b));
                    } else if let rw4::ComponentValue::UByte4(bytes) = value {
                        *d3d_g.entry(u32::from(bytes[1])).or_insert(0) += 1;
                        d3d_b_max = d3d_b_max.max(u32::from(bytes[2]));
                    }
                    continue;
                }
                if element.usage != DeclarationUsage::TexCoord {
                    continue;
                }
                match value {
                    ComponentValue::Float4(f) => {
                        formats.insert(format!("TC{} FLOAT4", element.index));
                        if element.index == 0 {
                            for k in 0..4 {
                                tc0_min[k] = tc0_min[k].min(f[k]);
                                tc0_max[k] = tc0_max[k].max(f[k]);
                            }
                        } else if element.index == 1 {
                            for k in 0..4 {
                                tc1_min[k] = tc1_min[k].min(f[k]);
                                tc1_max[k] = tc1_max[k].max(f[k]);
                            }
                            let column = (f[0] * 255.0 + 0.5) as u32;
                            *col_hist.entry(column).or_insert(0) += 1;
                        }
                    }
                    ComponentValue::Float2(uv) => {
                        formats.insert(format!("TC{} FLOAT2", element.index));
                        for k in 0..2 {
                            float2_min[k] = float2_min[k].min(uv[k]);
                            float2_max[k] = float2_max[k].max(uv[k]);
                        }
                    }
                    ComponentValue::UByte4(b) => {
                        formats.insert(format!("TC{} UBYTE4", element.index));
                        if element.index == 1 {
                            *tc1_ub0.entry(u32::from(b[0])).or_insert(0) += 1;
                            *tc1_ub1.entry(u32::from(b[1])).or_insert(0) += 1;
                        }
                    }
                    ComponentValue::D3DColor { r, g, b, a } => {
                        formats.insert("D3DCOLOR".to_string());
                        *d3d_g.entry(u32::from(*g)).or_insert(0) += 1;
                        d3d_b_max = d3d_b_max.max(u32::from(*b));
                    }
                    _ => {}
                }
            }
        }
        println!("  formats: {:?}", formats);
        println!("  TC0 min {tc0_min:?} max {tc0_max:?}");
        println!("  TC1 min {tc1_min:?} max {tc1_max:?}");
        if float2_min[0] != f32::MAX {
            println!("  FLOAT2 min {float2_min:?} max {float2_max:?}");
        }
        println!("  column histogram (TC1.x*255): {col_hist:?}");
        println!("  TC1 UBYTE4 .b0 histogram: {tc1_ub0:?}");
        println!("  TC1 UBYTE4 .b1 histogram: {tc1_ub1:?}");
        println!("  D3DCOLOR.G histogram: {d3d_g:?}");
        println!("  D3DCOLOR.B max: {d3d_b_max}");

        // 逐顶点 CSV（x,y,z,matcol,tc0.xy,tc0.zw）：空间定位窗扇顶点用。
        if let Some(path) = dump_path {
            use std::io::Write;
            let mut out = std::io::BufWriter::new(std::fs::File::create(path).expect("create"));
            writeln!(out, "x,y,z,col,tc0x,tc0y,tc0z,tc0w").unwrap();
            for vertex in &mesh.vertices {
                let (mut px, mut py, mut pz) = (f32::NAN, f32::NAN, f32::NAN);
                let mut tc0 = [0f32; 4];
                let mut col = -1i32;
                for (element, value) in &vertex.components {
                    match (element.usage, value) {
                        (DeclarationUsage::Position, ComponentValue::Float3(p)) => {
                            (px, py, pz) = (p[0], p[1], p[2]);
                        }
                        (DeclarationUsage::TexCoord, ComponentValue::Float4(f)) => {
                            tc0 = *f;
                        }
                        (DeclarationUsage::Color, ComponentValue::D3DColor { g, .. }) => {
                            col = i32::from(*g);
                        }
                        (DeclarationUsage::Color, ComponentValue::UByte4(bytes)) => {
                            col = i32::from(bytes[1]);
                        }
                        _ => {}
                    }
                }
                writeln!(out, "{px},{py},{pz},{col},{},{},{},{}", tc0[0], tc0[1], tc0[2], tc0[3]).unwrap();
            }
            println!("  vertex dump -> {path}");
            // 退化三角形诊断：真填充应为 (a,a,a)；若大量 (x,x,y) 形态
            // 出现则索引解码有问题
            let mut tri_aaa = 0usize;
            let mut tri_two_eq = 0usize;
            let mut tri_samples: Vec<[u16; 3]> = Vec::new();
            for t in &mesh.triangles {
                let deg = t[0] == t[1] || t[1] == t[2] || t[0] == t[2];
                if !deg {
                    continue;
                }
                if t[0] == t[1] && t[1] == t[2] {
                    tri_aaa += 1;
                } else {
                    tri_two_eq += 1;
                    if tri_samples.len() < 12 {
                        tri_samples.push(*t);
                    }
                }
            }
            println!(
                "  degenerate: total={} (a,a,a)={} two-equal={} samples={:?}",
                mesh.triangles.len() - (mesh.triangles.len() - tri_aaa - tri_two_eq),
                tri_aaa,
                tri_two_eq,
                tri_samples
            );
            // 三角形 CSV（含三顶点列号）：跨列三角形 = 列插值伪影来源
            let tris_path = path.replace(".csv", "_tris.csv");
            let mut tout =
                std::io::BufWriter::new(std::fs::File::create(&tris_path).expect("create"));
            writeln!(tout, "a,b,c,col_a,col_b,col_c").unwrap();
            let mut mixed = 0usize;
            let mut total = 0usize;
            for t in &mesh.triangles {
                if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
                    continue;
                }
                total += 1;
                let col_of = |vi: u16| -> i32 {
                    let vertex = &mesh.vertices[vi as usize];
                    for (element, value) in &vertex.components {
                        if element.usage == DeclarationUsage::Color {
                            match value {
                                rw4::ComponentValue::D3DColor { g, .. } => {
                                    return i32::from(*g);
                                }
                                rw4::ComponentValue::UByte4(bytes) => {
                                    return i32::from(bytes[1]);
                                }
                                _ => {}
                            }
                        }
                    }
                    -1
                };
                let (ca, cb, cc) = (col_of(t[0]), col_of(t[1]), col_of(t[2]));
                if ca != cb || cb != cc {
                    mixed += 1;
                }
                writeln!(tout, "{},{},{},{},{},{}", t[0], t[1], t[2], ca, cb, cc).unwrap();
            }
            println!(
                "  triangles: {total} ({mixed} mixed-column) -> {tris_path}"
            );
            // 参数表全量 CSV（col,row0..row3）：软渲染复现用
            let params_path = path.replace(".csv", "_params.csv");
            let params = file
                .sections_of_type(rw4::SectionType::TEXTURE)
                .next()
                .and_then(|s| file.decode_texture(&data, s.number).ok())
                .filter(|t| t.texture_type == rw4::TEXTURE_TYPE_PALETTE_F32)
                .and_then(|t| t.decode_palette_f32().ok());
            if let Some(values) = params {
                let pw = values.len() / 4;
                let mut pout = std::io::BufWriter::new(
                    std::fs::File::create(&params_path).expect("create"),
                );
                writeln!(pout, "col,row,row0,row1,row2,row3").unwrap();
                for c in 0..pw {
                    for r in 0..4 {
                        let v = values[r * pw + c];
                        writeln!(pout, "{c},{r},{},{},{},{}", v[0], v[1], v[2], v[3]).unwrap();
                    }
                }
                println!("  params dump -> {params_path}");
            }
        }
    }
}
