//! Read-only census of road extrusion links in the installed Game package.
fn main() {
    let p = dbpf::Package::open("D:/ea-games/SimCity/SimCityData/SimCity_Game.package").unwrap();
    let mut counts = [0usize; 3];
    for e in p.entries().iter().filter(|e| e.id.type_id == 0x00b1b104) {
        let Ok(d) = p.read(e) else { continue };
        let Ok(pf) = sc_properties::PropertyFile::parse(&d) else {
            continue;
        };
        for (i, hash) in [0x09532375, 0x09558821, 243988664].iter().enumerate() {
            if let Some(prop) = pf.get(*hash) {
                counts[i] += 1;
                if counts[i] <= 4 {
                    println!(
                        "{:08X}:{:08X} {:08X} {:?}",
                        e.id.group, e.id.instance, hash, prop.kind
                    );
                }
            }
        }
    }
    println!("staticExtrusion/material/stackToGround property counts: {counts:?}");
}
