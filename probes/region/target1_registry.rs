//! Target1: dump Cape Trinity registry ids and locale resource contents.
fn main() {
    let game = dbpf::Package::open(std::env::args().nth(1).expect("game")).unwrap();
    let locale = dbpf::Package::open(std::env::args().nth(2).expect("locale")).unwrap();
    let reg = sc_properties::region_3d::parse_template_registry(&game);
    println!("registry entries={}", reg.len());
    for (name, (rid, cities)) in reg.iter() {
        if *rid == 0x6779_B9D2 || name.to_ascii_lowercase().contains("trinity") {
            println!("template={name} region_id={rid:08X}");
            for (uid, id) in cities {
                println!("  uid={uid} city_id={id:08X}");
            }
        }
    }
    let inst = sc_properties::region_3d::fnv1_lower("automatedregiontemplates");
    println!("locale instance={inst:08X}");
    for e in locale
        .entries()
        .iter()
        .filter(|e| e.id.type_id == 0x0A98_EAF0 && e.id.instance == inst)
    {
        let d = locale.read(e).unwrap();
        println!(
            "locale TGI={:08X}:{:08X}:{:08X} bytes={}",
            e.id.type_id,
            e.id.group,
            e.id.instance,
            d.len()
        );
        let txt = String::from_utf8_lossy(&d);
        for needle in [
            "6779B9D2",
            "FC0C91CF",
            "Norwich",
            "Clearwater",
            "Trinity",
            "清水",
        ] {
            if let Some(i) = txt.find(needle) {
                let a = i.saturating_sub(120);
                let b = (i + 240).min(txt.len());
                println!("needle={needle}: {}", &txt[a..b]);
            }
        }
    }
}
