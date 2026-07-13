mod bbox;
mod circle;
mod close;
mod dispatch;
mod domain;
mod shared;

#[cfg(test)]
pub(crate) use circle::{
    read_method_c_calculated_circle_refinement_regions, read_method_c_circle_refinement_regions,
};
#[cfg(test)]
pub(crate) use close::read_method_c_close_refinement_regions;
pub(crate) use dispatch::{
    read_method_c_calculated_refinement_regions, read_method_c_specified_refinement_regions,
};
#[cfg(test)]
pub(crate) use domain::read_method_c_close_domain_regions;
pub(crate) use domain::read_method_c_domain_region;
#[cfg(test)]
pub(crate) use shared::method_c_calculated_region_level;
