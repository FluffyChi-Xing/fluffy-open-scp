use dbpf::{MemoryPackage, MemoryReadError, OverlayEntry, ResourceId, write_uncompressed_overlay};

fn fixture(data: &[u8]) -> Vec<u8> {
    write_uncompressed_overlay(&[OverlayEntry::new(
        ResourceId {
            type_id: 1,
            group: 2,
            instance: u32::MAX,
        },
        data,
    )])
    .unwrap()
}

#[test]
fn borrowed_stored_bytes_and_budgeted_payload() {
    let bytes = fixture(b"opaque");
    let package = MemoryPackage::parse(&bytes).unwrap();
    let entry = &package.entries()[0];
    assert_eq!(package.header().index_count, 1);
    assert_eq!(package.entry(entry.id).unwrap().id.instance, u32::MAX);
    assert_eq!(
        package.read_raw(entry).unwrap().as_ptr(),
        bytes[entry.offset as usize..].as_ptr()
    );
    assert_eq!(package.read(entry, 6).unwrap(), b"opaque");
    assert!(matches!(
        package.read(entry, 5),
        Err(MemoryReadError::LimitExceeded {
            declared: 6,
            limit: 5,
            ..
        })
    ));
    let mut forged = entry.clone();
    forged.offset = u64::MAX;
    assert!(package.read_raw(&forged).is_err());
    assert!(MemoryPackage::parse(&bytes[..16]).is_err());
}

#[test]
fn compressed_payload_uses_existing_refpack_contract() {
    let compressed = [0x10, 0xfb, 0, 0, 4, 0xe0, 1, 2, 3, 4, 0xfc];
    let mut bytes = fixture(&compressed);
    // DBPF writer uses an 8-byte index preamble and one 28-byte record.
    let record = 96 + 8;
    bytes[record + 20..record + 24].copy_from_slice(&4u32.to_le_bytes());
    bytes[record + 24..record + 26].copy_from_slice(&(-1i16).to_le_bytes());
    let package = MemoryPackage::parse(&bytes).unwrap();
    let entry = &package.entries()[0];
    assert_eq!(package.read_raw(entry).unwrap(), compressed);
    assert_eq!(package.read(entry, 4).unwrap(), [1, 2, 3, 4]);
    assert!(matches!(
        package.read(entry, 3),
        Err(MemoryReadError::LimitExceeded { .. })
    ));
}
