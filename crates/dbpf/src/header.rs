use crate::error::{Error, Result};
use crate::reader::Reader;

/// DBPF is the standard package container; DBBF ("big") is a 64-bit-offset variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageKind {
    Dbpf,
    Dbbf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub kind: PackageKind,
    pub major_version: i32,
    pub minor_version: i32,
    /// Number of index entries. A negative count in the file is treated as 0
    /// (same as the C# implementation, which skips index parsing).
    pub index_count: u32,
    pub index_offset: u64,
    pub index_size: u64,
}

pub const DBPF_HEADER_LEN: usize = 96;
pub const DBBF_HEADER_LEN: usize = 120;

fn le_u32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}

fn le_i64(data: &[u8], at: usize) -> i64 {
    i64::from_le_bytes(data[at..at + 8].try_into().unwrap())
}

/// Parse the fixed-size package header (magic + 92/116 bytes).
pub fn parse(data: &[u8]) -> Result<Header> {
    if data.len() < 4 {
        return Err(Error::Truncated {
            needed: 4,
            at: 0,
            size: data.len(),
        });
    }
    let kind = match &data[0..4] {
        b"DBPF" => PackageKind::Dbpf,
        b"DBBF" => PackageKind::Dbbf,
        _ => {
            return Err(Error::NotAPackage(u32::from_le_bytes([
                data[0], data[1], data[2], data[3],
            ])));
        }
    };

    let mut r = Reader::new(data, 4);
    let major_version = r.i32()?;
    let minor_version = r.i32()?;

    let header = match kind {
        PackageKind::Dbpf => {
            if data.len() < DBPF_HEADER_LEN {
                return Err(Error::Truncated {
                    needed: DBPF_HEADER_LEN,
                    at: 0,
                    size: data.len(),
                });
            }
            let reserved = le_u32(data, 0x3C);
            if reserved != 3 {
                return Err(Error::BadReservedField {
                    at: 0x3C,
                    got: reserved,
                });
            }
            Header {
                kind,
                major_version,
                minor_version,
                index_count: le_u32(data, 0x24),
                index_size: u64::from(le_u32(data, 0x2C)),
                index_offset: u64::from(le_u32(data, 0x40)),
            }
        }
        PackageKind::Dbbf => {
            if data.len() < DBBF_HEADER_LEN {
                return Err(Error::Truncated {
                    needed: DBBF_HEADER_LEN,
                    at: 0,
                    size: data.len(),
                });
            }
            let reserved = le_u32(data, 0x34);
            if reserved != 3 {
                return Err(Error::BadReservedField {
                    at: 0x34,
                    got: reserved,
                });
            }
            Header {
                kind,
                major_version,
                minor_version,
                index_count: le_u32(data, 0x24),
                index_size: le_i64(data, 0x28) as u64,
                index_offset: le_i64(data, 0x38) as u64,
            }
        }
    };
    Ok(header)
}
