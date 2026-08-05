mod area;
mod containment;
mod helpers;

pub use area::getcontain_is_in_area_ustr_one_based;
pub use containment::{
    getcontain_containment_matrix_flat_one_based, getcontain_containment_matrix_one_based,
};
#[cfg(test)]
pub(crate) use helpers::{
    getcontain_axis_candidate_range, getcontain_restore_dateline_source_index,
    getcontain_south_pole_scan_polygons,
};
pub(crate) use helpers::getcontain_validate_source_matrix;
