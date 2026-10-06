//! 冒烟工具：树 impostor 模型定性——section 清单 + 内嵌纹理落盘 + GLB 导出。
//! 用法：cargo run -p sc-exporter --release --example tree_model_dump -- <instance_hex> <out_dir> <pkg> [...]
use dbpf::Package;

const RW4_MODEL: u32 = 0x2F4E_681B;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let instance = u32::from_str_radix(args[0].trim_start_matches("0x"), 16).expect("hex");
    let out_dir = &args[1];
    std::fs::create_dir_all(out_dir).expect("mkdir");
    'outer: for path in &args[2..] {
        let package = Package::open(path).unwrap_or_else(|e| panic!("open {path}: {e}"));
        let Some(entry) = package
            .entries()
            .iter()
            .find(|e| e.id.instance == instance && e.id.type_id == RW4_MODEL)
            .cloned()
        else {
            continue;
        };
        let data = package.read(&entry).expect("read");
        let file = rw4::Rw4File::parse(&data).expect("parse");
        println!("== I {instance:08X} @ {path} ({}B)", data.len());
        let mut total_verts = 0usize;
        let mut total_tris = 0usize;
        for section in file.sections_of_type(rw4::SectionType::MESH) {
            match file.decode_mesh(&data, section.number) {
                Ok(mesh) => {
                    total_verts += mesh.vertices.len();
                    let tris = mesh.triangles.len();
                    total_tris += tris;
                    println!(
                        "  mesh #{} verts={} tris={} exportable={}",
                        section.number,
                        mesh.vertices.len(),
                        tris,
                        mesh.is_exportable()
                    );
                    if mesh.is_exportable() {
                        let out = sc_exporter::export_glb(&mesh, None, &[]);
                        let p = format!("{out_dir}/tree_{instance:08X}_mesh{}.glb", section.number);
                        std::fs::write(&p, &out.bytes).expect("write");
                        println!("    → {p}");
                    }
                }
                Err(e) => println!("  mesh #{} decode err {e}", section.number),
            }
        }
        for (i, tex) in file.sections_of_type(rw4::SectionType::TEXTURE).enumerate() {
            let Ok(t) = file.decode_texture(&data, tex.number) else { continue };
            let Ok(px) = t.decode_top_mip_rgba() else { continue };
            let (tw, th) = (t.width as usize, t.height as usize);
            let mut a_min = 255u8;
            let mut a_max = 0u8;
            let mut a0 = 0usize;
            for j in 0..(tw * th) {
                let a = px[j * 4 + 3];
                a_min = a_min.min(a);
                a_max = a_max.max(a);
                if a == 0 {
                    a0 += 1;
                }
            }
            println!(
                "  tex #{i} {}x{} a[{}..{}] 透明占比 {:.0}%",
                tw,
                th,
                a_min,
                a_max,
                a0 as f32 / (tw * th) as f32 * 100.0
            );
            let img = image::RgbaImage::from_fn(tw as u32, th as u32, |x, y| {
                let at = (x as usize + y as usize * tw) * 4;
                image::Rgba([px[at], px[at + 1], px[at + 2], px[at + 3]])
            });
            let p = format!("{out_dir}/tex_{instance:08X}_{i}.png");
            let _ = img.save(&p);
            println!("    → {p}");
        }
        println!(
            "  合计 verts={total_verts} tris={total_tris} textures={}",
            file.sections_of_type(rw4::SectionType::TEXTURE).count()
        );
        break 'outer;
    }
}
