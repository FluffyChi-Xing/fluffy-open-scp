//! Small, dependency-free format signature probe.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Dbpf,
    Dbbf,
    Rw4,
    Zip,
    Gzip,
    Zlib,
    Png,
    Jpeg,
    Dds,
    Wav,
    Webp,
    Ogg,
    Ico,
    Xml,
    Unknown,
}

pub type Result<T> = std::result::Result<T, ProbeError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    /// Reserved for future structural probes; signature probing itself is infallible.
    InvalidInput,
}

pub fn probe(bytes: &[u8]) -> Result<Format> {
    let f = if bytes.starts_with(b"DBPF") {
        Format::Dbpf
    } else if bytes.starts_with(b"DBBF") {
        Format::Dbbf
    } else if bytes.len() >= 8 && bytes[..8] == [137, 82, 87, 52, 119, 51, 50, 0] {
        Format::Rw4
    } else if bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
    {
        Format::Zip
    } else if bytes.starts_with(&[0x1f, 0x8b]) {
        Format::Gzip
    } else if is_zlib(bytes) {
        Format::Zlib
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Format::Png
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Format::Jpeg
    } else if bytes.starts_with(b"DDS ") {
        Format::Dds
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        Format::Wav
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Format::Webp
    } else if bytes.starts_with(b"OggS") {
        Format::Ogg
    } else if bytes.starts_with(&[0, 0, 1, 0]) {
        Format::Ico
    } else if looks_like_xml(bytes) {
        Format::Xml
    } else {
        Format::Unknown
    };
    Ok(f)
}

fn is_zlib(b: &[u8]) -> bool {
    if b.len() < 2 || b[0] & 0x0f != 8 || b[0] >> 4 > 7 {
        return false;
    }
    (u16::from(b[0]) << 8 | u16::from(b[1])) % 31 == 0
}

fn looks_like_xml(b: &[u8]) -> bool {
    let b = b.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(b);
    let b = b
        .strip_prefix(b"\xff\xfe")
        .or_else(|| b.strip_prefix(b"\xfe\xff"))
        .unwrap_or(b);
    let first = b.iter().position(|byte| !byte.is_ascii_whitespace());
    first.is_some_and(|at| b[at..].starts_with(b"<"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn assert_probe(bytes: &[u8], expected: Format) {
        assert_eq!(probe(bytes).unwrap(), expected);
    }
    #[test]
    fn all_signatures() {
        assert_probe(b"DBPF", Format::Dbpf);
        assert_probe(b"DBBF", Format::Dbbf);
        assert_probe(&[137, 82, 87, 52, 119, 51, 50, 0], Format::Rw4);
        assert_probe(b"PK\x03\x04", Format::Zip);
        assert_probe(b"\x1f\x8b", Format::Gzip);
        assert_probe(&[0x78, 0x9c], Format::Zlib);
        assert_probe(b"\x89PNG\r\n\x1a\n", Format::Png);
        assert_probe(b"\xff\xd8\xff", Format::Jpeg);
        assert_probe(b"DDS ", Format::Dds);
        assert_probe(b"RIFFxxxxWAVE", Format::Wav);
        assert_probe(b"RIFFxxxxWEBP", Format::Webp);
        assert_probe(b"OggS\0", Format::Ogg);
        assert_probe(&[0, 0, 1, 0], Format::Ico);
        assert_probe(b"  <?xml version=\"1.0\"?>", Format::Xml);
    }
    #[test]
    fn unknown_is_not_error() {
        assert_eq!(probe(&[]).unwrap(), Format::Unknown);
        assert_eq!(probe(b"nonsense").unwrap(), Format::Unknown);
    }
    #[test]
    fn signatures_need_not_be_complete() {
        assert_probe(b"RIFF", Format::Unknown);
        assert_probe(b"<not necessarily complete", Format::Xml);
    }
    #[test]
    fn zlib_requires_valid_header_checksum() {
        assert_probe(&[0x78, 0x00], Format::Unknown);
        assert_probe(&[0x78, 0x01], Format::Zlib);
    }
    #[test]
    fn xml_bom_and_whitespace() {
        assert_probe(&[0xef, 0xbb, 0xbf, b' ', b'<'], Format::Xml);
    }
}
