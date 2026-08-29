//! Geometry, topology, refinement, smoothing, and mesh-quality kernels for
//! EarthMesh.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use earthmesh_core::{deg_to_rad, rad_to_deg};

/// Configure the worker count used by EarthMesh's parallel numeric kernels.
///
/// The CLI calls this once, before any mesh work starts, using `NL%openmp`.
pub fn configure_global_thread_pool(thread_count: usize) -> io::Result<()> {
    if thread_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "EarthMesh worker count must be positive",
        ));
    }
    rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build_global()
        .map_err(|error| {
            io::Error::other(format!("failed to configure EarthMesh workers: {error}"))
        })
}

mod coordinates;
pub use coordinates::{cross, dot, magnitude, normalize_cartesian_to_radius, vector_between};
pub use coordinates::{
    lonlat_degrees_to_unit_xyz, lonlat_points_to_unit_xyz, xyz_points_to_lonlat_degrees,
    xyz_to_lonlat_degrees, CartesianPoint, CartesianPointF32, LonLatDegrees,
};
mod grid_xyz2lonlat;
pub use grid_xyz2lonlat::{
    grid_cartesian_xy_to_lonlat_placeholders_one_based_state, grid_xyz2lonlat_one_based_state,
    grid_xyz2lonlat_state,
};
mod distance_layers;
pub use distance_layers::{
    distance_layers, find_frac_index_canonical, CanonicalFracIndex, DistanceLayerSpacing,
};
mod distance_layer_updates;
pub(crate) use distance_layer_updates::boundary_cells_from_triangle_flags;
pub use distance_layer_updates::dists_on_edge_layers_one_based;
mod cellwidth_layer_updates;
pub use cellwidth_layer_updates::cellwidth_layers_one_based;
mod distance_global;
pub use distance_global::{
    set_dists_on_edge_global_one_based, GlobalDistanceStep, SetDistsOnEdgeGlobalInput,
    SetDistsOnEdgeGlobalOutput,
};
mod edge_connectivity;
pub use edge_connectivity::{connect_on_cell_one_based, CellConnectivityOnCell};
mod edge_distance_angle;
pub use edge_distance_angle::{
    edge_distance_angle_one_based, plane_angle_signed, EdgeDistanceAngleOutput,
};
mod edge_id_sort;
pub use edge_id_sort::{edge_id_sort_one_based, EdgeIdSortOutput};
mod edge_weights;
pub use edge_weights::{set_weights_on_edge_one_based, WeightsOnEdgeOutput};
mod mesh_vertex_ordering;
pub use mesh_vertex_ordering::{
    next_ccw_edge_candidate_slot, normalize_lon_m180_180, order_vertices_on_edge_one_based,
    should_swap_vertices_on_edge,
};
mod mesh_vertex_rotation;
pub use mesh_vertex_rotation::{
    normalize_vertex_rotation, standardize_vertices_on_cell_rotation_one_based,
};
mod mesh_vertex_array_ordering;
pub use mesh_vertex_array_ordering::{
    order_vertex_arrays_for_vertex, order_vertex_arrays_one_based, OrderedVertexArrays,
    OrderedVertexArraysOutput,
};
mod mesh_cell_vertex_shared_edges;
pub use mesh_cell_vertex_shared_edges::order_vertices_on_cell_by_shared_edges_one_based;
mod mesh_cell_vertex_ordering;
pub use mesh_cell_vertex_ordering::order_vertices_on_cell_one_based;
mod mesh_cart_hex_neighbors;
mod mesh_cart_hex_outer_pair;
pub(crate) use mesh_cart_hex_neighbors::fill_cart_hex_w_face_neighbors_from_edges;
pub(crate) use mesh_cart_hex_outer_pair::order_method_c_outer_w_pair_for_fill_rad3;
mod mesh_geometry;
pub use mesh_geometry::{
    face_following_two_vertices, face_following_vertex, lookup_method_c_midpoint,
    lookup_method_c_thirds, method_c_edge_key, validate_lonlat, validate_positive_distance,
};
mod mesh_repairable_error;
pub use mesh_repairable_error::{
    method_c_repairable_payload, repairable_error, set_first_two, MethodCRepairableError,
    RepairableKind,
};
mod mesh_triangle_seed;
pub use mesh_triangle_seed::MethodCTriangleSeed;
mod mesh_flip;
pub use mesh_flip::FlipError;
pub use spherical_circumcenter_mesh::circumcenter_is_local_enough;
mod mesh_insertion;
mod mesh_patch;
mod mesh_predicates;
mod mesh_retirement;
mod mesh_state;
mod mesh_tables;
mod mesh_triangle_topology;
mod mesh_voronoi;
pub use mesh_triangle_topology::{
    cells_on_edge_from_neighbor_cells, is_ngrmm, triangle_neighbors_from_cell_membership_one_based,
};
mod mesh_edge_topology;
pub use mesh_edge_topology::{
    get_edge_connectivity_one_based, shared_cell_for_edge_pair, vertex_cell_position,
    GetEdgeConnectivity,
};
mod mesh_edge_adjacency;
pub use mesh_edge_adjacency::edges_on_edge_tri_one_based;
mod mesh_edge_production;
pub use mesh_edge_production::{
    edge_midpoints_from_cells_one_based, get_edge_production_one_based, GetEdgeProductionOutput,
};
mod mesh_area_primitives;
pub use mesh_area_primitives::{
    arc_length_unit_sphere, spherical_cell_area_from_vertices_unit, spherical_kite_area_unit,
    spherical_triangle_area_unit,
};
mod mesh_area;
pub use mesh_area::{
    get_area_unit_one_based, AreaTriangleReconstructionError, GetAreaUnitInput, GetAreaUnitOutput,
};
mod mesh_area_production;
pub use mesh_area_production::{
    area_triangle_reconstruction_error_one_based, get_area_production_one_based,
    GetAreaProductionOutput,
};
mod mesh_metrics;
pub use mesh_metrics::{
    polygon_length_angle_metrics, polygon_mesh_quality, triangle_mesh_quality, MeshQualitySummary,
    PolygonLengthAngleMetrics,
};
mod mesh_spherical_area;
pub use mesh_spherical_area::robust_spherical_area_unit;
mod mesh_quality_metrics;
pub use mesh_quality_metrics::{
    triangle_mesh_quality_metrics_indexed, TriangleMeshQualityCanonicalOutput,
};
mod mesh_quality_polygon;
pub use mesh_quality_polygon::{
    polygon_mesh_quality_metrics_indexed, PolygonMeshQualityCanonicalOutput,
};
mod mesh_quality_global;
pub use mesh_quality_global::{
    grid_quality_check_global_one_based, GridQualityGlobalOutput, PolygonEdgeClassCounts,
};
mod mask_postproc_reindex;
pub use mask_postproc_reindex::{
    extract_unique_vertices_one_based, reindex_final_center_vertices_one_based,
    sort_and_reindex_vertices, VertexReindex,
};
mod mask_postproc_data;
pub use mask_postproc_data::{renew_mask_postproc_data_one_based, MaskPostprocRenewedData};
mod mask_postproc_final_data;
pub use mask_postproc_final_data::{finalize_mask_postproc_data_one_based, MaskPostprocFinalData};
mod mask_postproc_domain;
pub use mask_postproc_domain::renew_mask_postproc_domain_triangles_one_based;
mod mask_postproc_opposite_domain;
pub use mask_postproc_opposite_domain::renew_mask_postproc_opposite_domain_triangles_one_based;
mod mask_postproc_boundary;
// Public for the `earthmesh_refine_redgreen` compatibility crate in `extends/`,
// which ports the boundary walk this helper backs.
pub use mask_postproc_boundary::push_boundary_neighbor;
pub use mask_postproc_boundary::{boundary_closed_curves_one_based, BoundaryClosedCurves};
mod mask_postproc_boundary_connection;
pub use mask_postproc_boundary_connection::{boundary_connection_one_based, BoundaryConnection};
mod mask_postproc_orders;
pub use mask_postproc_orders::{classify_boundary_orders_one_based, BoundaryOrders};
mod mask_postproc_waterway;
pub use mask_postproc_waterway::{
    fill_vertex_only_ocean_contacts_one_based, widen_narrow_waterway_one_based,
};
mod mask_postproc_isolated;
pub use mask_postproc_isolated::{remove_isolated_ocean_one_based, IsolatedOceanRenewal};
mod mask_postproc_components;
pub use mask_postproc_components::{
    retain_edge_connected_components_with_hard_demand_one_based,
    retain_largest_edge_connected_component_one_based, LargestComponentRetention,
};
mod mask_postproc;
pub(crate) use mask_postproc::require_vertex_count;
pub use mask_postproc::{RefineBoundarySegments, RefineWeakConcavitySegments};
mod gridinit;
pub use gridinit::{method_c_gridinit_factorization_canonical, MethodCGridinitFactors};
mod icosahedron_types;
pub use icosahedron_types::{
    IcosahedronCounts, IcosahedronDiamondConnectivity, IcosahedronDiamondCorners,
    IcosahedronInitialGrid, IcosahedronMPointMetadata, IcosahedronMPointNeighbors,
    IcosahedronRelaxedGrid, IcosahedronSpringDynamicsOutput, IcosahedronSpringIterationOutput,
    IcosahedronSpringTopology, IcosahedronUEdge, IcosahedronWFace,
};
mod icosahedron_initial;
pub use icosahedron_initial::canonical_global_dist00;
pub use icosahedron_initial::{
    icosahedron_counts_canonical, icosahedron_diamond_corners_canonical,
    icosahedron_initial_grid_canonical, ICOSAHEDRON_MLOOPS, METHOD_C_CANONICAL_EARTH_RADIUS_METERS,
};
mod icosahedron_diamonds;
pub use icosahedron_diamonds::icosahedron_fill_diamonds_canonical;
mod icosahedron_spring_topology;
pub use icosahedron_spring_topology::icosahedron_spring_topology_canonical;
mod icosahedron_spring_grid;
pub use icosahedron_spring_grid::{
    icosahedron_spring_dynamics1_canonical, icosahedron_spring_iteration_canonical,
};
mod icosahedron_m_neighbors;
pub use icosahedron_m_neighbors::derive_icosahedron_m_neighbors_canonical;
// Not test-gated: Method-C's selection calls it, and a `#[cfg(test)]` export
// is invisible to another crate's tests as well as to its production code.
pub use icosahedron_m_neighbors::derive_icosahedron_m_neighbors_canonical_checked;
mod icosahedron_neighbors;
pub use icosahedron_m_neighbors::derive_icosahedron_m_neighbors_canonical_checked_with_prognostic;
pub use icosahedron_neighbors::tri_neighbors_outer_w_pair;
pub use icosahedron_neighbors::{
    derive_icosahedron_tri_neighbors_canonical, derive_icosahedron_w_neighbors_canonical,
};
mod icosahedron_u_neighbors;
pub use icosahedron_u_neighbors::derive_icosahedron_u_neighbors_canonical;
mod icosahedron_grid;
pub use icosahedron_grid::{
    apply_icosahedron_loop_flags_canonical, icosahedron_relaxed_grid_canonical,
};
mod spherical_projection;
pub use spherical_projection::{
    project_to_polar_stereographic, project_to_polar_stereographic_f32,
    unproject_from_polar_stereographic, unproject_from_polar_stereographic_f32, PlanePoint,
    PlanePointF32, PoleBasis, PoleBasisF32,
};
mod spherical_centroid;
pub use spherical_centroid::{centroid_spherical_mesh_one_based, spherical_centroid_degrees};
mod spherical_circumcenter;
pub use spherical_circumcenter::spherical_circumcenter_from_barycenter;
pub(crate) use spherical_circumcenter::spherical_circumcenter_from_barycenter_with_radius;
mod spherical_circumcenter_mesh;
pub use spherical_circumcenter_mesh::circumcenter_spherical_mesh_one_based;
mod spring_edge_dynamics;
pub use spring_edge_dynamics::{
    spring_apply_cell_displacements_one_based, spring_edge_adjustment_canonical,
    spring_edge_directions_one_based,
};
mod spring_dynamics;
pub use spring_dynamics::{spring_dynamics_global_one_based, spring_global_iteration_one_based};
mod spring_regional_dynamics;
pub use spring_regional_dynamics::spring_dynamics_regional_one_based;
mod spring_types;
pub use spring_types::{
    RefineRegionalMaskInput, RegionalMoveMaskInput, RegionalMoveMaskOutput,
    SpringDiagnosticMaxDisplacement, SpringDynamicsGlobalOutput, SpringDynamicsRegionalOutput,
    SpringEdgeAdjustment, SpringGlobalIterationOutput,
};
mod spring_adjustment_types;
pub use spring_adjustment_types::{
    SpringjustmentGlobalCoreInput, SpringjustmentGlobalCoreOutput, SpringjustmentRegionalCoreInput,
    SpringjustmentRegionalCoreOutput, SpringjustmentRegionalFromRefinementInput,
    SpringjustmentRegionalFromRefinementOutput, SpringjustmentRegionalFromSourceMaskInput,
    SpringjustmentRegionalFromSourceMaskOutput,
};
mod spring_masks;
pub use spring_masks::{refine_sjx_regional_make_one_based, set_dbx_move_regional_step_one_based};
mod spring_pipeline;
pub(crate) use spring_pipeline::spring_global_debug;
mod spring_global_core;
pub use spring_global_core::springjustment_global_core_one_based;
mod spring_regional_core;
pub use spring_regional_core::springjustment_regional_core_one_based;
mod spring_regional_wrappers;
pub use spring_regional_wrappers::{
    springjustment_regional_from_refinement_one_based,
    springjustment_regional_from_source_mask_one_based,
};
mod area_judge;
pub(crate) use area_judge::{
    expand_triangles_from_boundary_one_based, source_find_lat_one_based, source_find_lon_one_based,
};
mod area_judge_closed_curve;
pub use area_judge_closed_curve::{
    area_judge_closed_curve_fill_one_based, AreaJudgeClosedCurveFill,
};
mod area_judge_source;
pub use area_judge_source::{
    area_judge_minmax_range_make_one_based, area_judge_source_find_one_based, AreaJudgeAxis,
    AreaJudgeSourceBounds,
};
mod area_judge_mask_patch;
pub use area_judge_mask_patch::{area_judge_apply_mask_patch_one_based, AreaJudgeMaskPatchReport};
mod voronoi_grid;
pub use voronoi_grid::{
    voronoi_grid_from_icosahedron_relaxed, voronoi_grid_from_triangular_mesh,
    voronoi_grid_from_triangular_mesh_cartesian, VoronoiGridState,
};
mod voronoi_pcvt;
pub use voronoi_pcvt::pcvt_adjust_voronoi_grid_state;
mod voronoi_gridinit;
pub use voronoi_gridinit::gridinit_voronoi_state_canonical;
mod primal_dual_mesh;
pub use mesh_insertion::{
    DegreeForecast, Encroachment, InsertionError, InsertionReport, InsertionTransactionError,
};
pub use mesh_patch::{MeshPatch, PatchError};
pub use mesh_predicates::{in_circle_on_sphere, orient3d, orientation_on_sphere, Ambiguous, Sign};
pub use mesh_retirement::{
    RetirementDiagonal, RetirementError, RetirementPostconditionOutcome, RetirementReport,
    RetirementSearchOutcome,
};
pub use mesh_state::{EdgeId, FaceId, MeshState, MeshStateError, VertexId, MESH_STATE_FIRST_ID};
pub use mesh_voronoi::{VoronoiCell, VoronoiError};
pub use primal_dual_mesh::TriangularMesh;
mod mesh_from_gridfile;
pub use mesh_from_gridfile::MethodCGridfileMetadata;
mod refine_regions;
pub use refine_regions::scale_refinement_regions_radius;
pub use refine_regions::RefinementRegion;
pub(crate) use refine_regions::METHOD_C_MIN_GRID_SPACING_METERS;
mod refine_region_geometry;
pub use refine_region_geometry::{
    refine_regions_close_to_method_c, refine_regions_contain_method_c,
};
mod mesh_expansion;
mod mesh_topology_validation;
mod refine_region_selection;
pub use refine_region_selection::RefinementRegionIndex;
mod refine_region_validation;
pub use mesh_topology_validation::MethodCTopologyValidation;
mod mesh_cart_hex;
mod mesh_cart_hex_incidence;
mod mesh_incidence;
mod mesh_incidence_ring;
mod mesh_point_interpolation;
mod mesh_rebuild;
mod mesh_rebuild_metadata;
mod mesh_rebuild_neighbors;
mod mesh_rebuild_seeds;
mod mesh_table_checks;
mod refine_region_corridor;
mod refine_region_distance;
pub use mesh_cart_hex_incidence::derive_cart_hex_m_neighbors_from_active_faces;
pub(crate) use mesh_incidence::derive_method_c_m_neighbors_from_incidence;
pub use mesh_point_interpolation::{
    normalized_face_center, normalized_weighted_point, weighted_point,
};
pub(crate) use mesh_rebuild::method_c_mesh_from_triangle_seeds;
pub use mesh_rebuild_metadata::{
    default_method_c_m_metadata, derive_method_c_m_metadata_from_w_faces,
    method_c_identity_prognostic_map,
};
pub(crate) use mesh_rebuild_neighbors::fill_method_c_w_face_neighbors_from_edges;
pub use mesh_table_checks::{
    require_method_c_id, require_method_c_len, require_unique_active_triplet,
};
pub(crate) use refine_region_corridor::{
    method_c_cartesian_xy_segment_distance, method_c_closed_corridor_contains_cartesian,
    method_c_corridor_radius_at_segment, method_c_open_corridor_contains_cartesian,
};
pub(crate) use refine_region_distance::{
    method_c_corridor_segment_distance_meters, method_c_ec_ps_distance_meters,
    plane_segment_distance,
};
mod mesh_spring;
pub use mesh_spring::active_mesh_radius;
mod mesh_spring_iteration;
pub(crate) use mesh_spring_iteration::{
    method_c_global_spring_iteration_into, MethodCGlobalSpringScratch,
};
mod mesh_w_face_edge_replacement;
pub use mesh_w_face_edge_replacement::{
    replace_w_face_edge_after, replace_w_face_edge_before, replace_w_face_edge_with_side_return,
    replace_w_face_edges_at,
};

#[cfg(test)]
mod tests;
