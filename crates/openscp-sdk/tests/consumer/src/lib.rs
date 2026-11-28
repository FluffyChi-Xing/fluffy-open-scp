//! Independent consumer: imports only the public SDK dependency.

#[cfg(test)]
mod simoder_workload;

#[cfg(test)]
mod tests {
    use openscp_sdk::{ResourceId, format, package, properties, rw4};

    #[test]
    fn package_property_and_unknown_payload_roundtrip() -> openscp_sdk::Result<()> {
        // Synthetic scalar UInt32 PROP: no copyrighted game data required.
        let prop = [
            0, 0, 0, 1, // count
            0x12, 0x34, 0x56, 0x78, // hash
            0, 10, 0, 0, // UInt32 scalar
            0xff, 0xff, 0xff, 0xff,
        ];
        let parsed = properties::PropertyFile::parse(&prop)?;
        let canonical = parsed.encode_canonical()?;
        // Canonical encoding normalizes flags; payload semantics remain intact.
        assert_ne!(canonical, prop);
        assert_eq!(
            properties::PropertyFile::parse(&canonical)?.values[0].kind,
            parsed.values[0].kind
        );
        let prop_id = ResourceId {
            type_id: 0x00b1b104,
            group: 0,
            instance: u32::MAX,
        };
        let unknown_id = ResourceId {
            type_id: u32::MAX,
            group: u32::MAX,
            instance: 0,
        };
        let opaque = b"unknown resource must remain byte-identical";
        let bytes = package::write_uncompressed_overlay(&[
            package::OverlayEntry::new(prop_id, prop),
            package::OverlayEntry::new(unknown_id, opaque),
        ])?;
        assert_eq!(format::probe(&bytes).unwrap(), format::Format::Dbpf);
        // Keep generated fixtures in this consumer's ignored build directory.
        let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&fixture_dir)?;
        let path = fixture_dir.join(format!(
            "openscp-sdk-consumer-{}.package",
            std::process::id()
        ));
        std::fs::write(&path, bytes)?;
        let result = (|| -> openscp_sdk::Result<()> {
            let package = package::Package::open(&path)?;
            let entry = package.entry(prop_id).unwrap();
            assert_eq!(package.read_raw(entry)?, prop);
            assert_eq!(
                properties::PropertyFile::parse(&package.read(entry)?)?,
                parsed
            );
            assert_eq!(package.read(package.entry(unknown_id).unwrap())?, opaque);
            assert!(
                package
                    .entry(ResourceId {
                        instance: 1,
                        ..prop_id
                    })
                    .is_none()
            );
            Ok(())
        })();
        std::fs::remove_file(path)?;
        result
    }

    #[test]
    fn rw4_is_available_through_the_same_dependency() {
        let bytes = rw4::build_file(1, &[], &[]);
        assert!(rw4::Rw4File::parse(&bytes).is_ok());
    }
}
