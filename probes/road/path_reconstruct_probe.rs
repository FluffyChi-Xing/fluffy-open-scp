//! Reconstruct static road path endpoints from a region unit transform.
use sc_properties::{PropertyFile, Value};
use serde::Deserialize;
#[derive(Deserialize)]
struct State {
    units: Vec<Unit>,
    curves: Vec<Curve>,
}
#[derive(Deserialize)]
struct Unit {
    id: u32,
    rotation: [f32; 9],
    position: [f32; 3],
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Curve {
    entry_id: u32,
    controls: [[f32; 3]; 4],
}
fn main() {
    let root = std::path::PathBuf::from(std::env::args().nth(1).expect("package root"));
    let state: State = serde_json::from_slice(
        &std::fs::read(std::env::args().nth(2).expect("state json")).unwrap(),
    )
    .unwrap();
    let Some(unit) = state.units.iter().find(|u| u.id == 0x30360939) else {
        panic!("unit missing")
    };
    fn collect(d: &std::path::Path, o: &mut Vec<std::path::PathBuf>) {
        if let Ok(es) = std::fs::read_dir(d) {
            for e in es.flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, o)
                } else if p.extension().is_some_and(|x| x == "package") {
                    o.push(p)
                }
            }
        }
    }
    let mut ps = Vec::new();
    collect(&root, &mut ps);
    let mut prop = None;
    'outer: for path in ps {
        let Ok(pkg) = dbpf::Package::open(&path) else {
            continue;
        };
        for e in pkg.entries().iter().filter(|e| {
            e.id.type_id == 0x00B1_B104 && e.id.group == 0x40E1_C000 && e.id.instance == unit.id
        }) {
            if let Ok(b) = pkg.read(e) {
                if let Ok(p) = PropertyFile::parse(&b) {
                    prop = Some(p);
                    break 'outer;
                }
            }
        }
    }
    let p = prop.expect("static property");
    let points = match p.get(0x0CAA680D).and_then(|x| x.array()) {
        Some(v) => v,
        None => panic!("points"),
    };
    let pairs = p.get(0x0CAA6841).and_then(|x| x.array()).unwrap();
    let tans = p.get(0x0CB00ED8).and_then(|x| x.array()).unwrap();
    let entries = p.get(0x0CC8FE7A).and_then(|x| x.array()).unwrap();
    let tx = |v: &Value| match v {
        Value::Vector3(a) => Some(*a),
        _ => None,
    };
    let ti = |v: &Value| match v {
        Value::Int32(a) => Some(*a as usize),
        _ => None,
    };
    let xf = |a: [f32; 3]| {
        let r = &unit.rotation;
        [
            unit.position[0] + unit.rotation[0] * a[0] + r[1] * a[1] + r[2] * a[2],
            unit.position[1] + r[3] * a[0] + r[4] * a[1] + r[5] * a[2],
            unit.position[2] + r[6] * a[0] + r[7] * a[1] + r[8] * a[2],
        ]
    };
    for i in 0..entries.len() {
        let a = ti(&pairs[i * 2]).unwrap();
        let b = ti(&pairs[i * 2 + 1]).unwrap();
        let pa = xf(tx(&points[a]).unwrap());
        let pb = xf(tx(&points[b]).unwrap());
        let ta = tx(&tans[a]).unwrap();
        let tb = tx(&tans[b]).unwrap();
        println!(
            "entry {:08X} local {}..{} world {:?} -> {:?} tan {:?}/{:?}",
            match entries[i] {
                Value::Key(k) => k.instance,
                _ => 0,
            },
            a,
            b,
            pa,
            pb,
            ta,
            tb
        );
    }
    let _ = state.curves;
}
