//! Borrowed DBPF/DBBF inspection for callers that already own package bytes.

use crate::{Error, Header, IndexEntry, ResourceId, Result, header, index, refpack};

/// A read-only package backed by caller-owned bytes. No filesystem staging.
/// Uses the same header, index and RefPack parsers as [`crate::Package`].
#[derive(Debug)]
pub struct MemoryPackage<'a> {
    bytes: &'a [u8],
    header: Header,
    entries: Vec<IndexEntry>,
}

/// Admission limits checked before allocating the index or decoding payloads.
#[derive(Debug, Clone, Copy)]
pub struct MemoryParseLimits {
    /// Maximum size of the caller-owned container.
    pub max_input_bytes: usize,
    /// Maximum advertised resource count.
    pub max_entries: u32,
}

/// Container admission failure; no payload is decoded during admission.
#[derive(Debug, thiserror::Error)]
pub enum MemoryParseError {
    /// Input buffer exceeds the caller's limit.
    #[error("package input has {actual} bytes, limit is {limit}")]
    InputLimit { actual: usize, limit: usize },
    /// Header advertises too many index records.
    #[error("package index declares {actual} entries, limit is {limit}")]
    EntryLimit { actual: u32, limit: u32 },
    /// Original header/index error.
    #[error(transparent)]
    Decode(#[from] Error),
}

/// A decoding failure or caller-specified per-resource output budget violation.
#[derive(Debug, thiserror::Error)]
pub enum MemoryReadError {
    /// The index advertises more decoded bytes than permitted.
    #[error("resource {id} declares {declared} decoded bytes, limit is {limit}")]
    LimitExceeded {
        id: ResourceId,
        declared: u32,
        limit: usize,
    },
    /// Original container/RefPack error.
    #[error(transparent)]
    Decode(#[from] Error),
}

impl<'a> MemoryPackage<'a> {
    /// Check explicit input and index limits before index allocation.
    /// Caller ownership of input bytes is unchanged; this does not bound total
    /// process memory, concurrent reads, or codec allocations outside DBPF.
    pub fn parse_with_limits(
        bytes: &'a [u8],
        limits: MemoryParseLimits,
    ) -> std::result::Result<Self, MemoryParseError> {
        if bytes.len() > limits.max_input_bytes {
            return Err(MemoryParseError::InputLimit {
                actual: bytes.len(),
                limit: limits.max_input_bytes,
            });
        }
        let header = header::parse(bytes)?;
        if header.index_count > limits.max_entries {
            return Err(MemoryParseError::EntryLimit {
                actual: header.index_count,
                limit: limits.max_entries,
            });
        }
        let entries = index::parse(bytes, &header)?;
        Ok(Self {
            bytes,
            header,
            entries,
        })
    }

    /// Parse the header and index; payloads remain borrowed and undecoded.
    pub fn parse(bytes: &'a [u8]) -> Result<Self> {
        let header = header::parse(bytes)?;
        let entries = index::parse(bytes, &header)?;
        Ok(Self {
            bytes,
            header,
            entries,
        })
    }

    /// Parsed container header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// Resource records in original index order, including duplicate identities.
    pub fn entries(&self) -> &[IndexEntry] {
        &self.entries
    }

    /// First matching resource, mirroring [`crate::Package::entry`].
    pub fn entry(&self, id: ResourceId) -> Option<&IndexEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// Borrow stored bytes without decoding; validates the supplied range.
    pub fn read_raw(&self, entry: &IndexEntry) -> Result<&'a [u8]> {
        let end = entry
            .offset
            .checked_add(entry.stored_len())
            .ok_or(Error::OffsetOutOfRange {
                offset: entry.offset,
                size: self.bytes.len() as u64,
            })?;
        if end > self.bytes.len() as u64 {
            return Err(Error::OffsetOutOfRange {
                offset: end,
                size: self.bytes.len() as u64,
            });
        }
        Ok(&self.bytes[entry.offset as usize..end as usize])
    }

    /// Decode one payload after checking its advertised output size.
    /// This is a per-resource budget, not an index or total-process memory limit.
    pub fn read(
        &self,
        entry: &IndexEntry,
        max_decoded_bytes: usize,
    ) -> std::result::Result<Vec<u8>, MemoryReadError> {
        if u64::from(entry.decompressed_size) > max_decoded_bytes as u64 {
            return Err(MemoryReadError::LimitExceeded {
                id: entry.id,
                declared: entry.decompressed_size,
                limit: max_decoded_bytes,
            });
        }
        let raw = self.read_raw(entry)?;
        Ok(if entry.compressed {
            refpack::decompress(raw, entry.decompressed_size as usize)?
        } else {
            raw.to_vec()
        })
    }
}
