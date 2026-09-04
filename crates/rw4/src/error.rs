pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a RW4 file (bad magic)")]
    BadMagic,
    #[error("unknown RW4 file type code 0x{0:08X}")]
    UnknownFileType(u32),
    #[error("truncated: needed {needed} bytes at offset {at}, file size {size}")]
    Truncated {
        needed: usize,
        at: usize,
        size: usize,
    },
    #[error("{check}: expected 0x{expected:08X}, found 0x{actual:08X}")]
    UnexpectedValue {
        check: &'static str,
        expected: u64,
        actual: u64,
    },
    #[error("fixup target section {index} out of range ({count} sections)")]
    FixupTargetOutOfRange { index: usize, count: u32 },
    #[error("indirect type index {index} out of range ({count} table entries)")]
    IndirectTypeOutOfRange { index: u32, count: u32 },
    #[error("{check}: section type table[{index}] expected 0x{expected:08X}, found 0x{actual:08X}")]
    FixedSectionTypeMismatch {
        check: &'static str,
        index: u32,
        expected: u32,
        actual: u32,
    },
    #[error("{check}: section type[{index}] expected 0x{expected:08X}, found 0x{actual:08X}")]
    IndirectTypeMismatch {
        check: &'static str,
        index: u32,
        expected: u32,
        actual: u32,
    },
    #[error("section type 0x{code:08X} declared in the type table but used by no section")]
    UnusedSectionType { code: u32 },
    #[error("section #{number} out of range ({count} sections)")]
    SectionNumberOutOfRange { number: u32, count: u32 },
    #[error("section payload [{offset}..+{size}] exceeds file size {len}")]
    PayloadOutOfRange { offset: u32, size: u32, len: usize },
    #[error("{check}: payload provides {actual} bytes, need {needed}")]
    InsufficientPayload {
        check: &'static str,
        needed: usize,
        actual: usize,
    },
    #[error("{check}: section #{number} has type 0x{actual:08X}, expected 0x{expected:08X}")]
    BadSectionType {
        check: &'static str,
        number: u32,
        expected: u32,
        actual: u32,
    },
    #[error(
        "unsupported vertex declaration type {0} (factory returns null, component skipped upstream)"
    )]
    UnsupportedDeclarationType(u16),
    #[error("offset overflow while computing {check}")]
    Overflow { check: &'static str },
}
