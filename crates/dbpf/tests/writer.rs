use dbpf::{OverlayEntry, Package, ResourceId, WriterError, write_uncompressed_overlay};

fn id(t: u32, g: u32, i: u32) -> ResourceId {
    ResourceId {
        type_id: t,
        group: g,
        instance: i,
    }
}

#[test]
fn deterministic_sorted_roundtrip() {
    let entries = vec![
        OverlayEntry::new(id(2, 1, 1), b"two"),
        OverlayEntry::new(id(1, 9, 2), b"one"),
    ];
    let a = write_uncompressed_overlay(&entries).unwrap();
    let b = write_uncompressed_overlay(&entries).unwrap();
    assert_eq!(a, b);
    let path = std::env::temp_dir().join(format!("dbpf-writer-{}.package", std::process::id()));
    std::fs::write(&path, &a).unwrap();
    let package = Package::open(&path).unwrap();
    assert_eq!(package.entries()[0].id, id(1, 9, 2));
    assert_eq!(package.read(&package.entries()[0]).unwrap(), b"one");
    assert_eq!(package.read(&package.entries()[1]).unwrap(), b"two");
    std::fs::remove_file(path).ok();
}

/// 头部必须与零售 SimCity 包同构，否则游戏可能整包忽略。
#[test]
fn header_matches_retail_conventions() {
    let bytes = write_uncompressed_overlay(&[OverlayEntry::new(id(1, 2, 3), b"x")]).unwrap();
    assert_eq!(&bytes[0..4], b"DBPF");
    assert_eq!(
        i32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        3,
        "major version（零售包全部为 3）"
    );
    assert_eq!(i32::from_le_bytes(bytes[8..12].try_into().unwrap()), 0);
    assert_eq!(u32::from_le_bytes(bytes[0x24..0x28].try_into().unwrap()), 1, "索引条数");
    assert_eq!(u32::from_le_bytes(bytes[0x28..0x2C].try_into().unwrap()), 0);
    assert_eq!(
        u32::from_le_bytes(bytes[0x2C..0x30].try_into().unwrap()),
        8 + 28,
        "索引长度"
    );
    assert_eq!(u32::from_le_bytes(bytes[0x3C..0x40].try_into().unwrap()), 3, "保留值");
    assert_eq!(
        u32::from_le_bytes(bytes[0x40..0x44].try_into().unwrap()),
        96,
        "索引偏移"
    );
    // 索引前导：values=4（仅共享 unknown）+ 0，与零售包实测一致
    assert_eq!(i32::from_le_bytes(bytes[96..100].try_into().unwrap()), 4);
    assert_eq!(u32::from_le_bytes(bytes[100..104].try_into().unwrap()), 0);
}

#[test]
fn duplicate_tgi_rejected() {
    let entries = [
        OverlayEntry::new(id(1, 2, 3), b"a"),
        OverlayEntry::new(id(1, 2, 3), b"b"),
    ];
    assert!(matches!(
        write_uncompressed_overlay(&entries),
        Err(WriterError::DuplicateResource(_))
    ));
}
