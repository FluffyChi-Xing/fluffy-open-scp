//! Verify target1 overlay by reopening it and comparing expected semantic diffs.
use dbpf::Package;
use sc_properties::locale::parse_locale_items;
use sc_properties::{Kind, PropertyFile, Value};
fn main() {
    let mut a = std::env::args().skip(1);
    let out = Package::open(a.next().unwrap()).unwrap();
    let table = out
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x00B1_B104)
        .unwrap();
    let p = PropertyFile::parse(&out.read(table).unwrap()).unwrap();
    let expected = [0x80ED_9B57u32, 0xBF05_4698];
    let ids = match &p.get(0x16B7_B1EF).unwrap().kind {
        Kind::Array(v) => v
            .iter()
            .map(|x| match x {
                Value::UInt32(v) => *v,
                _ => 0,
            })
            .collect::<Vec<_>>(),
        _ => vec![],
    };
    assert_eq!(
        ids, expected,
        "remaining city groups must be Clearwater/Norwich"
    );
    for hash in [
        0x0543_BC96,
        0x16B7_B1EF,
        0x9F2F_9B65,
        0xC0B1_3E7E,
        0xF01D_E4B1,
    ] {
        match &p.get(hash).unwrap().kind {
            Kind::Array(v) => assert_eq!(v.len(), 2),
            _ => panic!("parallel field not array"),
        }
    }
    let loc = out
        .entries()
        .iter()
        .find(|e| e.id.type_id == 0x0A98_EAF0)
        .unwrap();
    let items = parse_locale_items(&out.read(loc).unwrap()).unwrap();
    assert_eq!(
        items
            .iter()
            .find(|x| x.id == Some(0xFC0C_91CF))
            .unwrap()
            .text,
        "赫尔辛基根"
    );
    println!(
        "verified overlay entries={} table cities={:?} locale=赫尔辛基根",
        out.entries().len(),
        ids
    );
}
