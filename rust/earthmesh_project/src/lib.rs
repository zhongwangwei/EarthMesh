//! EarthMesh v3 project schema (the L1 "intent" layer).
//!
//! A serde-(de)serializable [`ProjectConfig`] (YAML/JSON) that **lowers** to the
//! engine's [`earthmesh_core::EarthmeshConfig`] + [`earthmesh_core::RefineConfig`]
//! (+ the `&quality` / `&datalayers` blocks), reusing the core lowering built in
//! `earthmesh_core`. This keeps the friendly project layer separate from the 64
//! flat engine fields.
//!
//! Owns the project schema, validation, intent presets, criteria catalog, and
//! lowering into engine namelists.

mod schema;
pub use schema::*;
mod criteria;
pub use criteria::*;
mod display;
mod engine_mapping;
mod lowering;
pub use lowering::*;
mod manifest;
pub use manifest::*;
mod presets;
pub use presets::*;
mod validation;

// ----------------------------- tests -----------------------------

#[cfg(test)]
mod tests;
