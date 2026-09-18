//! 只读取证：facade mesh 的 TEXCOORD 结构直方图。
//! 输出每 mesh 的 TC 元素格式、TC0/TC1 值域、列索引（TC1.x×255）分布——
//! 用于比对"顶点选了哪些参数表列 → 哪些 tint 区域应该渲染"。
//! 用法：cargo run -p sc-exporter --release --example facade_uv_probe -- <package> <model_instance>
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
    }
}
