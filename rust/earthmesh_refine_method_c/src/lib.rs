//! Method-C nested refinement.
//!
//! Algorithm provenance: OLAM Method-C. Data ownership, validation, recovery,
//! and public interfaces are maintained by EarthMesh.
//!
//! Walko, R. L., & Avissar, R. (2011), *A direct method for constructing
//! refined regions in unstructured conforming triangular-hexagonal
//! computational grids: Application to OLAM*, Monthly Weather Review 139(12),
//! 3923-3937, doi:10.1175/MWR-D-11-00021.1.
//!
//! # What is here and what is not
//!
//! Here: the stride-3 seed lattice, the rad3 footprint, the mrow transition
//! rows, the five legality gates, the pass driver and its retries, the h-field
//! spawn, and the emit that renumbers a mesh after a pass.
//!
//! Not here, and in `earthmesh_mesh` instead: the mesh itself, its topology
//! validation, spherical geometry, spring relaxation, region containment,
//! gridfile round-tripping, and the rebuild from triangle seeds. None of that
//! is Method-C's, and twenty-one modules carried its name only because that is
//! where they were written.
//!
//! # `use super::*`
//!
//! The modules below reach the shared mesh through the glob re-export here,
//! which is what `use super::*` resolved to before the split. Keeping that
//! shape is why the move is a move: not one line inside the nesting had to
//! change to cross a crate boundary.

use std::{collections::BTreeSet, io};

// The nesting itself does not use these; the tests reach them through
// `use super::*`, which is the shape the modules were written in.
#[cfg(test)]
pub(crate) use method_c_nest_spring::method_c_nest_movable_m_points;
#[cfg(test)]
pub(crate) use method_c_nest_spring_iteration::method_c_nest_mrow_distance_multiplier;
#[cfg(test)]
use std::collections::BTreeMap;

// Was `pub use earthmesh_mesh::*;`. Named now: a wildcard makes this crate's
// public surface whatever `earthmesh_mesh` happens to hold, growing and
// shrinking without anyone editing this file, which is what the architecture
// gate forbids. The list is what callers actually reach through here, found by
// narrowing the export until the workspace stopped compiling.
pub use earthmesh_mesh::{
    active_mesh_radius, canonical_global_dist00, default_method_c_m_metadata,
    derive_cart_hex_m_neighbors_from_active_faces,
    derive_icosahedron_m_neighbors_canonical_checked,
    derive_icosahedron_m_neighbors_canonical_checked_with_prognostic,
    derive_icosahedron_u_neighbors_canonical, derive_icosahedron_w_neighbors_canonical, dot,
    face_following_two_vertices, face_following_vertex, icosahedron_spring_topology_canonical,
    lonlat_degrees_to_unit_xyz, magnitude, method_c_identity_prognostic_map,
    method_c_repairable_payload, normalize_cartesian_to_radius, normalized_face_center,
    refine_regions_close_to_method_c, refine_regions_contain_method_c, repairable_error,
    replace_w_face_edge_after, replace_w_face_edge_before, replace_w_face_edge_with_side_return,
    replace_w_face_edges_at, require_method_c_id, require_method_c_len,
    scale_refinement_regions_radius, set_first_two, tri_neighbors_outer_w_pair, vector_between,
    voronoi_grid_from_triangular_mesh, weighted_point, xyz_to_lonlat_degrees, CartesianPoint,
    IcosahedronDiamondConnectivity, IcosahedronMPointMetadata, IcosahedronMPointNeighbors,
    IcosahedronRelaxedGrid, IcosahedronSpringTopology, IcosahedronUEdge, IcosahedronWFace,
    LonLatDegrees, MethodCGridfileMetadata, RefinementRegion, RepairableKind, TriangularMesh,
};

mod lepp_delaunay;
pub use lepp_delaunay::{
    adaptive_hybrid_target_edge_from_level, find_lepp, improve_lepp_post_quality,
    insert_lepp_terminal_midpoint, insert_lepp_terminal_midpoint_constrained,
    refine_adaptive_hybrid, refine_adaptive_hybrid_constrained, refine_adaptive_hybrid_regions,
    spherical_edge_length, terminal_edge_midpoint, AdaptiveHybridConfig, AdaptiveHybridDemand,
    AdaptiveHybridError, AdaptiveHybridInsertionCounts, AdaptiveHybridPathStats,
    AdaptiveHybridRejection, AdaptiveHybridReport, AdaptiveHybridStopReason,
    AdaptiveHybridTargetSatisfaction, AdaptiveHybridUnresolvedDemand,
    AdaptiveHybridUnresolvedReason, FaceId, LeppEdgeId, LeppInsertionError, LeppInsertionGates,
    LeppInsertionReport, LeppInsertionSplitReason, LeppPath, LeppPostQualityConfig,
    LeppPostQualityError, LeppPostQualityRejection, LeppPostQualityReport,
    LeppPostQualityStopReason, LeppQualitySnapshot, LeppSearchConfig, LeppSearchError,
    LeppSearchReport, LeppTerminal,
};
mod method_c_mesh;
pub use method_c_lattice_mask::METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS;
pub use method_c_mesh::MethodCMesh;
pub use method_c_selection::method_c_connected_region_groups;
mod method_c_dump;
mod method_c_emit;
mod method_c_full;
mod method_c_tables;
pub(crate) use method_c_tables::*;
mod method_c_lattice_mask;
mod method_c_mask_annealing;
mod method_c_parent_mrlw_validation;
mod method_c_patch;
mod method_c_perimeter;
mod method_c_perimeter_mrows;
mod method_c_perimeter_repair;
mod method_c_perimeter_repair_candidates;
mod method_c_perimeter_repair_grow;
mod method_c_perimeter_repair_shrink;
mod method_c_perimeter_selection;
mod method_c_selection;
mod method_c_selection_fill;
mod method_c_selection_march;
mod method_c_selection_start;
mod method_c_selection_topology;
mod method_c_spawn;
mod method_c_spawn_hfield;
pub use method_c_spawn_hfield::MethodCHfieldSpawnDiagnostics;
mod method_c_spawn_internal;
mod method_c_spawn_pass;
mod method_c_spawn_retry;
pub(crate) mod method_c_spawn_retry_scaled;
mod method_c_table_helpers;
pub use method_c_nest_spring::method_c_edge_target_lengths_from_field;
mod method_c_nest_spring;
pub(crate) use method_c_nest_spring_iteration::{
    method_c_nest_spring_iteration_into, MethodCNestSpringScratch,
};
mod method_c_nest_spring_iteration;
pub(crate) use method_c_table_helpers::{
    canonical_other_endpoint_by_first, fill_missing_endpoint, method_c_split_outer_edges,
    other_edge_face,
};

#[cfg(test)]
mod tests;
