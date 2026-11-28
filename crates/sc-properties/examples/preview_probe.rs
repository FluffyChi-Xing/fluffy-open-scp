//! Read-only property diagnostic: preview_probe <package> <hex instance>...
use dbpf::Package;
use sc_properties::{Kind, PropertyFile};
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pkg = Package::open(&args[1]).expect("open package");
    for entry in pkg.entries() {
        if entry.id.type_id != 0x00b1b104
            || !args[2..]
                .iter()
                .any(|s| u32::from_str_radix(s, 16).ok() == Some(entry.id.instance))
        {
            continue;
        }
        println!("{:?}", entry.id);
        let file = PropertyFile::parse(&pkg.read(entry).expect("read")).expect("parse");
        let scene = sc_properties::vignette::parse(&file);
        if scene.recognized {
            println!("SCENE {}", serde_json::to_string(&scene).unwrap());
        }
        for p in &file.values {
            let values = match &p.kind {
                Kind::Array(v) => v.clone(),
                Kind::Scalar(v) => vec![v.clone()],
                Kind::Empty => vec![],
            };
            println!(
                "{:08X} {:?} count={} first={:?}",
                p.hash,
                p.prop_type,
                values.len(),
                &values[..values.len().min(3)]
            );
        }
    }
}
