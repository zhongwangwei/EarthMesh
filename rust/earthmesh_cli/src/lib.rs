//! EarthMesh execution pipelines, format adapters, and CLI-facing reports.

use earthmesh_delivery::atomic_output;

use earthmesh_core::MkgrdWorkspacePlan;
use earthmesh_mesh::RefinementRegion;

pub use earthmesh_inputs::mask_source_discovery;
use mask_source_discovery::discover_mask_sources;

use coordinate_types::{GridRegion, LonLatPoint};
pub use earthmesh_delivery::coordinate_types;
pub(crate) use mask_source_discovery::{source_extension, unsupported_mask_source};
mod certified_options;
use earthmesh_delivery::fs_support;
use earthmesh_delivery::json_support;
pub use earthmesh_delivery::unstructured_mesh_support;
pub use earthmesh_inputs::global_source_axes;
pub use earthmesh_inputs::merit_hydro_io;
pub use earthmesh_inputs::merit_tile_selection;
pub use earthmesh_inputs::v3_data_source_io;
pub(crate) use fs_support::ensure_parent_dir;
#[doc(hidden)]
pub use fs_support::resolve_project_path;
use global_source_axes::build_global_source_axes_one_based;
pub(crate) use json_support::{
    geojson_feature_nodes, json_escape_string, json_number, JsonNode, JsonParser,
};
#[cfg(test)]
use merit_tile_selection::MeritLonLatBbox;
pub(crate) use unstructured_mesh_support::{
    gridfile_m_row_layout, gridfile_w_row_layout, mesh_row_for_canonical_id, unstructured_dimc,
    validate_published_cell_degrees, validate_unstructured_mesh,
};
use unstructured_mesh_support::{
    GridfileCellKind, GridfileMeshPoints, GridfileMetadataSlices, IapMeshReadPayload,
    MethodCGridfileLineages, UnstructuredMesh, UnstructuredMeshWriteReport,
};
mod project_coast_refinement;
pub use earthmesh_delivery::hydro_workflow_types;
use earthmesh_inputs::hydro_close_composite;
pub use earthmesh_inputs::hydro_close_masks;
pub use earthmesh_inputs::hydro_close_recipe;
pub use earthmesh_inputs::hydro_close_types;
use earthmesh_inputs::merit_hydro_region_close;
pub use hydro_close_composite::write_hydro_composite_close_mask_nmls;
use hydro_workflow_types::HydroWorkflowReport;
pub use merit_hydro_region_close::write_merit_hydro_region_close_masks;
pub mod hydro_delivery_cells;
pub mod hydro_delivery_colm;
use earthmesh_inputs::hydro_delivery_common;
use earthmesh_inputs::hydro_delivery_complete_mask;
pub mod hydro_delivery_coupling_quality;
pub use earthmesh_delivery::hydro_delivery_manifest;
pub use earthmesh_delivery::hydro_delivery_qa;
pub use earthmesh_inputs::hydro_delivery_intersections;
pub mod hydro_delivery_refine_workflow;
pub use earthmesh_inputs::hydro_refinement_adapter;
pub mod hydro_refinement_runs;
pub use earthmesh_inputs::hydro_refinement_eval;
pub use earthmesh_inputs::hydro_sweep;
pub mod project_delivery;
pub mod project_hydro;
pub mod project_hydro_closed_loop;
pub mod project_quality;
use colm_types::{ColmCouplingNetcdfWriteReport, ColmSurfaceCounts};
pub use earthmesh_delivery::colm_types;
use hydro_delivery_colm::write_colm_coupling_csv_from_intersections;
pub(crate) use hydro_delivery_common::{format_coupling_number, read_text_maybe_gzip};
pub use hydro_delivery_complete_mask::write_complete_cell_mask_geojson;
use hydro_delivery_coupling_quality::{
    write_colm_coupling_csv_from_mesh_with_options, write_coupling_quality_from_gridfile,
    CouplingCsvOptions,
};
use hydro_delivery_intersections::write_earthmesh_intersection_geojson;
pub(crate) use hydro_delivery_intersections::{geometry_outer_rings, json_node_to_string};
pub mod colm_mesh_input;
use earthmesh_delivery::netcdf_io;
pub(crate) use netcdf_io::{
    create_netcdf, first_existing_dimension_len, netcdf_to_io_error, open_netcdf, require_len,
    required_dimension_len, required_values_f64, required_values_i32, required_values_i32_2d,
    required_values_i32_matrix,
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
use bbox_mask_io::{parse_bbox_mask_nml, read_bbox_refine_netcdf, write_bbox_mask_netcdf};
use circle_close_mask_io::{
    parse_circle_mask_nml, parse_close_mask_nml, read_circle_refine_netcdf, read_close_mask_netcdf,
    read_close_refine_netcdf, write_circle_mask_netcdf, write_close_mask_netcdf, CloseMask,
};
use colm_package_io::{
    write_colm_coupling_netcdf_from_csv, write_colm_package_delivery_manifest_with_quality,
};
pub use earthmesh_delivery::close_mesh_io;
pub use earthmesh_delivery::colm_package_io;
pub use earthmesh_inputs::bbox_mask_io;
pub use earthmesh_inputs::cama_binary_io;
pub use earthmesh_inputs::cama_binary_params;
pub use earthmesh_inputs::cama_binary_window_readers;
pub use earthmesh_inputs::cama_reach_inventory;
pub use earthmesh_inputs::circle_close_mask_io;
pub use earthmesh_inputs::coastal_band_io;
pub mod mode4mesh_make;
pub mod mode_file_io;
use area_judge_branch_builders::build_area_judge_restart_one_based;
use area_judge_close_sources::build_area_judge_close_area_source_cells_one_based;
use area_judge_domain_builders::classify_area_judge_landtype_one_based;
use area_judge_getcontain_refine::run_getcontain_refine_file_one_based;
use area_judge_grid_io::{write_area_judge_grid_netcdf, AreaJudgeGridPayload};
pub(crate) use area_judge_grid_runs::write_area_judge_selected_grid_report;
use area_judge_types::{
    AreaJudgeGridWriteReport, AreaJudgeLandtypeClass, AreaJudgePatchConfig, AreaJudgeRestartReport,
};
use contain_io::read_contain_netcdf;
pub use earthmesh_delivery::contain_io;
pub use earthmesh_delivery::fvcom_mesh_writer;
pub use earthmesh_delivery::mask_postproc_types;
pub use earthmesh_delivery::mask_postproc_writers;
pub use earthmesh_delivery::mesh_conversion_gridfile_state;
use earthmesh_delivery::mesh_conversion_iap;
use earthmesh_delivery::mesh_conversion_support;
pub use earthmesh_delivery::obc_boundary_io;
pub use earthmesh_delivery::unstructured_mesh_io;
pub use earthmesh_inputs::area_judge_bbox_sources;
pub use earthmesh_inputs::area_judge_branch_builders;
pub use earthmesh_inputs::area_judge_circle_sources;
pub use earthmesh_inputs::area_judge_close_sources;
pub use earthmesh_inputs::area_judge_domain_builders;
pub use earthmesh_inputs::area_judge_getcontain_refine;
pub use earthmesh_inputs::area_judge_grid_io;
pub use earthmesh_inputs::area_judge_grid_runs;
pub use earthmesh_inputs::area_judge_lambert_sources;
pub use earthmesh_inputs::area_judge_refine_steps;
pub use earthmesh_inputs::area_judge_sources;
pub use earthmesh_inputs::area_judge_threshold_inputs;
pub use earthmesh_inputs::area_judge_types;
pub use earthmesh_inputs::getcontain_geometry;
pub use earthmesh_inputs::getcontain_types;
pub use earthmesh_inputs::lambert_mode4_io;
pub(crate) use fvcom_mesh_writer::write_fvcom_ns_records;
use fvcom_mesh_writer::{fvcom_mesh_2dm_output_path, FvcomMesh2dmWriteReport};
use getcontain_types::{
    GetContainMeshKind, GetContainRefineFileRunConfig, GetContainRefineFileRunReport,
    GetContainRuntimeCounts,
};
use lambert_mode4_io::{
    convert_lambert_mask_netcdf, lambert_vertices_to_mode4_mesh, read_lambert_vertices_netcdf,
    write_mode4_mesh_netcdf,
};
use mask_postproc_types::{
    MaskPostprocDomainIoPlan, MaskPostprocEarthDomainReport, MaskPostprocEarthRunOptions,
    MaskPostprocLandDomainReport, MaskPostprocLandRunOptions, MaskPostprocLayout,
    MaskPostprocOceanDomainReport, MaskPostprocOceanRunOptions, MaskRestartAction,
    MaskRestartRemaskPlan,
};
pub(crate) use mesh_conversion_gridfile_state::earthmesh_runtime_state_from_compact_mesh;
use mesh_conversion_gridfile_state::{
    gridfile_mesh_from_one_based_state, gridfile_mesh_from_state,
};
pub(crate) use mesh_conversion_iap::derive_iap_w_to_m_one_based;
pub(crate) use mesh_conversion_support::{
    cells_on_triangle_one_based_from_mesh, lonlat_degrees_from_points,
    n_edges_on_cell_usize_from_mesh, normalize_degrees, rad_to_deg,
    scale_cartesian_points_by_earth_radius, triangles_on_cell_one_based_from_mesh,
    usize_from_i32_connectivity,
};
use mode_file_io::{
    convert_fvcom_mode_file_to_earthmesh, convert_iap_ocean_mode_file_to_earthmesh,
    convert_mpas_mode_file_to_earthmesh, copy_existing_earthmesh_mode_file,
};
use obc_boundary_io::{
    obc_boundary_output_path, obcv2_boundary_output_path, write_obc_boundary_netcdf,
    write_obcv2_boundary_netcdf,
};
use unstructured_mesh_io::{
    gridfile_output_path, read_unstructured_mesh_netcdf, write_unstructured_mesh_netcdf,
    write_unstructured_mesh_netcdf_with_metadata,
};
pub mod mask_postproc_atmos;
pub use earthmesh_delivery::mask_postproc_layout;
pub use earthmesh_delivery::mask_postproc_ocean;
pub use earthmesh_delivery::mask_postproc_patchtypes;
use mask_postproc_atmos::{
    write_mask_postproc_atmos_mpas_netcdf, write_mask_postproc_atmos_mpas_simple_netcdf,
};
pub(crate) use mask_postproc_layout::ensure_leading_mask_postproc_placeholder;
use mask_postproc_layout::{
    finalize_mask_postproc_layout_with_reindex_report, mask_postproc_layout_from_unstructured_mesh,
    read_mask_postproc_domain_inputs, write_mask_postproc_final_gridfile,
};
use mask_postproc_ocean::{
    apply_ocean_mask_sea_ratio_one_based, renew_mask_postproc_ocean_domain_one_based,
};
use mask_postproc_patchtypes::{
    build_earth_patchtypes_one_based, build_land_patchtypes_one_based,
    write_mask_postproc_earth_info_netcdf, write_mask_postproc_patchtype_netcdf,
};
pub mod mask_postproc_domain;
pub use earthmesh_delivery::gridfile_output_writers;
pub use earthmesh_delivery::hfield_gridfile_context;
use earthmesh_delivery::icon_writer;
pub use earthmesh_delivery::mesh_metric_writers;
pub use earthmesh_delivery::mpas_edge_index_io;
use earthmesh_delivery::mpas_full_writer;
pub use earthmesh_delivery::mpas_graph_info_writer;
pub use earthmesh_delivery::mpas_gridfile_context;
pub use earthmesh_delivery::mpas_gridfile_writers;
pub use earthmesh_delivery::mpas_mesh_types;
pub use earthmesh_delivery::mpas_simple_writer;
pub use earthmesh_delivery::mpas_topology;
pub use earthmesh_delivery::mpas_unstructured_mesh_builders;
pub use earthmesh_delivery::quality_global_writer;
use gridfile_output_writers::{
    write_mpas_mesh_from_netcdf_inputs, write_mpas_simple_mesh_from_netcdf_inputs,
};
pub use icon_writer::{
    write_icon_from_final_gridfile, write_icon_from_final_gridfile_with_parent,
    write_icon_grid_netcdf, IconGridWriteReport, ICON_SPHERE_RADIUS_METERS,
};
use mask_postproc_domain::{plan_mask_postproc_domain_io, run_mask_postproc_ocean_domain};
use mesh_metric_writers::{
    write_cellwidth_netcdf, write_dists_on_edge_netcdf, CellwidthMesh, CellwidthWriteReport,
    DistsOnEdgeMesh, DistsOnEdgeWriteReport,
};
pub use mpas_full_writer::{
    write_mpas_mesh_netcdf, write_mpas_ocean_mesh_netcdf, MPAS_OCEAN_SPHERE_RADIUS_METERS,
};
use mpas_graph_info_writer::write_mpas_graph_info;
use mpas_mesh_types::MpasFullMeshPipelineReport;
use mpas_simple_writer::MpasSimpleMeshWriteReport;
use mpas_unstructured_mesh_builders::build_mpas_mesh_from_unstructured_one_based;
pub mod regional_gridfile_writers;
pub use earthmesh_inputs::mask_counts;
use mask_counts::MaskCountState;
use regional_gridfile_writers::{
    write_clean_regional_ocean_gridfile, write_landtype_masked_gridfile_with_refine_levels,
    write_regional_gridfile_with_refine_levels,
};
pub mod mask_operation_apply;
use mask_operation_apply::{
    apply_mask_operation, validate_mask_refine_reaches_max_iter_spc, MaskOperationReport,
};
pub mod springjustment_gridfile_types;
use earthmesh_delivery::grid_production_adapters;
use earthmesh_delivery::grid_quality_global;
use springjustment_gridfile_types::{
    SpringjustmentGlobalGridfileReport, SpringjustmentGlobalPersistenceReport,
    SpringjustmentGlobalRunOptions, SpringjustmentRegionalGridfileReport,
    SpringjustmentRegionalRunOptions,
};
mod grid_quality_inputs;
pub mod grid_quality_pipeline;
pub(crate) use grid_quality_pipeline::{
    get_edge_from_unstructured_mesh, read_gridfile_cell_lineages, read_gridfile_mesh_points,
};
mod springjustment_gridfile_adapters;
pub mod workspace_apply;
use workspace_apply::{apply_read_nl_workspace_plan, WorkspaceApplyReport};
pub mod workspace_mask_apply;
pub use earthmesh_inputs::data_preprocess_types;
use workspace_mask_apply::{apply_workspace_and_mask_operations, WorkspaceMaskApplyReport};
pub mod adaptive_refine;
pub use earthmesh_delivery::boundary_model;
pub mod coast_refinement_regions;
pub mod method_c_adaptive_nest;
pub mod method_c_algorithm;
pub use earthmesh_inputs::mkgrd_data_preprocess_source;
pub mod redgreen_bridge;
pub use earthmesh_inputs::refinement_demand;
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
use earthmesh_inputs::mkgrd_data_preprocess_source::{
    landtype_file_is_real, namelist_sets_landtype_file,
};
use mkgrd_default_restart_handoff::{
    infer_mask_restart_ocean_num_vertex_from_config,
    maybe_infer_mask_restart_non_ocean_num_vertex_from_config,
    maybe_infer_mask_restart_ocean_num_vertex_from_config,
};
pub mod mkgrd_run_types;
use mkgrd_run_types::{
    MkgrdGridinitRunReport, MkgrdTopLevelDefaultRestartRefineRunReport,
    MkgrdTopLevelDispatchRunReport, RefineCoupledOutputReport, RefinePipelineRunReport,
};
mod native_grid_config;
use earthmesh_inputs::namelist_reader;
pub(crate) use earthmesh_inputs::region_sources;
pub(crate) use native_grid_config::*;
pub(crate) use region_sources::*;
mod refine_runtime;
pub(crate) use refine_runtime::*;
mod refine_gridfile;
pub(crate) use refine_gridfile::*;
mod refine_controls;
pub(crate) use refine_controls::*;
pub mod mkgrd_gridinit_driver;
use mkgrd_gridinit_driver::run_mkgrd_gridinit_global_namelist;

use earthmesh_inputs::hfield_refine;
pub use hfield_refine::{
    build_hfield_from_regions, read_hfield_refine_options, HfieldRefineOptions,
};
mod refine_pipeline;
pub use refine_pipeline::{
    run_refine_pipeline_namelist, run_refine_pipeline_with_delivery, LeppResolvedTargets,
};
pub mod mkgrd_top_level_dispatch;
