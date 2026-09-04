use thiserror::Error;

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

    #[error("array type id (99) used directly as entry type for hash {0:#010x}")]
    ArrayTypeAsEntry(u32),
}
