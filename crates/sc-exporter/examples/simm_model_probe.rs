//! 冒烟工具：GI_Simm_*（市民/顾问小人）instance 在游戏包中的资源形态定性。
//! 对 RW4 模型命中输出 bbox/网格/纹理/材质引用清单。
//! 用法：cargo run -p sc-exporter --release --example simm_model_probe -- <pkg> [...]
use dbpf::Package;
use std::collections::BTreeMap;

fn main() {
    // GI_Simm_*/GI_SimM_* 注册表 instance（docs database_main.s3db Instances 表）
    let names: BTreeMap<u32, &str> = [
        (0xB7B7B92F_u32, "GI_SimM_airtraffcontrol_fa"),
    ]
    .into_iter()
    .collect();
    let _ = names; // 名字映射从 tmp/simm_names.json 读取（构建期无 serde 依赖）
    let Ok(json) = std::fs::read_to_string("tmp/simm_names.json") else {
        panic!("先运行 python 生成 tmp/simm_names.json");
    };
    // 极简 JSON 解析："XXXXXXXX": "name"
    let mut targets: BTreeMap<u32, String> = BTreeMap::new();
    for part in json.split('"').skip(1).step_by(2) {
        if let Ok(id) = u32::from_str_radix(part, 16) {
            let name = json.split('"').skip(1).step_by(2).skip(1).next();
            let _ = name;
            targets.insert(id, String::new());
        }
    }
    // 名字第二遍填（同序）
    let mut names_iter = json.split('"').skip(1).step_by(2);
    let mut pairs: Vec<(String, String)> = Vec::new();
    while let (Some(k), Some(v)) = (names_iter.next(), names_iter.next()) {
        pairs.push((k.to_string(), v.to_string()));
    }
    targets.clear();
    for (k, v) in pairs {
        if let Ok(id) = u32::from_str_radix(&k, 16) {
            targets.insert(id, v);
        }
    }
    println!("注册表目标 {} 个", targets.len());

    for path in std::env::args().skip(1) {
        let package = Package::open(&path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        for e in package.entries().iter() {
            let Some(name) = targets.get(&e.id.instance) else {
                continue;
            };
            let Ok(data) = package.read(e) else { continue };
            print!(
                "{name} @ {} T {:08X} G {:08X} {}B",
                path.rsplit('/').next().unwrap(),
                e.id.type_id,
                e.id.group,
                data.len()
            );
            if e.id.type_id == 0x2F4E_681B {
                if let Ok(file) = rw4::Rw4File::parse(&data) {
                    if let Some(bbox) = file.sections_of_type(rw4::SectionType::BBOX).next() {
                        if let Ok(payload) = file.payload(&data, bbox.number) {
                            if payload.len() >= 24 {
                                let rd = |off: usize| -> f32 {
                                    f32::from_le_bytes(payload[off..off + 4].try_into().unwrap())
                                };
                                print!(
                                    " bbox {:.2}x{:.2}x{:.2}",
                                    rd(12) - rd(0),
                                    rd(16) - rd(4),
                                    rd(20) - rd(8)
                                );
                            }
                        }
                    }
                    let meshes = file.sections_of_type(rw4::SectionType::MESH).count();
                    let textures = file.sections_of_type(rw4::SectionType::TEXTURE).count();
                    print!(" meshes={meshes} textures={textures}");
                    for (i, tex) in file.sections_of_type(rw4::SectionType::TEXTURE).enumerate() {
                        if let Ok(t) = file.decode_texture(&data, tex.number) {
                            print!(" tex{i} {}x{}", t.width, t.height);
                            let out =
                                format!("tmp/simm/tex_{}_{}.png", name, i);
                            let _ = std::fs::create_dir_all("tmp/simm");
                            if let Ok(px) = t.decode_top_mip_rgba() {
                                let img = image::RgbaImage::from_fn(
                                    t.width as u32,
                                    t.height as u32,
                                    |x, y| {
                                        let at = (x as usize + y as usize * t.width as usize) * 4;
                                        image::Rgba([
                                            px[at],
                                            px[at + 1],
                                            px[at + 2],
                                            px[at + 3],
                                        ])
                                    },
                                );
                                let _ = img.save(&out);
                                print!("→{}", out);
                            }
                        }
                    }
                    // GLB 导出第一个可导出 mesh
                    for section in file.sections_of_type(rw4::SectionType::MESH) {
                        if let Ok(mesh) = file.decode_mesh(&data, section.number) {
                            if mesh.is_exportable() {
                                let _ = std::fs::create_dir_all("tmp/simm");
                                let out = sc_exporter::export_glb(&mesh, None, &[]);
                                let p = format!("tmp/simm/{}_mesh{}.glb", name, section.number);
                                let _ = std::fs::write(&p, &out.bytes);
                                print!(" verts={} tris={} →{p}", mesh.vertices.len(), mesh.triangles.len());
                                break;
                            }
                        }
                    }
                }
            }
            println!();
        }
    }
}
