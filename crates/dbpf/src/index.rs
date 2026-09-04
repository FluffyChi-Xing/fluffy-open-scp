use std::fmt;

use crate::error::{Error, Result};
use crate::header::{Header, PackageKind};
use crate::reader::Reader;

/// TGI key identifying a resource inside a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId {
    pub type_id: u32,
    pub group: u32,
    pub instance: u32,
}

impl fmt::Display for ResourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:08X}:{:08X}:{:08X}",
            self.type_id, self.group, self.instance
        )
    }
}

/// One index record: where a resource lives and how to decode it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub id: ResourceId,
    /// Per-entry unknown u32 (only read when the index header carries it).
    pub unknown: u32,
    /// Absolute byte offset of the resource payload in the package.
    pub offset: u64,
    /// Stored size; the 0x8000_0000 flag bit is masked off like the C# reader.
    pub compressed_size: u32,
    pub decompressed_size: u32,
    pub flags: u16,
    /// From the i16 compression field: -1 = RefPack-compressed, 0 = stored.
    pub compressed: bool,
}

impl IndexEntry {
    /// Number of payload bytes stored in the file for this entry.
    pub fn stored_len(&self) -> u64 {
        if self.compressed {
            u64::from(self.compressed_size)
        } else {
            u64::from(self.decompressed_size)
        }
    }
}

/// Parse the index (header values + `index_count` entries).
///
/// Mirrors `DatabasePackedFile.Read` in the C# implementation: entry TGI
/// fields are read per-entry unless the index header shares them via bit flags.
pub(crate) fn parse(data: &[u8], header: &Header) -> Result<Vec<IndexEntry>> {
    if header.index_count == 0 {
        return Ok(Vec::new());
    }

    let mut r = Reader::new(data, header.index_offset as usize);
    let values = r.i32()?;
    if !(4..=7).contains(&values) {
        return Err(Error::BadIndexHeaderValues(values));
    }

    let shared_type = if values & (1 << 0) != 0 {
        Some(r.u32()?)
    } else {
        None
    };
    let shared_group = if values & (1 << 1) != 0 {
        Some(r.u32()?)
    } else {
        None
    };
    let shared_unknown = if values & (1 << 2) != 0 {
        Some(r.u32()?)
    } else {
        None
    };

    let mut entries = Vec::with_capacity(header.index_count as usize);
    for _ in 0..header.index_count {
        let type_id = match shared_type {
            Some(v) => v,
            None => r.u32()?,
        };
        let group = match shared_group {
            Some(v) => v,
            None => r.u32()?,
        };
        let unknown = match shared_unknown {
            Some(v) => v,
            None => r.u32()?,
        };
        let instance = r.u32()?;

        let offset = match header.kind {
            PackageKind::Dbbf => r.i64()? as u64,
            PackageKind::Dbpf => r.i32()? as u64,
        };

        // The 0x8000_0000 bit is stripped, matching the C# reader; the
        // compression state comes from the i16 flag below, not this bit.
        let compressed_size = r.u32()? & !0x8000_0000;
        let decompressed_size = r.u32()?;
        let compressed_flags = r.i16()?;
        let flags = r.u16()?;

        let compressed = match compressed_flags {
            0 => false,
            -1 => true,
            other => return Err(Error::BadCompressionFlags(other)),
        };

        entries.push(IndexEntry {
            id: ResourceId {
                type_id,
                group,
                instance,
            },
            unknown,
            offset,
            compressed_size,
            decompressed_size,
            flags,
            compressed,
        });
    }
    Ok(entries)
}
