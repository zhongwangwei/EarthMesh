//! Red-green refinement: subdivide any marked set of triangles, then close the
//! seams the subdivision left behind.
//!
//! Marked triangles are split into four (the *red* step). That leaves their
//! unmarked neighbours with a hanging node on the shared edge, so each of those
//! is split into two (the *green* step), and Lawson edge flips clean up the
//! angles the green triangles cost. The iterA..G judges run before any of it,
//! growing the marking until the result is a legal triangulation.
//!
//! # What this buys over Method-C
//!
//! The judges *grow* a marking they cannot take as given; they never refuse it.
//! Every error in this crate is an input-validation error — an array too short,
//! an index out of range, connectivity that does not close — and none of them
//! says a region is the wrong shape to refine. Method-C, in `earthmesh_mesh`,
//! does refuse: its seed lattice steps three cells at a time, its perimeter has
//! to be a multiple of three, and its transition patch reaches two faces beyond
//! the mask. That is the whole difference between refining an arbitrary coastal
//! region and refining one shaped like the blocks Method-C can build.
//!
//! What Method-C buys in exchange is vertex degree held to {5, 6, 7}, which is
//! what keeps the hexagonal dual usable. A model that consumes the triangles
//! directly, as FVCOM does, is not paying for that.
//!
//! # Provenance
//!
//! Ported from `MOD_grid_preprocess.F90` / `MOD_refine.F90` (EarthMesh v2)
//! kernel by kernel, which is what makes table-level exact comparison against
//! the Fortran reference possible: `nmd`/`nud`/`nwd`, per-level W face counts
//! and mrow envelopes all match row for row. Method-C and the h-field path can
//! only be checked at the topology and statistics level.
//!
//! See `docs/mesh_construction_technical_guide.md` section 3 for the algorithm
//! and section 6 for where this fits in the acceptance tiers.

// The ported modules use `use super::*` the same way they did inside
// `earthmesh_mesh`, so the names they expect have to be in scope here.
use std::io;

use earthmesh_mesh::{
    boundary_closed_curves_one_based, is_ngrmm, push_boundary_neighbor, robust_spherical_area_unit,
    spherical_centroid_degrees, BoundaryConnection, LonLatDegrees, RefineBoundarySegments,
    RefineWeakConcavitySegments,
};

mod refine_iter;
pub use refine_iter::{refine_iter_b_judge_one_based, refine_orial_vertices_protect_one_based};
mod refine_iter_c;
pub use refine_iter_c::refine_iter_c_judge_one_based;
mod refine_iter_d;
pub use refine_iter_d::refine_iter_d_judge_one_based;
mod refine_iter_e;
pub use refine_iter_e::refine_iter_e_judge_one_based;
mod refine_iter_f;
pub use refine_iter_f::refine_iter_f_judge_one_based;
mod refine_iter_g;
pub use refine_iter_g::refine_iter_g_judge_one_based;
mod refine_iter_helpers;
pub(crate) use refine_iter_helpers::{
    unique_triangle_cell, validate_refine_cell_neighbors, validate_triangle_neighbor_rows,
};
mod refine_loop;
pub use refine_loop::{
    redgreen_mesh_from_method_c, refine_redgreen_round_inside, refine_redgreen_round_one_based,
    RedGreenMesh, RedGreenOutcome, RedGreenSettings,
};
mod refine_num_ref;
pub use refine_num_ref::refine_num_ref_cal_one_based;
mod refine_onedivide_four_renew;
pub use refine_onedivide_four_renew::refine_onedivide_four_renew_one_based;
mod refine_onedivide_two;
pub use refine_onedivide_two::refine_onedivide_two_one_based;
mod refine_onedivide_four_connection;
pub use refine_onedivide_four_connection::refine_onedivide_four_connection_one_based;
mod refine_lop;
pub use refine_lop::refine_delaunay_lop_one_based;
mod refine_lop_pair;
pub use refine_lop_pair::refine_m1w1_to_m11w11_one_based;
mod refine_lop_sharp;
pub use refine_lop_sharp::refine_sharp_concav_lop_judge_one_based;
mod refine_lop_weak;
pub use refine_lop_weak::refine_weak_concav_lop_judge_one_based;
mod refine_lop_weak_pair;
pub use refine_lop_weak_pair::refine_weak_concav_pair_special_one_based;
mod refine_isreverse_judge;
pub use refine_isreverse_judge::refine_isreverse_judge_one_based;
mod refine_renewal;
pub use refine_renewal::refine_ngr_renew_one_based;
mod refine_renewal_core;
pub use refine_renewal_core::{refine_ngr_renew_core_one_based, RefineNgrRenewCore};
mod refine_boundary;
pub use refine_boundary::refine_boundary_closed_curves_one_based;
mod refine_boundary_connection;
pub use refine_boundary_connection::refine_boundary_connection_make_one_based;
mod refine_boundary_segments;
pub use refine_boundary_segments::refine_boundary_segments_one_based;
mod refine_boundary_segments_make;
pub use refine_boundary_segments_make::refine_boundary_segments_make_one_based;
mod refine_boundary_weak;
pub use refine_boundary_weak::refine_weak_concav_segment_make_one_based;
mod refine_edge_flip;
pub use refine_edge_flip::{checked_lop_edge_flip, CheckedEdgeFlip};
mod refine_hfield_marks;
pub use refine_hfield_marks::refine_marks_from_target_levels_one_based;
mod refine_subdivision_points;
pub use refine_subdivision_points::{
    average_lonlat3, check_crossing_canonical_lonlat, crossline_check_canonical, midpoint_lonlat,
};
mod refine_array_length;
pub use refine_array_length::{
    refine_array_length_calculation_one_based, refine_array_length_halo_one_based,
    RefineArrayLengthCalculation, RefineArrayLengthHalo,
};
mod get_sort_new;
pub use get_sort_new::get_sort_new_one_based;
