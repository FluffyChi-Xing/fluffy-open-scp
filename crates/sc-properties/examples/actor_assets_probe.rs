//! Read-only investigation of actor parts, animations and embedded textures.
use dbpf::Package;
use rw4::SectionType;
fn main() {
    for path in std::env::args().skip(1) {
        let p = Package::open(path).unwrap();
        for e in p.entries().iter().filter(|e| {
            [
                0x22706EFA, 0x66BE4E6E, 0xE5537FAB, 0x22A445FE, 0x0000F12D, 0x0000F12E, 0x1B3A01EE,
                0x68CD1C10, 0xBE3DCBE1, 0xBE3DCBE3, 0x14C5F13F, 0x69C94C5D,
            ]
            .contains(&e.id.instance)
        }) {
            let data = p.read(e).unwrap();
            println!("{:?}", e.id);
            if e.id.type_id == 0x00B1B104 {
                let f = sc_properties::PropertyFile::parse(&data).unwrap();
                for h in [0x0BD62577, 0x0BD62579] {
                    println!("{h:X} {:?}", sc_properties::actor_graphics::keys(&f, h));
                }
            }
            if e.id.type_id == 0x2F4E681B {
                let f = rw4::Rw4File::parse(&data).unwrap();
                for sec in f.sections_of_type(SectionType::RW4_SKELETON) {
                    let sk = f.decode_skeleton(&data, sec.number).unwrap();
                    println!(
                        "joints={} bind={} inv={}",
                        sk.hierarchy.joints.len(),
                        sk.bind_matrices.len(),
                        sk.matrices_4x3.len()
                    );
                }
                for sec in f.sections_of_type(SectionType::MESH) {
                    let m = f.decode_mesh(&data, sec.number).unwrap();
                    println!("vertex={:?}", m.vertices.first());
                }
                println!(
                    "skeletons={} anims={}",
                    f.sections_of_type(SectionType::RW4_SKELETON).count(),
                    f.sections_of_type(SectionType::ANIM).count()
                );
                for s in f.sections_of_type(SectionType::TEXTURE) {
                    let t = f.decode_texture(&data, s.number).unwrap();
                    println!("texture #{} {}x{}", s.number, t.width, t.height);
                    if let Ok(rgba) = t.decode_top_mip_rgba() {
                        image::save_buffer(
                            format!("tmp/actor-{:08x}-{}.png", e.id.instance, s.number),
                            &rgba,
                            t.width as u32,
                            t.height as u32,
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
}
