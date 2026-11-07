//! Find units carrying the jobs bundle (F236BB93/D07C5A04) and dump their
//! bundle tables + job leaves; also dump the jobs bundles' own contents.
//! Usage: ... -- <package>
use dbpf::{Package, ResourceId};
use sc_properties::{Kind, PropertyFile, Value};

fn bundle_instances(file: &PropertyFile) -> Vec<u32> {
    let mut out = Vec::new();
    for hash in [0x0A02_09A2u32, 0x0A02_09A3, 0x0A02_09A4, 0x0A02_09A5] {
        if let Some(Kind::Array(vs)) = file.get(hash).map(|p| &p.kind) {
            for v in vs {
                if let Value::Key(k) = v { out.push(k.instance); }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
fn float_of(file: &PropertyFile, hash: u32) -> Option<f32> {
    let value = file.get(hash)?; 
    match &value.kind {
        Kind::Scalar(Value::Float(x)) => Some(*x),
        Kind::Array(vs) => vs.first().and_then(|v| match v { Value::Float(x) => Some(*x), _ => None }),
        _ => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let package = Package::open(std::path::Path::new(&args[1])).expect("open");
    const JOBS_A: u32 = 0xF236_BB93;
    const JOBS_B: u32 = 0xD07C_5A04;
    let mut found_bundles = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id == 0x00B1_B104 && entry.id.group == 0x40E0_C700
            && (entry.id.instance == JOBS_A || entry.id.instance == JOBS_B) {
            found_bundles.push(entry.id.instance);
        }
    }
    println!("# bundles in 40E0C700 matching: {:?}", found_bundles.iter().map(|b| format!("{b:08X}")).collect::<Vec<_>>());
    let mut hits: Vec<(u32, u32, Vec<u32>)> = Vec::new();
    for entry in package.entries() {
        if entry.id.type_id != 0x00B1_B104 { continue; }
        let Ok(bytes) = package.read(entry) else { continue };
        let Ok(file) = PropertyFile::parse(&bytes) else { continue };
        let bundles = bundle_instances(&file);
        if bundles.iter().any(|b| *b == JOBS_A || *b == JOBS_B) {
            hits.push((entry.id.group, entry.id.instance, bundles));
        }
    }
    println!("# units carrying jobs bundle: {}", hits.len());
    for (group, instance, bundles) in hits.iter().take(12) {
        let w1 = float_of_by(&package, *group, *instance, 0x0A79_17C5);
        println!(
            "{group:08X}:{instance:08X} bundles={} w1={w1:?}",
            bundles.iter().map(|b| format!("{b:08X}")).collect::<Vec<_>>().join(",")
        );
    }
    // 教育类 unit 反查：谁挂了大学/社区大学教育 bundle
    for (name, edu) in [("univ-3256C0A2", 0x3256_C0A2u32), ("college-6C042366", 0x6C04_2366u32)] {
        let mut edu_hits = Vec::new();
        for entry in package.entries() {
            if entry.id.type_id != 0x00B1_B104 || entry.id.group != 0x48E0_C000 { continue; }
            let Ok(bytes) = package.read(entry) else { continue };
            let Ok(file) = PropertyFile::parse(&bytes) else { continue };
            let bundles = bundle_instances(&file);
            if bundles.contains(&edu) {
                edu_hits.push((entry.id.instance, bundles));
            }
        }
        println!("\n# units carrying {name}: {}", edu_hits.len());
        for (instance, bundles) in edu_hits.iter().take(6) {
            let w1 = float_of_by(&package, 0x48E0_C000, *instance, 0x0A79_17C5);
            println!("  {:08X} bundles={} w1={w1:?}", instance,
                bundles.iter().map(|b| format!("{b:08X}")).collect::<Vec<_>>().join(","));
        }
    }
    // contents of the jobs bundles themselves
    for target in [JOBS_A, JOBS_B] {
        if let Some(entry) = package.entry(ResourceId { type_id: 0x00B1_B104, group: 0x40E0_C700, instance: target }) {
            let bytes = package.read(entry).expect("read");
            if let Ok(file) = PropertyFile::parse(&bytes) {
                println!("\njobs bundle {target:08X}: {} properties", file.values.len());
                for property in &file.values {
                    let brief = match &property.kind {
                        Kind::Array(vs) => format!(
                            "array[{}] {} .. {}",
                            vs.len(),
                            vs.first().map(|v| format!("{v:?}")).unwrap_or_default(),
                            vs.last().map(|v| format!("{v:?}")).unwrap_or_default()
                        ),
                        other => format!("{other:?}"),
                    };
                    let brief: String = brief.chars().take(170).collect();
                    println!("  {:08X} {brief}", property.hash);
                }
            }
        } else {
            println!("\njobs bundle {target:08X}: not found in group 40E0C700");
        }
    }
}
fn float_of_by(package: &Package, group: u32, instance: u32, hash: u32) -> Option<f32> {
    let entry = package.entry(ResourceId { type_id: 0x00B1_B104, group, instance })?;
    let bytes = package.read(entry).ok()?;
    let file = PropertyFile::parse(&bytes).ok()?;
    float_of(&file, hash)
}
