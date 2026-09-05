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
