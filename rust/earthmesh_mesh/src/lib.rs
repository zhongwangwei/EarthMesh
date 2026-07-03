//! Rust mesh kernels migrated from EarthMesh Fortran.
//!
//! Clippy lints that fire on Fortran-mirroring patterns (multi-arg signatures, 1-based
//! index loops, connectivity-table types) are allowed package-wide in `Cargo.toml`.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use earthmesh_core::{deg_to_rad, rad_to_deg};

mod coordinates;
pub use coordinates::*;
use coordinates::{cross, dot, magnitude, normalize_cartesian_to_radius, vector_between};
mod grid_xyz2lonlat;
pub use grid_xyz2lonlat::*;
mod distance_layers;
pub use distance_layers::*;
mod distance_layer_updates;
pub(crate) use distance_layer_updates::boundary_cells_from_triangle_flags;
pub use distance_layer_updates::*;
mod cellwidth_layer_updates;
pub use cellwidth_layer_updates::*;
mod distance_global;
pub use distance_global::*;
mod edge_connectivity;
pub use edge_connectivity::*;
mod edge_distance_angle;
pub use edge_distance_angle::*;
mod edge_id_sort;
pub use edge_id_sort::*;
mod edge_weights;
pub use edge_weights::*;
mod mesh_vertex_ordering;
pub use mesh_vertex_ordering::*;
mod mesh_vertex_rotation;
pub use mesh_vertex_rotation::*;
mod mesh_vertex_array_ordering;
pub use mesh_vertex_array_ordering::*;
mod mesh_cell_vertex_shared_edges;
pub use mesh_cell_vertex_shared_edges::*;
mod mesh_cell_vertex_ordering;
pub use mesh_cell_vertex_ordering::*;
mod mesh_triangle_topology;
pub use mesh_triangle_topology::*;
mod mesh_edge_topology;
pub use mesh_edge_topology::*;
mod mesh_edge_adjacency;
pub use mesh_edge_adjacency::*;
mod mesh_edge_production;
pub use mesh_edge_production::*;
mod mesh_area_primitives;
pub use mesh_area_primitives::*;
mod mesh_area;
pub use mesh_area::*;
mod mesh_area_production;
pub use mesh_area_production::*;
mod mesh_metrics;
pub use mesh_metrics::*;
mod mesh_spherical_area;
pub use mesh_spherical_area::*;
mod mesh_quality_fortran;
pub use mesh_quality_fortran::*;
mod mesh_quality_polygon;
pub use mesh_quality_polygon::*;
mod mesh_quality_global;
pub use mesh_quality_global::*;
mod mask_postproc_reindex;
pub use mask_postproc_reindex::*;
mod mask_postproc_data;
pub use mask_postproc_data::*;
mod mask_postproc_final_data;
pub use mask_postproc_final_data::*;
mod mask_postproc_domain;
pub use mask_postproc_domain::*;
mod mask_postproc_opposite_domain;
pub use mask_postproc_opposite_domain::*;
mod mask_postproc_boundary;
pub(crate) use mask_postproc_boundary::push_boundary_neighbor;
pub use mask_postproc_boundary::*;
mod mask_postproc_boundary_connection;
pub use mask_postproc_boundary_connection::*;
mod mask_postproc_orders;
pub use mask_postproc_orders::*;
mod mask_postproc_waterway;
pub use mask_postproc_waterway::*;
mod mask_postproc_isolated;
pub use mask_postproc_isolated::*;
mod mask_postproc;
pub(crate) use mask_postproc::require_vertex_count;
pub use mask_postproc::*;
mod refine_renewal;
pub use refine_renewal::*;
mod refine_renewal_core;
pub use refine_renewal_core::*;
mod get_sort_new;
pub use get_sort_new::*;
mod refine_array_length;
pub use refine_array_length::*;
mod refine_hfield_marks;
pub use refine_hfield_marks::*;
mod refine_iter;
pub use refine_iter::*;
mod refine_iter_helpers;
pub(crate) use refine_iter_helpers::{
    unique_triangle_cell, validate_refine_cell_neighbors, validate_triangle_neighbor_rows,
};
mod refine_iter_c;
pub use refine_iter_c::*;
mod refine_iter_d;
pub use refine_iter_d::*;
mod refine_iter_e;
pub use refine_iter_e::*;
mod refine_iter_f;
pub use refine_iter_f::*;
mod refine_iter_g;
pub use refine_iter_g::*;
mod refine_onedivide_four;
pub use refine_onedivide_four::*;
mod refine_onedivide_four_connection;
pub use refine_onedivide_four_connection::*;
mod refine_isreverse_judge;
pub use refine_isreverse_judge::*;
mod refine_onedivide_two;
pub use refine_onedivide_two::*;
mod refine_lop;
pub use refine_lop::*;
mod refine_lop_pair;
pub use refine_lop_pair::*;
mod refine_lop_weak_pair;
pub use refine_lop_weak_pair::*;
mod refine_lop_weak;
pub use refine_lop_weak::*;
mod refine_lop_sharp;
pub use refine_lop_sharp::*;
mod refine_subdivision_points;
pub(crate) use refine_subdivision_points::{
    average_lonlat3, check_crossing_fortran_lonlat, crossline_check_fortran, midpoint_lonlat,
};
mod refine_boundary_segments;
pub(crate) use refine_boundary_segments::refine_boundary_segments_fortran_indexed;
mod refine_boundary_segments_make;
pub use refine_boundary_segments_make::*;
mod refine_boundary_weak;
pub use refine_boundary_weak::*;
mod refine_boundary_connection;
pub use refine_boundary_connection::*;
mod refine_boundary;
pub(crate) use refine_boundary::refine_boundary_closed_curves_fortran_indexed;
mod gridinit;
pub use gridinit::*;
mod icosahedron_types;
pub use icosahedron_types::*;
mod icosahedron_initial;
pub(crate) use icosahedron_initial::olam_fortran_global_dist00;
pub use icosahedron_initial::*;
mod icosahedron_diamonds;
pub use icosahedron_diamonds::*;
mod icosahedron_spring_topology;
pub use icosahedron_spring_topology::*;
mod icosahedron_spring_grid;
pub use icosahedron_spring_grid::*;
mod icosahedron_m_neighbors;
#[cfg(test)]
pub(crate) use icosahedron_m_neighbors::derive_icosahedron_m_neighbors_fortran_checked;
pub use icosahedron_m_neighbors::*;
mod icosahedron_neighbors;
pub(crate) use icosahedron_m_neighbors::derive_icosahedron_m_neighbors_fortran_checked_with_prognostic;
pub(crate) use icosahedron_neighbors::tri_neighbors_outer_w_pair;
pub use icosahedron_neighbors::*;
mod icosahedron_u_neighbors;
pub use icosahedron_u_neighbors::*;
mod icosahedron_grid;
pub use icosahedron_grid::*;
mod spherical_projection;
pub use spherical_projection::*;
pub(crate) use spherical_projection::{
    project_to_polar_stereographic_with_radius, unproject_from_polar_stereographic_with_radius,
};
mod spherical_centroid;
pub use spherical_centroid::*;
mod spherical_circumcenter;
pub(crate) use spherical_circumcenter::spherical_circumcenter_from_barycenter_with_radius;
pub use spherical_circumcenter::*;
mod spherical_circumcenter_mesh;
pub use spherical_circumcenter_mesh::*;
mod spring_edge_dynamics;
pub use spring_edge_dynamics::*;
mod spring_dynamics;
pub use spring_dynamics::*;
mod spring_regional_dynamics;
pub use spring_regional_dynamics::*;
mod spring_types;
pub use spring_types::*;
mod spring_adjustment_types;
pub use spring_adjustment_types::*;
mod spring_masks;
pub use spring_masks::*;
mod spring_pipeline;
pub(crate) use spring_pipeline::spring_global_debug;
mod spring_global_core;
pub use spring_global_core::*;
mod spring_regional_core;
pub use spring_regional_core::*;
mod spring_regional_wrappers;
pub use spring_regional_wrappers::*;
mod area_judge;
pub(crate) use area_judge::{
    expand_triangles_from_boundary_fortran_indexed, source_find_lat_fortran_indexed,
    source_find_lon_fortran_indexed,
};
mod area_judge_closed_curve;
pub use area_judge_closed_curve::*;
mod area_judge_source;
pub use area_judge_source::*;
mod area_judge_mask_patch;
pub use area_judge_mask_patch::*;
mod voronoi_grid;
pub use voronoi_grid::*;
mod voronoi_pcvt;
pub use voronoi_pcvt::*;
mod voronoi_gridinit;
pub use voronoi_gridinit::*;
mod olam_mesh;
pub use olam_mesh::OlamDelaunayMesh;
mod olam_mesh_gridfile;
mod olam_regions;
pub use olam_regions::OlamRefinementRegion;
pub(crate) use olam_regions::*;
mod olam_region_geometry;
mod olam_region_selection;
mod olam_region_validation;
pub(crate) use olam_region_geometry::*;
mod olam_expansion;
mod olam_topology;
pub use olam_topology::OlamTopologyValidation;
mod olam_cart_hex;
mod olam_cart_hex_neighbors;
pub(crate) use olam_cart_hex_neighbors::fill_cart_hex_w_face_neighbors_from_edges;
mod olam_cart_hex_outer_pair;
pub(crate) use olam_cart_hex_outer_pair::order_olam_outer_w_pair_for_fill_rad3;
mod olam_cart_hex_incidence;
mod olam_checks;
mod olam_distance;
mod olam_dump;
mod olam_emit;
mod olam_geometry;
mod olam_incidence;
mod olam_incidence_ring;
mod olam_method_c_full;
mod olam_method_c_tables;
pub(crate) use olam_method_c_tables::*;
mod olam_mask_annealing;
mod olam_method_c_patch;
mod olam_parent_mrlw_validation;
mod olam_perimeter;
mod olam_perimeter_mrows;
mod olam_perimeter_repair;
mod olam_perimeter_repair_candidates;
mod olam_perimeter_repair_grow;
mod olam_perimeter_repair_shrink;
mod olam_perimeter_selection;
mod olam_point_interpolation;
mod olam_rebuild;
mod olam_rebuild_metadata;
mod olam_rebuild_neighbors;
mod olam_rebuild_seeds;
mod olam_region_corridor;
mod olam_selection;
mod olam_selection_fill;
mod olam_selection_march;
mod olam_selection_start;
mod olam_selection_topology;
mod olam_spawn;
mod olam_spawn_hfield;
mod olam_spawn_internal;
mod olam_spawn_pass;
mod olam_spawn_retry;
mod olam_spawn_retry_scaled;
mod olam_table_helpers;
pub(crate) use olam_cart_hex_incidence::derive_cart_hex_m_neighbors_from_active_faces;
pub(crate) use olam_checks::{require_olam_id, require_olam_len, require_unique_active_triplet};
pub(crate) use olam_distance::{
    olam_corridor_segment_distance_meters, olam_ec_ps_distance_meters, plane_segment_distance,
};
pub(crate) use olam_geometry::{
    face_following_two_vertices, face_following_vertex, lookup_olam_midpoint, lookup_olam_thirds,
    olam_edge_key, validate_lonlat, validate_positive_distance,
};
pub(crate) use olam_incidence::derive_olam_m_neighbors_from_incidence;
pub(crate) use olam_point_interpolation::{
    normalized_face_center, normalized_weighted_point, weighted_point,
};
pub(crate) use olam_rebuild::olam_mesh_from_triangle_seeds;
pub(crate) use olam_rebuild_metadata::{
    default_olam_m_metadata, derive_olam_m_metadata_from_w_faces, olam_identity_prognostic_map,
};
pub(crate) use olam_rebuild_neighbors::fill_olam_w_face_neighbors_from_edges;
pub(crate) use olam_region_corridor::{
    olam_cartesian_xy_segment_distance, olam_closed_corridor_contains_cartesian,
    olam_corridor_radius_at_segment, olam_open_corridor_contains_cartesian,
};
mod olam_spring;
pub use olam_nest_spring::olam_edge_target_lengths_from_field;
#[cfg(test)]
pub(crate) use olam_nest_spring::olam_nest_movable_m_points;
pub(crate) use olam_spring::active_mesh_radius;
mod olam_nest_spring;
mod olam_spring_iteration;
#[cfg(test)]
pub(crate) use olam_nest_spring_iteration::olam_nest_mrow_distance_multiplier;
pub(crate) use olam_nest_spring_iteration::{
    olam_nest_spring_iteration_into, OlamNestSpringScratch,
};
mod olam_nest_spring_iteration;
pub(crate) use olam_spring_iteration::{
    olam_global_spring_iteration_into, OlamGlobalSpringScratch,
};
pub(crate) use olam_table_helpers::{
    fill_missing_endpoint, fortran_other_endpoint_by_first, method_c_split_outer_edges,
    other_edge_face, set_first_two,
};
mod olam_w_face_edge_replacement;
pub(crate) use olam_w_face_edge_replacement::{
    replace_w_face_edge_after, replace_w_face_edge_before, replace_w_face_edge_with_side_return,
    replace_w_face_edges_at,
};

#[cfg(test)]
mod tests;
