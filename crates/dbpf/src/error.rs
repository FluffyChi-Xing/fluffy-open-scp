use std::io;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("not a DBPF/DBBF package (magic {0:#010x})")]
    NotAPackage(u32),

    #[error("corrupt header: reserved field at 0x{at:X} is {got}, expected 3")]
    BadReservedField { at: usize, got: u32 },

    #[error("invalid index header values {0} (expected 4..=7)")]
    BadIndexHeaderValues(i32),

    #[error("invalid compression flags {0} (expected 0 or -1)")]
    BadCompressionFlags(i16),

    #[error("file truncated: need {needed} bytes at offset {at}, file size {size}")]
    Truncated { needed: usize, at: usize, size: usize },

    #[error("offset {offset} is outside the file (size {size})")]
    OffsetOutOfRange { offset: u64, size: u64 },

    #[error("refpack stream: {0}")]
    Refpack(&'static str),

    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
