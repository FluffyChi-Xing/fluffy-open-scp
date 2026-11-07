//! Scan for Menu2 with culture category; try broader locations/groups.
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};

const CULTURE: u32 = 0x94E4_12ED;
const VIA: [u32; 4] = [0x3F43_E038, 0x546F_D9CE, 0x0653_C631, 0xE286_FFA5];
const LANDMARKS_TAB: u32 = 0x0CC8_BE51;

fn keys(file: &PropertyFile, hash: u32) -> Vec<u32> {
    file.get(hash)
        .and_then(|p| match &p.kind {
            Kind::Array(vs) => Some(
                vs.iter()
                    .filter_map(|v| match v {
                        Value::Key(k) => Some(k.instance),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

fn main() {
    let packages: Vec<String> = std::env::args().skip(1).collect();
    for path in &packages {
        let Ok(package) = Package::open(std::path::Path::new(path)) else { continue };
        let name = std::path::Path::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let mut menu2 = 0usize;
        let mut with_cat = 0usize;
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x0987_8A01 {
                continue;
            }
            menu2 += 1;
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let cats = keys(&file, 0x0DB9_FC63);
            if cats.contains(&CULTURE) {
                with_cat += 1;
                let unit = keys(&file, 0x0975_695F).first().copied().unwrap_or(0);
                println!("{} menu2={:08X} unit={:08X}", name, entry.id.instance, unit);
            }
        }
        println!("# {}: menu2={} culture_hits={}", name, menu2, with_cat);
    }
}
