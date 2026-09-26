//! The output layer: what a refined mesh becomes on disk.
//!
//! The gridfile mesh types, their NetCDF reading and writing, and the records
//! a gridfile carries beside the mesh (the h-field a run marked from, the
//! nominal MPAS widths). Nothing here depends on a refinement backend or on
//! how the demand was produced -- `scripts/check_architecture.py` holds that
//! -- so every backend's mesh is written the same way. See
//! docs/architecture_layering_audit_2026-09-25.md, step 5.

pub mod coordinate_types;
pub mod fs_support;
pub mod hfield_gridfile_context;
pub mod mpas_gridfile_context;
pub mod netcdf_io;
pub mod unstructured_mesh_io;
pub mod unstructured_mesh_support;

pub mod atomic_output;
pub mod colm_coupling_csv;
pub mod colm_coupling_netcdf;
pub mod colm_manifest_writer;
pub mod colm_package_io;
pub mod colm_surface_reader;
pub mod colm_template_writers;
pub mod colm_types;
pub mod contain_io;
pub mod fvcom_mesh_writer;
pub mod grid_quality_global;
pub mod json_support;
pub mod mask_postproc_types;
pub mod mask_postproc_writers;
pub mod mesh_conversion_gridfile_state;
pub mod mesh_conversion_iap;
pub mod mesh_conversion_support;
pub mod mesh_metric_writers;
pub mod mpas_edge_index_io;
pub mod mpas_graph_info_writer;
pub mod mpas_mesh_types;
pub mod mpas_regional_connectivity;
pub mod mpas_subset;
pub mod mpas_topology_checker;
pub mod obc_boundary_io;
pub mod quality_global_writer;
pub use colm_package_io::{
    write_colm_coupling_netcdf_from_csv, write_colm_package_delivery_manifest_with_quality,
};
pub use colm_types::{
    ColmCouplingNetcdfWriteReport, ColmForcingTemplateNetcdfWriteReport,
    ColmRestartTemplateNetcdfWriteReport, ColmSurfaceClassPoint, ColmSurfaceCounts,
};
pub use contain_io::validate_contain_mesh;
pub use contain_io::{
    read_contain_netcdf, write_flat_contain_netcdf, ContainMesh, ContainWriteReport,
    FlatContainMesh,
};
pub use coordinate_types::{lat_values, lon_values, LonLatPoint};
pub use fs_support::ensure_parent_dir;
pub use fvcom_mesh_writer::write_fvcom_ns_records;
pub use fvcom_mesh_writer::{fvcom_mesh_2dm_output_path, FvcomMesh2dmWriteReport};
pub use json_support::{
    geojson_feature_nodes, json_escape_string, json_node_to_f64, json_node_to_usize, json_number,
    json_string_array, json_string_usize_map, json_usize_f64_map, json_usize_f64_map_node,
    json_usize_map, JsonNode, JsonParser,
};
pub use mask_postproc_types::{
    EarthPatchtypes, LandPatchtypes, MaskPostprocDomainInputs, MaskPostprocDomainIoPlan,
    MaskPostprocEarthDomainReport, MaskPostprocEarthRunOptions, MaskPostprocFinalizationReport,
    MaskPostprocLandDomainReport, MaskPostprocLandRunOptions, MaskPostprocLayout,
    MaskPostprocOceanDomainReport, MaskPostprocOceanRenewalReport, MaskPostprocOceanRunOptions,
    MaskRestartAction, MaskRestartRemaskPlan,
};
pub use mask_postproc_writers::{
    write_earthmesh_info_netcdf, write_patchid_netcdf, EarthmeshInfo, EarthmeshInfoWriteReport,
    PatchIdMesh, PatchIdWriteReport,
};
pub use mesh_conversion_gridfile_state::earthmesh_runtime_state_from_compact_mesh;
pub use mesh_conversion_gridfile_state::{
    gridfile_mesh_from_one_based_state, gridfile_mesh_from_state,
};
pub use mesh_conversion_iap::derive_iap_w_to_m_one_based;
pub use mesh_conversion_support::{
    cells_on_triangle_one_based_from_mesh, f64_matrix_width, flatten_i32_rows, i32_counts_as_usize,
    i32_matrix_from_flat, i32_rows_as_usize, lonlat_degrees_from_points, lonlat_pairs_from_points,
    lonlat_points_from_pairs, lookup_f64, m_to_w_as_usize_rows, matrix_width,
    n_edges_on_cell_usize_from_mesh, normalize_degrees, one_to_n_i32, parse_value_after_equals,
    patchtype_indices, rad_to_deg, rows_from_flat_i32, rows_to_triangle_connectivity,
    scale_cartesian_points_by_earth_radius, split_cartesian_components,
    triangles_on_cell_one_based_from_mesh, usize_from_i32_connectivity, usize_from_i32_nonnegative,
    usize_from_i32_positive, usize_rows_to_i32, usize_to_i32, usize_values_to_i32,
    validate_mask_postproc_layout, write_f64_1d, write_f64_matrix_rows, write_i32_1d,
    write_i32_matrix_rows, write_i32_pair_rows,
};
pub use mesh_metric_writers::{
    read_cellwidth_netcdf, write_cellwidth_netcdf, write_dists_on_edge_netcdf, CellwidthMesh,
    CellwidthWriteReport, DistsOnEdgeMesh, DistsOnEdgeWriteReport,
};
pub use mpas_graph_info_writer::{write_mpas_graph_info, MpasGraphInfoWriteReport};
pub use mpas_mesh_types::{
    MeshTopologyReport, MpasFullMeshPipelineReport, MpasMesh, MpasMeshWriteReport,
    RegionalMpasConnectivity,
};
pub use netcdf_io::{
    create_netcdf, netcdf_to_io_error, open_netcdf, require_len, required_dimension_len,
    required_values_f64, required_values_f64_any, required_values_i32, required_values_i32_matrix,
    required_values_i8, write_f64_scalar,
};
pub use obc_boundary_io::{
    obc_boundary_output_path, obcv2_boundary_output_path, read_obc_order_netcdf,
    write_obc_boundary_netcdf, write_obcv2_boundary_netcdf, ObcBoundaryWriteReport,
    Obcv2BoundaryWriteReport,
};
pub use quality_global_writer::{
    write_quality_global_netcdf, GlobalQualityMesh, GlobalQualityWriteReport, QualityClassMetrics,
};
pub use unstructured_mesh_support::{
    unstructured_dimc, validate_unstructured_mesh, GridfileMetadataSlices, UnstructuredMesh,
    UnstructuredMeshWriteReport,
};
