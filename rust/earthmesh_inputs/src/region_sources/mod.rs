mod bbox;
mod circle;
mod close;
mod dispatch;
mod domain;
mod shared;

// Also used by earthmesh_cli's own tests, so not test-only here.
pub use circle::{
    read_method_c_calculated_circle_refinement_regions, read_method_c_circle_refinement_regions,
};
// Also used by earthmesh_cli's own tests, so not test-only here.
pub use close::read_method_c_close_refinement_regions;
pub use dispatch::{
    read_method_c_calculated_refinement_regions, read_method_c_specified_refinement_regions,
};
// Also used by earthmesh_cli's own tests, so not test-only here.
pub use domain::read_method_c_close_domain_regions;
pub use domain::read_method_c_domain_region;
// Also used by earthmesh_cli's own tests, so not test-only here.
pub use shared::method_c_calculated_region_level;
