//! Isolated M1 Property writeback pipeline. No automatic workbench integration.
pub mod atlas_prepare;
pub mod author_building40;
pub mod author_materials;
pub mod behavior;
mod behavior_conflicts;
pub mod behavior_family;
pub mod behavior_nodes;
mod behavior_templates;
mod behavior_tourism;
pub mod build_cache;
pub mod collision;
pub mod commands;
mod connector;
pub mod delta_audit;
pub mod events;
mod filesystem;
pub mod geometry_route;
pub mod lotm;
pub mod material_atlas;
pub mod material_gate;
pub mod materials;
pub mod menu;
pub mod menu_contract;
pub mod native_lot;
pub mod native_model;
pub mod native_profile;
mod package;
pub mod patch;
pub mod path_catalog;
pub mod plan;
mod processor;
pub mod property_edit;
pub mod property_gate;
pub mod raster;
pub mod resource_closure;
pub mod scene;
pub mod schema_discovery;
pub mod single_compatible;
pub mod stages;
pub mod template_material;
mod value;
pub mod workbench_logic;
mod written_contract;

pub use package::{M1BuildResult, build_from_schema};

#[cfg(test)]
mod e2e_tests;
#[cfg(test)]
mod tests;

pub use package::build_from_schema_with_progress;
