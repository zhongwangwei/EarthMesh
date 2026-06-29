//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use earthmesh_core::{MkgrdWorkspacePlan, RefineConfig};
use earthmesh_mesh::{
    connect_on_cell_fortran_indexed, edge_distance_angle_fortran_indexed,
    get_area_production_fortran_indexed, get_edge_production_fortran_indexed,
    lonlat_points_to_unit_xyz, order_vertices_on_cell_by_shared_edges_fortran_indexed,
    order_vertices_on_cell_fortran_indexed, refine_boundary_segments_make_fortran_indexed,
    refine_iter_b_judge_fortran_indexed, refine_iter_c_judge_fortran_indexed,
    refine_iter_e_judge_fortran_indexed, refine_iter_g_judge_fortran_indexed,
    set_weights_on_edge_fortran_indexed, springjustment_global_core_fortran_indexed,
    springjustment_regional_core_fortran_indexed,
    standardize_vertices_on_cell_rotation_fortran_indexed,
    triangle_neighbors_from_cell_membership_fortran_indexed, AreaJudgeSourceBounds,
    GetAreaProductionOutput, GetAreaUnitInput, GetEdgeProductionOutput, LonLatDegrees,
    OlamRefinementRegion, SpringjustmentGlobalCoreInput, SpringjustmentGlobalCoreOutput,
    SpringjustmentRegionalCoreInput, SpringjustmentRegionalCoreOutput,
};

mod mask_source_discovery;
pub use mask_source_discovery::*;
pub(crate) use mask_source_discovery::{source_extension, unsupported_mask_source};
mod coordinate_types;
pub use coordinate_types::*;
mod fs_support;
pub(crate) use fs_support::ensure_parent_dir;
mod json_support;
pub(crate) use json_support::{
    geojson_feature_nodes, json_escape_string, json_node_to_f64, json_node_to_usize, json_number,
    json_string_array, json_string_usize_map, json_usize_f64_map, json_usize_f64_map_node,
    json_usize_map, JsonNode, JsonParser,
};
mod v3_data_source_io;
pub use v3_data_source_io::*;
mod mkgrd_compact_source_state;
pub use mkgrd_compact_source_state::*;
mod global_source_axes;
pub use global_source_axes::*;
mod unstructured_mesh_support;
pub(crate) use unstructured_mesh_support::mesh_row_for_fortran_id;
pub use unstructured_mesh_support::*;
pub(crate) use unstructured_mesh_support::{unstructured_dimc, validate_unstructured_mesh};
mod merit_tile_selection;
pub use merit_tile_selection::*;
mod merit_hydro_io;
pub use merit_hydro_io::*;
mod hydro_close_types;
pub use hydro_close_types::*;
mod hydro_close_buffer;
mod hydro_close_composite;
mod hydro_close_envelope_merge;
mod hydro_close_geometry;
mod hydro_close_geometry_utils;
mod hydro_close_hole_decomposition;
mod hydro_close_hole_slabs;
mod hydro_close_hole_spans;
mod hydro_close_masks;
mod hydro_close_proximity;
mod hydro_close_recipe;
mod merit_hydro_region_close;
pub use hydro_close_composite::write_hydro_composite_close_mask_nmls;
pub use hydro_close_masks::*;
pub use hydro_close_recipe::*;
pub use merit_hydro_region_close::write_merit_hydro_region_close_masks;
mod hydro_workflow_types;
pub use hydro_workflow_types::*;
mod hydro_refinement_eval;
pub use hydro_refinement_eval::*;
mod hydro_sweep;
pub use hydro_sweep::*;
mod hydro_delivery_cells;
mod hydro_delivery_colm;
mod hydro_delivery_common;
mod hydro_delivery_complete_mask;
mod hydro_delivery_coupling_quality;
mod hydro_delivery_intersections;
mod hydro_delivery_manifest;
mod hydro_delivery_qa;
mod hydro_delivery_refine_workflow;
pub use hydro_delivery_cells::*;
pub(crate) use hydro_delivery_cells::{
    convex_hull_order_indices, gridfile_lonlat_has_two_placeholders,
};
pub use hydro_delivery_colm::*;
pub(crate) use hydro_delivery_common::{
    format_coupling_number, read_text_maybe_gzip, HYDRO_EARTH_RADIUS_M,
};
pub use hydro_delivery_complete_mask::write_complete_cell_mask_geojson;
pub use hydro_delivery_coupling_quality::*;
pub use hydro_delivery_intersections::*;
pub(crate) use hydro_delivery_intersections::{geometry_outer_rings, json_node_to_string};
pub use hydro_delivery_manifest::*;
pub use hydro_delivery_qa::*;
pub use hydro_delivery_refine_workflow::*;
mod colm_types;
pub use colm_types::*;
mod colm_coupling_csv;
mod colm_coupling_netcdf;
mod colm_manifest_writer;
mod colm_surface_reader;
mod colm_template_writers;
mod netcdf_io;
pub(crate) use netcdf_io::{
    create_netcdf, first_existing_dimension_len, netcdf_to_io_error, open_netcdf,
    optional_values_i32_2d, required_dimension_len, required_scalar_usize_i32, required_values_f64,
    required_values_f64_any, required_values_f64_any_matrix, required_values_i32,
    required_values_i32_2d, required_values_i32_any_matrix, required_values_i32_matrix,
    required_values_i8, required_values_i8_matrix, write_f64_scalar, write_i32_scalar,
};
mod colm_package_io;
pub use colm_package_io::*;
mod cama_binary_io;
mod cama_binary_params;
mod cama_binary_window_readers;
mod cama_reach_inventory;
pub use cama_binary_io::*;
pub use cama_binary_params::*;
pub use cama_binary_window_readers::*;
pub use cama_reach_inventory::*;
mod coastal_band_io;
pub use coastal_band_io::*;
mod bbox_mask_io;
pub(crate) use bbox_mask_io::validate_bbox_mask;
pub use bbox_mask_io::*;
mod close_mesh_io;
pub use close_mesh_io::*;
mod circle_close_mask_io;
pub use circle_close_mask_io::*;
pub(crate) use circle_close_mask_io::{validate_circle_mask, validate_close_mask};
mod mode4mesh_make;
pub use mode4mesh_make::*;
mod mode_file_io;
pub use mode_file_io::*;
mod contain_io;
pub(crate) use contain_io::validate_contain_mesh;
pub use contain_io::*;
mod getcontain_types;
pub use getcontain_types::*;
mod getcontain_geometry;
pub use getcontain_geometry::*;
pub(crate) use getcontain_geometry::{
    getcontain_mesh_kind_from_mesh_type, getcontain_validate_source_matrix,
};
mod unstructured_mesh_io;
pub use unstructured_mesh_io::*;
mod mesh_conversion_support;
pub(crate) use mesh_conversion_support::{
    aggregate_getref_ref_sjx, cells_on_triangle_fortran_indexed_from_mesh,
    copy_getref_threshold_column, f64_matrix_width, flatten_i32_rows, get_getref_layer_value,
    i32_counts_as_usize, i32_matrix_from_flat, i32_rows_as_usize, lat_values, lon_values,
    lonlat_degrees_from_points, lonlat_pairs_from_points, lonlat_points_from_pairs, lookup_f64,
    m_to_w_as_usize_rows, matrix_width, n_edges_on_cell_usize_from_mesh, normalize_degrees,
    one_to_n_i32, parse_value_after_equals, patchtype_indices, rad_to_deg,
    require_getref_two_layer_values, require_len, rows_from_flat_i32,
    rows_to_triangle_connectivity, scale_cartesian_points_by_earth_radius,
    split_cartesian_components, triangles_on_cell_fortran_indexed_from_mesh,
    usize_from_i32_connectivity, usize_from_i32_nonnegative, usize_from_i32_positive,
    usize_rows_to_i32, usize_to_i32, usize_values_to_i32, validate_mask_postproc_layout,
    write_f64_1d, write_f64_matrix_rows, write_i32_1d, write_i32_matrix_rows, write_i32_pair_rows,
};
mod mesh_conversion_gridfile_state;
pub(crate) use mesh_conversion_gridfile_state::earthmesh_runtime_state_from_compact_mesh;
pub use mesh_conversion_gridfile_state::*;
mod mesh_conversion_iap;
pub(crate) use mesh_conversion_iap::derive_iap_w_to_m_fortran_indexed;
mod fvcom_mesh_writer;
pub(crate) use fvcom_mesh_writer::write_fvcom_ns_records;
pub use fvcom_mesh_writer::*;
mod obc_boundary_io;
pub use obc_boundary_io::*;
mod lambert_mode4_io;
pub(crate) use lambert_mode4_io::validate_mode4_mesh_for_area_judge;
pub use lambert_mode4_io::*;
mod area_judge_grid_io;
pub use area_judge_grid_io::*;
pub(crate) use area_judge_grid_io::{
    grid_covers_area_judge_bounds_fortran_indexed, validate_area_judge_grid_payload,
    validate_i32_matrix_shape,
};
mod area_judge_types;
pub use area_judge_types::*;
mod area_judge_domain_builders;
pub use area_judge_domain_builders::*;
mod area_judge_getcontain_refine;
pub use area_judge_getcontain_refine::*;
mod area_judge_refine_steps;
pub use area_judge_refine_steps::*;
mod area_judge_branch_builders;
pub use area_judge_branch_builders::*;
mod area_judge_grid_runs;
pub(crate) use area_judge_grid_runs::write_area_judge_selected_grid_report;
pub use area_judge_grid_runs::*;
mod area_judge_sources;
pub use area_judge_sources::*;
pub(crate) use area_judge_sources::{area_judge_area_source_path, merge_area_judge_source_bounds};
mod area_judge_bbox_sources;
pub use area_judge_bbox_sources::*;
mod area_judge_circle_sources;
pub use area_judge_circle_sources::*;
mod area_judge_close_sources;
pub use area_judge_close_sources::*;
pub(crate) use area_judge_close_sources::{
    area_judge_check_crossing, area_judge_close_crosses_dateline,
};
mod area_judge_lambert_sources;
pub use area_judge_lambert_sources::*;
mod area_judge_threshold_inputs;
pub use area_judge_threshold_inputs::*;
mod mask_postproc_writers;
pub use mask_postproc_writers::*;
mod mask_postproc_types;
pub use mask_postproc_types::*;
mod mask_postproc_atmos;
pub use mask_postproc_atmos::*;
mod mask_postproc_ocean;
pub use mask_postproc_ocean::*;
mod mask_postproc_patchtypes;
pub use mask_postproc_patchtypes::*;
mod mask_postproc_layout;
pub(crate) use mask_postproc_layout::ensure_leading_mask_postproc_placeholder;
pub use mask_postproc_layout::*;
mod mask_postproc_domain;
pub use mask_postproc_domain::*;
mod mesh_metric_writers;
pub use mesh_metric_writers::*;
mod quality_global_writer;
pub use quality_global_writer::*;
mod mpas_edge_reference_io;
pub use mpas_edge_reference_io::*;
mod mpas_mesh_types;
pub use mpas_mesh_types::*;
mod mpas_netcdf_rows;
mod mpas_regional_connectivity;
mod mpas_subset;
mod mpas_topology;
mod mpas_topology_checker;
pub use mpas_topology::*;
pub(crate) use mpas_topology::{
    mpas_lat_lon_radians, pad_f64_rows, validate_mpas_mesh, validate_mpas_simple_mesh,
    zero_based_padded_rows, zero_based_pair_rows, zero_based_triplet_rows,
};
mod mpas_graph_info_writer;
pub use mpas_graph_info_writer::*;
mod mpas_simple_writer;
pub use mpas_simple_writer::*;
mod mpas_full_writer;
pub use mpas_full_writer::write_mpas_mesh_netcdf;
mod mpas_unstructured_mesh_builders;
pub use mpas_unstructured_mesh_builders::*;
pub(crate) use mpas_unstructured_mesh_builders::{
    normalize_unstructured_mesh_legacy_placeholders, restore_unstructured_mesh_shape,
};
mod gridfile_output_writers;
pub use gridfile_output_writers::*;
mod mpas_gridfile_writers;
pub use mpas_gridfile_writers::*;
mod regional_gridfile_writers;
pub use regional_gridfile_writers::*;
mod getref_threshold_io;
pub use getref_threshold_io::*;
mod getref_types;
pub use getref_types::*;
mod getref_threshold_support;
pub(crate) use getref_threshold_support::require_getref_lookup_width;
mod getref_threshold_basic;
pub use getref_threshold_basic::calculate_getref_land_basic_fortran_indexed;
mod getref_threshold_statistics;
pub use getref_threshold_statistics::*;
mod getref_threshold_inputs;
pub use getref_threshold_inputs::*;
mod getref_threshold_loc;
pub use getref_threshold_loc::split_getref_loc_containment_fortran_indexed;
mod getref_threshold_land;
pub use getref_threshold_land::calculate_getref_land_threshold_report_fortran_indexed;
mod getref_threshold_ocean;
pub use getref_threshold_ocean::*;
mod getref_threshold_atmos;
pub use getref_threshold_atmos::*;
mod getref_threshold_aggregation;
pub use getref_threshold_aggregation::*;
mod getref_threshold_calculation;
pub use getref_threshold_calculation::*;
mod getref_threshold_runners;
pub use getref_threshold_runners::*;
mod getref_threshold_land_writer;
mod getref_threshold_ocean_atmos_writers;
mod getref_threshold_writer_helpers;
mod getref_threshold_writers;
pub use getref_threshold_writers::*;
pub(crate) use getref_threshold_writers::{
    validate_getref_atmos_threshold_report_for_aggregation,
    validate_getref_land_threshold_report_for_aggregation,
    validate_getref_ocean_threshold_report_for_aggregation,
};
mod mask_counts;
pub use mask_counts::*;
mod mask_operation_apply;
pub use mask_operation_apply::*;
mod refine_array_length_adapter;
pub use refine_array_length_adapter::*;
mod refine_loop_plan_types;
pub use refine_loop_plan_types::*;
mod refine_loop_types;
pub use refine_loop_types::*;
mod refine_loop_adapters;
pub(crate) use refine_loop_adapters::fortran_rows_to_triangle_major;
mod refine_loop_concavity_adapters;
pub use refine_loop_concavity_adapters::*;
mod refine_loop_onedivide_four;
pub use refine_loop_onedivide_four::*;
mod refine_loop_topology_adapters;
pub use refine_loop_topology_adapters::*;
mod refine_loop_handoff;
pub use refine_loop_handoff::*;
mod refine_loop_io_plan;
pub use refine_loop_io_plan::*;
pub(crate) use refine_loop_io_plan::{
    effective_mkgrd_refine_loop_io_plan, final_quality_non_negative_usize, mkgrd_tmpfile_path,
};
mod refine_loop_state_mesh;
mod refine_loop_transition_helpers;
mod refine_loop_working_state;
pub use refine_loop_working_state::*;
mod refine_loop_executor;
mod refine_loop_working_state_methods;
pub use refine_loop_executor::*;
mod refine_loop_source_executors;
pub use refine_loop_source_executors::*;
mod refine_loop_composite_executor;
pub use refine_loop_composite_executor::*;
mod mkgrd_refine_orchestration;
pub use mkgrd_refine_orchestration::*;
mod mkgrd_refine_source_orchestration;
pub(crate) use mkgrd_refine_source_orchestration::runtime_refine_from_prepare;
pub use mkgrd_refine_source_orchestration::*;
mod springjustment_gridfile_types;
pub use springjustment_gridfile_types::*;
mod grid_production_adapters;
mod grid_quality_global;
mod grid_quality_inputs;
mod grid_quality_pipeline;
mod springjustment_gridfile_adapters;
pub use grid_quality_pipeline::*;
mod mkgrd_quality_checks;
pub use mkgrd_quality_checks::*;
mod workspace_apply;
pub use workspace_apply::*;
mod workspace_mask_apply;
pub use workspace_mask_apply::*;
mod data_preprocess_types;
pub use data_preprocess_types::*;
mod mkgrd_data_preprocess_source;
pub use mkgrd_data_preprocess_source::*;
mod mkgrd_final_handoff;
pub use mkgrd_final_handoff::*;
mod mkgrd_restart_types;
pub use mkgrd_restart_types::*;
mod mkgrd_selected_land_domain;
pub use mkgrd_selected_land_domain::*;
mod mkgrd_mask_restart;
pub use mkgrd_mask_restart::*;
mod mkgrd_mask_restart_plan;
pub use mkgrd_mask_restart_plan::*;
mod mkgrd_mask_restart_ocean;
pub use mkgrd_mask_restart_ocean::*;
mod mkgrd_default_restart_handoff;
pub use mkgrd_default_restart_handoff::*;
mod mkgrd_run_types;
pub use mkgrd_run_types::*;
mod olam_native_namelist;
pub(crate) use olam_native_namelist::*;
mod olam_native_parser;
mod olam_region_sources;
pub(crate) use olam_region_sources::*;
mod olam_method_c_support;
pub(crate) use olam_method_c_support::*;
mod olam_mesh_gridfile_handoff;
pub(crate) use olam_mesh_gridfile_handoff::*;
mod olam_direct_refine_support;
pub(crate) use olam_direct_refine_support::*;
mod mkgrd_gridinit_driver;
pub use mkgrd_gridinit_driver::*;

mod mkgrd_refine_namelist;
pub use mkgrd_refine_namelist::*;
mod mkgrd_olam_refine_namelist;
pub use mkgrd_olam_refine_namelist::run_mkgrd_olam_specified_refine_global_source_namelist;
mod mkgrd_top_level_dispatch;
pub use mkgrd_top_level_dispatch::*;
