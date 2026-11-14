//! All vanilla units carrying the 8-value schedule column (0D00853A..41):
//! dump values + W1 + jobs-bundle presence, to infer the column semantics.
use dbpf::Package;
use sc_properties::{Kind, PropertyFile, Value};
fn ints(file: &PropertyFile, hash: u32) -> Vec<i32> {
    file.get(hash)
        .and_then(|p| match &p.kind {
            Kind::Array(vs) => Some(
                vs.iter()
                    .filter_map(|v| match v {
                        Value::Int32(x) => Some(*x),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}
fn main() {
    let dir = std::path::Path::new(
        "D:/ea-games/simcity_offline/SimCity：Cites of Tomorrow/SimCityUserData/EcoGame",
    );
    let mut seen: std::collections::BTreeMap<String, (Vec<i32>, Option<f32>, bool, u32)> =
        Default::default();
    for entry in std::fs::read_dir(dir).expect("dir") {
        let path = entry.expect("e").path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !name.contains("Scripts_") || !name.starts_with("SimCity-") {
            continue;
        }
        let Ok(package) = Package::open(&path) else {
            continue;
        };
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x48E0_C000 {
                continue;
            }
            let Ok(bytes) = package.read(entry) else {
                continue;
            };
            let Ok(file) = PropertyFile::parse(&bytes) else {
                continue;
            };
            let mut schedule = Vec::new();
            for offset in 0..8u32 {
                let v = file.get(0x0D00_853A + offset).and_then(|p| match &p.kind {
                    Kind::Scalar(Value::Int32(x)) => Some(*x),
                    _ => None,
                });
                match v {
                    Some(x) => schedule.push(x),
                    None => {
                        schedule.clear();
                        break;
                    }
                }
            }
            if schedule.len() != 8 {
                continue;
            }
            let w1 = file.get(0x0A79_17C5).and_then(|p| match &p.kind {
                Kind::Scalar(Value::Float(x)) => Some(*x),
                _ => None,
            });
            let mut mounted = false;
            for hash in [0x0A02_09A2u32, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5] {
                if let Some(Kind::Array(vs)) = file.get(hash).map(|p| &p.kind) {
                    for v in vs {
                        if let Value::Key(k) = v {
                            if k.instance == 0xD07C_5A04 {
                                mounted = true;
                            }
                        }
                    }
                }
            }
            let key = format!("{:?}", schedule);
            seen.entry(key).or_insert((schedule, w1, mounted, 0)).3 += 1;
        }
    }
    println!("# distinct schedule patterns: {}", seen.len());
    for (key, (schedule, w1, mounted, count)) in &seen {
        println!("x{count:3} {key}  w1={w1:?} jobs_mounted={mounted}");
    }
}
