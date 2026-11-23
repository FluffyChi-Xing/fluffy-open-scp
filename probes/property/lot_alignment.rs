//! Read-only lot coordinate evidence: parent properties and model geometry bounds.
use sc_properties::{PropertyFile, Value};
fn main() {
    let root = std::env::args().nth(1).expect("SimCityData");
    let packages: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            if !p.file_name()?.to_str()?.starts_with("SimCity") || p.extension()? != "package" {
                return None;
            }
            Some(dbpf::Package::open(p).ok()?)
        })
        .collect();
    for (group, instance) in [(0x41e1c000, 0xb8415838), (0x56f0c000, 0x3eab97c1)] {
        let mut key = sc_properties::Key {
            type_id: 0x00b1b104,
            group,
            instance,
        };
        for _ in 0..12 {
            let found = packages.iter().find_map(|p| {
                p.entries()
                    .iter()
                    .find(|e| {
                        e.id.type_id == key.type_id
                            && e.id.group == key.group
                            && e.id.instance == key.instance
                    })
                    .map(|e| (p, e))
            });
            let Some((pkg, entry)) = found else {
                println!("missing {key:?}");
                break;
            };
            let file = PropertyFile::parse(&pkg.read(entry).unwrap()).unwrap();
            if key.instance == instance {
                let flat = sc_properties::flatten_parent_inheritance(file.clone(), |k| {
                    std::iter::once(pkg).chain(packages.iter()).find_map(|p| {
                        let e = p
                            .entries()
                            .iter()
                            .find(|e| e.id.type_id == 0x00b1b104 && e.id.instance == k.instance)?;
                        PropertyFile::parse(&p.read(e).ok()?).ok()
                    })
                });
                println!("CURRENT RESOLVER EFFECTIVE");
                if let Some(Value::Key(k)) = flat.get(0x0ccb7fd5).and_then(|p| p.scalar()) {
                    for pack in &packages {
                        if let Some(e) = pack
                            .entries()
                            .iter()
                            .find(|e| e.id.type_id == 0x2f4e681c && e.id.instance == k.instance)
                        {
                            let raster = rw4::RasterImage::parse(&pack.read(e).unwrap()).unwrap();
                            println!("MASK {:08X} {}x{}", k.instance, raster.width, raster.height);
                            let rgba = raster
                                .decode_lot_mask_rgba(&[
                                    [155, 140, 112, 255],
                                    [111, 137, 117, 255],
                                    [133, 108, 54, 255],
                                    [83, 83, 83, 255],
                                ])
                                .unwrap();
                            std::fs::create_dir_all("tmp/lot-alignment").unwrap();
                            image::save_buffer(
                                format!("tmp/lot-alignment/{instance:08X}.png"),
                                &rgba,
                                raster.width,
                                raster.height,
                                image::ColorType::Rgba8,
                            )
                            .unwrap();
                        }
                    }
                }
                for h in [
                    0x00f9efba, 0x0ccb7fc8, 0x0ccb7fc9, 0x0db7fb17, 0x0bf76a29, 0x0bf76a2a,
                ] {
                    if let Some(p) = flat.get(h) {
                        println!("{h:08X} {:?}", p.kind);
                    }
                }
            }
            println!("{} {}", pkg.path().display(), entry.id);
            for h in [0x00f9efba, 0x0ccb7fc8, 0x0ccb7fc9, 0x0db7fb17] {
                if let Some(p) = file.get(h) {
                    println!("{h:08X} {:?}", p.kind);
                }
            }
            if let Some(Value::Key(model)) = file.get(0x00f9efbb).and_then(|p| p.scalar()) {
                for pkg in &packages {
                    for e in pkg.entries().iter().filter(|e| {
                        e.id.type_id == model.type_id && e.id.instance == model.instance
                    }) {
                        let bytes = pkg.read(e).unwrap();
                        let rw = rw4::Rw4File::parse(&bytes).unwrap();
                        println!(
                            "sections {:?}",
                            rw.sections()
                                .iter()
                                .map(|s| (s.number, rw4::SectionType::name(s.type_code)))
                                .collect::<Vec<_>>()
                        );
                        let mut min = [f32::INFINITY; 3];
                        let mut max = [f32::NEG_INFINITY; 3];
                        let mut triangles = Vec::new();
                        for s in rw.sections_of_type(rw4::SectionType::MESH) {
                            let m = rw.decode_mesh(&bytes, s.number).unwrap();
                            for tri in &m.triangles {
                                triangles.push(
                                    tri.iter()
                                        .map(|i| m.vertices[*i as usize].position().unwrap())
                                        .collect::<Vec<_>>(),
                                );
                            }
                        }
                        std::fs::write(
                            format!("tmp/lot-alignment/{instance:08X}.json"),
                            serde_json::to_vec(&triangles).unwrap(),
                        )
                        .unwrap();
                        let mut overlay =
                            image::open(format!("tmp/lot-alignment/{instance:08X}.png"))
                                .unwrap()
                                .resize_exact(768, 768, image::imageops::FilterType::Nearest)
                                .to_rgba8();
                        for tri in &triangles {
                            for edge in 0..3 {
                                let a = tri[edge];
                                let b = tri[(edge + 1) % 3];
                                let a = [(a[0] / 96.0 + 0.5) * 768.0, (0.5 - a[1] / 96.0) * 768.0];
                                let b = [(b[0] / 96.0 + 0.5) * 768.0, (0.5 - b[1] / 96.0) * 768.0];
                                let n =
                                    ((b[0] - a[0]).abs().max((b[1] - a[1]).abs()) as usize).max(1);
                                for t in 0..=n {
                                    let f = t as f32 / n as f32;
                                    let x = (a[0] + (b[0] - a[0]) * f) as i32;
                                    let y = (a[1] + (b[1] - a[1]) * f) as i32;
                                    if x >= 0 && y >= 0 && x < 768 && y < 768 {
                                        overlay.put_pixel(
                                            x as u32,
                                            y as u32,
                                            image::Rgba([240, 90, 70, 255]),
                                        );
                                    }
                                }
                            }
                        }
                        overlay
                            .save(format!("tmp/lot-alignment/{instance:08X}-overlay.png"))
                            .unwrap();
                        for s in rw.sections_of_type(rw4::SectionType::MESH) {
                            let m = rw.decode_mesh(&bytes, s.number).unwrap();
                            for v in m.vertices {
                                if let Some(p) = v.position() {
                                    for i in 0..3 {
                                        min[i] = min[i].min(p[i]);
                                        max[i] = max[i].max(p[i]);
                                    }
                                }
                            }
                        }
                        println!("model {:08X} geometry {min:?} {max:?}", model.instance);
                    }
                }
            }
            let Some(Value::Key(parent)) = file.get(0x00b2cccb).and_then(|p| p.scalar()) else {
                break;
            };
            key = parent.clone();
        }
    }
}
