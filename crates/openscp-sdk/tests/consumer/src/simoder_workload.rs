//! Reproduces Simoder's format call patterns with synthetic original data.
use openscp_sdk::{
    Error, ResourceId, package,
    properties::{self, Kind, PropertyFile, Value},
};

#[test]
fn scalar_patch_locale_overlay_and_memory_preflight() {
    // Same scalar operation exercised by simoder-formats/tests/formats_tests.rs.
    let mut bytes = Vec::from(2u32.to_be_bytes());
    for (hash, ty, value) in [
        (0x09ae19d7u32, 13u16, 300f32.to_bits()),
        (0xdeadbeef, 10, 0x12345678),
    ] {
        bytes.extend(hash.to_be_bytes());
        bytes.extend(ty.to_be_bytes());
        bytes.extend(0u16.to_be_bytes());
        bytes.extend(value.to_be_bytes());
    }
    let mut prop = PropertyFile::parse(&bytes).unwrap();
    prop.values
        .iter_mut()
        .find(|p| p.hash == 0x09ae19d7)
        .unwrap()
        .kind = Kind::Scalar(Value::Float(345.0));
    let encoded = prop.encode_canonical().unwrap();
    let locale = properties::locale::serialize_locale_items(&[properties::locale::LocaleItem {
        key: "0x00000001".into(),
        id: Some(1),
        text: "测试 / Test".into(),
    }])
    .unwrap();
    let target = ResourceId {
        type_id: 0x00b1b104,
        group: 0x61efc000,
        instance: 0x719436bd,
    };
    let locale_id = ResourceId {
        type_id: properties::LOCALE_RESOURCE_TYPE,
        group: 0,
        instance: 42,
    };
    let overlay = package::write_uncompressed_overlay(&[
        package::OverlayEntry::new(target, encoded),
        package::OverlayEntry::new(locale_id, locale),
    ])
    .unwrap();
    let package = package::MemoryPackage::parse_with_limits(
        &overlay,
        package::MemoryParseLimits {
            max_input_bytes: overlay.len(),
            max_entries: 2,
        },
    )
    .unwrap();
    assert_eq!(
        openscp_sdk::resource::checked_id(target.type_id, target.group, u64::from(target.instance))
            .unwrap(),
        target
    );
    // The host, not the format writer, owns package-info admission policy.
    assert_eq!(
        package
            .entries()
            .iter()
            .filter(|e| e.id.type_id == 0x00b1b104 && e.id.group == 0x40404000)
            .count(),
        0
    );
    let entry = package.entry(target).unwrap();
    let decoded = PropertyFile::parse(&package.read(entry, 4096).unwrap()).unwrap();
    assert_eq!(
        decoded.get(0x09ae19d7).unwrap().kind,
        Kind::Scalar(Value::Float(345.0))
    );
    assert_eq!(
        decoded.get(0xdeadbeef).unwrap().kind,
        Kind::Scalar(Value::UInt32(0x12345678))
    );
    let locale = package
        .read(package.entry(locale_id).unwrap(), 4096)
        .unwrap();
    assert_eq!(
        properties::locale::parse_string_table(&locale).unwrap()[&1],
        "测试 / Test"
    );
    let error: Error = package.read(entry, 1).unwrap_err().into();
    assert_eq!(error.code(), "openscp.package.memory");
    let error: Error = package::erz_parse_summary(b"broken").unwrap_err().into();
    assert_eq!(error.code(), "openscp.erz");
}
