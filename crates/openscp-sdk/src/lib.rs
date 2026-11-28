#![forbid(unsafe_code)]

//! Host-independent access to the existing OpenSCP format libraries.
//!
//! Modules re-export the original crates: types and codec behavior retain
//! their identity, allowing incremental adoption by existing consumers.
//! Format detection is always available; other modules are feature-gated.
//!
//! ```
//! use openscp_sdk::format::{probe, Format};
//! assert_eq!(probe(b"DBPF").unwrap(), Format::Dbpf);
//! ```

mod error;
#[cfg(feature = "package")]
pub mod resource;
pub use error::{Error, Result};

#[cfg(feature = "package")]
pub use dbpf as package;
#[cfg(feature = "package")]
pub use dbpf::ResourceId;
#[cfg(feature = "rw4")]
pub use rw4;
pub use sc_format as format;
#[cfg(feature = "properties")]
pub use sc_properties as properties;
