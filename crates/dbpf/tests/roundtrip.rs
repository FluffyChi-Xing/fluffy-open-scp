//! 合成 DBPF/DBBF fixture 的往返测试。
//!
//! 真实游戏包的差分测试需要本机有 SimCity 数据（见 docs/roadmap/migration.md
//! M1 验收标准）；这里用手工编码的字节流先锁死格式行为。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use dbpf::{CachedPackage, Error, Package, PackageKind};

const SHARED_UNKNOWN: u32 = 0x00DE_AD00;

struct TEntry {
    type_id: u32,
    group: u32,
    instance: u32,
    compressed_flags: i16,
    decompressed_size: u32,
    payload: Vec<u8>,
}

fn stored(type_id: u32, group: u32, instance: u32, payload: &[u8]) -> TEntry {
    TEntry {
        type_id,
        group,
        instance,
        compressed_flags: 0,
        decompressed_size: payload.len() as u32,
        payload: payload.to_vec(),
    }
}

fn build_package(kind: PackageKind, values: i32, entries: &[TEntry]) -> Vec<u8> {
    let header_len = match kind {
        PackageKind::Dbpf => 96,
        PackageKind::Dbbf => 120,
    };

    let mut idx: Vec<u8> = Vec::new();
    idx.extend_from_slice(&values.to_le_bytes());
    if values & 1 != 0 {
        idx.extend_from_slice(&entries[0].type_id.to_le_bytes());
    }
    if values & 2 != 0 {
        idx.extend_from_slice(&entries[0].group.to_le_bytes());
    }
    if values & 4 != 0 {
        idx.extend_from_slice(&SHARED_UNKNOWN.to_le_bytes());
    }

    let mut record_len = 4 + 4 + 4 + 4 + 2 + 2; // instance, offset, csize, dsize, flags16, flags
    if values & 1 == 0 {
        record_len += 4;
    }
    if values & 2 == 0 {
        record_len += 4;
    }
    if values & 4 == 0 {
        record_len += 4;
    }
    if kind == PackageKind::Dbbf {
        record_len += 4;
    } // i64 offset

    let records_len = record_len * entries.len();
    let mut payload_base = (header_len + idx.len() + records_len) as u64;

    let mut offsets = Vec::with_capacity(entries.len());
    for e in entries {
        offsets.push(payload_base);
        payload_base += e.payload.len() as u64;
    }

    for (i, e) in entries.iter().enumerate() {
        if values & 1 == 0 {
            idx.extend_from_slice(&e.type_id.to_le_bytes());
        }
        if values & 2 == 0 {
            idx.extend_from_slice(&e.group.to_le_bytes());
        }
        if values & 4 == 0 {
            idx.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        }
        idx.extend_from_slice(&e.instance.to_le_bytes());
        match kind {
            PackageKind::Dbbf => idx.extend_from_slice(&(offsets[i] as i64).to_le_bytes()),
            PackageKind::Dbpf => idx.extend_from_slice(&(offsets[i] as u32).to_le_bytes()),
        }
        idx.extend_from_slice(&(e.payload.len() as u32).to_le_bytes());
        idx.extend_from_slice(&e.decompressed_size.to_le_bytes());
        idx.extend_from_slice(&e.compressed_flags.to_le_bytes());
        idx.extend_from_slice(&0u16.to_le_bytes());
    }

    let mut file: Vec<u8> = Vec::new();
    match kind {
        PackageKind::Dbpf => {
            file.extend_from_slice(b"DBPF");
            file.extend_from_slice(&1i32.to_le_bytes()); // major
            file.extend_from_slice(&0i32.to_le_bytes()); // minor
            file.extend_from_slice(&[0u8; 24]); // 0x0C..0x24
            file.extend_from_slice(&(entries.len() as u32).to_le_bytes()); // 0x24 count
            file.extend_from_slice(&0u32.to_le_bytes()); // 0x28
            file.extend_from_slice(&(idx.len() as u32).to_le_bytes()); // 0x2C index size
            file.extend_from_slice(&[0u8; 12]); // 0x30..0x3C
            file.extend_from_slice(&3u32.to_le_bytes()); // 0x3C always3
            file.extend_from_slice(&(header_len as u32).to_le_bytes()); // 0x40 index offset
            file.extend_from_slice(&[0u8; 28]); // 0x44..0x60
            assert_eq!(file.len(), 96);
        }
        PackageKind::Dbbf => {
            file.extend_from_slice(b"DBBF");
            file.extend_from_slice(&1i32.to_le_bytes());
            file.extend_from_slice(&0i32.to_le_bytes());
            file.extend_from_slice(&[0u8; 20]); // 0x0C..0x20
            file.extend_from_slice(&7u32.to_le_bytes()); // 0x20 index major
            file.extend_from_slice(&(entries.len() as u32).to_le_bytes()); // 0x24
            file.extend_from_slice(&(idx.len() as i64).to_le_bytes()); // 0x28 index size
            file.extend_from_slice(&0u32.to_le_bytes()); // 0x30
            file.extend_from_slice(&3u32.to_le_bytes()); // 0x34 always3
            file.extend_from_slice(&(header_len as i64).to_le_bytes()); // 0x38 index offset
            file.extend_from_slice(&[0u8; 56]); // 0x40..0x78
            assert_eq!(file.len(), 120);
        }
    }

    file.extend_from_slice(&idx);
    for e in entries {
        file.extend_from_slice(&e.payload);
    }
    file
}

static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let id = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("dbpf-test-{}-{}.package", id, name));
    std::fs::write(&path, bytes).expect("write fixture");
    path
}

fn refpack_blob(inner: &[u8], decompressed_size: u32) -> TEntry {
    // 0x10FB + 3-byte size + payload + 0xFC stop
    let mut payload = vec![0x10, 0xFB];
    let size = inner.len() as u32;
    payload.extend_from_slice(&[(size >> 16) as u8, (size >> 8) as u8, size as u8]);
    payload.extend_from_slice(inner);
    payload.push(0xFC);
    TEntry {
        type_id: 0x2f4e_681b,
        group: 0x0100_0001,
        instance: 0x0000_0042,
        compressed_flags: -1,
        decompressed_size,
        payload,
    }
}

#[test]
fn dbpf_stored_roundtrip() {
    let file = build_package(
        PackageKind::Dbpf,
        4, // bit2: shared unknown
        &[
            stored(0x0000_0001, 0x0000_00AA, 0x0000_0011, b"hello"),
            stored(0x0000_0002, 0x0000_00AA, 0x0000_0022, b"world!!"),
        ],
    );
    let path = write_temp("stored", &file);
    let package = Package::open(&path).unwrap();

    assert_eq!(package.header().kind, PackageKind::Dbpf);
    assert_eq!(package.entries().len(), 2);

    let e = &package.entries()[0];
    assert_eq!(e.id.type_id, 1);
    assert_eq!(e.id.group, 0xAA);
    assert_eq!(e.id.instance, 0x11);
    assert_eq!(e.unknown, SHARED_UNKNOWN);
    assert!(!e.compressed);
    assert_eq!(package.read(e).unwrap(), b"hello");

    assert_eq!(package.read(&package.entries()[1]).unwrap(), b"world!!");

    let by_tgi = package.entry(dbpf::ResourceId {
        type_id: 2,
        group: 0xAA,
        instance: 0x22,
    });
    assert!(by_tgi.is_some());
    std::fs::remove_file(&path).ok();
}

#[test]
fn dbpf_compressed_roundtrip_and_cache() {
    // prefix 0x06: 2 literals "AB", copy 4 bytes from offset 2 -> "ABABAB"
    let entry = refpack_blob(&[0x06, 0x01, b'A', b'B'], 6);
    let file = build_package(PackageKind::Dbpf, 4, &[entry]);
    let path = write_temp("compressed", &file);
    let package = Package::open(&path).unwrap();

    let e = &package.entries()[0];
    assert!(e.compressed);
    assert_eq!(e.decompressed_size, 6);
    assert_eq!(package.read(e).unwrap(), b"ABABAB");

    let cached = CachedPackage::new(Package::open(&path).unwrap(), 8);
    let first = cached.read(e).unwrap();
    let second = cached.read(e).unwrap();
    assert_eq!(&*first, b"ABABAB");
    assert!(Arc::ptr_eq(&first, &second), "repeat read should hit cache");
    std::fs::remove_file(&path).ok();
}

#[test]
fn shared_tgi_entries() {
    let file = build_package(
        PackageKind::Dbpf,
        7, // type, group, unknown all shared in the index header
        &[stored(
            0x0A0B_0C0D,
            0x0A0B_0C0D,
            0x0000_00FE,
            b"only-instance-varies",
        )],
    );
    let path = write_temp("shared", &file);
    let package = Package::open(&path).unwrap();

    let e = &package.entries()[0];
    assert_eq!(e.id.type_id, 0x0A0B_0C0D); // from entries[0] echoed into header
    assert_eq!(e.id.group, 0x0A0B_0C0D);
    assert_eq!(e.unknown, SHARED_UNKNOWN);
    assert_eq!(package.read(e).unwrap(), b"only-instance-varies");
    std::fs::remove_file(&path).ok();
}

#[test]
fn dbbf_roundtrip_with_64bit_offsets() {
    let file = build_package(
        PackageKind::Dbbf,
        4,
        &[stored(
            0x0000_0003,
            0x0000_00BB,
            0x0000_0033,
            b"big package",
        )],
    );
    let path = write_temp("dbbf", &file);
    let package = Package::open(&path).unwrap();

    assert_eq!(package.header().kind, PackageKind::Dbbf);
    let e = &package.entries()[0];
    assert_eq!(package.read(e).unwrap(), b"big package");
    std::fs::remove_file(&path).ok();
}

#[test]
fn high_bit_on_compressed_size_is_masked() {
    let entry = stored(1, 2, 3, b"abcd");
    let mut file = build_package(PackageKind::Dbpf, 4, &[entry]);
    // locate the csize field: index at 96, values(4) + shared unknown(4) +
    // record: type(4) group(4) instance(4) offset(4) -> csize at 96+8+16 = 120
    file[120..124].copy_from_slice(&0x8000_0004u32.to_le_bytes());
    let path = write_temp("highbit", &file);
    let package = Package::open(&path).unwrap();

    let e = &package.entries()[0];
    assert_eq!(e.compressed_size, 4);
    assert_eq!(package.read(e).unwrap(), b"abcd");
    std::fs::remove_file(&path).ok();
}

#[test]
fn bad_compression_flags_rejected() {
    let mut entry = stored(1, 2, 3, b"abcd");
    entry.compressed_flags = 5;
    let file = build_package(PackageKind::Dbpf, 4, &[entry]);
    let path = write_temp("badflags", &file);
    let err = Package::open(&path).unwrap_err();
    assert!(matches!(err, Error::BadCompressionFlags(5)));
    std::fs::remove_file(&path).ok();
}

#[test]
fn bad_magic_rejected() {
    let mut file = build_package(PackageKind::Dbpf, 4, &[stored(1, 2, 3, b"x")]);
    file[0..4].copy_from_slice(b"XXXX");
    let path = write_temp("badmagic", &file);
    assert!(matches!(Package::open(&path), Err(Error::NotAPackage(_))));
    std::fs::remove_file(&path).ok();
}

#[test]
fn truncated_index_detected() {
    let mut file = build_package(PackageKind::Dbpf, 4, &[stored(1, 2, 3, b"data")]);
    // claim 3 entries in the header
    let count_at = 0x24;
    file[count_at..count_at + 4].copy_from_slice(&3u32.to_le_bytes());
    let path = write_temp("truncated", &file);
    assert!(matches!(Package::open(&path), Err(Error::Truncated { .. })));
    std::fs::remove_file(&path).ok();
}

#[test]
fn resource_offset_out_of_range() {
    let mut file = build_package(PackageKind::Dbpf, 4, &[stored(1, 2, 3, b"data")]);
    // index at 96: values(4) + shared unknown(4) + record: type(4) group(4)
    // instance(4) -> offset field at 96+8+12 = 116
    file[116..120].copy_from_slice(&0xFFFF_FFF0u32.to_le_bytes());
    let path = write_temp("oor", &file);
    let package = Package::open(&path).unwrap();
    let e = &package.entries()[0];
    assert!(matches!(
        package.read(e),
        Err(Error::OffsetOutOfRange { .. })
    ));
    std::fs::remove_file(&path).ok();
}
