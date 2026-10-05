//! 探针：逐三角形导出建筑网格的位置/UV/调色板列号（D3DCOLOR.G），
//! 用于「半窗」类逐顶点选列问题的空间对拍。
//!
//! 用法：
//! ```text
//! cargo run -p sc-exporter --release --example building_vertex_cols -- \
//!   <model_package> <model_instance_hex> <mesh_number>
//! ```
//! 输出：tmp/vertex_cols_<instance>.csv（三角形质心 xyz、uv 范围、三顶点列号）。
use dbpf::Package;
use rw4::{Rw4File, SectionType};

const RW4_TYPE: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (Some(pkg_path), Some(inst_hex), Some(mesh_no)) = (
        args.get(1),
        args.get(2),
        args.get(3).and_then(|s| s.parse::<u32>().ok()),
    ) else {
        eprintln!("usage: building_vertex_cols <package> <instance_hex> <mesh_number>");
        std::process::exit(2);
    };
    let instance = u32::from_str_radix(inst_hex.trim_start_matches("0x"), 16).unwrap();
    let pkg = Package::open(pkg_path).unwrap();
    let entry = pkg
        .entries()
        .iter()
        .find(|e| e.id.type_id == RW4_TYPE && e.id.instance == instance)
        .unwrap_or_else(|| panic!("model 0x{instance:08X} not found"));
    let bytes = pkg.read(entry).unwrap();
    let file = Rw4File::parse(&bytes).unwrap();
    let mesh = file.decode_mesh(&bytes, mesh_no).unwrap();
    println!(
        "mesh#{mesh_no} verts={} tris={}",
        mesh.vertices.len(),
        mesh.triangles.len()
    );

    // 列号直方图
    let mut hist = [0u32; 256];
    for v in &mesh.vertices {
        hist[usize::from(v.d3d_color_g().unwrap_or(0))] += 1;
    }
    println!("列号直方图（col: 顶点数）:");
    for (col, count) in hist.iter().enumerate() {
        if *count > 0 {
            println!("  col {col:3}: {count}");
        }
    }

    let mut csv = String::from("tri,x0,y0,z0,x1,y1,z1,x2,y2,z2,u0,v0,u1,v1,u2,v2,w0,z0_,w1,z1_,w2,z2_,col0,col1,col2\n");
    for (i, tri) in mesh.triangles.iter().enumerate() {
        let vs: Vec<_> = tri.iter().map(|&vi| &mesh.vertices[usize::from(vi)]).collect();
        let pos: Vec<[f32; 3]> = vs.iter().map(|v| v.position().unwrap_or([0.0; 3])).collect();
        let uv: Vec<[f32; 2]> = vs.iter().map(|v| v.uv().unwrap_or([0.0; 2])).collect();
        // uv2 = FLOAT4 TexCoord 的 zw（Top 层域，引擎 In.texcoord<t0>.zw）
        let uv2: Vec<[f32; 2]> = vs
            .iter()
            .map(|v| {
                v.components
                    .iter()
                    .find_map(|(e, val)| {
                        (e.usage == rw4::DeclarationUsage::TexCoord)
                            .then_some(())
                            .and_then(|_| match val {
                                rw4::ComponentValue::Float4(f) => Some([f[2], f[3]]),
                                _ => None,
                            })
                    })
                    .unwrap_or([0.0; 2])
            })
            .collect();
        let cols: Vec<u8> = vs.iter().map(|v| v.d3d_color_g().unwrap_or(0)).collect();
        csv.push_str(&format!(
            "{i},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},{},{},{}\n",
            pos[0][0], pos[0][1], pos[0][2],
            pos[1][0], pos[1][1], pos[1][2],
            pos[2][0], pos[2][1], pos[2][2],
            uv[0][0], uv[0][1], uv[1][0], uv[1][1], uv[2][0], uv[2][1],
            uv2[0][0], uv2[0][1], uv2[1][0], uv2[1][1], uv2[2][0], uv2[2][1],
            cols[0], cols[1], cols[2]
        ));
    }
    let out = format!("tmp/vertex_cols_{instance:08X}.csv");
    std::fs::write(&out, csv).unwrap();
    println!("已写出 {out}");
    let _ = SectionType::MESH;
}
