//! The output layer: what a refined mesh becomes on disk.
//!
//! The gridfile mesh types, their NetCDF reading and writing, and the records
//! a gridfile carries beside the mesh (the h-field a run marked from, the
//! nominal MPAS widths). Nothing here depends on a refinement backend or on
//! how the demand was produced -- `scripts/check_architecture.py` holds that
//! -- so every backend's mesh is written the same way. See
//! docs/architecture_layering_audit_2026-09-25.md, step 5.

pub mod coordinate_types;
pub mod fs_support;
pub mod hfield_gridfile_context;
pub mod mpas_gridfile_context;
pub mod netcdf_io;
pub mod unstructured_mesh_io;
pub mod unstructured_mesh_support;

pub use coordinate_types::{lat_values, lon_values, LonLatPoint};
pub use fs_support::ensure_parent_dir;
pub use netcdf_io::{
    create_netcdf, netcdf_to_io_error, open_netcdf, require_len, required_dimension_len,
    required_values_f64, required_values_i32, required_values_i32_matrix,
};
pub use unstructured_mesh_support::{
    unstructured_dimc, validate_unstructured_mesh, GridfileMetadataSlices, UnstructuredMesh,
    UnstructuredMeshWriteReport,
};
