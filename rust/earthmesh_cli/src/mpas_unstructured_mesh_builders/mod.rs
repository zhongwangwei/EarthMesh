mod full;
mod legacy;
mod simple;

pub use full::build_mpas_mesh_from_unstructured_fortran_indexed;
pub(crate) use legacy::{
    normalize_unstructured_mesh_legacy_placeholders, restore_unstructured_mesh_shape,
};
pub use simple::build_mpas_simple_mesh_from_unstructured_fortran_indexed;
