//! Isolated M1 Property writeback pipeline. No automatic workbench integration.
pub mod commands;
mod connector;
mod filesystem;
mod package;
pub mod patch;
pub mod plan;
mod processor;
mod value;

pub use package::{M1BuildResult, build_from_schema};

#[cfg(test)]
mod tests;
