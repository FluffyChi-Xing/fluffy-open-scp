#[test]
fn probes_signatures_without_claiming_structural_validity() {
    use openscp_sdk::format::{Format, probe};
    assert_eq!(probe(b"DBPF").unwrap(), Format::Dbpf);
    assert_eq!(probe(b"unknown").unwrap(), Format::Unknown);
}

#[cfg(feature = "package")]
#[test]
fn retains_resource_identity_and_reports_duplicate_writes() {
    use openscp_sdk::{Error, ResourceId, package};
    let original: dbpf::ResourceId = ResourceId {
        type_id: u32::MAX,
        group: 0,
        instance: u32::MAX,
    };
    let entry = package::OverlayEntry::new(original, b"opaque");
    let error: Error = package::write_uncompressed_overlay(&[entry.clone(), entry])
        .unwrap_err()
        .into();
    assert_eq!(error.code(), "openscp.package.write");
    assert!(std::error::Error::source(&error).is_some());
}

#[cfg(feature = "properties")]
#[test]
fn preserves_property_limits_and_error_category() {
    use openscp_sdk::{
        Error,
        properties::{ParseLimits, PropertyFile},
    };
    let bytes = 2u32.to_be_bytes();
    let limits = ParseLimits {
        max_entries: 1,
        ..Default::default()
    };
    let error: Error = PropertyFile::parse_with_limits(&bytes, limits)
        .unwrap_err()
        .into();
    assert_eq!(error.code(), "openscp.property");
    assert!(PropertyFile::parse(&[0, 0]).is_err());
}

#[cfg(feature = "rw4")]
#[test]
fn rw4_synthetic_sections_roundtrip_and_invalid_input_fails() {
    use openscp_sdk::{Error, rw4};
    let bytes = rw4::build_file(1, &[], &[]);
    let model = rw4::Rw4File::parse(&bytes).unwrap();
    assert_eq!(model.sections().len(), 0);
    let error: Error = rw4::Rw4File::parse(b"invalid").unwrap_err().into();
    assert_eq!(error.code(), "openscp.rw4");
}
