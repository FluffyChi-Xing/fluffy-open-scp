use crate::error::{Error, Result};

/// Little-endian, bounds-checked reader over an RW4 byte slice.
/// RW4（Gibbed `StreamHelpers`）为小端；所有读取都带检查标签，出错可定位。
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn seek(&mut self, pos: usize) -> Result<()> {
        if pos > self.data.len() {
            return Err(Error::Truncated {
                needed: pos,
                at: self.pos,
                size: self.data.len(),
            });
        }
        self.pos = pos;
        Ok(())
    }

    fn take(&mut self, n: usize, check: &'static str) -> Result<&'a [u8]> {
        let start = self.pos;
        let end = start.checked_add(n).ok_or(Error::Overflow { check })?;
        let slice = self.data.get(start..end).ok_or(Error::Truncated {
            needed: n,
            at: start,
            size: self.data.len(),
        })?;
        self.pos = end;
        Ok(slice)
    }

    pub fn u32(&mut self, check: &'static str) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4, check)?.try_into().unwrap()))
    }

    pub fn i32(&mut self, check: &'static str) -> Result<i32> {
        Ok(i32::from_le_bytes(self.take(4, check)?.try_into().unwrap()))
    }

    pub fn i16(&mut self, check: &'static str) -> Result<i16> {
        Ok(i16::from_le_bytes(self.take(2, check)?.try_into().unwrap()))
    }

    pub fn f32(&mut self, check: &'static str) -> Result<f32> {
        Ok(f32::from_le_bytes(self.take(4, check)?.try_into().unwrap()))
    }

    pub fn u16(&mut self, check: &'static str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2, check)?.try_into().unwrap()))
    }

    pub fn u16be(&mut self, check: &'static str) -> Result<u16> {
        Ok(u16::from_be_bytes(self.take(2, check)?.try_into().unwrap()))
    }

    pub fn u32be(&mut self, check: &'static str) -> Result<u32> {
        Ok(u32::from_be_bytes(self.take(4, check)?.try_into().unwrap()))
    }

    pub fn u8(&mut self, check: &'static str) -> Result<u8> {
        Ok(self.take(1, check)?[0])
    }

    pub fn expect_u32(&mut self, expected: u32, check: &'static str) -> Result<()> {
        let actual = self.u32(check)?;
        if actual != expected {
            return Err(Error::UnexpectedValue {
                check,
                expected: expected as u64,
                actual: actual as u64,
            });
        }
        Ok(())
    }

    pub fn expect_u64(&mut self, expected: u64, check: &'static str) -> Result<()> {
        let actual = self.u32(check)?;
        if actual as u64 != expected {
            return Err(Error::UnexpectedValue {
                check,
                expected,
                actual: actual as u64,
            });
        }
        Ok(())
    }

    pub fn expect_slice(&mut self, expected: &[u32], check: &'static str) -> Result<()> {
        for (i, want) in expected.iter().enumerate() {
            let actual = self.u32(check)?;
            if actual != *want {
                return Err(Error::UnexpectedValue {
                    check,
                    expected: *want as u64,
                    actual: actual as u64,
                });
            }
            let _ = i;
        }
        Ok(())
    }
}
