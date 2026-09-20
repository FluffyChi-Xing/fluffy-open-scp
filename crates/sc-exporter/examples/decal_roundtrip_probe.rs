//! 探针：对真实包内的 Decal Atlas property 做写回闭环验证。
//!
//! parse → upsert(追加+替换) → encode_canonical → parse 全链，
//! 实证真实文件的 encoding flags / item_size 与 encoder 兼容。
//!
//! 用法：
//! ```text
//! cargo run -p sc-exporter --release --example decal_roundtrip_probe -- <package>
//! ```

use dbpf::Package;
use sc_properties::{DecalEntryUpsert, Key, upsert_entry, PROPERTY_RESOURCE_TYPE,
    is_decal_dictionary_group};

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: decal_roundtrip_probe <package>");
    let package = Package::open(&path).expect("open package");

    let mut checked = 0usize;
    for entry in package.entries().iter() {
        if entry.id.type_id != PROPERTY_RESOURCE_TYPE
            || !is_decal_dictionary_group(entry.id.group)
        {
            continue;
        }
        let data = match package.read(entry) {
            Ok(data) => data,
            Err(_) => continue,
        };
        let mut file = match sc_properties::PropertyFile::parse(&data) {
            Ok(file) => file,
            Err(error) => {
                println!("SKIP {:08X}: parse failed: {error}", entry.id.instance);
                continue;
            }
        };
        let before = sc_properties::DecalDictionary::from_file(&file);
        println!(
            "atlas {:08X} group {:04X}: {} entries (uniform={})",
            entry.id.instance,
            entry.id.group as u16,
            before.entries.len(),
            before.uniform_arrays
        );

        // 1) 追加一条新条目。
        let new_id = 0xdead_beefu32;
        let upsert = DecalEntryUpsert {
            id: Key {
                instance: new_id,
                type_id: 0,
                group: 0,
            },
            raster: Key {
                instance: 0x1234_5678,
                type_id: 0x2F4E_681C,
                group: 0,
            },
            aspect_ratio: 1.25,
            colors: [
                [0.5, 0.25, 0.125, 0.0],
                [0.125, 0.5, 0.25, 0.0],
                [0.25, 0.125, 0.5, 0.0],
                [0.5, 0.5, 0.5, 0.0],
            ],
        };
        let appended =
            upsert_entry(&mut file, &upsert, None).expect("upsert append");
        let encoded = file.encode_canonical().expect("encode after append");
        let reparsed =
            sc_properties::DecalDictionary::parse(&encoded).expect("reparse after append");
        assert_eq!(reparsed.entries.len(), before.entries.len() + 1);
        let added = &reparsed.entries[appended];
        assert_eq!(added.id.as_ref().map(|key| key.instance), Some(new_id));
        assert_eq!(added.aspect_ratio, Some(1.25));
        assert!(added.colors.iter().all(Option::is_some));
        // 旧条目原样保留。
        for (index, original) in before.entries.iter().enumerate() {
            assert_eq!(reparsed.entries[index], *original, "entry {index} changed");
        }
        println!("  append OK: index {appended}, {} entries", reparsed.entries.len());

        // 2) 按 ID 替换刚写入的条目。
        let mut replaced = upsert.clone();
        replaced.aspect_ratio = 0.75;
        let at = upsert_entry(&mut file, &replaced, Some(new_id)).expect("upsert replace");
        let encoded = file.encode_canonical().expect("encode after replace");
        let reparsed =
            sc_properties::DecalDictionary::parse(&encoded).expect("reparse after replace");
        assert_eq!(reparsed.entries.len(), before.entries.len() + 1);
        assert_eq!(reparsed.entries[at].aspect_ratio, Some(0.75));
        println!("  replace OK: index {at}");

        // 3) 二次编码稳定性：canonical 产物再编码应逐字节一致。
        let file2 =
            sc_properties::PropertyFile::parse(&encoded).expect("parse canonical");
        let encoded2 = file2.encode_canonical().expect("re-encode");
        assert_eq!(encoded, encoded2, "canonical encoding not stable");
        println!("  canonical-stable OK ({} bytes)", encoded.len());

        checked += 1;
        if checked >= 3 {
            break;
        }
    }
    if checked == 0 {
        println!("no decal atlas in this package");
    } else {
        println!("ALL ROUND-TRIPS PASSED on {checked} real atlas(es)");
    }
}
