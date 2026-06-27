mod area;
mod containment;
mod helpers;
mod kinds;

pub use area::getcontain_is_in_area_ustr_fortran_indexed;
pub use containment::{
    getcontain_containment_matrix_flat_fortran_indexed,
    getcontain_containment_matrix_fortran_indexed,
};
pub(crate) use helpers::getcontain_validate_source_matrix;
pub(crate) use kinds::getcontain_mesh_kind_from_mesh_type;
