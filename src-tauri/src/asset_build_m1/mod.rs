//! Isolated M1 Property writeback pipeline. No automatic workbench integration.
pub mod commands;
mod connector;
mod filesystem;
pub mod lotm;
pub mod native_model;
mod package;
pub mod patch;
pub mod plan;
mod processor;
pub mod raster;
mod value;

pub use package::{M1BuildResult, build_from_schema};

#[cfg(test)]
mod tests;
