mod namelist;
mod netcdf;
mod types;
mod validation;

pub use namelist::parse_bbox_mask_nml;
pub use netcdf::{
    read_bbox_mask_netcdf, read_bbox_mesh_netcdf, read_bbox_refine_netcdf, write_bbox_mask_netcdf,
    write_bbox_mesh_netcdf,
};
pub use types::{BBoxMask, BBoxMesh, BBoxPoint};
pub(crate) use validation::validate_bbox_mask;
