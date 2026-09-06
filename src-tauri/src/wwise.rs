use std::fmt;

use serde::Serialize;

pub const BKHD_MAGIC: &[u8; 4] = b"BKHD";
pub const DIDX_MAGIC: &[u8; 4] = b"DIDX";
pub const DATA_MAGIC: &[u8; 4] = b"DATA";
pub const DIDX_ENTRY_SIZE: usize = 12;
pub const MAX_DIDX_ENTRIES: usize = 65_536;
pub const MAX_BANK_DATA: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WwiseMediaEntry {
    pub id: u32,
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WwiseBank {
    pub version: Option<u32>,
    pub media: Vec<WwiseMediaEntry>,
    pub data_size: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub struct WwiseBankData<'a> {
    pub bank: WwiseBank,
    data: &'a [u8],
}

impl<'a> WwiseBankData<'a> {
    pub fn parse(input: &'a [u8]) -> Result<Self, WwiseError> {
        let mut cursor = Cursor::new(input);
        let mut version = None;
        let mut saw_bkhd = false;
        let mut media = Vec::new();
        let mut data = None;
        while cursor.remaining() > 0 {
            let tag = cursor.bytes(4)?;
            let size = usize::try_from(cursor.u32()?).map_err(|_| WwiseError::SizeOverflow)?;
            let chunk = cursor.bytes(size)?;
            match tag {
                tag if tag == BKHD_MAGIC => {
                    saw_bkhd = true;
                    if chunk.len() < 4 {
                        return Err(WwiseError::InvalidBankHeader);
                    }
                    version = Some(u32::from_le_bytes(chunk[..4].try_into().unwrap()));
                }
                tag if tag == DIDX_MAGIC => {
                    if chunk.len() % DIDX_ENTRY_SIZE != 0 {
                        return Err(WwiseError::InvalidDidxLength(chunk.len()));
                    }
                    let count = chunk.len() / DIDX_ENTRY_SIZE;
                    if count > MAX_DIDX_ENTRIES {
                        return Err(WwiseError::TooManyEntries(count));
                    }
                    media.clear();
                    for item in chunk.chunks_exact(DIDX_ENTRY_SIZE) {
                        media.push(WwiseMediaEntry {
                            id: u32::from_le_bytes(item[..4].try_into().unwrap()),
                            offset: u32::from_le_bytes(item[4..8].try_into().unwrap()),
                            size: u32::from_le_bytes(item[8..12].try_into().unwrap()),
                        });
                    }
                }
                tag if tag == DATA_MAGIC => {
                    if chunk.len() > MAX_BANK_DATA {
                        return Err(WwiseError::DataTooLarge(chunk.len()));
                    }
                    data = Some(chunk);
                }
                _ => {}
            }
        }
        let data = data.ok_or(WwiseError::MissingData)?;
        if !saw_bkhd {
            return Err(WwiseError::MissingBankHeader);
        }
        if !media.is_empty() {
            for entry in &media {
                let end = u64::from(entry.offset)
                    .checked_add(u64::from(entry.size))
                    .ok_or(WwiseError::EntryOutOfBounds(entry.id))?;
                if end > data.len() as u64 {
                    return Err(WwiseError::EntryOutOfBounds(entry.id));
                }
            }
        }
        Ok(Self {
            bank: WwiseBank {
                version,
                media,
                data_size: data.len(),
            },
            data,
        })
    }

    pub fn wem(&self, id: u32) -> Result<&'a [u8], WwiseError> {
        let entry = self
            .bank
            .media
            .iter()
            .find(|entry| entry.id == id)
            .ok_or(WwiseError::MediaNotFound(id))?;
        let start = entry.offset as usize;
        let end = start + entry.size as usize;
        Ok(&self.data[start..end])
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WwiseError {
    #[error("truncated Wwise SoundBank at offset {offset}: need {needed} bytes, size {size}")]
    Truncated {
        offset: usize,
        needed: usize,
        size: usize,
    },
    #[error("Wwise DIDX chunk has invalid length {0}")]
    InvalidDidxLength(usize),
    #[error("Wwise SoundBank contains too many media entries: {0}")]
    TooManyEntries(usize),
    #[error("Wwise SoundBank size overflow")]
    SizeOverflow,
    #[error("Wwise SoundBank has no DATA chunk")]
    MissingData,
    #[error("Wwise SoundBank has an invalid BKHD chunk")]
    InvalidBankHeader,
    #[error("Wwise SoundBank has no BKHD chunk")]
    MissingBankHeader,
    #[error("Wwise DATA chunk is too large: {0} bytes")]
    DataTooLarge(usize),
    #[error("Wwise media entry {0} is outside DATA")]
    EntryOutOfBounds(u32),
    #[error("Wwise media entry {0} was not found")]
    MediaNotFound(u32),
    #[error("Wwise SoundBank has no media entries")]
    NoMedia,
}

impl<'a> fmt::Debug for WwiseBankData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WwiseBankData")
            .field("bank", &self.bank)
            .field("data_len", &self.data.len())
            .finish()
    }
}

struct Cursor<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.input.len() - self.offset
    }

    fn bytes(&mut self, count: usize) -> Result<&'a [u8], WwiseError> {
        let start = self.offset;
        let end = start.checked_add(count).ok_or(WwiseError::SizeOverflow)?;
        let output = self.input.get(start..end).ok_or(WwiseError::Truncated {
            offset: start,
            needed: count,
            size: self.input.len(),
        })?;
        self.offset = end;
        Ok(output)
    }

    fn u32(&mut self) -> Result<u32, WwiseError> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut output = tag.to_vec();
        output.extend_from_slice(&(data.len() as u32).to_le_bytes());
        output.extend_from_slice(data);
        output
    }

    fn bank() -> Vec<u8> {
        let bkhd = chunk(b"BKHD", &1u32.to_le_bytes());
        let mut didx_data = Vec::new();
        didx_data.extend_from_slice(&7u32.to_le_bytes());
        didx_data.extend_from_slice(&2u32.to_le_bytes());
        didx_data.extend_from_slice(&3u32.to_le_bytes());
        let didx = chunk(b"DIDX", &didx_data);
        let data = chunk(b"DATA", b"xxabc");
        [bkhd, chunk(b"HIRC", b"ignored"), didx, data]
            .into_iter()
            .flatten()
            .collect()
    }

    #[test]
    fn parses_chunks_and_extracts_wem() {
        let input = bank();
        let parsed = WwiseBankData::parse(&input).unwrap();
        assert_eq!(parsed.bank.version, Some(1));
        assert_eq!(parsed.bank.media.len(), 1);
        assert_eq!(parsed.wem(7).unwrap(), b"abc");
    }

    #[test]
    fn rejects_invalid_didx_and_bounds() {
        let input = [chunk(b"DIDX", b"x"), chunk(b"DATA", b"")].concat();
        assert!(matches!(
            WwiseBankData::parse(&input),
            Err(WwiseError::InvalidDidxLength(1))
        ));

        let mut didx = Vec::new();
        didx.extend_from_slice(&1u32.to_le_bytes());
        didx.extend_from_slice(&9u32.to_le_bytes());
        didx.extend_from_slice(&1u32.to_le_bytes());
        let input = [
            chunk(b"BKHD", &1u32.to_le_bytes()),
            chunk(b"DIDX", &didx),
            chunk(b"DATA", b"x"),
        ]
        .concat();
        assert!(matches!(
            WwiseBankData::parse(&input),
            Err(WwiseError::EntryOutOfBounds(1))
        ));
    }

    #[test]
    fn missing_data_and_truncated_chunks_are_errors() {
        assert_eq!(
            WwiseBankData::parse(&chunk(b"BKHD", &1u32.to_le_bytes())).unwrap_err(),
            WwiseError::MissingData
        );
        assert!(matches!(
            WwiseBankData::parse(b"BKHD\x04\x00"),
            Err(WwiseError::Truncated { .. })
        ));
    }
}
