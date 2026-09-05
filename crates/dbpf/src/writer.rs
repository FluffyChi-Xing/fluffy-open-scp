use std::io;
use std::path::Path;

use crate::index::ResourceId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayEntry {
    pub id: ResourceId,
    pub data: Vec<u8>,
}

impl OverlayEntry {
    pub fn new(id: ResourceId, data: impl Into<Vec<u8>>) -> Self {
        Self {
            id,
            data: data.into(),
        }
    }
}

#[derive(Debug)]
pub enum WriterError {
    DuplicateResource(ResourceId),
    TooManyEntries(usize),
    SizeOverflow(&'static str),
    Io(io::Error),
}

impl std::fmt::Display for WriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateResource(id) => write!(f, "duplicate resource TGI {id}"),
            Self::TooManyEntries(n) => write!(f, "too many DBPF entries: {n}"),
            Self::SizeOverflow(what) => write!(f, "DBPF size overflow while calculating {what}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}
impl std::error::Error for WriterError {}
impl From<io::Error> for WriterError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
pub type WriterResult<T> = std::result::Result<T, WriterError>;

/// Write a deterministic DBPF overlay containing stored (uncompressed) resources.
/// Entries are sorted by (type, group, instance), and all index fields are fixed.
pub fn write_uncompressed_overlay(entries: &[OverlayEntry]) -> WriterResult<Vec<u8>> {
    let mut sorted = entries.to_vec();
    sorted.sort_by_key(|e| (e.id.type_id, e.id.group, e.id.instance));
    for pair in sorted.windows(2) {
        if pair[0].id == pair[1].id {
            return Err(WriterError::DuplicateResource(pair[0].id));
        }
    }
    let count =
        u32::try_from(sorted.len()).map_err(|_| WriterError::TooManyEntries(sorted.len()))?;
    let records = sorted
        .len()
        .checked_mul(28)
        .ok_or(WriterError::SizeOverflow("index records"))?;
    let index_len = 8usize
        .checked_add(records)
        .ok_or(WriterError::SizeOverflow("index size"))?;
    let payload_base = 96usize
        .checked_add(index_len)
        .ok_or(WriterError::SizeOverflow("payload offset"))?;
    let mut payload_len = 0usize;
    for e in &sorted {
        payload_len = payload_len
            .checked_add(e.data.len())
            .ok_or(WriterError::SizeOverflow("payload size"))?;
    }
    let total = payload_base
        .checked_add(payload_len)
        .ok_or(WriterError::SizeOverflow("file size"))?;
    let index_len_u32 =
        u32::try_from(index_len).map_err(|_| WriterError::SizeOverflow("index size"))?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"DBPF");
    out.extend_from_slice(&1i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&[0; 24]);
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&index_len_u32.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    out.extend_from_slice(&3u32.to_le_bytes());
    out.extend_from_slice(&96u32.to_le_bytes());
    out.extend_from_slice(&[0; 28]);
    debug_assert_eq!(out.len(), 96);
    out.extend_from_slice(&4i32.to_le_bytes()); // shared unknown only
    out.extend_from_slice(&0u32.to_le_bytes());
    let mut offset = payload_base;
    for e in &sorted {
        let offset32 =
            u32::try_from(offset).map_err(|_| WriterError::SizeOverflow("resource offset"))?;
        let len =
            u32::try_from(e.data.len()).map_err(|_| WriterError::SizeOverflow("resource size"))?;
        out.extend_from_slice(&e.id.type_id.to_le_bytes());
        out.extend_from_slice(&e.id.group.to_le_bytes());
        out.extend_from_slice(&e.id.instance.to_le_bytes());
        out.extend_from_slice(&offset32.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&0i16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        offset = offset
            .checked_add(e.data.len())
            .ok_or(WriterError::SizeOverflow("resource offset"))?;
    }
    for e in &sorted {
        out.extend_from_slice(&e.data);
    }
    debug_assert_eq!(out.len(), total);
    Ok(out)
}

pub fn write_uncompressed_overlay_to_path(
    path: impl AsRef<Path>,
    entries: &[OverlayEntry],
) -> WriterResult<()> {
    std::fs::write(path, write_uncompressed_overlay(entries)?).map_err(WriterError::from)
}
