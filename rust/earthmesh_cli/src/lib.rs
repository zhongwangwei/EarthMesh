//! EarthMesh execution pipelines, format adapters, and CLI-facing reports.

use earthmesh_core::MkgrdWorkspacePlan;
use earthmesh_mesh::{LonLatDegrees, MethodCRefinementRegion};

pub mod mask_source_discovery;
use mask_source_discovery::discover_mask_sources;

pub(crate) use mask_source_discovery::{source_extension, unsupported_mask_source};
pub mod coordinate_types;
use coordinate_types::{GridRegion, LonLatPoint};
mod fs_support;
pub(crate) use fs_support::ensure_parent_dir;
#[doc(hidden)]
pub use fs_support::resolve_project_path;
mod json_support;
pub(crate) use json_support::{
    geojson_feature_nodes, json_escape_string, json_node_to_f64, json_node_to_usize, json_number,
    json_string_array, json_string_usize_map, json_usize_f64_map, json_usize_f64_map_node,
    json_usize_map, JsonNode, JsonParser,
};
pub mod v3_data_source_io;
use v3_data_source_io::{
    build_v3_data_source_descriptor, V3DataSourceDescriptor, V3DataSourceKind,
};
pub mod global_source_axes;
use global_source_axes::build_global_source_axes_one_based;
pub mod unstructured_mesh_support;
pub(crate) use unstructured_mesh_support::{
    gridfile_m_row_layout, gridfile_w_row_layout, mesh_row_for_canonical_id, unstructured_dimc,
    validate_unstructured_mesh, GridfileRowLayout,
};
use unstructured_mesh_support::{
    GridfileCellKind, GridfileMeshPoints, IapMeshReadPayload, MethodCGridfileMetadataSlices,
    UnstructuredMesh, UnstructuredMeshWriteReport,
};
pub mod merit_tile_selection;
use merit_tile_selection::{select_merit_hydro_tiles, MeritLonLatBbox};
pub mod merit_hydro_io;
mod project_coast_refinement;
use merit_hydro_io::{
    read_merit_hydro_window, write_merit_hydro_mask_geojson_layers,
    MeritHydroGeoJsonLayerWriteReport, MeritMaskThresholds,
};
pub mod hydro_close_types;
use hydro_close_types::{
    HydroCloseMaskNmlOptions, HydroCloseMaskNmlWriteReport, HydroCloseMaskSpec,
    HydroCloseRefinementRecipeOptions, HydroCloseRefinementRecipeWriteReport,
    HydroCompositeCloseMaskComponentSummary, HydroCompositeCloseMaskNmlWriteReport,
    MeritHydroRegionWorkflowReport,
};
mod hydro_close_buffer;
mod hydro_close_composite;
mod hydro_close_envelope_merge;
mod hydro_close_geometry;
mod hydro_close_geometry_utils;
mod hydro_close_hole_decomposition;
mod hydro_close_hole_slabs;
mod hydro_close_hole_spans;
pub mod hydro_close_masks;
mod hydro_close_proximity;
pub mod hydro_close_recipe;
mod merit_hydro_region_close;
pub use hydro_close_composite::write_hydro_composite_close_mask_nmls;
use hydro_close_masks::{
    read_hydro_close_mask_specs, write_hydro_close_mask_nmls, write_hydro_close_mask_specs,
};
use hydro_close_recipe::default_hydro_close_class_refine;
pub use merit_hydro_region_close::write_merit_hydro_region_close_masks;
pub mod hydro_workflow_types;
use hydro_workflow_types::{HydroMeshQaCheck, HydroMeshQaReport, HydroWorkflowReport};
pub mod hydro_delivery_cells;
pub mod hydro_delivery_colm;
mod hydro_delivery_common;
mod hydro_delivery_complete_mask;
pub mod hydro_delivery_coupling_quality;
pub mod hydro_delivery_intersections;
pub mod hydro_delivery_manifest;
pub mod hydro_delivery_qa;
pub mod hydro_delivery_refine_workflow;
pub mod hydro_refinement_adapter;
pub mod hydro_refinement_eval;
pub mod hydro_sweep;
pub mod project_hydro;
pub mod project_hydro_closed_loop;
pub mod project_quality;
use hydro_delivery_colm::write_colm_coupling_csv_from_intersections;
pub(crate) use hydro_delivery_common::{
    format_coupling_number, read_text_maybe_gzip, HYDRO_EARTH_RADIUS_M,
};
pub use hydro_delivery_complete_mask::write_complete_cell_mask_geojson;
use hydro_delivery_coupling_quality::{
    write_colm_coupling_csv_from_mesh_with_options, write_coupling_quality_from_gridfile,
    CouplingCsvOptions,
};
use hydro_delivery_intersections::write_earthmesh_intersection_geojson;
pub(crate) use hydro_delivery_intersections::{geometry_outer_rings, json_node_to_string};
pub mod colm_types;
use colm_types::{
    ColmCouplingNetcdfWriteReport, ColmForcingTemplateNetcdfWriteReport,
    ColmRestartTemplateNetcdfWriteReport, ColmSurfaceClassPoint, ColmSurfaceCounts,
};
mod colm_coupling_csv;
mod colm_coupling_netcdf;
mod colm_manifest_writer;
mod colm_surface_reader;
mod colm_template_writers;
mod netcdf_io;
pub(crate) use netcdf_io::{
    create_netcdf, first_existing_dimension_len, netcdf_to_io_error, open_netcdf,
    optional_values_i32_2d, required_dimension_len, required_scalar_usize_i32, required_values_f64,
    required_values_f64_any, required_values_i32, required_values_i32_2d,
    required_values_i32_matrix, required_values_i8, required_values_i8_matrix, write_f64_scalar,
    write_i32_scalar,
};

/// Create a NetCDF file with HDF5's diagnostic stack silenced.
///
/// This is mainly useful for test/fixture writers that need direct NetCDF
/// access without noisy `HDF5-DIAG` stderr output from libnetcdf's existence
/// checks.
#[doc(hidden)]
pub fn create_netcdf_quiet(
    path: impl AsRef<std::path::Path>,
) -> Result<netcdf::FileMut, netcdf::Error> {
    create_netcdf(path)
}
pub mod colm_package_io;
use colm_package_io::{
    write_colm_coupling_netcdf_from_csv, write_colm_package_delivery_manifest_with_quality,
};
pub mod cama_binary_io;
pub mod cama_binary_params;
pub mod cama_binary_window_readers;
pub mod cama_reach_inventory;
use cama_binary_io::CamaSurfaceClass;
use cama_binary_params::read_cama_grid_spec_from_params_file;
use cama_binary_window_readers::read_cama_elevtn_surface_window;
pub mod bbox_mask_io;
pub mod coastal_band_io;
pub(crate) use bbox_mask_io::validate_bbox_mask_geographic;
use bbox_mask_io::{
    parse_bbox_mask_nml, read_bbox_mask_netcdf, read_bbox_refine_netcdf, write_bbox_mask_netcdf,
};
pub mod close_mesh_io;
use close_mesh_io::read_close_mesh_netcdf;
pub mod circle_close_mask_io;
use circle_close_mask_io::{
    close_mask_netcdf_has_refine, parse_circle_mask_nml, parse_close_mask_nml,
    read_circle_mask_netcdf, read_circle_refine_netcdf, read_close_mask_netcdf,
    read_close_refine_netcdf, write_circle_mask_netcdf, write_close_mask_netcdf, CloseMask,
};
pub(crate) use circle_close_mask_io::{
    validate_circle_mask_geographic, validate_close_mask_geographic,
};
pub mod mode4mesh_make;
pub mod mode_file_io;
use mode_file_io::{
    convert_fvcom_mode_file_to_earthmesh, convert_iap_ocean_mode_file_to_earthmesh,
    convert_mpas_mode_file_to_earthmesh, copy_existing_earthmesh_mode_file,
    write_gridfile_from_one_based_state,
};
pub mod contain_io;
pub(crate) use contain_io::validate_contain_mesh;
use contain_io::{
    read_contain_netcdf, write_flat_contain_netcdf, ContainMesh, ContainWriteReport,
    FlatContainMesh,
};
pub mod getcontain_types;
use getcontain_types::{
    GetContainAreaBounds, GetContainMeshKind, GetContainRefineFileRunConfig,
    GetContainRefineFileRunReport, GetContainRuntimeCounts,
};
pub mod getcontain_geometry;
pub(crate) use getcontain_geometry::getcontain_validate_source_matrix;
use getcontain_geometry::{
    getcontain_containment_matrix_flat_one_based, getcontain_is_in_area_ustr_one_based,
};
pub mod unstructured_mesh_io;
use unstructured_mesh_io::{
    gridfile_output_path, read_unstructured_mesh_netcdf, write_unstructured_mesh_netcdf,
    write_unstructured_mesh_netcdf_with_method_c_metadata,
};
mod mesh_conversion_support;
pub(crate) use mesh_conversion_support::{
    cells_on_triangle_one_based_from_mesh, f64_matrix_width, flatten_i32_rows, i32_counts_as_usize,
    i32_matrix_from_flat, i32_rows_as_usize, lat_values, lon_values, lonlat_degrees_from_points,
    lonlat_pairs_from_points, lonlat_points_from_pairs, lookup_f64, m_to_w_as_usize_rows,
    matrix_width, n_edges_on_cell_usize_from_mesh, normalize_degrees, one_to_n_i32,
    parse_value_after_equals, patchtype_indices, rad_to_deg, require_len, rows_from_flat_i32,
    rows_to_triangle_connectivity, scale_cartesian_points_by_earth_radius,
    split_cartesian_components, triangles_on_cell_one_based_from_mesh, usize_from_i32_connectivity,
    usize_from_i32_nonnegative, usize_from_i32_positive, usize_rows_to_i32, usize_to_i32,
    usize_values_to_i32, validate_mask_postproc_layout, write_f64_1d, write_f64_matrix_rows,
    write_i32_1d, write_i32_matrix_rows, write_i32_pair_rows,
};
pub mod mesh_conversion_gridfile_state;
pub(crate) use mesh_conversion_gridfile_state::earthmesh_runtime_state_from_compact_mesh;
use mesh_conversion_gridfile_state::{
    gridfile_mesh_from_one_based_state, gridfile_mesh_from_state,
};
mod mesh_conversion_iap;
pub(crate) use mesh_conversion_iap::derive_iap_w_to_m_one_based;
pub mod fvcom_mesh_writer;
pub(crate) use fvcom_mesh_writer::write_fvcom_ns_records;
use fvcom_mesh_writer::{
    fvcom_mesh_2dm_output_path, write_fvcom_mesh_2dm, FvcomMesh2dmWriteReport,
};
pub mod obc_boundary_io;
use obc_boundary_io::{
    obc_boundary_output_path, obcv2_boundary_output_path, read_obc_order_netcdf,
    write_obc_boundary_netcdf, write_obcv2_boundary_netcdf, ObcBoundaryWriteReport,
    Obcv2BoundaryWriteReport,
};
pub mod lambert_mode4_io;
pub(crate) use lambert_mode4_io::validate_mode4_mesh_for_area_judge;
use lambert_mode4_io::{
    convert_lambert_mask_netcdf, lambert_vertices_to_mode4_mesh, read_lambert_vertices_netcdf,
    read_mode4_mesh_netcdf, write_mode4_mesh_netcdf,
};
pub mod area_judge_grid_io;
pub(crate) use area_judge_grid_io::{
    grid_covers_area_judge_bounds_one_based, validate_area_judge_grid_payload,
    validate_i32_matrix_shape,
};
use area_judge_grid_io::{
    read_area_judge_grid_netcdf, run_area_judge_restart_grid_one_based,
    select_area_judge_grid_one_based, write_area_judge_grid_netcdf, AreaJudgeGridPayload,
    AreaJudgeRestartGridRunConfig,
};
pub mod area_judge_types;
use area_judge_types::{
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
pub mod area_judge_domain_builders;
use area_judge_domain_builders::{
    build_area_judge_base_state_one_based, build_area_judge_seaorland_one_based,
    classify_area_judge_landtype_one_based,
};
pub mod area_judge_getcontain_refine;
use area_judge_getcontain_refine::run_getcontain_refine_file_one_based;
pub mod area_judge_refine_steps;
use area_judge_refine_steps::{
    build_area_judge_calculated_refine_one_based, run_area_judge_refine_one_based,
};
pub mod area_judge_branch_builders;
use area_judge_branch_builders::{
    build_area_judge_non_restart_one_based, build_area_judge_restart_one_based,
};
pub mod area_judge_grid_runs;
pub(crate) use area_judge_grid_runs::write_area_judge_selected_grid_report;
pub mod area_judge_sources;
pub(crate) use area_judge_sources::merge_area_judge_source_bounds;
use area_judge_sources::{
    apply_area_judge_patch_sources_one_based, build_area_judge_area_sources_one_based,
};
pub mod area_judge_bbox_sources;
use area_judge_bbox_sources::{
    apply_area_judge_bbox_patch_source_one_based, build_area_judge_bbox_area_source_one_based,
};
pub mod area_judge_circle_sources;
use area_judge_circle_sources::{
    apply_area_judge_circle_patch_source_one_based, build_area_judge_circle_area_source_one_based,
};
pub mod area_judge_close_sources;
use area_judge_close_sources::{
    apply_area_judge_close_patch_source_one_based,
    build_area_judge_close_area_source_cells_one_based,
};
pub(crate) use area_judge_close_sources::{
    area_judge_check_crossing, area_judge_close_crosses_dateline,
};
pub mod area_judge_lambert_sources;
use area_judge_lambert_sources::{
    apply_area_judge_lambert_patch_source_one_based, build_area_judge_lambert_area_source_one_based,
};
pub mod area_judge_threshold_inputs;
pub mod mask_postproc_writers;
use mask_postproc_writers::{
    write_earthmesh_info_netcdf, write_patchid_netcdf, EarthmeshInfo, EarthmeshInfoWriteReport,
    PatchIdMesh, PatchIdWriteReport,
};
pub mod mask_postproc_types;
use mask_postproc_types::{
    EarthPatchtypes, LandPatchtypes, MaskPostprocDomainInputs, MaskPostprocDomainIoPlan,
    MaskPostprocEarthDomainReport, MaskPostprocEarthRunOptions, MaskPostprocFinalizationReport,
    MaskPostprocLandDomainReport, MaskPostprocLandRunOptions, MaskPostprocLayout,
    MaskPostprocOceanDomainReport, MaskPostprocOceanRenewalReport, MaskPostprocOceanRunOptions,
    MaskRestartAction, MaskRestartRemaskPlan,
};
pub mod mask_postproc_atmos;
use mask_postproc_atmos::{
    write_mask_postproc_atmos_mpas_netcdf, write_mask_postproc_atmos_mpas_simple_netcdf,
};
pub mod mask_postproc_ocean;
use mask_postproc_ocean::{
    apply_ocean_mask_sea_ratio_one_based, renew_mask_postproc_ocean_domain_one_based,
};
pub mod mask_postproc_patchtypes;
use mask_postproc_patchtypes::{
    build_earth_patchtypes_one_based, build_land_patchtypes_one_based,
    write_mask_postproc_earth_info_netcdf, write_mask_postproc_patchtype_netcdf,
};
pub mod mask_postproc_layout;
pub(crate) use mask_postproc_layout::ensure_leading_mask_postproc_placeholder;
use mask_postproc_layout::{
    finalize_mask_postproc_layout_with_reindex_report, mask_postproc_layout_from_unstructured_mesh,
    read_mask_postproc_domain_inputs, write_mask_postproc_final_gridfile,
};
pub mod mask_postproc_domain;
use mask_postproc_domain::{
    plan_mask_postproc_domain_io, run_mask_postproc_earth_domain, run_mask_postproc_land_domain,
    run_mask_postproc_ocean_domain,
};
pub mod mesh_metric_writers;
use mesh_metric_writers::{
    read_cellwidth_netcdf, write_cellwidth_netcdf, write_dists_on_edge_netcdf, CellwidthMesh,
    CellwidthWriteReport, DistsOnEdgeMesh, DistsOnEdgeWriteReport,
};
pub mod quality_global_writer;
use quality_global_writer::{
    write_quality_global_netcdf, GlobalQualityMesh, GlobalQualityWriteReport, QualityClassMetrics,
};
pub mod mpas_edge_index_io;
pub mod mpas_mesh_types;
use mpas_mesh_types::{
    MeshTopologyReport, MpasFullMeshPipelineReport, MpasMesh, MpasMeshWriteReport,
    RegionalMpasConnectivity,
};
mod mpas_netcdf_rows;
mod mpas_regional_connectivity;
mod mpas_subset;
pub mod mpas_topology;
mod mpas_topology_checker;
use mpas_topology::subset_mpas_mesh;
pub(crate) use mpas_topology::{
    mpas_lat_lon_radians, pad_f64_rows, validate_mpas_mesh, validate_mpas_simple_mesh,
    zero_based_padded_rows, zero_based_pair_rows, zero_based_triplet_rows,
};
pub mod mpas_graph_info_writer;
use mpas_graph_info_writer::{write_mpas_graph_info, MpasGraphInfoWriteReport};
pub mod mpas_simple_writer;
use mpas_simple_writer::{
    write_mpas_simple_mesh_netcdf, MpasSimpleMesh, MpasSimpleMeshWriteReport,
};
mod mpas_full_writer;
pub use mpas_full_writer::write_mpas_mesh_netcdf;
pub mod mpas_unstructured_mesh_builders;
use mpas_unstructured_mesh_builders::{
    build_mpas_mesh_from_unstructured_one_based, build_mpas_simple_mesh_from_unstructured_one_based,
};
pub mod gridfile_output_writers;
use gridfile_output_writers::{
    write_mpas_mesh_from_netcdf_inputs, write_mpas_simple_mesh_from_netcdf_inputs,
};
pub mod mpas_gridfile_writers;
pub mod regional_gridfile_writers;
use regional_gridfile_writers::{
    write_clean_regional_ocean_gridfile, write_fvcom_2dm_from_carved,
    write_landtype_masked_gridfile_with_refine_levels, write_regional_gridfile_with_refine_levels,
};
pub mod mask_counts;
use mask_counts::MaskCountState;
pub mod mask_operation_apply;
use mask_operation_apply::{
    apply_mask_operation, validate_mask_refine_reaches_max_iter_spc, MaskOperationReport,
};
pub mod springjustment_gridfile_types;
use springjustment_gridfile_types::{
    SpringjustmentGlobalGridfileReport, SpringjustmentGlobalPersistenceReport,
    SpringjustmentGlobalRunOptions, SpringjustmentRegionalGridfileReport,
    SpringjustmentRegionalRunOptions,
};
mod grid_production_adapters;
mod grid_quality_global;
mod grid_quality_inputs;
pub mod grid_quality_pipeline;
pub(crate) use grid_quality_pipeline::{
    get_edge_from_unstructured_mesh, read_gridfile_mesh_points,
};
mod springjustment_gridfile_adapters;
pub mod workspace_apply;
use workspace_apply::{apply_read_nl_workspace_plan, WorkspaceApplyReport};
pub mod workspace_mask_apply;
use workspace_mask_apply::{apply_workspace_and_mask_operations, WorkspaceMaskApplyReport};
pub mod data_preprocess_types;
use data_preprocess_types::{DataPreprocessAreaJudgeSourceReport, MkgrdDataPreprocessSourceState};
pub mod mkgrd_data_preprocess_source;
use mkgrd_data_preprocess_source::sample_landtype_values_for_points_one_based;
pub mod mkgrd_restart_types;
use mkgrd_restart_types::{
    MkgrdDefaultRestartRefineHandoff, MkgrdFinalDomainPostprocReport,
    MkgrdMaskRestartOceanRunReport, MkgrdMaskRestartPatchRunReport, MkgrdMaskRestartPlanReport,
    MkgrdRestartAreaJudgeGlobalSourceRunReport, MkgrdRestartAreaJudgeOptions,
    MkgrdRestartAreaJudgePostprocOptions, MkgrdRestartAreaJudgePostprocRunReport,
    MkgrdRestartAreaJudgeRunReport,
};
pub mod mkgrd_mask_restart;
use mkgrd_mask_restart::{
    plan_mkgrd_mask_restart_namelist,
    run_mkgrd_mask_restart_area_judge_configured_global_source_namelist,
    run_mkgrd_mask_restart_area_judge_namelist,
    run_mkgrd_mask_restart_area_judge_postproc_namelist, run_mkgrd_mask_restart_ocean_namelist,
    run_mkgrd_mask_restart_patch_namelist,
};
pub mod mkgrd_default_restart_handoff;
use mkgrd_default_restart_handoff::{
    infer_mask_restart_ocean_num_vertex_from_config, landtype_file_is_real,
    maybe_infer_mask_restart_non_ocean_num_vertex_from_config,
    maybe_infer_mask_restart_ocean_num_vertex_from_config, namelist_sets_landtype_file,
};
pub mod mkgrd_run_types;
use mkgrd_run_types::{
    LandtypeDataPreprocessReport, MkgrdGridinitRunReport,
    MkgrdTopLevelDefaultRestartRefineRunReport, MkgrdTopLevelDispatchRunReport,
    RefineCoupledOutputReport, RefinePipelineRunReport,
};
mod native_grid_config;
pub(crate) use native_grid_config::*;
mod namelist_reader;
mod region_sources;
pub(crate) use region_sources::*;
mod refine_runtime;
pub(crate) use refine_runtime::*;
mod refine_gridfile;
pub(crate) use refine_gridfile::*;
mod refine_controls;
pub(crate) use refine_controls::*;
pub mod mkgrd_gridinit_driver;
use mkgrd_gridinit_driver::{
    run_mkgrd_gridinit_global_namelist, run_mkgrd_regional_clip_base_namelist,
};

mod hfield_refine;
pub use hfield_refine::{
    build_hfield_from_regions, read_hfield_refine_options, HfieldRefineOptions,
};
mod refine_pipeline;
pub use refine_pipeline::run_refine_pipeline_namelist;
pub mod mkgrd_top_level_dispatch;
use mkgrd_top_level_dispatch::run_mkgrd_top_level_namelist;
