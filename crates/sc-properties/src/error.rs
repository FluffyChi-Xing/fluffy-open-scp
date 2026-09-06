use thiserror::Error;

use crate::PropType;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("truncated property file: need {needed} bytes at offset {at}, size {size}")]
    Truncated {
        needed: usize,
        at: usize,
        size: usize,
    },

    #[error("unknown property type id {0:#06x} for hash {1:#010x}")]
    UnknownPropertyType(u16, u32),

    #[error("duplicate property hash {0:#010x}")]
    DuplicateHash(u32),

    #[error("invalid property flags {flags:#06x} for hash {hash:#010x}")]
    InvalidFlags { hash: u32, flags: u16 },

    #[error("property value type does not match {prop_type:?} for hash {hash:#010x}")]
    ValueTypeMismatch { hash: u32, prop_type: PropType },

    #[error("invalid array item size {actual} for hash {hash:#010x}; expected {expected}")]
    InvalidArrayItemSize {
        hash: u32,
        expected: i32,
        actual: i32,
    },

    #[error("property string is too long for hash {hash:#010x}")]
    StringTooLong { hash: u32 },

    #[error("property array is too large for hash {hash:#010x}")]
    ArrayTooLarge { hash: u32 },

    #[error("invalid transform encoding for hash {hash:#010x}")]
    InvalidTransform { hash: u32 },

    #[error("property file is too large to encode")]
    EncodeSizeOverflow,

    #[error("property parse limit exceeded: {0}")]
    LimitExceeded(&'static str),
}
