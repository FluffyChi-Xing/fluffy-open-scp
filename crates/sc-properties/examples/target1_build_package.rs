//! Build the first real target1 Cape Trinity overlay from retail resources.
//! Usage: target1_build_package <rt0> <locale> <out.package> <manifest.json>
use dbpf::{OverlayEntry, Package, ResourceId};
use sc_properties::locale::{parse_locale_items, serialize_locale_items};
use sc_properties::{Kind, PropertyFile};

const PROP_TYPE: u32 = 0x00B1_B104;
const REGION: u32 = 0xC2A9_C48F;
const TABLE: u32 = 0xF0E2_E39C;
const TABLE_INSTANCE: u32 = 0x2B9C_480C;
const LOCALE_TYPE: u32 = 0x0A98_EAF0;
const LOCALE_GROUP: u32 = 0x02FA_BF01;
const LOCALE_INSTANCE: u32 = 0x2C1D_9BDE;
const CLEARWATER_ID: u32 = 0xFC0C_91CF;

fn main() {
    let mut a = std::env::args().skip(1);
    let rt0 = Package::open(a.next().expect("rt0")).unwrap();
    let locale = Package::open(a.next().expect("locale")).unwrap();
    let out_path = a.next().expect("out package");
    let manifest_path = a.next().expect("manifest");
    let table_entry = rt0
        .entries()
        .iter()
        .find(|e| {
            e.id.type_id == PROP_TYPE && e.id.group == TABLE && e.id.instance == TABLE_INSTANCE
        })
        .expect("Cape Trinity plot table");
    let table_before = rt0.read(table_entry).unwrap();
    let mut table = PropertyFile::parse(&table_before).unwrap();
    let before = table.clone();
    for hash in [
        0x0543_BC96,
        0x16B7_B1EF,
        0x9F2F_9B65,
        0xC0B1_3E7E,
        0xF01D_E4B1,
    ] {
        let p = table
            .values
            .iter_mut()
            .find(|p| p.hash == hash)
            .expect("parallel table field");
        match &mut p.kind {
            Kind::Array(v) => {
                assert_eq!(v.len(), 3);
                v.remove(0);
            }
            _ => panic!("table field is not an array"),
        }
    }
    let table_after = table.encode_canonical().unwrap();

    let locale_entry = locale
        .entries()
        .iter()
        .find(|e| {
            e.id.type_id == LOCALE_TYPE
                && e.id.group == LOCALE_GROUP
                && e.id.instance == LOCALE_INSTANCE
        })
        .expect("automatedregiontemplates locale table");
    let locale_before = locale.read(locale_entry).unwrap();
    let mut items = parse_locale_items(&locale_before).unwrap();
    let item = items
        .iter_mut()
        .find(|x| x.id == Some(CLEARWATER_ID))
        .expect("Clearwater locale id");
    assert_eq!(item.text, "清水");
    item.text = "赫尔辛基根".to_string();
    let locale_after = serialize_locale_items(&items).unwrap();

    let entries = vec![
        OverlayEntry::new(
            ResourceId {
                type_id: PROP_TYPE,
                group: TABLE,
                instance: TABLE_INSTANCE,
            },
            table_after.clone(),
        ),
        OverlayEntry::new(
            ResourceId {
                type_id: LOCALE_TYPE,
                group: LOCALE_GROUP,
                instance: LOCALE_INSTANCE,
            },
            locale_after.clone(),
        ),
    ];
    std::fs::write(
        &out_path,
        dbpf::write_uncompressed_overlay(&entries).unwrap(),
    )
    .unwrap();
    let manifest = serde_json::json!({
        "target": "cape-trinity-clearwater-helsinki-root",
        "regionGroup": format!("{REGION:08X}"),
        "hiddenCity": {"uid":"1026", "nameEn":"Trinity Point", "cityGroup":"C8156BE2"},
        "renamedCity": {"uid":"1027", "nameEn":"Clearwater", "stringId":format!("{CLEARWATER_ID:08X}"), "text":"赫尔辛基根"},
        "entries": [
            {"tgi":"00B1B104:F0E2E39C:2B9C480C", "sourceBytes":table_before.len(), "replacementBytes":table_after.len(), "removedIndex":0},
            {"tgi":"0A98EAF0:02FABF01:2C1D9BDE", "sourceBytes":locale_before.len(), "replacementBytes":locale_after.len(), "changedStringId":format!("{CLEARWATER_ID:08X}")}
        ],
        "tableBefore": summarize(&before),
        "tableAfter": summarize(&table),
    });
    std::fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    eprintln!("wrote {} with {} entries", out_path, entries.len());
}

fn summarize(p: &PropertyFile) -> serde_json::Value {
    let mut o = serde_json::Map::new();
    for x in &p.values {
        if let Kind::Array(v) = &x.kind {
            o.insert(format!("{:08X}", x.hash), serde_json::json!({"len":v.len(),"values":v.iter().map(|x| format!("{x}")).collect::<Vec<_>>() }));
        }
    }
    serde_json::Value::Object(o)
}
