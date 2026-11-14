//! 一次性诊断：全包普查原生 RW4 模型文件的真实形状，并与我们编译的模型
//! 对拍。统计每个 0x2F4E681B 文件的 section 构成（mesh/material/纹理/顶点
//! 数组/三角形数组/blob/collision/bbox/骨架）、大小分布、fixup 计数。
//! 用法：`cargo run -p rw4 --release --example rw4_census -- <outDir> <pkg>...`
//! 可选 `SC_CENSUS_SAMPLE=20`：随机抽取 N 个文件把原始字节写到 outDir。

use std::collections::BTreeMap;
use std::io::Write as _;

use dbpf::Package;
use rw4::{Rw4File, SectionType};

struct Shape {
    size: u64,
    meshes: usize,
    materials: usize,
    textures: usize,
    vertex_arrays: usize,
    vertex_formats: usize,
    triangle_arrays: usize,
    blobs: usize,
    collision: usize,
    bbox: usize,
    skeleton: usize,
    bindings: usize,
    fixups: usize,
    total_verts: u64,
    total_tris: u64,
    /// 头部字节 0x64（word 25）——原版形态 0xC0xxxx，我们写死 0x40。
    w25: u32,
    /// section_index_begin（w12）。
    sib: u32,
    /// section_index_end（w17）。
    sie: u32,
}

fn shape_of(data: &[u8]) -> Option<Shape> {
    let file = Rw4File::parse(data).ok()?;
    let mut s = Shape {
        size: data.len() as u64,
        meshes: 0,
        materials: 0,
        textures: 0,
        vertex_arrays: 0,
        vertex_formats: 0,
        triangle_arrays: 0,
        blobs: 0,
        collision: 0,
        bbox: 0,
        skeleton: 0,
        bindings: 0,
        fixups: 0,
        total_verts: 0,
        total_tris: 0,
        w25: u32::from_le_bytes(data[0x64..0x68].try_into().unwrap()),
        sib: u32::from_le_bytes(data[0x30..0x34].try_into().unwrap()),
        sie: u32::from_le_bytes(data[0x44..0x48].try_into().unwrap()),
    };
    for section in file.sections() {
        s.fixups += section.fixups.len();
        match section.type_code {
            SectionType::MESH => s.meshes += 1,
            SectionType::MATERIAL => s.materials += 1,
            SectionType::TEXTURE => s.textures += 1,
            SectionType::VERTEX_ARRAY => s.vertex_arrays += 1,
            SectionType::VERTEX_FORMAT => s.vertex_formats += 1,
            SectionType::TRIANGLE_ARRAY => s.triangle_arrays += 1,
            SectionType::BLOB => s.blobs += 1,
            SectionType::COLLISION_MESH => s.collision += 1,
            SectionType::BBOX => s.bbox += 1,
            SectionType::RW4_SKELETON => s.skeleton += 1,
            _ => {}
        }
    }
    s.bindings = file.decode_mesh_material_bindings(data).len();
    for section in file.sections_of_type(SectionType::MESH) {
        if let Ok(mesh) = file.decode_mesh(data, section.number) {
            s.total_verts += mesh.vertices.len() as u64;
            s.total_tris += mesh.triangles.len() as u64;
        }
    }
    Some(s)
}

fn histogram(values: impl Iterator<Item = usize>) -> BTreeMap<usize, usize> {
    let mut out: BTreeMap<usize, usize> = BTreeMap::new();
    for v in values {
        *out.entry(v).or_default() += 1;
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args.get(1).expect("usage: rw4_census <outDir> <pkg>...");
    let sample: usize = std::env::var("SC_CENSUS_SAMPLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20);

    let mut shapes: Vec<(String, Shape)> = Vec::new();
    for path in &args[2..] {
        let package = Package::open(path)?;
        println!("scanning {} ({} entries)", path, package.entries().len());
        for entry in package.entries() {
            if entry.id.type_id != 0x2F4E_681B {
                continue;
            }
            let data = package.read(entry)?;
            if data.len() < 28 || data[..12] != [137, 82, 87, 52, 119, 51, 50, 0, 13, 10, 26, 10] {
                continue;
            }
            let Some(shape) = shape_of(&data) else {
                println!(
                    "  UNPARSEABLE {:08X}:{:08X} ({} bytes)",
                    entry.id.group,
                    entry.id.instance,
                    data.len()
                );
                continue;
            };
            shapes.push((
                format!(
                    "{}::{:08X}",
                    std::path::Path::new(path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(path),
                    entry.id.instance
                ),
                shape,
            ));
        }
    }

    println!("\n=== census: {} RW4 model files ===", shapes.len());
    println!(
        "mesh/file histogram {:?}",
        histogram(shapes.iter().map(|(_, s)| s.meshes))
    );
    println!(
        "material/file histogram {:?}",
        histogram(shapes.iter().map(|(_, s)| s.materials))
    );
    println!(
        "vertex_format/file {:?}",
        histogram(shapes.iter().map(|(_, s)| s.vertex_formats))
    );
    println!(
        "collision/file {:?}",
        histogram(shapes.iter().map(|(_, s)| s.collision))
    );
    println!(
        "bbox/file {:?}",
        histogram(shapes.iter().map(|(_, s)| s.bbox))
    );
    println!(
        "texture/file {:?}",
        histogram(shapes.iter().map(|(_, s)| s.textures))
    );
    let mut sizes: Vec<u64> = shapes.iter().map(|(_, s)| s.size).collect();
    sizes.sort_unstable();
    if !sizes.is_empty() {
        println!(
            "size KiB: min {:.0} p50 {:.0} p90 {:.0} p99 {:.0} max {:.0}",
            sizes[0] as f64 / 1024.0,
            sizes[sizes.len() / 2] as f64 / 1024.0,
            sizes[sizes.len() * 9 / 10] as f64 / 1024.0,
            sizes[sizes.len() * 99 / 100] as f64 / 1024.0,
            *sizes.last().unwrap() as f64 / 1024.0,
        );
    }
    let multi: Vec<&(String, Shape)> = shapes.iter().filter(|(_, s)| s.meshes > 1).collect();
    println!("\n-- files with >1 mesh: {} --", multi.len());
    for (name, s) in multi.iter().take(10) {
        println!(
            "  {name}: {} meshes, {} mats, {} bytes, fixups {}",
            s.meshes, s.materials, s.size, s.fixups
        );
    }

    // 随机抽样落盘（确定性种子），供字节级对拍。
    let out = std::path::Path::new(out_dir);
    std::fs::create_dir_all(out)?;
    let mut seed = 0x9E3779B9u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut report = std::fs::File::create(out.join("census-report.txt"))?;
    let taken = shapes.len();
    for _ in 0..taken {
        let index = (next() as usize) % shapes.len();
        let (name, shape) = &shapes[index];
        writeln!(
            report,
            "{name}\tsize={}\tmeshes={}\tmats={}\ttextures={}\tva={}\tvf={}\tta={}\tblobs={}\tcollision={}\tbbox={}\tskeleton={}\tbindings={}\tfixups={}\tverts={}\ttris={}\tw25={:#x}\tsib={:#x}\tsie={:#x}",
            shape.size,
            shape.meshes,
            shape.materials,
            shape.textures,
            shape.vertex_arrays,
            shape.vertex_formats,
            shape.triangle_arrays,
            shape.blobs,
            shape.collision,
            shape.bbox,
            shape.skeleton,
            shape.bindings,
            shape.fixups,
            shape.total_verts,
            shape.total_tris,
            shape.w25,
            shape.sib,
            shape.sie
        )?;
    }
    println!(
        "\nsampled {taken} shapes -> census-report.txt (no raw bytes written; use nighthouse_dump for bytes)"
    );
    Ok(())
}
