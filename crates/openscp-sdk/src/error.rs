/// Result for consumers composing multiple SDK codecs.
pub type Result<T> = std::result::Result<T, Error>;

/// Preserves the underlying codec error and its source chain.
/// Codec modules also expose their original, more specific result types.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[cfg(feature = "package")]
    #[error("DBPF read: {0}")]
    PackageRead(#[from] dbpf::Error),
    #[cfg(feature = "package")]
    #[error("DBPF write: {0}")]
    PackageWrite(#[from] dbpf::WriterError),
    #[cfg(feature = "package")]
    #[error("DBPF memory read: {0}")]
    PackageMemory(#[from] dbpf::MemoryReadError),
    #[cfg(feature = "package")]
    #[error("ERZ: {0}")]
    Erz(#[from] dbpf::ErzError),
    #[cfg(feature = "properties")]
    #[error("PROP: {0}")]
    Property(#[from] sc_properties::Error),
    #[cfg(feature = "rw4")]
    #[error("RW4: {0}")]
    Rw4(#[from] rw4::Error),
}

impl Error {
    /// Machine-readable category; callers translate presentation text.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "openscp.io",
            #[cfg(feature = "package")]
            Self::PackageRead(_) => "openscp.package.read",
            #[cfg(feature = "package")]
            Self::PackageWrite(_) => "openscp.package.write",
            #[cfg(feature = "package")]
            Self::PackageMemory(_) => "openscp.package.memory",
            #[cfg(feature = "package")]
            Self::Erz(_) => "openscp.erz",
            #[cfg(feature = "properties")]
            Self::Property(_) => "openscp.property",
            #[cfg(feature = "rw4")]
            Self::Rw4(_) => "openscp.rw4",
        }
    }
}
