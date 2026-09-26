//! The input layer: reading the source data a run refines against.
//!
//! Land-cover and threshold rasters, CaMa-Flood and MERIT Hydro readers, the
//! named-region mask files (bbox, circle, close, Lambert), and the
//! `Area_judge` / `Get_Contain` evaluation that turns them into per-cell
//! demand. Nothing here depends on a refinement backend
//! (`scripts/check_architecture.py`), so every backend reads the same inputs.
//! It builds on the gridfile and NetCDF helpers of `earthmesh_delivery`. See
//! docs/architecture_layering_audit_2026-09-25.md, step 5.
pub mod landtype_preprocess_report;

pub mod area_judge_bbox_sources;
pub mod area_judge_branch_builders;
pub mod area_judge_circle_sources;
pub mod area_judge_close_sources;
pub mod area_judge_domain_builders;
pub mod area_judge_getcontain_refine;
pub mod area_judge_grid_io;
pub mod area_judge_grid_runs;
pub mod area_judge_lambert_sources;
pub mod area_judge_refine_steps;
pub mod area_judge_sources;
pub mod area_judge_threshold_inputs;
pub mod area_judge_types;
pub mod bbox_mask_io;
pub mod cama_binary_io;
pub mod cama_binary_params;
pub mod cama_binary_window_readers;
pub mod cama_reach_inventory;
pub mod circle_close_mask_io;
pub mod coastal_band_io;
pub mod data_preprocess_types;
pub mod getcontain_geometry;
pub mod getcontain_types;
pub mod global_source_axes;
pub mod hfield_refine;
pub mod hydro_close_buffer;
pub mod hydro_close_composite;
pub mod hydro_close_envelope_merge;
pub mod hydro_close_geometry;
pub mod hydro_close_geometry_utils;
pub mod hydro_close_hole_decomposition;
pub mod hydro_close_hole_slabs;
pub mod hydro_close_hole_spans;
pub mod hydro_close_masks;
pub mod hydro_close_proximity;
pub mod hydro_close_recipe;
pub mod hydro_close_types;
pub mod hydro_delivery_common;
pub mod hydro_delivery_complete_mask;
pub mod hydro_delivery_intersections;
pub mod hydro_refinement_adapter;
pub mod hydro_refinement_eval;
pub mod hydro_sweep;
pub mod lambert_mode4_io;
pub mod mask_counts;
pub mod mask_source_discovery;
pub mod merit_hydro_io;
pub mod merit_hydro_region_close;
pub mod merit_tile_selection;
pub mod mkgrd_data_preprocess_source;
pub mod namelist_reader;
pub mod refinement_demand;
pub mod region_sources;
pub mod v3_data_source_io;
pub use area_judge_bbox_sources::{
    apply_area_judge_bbox_patch_source_one_based, build_area_judge_bbox_area_source_one_based,
};
pub use area_judge_branch_builders::{
    build_area_judge_non_restart_one_based, build_area_judge_restart_one_based,
};
pub use area_judge_circle_sources::{
    apply_area_judge_circle_patch_source_one_based, build_area_judge_circle_area_source_one_based,
};
pub use area_judge_close_sources::{
    apply_area_judge_close_patch_source_one_based,
    build_area_judge_close_area_source_cells_one_based,
};
pub use area_judge_close_sources::{area_judge_check_crossing, area_judge_close_crosses_dateline};
pub use area_judge_domain_builders::{
    build_area_judge_base_state_one_based, build_area_judge_seaorland_one_based,
    classify_area_judge_landtype_one_based,
};
pub use area_judge_getcontain_refine::run_getcontain_refine_file_one_based;
pub use area_judge_grid_io::{
    grid_covers_area_judge_bounds_one_based, validate_area_judge_grid_payload,
    validate_i32_matrix_shape,
};
pub use area_judge_grid_io::{
    read_area_judge_grid_netcdf, run_area_judge_restart_grid_one_based,
    select_area_judge_grid_one_based, write_area_judge_grid_netcdf, AreaJudgeGridPayload,
    AreaJudgeRestartGridRunConfig,
};
pub use area_judge_grid_runs::write_area_judge_selected_grid_report;
pub use area_judge_lambert_sources::{
    apply_area_judge_lambert_patch_source_one_based, build_area_judge_lambert_area_source_one_based,
};
pub use area_judge_refine_steps::{
    build_area_judge_calculated_refine_one_based, run_area_judge_refine_one_based,
};
pub use area_judge_sources::merge_area_judge_source_bounds;
pub use area_judge_sources::{
    apply_area_judge_patch_sources_one_based, build_area_judge_area_sources_one_based,
};
pub use area_judge_types::{
    AreaJudgeAreaSourceReport, AreaJudgeBaseStateReport, AreaJudgeCalculatedRefineConfig,
    AreaJudgeDomainInitializationReport, AreaJudgeGridRunConfig, AreaJudgeGridRunReport,
    AreaJudgeGridWriteReport, AreaJudgeLandtypeClass, AreaJudgeNonRestartReport,
    AreaJudgePatchConfig, AreaJudgePatchModifyReport, AreaJudgePatchSourceReport,
    AreaJudgeRefineActivationReport, AreaJudgeRefineGridRunConfig, AreaJudgeRefineGridRunReport,
    AreaJudgeRefineStepReport, AreaJudgeRestartGridsRunConfig, AreaJudgeRestartGridsRunReport,
    AreaJudgeRestartReport, AreaJudgeSeaOrLandReport, AreaJudgeSparseAreaSourceReport,
    AreaJudgeThreshold2D, AreaJudgeThreshold2Layer, AreaJudgeThresholdInputsReport,
    AreaJudgeThresholdReadConfig, ThresholdReadAtmosConfig, ThresholdReadAtmosReport,
    ThresholdReadLndConfig, ThresholdReadLndReport, ThresholdReadOcnConfig, ThresholdReadOcnReport,
};
pub use bbox_mask_io::validate_bbox_mask_geographic;
pub use bbox_mask_io::{
    parse_bbox_mask_nml, read_bbox_mask_netcdf, read_bbox_refine_netcdf, write_bbox_mask_netcdf,
};
pub use cama_binary_io::CamaSurfaceClass;
pub use cama_binary_params::read_cama_grid_spec_from_params_file;
pub use cama_binary_window_readers::read_cama_elevtn_surface_window;
pub use circle_close_mask_io::{
    close_mask_netcdf_has_refine, parse_circle_mask_nml, parse_close_mask_nml,
    read_circle_mask_netcdf, read_circle_refine_netcdf, read_close_mask_netcdf,
    read_close_refine_netcdf, write_circle_mask_netcdf, write_close_mask_netcdf, CloseMask,
};
pub use circle_close_mask_io::{validate_circle_mask_geographic, validate_close_mask_geographic};
pub use data_preprocess_types::{
    DataPreprocessAreaJudgeSourceReport, MkgrdDataPreprocessSourceState,
};
pub use getcontain_geometry::getcontain_validate_source_matrix;
pub use getcontain_geometry::{
    getcontain_containment_matrix_flat_one_based, getcontain_is_in_area_ustr_one_based,
};
pub use getcontain_types::{
    GetContainAreaBounds, GetContainMeshKind, GetContainRefineFileRunConfig,
    GetContainRefineFileRunReport, GetContainRuntimeCounts,
};
pub use global_source_axes::build_global_source_axes_one_based;
pub use lambert_mode4_io::validate_mode4_mesh_for_area_judge;
pub use lambert_mode4_io::{
    convert_lambert_mask_netcdf, lambert_vertices_to_mode4_mesh, read_lambert_vertices_netcdf,
    read_mode4_mesh_netcdf, write_mode4_mesh_netcdf,
};
pub use landtype_preprocess_report::LandtypeDataPreprocessReport;
pub use mask_counts::MaskCountState;
pub use mask_source_discovery::discover_mask_sources;
pub use mask_source_discovery::{source_extension, unsupported_mask_source};
pub use merit_hydro_io::{
    read_merit_hydro_window, write_merit_hydro_mask_geojson_layers,
    MeritHydroGeoJsonLayerWriteReport, MeritMaskThresholds,
};
pub use merit_tile_selection::{select_merit_hydro_tiles, MeritLonLatBbox};
pub use mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based;
pub use v3_data_source_io::{
    build_v3_data_source_descriptor, V3DataSourceDescriptor, V3DataSourceKind,
};

// Gridfile, NetCDF and JSON helpers the readers share with the output layer.
pub use earthmesh_delivery::boundary_model;
pub use earthmesh_delivery::hydro_workflow_types;
pub use earthmesh_delivery::netcdf_io::first_existing_dimension_len;
pub use earthmesh_delivery::netcdf_io::optional_values_i32_2d;
pub use earthmesh_delivery::netcdf_io::required_scalar_usize_i32;
pub use earthmesh_delivery::netcdf_io::required_values_i32_2d;
pub use earthmesh_delivery::netcdf_io::required_values_i8_matrix;
pub use earthmesh_delivery::netcdf_io::write_i32_scalar;
pub use earthmesh_delivery::{
    create_netcdf, ensure_parent_dir, geojson_feature_nodes, i32_matrix_from_flat,
    json_escape_string, json_number, json_string_array, matrix_width, netcdf_to_io_error,
    open_netcdf, parse_value_after_equals, read_close_mesh_netcdf, read_unstructured_mesh_netcdf,
    require_len, required_dimension_len, required_values_f64, required_values_f64_any,
    required_values_i32, usize_to_i32, write_f64_1d, write_flat_contain_netcdf,
    write_i32_matrix_rows, ContainMesh, ContainWriteReport, FlatContainMesh, GridRegion, JsonNode,
    JsonParser, LonLatPoint,
};
pub use earthmesh_delivery::{
    json_node_to_f64, json_node_to_usize, json_string_usize_map, json_usize_f64_map,
    json_usize_f64_map_node, json_usize_map,
};
pub use hydro_close_composite::write_hydro_composite_close_mask_nmls;
pub use hydro_close_masks::{
    read_hydro_close_mask_specs, write_hydro_close_mask_nmls, write_hydro_close_mask_specs,
};
pub use hydro_close_recipe::default_hydro_close_class_refine;
pub use hydro_close_types::{
    HydroCloseMaskNmlOptions, HydroCloseMaskNmlWriteReport, HydroCloseMaskSpec,
    HydroCloseRefinementRecipeOptions, HydroCloseRefinementRecipeWriteReport,
    HydroCompositeCloseMaskComponentSummary, HydroCompositeCloseMaskNmlWriteReport,
    MeritHydroRegionWorkflowReport,
};
pub use hydro_delivery_common::{
    format_coupling_number, read_text_maybe_gzip, HYDRO_EARTH_RADIUS_M,
};
pub use hydro_delivery_complete_mask::write_complete_cell_mask_geojson;
pub use hydro_delivery_intersections::write_earthmesh_intersection_geojson;
pub use hydro_delivery_intersections::{geometry_outer_rings, json_node_to_string};
pub use merit_hydro_region_close::write_merit_hydro_region_close_masks;
pub mod hydro_cell_features;
pub use earthmesh_delivery::hfield_gridfile_context;
pub use earthmesh_delivery::mpas_gridfile_context;
pub use earthmesh_delivery::unstructured_mesh_support;
pub use earthmesh_delivery::UnstructuredMesh;
pub use hfield_refine::{
    build_hfield_from_regions, read_hfield_refine_options, HfieldRefineOptions,
};
pub use mkgrd_data_preprocess_source::{landtype_file_is_real, namelist_sets_landtype_file};
pub use region_sources::read_method_c_calculated_refinement_regions;
