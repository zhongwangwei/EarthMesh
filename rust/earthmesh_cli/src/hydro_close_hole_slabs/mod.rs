mod non_axis;
mod non_rectilinear;
mod support;
mod triangular;

pub(crate) use non_axis::decompose_non_axis_aligned_exterior_holes_vertical_slabs;
pub(crate) use non_rectilinear::decompose_axis_aligned_exterior_non_rectilinear_holes_vertical_slabs;
pub(crate) use triangular::{
    decompose_axis_aligned_exterior_triangular_hole_vertical_slabs,
    decompose_axis_aligned_exterior_triangular_holes_vertical_slabs,
};
