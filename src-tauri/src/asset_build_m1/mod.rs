//! Isolated M1 Property writeback pipeline. No automatic workbench integration.
pub mod commands;
mod connector;
pub mod collision;
mod filesystem;
pub mod lotm;
pub mod atlas_prepare;
pub mod material_atlas;
pub mod materials;
pub mod menu;
pub mod native_model;
pub mod native_lot;
pub mod native_profile;
mod package;
pub mod events;
pub mod patch;
pub mod plan;
mod processor;
pub mod raster;
pub mod resource_closure;
pub mod scene;
pub mod stages;
pub mod single_compatible;
pub mod template_material;
mod value;

pub use package::{M1BuildResult, build_from_schema};

#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod tests;
