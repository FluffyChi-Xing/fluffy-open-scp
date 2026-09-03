use crate::error::{Error, Result};

/// Bounds-checked little-endian cursor over a byte slice.
pub(super) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(data: &'a [u8], pos: usize) -> Self {
        Self { data, pos }
    }

    pub(super) fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let start = self.pos;
        let end = start
            .checked_add(n)
            .ok_or(Error::Truncated { needed: usize::MAX, at: start, size: self.data.len() })?;
        if end > self.data.len() {
            return Err(Error::Truncated { needed: n, at: start, size: self.data.len() });
        }
        self.pos = end;
        Ok(&self.data[start..end])
    }

    pub(super) fn i16(&mut self) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(super) fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(super) fn i32(&mut self) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(super) fn i64(&mut self) -> Result<i64> {
        Ok(i64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
}
