//! Local UI preview transport compatible with the updater's ES5 client.

#![forbid(unsafe_code)]

mod background;
mod http;
mod session;

pub use background::{init, publish};
pub use http::serve;

pub const DEFAULT_PORT: u16 = 47837;
