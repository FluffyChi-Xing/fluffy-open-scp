//! Native SC5 Asset Editor export pipeline.
//!
//! The Asset Editor plug-in is not shipped at runtime.  Blender is used only
//! as a `.blend` reader (`blender_scripts/extract_blend.py`); every export
//! decision is made here in Rust, with one responsibility per module:
//!
//! | module | responsibility |
//! | --- | --- |
//! | [`dump`] | serde model of the extraction bundle (no I/O) |
//! | [`settings`] | per-face `settings_json` + plug-in defaults |
//! | [`layout`] | window grid geometry (`layout_values`) |
//! | [`appearance`] | deterministic window occupancy |
//! | [`catalog`] | pack/atlas catalog data |
//! | [`channels`] | face state resolution (layers/materials) |
//! | [`material_state`] | node-graph state recovery, UV domains |
//! | [`resources`] | pack image lookup and RGBA pixel access |
//! | [`recipe`] | per-material texture generation |
//! | [`geometry`] | tessellation, clipping, tangents |
//! | [`decals`] | decal layout/placement |
//! | [`glb`] | GLB assembly |
//! | [`lotm`] | LOTM v9 container writer |
//! | [`export`] | orchestration; the only place that combines modules |
//!
//! Only `export` returns bytes; the pure modules take explicit inputs and have
//! no side effects.  File/Blender/network I/O stays in the command layer.

pub mod appearance;
pub mod catalog;
pub mod channels;
pub mod decals;
pub mod dump;
pub mod export;
pub mod geometry;
pub mod glb;
pub mod layout;
pub mod lotm;
pub mod material_state;
pub mod recipe;
pub mod resources;
pub mod settings;
pub mod trace;
