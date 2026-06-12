//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, GridMemory, IjTabs, MaskOperation, MkgrdWorkspacePlan};
use earthmesh_geometry::{
    area_judge_first_self_intersection_fortran_indexed, is_point_in_circle_km,
    is_point_in_convex_polygon, Point as AreaJudgePoint,
};
use earthmesh_mesh::{
    area_judge_apply_mask_patch_fortran_indexed, area_judge_closed_curve_fill_fortran_indexed,
    area_judge_minmax_range_make_fortran_indexed, area_judge_source_find_fortran_indexed,
    boundary_connection_fortran_indexed, centroid_spherical_mesh_fortran_indexed,
    circumcenter_spherical_mesh_fortran_indexed, classify_boundary_orders_fortran_indexed,
    connect_on_cell_fortran_indexed, edge_distance_angle_fortran_indexed,
    get_area_production_fortran_indexed, get_edge_production_fortran_indexed,
    lonlat_points_to_unit_xyz, order_vertices_on_cell_fortran_indexed,
    remove_isolated_ocean_fortran_indexed, renew_mask_postproc_domain_triangles_fortran_indexed,
    renew_mask_postproc_opposite_domain_triangles_fortran_indexed,
    set_weights_on_edge_fortran_indexed, springjustment_global_core_fortran_indexed,
    springjustment_regional_core_fortran_indexed,
    standardize_vertices_on_cell_rotation_fortran_indexed,
    triangle_neighbors_from_cell_membership_fortran_indexed, widen_narrow_waterway_fortran_indexed,
    xyz_to_lonlat_degrees, AreaJudgeAxis, AreaJudgeSourceBounds, BoundaryConnection,
    BoundaryOrders, CartesianPoint, DistanceLayerSpacing, GetAreaProductionOutput,
    GetAreaUnitInput, GetEdgeProductionOutput, GlobalDistanceStep, IsolatedOceanRenewal,
    LonLatDegrees, MaskPostprocRenewedData, SpringjustmentGlobalCoreInput,
    SpringjustmentGlobalCoreOutput, SpringjustmentRegionalCoreInput,
    SpringjustmentRegionalCoreOutput,
};

/// Report for the migrated initial-grid branch of the `mkgrd.x` driver.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdGridinitRunReport {
    pub config: EarthmeshConfig,
    pub workspace_mask: WorkspaceMaskApplyReport,
    pub gridfile: UnstructuredMeshWriteReport,
}

/// Report for the migrated top-level `mkgrd.F90` mask-restart branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdMaskRestartPlanReport {
    pub config: EarthmeshConfig,
    pub workspace_plan: MkgrdWorkspacePlan,
    pub remask: MaskRestartRemaskPlan,
}

/// Report for the migrated executable subset of the top-level `mkgrd.F90`
/// mask-restart branch.
#[derive(Debug, Clone, PartialEq)]
pub struct MkgrdMaskRestartOceanRunReport {
    pub plan: MkgrdMaskRestartPlanReport,
    pub postproc: MaskPostprocOceanDomainReport,
}

/// File-level I/O contract for the domain branches of
/// `MOD_mask_postproc.F90:mask_postproc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskPostprocDomainIoPlan {
    pub file_dir: PathBuf,
    pub mesh_type: String,
    pub mode_grid: String,
    pub source_gridfile: PathBuf,
    pub contain_domain: PathBuf,
    pub result_gridfile: PathBuf,
    pub patchtype_output: Option<PathBuf>,
    pub obc_output: Option<PathBuf>,
    pub obcv2_output: Option<PathBuf>,
}

/// NetCDF inputs loaded for domain `mask_postproc_Earth/Lnd/Ocn` orchestration.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocDomainInputs {
    pub layout: MaskPostprocLayout,
    pub contain: ContainMesh,
    pub is_in_domain_ustr: Vec<i32>,
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Earth` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocEarthRunOptions<'a> {
    pub mask_sea_ratio: f64,
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
    pub num_mp_step: &'a [usize],
    pub sjx_points: usize,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Earth`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocEarthDomainReport {
    pub patchtypes: EarthPatchtypes,
    pub patchtype: PatchIdWriteReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
    pub earthmesh_info: EarthmeshInfoWriteReport,
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Lnd` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocLandRunOptions<'a> {
    pub seaorland: &'a [Vec<i32>],
    pub minlon_dm_area: i32,
    pub maxlat_dm_area: i32,
    pub nlons_dm_select: usize,
    pub nlats_dm_select: usize,
    pub lon_vertex: &'a [f64],
    pub lat_vertex: &'a [f64],
    pub lon_i: &'a [f64],
    pub lat_i: &'a [f64],
}

/// Runtime controls needed to reproduce the file-backed
/// `MOD_mask_postproc.F90:mask_postproc_Ocn` branch from an I/O plan.
#[derive(Debug, Clone, Copy)]
pub struct MaskPostprocOceanRunOptions {
    pub mask_sea_ratio: f64,
    pub num_vertex: usize,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Lnd`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocLandDomainReport {
    pub patchtypes: LandPatchtypes,
    pub patchtype: PatchIdWriteReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
}

/// Evidence report from the gridfile/contain-backed Rust replacement path for
/// `MOD_mask_postproc.F90:mask_postproc_Ocn`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocOceanDomainReport {
    pub renewal: MaskPostprocOceanRenewalReport,
    pub finalization: MaskPostprocFinalizationReport,
    pub final_gridfile: UnstructuredMeshWriteReport,
    pub boundary_orders: Option<BoundaryOrders>,
    pub obc: Option<ObcBoundaryWriteReport>,
    pub obcv2: Option<Obcv2BoundaryWriteReport>,
}

/// Result of composing the tri-only ocean postprocess mask renewal routines
/// before final gridfile/OBC writing.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocOceanRenewalReport {
    pub is_in_domain_ustr: Vec<i32>,
    pub renewed: MaskPostprocRenewedData,
    pub boundary: Option<BoundaryConnection>,
    pub isolated: Option<IsolatedOceanRenewal>,
}

/// Restart action selected by the top-level `mkgrd.F90` mask-restart branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskRestartAction {
    /// Fortran calls `mask_postproc(mesh_type)` and stops immediately.
    RunMaskPostproc,
    /// Fortran continues into the normal mkgrd path after the read_nl restart short-circuit.
    ContinueMkgrd,
}

/// Non-destructive plan for the remask-specific state mutation around `mask_postproc`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskRestartRemaskPlan {
    pub file_dir: PathBuf,
    pub mesh_type: String,
    pub step: i32,
    pub refine: bool,
    pub action: MaskRestartAction,
}

/// Plan the top-level `mkgrd.F90` mask-restart branch without running
/// `MOD_mask_postproc.F90:mask_postproc`.
///
/// This ports the branch decision and state changes around the Fortran call:
/// `refine=.false.`, `step=max_iter+1`, and immediate `mask_postproc` only for
/// `mesh_type='oceanmesh'` with `mask_patch_on=.false.`.  The heavy postprocess
/// kernel remains a separate migration surface.
pub fn plan_mkgrd_mask_restart_namelist(
    namelist_source: impl AsRef<Path>,
    _workdir: impl AsRef<Path>,
    max_iter: i32,
) -> io::Result<MkgrdMaskRestartPlanReport> {
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    if !config.mask_restart {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_restart branch requires NL%mask_restart=.true.",
        ));
    }

    let workspace_plan = config.read_nl_workspace_plan(None);
    let action = if config.mesh_type == "oceanmesh" && !config.mask_patch_on {
        MaskRestartAction::RunMaskPostproc
    } else {
        MaskRestartAction::ContinueMkgrd
    };
    let remask = MaskRestartRemaskPlan {
        file_dir: PathBuf::from(config.file_dir()),
        mesh_type: config.mesh_type.clone(),
        step: max_iter + 1,
        refine: false,
        action,
    };

    Ok(MkgrdMaskRestartPlanReport {
        config,
        workspace_plan,
        remask,
    })
}

/// Execute the migrated `mkgrd.F90` mask-restart branch that immediately calls
/// `mask_postproc` for `mesh_type='oceanmesh'` and `mask_patch_on=.false.`.
///
/// Other restart continuations still return an explicit error because their
/// downstream refine/postprocess loop is tracked separately in the migration
/// manifest.
pub fn run_mkgrd_mask_restart_ocean_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_iter: i32,
    options: MaskPostprocOceanRunOptions,
) -> io::Result<MkgrdMaskRestartOceanRunReport> {
    let plan = plan_mkgrd_mask_restart_namelist(namelist_source, workdir, max_iter)?;
    if plan.remask.action != MaskRestartAction::RunMaskPostproc {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_restart execution is only migrated for oceanmesh without mask_patch_on; got action {:?}",
                plan.remask.action
            ),
        ));
    }
    if plan.config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for mask_restart postproc",
        ));
    }
    let nxp = usize::try_from(plan.config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let postproc_plan = plan_mask_postproc_domain_io(
        &plan.remask.file_dir,
        nxp,
        &plan.config.mode_grid,
        &plan.config.mesh_type,
        plan.config.mask_patch_on,
    )?;
    let postproc = run_mask_postproc_ocean_domain(&postproc_plan, options)?;

    Ok(MkgrdMaskRestartOceanRunReport { plan, postproc })
}

/// Run the Rust replacement path for the initial global `mkgrd.x` gridinit branch.
///
/// This mirrors the branch where `mode_grid` is `hex`/`tri` and `mode_file` does
/// not exist: parse the mkgrd namelist, apply the read_nl workspace/mask plan,
/// generate the in-memory global grid, and write
/// `gridfile/gridfile_NXP####_01_<mode_grid>.nc4`.  Restart mode and reading an
/// existing `mode_file` remain explicit `InvalidInput` errors until those legacy
/// branches are migrated behind tests.
pub fn run_mkgrd_gridinit_global_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
) -> io::Result<MkgrdGridinitRunReport> {
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;

    if config.mask_restart {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_restart mkgrd branch is not yet migrated to Rust",
        ));
    }
    if !matches!(config.mode_grid.as_str(), "hex" | "tri") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mode_grid {} is not supported by the gridinit branch",
                config.mode_grid
            ),
        ));
    }
    if config.nxp <= 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "NXP must be positive for gridinit",
        ));
    }
    if config.niter < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "niter must be non-negative for gridinit",
        ));
    }
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let niter = usize::try_from(config.niter)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "niter must fit usize"))?;

    let plan = config.read_nl_workspace_plan(None);
    let workspace_mask =
        apply_workspace_and_mask_operations(&plan, namelist_source, workdir, 9, false)?;

    let mode_file = PathBuf::from(config.mode_file.trim());
    let gridfile = if mode_file.exists() {
        match config.mode_file_description.trim() {
            "EarthMesh" => copy_existing_earthmesh_mode_file(
                &mode_file,
                &config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "MPAS" => convert_mpas_mode_file_to_earthmesh(
                &mode_file,
                &config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "FVCOM" => convert_fvcom_mode_file_to_earthmesh(
                &mode_file,
                &config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            "IAP-Ocean" => convert_iap_ocean_mode_file_to_earthmesh(
                &mode_file,
                &config.file_dir(),
                nxp,
                &config.mode_grid,
            )?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "only existing EarthMesh, MPAS, FVCOM, and IAP-Ocean mode_file ingestion are migrated to Rust",
                ));
            }
        }
    } else {
        let state = earthmesh_mesh::gridinit_voronoi_state_fortran(
            nxp,
            niter,
            f64::from(config.beta),
            f64::from(config.relax),
            max_tris,
        )?;
        write_gridfile_from_fortran_indexed_state(
            config.file_dir(),
            nxp,
            1,
            &config.mode_grid,
            &state.grid,
            &state.tabs,
        )?
    };

    Ok(MkgrdGridinitRunReport {
        config,
        workspace_mask,
        gridfile,
    })
}

/// Rust data shape written by `MOD_file_preprocess.F90:Unstructured_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct UnstructuredMesh {
    pub m_points: Vec<LonLatPoint>,
    pub w_points: Vec<LonLatPoint>,
    pub m_to_w: Vec<[i32; 3]>,
    pub w_to_m: Vec<Vec<i32>>,
    pub n_w_to_m: Vec<i32>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:Contain_Save`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainMesh {
    pub ustr_id: Vec<Vec<i32>>,
    pub ustr_ii: Vec<Vec<i32>>,
    pub is_in_area_ustr: Vec<i32>,
}

/// Rust data shape written by `MOD_mask_postproc.F90:PatchID_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct PatchIdMesh {
    pub elmindex: Vec<Vec<i32>>,
    pub lon_w: Vec<f64>,
    pub lon_e: Vec<f64>,
    pub lat_n: Vec<f64>,
    pub lat_s: Vec<f64>,
    pub longitude: Vec<f64>,
    pub latitude: Vec<f64>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:LOCmesh_info_save`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthmeshInfo {
    pub num_step_f: Vec<i32>,
    pub refine_degree_f: Vec<i32>,
    pub seaorland_ustr_f: Vec<i32>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasSimpleMesh {
    pub x_cell: Vec<f64>,
    pub y_cell: Vec<f64>,
    pub z_cell: Vec<f64>,
    pub x_vertex: Vec<f64>,
    pub y_vertex: Vec<f64>,
    pub z_vertex: Vec<f64>,
    pub cells_on_vertex: Vec<Vec<i32>>,
    pub mesh_density: Vec<f64>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:cellwidth_save`.
#[derive(Debug, Clone, PartialEq)]
pub struct CellwidthMesh {
    pub cell_points: Vec<LonLatPoint>,
    pub cellwidth: Vec<f64>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:distsOnEdge_save`.
#[derive(Debug, Clone, PartialEq)]
pub struct DistsOnEdgeMesh {
    pub edge_points: Vec<LonLatPoint>,
    pub dists_on_edge: Vec<f64>,
}

/// One polygon/triangle class in `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct QualityClassMetrics {
    pub length: Vec<Vec<f64>>,
    pub angle: Vec<Vec<f64>>,
    pub extr: [f64; 2],
    pub eavg: [f64; 2],
    pub savg: f64,
    pub less: Vec<i32>,
    pub more: Vec<i32>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct GlobalQualityMesh {
    pub sjx: QualityClassMetrics,
    pub wbx: QualityClassMetrics,
    pub lbx: QualityClassMetrics,
    pub qbx: Option<QualityClassMetrics>,
}

/// Edge-reference payload read by `MOD_file_preprocess.F90:data_read`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasEdgeReference {
    pub cells_on_edge_reference: Vec<[i32; 2]>,
    pub edge_points: Vec<LonLatPoint>,
}

/// Rust data shape written by `MOD_file_preprocess.F90:MPAS_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct MpasMesh {
    pub lat_cell: Vec<f64>,
    pub lon_cell: Vec<f64>,
    pub x_cell: Vec<f64>,
    pub y_cell: Vec<f64>,
    pub z_cell: Vec<f64>,
    pub lat_vertex: Vec<f64>,
    pub lon_vertex: Vec<f64>,
    pub x_vertex: Vec<f64>,
    pub y_vertex: Vec<f64>,
    pub z_vertex: Vec<f64>,
    pub lat_edge: Vec<f64>,
    pub lon_edge: Vec<f64>,
    pub x_edge: Vec<f64>,
    pub y_edge: Vec<f64>,
    pub z_edge: Vec<f64>,
    pub n_edges_on_cell: Vec<i32>,
    pub cells_on_cell: Vec<Vec<i32>>,
    pub vertices_on_cell: Vec<Vec<i32>>,
    pub edges_on_cell: Vec<Vec<i32>>,
    pub cells_on_vertex: Vec<Vec<i32>>,
    pub edges_on_vertex: Vec<Vec<i32>>,
    pub cells_on_edge: Vec<[i32; 2]>,
    pub vertices_on_edge: Vec<[i32; 2]>,
    pub n_edges_on_edge: Vec<i32>,
    pub edges_on_edge: Vec<Vec<i32>>,
    pub area_cell: Vec<f64>,
    pub area_triangle: Vec<f64>,
    pub kite_areas_on_vertex: Vec<Vec<f64>>,
    pub dv_edge: Vec<f64>,
    pub dc_edge: Vec<f64>,
    pub angle_edge: Vec<f64>,
    pub weights_on_edge: Vec<Vec<f64>>,
    pub mesh_density: Vec<f64>,
    pub nominal_min_dc: f64,
    pub error_segment: Vec<f64>,
}

/// Result of the pure `mask_postproc_Earth` patchtypes_make loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthPatchtypes {
    pub seaorland_ustr: Vec<i32>,
    pub patchtypes_select: Vec<Vec<i32>>,
    pub sum_land_ustr: usize,
    pub sum_sea_ustr: usize,
}

/// Result of the pure `mask_postproc_Lnd` `patchtypes_make` loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandPatchtypes {
    pub seaorland: Vec<Vec<i32>>,
    pub patchtypes_select: Vec<Vec<i32>>,
    pub filled_ignored_land_pixels: usize,
}

/// Working mesh orientation used by `MOD_mask_postproc.F90:mask_postproc_*`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocLayout {
    pub ustr_points: usize,
    pub ustr_bounds: usize,
    pub center_points: Vec<LonLatPoint>,
    pub vertex_points: Vec<LonLatPoint>,
    pub center_neighbors: Vec<Vec<usize>>,
    pub vertex_neighbors: Vec<Vec<usize>>,
    pub center_neighbor_counts: Vec<usize>,
    pub vertex_neighbor_counts: Vec<usize>,
}

/// Final gridfile payload plus the vertex reindex evidence produced by the
/// final `MOD_mask_postproc.F90:mask_postproc_*` compaction sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocFinalizationReport {
    pub mesh: UnstructuredMesh,
    pub final_data: earthmesh_mesh::MaskPostprocFinalData,
    pub vertex_reindex: earthmesh_mesh::VertexReindex,
}

/// Evidence report from writing an unstructured gridfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstructuredMeshWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub lbx_points: usize,
    pub dimc: usize,
}

/// Evidence report from reading/writing a contain-domain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainWriteReport {
    pub output: PathBuf,
    pub num_ustr: usize,
    pub num_ii: usize,
    pub dim_a: usize,
    pub dim_b: usize,
}

/// Evidence report from writing a patchtype file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchIdWriteReport {
    pub output: PathBuf,
    pub nlon: usize,
    pub nlat: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:LOCmesh_info_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthmeshInfoWriteReport {
    pub output: PathBuf,
    pub num_step: usize,
    pub num_ustr: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasSimpleMeshWriteReport {
    pub output: PathBuf,
    pub n_cells: usize,
    pub n_vertices: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_Mesh_Save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasMeshWriteReport {
    pub output: PathBuf,
    pub n_cells: usize,
    pub n_vertices: usize,
    pub n_edges: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:cellwidth_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellwidthWriteReport {
    pub output: PathBuf,
    pub num_dbx: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:distsOnEdge_save` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistsOnEdgeWriteReport {
    pub output: PathBuf,
    pub num_edge: usize,
}

/// Evidence report from writing `MOD_file_preprocess.F90:quality_save_global`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalQualityWriteReport {
    pub output: PathBuf,
    pub num_sjx: usize,
    pub num_wbx: usize,
    pub num_lbx: usize,
    pub num_qbx: usize,
}

/// Evidence report from writing `MOD_grid_preprocess.F90:Springjustment_global`
/// persistence side effects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpringjustmentGlobalPersistenceReport {
    pub dists_on_edge: DistsOnEdgeWriteReport,
    pub cellwidth: Option<CellwidthWriteReport>,
}

/// Runtime controls needed to reproduce the migrated
/// `MOD_grid_preprocess.F90:Springjustment_global` calculation from a gridfile.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentGlobalRunOptions<'a> {
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub distance_num_rc: usize,
    pub distance_spacing: DistanceLayerSpacing,
    pub distance_steps: &'a [GlobalDistanceStep<'a>],
    pub niter_refine: usize,
    pub relax: f64,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Evidence report from the gridfile-backed Rust replacement path for
/// `MOD_grid_preprocess.F90:Springjustment_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentGlobalGridfileReport {
    pub core: SpringjustmentGlobalCoreOutput,
    pub persistence: SpringjustmentGlobalPersistenceReport,
    pub mesh: UnstructuredMesh,
}

/// Runtime controls needed to reproduce the migrated
/// `MOD_grid_preprocess.F90:Springjustment_regional_step` calculation from a
/// gridfile when the regional move mask has already been resolved.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalRunOptions<'a> {
    pub move_mask: &'a [bool],
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Evidence report from the gridfile-backed Rust replacement path for
/// `MOD_grid_preprocess.F90:Springjustment_regional_step`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalGridfileReport {
    pub core: SpringjustmentRegionalCoreOutput,
    pub mesh: UnstructuredMesh,
}

/// Evidence report from writing `MOD_file_preprocess.F90:MPAS_info_Save` graph.info.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasGraphInfoWriteReport {
    pub output: PathBuf,
    pub n_cells_written: usize,
    pub interior_edges: usize,
    pub cells_with_boundary_edges: usize,
}

/// Evidence report from the full `MOD_mask_postproc.F90:MPAS_Mesh_Cal` file pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpasFullMeshPipelineReport {
    pub mesh: MpasMeshWriteReport,
    pub graph_info: MpasGraphInfoWriteReport,
}

/// Evidence report from writing `MOD_mask_postproc.F90:bdy_calculation` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObcBoundaryWriteReport {
    pub output: PathBuf,
    pub boundary_points: usize,
}

/// Evidence report from writing `MOD_mask_postproc.F90:bdy_connection` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obcv2BoundaryWriteReport {
    pub output: PathBuf,
    pub longest_curve_slots: usize,
    pub closed_curves: usize,
}

/// Build the `Unstructured_Mesh_Save` payload from the Rust-owned grid and
/// connectivity state used by `mkgrd.F90:gridfile_write`.
pub fn gridfile_mesh_from_state(grid: &GridMemory, tabs: &IjTabs) -> io::Result<UnstructuredMesh> {
    let nma = grid.nma;
    let nwa = grid.nwa;
    require_len("grid.glonm", grid.glonm.len(), nma)?;
    require_len("grid.glatm", grid.glatm.len(), nma)?;
    require_len("grid.glonw", grid.glonw.len(), nwa)?;
    require_len("grid.glatw", grid.glatw.len(), nwa)?;
    require_len("itab_m", tabs.m.len(), nma)?;
    require_len("itab_w", tabs.w.len(), nwa)?;

    let m_points = (0..nma)
        .map(|idx| LonLatPoint {
            lon: f64::from(grid.glonm[idx]),
            lat: f64::from(grid.glatm[idx]),
        })
        .collect();
    let w_points = (0..nwa)
        .map(|idx| LonLatPoint {
            lon: f64::from(grid.glonw[idx]),
            lat: f64::from(grid.glatw[idx]),
        })
        .collect();
    let m_to_w = tabs.m.iter().take(nma).map(|tab| tab.iw).collect();

    let mut n_w_to_m = vec![1; nwa];
    let mut w_to_m = Vec::with_capacity(nwa);
    for (idx, tab) in tabs.w.iter().take(nwa).enumerate() {
        if idx == 0 {
            n_w_to_m[idx] = 1;
        } else if tab.im[5] == 1 {
            n_w_to_m[idx] = 5;
        } else {
            n_w_to_m[idx] = 6;
        }
        w_to_m.push(tab.im.to_vec());
    }

    Ok(UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

/// Build the `Unstructured_Mesh_Save` payload from Fortran-indexed grid state.
///
/// Some migrated kernels, especially the remaining `gridinit/voronoi/pcvt`
/// path, keep a direct Fortran-compatible layout with slot `0` unused and valid
/// records in `1..=nma` / `1..=nwa`. This adapter deliberately slices those
/// one-based slots into the compact NetCDF payload written by
/// `Unstructured_Mesh_Save`, without changing connectivity IDs.
pub fn gridfile_mesh_from_fortran_indexed_state(
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMesh> {
    let nma = grid.nma;
    let nwa = grid.nwa;
    require_len("grid.glonm", grid.glonm.len(), nma + 1)?;
    require_len("grid.glatm", grid.glatm.len(), nma + 1)?;
    require_len("grid.glonw", grid.glonw.len(), nwa + 1)?;
    require_len("grid.glatw", grid.glatw.len(), nwa + 1)?;
    require_len("itab_m", tabs.m.len(), nma + 1)?;
    require_len("itab_w", tabs.w.len(), nwa + 1)?;

    let m_points = (1..=nma)
        .map(|idx| LonLatPoint {
            lon: f64::from(grid.glonm[idx]),
            lat: f64::from(grid.glatm[idx]),
        })
        .collect();
    let w_points = (1..=nwa)
        .map(|idx| LonLatPoint {
            lon: f64::from(grid.glonw[idx]),
            lat: f64::from(grid.glatw[idx]),
        })
        .collect();
    let m_to_w = (1..=nma).map(|idx| tabs.m[idx].iw).collect();

    let mut n_w_to_m = Vec::with_capacity(nwa);
    let mut w_to_m = Vec::with_capacity(nwa);
    for iw in 1..=nwa {
        if iw == 1 {
            n_w_to_m.push(1);
        } else if tabs.w[iw].im[5] == 1 {
            n_w_to_m.push(5);
        } else {
            n_w_to_m.push(6);
        }
        w_to_m.push(tabs.w[iw].im.to_vec());
    }

    Ok(UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

/// Write the compact EarthMesh unstructured gridfile schema used by legacy
/// refinement and mask post-processing code.
pub fn write_unstructured_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &UnstructuredMesh,
) -> io::Result<UnstructuredMeshWriteReport> {
    validate_unstructured_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let dimc = unstructured_dimc(mesh);
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("sjx_points", mesh.m_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("lbx_points", mesh.w_points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dimb", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("dimc", dimc)
        .map_err(netcdf_to_io_error)?;

    {
        let mut var = file
            .add_variable::<f64>("GLONM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATM", &["sjx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.m_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLONW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lon_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("GLATW", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&lat_values(&mesh.w_points), ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_m_to_w(&mesh.m_to_w), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_w_to_m(&mesh.w_to_m, dimc), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("n_ngrwm", &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&mesh.n_w_to_m, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(UnstructuredMeshWriteReport {
        output: output.to_path_buf(),
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
        dimc,
    })
}

/// Read the compact EarthMesh unstructured gridfile schema produced by
/// `MOD_file_preprocess.F90:Unstructured_Mesh_Save`.
pub fn read_unstructured_mesh_netcdf(input: impl AsRef<Path>) -> io::Result<UnstructuredMesh> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let sjx_points = required_dimension_len(&file, "sjx_points")?;
    let lbx_points = required_dimension_len(&file, "lbx_points")?;
    let dimb = required_dimension_len(&file, "dimb")?;
    let dimc = required_dimension_len(&file, "dimc")?;
    if dimb != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("dimb must be 3 for EarthMesh triangle connectivity, got {dimb}"),
        ));
    }

    let glonm = required_values_f64(&file, "GLONM")?;
    let glatm = required_values_f64(&file, "GLATM")?;
    let glonw = required_values_f64(&file, "GLONW")?;
    let glatw = required_values_f64(&file, "GLATW")?;
    require_len("GLONM", glonm.len(), sjx_points)?;
    require_len("GLATM", glatm.len(), sjx_points)?;
    require_len("GLONW", glonw.len(), lbx_points)?;
    require_len("GLATW", glatw.len(), lbx_points)?;

    let m_to_w_values =
        required_values_i32_matrix(&file, "itab_m%iw", "sjx_points", "dimb", sjx_points, dimb)?;
    let w_to_m_values =
        required_values_i32_matrix(&file, "itab_w%im", "lbx_points", "dimc", lbx_points, dimc)?;
    let n_w_to_m = required_values_i32(&file, "n_ngrwm")?;
    require_len("n_ngrwm", n_w_to_m.len(), lbx_points)?;

    let m_points = (0..sjx_points)
        .map(|idx| LonLatPoint {
            lon: glonm[idx],
            lat: glatm[idx],
        })
        .collect();
    let w_points = (0..lbx_points)
        .map(|idx| LonLatPoint {
            lon: glonw[idx],
            lat: glatw[idx],
        })
        .collect();
    let m_to_w = m_to_w_values
        .chunks_exact(3)
        .map(|row| [row[0], row[1], row[2]])
        .collect();
    let w_to_m = w_to_m_values
        .chunks_exact(dimc)
        .map(trim_trailing_zero_connectivity)
        .collect();

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    validate_unstructured_mesh(&mesh)?;
    Ok(mesh)
}

/// Write the `distsOnEdge_NXP####_##_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:distsOnEdge_save`.
pub fn write_dists_on_edge_netcdf(
    output: impl AsRef<Path>,
    mesh: &DistsOnEdgeMesh,
) -> io::Result<DistsOnEdgeWriteReport> {
    validate_dists_on_edge_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_edge = mesh.edge_points.len();
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_edge", num_edge)
        .map_err(netcdf_to_io_error)?;
    write_f64_1d(
        &mut file,
        "lonv",
        "num_edge",
        &lon_values(&mesh.edge_points),
    )?;
    write_f64_1d(
        &mut file,
        "latv",
        "num_edge",
        &lat_values(&mesh.edge_points),
    )?;
    write_f64_1d(&mut file, "distsOnEdge", "num_edge", &mesh.dists_on_edge)?;

    Ok(DistsOnEdgeWriteReport {
        output: output.to_path_buf(),
        num_edge,
    })
}

/// Write the `cellwidth_NXP####_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:cellwidth_save`.
pub fn write_cellwidth_netcdf(
    output: impl AsRef<Path>,
    mesh: &CellwidthMesh,
) -> io::Result<CellwidthWriteReport> {
    validate_cellwidth_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_dbx = mesh.cell_points.len();
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_dbx", num_dbx)
        .map_err(netcdf_to_io_error)?;
    write_f64_1d(&mut file, "lonw", "num_dbx", &lon_values(&mesh.cell_points))?;
    write_f64_1d(&mut file, "latw", "num_dbx", &lat_values(&mesh.cell_points))?;
    write_f64_1d(&mut file, "cellwidth", "num_dbx", &mesh.cellwidth)?;

    Ok(CellwidthWriteReport {
        output: output.to_path_buf(),
        num_dbx,
    })
}

/// Write the `quality_save_global` schema produced by
/// `MOD_file_preprocess.F90:quality_save_global`.
pub fn write_quality_global_netcdf(
    output: impl AsRef<Path>,
    quality: &GlobalQualityMesh,
) -> io::Result<GlobalQualityWriteReport> {
    validate_global_quality_mesh(quality)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_sjx = quality.sjx.length.len();
    let num_wbx = quality.wbx.length.len();
    let num_lbx = quality.lbx.length.len();
    let num_qbx = quality.qbx.as_ref().map_or(0, |qbx| qbx.length.len());

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_sjx", num_sjx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_wbx", num_wbx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_lbx", num_lbx)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("thr", 3).map_err(netcdf_to_io_error)?;
    file.add_dimension("fiv", 5).map_err(netcdf_to_io_error)?;
    file.add_dimension("six", 6).map_err(netcdf_to_io_error)?;

    write_quality_class(&mut file, "sjx", "num_sjx", "thr", &quality.sjx)?;
    write_quality_class(&mut file, "wbx", "num_wbx", "fiv", &quality.wbx)?;
    write_quality_class(&mut file, "lbx", "num_lbx", "six", &quality.lbx)?;
    if let Some(qbx) = &quality.qbx {
        file.add_dimension("num_qbx", num_qbx)
            .map_err(netcdf_to_io_error)?;
        file.add_dimension("sev", 7).map_err(netcdf_to_io_error)?;
        write_quality_class(&mut file, "qbx", "num_qbx", "sev", qbx)?;
    }

    Ok(GlobalQualityWriteReport {
        output: output.to_path_buf(),
        num_sjx,
        num_wbx,
        num_lbx,
        num_qbx,
    })
}

/// Convert the pure `Grid_Quality_Check_Global` Rust kernel output into the
/// `quality_save_global` writer payload.
pub fn global_quality_mesh_from_grid_quality(
    quality: &earthmesh_mesh::GridQualityGlobalOutput,
) -> GlobalQualityMesh {
    GlobalQualityMesh {
        sjx: quality_class_from_triangle_quality(&quality.triangle),
        wbx: quality.pentagon.as_ref().map_or_else(
            || empty_quality_class(5),
            quality_class_from_polygon_quality,
        ),
        lbx: quality.hexagon.as_ref().map_or_else(
            || empty_quality_class(6),
            quality_class_from_polygon_quality,
        ),
        qbx: quality
            .heptagon
            .as_ref()
            .map(quality_class_from_polygon_quality),
    }
}

/// Compose the migrated `Grid_Quality_Check_Global` pure output with the
/// `quality_save_global` NetCDF side effect.
pub fn write_grid_quality_global_netcdf(
    output: impl AsRef<Path>,
    quality: &earthmesh_mesh::GridQualityGlobalOutput,
) -> io::Result<GlobalQualityWriteReport> {
    let mesh = global_quality_mesh_from_grid_quality(quality);
    write_quality_global_netcdf(output, &mesh)
}

/// Persist the file side effects produced near the start of
/// `MOD_grid_preprocess.F90:Springjustment_global`.
///
/// The pure mesh kernel owns the calculations. This adapter preserves the
/// legacy result filenames and writes `distsOnEdge` for every global run, plus
/// `cellwidth` when the MPAS/MPAS-Simple distance branch produced it.  The
/// `cell_points_for_cellwidth` argument is intentionally separate because the
/// Fortran writer receives the pre-spring `wp` coordinates.
pub fn write_springjustment_global_persistence(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    cell_points_for_cellwidth: &[LonLatDegrees],
    output: &earthmesh_mesh::SpringjustmentGlobalCoreOutput,
) -> io::Result<SpringjustmentGlobalPersistenceReport> {
    let file_dir = file_dir.as_ref();
    let result_dir = file_dir.join("result");
    fs::create_dir_all(&result_dir)?;

    let edge_points = output
        .edge_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_lonlat_point)
        .collect::<Vec<_>>();
    let dists_on_edge = write_dists_on_edge_netcdf(
        result_dir.join(format!("distsOnEdge_NXP{nxp:04}_{step:02}_global.nc4")),
        &DistsOnEdgeMesh {
            edge_points,
            dists_on_edge: output.dists_on_edge.clone(),
        },
    )?;

    let cellwidth = if let Some(cellwidth) = &output.cellwidth {
        let cell_points = cell_points_for_cellwidth
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect::<Vec<_>>();
        Some(write_cellwidth_netcdf(
            result_dir.join(format!("cellwidth_NXP{nxp:04}_global.nc4")),
            &CellwidthMesh {
                cell_points,
                cellwidth: cellwidth.clone(),
            },
        )?)
    } else {
        None
    };

    Ok(SpringjustmentGlobalPersistenceReport {
        dists_on_edge,
        cellwidth,
    })
}

/// Read an `Unstructured_Mesh_Read` gridfile, run the migrated
/// `MOD_grid_preprocess.F90:Springjustment_global` core, and persist the
/// legacy `distsOnEdge`/`cellwidth` result files.
///
/// The returned mesh carries the updated triangle/cell lon-lat coordinates but
/// preserves the original gridfile connectivity; callers can pass it to
/// `write_unstructured_mesh_netcdf` to match the legacy caller's final
/// `Unstructured_Mesh_Save` side effect.
pub fn run_springjustment_global_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    options: SpringjustmentGlobalRunOptions<'_>,
) -> io::Result<SpringjustmentGlobalGridfileReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    run_springjustment_global_from_unstructured_mesh(&mesh, file_dir, nxp, step, options)
}

/// Run the migrated `Springjustment_global` core from an already-loaded
/// unstructured mesh and persist its legacy result files.
pub fn run_springjustment_global_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    options: SpringjustmentGlobalRunOptions<'_>,
) -> io::Result<SpringjustmentGlobalGridfileReport> {
    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);

    let core = springjustment_global_core_fortran_indexed(SpringjustmentGlobalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        base_dists_on_edge: options.base_dists_on_edge,
        base_cellwidth: options.base_cellwidth,
        distance_num_rc: options.distance_num_rc,
        distance_spacing: options.distance_spacing,
        distance_steps: options.distance_steps,
        niter_refine: options.niter_refine,
        relax: options.relax,
        radius: options.radius,
        diagnostic_every: options.diagnostic_every,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run Springjustment_global core from unstructured mesh",
        )
    })?;

    let persistence =
        write_springjustment_global_persistence(file_dir, nxp, step, &cell_lonlat, &core)?;
    let mesh = unstructured_mesh_from_springjustment_global(mesh, &core)?;

    Ok(SpringjustmentGlobalGridfileReport {
        core,
        persistence,
        mesh,
    })
}

/// Read an `Unstructured_Mesh_Read` gridfile and run the migrated
/// `MOD_grid_preprocess.F90:Springjustment_regional_step` core with an
/// already-derived move mask.
pub fn run_springjustment_regional_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
    options: SpringjustmentRegionalRunOptions<'_>,
) -> io::Result<SpringjustmentRegionalGridfileReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    run_springjustment_regional_from_unstructured_mesh(&mesh, options)
}

/// Run the migrated `Springjustment_regional_step` core from an already-loaded
/// unstructured mesh with an already-derived move mask.
pub fn run_springjustment_regional_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    options: SpringjustmentRegionalRunOptions<'_>,
) -> io::Result<SpringjustmentRegionalGridfileReport> {
    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);

    let core = springjustment_regional_core_fortran_indexed(SpringjustmentRegionalCoreInput {
        triangle_lonlat: &triangle_lonlat,
        cell_lonlat: &cell_lonlat,
        cells_on_triangle: &cells_on_triangle,
        triangles_on_cell: &triangles_on_cell,
        n_edges_on_cell: &n_edges_on_cell,
        move_mask: options.move_mask,
        niter_refine: options.niter_refine,
        radius: options.radius,
        diagnostic_every: options.diagnostic_every,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run Springjustment_regional_step core from unstructured mesh",
        )
    })?;

    let mesh = unstructured_mesh_from_springjustment_regional(mesh, &core)?;
    Ok(SpringjustmentRegionalGridfileReport { core, mesh })
}

/// Persist the updated gridfile payload returned by the migrated
/// `Springjustment_regional_step` adapter.
pub fn write_springjustment_regional_gridfile(
    output: impl AsRef<Path>,
    report: &SpringjustmentRegionalGridfileReport,
) -> io::Result<UnstructuredMeshWriteReport> {
    write_unstructured_mesh_netcdf(output, &report.mesh)
}

/// Read an `Unstructured_Mesh_Read` gridfile and run the migrated
/// `MOD_grid_preprocess.F90:GetEdge` production adapter.
pub fn get_edge_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<GetEdgeProductionOutput> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    get_edge_from_unstructured_mesh(&mesh)
}

/// Run the migrated `GetEdge` production adapter from a Rust unstructured mesh.
///
/// The input is expected to preserve the legacy gridfile's Fortran-compatible
/// placeholder rows, so ids in `itab_m%iw` and `itab_w%im` are passed through
/// unchanged after validation.
pub fn get_edge_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<GetEdgeProductionOutput> {
    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let triangles_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_neighbors = triangle_neighbors_from_cell_membership_fortran_indexed(
        &cells_on_triangle,
        &triangles_on_cell,
        &n_edges_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to derive triangle neighbors for GetEdge adapter",
        )
    })?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    get_edge_production_fortran_indexed(
        &triangle_neighbors,
        &cells_on_triangle,
        &triangle_lonlat,
        &cell_lonlat,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run GetEdge production adapter from unstructured mesh",
        )
    })
}

/// Read an `Unstructured_Mesh_Read` gridfile and run the migrated
/// `MOD_grid_preprocess.F90:GetArea` production adapter.
pub fn get_area_from_unstructured_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<GetAreaProductionOutput> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    get_area_from_unstructured_mesh(&mesh)
}

/// Run the migrated `GetArea` production adapter from a Rust unstructured mesh.
///
/// This composes the `GetEdge` gridfile adapter with the pure unit-sphere area
/// workflow. The returned areas are unit-sphere areas, matching the migrated
/// `earthmesh_mesh` production helper before any caller-specific radius scaling.
pub fn get_area_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<GetAreaProductionOutput> {
    let edge_output = get_edge_from_unstructured_mesh(mesh)?;
    let cells_on_vertex = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    let edge_lonlat = edge_output.edge_points.clone();
    let vertices = lonlat_points_to_unit_xyz(&triangle_lonlat);
    let cell_points = lonlat_points_to_unit_xyz(&cell_lonlat);
    let edge_points = lonlat_points_to_unit_xyz(&edge_lonlat);
    let vertices_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;

    get_area_production_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cell_points,
        cells_on_vertex: &cells_on_vertex,
        edges_on_vertex: &edge_output.edges_on_vertex,
        cells_on_edge: &edge_output.cells_on_edge,
        vertices_on_cell: &vertices_on_cell,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to run GetArea production adapter from unstructured mesh",
        )
    })
}

/// Read the MPAS edge-reference fields consumed by
/// `MOD_file_preprocess.F90:data_read`.
///
/// The returned payload preserves the Fortran placeholder row at index `0`,
/// shifts `cellsOnEdge` by `+1` after reading, converts edge coordinates from
/// radians to degrees, and applies the legacy single-step `lon > 180 => lon -=
/// 360` normalization.
pub fn read_mpas_edge_reference_netcdf(input: impl AsRef<Path>) -> io::Result<MpasEdgeReference> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let n_edges = required_dimension_len(&file, "nEdges")?;
    let two = required_dimension_len(&file, "TWO")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("TWO dimension must be 2 for MPAS cellsOnEdge, got {two}"),
        ));
    }

    let cells = required_values_i32_matrix(&file, "cellsOnEdge", "nEdges", "TWO", n_edges, 2)?;
    let lon_edge = required_values_f64(&file, "lonEdge")?;
    let lat_edge = required_values_f64(&file, "latEdge")?;
    require_len("lonEdge", lon_edge.len(), n_edges)?;
    require_len("latEdge", lat_edge.len(), n_edges)?;

    let mut cells_on_edge_reference = Vec::with_capacity(n_edges + 1);
    cells_on_edge_reference.push([1, 1]);
    for edge in 0..n_edges {
        let base = edge * 2;
        cells_on_edge_reference.push([cells[base] + 1, cells[base + 1] + 1]);
    }

    let mut edge_points = Vec::with_capacity(n_edges + 1);
    edge_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for edge in 0..n_edges {
        let mut lon = rad_to_deg(lon_edge[edge]);
        if lon > 180.0 {
            lon -= 360.0;
        }
        edge_points.push(LonLatPoint {
            lon,
            lat: rad_to_deg(lat_edge[edge]),
        });
    }

    Ok(MpasEdgeReference {
        cells_on_edge_reference,
        edge_points,
    })
}

/// Read the `cellwidth_NXP####_global.nc4` schema produced by
/// `MOD_file_preprocess.F90:cellwidth_save`.
pub fn read_cellwidth_netcdf(input: impl AsRef<Path>) -> io::Result<Vec<f64>> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let num_dbx = required_dimension_len(&file, "num_dbx")?;
    let cellwidth = required_values_f64(&file, "cellwidth")?;
    require_len("cellwidth", cellwidth.len(), num_dbx)?;
    Ok(cellwidth.into_iter().take(num_dbx).collect())
}

/// Read the `contain_*.nc4` schema produced by
/// `MOD_file_preprocess.F90:Contain_Save`.
pub fn read_contain_netcdf(input: impl AsRef<Path>) -> io::Result<ContainMesh> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let num_ustr = required_dimension_len(&file, "num_ustr")?;
    let num_ii = required_dimension_len(&file, "num_ii")?;
    let dim_a = required_dimension_len(&file, "dim_a")?;
    let dim_b = required_dimension_len(&file, "dim_b")?;
    let ustr_id_values =
        required_values_i32_matrix(&file, "ustr_id", "num_ustr", "dim_a", num_ustr, dim_a)?;
    let ustr_ii_values =
        required_values_i32_matrix(&file, "ustr_ii", "num_ii", "dim_b", num_ii, dim_b)?;
    let is_in_area_ustr = required_values_i32(&file, "IsInArea_ustr")?;
    require_len("IsInArea_ustr", is_in_area_ustr.len(), num_ustr)?;

    let contain = ContainMesh {
        ustr_id: rows_from_flat_i32(&ustr_id_values, dim_a),
        ustr_ii: rows_from_flat_i32(&ustr_ii_values, dim_b),
        is_in_area_ustr,
    };
    validate_contain_mesh(&contain)?;
    Ok(contain)
}

/// Write the `contain_*.nc4` schema consumed by
/// `MOD_file_preprocess.F90:Contain_Read`.
pub fn write_contain_netcdf(
    output: impl AsRef<Path>,
    contain: &ContainMesh,
) -> io::Result<ContainWriteReport> {
    validate_contain_mesh(contain)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_ustr = contain.ustr_id.len();
    let num_ii = contain.ustr_ii.len();
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    let dim_b = matrix_width("ustr_ii", &contain.ustr_ii)?;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ustr", num_ustr)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ii", num_ii)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dim_a", dim_a)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("dim_b", dim_b)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("ustr_id", &["num_ustr", "dim_a"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&contain.ustr_id), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("ustr_ii", &["num_ii", "dim_b"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&contain.ustr_ii), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("IsInArea_ustr", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&contain.is_in_area_ustr, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(ContainWriteReport {
        output: output.to_path_buf(),
        num_ustr,
        num_ii,
        dim_a,
        dim_b,
    })
}

/// File-backed payload for `MOD_Area_judge.F90:IsInArea_grid_Save/Read`.
///
/// The legacy Fortran path writes the selected area-mask window with
/// one-based source-index bounds.  The reader path historically looks for
/// `IsInDmArea_select`, while the save path writes `IsInArea_select`; the Rust
/// writer preserves both names so restarts remain compatible with either side.
#[derive(Debug, Clone, PartialEq)]
pub struct AreaJudgeGridPayload {
    pub bounds: AreaJudgeSourceBounds,
    pub longitude: Vec<f64>,
    pub latitude: Vec<f64>,
    pub is_in_area_select: Vec<Vec<i32>>,
    pub seaorland_select: Option<Vec<Vec<i32>>>,
}

fn validate_area_judge_grid_payload(payload: &AreaJudgeGridPayload) -> io::Result<()> {
    let expected_lon = payload
        .bounds
        .maxlon_source
        .checked_sub(payload.bounds.minlon_source)
        .and_then(|span| span.checked_add(1))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "area longitude bounds {}..{} are invalid",
                    payload.bounds.minlon_source, payload.bounds.maxlon_source
                ),
            )
        })?;
    let expected_lat = payload
        .bounds
        .minlat_source
        .checked_sub(payload.bounds.maxlat_source)
        .and_then(|span| span.checked_add(1))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "area latitude bounds {}..{} are invalid",
                    payload.bounds.maxlat_source, payload.bounds.minlat_source
                ),
            )
        })?;
    if payload.longitude.len() != expected_lon {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "longitude length {} must match selected nlons {expected_lon}",
                payload.longitude.len()
            ),
        ));
    }
    if payload.latitude.len() != expected_lat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "latitude length {} must match selected nlats {expected_lat}",
                payload.latitude.len()
            ),
        ));
    }
    validate_i32_matrix_shape(
        "IsInArea_select",
        &payload.is_in_area_select,
        expected_lon,
        expected_lat,
    )?;
    if let Some(seaorland) = payload.seaorland_select.as_ref() {
        validate_i32_matrix_shape("seaorland_select", seaorland, expected_lon, expected_lat)?;
    }
    Ok(())
}

fn validate_i32_matrix_shape(
    name: &str,
    rows: &[Vec<i32>],
    expected_rows: usize,
    expected_width: usize,
) -> io::Result<()> {
    if rows.len() != expected_rows {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} row count {} must match selected nlons {expected_rows}",
                rows.len()
            ),
        ));
    }
    let width = matrix_width(name, rows)?;
    if width != expected_width {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} width {width} must match selected nlats {expected_width}"),
        ));
    }
    Ok(())
}

fn grid_covers_area_judge_bounds_fortran_indexed<T>(
    name: &str,
    grid: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<()> {
    require_len(name, grid.len(), bounds.maxlon_source + 1)?;
    for lon_index in bounds.minlon_source..=bounds.maxlon_source {
        require_len(
            &format!("{name}[{lon_index}]"),
            grid[lon_index].len(),
            bounds.minlat_source + 1,
        )?;
    }
    Ok(())
}

/// Select a one-based source-grid window using the bounds produced by
/// `MOD_Area_judge.F90:Source_Find/minmax_range_make`.
pub fn select_area_judge_grid_fortran_indexed(
    is_in_area: &[Vec<i32>],
    seaorland: Option<&[Vec<i32>]>,
    lon_i: &[f64],
    lat_i: &[f64],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeGridPayload> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge source bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    grid_covers_area_judge_bounds_fortran_indexed("IsInArea", is_in_area, bounds)?;
    if let Some(seaorland) = seaorland {
        grid_covers_area_judge_bounds_fortran_indexed("seaorland", seaorland, bounds)?;
    }
    require_len("longitude source", lon_i.len(), bounds.maxlon_source + 1)?;
    require_len("latitude source", lat_i.len(), bounds.minlat_source + 1)?;

    let longitude = (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| lon_i[lon_index])
        .collect::<Vec<_>>();
    let latitude = (bounds.maxlat_source..=bounds.minlat_source)
        .map(|lat_index| lat_i[lat_index])
        .collect::<Vec<_>>();
    let is_in_area_select = select_i32_matrix_fortran_indexed(is_in_area, bounds);
    let seaorland_select =
        seaorland.map(|values| select_i32_matrix_fortran_indexed(values, bounds));

    let payload = AreaJudgeGridPayload {
        bounds,
        longitude,
        latitude,
        is_in_area_select,
        seaorland_select,
    };
    validate_area_judge_grid_payload(&payload)?;
    Ok(payload)
}

fn select_i32_matrix_fortran_indexed(
    values: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> Vec<Vec<i32>> {
    (bounds.minlon_source..=bounds.maxlon_source)
        .map(|lon_index| {
            (bounds.maxlat_source..=bounds.minlat_source)
                .map(|lat_index| values[lon_index][lat_index])
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Source-mask state produced by an `IsInArea_*_Calculation` input file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeAreaSourceReport {
    pub is_in_area: Vec<Vec<i32>>,
    pub bounds: AreaJudgeSourceBounds,
    pub numpatch: usize,
}

/// Summary from building and applying a patch-source mask to `seaorland`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgePatchSourceReport {
    pub bounds: AreaJudgeSourceBounds,
    pub patched_cells: usize,
}

/// Summary from applying the `mask_patch_modify` source loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgePatchModifyReport {
    pub source_reports: Vec<AreaJudgePatchSourceReport>,
    pub bounds: Option<AreaJudgeSourceBounds>,
    pub patched_cells: usize,
}

/// Domain mask state produced by `Area_judge` when `mask_domain_global` is true.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeDomainInitializationReport {
    pub is_in_domain: Vec<Vec<i32>>,
    pub bounds: AreaJudgeSourceBounds,
    pub numpatch: usize,
    pub nlons_select: usize,
    pub nlats_select: usize,
}

/// Initialize the global-domain branch of `MOD_Area_judge.F90:Area_judge`.
pub fn initialize_area_judge_global_domain_fortran_indexed(
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeDomainInitializationReport> {
    if nlons_source == 0 || nlats_source == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "global domain source dimensions must be positive",
        ));
    }

    let mut is_in_domain = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    for row in is_in_domain.iter_mut().take(nlons_source + 1).skip(1) {
        for value in row.iter_mut().take(nlats_source + 1).skip(1) {
            *value = 1;
        }
    }

    Ok(AreaJudgeDomainInitializationReport {
        is_in_domain,
        bounds: AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: nlons_source,
            maxlat_source: 1,
            minlat_source: nlats_source,
        },
        numpatch: nlons_source * nlats_source,
        nlons_select: nlons_source,
        nlats_select: nlats_source,
    })
}

/// Result of the `Area_judge` sea/land classification over the domain bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeSeaOrLandReport {
    pub seaorland: Vec<Vec<i32>>,
    pub sum_land_grid: i32,
}

/// Build `seaorland` from `IsInDmArea_grid` and `landtypes_global`.
pub fn build_area_judge_seaorland_fortran_indexed(
    is_in_domain: &[Vec<i32>],
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
    mesh_type: &str,
    refine: bool,
) -> io::Result<AreaJudgeSeaOrLandReport> {
    grid_covers_area_judge_bounds_fortran_indexed("IsInDmArea_grid", is_in_domain, bounds)?;
    grid_covers_area_judge_bounds_fortran_indexed("landtypes_global", landtypes_global, bounds)?;

    let nlons_source = is_in_domain.len().saturating_sub(1);
    let nlats_source = is_in_domain
        .get(1)
        .map(|row| row.len().saturating_sub(1))
        .unwrap_or(0);
    let mut seaorland = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];

    if mesh_type == "atmosmesh" && !refine {
        return Ok(AreaJudgeSeaOrLandReport {
            seaorland,
            sum_land_grid: 0,
        });
    }

    let mut sum_land_grid = 0_i32;
    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if is_in_domain[lon_index][lat_index] != 0
                && landtypes_global[lon_index][lat_index] != 0
            {
                seaorland[lon_index][lat_index] = 1;
                sum_land_grid += 1;
            }
        }
    }

    Ok(AreaJudgeSeaOrLandReport {
        seaorland,
        sum_land_grid,
    })
}

/// Active refine-grid state produced by `Area_judge_refine(iter=0)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeRefineActivationReport {
    pub is_in_refine: Vec<Vec<i32>>,
    pub bounds: AreaJudgeSourceBounds,
    pub nlons_select: usize,
    pub nlats_select: usize,
    pub selected_cells: usize,
}

/// Copy calculated refine state into the active refine state for
/// `MOD_Area_judge.F90:Area_judge_refine(iter == 0)`.
pub fn activate_area_judge_calculated_refine_fortran_indexed(
    is_in_refine_calculated: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeRefineActivationReport> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge refine bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    grid_covers_area_judge_bounds_fortran_indexed(
        "IsInRfArea_cal_grid",
        is_in_refine_calculated,
        bounds,
    )?;

    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let selected_cells = (bounds.maxlat_source..=bounds.minlat_source)
        .flat_map(|lat_index| {
            (bounds.minlon_source..=bounds.maxlon_source)
                .map(move |lon_index| (lon_index, lat_index))
        })
        .filter(|(lon_index, lat_index)| is_in_refine_calculated[*lon_index][*lat_index] != 0)
        .count();

    Ok(AreaJudgeRefineActivationReport {
        is_in_refine: is_in_refine_calculated.to_vec(),
        bounds,
        nlons_select,
        nlats_select,
        selected_cells,
    })
}

/// Validate the `Area_judge`/`Area_judge_refine` containment rule.
pub fn validate_area_judge_refine_within_domain_fortran_indexed(
    is_in_refine: &[Vec<i32>],
    is_in_domain: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<()> {
    grid_covers_area_judge_bounds_fortran_indexed("IsInRfArea_grid", is_in_refine, bounds)?;
    grid_covers_area_judge_bounds_fortran_indexed("IsInDmArea_grid", is_in_domain, bounds)?;

    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if is_in_refine[lon_index][lat_index] != 0 && is_in_domain[lon_index][lat_index] == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("refine area exceeds domain area at lon {lon_index} lat {lat_index}"),
                ));
            }
        }
    }

    Ok(())
}

fn merge_area_judge_source_bounds(
    current: Option<AreaJudgeSourceBounds>,
    next: AreaJudgeSourceBounds,
) -> AreaJudgeSourceBounds {
    current.map_or(next, |bounds| AreaJudgeSourceBounds {
        minlon_source: bounds.minlon_source.min(next.minlon_source),
        maxlon_source: bounds.maxlon_source.max(next.maxlon_source),
        maxlat_source: bounds.maxlat_source.min(next.maxlat_source),
        minlat_source: bounds.minlat_source.max(next.minlat_source),
    })
}

fn area_judge_patch_source_path(
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    source_index: usize,
) -> io::Result<PathBuf> {
    let count_width = match mask_patch_type {
        "close" => 3,
        "bbox" | "circle" | "lambert" => 2,
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported mask_patch_type {other}"),
            ));
        }
    };
    Ok(file_dir.as_ref().join("tmpfile").join(format!(
        "mask_patch_{mask_patch_type}_{iter}_{source_index:0count_width$}.nc4"
    )))
}

/// Apply the file-numbered source loop from `MOD_Area_judge:mask_patch_modify`.
pub fn apply_area_judge_patch_sources_fortran_indexed(
    file_dir: impl AsRef<Path>,
    mask_patch_type: &str,
    iter: usize,
    ndm: usize,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchModifyReport> {
    let mut source_reports = Vec::with_capacity(ndm);
    let mut bounds = None;
    let mut patched_cells = 0usize;

    for source_index in 1..=ndm {
        let source = area_judge_patch_source_path(&file_dir, mask_patch_type, iter, source_index)?;
        let report = match mask_patch_type {
            "bbox" => apply_area_judge_bbox_patch_source_fortran_indexed(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "circle" => apply_area_judge_circle_patch_source_fortran_indexed(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "close" => apply_area_judge_close_patch_source_fortran_indexed(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            "lambert" => apply_area_judge_lambert_patch_source_fortran_indexed(
                &source,
                seaorland,
                lon_vertex,
                lat_vertex,
                lon_i,
                lat_i,
                gridnum_perdegree,
                nlons_source,
                nlats_source,
            )?,
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported mask_patch_type {other}"),
                ));
            }
        };
        bounds = Some(merge_area_judge_source_bounds(bounds, report.bounds));
        patched_cells += report.patched_cells;
        source_reports.push(report);
    }

    Ok(AreaJudgePatchModifyReport {
        source_reports,
        bounds,
        patched_cells,
    })
}

/// Build the bbox `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_bbox_area_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let mask = read_bbox_mask_netcdf(inputfile)?;
    validate_bbox_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox area source: {err}"),
        )
    })?;
    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut merged_bounds = None;
    let mut numpatch = 0usize;

    for point in &mask.points {
        let bounds = area_judge_minmax_range_make_fortran_indexed(
            point.west,
            point.east,
            point.north,
            point.south,
            lon_vertex,
            lat_vertex,
            gridnum_perdegree,
            nlons_source,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bbox area bounds west/east/north/south = {}/{}/{}/{} are outside source grid",
                    point.west, point.east, point.north, point.south
                ),
            )
        })?;
        grid_covers_area_judge_bounds_fortran_indexed("bbox area mask", &is_in_area, bounds)?;
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            for lat_index in bounds.maxlat_source..=bounds.minlat_source {
                is_in_area[lon_index][lat_index] = 1;
            }
        }
        numpatch += (bounds.maxlon_source - bounds.minlon_source + 1)
            * (bounds.minlat_source - bounds.maxlat_source + 1);
        merged_bounds = Some(merge_area_judge_source_bounds(merged_bounds, bounds));
    }

    let bounds = merged_bounds.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox area source must contain at least one bbox point",
        )
    })?;

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}

/// Build the bbox `IsInPaArea_grid` patch mask and apply it to `seaorland`.
///
/// This is the file-backed orchestration slice of
/// `MOD_Area_judge.F90:mask_patch_modify` for bbox patch sources: read the
/// bbox source, derive Fortran one-based source bounds through
/// `minmax_range_make`, fill the selected patch grid, then call the already
/// migrated `seaorland(i,j)=0` patch core.
pub fn apply_area_judge_bbox_patch_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let source = build_area_judge_bbox_area_source_fortran_indexed(
        inputfile,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .map_err(|err| {
        io::Error::new(
            err.kind(),
            err.to_string().replace("bbox area", "bbox patch"),
        )
    })?;
    let report =
        area_judge_apply_mask_patch_fortran_indexed(seaorland, &source.is_in_area, source.bounds)
            .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seaorland or bbox patch mask does not cover selected source bounds",
            )
        })?;

    Ok(AreaJudgePatchSourceReport {
        bounds: source.bounds,
        patched_cells: report.patched_cells,
    })
}

fn area_judge_circle_scan_bounds_fortran(
    center: LonLatPoint,
    radius_km: f64,
) -> io::Result<(f64, f64, f64, f64)> {
    if !center.lon.is_finite()
        || !center.lat.is_finite()
        || !radius_km.is_finite()
        || radius_km < 0.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "circle center coordinates and radius must be finite, with non-negative radius",
        ));
    }
    let temp = std::f64::consts::PI / 180.0 * earthmesh_core::EARTH_RADIUS_METERS / 1000.0;
    let cos_lat = center.lat.to_radians().cos();
    let mut edgew_temp = center.lon - (radius_km / (temp * cos_lat)) * 1.2;
    let mut edgee_temp = center.lon + (radius_km / (temp * cos_lat)) * 1.2;
    let mut edgen_temp = center.lat + (radius_km / temp) * 1.2;
    let mut edges_temp = center.lat - (radius_km / temp) * 1.2;

    if edgee_temp > 180.0 || edgew_temp < -180.0 || edgen_temp > 90.0 || edgen_temp < -90.0 {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
    }
    if edgen_temp > 90.0 {
        edges_temp = edges_temp.min(edgen_temp);
        edgen_temp = 90.0;
    } else if edges_temp < -90.0 {
        edgen_temp = edges_temp.max(edgen_temp);
        edges_temp = -90.0;
    }
    Ok((edgew_temp, edgee_temp, edgen_temp, edges_temp))
}

/// Build the circle `IsInArea_grid` source mask used by domain/refine/patch paths.
pub fn build_area_judge_circle_area_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeAreaSourceReport> {
    let mask = read_circle_mask_netcdf(inputfile)?;
    validate_circle_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle area source: {err}"),
        )
    })?;
    let mut is_in_area = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut merged_bounds = None;
    let mut numpatch = 0usize;

    for (&center, &radius_km) in mask.points.iter().zip(mask.radius_km.iter()) {
        let (edgew_temp, edgee_temp, edgen_temp, edges_temp) =
            area_judge_circle_scan_bounds_fortran(center, radius_km)?;
        let bounds = area_judge_minmax_range_make_fortran_indexed(
            edgew_temp,
            edgee_temp,
            edgen_temp,
            edges_temp,
            lon_vertex,
            lat_vertex,
            gridnum_perdegree,
            nlons_source,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "circle area scan bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
                ),
            )
        })?;
        let minlon_source = area_judge_source_find_fortran_indexed(
            edgew_temp,
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle min longitude source",
            )
        })?;
        let maxlon_source = area_judge_source_find_fortran_indexed(
            edgee_temp,
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle max longitude source",
            )
        })?;
        let maxlat_source = area_judge_source_find_fortran_indexed(
            edgen_temp,
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle max latitude source",
            )
        })?;
        let minlat_source = area_judge_source_find_fortran_indexed(
            edges_temp,
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "missing circle min latitude source",
            )
        })?;
        if minlon_source >= maxlon_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "circle minlon_source must be smaller than maxlon_source",
            ));
        }
        if maxlat_source >= minlat_source {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "circle maxlat_source must be smaller than minlat_source",
            ));
        }
        require_len("circle area mask", is_in_area.len(), maxlon_source)?;
        require_len("circle longitude centers", lon_i.len(), maxlon_source)?;
        require_len("circle latitude centers", lat_i.len(), minlat_source)?;
        for row in is_in_area.iter().take(maxlon_source).skip(minlon_source) {
            require_len("circle area mask row", row.len(), minlat_source)?;
        }

        for lon_index in minlon_source..maxlon_source {
            for lat_index in maxlat_source..minlat_source {
                if is_in_area[lon_index][lat_index] != 0 {
                    continue;
                }
                let point = AreaJudgePoint::new(lon_i[lon_index], lat_i[lat_index]);
                let center = AreaJudgePoint::new(center.lon, center.lat);
                if is_point_in_circle_km(point, center, radius_km) {
                    is_in_area[lon_index][lat_index] = 1;
                    numpatch += 1;
                }
            }
        }
        merged_bounds = Some(merge_area_judge_source_bounds(merged_bounds, bounds));
    }

    let bounds = merged_bounds.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "circle area source must contain at least one circle",
        )
    })?;

    Ok(AreaJudgeAreaSourceReport {
        is_in_area,
        bounds,
        numpatch,
    })
}

/// Build the circle `IsInPaArea_grid` patch mask and apply it to `seaorland`.
pub fn apply_area_judge_circle_patch_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let source = build_area_judge_circle_area_source_fortran_indexed(
        inputfile,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .map_err(|err| {
        io::Error::new(
            err.kind(),
            err.to_string().replace("circle area", "circle patch"),
        )
    })?;
    let report =
        area_judge_apply_mask_patch_fortran_indexed(seaorland, &source.is_in_area, source.bounds)
            .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seaorland or circle patch mask does not cover selected source bounds",
            )
        })?;

    Ok(AreaJudgePatchSourceReport {
        bounds: source.bounds,
        patched_cells: report.patched_cells,
    })
}

fn area_judge_close_crosses_dateline(points: &[LonLatDegrees]) -> bool {
    if points.len() < 2 {
        return false;
    }
    let edgew = points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::INFINITY, f64::min);
    let edgee = points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let widest_edge = points
        .windows(2)
        .map(|pair| (pair[1].lon_degrees - pair[0].lon_degrees).abs())
        .fold(0.0, f64::max);
    widest_edge > (edgee - edgew).abs()
}

fn area_judge_check_crossing(points: &mut [LonLatDegrees]) {
    for point in points {
        if point.lon_degrees < 0.0 {
            point.lon_degrees += 180.0;
        } else {
            point.lon_degrees -= 180.0;
        }
    }
}

/// Build the close-curve `IsInPaArea_grid` patch mask and apply it to `seaorland`.
pub fn apply_area_judge_close_patch_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    let mask = read_close_mask_netcdf(inputfile)?;
    validate_close_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid close patch source: {err}"),
        )
    })?;
    let mut patch_mask = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let close_points = &mask.points;
    let geometry_points = close_points
        .iter()
        .map(|point| AreaJudgePoint::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    if let Some(intersection) = area_judge_first_self_intersection_fortran_indexed(&geometry_points)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close polygon self-intersects between segments {} and {}",
                intersection.first_segment_id, intersection.second_segment_id
            ),
        ));
    }

    let mut fill_points = close_points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect::<Vec<_>>();
    let mut edgew_temp = fill_points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::INFINITY, f64::min);
    let mut edgee_temp = fill_points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edgen_temp = fill_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = fill_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::INFINITY, f64::min);
    let restore_dateline_shift = area_judge_close_crosses_dateline(&fill_points);
    if restore_dateline_shift {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
        area_judge_check_crossing(&mut fill_points);
    }

    let bounds = area_judge_minmax_range_make_fortran_indexed(
        edgew_temp,
        edgee_temp,
        edgen_temp,
        edges_temp,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close patch bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
            ),
        )
    })?;
    let fill = area_judge_closed_curve_fill_fortran_indexed(
        &fill_points,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        restore_dateline_shift,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "close patch source could not be converted to source-grid cells",
        )
    })?;
    for (lon_index, lat_index) in fill.cells {
        require_len("close patch mask", patch_mask.len(), lon_index + 1)?;
        require_len(
            &format!("close patch mask[{lon_index}]"),
            patch_mask[lon_index].len(),
            lat_index + 1,
        )?;
        patch_mask[lon_index][lat_index] = 1;
    }
    let report = area_judge_apply_mask_patch_fortran_indexed(seaorland, &patch_mask, bounds)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seaorland or close patch mask does not cover selected source bounds",
            )
        })?;

    Ok(AreaJudgePatchSourceReport {
        bounds,
        patched_cells: report.patched_cells,
    })
}

fn area_judge_checked_source_index_minus_one(index: usize) -> usize {
    index.saturating_sub(1).max(1)
}

/// Build the Lambert/mode4 `IsInPaArea_grid` patch mask and apply it to `seaorland`.
pub fn apply_area_judge_lambert_patch_source_fortran_indexed(
    inputfile: impl AsRef<Path>,
    seaorland: &mut [Vec<i32>],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgePatchSourceReport> {
    require_len("lon_i", lon_i.len(), nlons_source + 1)?;
    require_len("lat_i", lat_i.len(), nlats_source + 1)?;

    let mesh = read_mode4_mesh_netcdf(inputfile)?;
    validate_mode4_mesh_for_area_judge(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid lambert patch source: {err}"),
        )
    })?;

    let mesh_points = &mesh.lonlat_bound[1..];
    let mut edgew_temp = mesh_points
        .iter()
        .map(|point| point.lon)
        .fold(f64::INFINITY, f64::min);
    let mut edgee_temp = mesh_points
        .iter()
        .map(|point| point.lon)
        .fold(f64::NEG_INFINITY, f64::max);
    let edgen_temp = mesh_points
        .iter()
        .map(|point| point.lat)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = mesh_points
        .iter()
        .map(|point| point.lat)
        .fold(f64::INFINITY, f64::min);
    let global_points = mesh_points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect::<Vec<_>>();
    if area_judge_close_crosses_dateline(&global_points) {
        edgew_temp = -180.0;
        edgee_temp = 180.0;
    }

    let bounds = area_judge_minmax_range_make_fortran_indexed(
        edgew_temp,
        edgee_temp,
        edgen_temp,
        edges_temp,
        lon_vertex,
        lat_vertex,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "lambert patch bounds west/east/north/south = {edgew_temp}/{edgee_temp}/{edgen_temp}/{edges_temp} are outside source grid"
            ),
        )
    })?;

    let mut patch_mask = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    for cell_index in 1..mesh.mode_points() {
        if mesh.n_ngr[cell_index] < 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} must have at least four vertices"),
            ));
        }

        let mut cell_points = mesh.ngr_bound[cell_index]
            .iter()
            .map(|&bound_index| {
                let bound_index = usize::try_from(bound_index).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "lambert mode4 cell {cell_index} has negative vertex id {bound_index}"
                        ),
                    )
                })?;
                mesh.lonlat_bound.get(bound_index - 1).copied().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "lambert mode4 cell {cell_index} references out-of-range vertex {bound_index}"
                        ),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;

        let cell_lon_span = cell_points
            .iter()
            .map(|point| point.lon)
            .fold(f64::NEG_INFINITY, f64::max)
            - cell_points
                .iter()
                .map(|point| point.lon)
                .fold(f64::INFINITY, f64::min);
        let restore_dateline_shift = cell_lon_span > 180.0;
        if restore_dateline_shift {
            let mut shifted = cell_points
                .iter()
                .map(|point| LonLatDegrees {
                    lon_degrees: point.lon,
                    lat_degrees: point.lat,
                })
                .collect::<Vec<_>>();
            area_judge_check_crossing(&mut shifted);
            for (point, shifted_point) in cell_points.iter_mut().zip(shifted) {
                point.lon = shifted_point.lon_degrees;
                point.lat = shifted_point.lat_degrees;
            }
        }

        let minlon_source = area_judge_source_find_fortran_indexed(
            cell_points
                .iter()
                .map(|point| point.lon)
                .fold(f64::INFINITY, f64::min),
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .map(area_judge_checked_source_index_minus_one)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} west edge is outside source grid"),
            )
        })?;
        let maxlon_source = area_judge_source_find_fortran_indexed(
            cell_points
                .iter()
                .map(|point| point.lon)
                .fold(f64::NEG_INFINITY, f64::max),
            lon_vertex,
            AreaJudgeAxis::Longitude,
            gridnum_perdegree,
            nlons_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} east edge is outside source grid"),
            )
        })?;
        let maxlat_source = area_judge_source_find_fortran_indexed(
            cell_points
                .iter()
                .map(|point| point.lat)
                .fold(f64::NEG_INFINITY, f64::max),
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .map(area_judge_checked_source_index_minus_one)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} north edge is outside source grid"),
            )
        })?;
        let minlat_source = area_judge_source_find_fortran_indexed(
            cell_points
                .iter()
                .map(|point| point.lat)
                .fold(f64::INFINITY, f64::min),
            lat_vertex,
            AreaJudgeAxis::Latitude,
            gridnum_perdegree,
            nlats_source,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lambert mode4 cell {cell_index} south edge is outside source grid"),
            )
        })?;

        let polygon = cell_points
            .iter()
            .map(|point| AreaJudgePoint::new(point.lon, point.lat))
            .collect::<Vec<_>>();
        for lon_index in minlon_source..maxlon_source {
            for lat_index in maxlat_source..minlat_source {
                let point = AreaJudgePoint::new(lon_i[lon_index], lat_i[lat_index]);
                if !is_point_in_convex_polygon(&polygon, point) {
                    continue;
                }
                let restored_lon_index =
                    if restore_dateline_shift && lon_index < nlons_source / 2 + 1 {
                        lon_index + nlons_source / 2
                    } else if restore_dateline_shift {
                        lon_index - nlons_source / 2
                    } else {
                        lon_index
                    };
                require_len(
                    "lambert patch mask",
                    patch_mask.len(),
                    restored_lon_index + 1,
                )?;
                require_len(
                    &format!("lambert patch mask[{restored_lon_index}]"),
                    patch_mask[restored_lon_index].len(),
                    lat_index + 1,
                )?;
                patch_mask[restored_lon_index][lat_index] = 1;
            }
        }
    }

    let report = area_judge_apply_mask_patch_fortran_indexed(seaorland, &patch_mask, bounds)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seaorland or lambert patch mask does not cover selected source bounds",
            )
        })?;

    Ok(AreaJudgePatchSourceReport {
        bounds,
        patched_cells: report.patched_cells,
    })
}

/// Write an Area_judge selected-grid restart payload.
pub fn write_area_judge_grid_netcdf(
    output: impl AsRef<Path>,
    payload: &AreaJudgeGridPayload,
) -> io::Result<()> {
    validate_area_judge_grid_payload(payload)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nlons_select", payload.longitude.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nlats_select", payload.latitude.len())
        .map_err(netcdf_to_io_error)?;
    write_i32_scalar(
        &mut file,
        "minlon_DmArea",
        usize_to_i32("minlon_DmArea", payload.bounds.minlon_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "maxlon_DmArea",
        usize_to_i32("maxlon_DmArea", payload.bounds.maxlon_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "maxlat_DmArea",
        usize_to_i32("maxlat_DmArea", payload.bounds.maxlat_source)?,
    )?;
    write_i32_scalar(
        &mut file,
        "minlat_DmArea",
        usize_to_i32("minlat_DmArea", payload.bounds.minlat_source)?,
    )?;
    write_f64_1d(&mut file, "longitude", "nlons_select", &payload.longitude)?;
    write_f64_1d(&mut file, "latitude", "nlats_select", &payload.latitude)?;
    write_i32_matrix_rows(
        &mut file,
        "IsInArea_select",
        &["nlons_select", "nlats_select"],
        &payload.is_in_area_select,
    )?;
    write_i32_matrix_rows(
        &mut file,
        "IsInDmArea_select",
        &["nlons_select", "nlats_select"],
        &payload.is_in_area_select,
    )?;
    if let Some(seaorland) = payload.seaorland_select.as_ref() {
        write_i32_matrix_rows(
            &mut file,
            "seaorland_select",
            &["nlons_select", "nlats_select"],
            seaorland,
        )?;
    }
    Ok(())
}

/// Read an Area_judge selected-grid restart payload.
pub fn read_area_judge_grid_netcdf(input: impl AsRef<Path>) -> io::Result<AreaJudgeGridPayload> {
    let file = netcdf::open(input.as_ref()).map_err(netcdf_to_io_error)?;
    let nlons = required_dimension_len(&file, "nlons_select")?;
    let nlats = required_dimension_len(&file, "nlats_select")?;
    let bounds = AreaJudgeSourceBounds {
        minlon_source: required_scalar_usize_i32(&file, "minlon_DmArea")?,
        maxlon_source: required_scalar_usize_i32(&file, "maxlon_DmArea")?,
        maxlat_source: required_scalar_usize_i32(&file, "maxlat_DmArea")?,
        minlat_source: required_scalar_usize_i32(&file, "minlat_DmArea")?,
    };
    let longitude = required_values_f64(&file, "longitude")?;
    let latitude = required_values_f64(&file, "latitude")?;
    if longitude.len() != nlons {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "longitude length {} must match nlons_select {nlons}",
                longitude.len()
            ),
        ));
    }
    if latitude.len() != nlats {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "latitude length {} must match nlats_select {nlats}",
                latitude.len()
            ),
        ));
    }
    let area_values = if let Some(values) = optional_values_i32_2d(&file, "IsInDmArea_select")? {
        values
    } else if let Some(values) = optional_values_i32_2d(&file, "IsInArea_select")? {
        values
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing IsInDmArea_select or IsInArea_select variable",
        ));
    };
    let is_in_area_select = i32_matrix_from_flat("IsInArea_select", area_values, nlons, nlats)?;
    let seaorland_select = optional_values_i32_2d(&file, "seaorland_select")?
        .map(|values| i32_matrix_from_flat("seaorland_select", values, nlons, nlats))
        .transpose()?;

    let payload = AreaJudgeGridPayload {
        bounds,
        longitude,
        latitude,
        is_in_area_select,
        seaorland_select,
    };
    validate_area_judge_grid_payload(&payload).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Area_judge grid payload: {err}"),
        )
    })?;
    Ok(payload)
}

/// Write the `patchtype_NXP*.nc4` schema produced by
/// `MOD_mask_postproc.F90:PatchID_Save`.
pub fn write_patchid_netcdf(
    output: impl AsRef<Path>,
    patch: &PatchIdMesh,
) -> io::Result<PatchIdWriteReport> {
    validate_patchid_mesh(patch)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let nlon = patch.elmindex.len();
    let nlat = matrix_width("elmindex", &patch.elmindex)?;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nlon", nlon)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nlat", nlat)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("elmindex", &["nlon", "nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&patch.elmindex), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lon_w", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lon_w, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lon_e", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lon_e, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lat_n", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lat_n, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("lat_s", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.lat_s, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("longitude", &["nlon"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.longitude, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<f64>("latitude", &["nlat"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&patch.latitude, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(PatchIdWriteReport {
        output: output.to_path_buf(),
        nlon,
        nlat,
    })
}

/// Write the `earthmesh_info.nc4` schema produced by
/// `MOD_file_preprocess.F90:LOCmesh_info_save` in the Earth postprocess branch.
pub fn write_earthmesh_info_netcdf(
    output: impl AsRef<Path>,
    info: &EarthmeshInfo,
) -> io::Result<EarthmeshInfoWriteReport> {
    validate_earthmesh_info(info)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let num_step = info.num_step_f.len();
    let num_ustr = info.refine_degree_f.len();

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num_step", num_step)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num_ustr", num_ustr)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("num_step_f", &["num_step"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.num_step_f, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("refine_degree_f", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.refine_degree_f, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("seaorland_ustr_f", &["num_ustr"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&info.seaorland_ustr_f, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(EarthmeshInfoWriteReport {
        output: output.to_path_buf(),
        num_step,
        num_ustr,
    })
}

/// Write the simple MPAS mesh schema produced by
/// `MOD_file_preprocess.F90:MPAS_Mesh_Simple_Save`.
pub fn write_mpas_simple_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasSimpleMesh,
) -> io::Result<MpasSimpleMeshWriteReport> {
    validate_mpas_simple_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let n_cells = mesh.x_cell.len() - 1;
    let n_vertices = mesh.x_vertex.len() - 1;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nCells", n_cells)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nVertices", n_vertices)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("vertexDegree", 3)
        .map_err(netcdf_to_io_error)?;

    write_f64_1d(&mut file, "xCell", "nCells", &mesh.x_cell[1..])?;
    write_f64_1d(&mut file, "yCell", "nCells", &mesh.y_cell[1..])?;
    write_f64_1d(&mut file, "zCell", "nCells", &mesh.z_cell[1..])?;
    write_f64_1d(&mut file, "xVertex", "nVertices", &mesh.x_vertex[1..])?;
    write_f64_1d(&mut file, "yVertex", "nVertices", &mesh.y_vertex[1..])?;
    write_f64_1d(&mut file, "zVertex", "nVertices", &mesh.z_vertex[1..])?;
    {
        let mut var = file
            .add_variable::<i32>("cellsOnVertex", &["nVertices", "vertexDegree"])
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("units", "-")
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("long_name", "IDs of the cells that meet at a vertex")
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flatten_i32_rows(&mesh.cells_on_vertex[1..]), (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    write_f64_1d(&mut file, "meshDensity", "nCells", &mesh.mesh_density[1..])?;

    file.add_attribute("on_a_sphere", "YES")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("sphere_radius", 1.0_f64)
        .map_err(netcdf_to_io_error)?;

    Ok(MpasSimpleMeshWriteReport {
        output: output.to_path_buf(),
        n_cells,
        n_vertices,
    })
}

/// Write the full MPAS mesh schema produced by
/// `MOD_file_preprocess.F90:MPAS_Mesh_Save`.
///
/// The Rust data shape preserves the legacy placeholder row at index `0`; all
/// MPAS-facing variables are written from index `1..`, matching Fortran slices
/// such as `2:num_dbx`, `2:num_sjx`, and `2:num_edge` after the earlier
/// zero-based connectivity conversion in `mask_postproc_Atmos`.
pub fn write_mpas_mesh_netcdf(
    output: impl AsRef<Path>,
    mesh: &MpasMesh,
) -> io::Result<MpasMeshWriteReport> {
    validate_mpas_mesh(mesh)?;
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let n_cells = mesh.x_cell.len() - 1;
    let n_vertices = mesh.x_vertex.len() - 1;
    let n_edges = mesh.x_edge.len() - 1;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("nCells", n_cells)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nVertices", n_vertices)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("nEdges", n_edges)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("maxEdges", 10)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("maxEdges2", 20)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("TWO", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("vertexDegree", 3)
        .map_err(netcdf_to_io_error)?;

    write_i32_1d(
        &mut file,
        "indexToCellID",
        "nCells",
        &one_to_n_i32(n_cells, "indexToCellID")?,
    )?;
    write_f64_1d(&mut file, "latCell", "nCells", &mesh.lat_cell[1..])?;
    write_f64_1d(&mut file, "lonCell", "nCells", &mesh.lon_cell[1..])?;
    write_f64_1d(&mut file, "xCell", "nCells", &mesh.x_cell[1..])?;
    write_f64_1d(&mut file, "yCell", "nCells", &mesh.y_cell[1..])?;
    write_f64_1d(&mut file, "zCell", "nCells", &mesh.z_cell[1..])?;
    write_i32_1d(
        &mut file,
        "indexToVertexID",
        "nVertices",
        &one_to_n_i32(n_vertices, "indexToVertexID")?,
    )?;
    write_f64_1d(&mut file, "latVertex", "nVertices", &mesh.lat_vertex[1..])?;
    write_f64_1d(&mut file, "lonVertex", "nVertices", &mesh.lon_vertex[1..])?;
    write_f64_1d(&mut file, "xVertex", "nVertices", &mesh.x_vertex[1..])?;
    write_f64_1d(&mut file, "yVertex", "nVertices", &mesh.y_vertex[1..])?;
    write_f64_1d(&mut file, "zVertex", "nVertices", &mesh.z_vertex[1..])?;
    write_i32_1d(
        &mut file,
        "indexToEdgeID",
        "nEdges",
        &one_to_n_i32(n_edges, "indexToEdgeID")?,
    )?;
    write_f64_1d(&mut file, "latEdge", "nEdges", &mesh.lat_edge[1..])?;
    write_f64_1d(&mut file, "lonEdge", "nEdges", &mesh.lon_edge[1..])?;
    write_f64_1d(&mut file, "xEdge", "nEdges", &mesh.x_edge[1..])?;
    write_f64_1d(&mut file, "yEdge", "nEdges", &mesh.y_edge[1..])?;
    write_f64_1d(&mut file, "zEdge", "nEdges", &mesh.z_edge[1..])?;
    write_i32_1d(
        &mut file,
        "nEdgesOnCell",
        "nCells",
        &mesh.n_edges_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "cellsOnCell",
        &["nCells", "maxEdges"],
        &mesh.cells_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "verticesOnCell",
        &["nCells", "maxEdges"],
        &mesh.vertices_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnCell",
        &["nCells", "maxEdges"],
        &mesh.edges_on_cell[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "cellsOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.cells_on_vertex[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.edges_on_vertex[1..],
    )?;
    write_i32_pair_rows(
        &mut file,
        "cellsOnEdge",
        &["nEdges", "TWO"],
        &mesh.cells_on_edge[1..],
    )?;
    write_i32_pair_rows(
        &mut file,
        "verticesOnEdge",
        &["nEdges", "TWO"],
        &mesh.vertices_on_edge[1..],
    )?;
    write_i32_1d(
        &mut file,
        "nEdgesOnEdge",
        "nEdges",
        &mesh.n_edges_on_edge[1..],
    )?;
    write_i32_matrix_rows(
        &mut file,
        "edgesOnEdge",
        &["nEdges", "maxEdges2"],
        &mesh.edges_on_edge[1..],
    )?;
    write_f64_1d(&mut file, "areaCell", "nCells", &mesh.area_cell[1..])?;
    write_f64_1d(
        &mut file,
        "areaTriangle",
        "nVertices",
        &mesh.area_triangle[1..],
    )?;
    write_f64_matrix_rows(
        &mut file,
        "kiteAreasOnVertex",
        &["nVertices", "vertexDegree"],
        &mesh.kite_areas_on_vertex[1..],
    )?;
    write_f64_1d(&mut file, "dvEdge", "nEdges", &mesh.dv_edge[1..])?;
    write_f64_1d(&mut file, "dcEdge", "nEdges", &mesh.dc_edge[1..])?;
    write_f64_1d(&mut file, "angleEdge", "nEdges", &mesh.angle_edge[1..])?;
    write_f64_matrix_rows(
        &mut file,
        "weightsOnEdge",
        &["nEdges", "maxEdges2"],
        &mesh.weights_on_edge[1..],
    )?;
    write_f64_1d(&mut file, "meshDensity", "nCells", &mesh.mesh_density[1..])?;
    {
        let mut var = file
            .add_variable::<f64>("nominalMinDc", &[])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&[mesh.nominal_min_dc], ..)
            .map_err(netcdf_to_io_error)?;
    }
    write_f64_1d(
        &mut file,
        "error_segment",
        "nEdges",
        &mesh.error_segment[1..],
    )?;

    file.add_attribute("mesh_spec", "1.0")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("on_a_sphere", "YES")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("sphere_radius", 1.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("is_periodic", "NO")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("x_period", 0.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("y_period", 0.0_f64)
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("file_id", "bbdd9043")
        .map_err(netcdf_to_io_error)?;
    file.add_attribute("source", "Generated by EarthMesh")
        .map_err(netcdf_to_io_error)?;

    Ok(MpasMeshWriteReport {
        output: output.to_path_buf(),
        n_cells,
        n_vertices,
        n_edges,
    })
}

/// Write the METIS-style `graph.info` text produced by
/// `MOD_file_preprocess.F90:MPAS_info_Save`.
///
/// Inputs keep the legacy placeholder row at Rust index `0`; only rows/edges
/// from index `1` onward are written or counted, matching Fortran `2:nCells`
/// and `2:nEdges` loops after MPAS zero-based connectivity conversion.
pub fn write_mpas_graph_info(
    output: impl AsRef<Path>,
    max_edges: usize,
    cells_on_cell: &[Vec<i32>],
    cells_on_edge: &[[i32; 2]],
    n_edges_on_cell: &[i32],
) -> io::Result<MpasGraphInfoWriteReport> {
    if max_edges == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_edges must be positive",
        ));
    }
    if cells_on_cell.is_empty() || cells_on_edge.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS graph info inputs must include the legacy placeholder row",
        ));
    }
    if cells_on_cell.len() != n_edges_on_cell.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "n_edges_on_cell length {} must match cells_on_cell length {}",
                n_edges_on_cell.len(),
                cells_on_cell.len()
            ),
        ));
    }
    for (idx, row) in cells_on_cell.iter().enumerate() {
        if row.len() < max_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cells_on_cell row {idx} width {} must be at least max_edges {max_edges}",
                    row.len()
                ),
            ));
        }
    }
    if n_edges_on_cell.iter().any(|&value| value < 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_edges_on_cell values must be non-negative",
        ));
    }

    let mut n_edges_on_cell_usize = Vec::with_capacity(n_edges_on_cell.len());
    for (idx, &value) in n_edges_on_cell.iter().enumerate() {
        let count = usize::try_from(value).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell row {idx} is out of range"),
            )
        })?;
        if count > max_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("n_edges_on_cell row {idx} exceeds max_edges {max_edges}"),
            ));
        }
        n_edges_on_cell_usize.push(count);
    }

    let interior_edges = cells_on_edge
        .iter()
        .skip(1)
        .filter(|edge| edge[0] != 0 && edge[1] != 0)
        .count();
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::File::create(output)?;
    writeln!(file, "{:10}{:10}", cells_on_cell.len() - 1, interior_edges)?;

    let mut cells_with_boundary_edges = 0;
    for cell_id in 1..cells_on_cell.len() {
        let expected_edges = n_edges_on_cell_usize[cell_id];
        let mut neighbors = Vec::new();
        for &neighbor in cells_on_cell[cell_id].iter().take(expected_edges) {
            if neighbor > 0 {
                neighbors.push(neighbor);
            }
        }
        for neighbor in &neighbors {
            write!(file, "{:10}", neighbor)?;
        }
        writeln!(file)?;
        if neighbors.len() < expected_edges {
            cells_with_boundary_edges += 1;
        }
    }

    Ok(MpasGraphInfoWriteReport {
        output: output.to_path_buf(),
        n_cells_written: cells_on_cell.len() - 1,
        interior_edges,
        cells_with_boundary_edges,
    })
}

/// Build the in-memory payload produced by
/// `MOD_mask_postproc.F90:MPAS_Mesh_Cal_Simple` before
/// `MPAS_Mesh_Simple_Save` writes NetCDF.
///
/// The legacy file includes a first placeholder/non-existent polygon/vertex
/// record.  This adapter preserves that record at Rust index `0`, computes
/// unit-sphere Cartesian coordinates with the same `lonlat2xyz` convention,
/// converts `ngrmw`/`m_to_w` to MPAS zero-based `cellsOnVertex`, and derives
/// `meshDensity = (min(cellwidth) / cellwidth) ** 4` using the full legacy
/// `1:num_dbx` cellwidth range.
/// Build the in-memory payload produced by `MOD_mask_postproc.F90:MPAS_Mesh_Cal`
/// before `MPAS_Mesh_Save` and `MPAS_info_Save` write side effects.
///
/// The input mesh preserves EarthMesh/Fortran indexing. The returned payload
/// keeps a placeholder row at index 0 but converts connectivity ids to MPAS
/// zero-based ids for rows written by `write_mpas_mesh_netcdf`.
pub fn build_mpas_mesh_from_unstructured_fortran_indexed(
    mesh: &UnstructuredMesh,
    cellwidth: &[f64],
    nxp: usize,
    step: usize,
) -> io::Result<MpasMesh> {
    validate_unstructured_mesh(mesh)?;
    if cellwidth.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match w_points length {}",
                cellwidth.len(),
                mesh.w_points.len()
            ),
        ));
    }
    if nxp == 0 || step == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "nxp and step must be positive for MPAS nominalMinDc",
        ));
    }
    if cellwidth
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth values must be finite and positive",
        ));
    }

    let cells_on_triangle = cells_on_triangle_fortran_indexed_from_mesh(mesh)?;
    let vertices_on_cell = triangles_on_cell_fortran_indexed_from_mesh(mesh)?;
    let n_edges_on_cell = n_edges_on_cell_usize_from_mesh(mesh)?;
    let triangle_lonlat = lonlat_degrees_from_points(&mesh.m_points);
    let cell_lonlat = lonlat_degrees_from_points(&mesh.w_points);
    let edge_output = get_edge_from_unstructured_mesh(mesh)?;

    let vertices = lonlat_points_to_unit_xyz(&triangle_lonlat);
    let cells = lonlat_points_to_unit_xyz(&cell_lonlat);
    let edge_points = lonlat_points_to_unit_xyz(&edge_output.edge_points);

    let ordered_vertices_on_cell = order_vertices_on_cell_fortran_indexed(
        &cells,
        &vertices,
        &vertices_on_cell,
        &n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, &n_edges_on_cell)
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to order MPAS verticesOnCell from unstructured mesh",
        )
    })?;
    let cell_connectivity = connect_on_cell_fortran_indexed(
        &n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        &ordered_vertices_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to build MPAS cell connectivity from unstructured mesh",
        )
    })?;

    let area = get_area_production_fortran_indexed(GetAreaUnitInput {
        vertices: &vertices,
        edge_points: &edge_points,
        cell_points: &cells,
        cells_on_vertex: &cells_on_triangle,
        edges_on_vertex: &edge_output.edges_on_vertex,
        cells_on_edge: &edge_output.cells_on_edge,
        vertices_on_cell: &ordered_vertices_on_cell,
    })
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS area payload from unstructured mesh",
        )
    })?;

    let lon_edge_degrees = edge_output
        .edge_points
        .iter()
        .map(|point| point.lon_degrees)
        .collect::<Vec<_>>();
    let lat_edge_degrees = edge_output
        .edge_points
        .iter()
        .map(|point| point.lat_degrees)
        .collect::<Vec<_>>();
    let lat_vertex_degrees = triangle_lonlat
        .iter()
        .map(|point| point.lat_degrees)
        .collect::<Vec<_>>();
    let edge_metrics = edge_distance_angle_fortran_indexed(
        &vertices,
        &cells,
        &edge_points,
        &edge_output.vertices_on_edge,
        &edge_output.cells_on_edge,
        &lat_vertex_degrees,
        &lon_edge_degrees,
        &lat_edge_degrees,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS edge distance/angle payload",
        )
    })?;

    let weights = set_weights_on_edge_fortran_indexed(
        &area.unit.area_cell,
        &edge_metrics.angle_edge,
        &edge_metrics.dc_edge,
        &edge_metrics.dv_edge,
        &area.unit.kite_areas_on_vertex,
        &cell_connectivity.edges_on_cell,
        &cells_on_triangle,
        &edge_output.cells_on_edge,
        &ordered_vertices_on_cell,
        &edge_output.vertices_on_edge,
        &n_edges_on_cell,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to compute MPAS weightsOnEdge payload",
        )
    })?;

    let (x_vertex, y_vertex, z_vertex) = split_cartesian_components(&vertices);
    let (x_cell, y_cell, z_cell) = split_cartesian_components(&cells);
    let (x_edge, y_edge, z_edge) = split_cartesian_components(&edge_points);
    let (lat_cell, lon_cell) = mpas_lat_lon_radians(&cell_lonlat);
    let (lat_vertex, lon_vertex) = mpas_lat_lon_radians(&triangle_lonlat);
    let (lat_edge, lon_edge) = mpas_lat_lon_radians(&edge_output.edge_points);

    let min_cellwidth = cellwidth.iter().copied().fold(f64::INFINITY, f64::min);
    let mesh_density = cellwidth
        .iter()
        .map(|width| (min_cellwidth / width).powi(4))
        .collect::<Vec<_>>();
    let nominal_min_dc =
        7680.0 / nxp as f64 / 2.0_f64.powi((step - 1) as i32) / earthmesh_core::EARTH_RADIUS_METERS
            * 1000.0;

    let mpas = MpasMesh {
        lat_cell,
        lon_cell,
        x_cell,
        y_cell,
        z_cell,
        lat_vertex,
        lon_vertex,
        x_vertex,
        y_vertex,
        z_vertex,
        lat_edge,
        lon_edge,
        x_edge,
        y_edge,
        z_edge,
        n_edges_on_cell: usize_values_to_i32("n_edges_on_cell", &n_edges_on_cell)?,
        cells_on_cell: zero_based_padded_rows(
            "cells_on_cell",
            &cell_connectivity.cells_on_cell,
            10,
        )?,
        vertices_on_cell: zero_based_padded_rows(
            "vertices_on_cell",
            &ordered_vertices_on_cell,
            10,
        )?,
        edges_on_cell: zero_based_padded_rows(
            "edges_on_cell",
            &cell_connectivity.edges_on_cell,
            10,
        )?,
        cells_on_vertex: zero_based_triplet_rows("cells_on_vertex", &edge_output.cells_on_vertex)?,
        edges_on_vertex: zero_based_triplet_rows("edges_on_vertex", &edge_output.edges_on_vertex)?,
        cells_on_edge: zero_based_pair_rows("cells_on_edge", &edge_output.cells_on_edge)?,
        vertices_on_edge: zero_based_pair_rows("vertices_on_edge", &edge_output.vertices_on_edge)?,
        n_edges_on_edge: usize_values_to_i32("n_edges_on_edge", &weights.n_edges_on_edge)?,
        edges_on_edge: zero_based_padded_rows("edges_on_edge", &weights.edges_on_edge, 20)?,
        area_cell: area.unit.area_cell,
        area_triangle: area.unit.area_triangle,
        kite_areas_on_vertex: area
            .unit
            .kite_areas_on_vertex
            .into_iter()
            .map(|row| row.to_vec())
            .collect(),
        dv_edge: edge_metrics.dv_edge,
        dc_edge: edge_metrics.dc_edge,
        angle_edge: edge_metrics.angle_edge,
        weights_on_edge: pad_f64_rows(&weights.weights_on_edge, 20),
        mesh_density,
        nominal_min_dc,
        error_segment: weights.error_segment,
    };
    validate_mpas_mesh(&mpas)?;
    Ok(mpas)
}

pub fn build_mpas_simple_mesh_from_unstructured_fortran_indexed(
    mesh: &UnstructuredMesh,
    cellwidth: &[f64],
) -> io::Result<MpasSimpleMesh> {
    validate_unstructured_mesh(mesh)?;
    if cellwidth.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match w_points length {}",
                cellwidth.len(),
                mesh.w_points.len()
            ),
        ));
    }
    if cellwidth.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth must include the legacy placeholder row",
        ));
    }
    if cellwidth
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cellwidth values must be finite and positive",
        ));
    }

    let vertex_xyz = lonlat_points_to_unit_xyz(&lonlat_degrees_from_points(&mesh.m_points));
    let cell_xyz = lonlat_points_to_unit_xyz(&lonlat_degrees_from_points(&mesh.w_points));
    let (x_vertex, y_vertex, z_vertex) = split_cartesian_components(&vertex_xyz);
    let (x_cell, y_cell, z_cell) = split_cartesian_components(&cell_xyz);

    let cells_on_vertex = mesh
        .m_to_w
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            row.iter()
                .map(|&value| {
                    value.checked_sub(1).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("m_to_w row {row_idx} contains non-positive cell id {value}"),
                        )
                    })
                })
                .collect::<io::Result<Vec<i32>>>()
        })
        .collect::<io::Result<Vec<Vec<i32>>>>()?;

    let min_cellwidth = cellwidth.iter().copied().fold(f64::INFINITY, f64::min);
    let mesh_density = cellwidth
        .iter()
        .map(|width| (min_cellwidth / width).powi(4))
        .collect();

    let simple = MpasSimpleMesh {
        x_cell,
        y_cell,
        z_cell,
        x_vertex,
        y_vertex,
        z_vertex,
        cells_on_vertex,
        mesh_density,
    };
    validate_mpas_simple_mesh(&simple)?;
    Ok(simple)
}

/// File-level replacement for the `MPAS_Mesh_Cal_Simple` path that reads the
/// EarthMesh gridfile plus `cellwidth_NXP####_global.nc4`, builds the simple
/// MPAS payload, and writes `MPASOUT_NXP####_global_Simple.nc4`-compatible
/// NetCDF.
pub fn write_mpas_simple_mesh_from_netcdf_inputs(
    gridfile: impl AsRef<Path>,
    cellwidth_file: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> io::Result<MpasSimpleMeshWriteReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let cellwidth = read_cellwidth_netcdf(cellwidth_file)?;
    let simple = build_mpas_simple_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth)?;
    write_mpas_simple_mesh_netcdf(output, &simple)
}

/// File-level replacement for the full `MPAS_Mesh_Cal` path that reads the
/// EarthMesh gridfile plus `cellwidth_NXP####_global.nc4`, builds the full MPAS
/// payload, and writes both `MPASOUT_NXP####_global.nc4` and graph.info.
pub fn write_mpas_mesh_from_netcdf_inputs(
    gridfile: impl AsRef<Path>,
    cellwidth_file: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    nxp: usize,
    step: usize,
) -> io::Result<MpasFullMeshPipelineReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let cellwidth = read_cellwidth_netcdf(cellwidth_file)?;
    let mpas = build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cellwidth, nxp, step)?;
    let mesh_report = write_mpas_mesh_netcdf(mesh_output, &mpas)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &mpas.cells_on_cell,
        &mpas.cells_on_edge,
        &mpas.n_edges_on_cell,
    )?;
    Ok(MpasFullMeshPipelineReport {
        mesh: mesh_report,
        graph_info,
    })
}

/// Rust entry point for the `mask_postproc_Atmos` branch when
/// `output_format == 'MPAS-Simple'`.
///
/// This preserves the legacy result-file names used by
/// `MPAS_Mesh_Cal_Simple`:
/// `result/gridfile_NXP####_<mode_grid>.nc4`,
/// `result/cellwidth_NXP####_global.nc4`, and
/// `result/MPASOUT_NXP####_global_Simple.nc4`.
pub fn write_mask_postproc_atmos_mpas_simple_netcdf(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
    mesh_type: &str,
    output_format: &str,
) -> io::Result<MpasSimpleMeshWriteReport> {
    if mesh_type.trim() != "atmosmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer requires mesh_type atmosmesh",
        ));
    }
    if output_format.trim() != "MPAS-Simple" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer requires output_format MPAS-Simple",
        ));
    }
    let mode_grid = mode_grid.trim();
    if !matches!(mode_grid, "tri" | "hex") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS-Simple mask_postproc writer supports tri or hex mode_grid only",
        ));
    }

    let file_dir = file_dir.as_ref();
    let nxpc = format!("{nxp:04}");
    let gridfile = file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxpc}_{mode_grid}.nc4"));
    let cellwidth = file_dir
        .join("result")
        .join(format!("cellwidth_NXP{nxpc}_global.nc4"));
    let output = file_dir
        .join("result")
        .join(format!("MPASOUT_NXP{nxpc}_global_Simple.nc4"));

    write_mpas_simple_mesh_from_netcdf_inputs(gridfile, cellwidth, output)
}

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Ocn` adjustment:
///
/// ```text
/// IsInDmArea_ustr = IsInDmArea_ustr_read
/// do i = num_vertex + 1, ustr_points
///   if (ustr_id(i, 1) > 0) then
///     if (ustr_id(i, 1) / real(ustr_id(i, 3)) < mask_sea_ratio) IsInDmArea_ustr(i) = -1
///   end if
/// end do
/// ```
///
/// `num_vertex` is the Fortran one-based last initial vertex id; Rust row `0`
/// corresponds to Fortran row `1`.
pub fn apply_ocean_mask_sea_ratio_fortran_indexed(
    contain: &ContainMesh,
    num_vertex: usize,
    mask_sea_ratio: f64,
) -> io::Result<Vec<i32>> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    if dim_a < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "ocean mask ratio adjustment requires ustr_id rows with at least three columns",
        ));
    }
    if num_vertex > contain.ustr_id.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_vertex {num_vertex} exceeds num_ustr {}",
                contain.ustr_id.len()
            ),
        ));
    }

    let mut is_in_domain = contain.is_in_area_ustr.clone();
    for fortran_id in (num_vertex + 1)..=contain.ustr_id.len() {
        let row_idx = fortran_id - 1;
        let selected_pixels = contain.ustr_id[row_idx][0];
        if selected_pixels <= 0 {
            continue;
        }
        let denominator = contain.ustr_id[row_idx][2];
        if denominator <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "ocean mask ratio row {fortran_id} has non-positive denominator {denominator}"
                ),
            ));
        }
        if f64::from(selected_pixels) / f64::from(denominator) < mask_sea_ratio {
            is_in_domain[row_idx] = -1;
        }
    }

    Ok(is_in_domain)
}

/// Pure-data composition of the ocean branch renewal sequence in
/// `MOD_mask_postproc.F90:mask_postproc_Ocn`.
///
/// This starts after the sea-ratio mask has been applied and before the final
/// `Data_Finial`/gridfile/OBC writers.  Hex grids only need the generic
/// `Data_Renew` compaction.  Tri grids also run the legacy triangle cleanups,
/// narrow-waterway widening, boundary-curve discovery, and isolated-ocean
/// peeling metadata.
pub fn renew_mask_postproc_ocean_domain_fortran_indexed(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocOceanRenewalReport> {
    validate_mask_postproc_layout(layout)?;
    if is_in_domain_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInDmArea_ustr length {} must cover ustr_points {}",
                is_in_domain_ustr.len(),
                layout.ustr_points
            ),
        ));
    }

    let mode_grid = mode_grid.trim();
    let mut is_in_domain = is_in_domain_ustr[..layout.ustr_points].to_vec();
    let mut renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

    match mode_grid {
        "hex" => Ok(MaskPostprocOceanRenewalReport {
            is_in_domain_ustr: is_in_domain,
            renewed,
            boundary: None,
            isolated: None,
        }),
        "tri" => {
            let mut points_new = isize::try_from(renewed.points_next).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "renewed point count does not fit isize",
                )
            })?;
            renew_mask_postproc_domain_triangles_fortran_indexed(
                &mut is_in_domain,
                &layout.vertex_neighbors,
                &renewed.vertex_neighbors_next,
                &layout.vertex_neighbor_counts,
                &renewed.vertex_neighbor_counts_next,
                &mut points_new,
            )?;
            restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
            renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

            for _ in 0..128 {
                let before_opposite = renewed.points_next;
                let mut points_new = isize::try_from(renewed.points_next).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "renewed point count does not fit isize",
                    )
                })?;
                renew_mask_postproc_opposite_domain_triangles_fortran_indexed(
                    &mut is_in_domain,
                    &layout.vertex_neighbors,
                    &layout.vertex_neighbor_counts,
                    &renewed.vertex_neighbor_counts_next,
                    &mut points_new,
                )?;
                restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
                renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

                let before_widen = renewed.points_next;
                widen_narrow_waterway_fortran_indexed(
                    &mut is_in_domain,
                    &layout.vertex_neighbors,
                    &renewed.center_neighbors_next,
                    &layout.vertex_neighbor_counts,
                    &renewed.vertex_neighbor_counts_next,
                    &renewed.center_neighbor_counts_next,
                )?;
                restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
                renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

                if renewed.points_next == before_opposite || renewed.points_next == before_widen {
                    break;
                }
            }

            let boundary = boundary_connection_fortran_indexed(
                &renewed.center_neighbors_next,
                &renewed.center_neighbor_counts_next,
                &layout.vertex_neighbor_counts,
                &renewed.vertex_neighbor_counts_next,
            )?;
            let mut vertex_neighbor_counts_after = renewed.vertex_neighbor_counts_next.clone();
            let isolated = remove_isolated_ocean_fortran_indexed(
                &mut is_in_domain,
                &layout.center_neighbors,
                &layout.center_neighbor_counts,
                &renewed.vertex_neighbors_next,
                &layout.vertex_neighbor_counts,
                &mut vertex_neighbor_counts_after,
                &boundary,
            )?;
            restore_mask_postproc_placeholders(&mut is_in_domain, is_in_domain_ustr);
            renewed = renew_mask_postproc_data_from_layout(layout, &is_in_domain, mode_grid)?;

            Ok(MaskPostprocOceanRenewalReport {
                is_in_domain_ustr: is_in_domain,
                renewed,
                boundary: Some(boundary),
                isolated: Some(isolated),
            })
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("ocean mask_postproc renewal supports tri or hex mode_grid only, got {other}"),
        )),
    }
}

fn renew_mask_postproc_data_from_layout(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocRenewedData> {
    let active_centers = is_in_domain_ustr
        .iter()
        .map(|&value| value == 1)
        .collect::<Vec<_>>();
    earthmesh_mesh::renew_mask_postproc_data_fortran_indexed(
        mode_grid,
        &active_centers,
        &layout.center_neighbors,
        &layout.center_neighbor_counts,
        layout.ustr_bounds.saturating_sub(1),
    )
}

fn restore_mask_postproc_placeholders(is_in_domain: &mut [i32], original: &[i32]) {
    for placeholder_id in 0..=1 {
        if placeholder_id < is_in_domain.len() && placeholder_id < original.len() {
            is_in_domain[placeholder_id] = original[placeholder_id];
        }
    }
}

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Earth`
/// `patchtypes_make` loop.
///
/// Rust row `0` corresponds to Fortran row `1`; output `patchtypes_select`
/// is row-major by selected longitude index, then selected latitude index.
pub fn build_earth_patchtypes_fortran_indexed(
    contain: &ContainMesh,
    mask_sea_ratio: f64,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<EarthPatchtypes> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    let dim_b = matrix_width("ustr_ii", &contain.ustr_ii)?;
    if dim_a < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "earth patchtype construction requires ustr_id rows with at least two columns",
        ));
    }
    if dim_b < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "earth patchtype construction requires ustr_ii rows with at least three columns",
        ));
    }

    let mut seaorland_ustr = vec![0_i32; contain.ustr_id.len()];
    let mut patchtypes_select = vec![vec![0_i32; nlats_dm_select]; nlons_dm_select];
    let mut sum_land_ustr = 0usize;
    let mut sum_sea_ustr = 0usize;

    for fortran_cell_id in 2..=contain.ustr_id.len() {
        let cell_idx = fortran_cell_id - 1;
        if contain.is_in_area_ustr[cell_idx] != 1 {
            continue;
        }
        let pixel_count =
            usize_from_i32_nonnegative(contain.ustr_id[cell_idx][0], "ustr_id(:,1) pixel count")?;
        let first_pixel_id =
            usize_from_i32_positive(contain.ustr_id[cell_idx][1], "ustr_id(:,2) first pixel id")?;
        if pixel_count == 0 {
            continue;
        }
        let last_pixel_id = first_pixel_id + pixel_count - 1;
        if last_pixel_id > contain.ustr_ii.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {fortran_cell_id} references pixel id {last_pixel_id}, outside 1..={}",
                    contain.ustr_ii.len()
                ),
            ));
        }

        let mut land_pixels = 0_i32;
        for fortran_pixel_id in first_pixel_id..=last_pixel_id {
            land_pixels += contain.ustr_ii[fortran_pixel_id - 1][2];
        }

        if f64::from(land_pixels) / pixel_count as f64 > mask_sea_ratio {
            seaorland_ustr[cell_idx] = 1;
            sum_land_ustr += 1;
            for fortran_pixel_id in first_pixel_id..=last_pixel_id {
                let pixel = &contain.ustr_ii[fortran_pixel_id - 1];
                if pixel[2] == 0 {
                    continue;
                }
                let (lon_idx, lat_idx) = patchtype_indices(
                    pixel[0],
                    pixel[1],
                    minlon_dm_area,
                    maxlat_dm_area,
                    nlons_dm_select,
                    nlats_dm_select,
                )?;
                patchtypes_select[lon_idx][lat_idx] =
                    i32::try_from(fortran_cell_id).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("cell id {fortran_cell_id} does not fit i32"),
                        )
                    })?;
            }
        } else {
            seaorland_ustr[cell_idx] = -1;
            sum_sea_ustr += 1;
        }
    }

    Ok(EarthPatchtypes {
        seaorland_ustr,
        patchtypes_select,
        sum_land_ustr,
        sum_sea_ustr,
    })
}

/// Pure-data port of the `MOD_mask_postproc.F90:mask_postproc_Lnd`
/// `patchtypes_make` loop.
///
/// Rust row `0` corresponds to Fortran row `1`.  `seaorland` is the selected
/// domain land mask in the same row-major layout as `patchtypes_select`.
pub fn build_land_patchtypes_fortran_indexed(
    contain: &ContainMesh,
    seaorland: &[Vec<i32>],
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<LandPatchtypes> {
    validate_contain_mesh(contain)?;
    let dim_a = matrix_width("ustr_id", &contain.ustr_id)?;
    let dim_b = matrix_width("ustr_ii", &contain.ustr_ii)?;
    let seaorland_width = matrix_width("seaorland", seaorland)?;
    if dim_a < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land patchtype construction requires ustr_id rows with at least two columns",
        ));
    }
    if dim_b < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "land patchtype construction requires ustr_ii rows with at least two columns",
        ));
    }
    if seaorland.len() != nlons_dm_select || seaorland_width != nlats_dm_select {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "seaorland shape {}x{} must match patchtype grid {nlons_dm_select}x{nlats_dm_select}",
                seaorland.len(),
                seaorland_width
            ),
        ));
    }

    let mut seaorland = seaorland.to_vec();
    let mut patchtypes_select = vec![vec![0_i32; nlats_dm_select]; nlons_dm_select];

    for fortran_cell_id in 2..=contain.ustr_id.len() {
        let cell_idx = fortran_cell_id - 1;
        if contain.is_in_area_ustr[cell_idx] == 0 {
            continue;
        }
        let pixel_count =
            usize_from_i32_nonnegative(contain.ustr_id[cell_idx][0], "ustr_id(:,1) pixel count")?;
        if pixel_count == 0 {
            continue;
        }
        let first_pixel_id =
            usize_from_i32_positive(contain.ustr_id[cell_idx][1], "ustr_id(:,2) first pixel id")?;
        let last_pixel_id = first_pixel_id + pixel_count - 1;
        if last_pixel_id > contain.ustr_ii.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {fortran_cell_id} references pixel id {last_pixel_id}, outside 1..={}",
                    contain.ustr_ii.len()
                ),
            ));
        }

        for fortran_pixel_id in first_pixel_id..=last_pixel_id {
            let pixel = &contain.ustr_ii[fortran_pixel_id - 1];
            let (lon_idx, lat_idx) = patchtype_indices(
                pixel[0],
                pixel[1],
                minlon_dm_area,
                maxlat_dm_area,
                nlons_dm_select,
                nlats_dm_select,
            )?;
            seaorland[lon_idx][lat_idx] = 0;
            patchtypes_select[lon_idx][lat_idx] = i32::try_from(fortran_cell_id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell id {fortran_cell_id} does not fit i32"),
                )
            })?;
        }
    }

    let mut filled_ignored_land_pixels = 0usize;
    for lat_idx in 0..nlats_dm_select {
        for lon_idx in 0..nlons_dm_select {
            if seaorland[lon_idx][lat_idx] == 0 {
                continue;
            }
            if lat_idx == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ignored land pixel on first latitude row has no previous latitude patch id",
                ));
            }
            let previous_patch = patchtypes_select[lon_idx][lat_idx - 1];
            if previous_patch == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "ignored land pixel previous latitude patch id is zero",
                ));
            }
            patchtypes_select[lon_idx][lat_idx] = previous_patch;
            seaorland[lon_idx][lat_idx] = 0;
            filled_ignored_land_pixels += 1;
        }
    }

    Ok(LandPatchtypes {
        seaorland,
        patchtypes_select,
        filled_ignored_land_pixels,
    })
}

/// Build the `earthmesh_info.nc4` payload from the final
/// `MOD_mask_postproc.F90:mask_postproc_Earth` role/refinement loop.
pub fn build_earthmesh_info_fortran_indexed(
    mode_grid: &str,
    num_mp_step: &[usize],
    sjx_points: usize,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    seaorland_ustr: &[i32],
) -> io::Result<EarthmeshInfo> {
    validate_mask_postproc_layout(layout)?;
    if is_in_domain_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInDmArea_ustr length {} must cover ustr_points {}",
                is_in_domain_ustr.len(),
                layout.ustr_points
            ),
        ));
    }
    if seaorland_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "seaorland_ustr length {} must cover ustr_points {}",
                seaorland_ustr.len(),
                layout.ustr_points
            ),
        ));
    }

    let mut num_step_f = num_mp_step
        .iter()
        .map(|&value| usize_to_i32("num_mp_step", value))
        .collect::<io::Result<Vec<_>>>()?;
    num_step_f.push(usize_to_i32("sjx_points", sjx_points)?);

    let active_count = is_in_domain_ustr
        .iter()
        .take(layout.ustr_points)
        .skip(2)
        .filter(|&&value| value == 1)
        .count();
    let mut seaorland_ustr_f = vec![0_i32; active_count + 2];
    let mut refine_degree_f = vec![0_i32; active_count + 2];

    let mut compact_id = 1_usize;
    match mode_grid.trim() {
        "tri" => {
            let mut step_idx = 1_usize;
            for source_id in 2..layout.ustr_points {
                if step_idx < num_step_f.len()
                    && usize::try_from(num_step_f[step_idx]).unwrap_or(usize::MAX) <= source_id
                {
                    num_step_f[step_idx] = usize_to_i32("num_step_f compact id", compact_id)?;
                    step_idx += 1;
                }
                if is_in_domain_ustr[source_id] != 1 {
                    continue;
                }
                compact_id += 1;
                seaorland_ustr_f[compact_id] = seaorland_ustr[source_id];
                refine_degree_f[compact_id] =
                    usize_to_i32("refine_degree_f", step_idx.saturating_sub(1))?;
            }
        }
        "hex" => {
            for source_id in 2..layout.ustr_points {
                let max_center_vertex = layout.center_neighbors[source_id]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0);
                let mut step_idx = 1_usize;
                while step_idx < num_step_f.len()
                    && usize::try_from(num_step_f[step_idx]).unwrap_or(usize::MAX)
                        < max_center_vertex
                {
                    step_idx += 1;
                }
                if is_in_domain_ustr[source_id] != 1 {
                    continue;
                }
                compact_id += 1;
                seaorland_ustr_f[compact_id] = seaorland_ustr[source_id];
                refine_degree_f[compact_id] =
                    usize_to_i32("refine_degree_f", step_idx.saturating_sub(1))?;
            }
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("earthmesh_info supports tri or hex mode_grid only, got {other}"),
            ));
        }
    }

    Ok(EarthmeshInfo {
        num_step_f,
        refine_degree_f,
        seaorland_ustr_f,
    })
}

/// Build the `PatchID_Save` payload from a selected-domain patch index grid and
/// the `MOD_Area_judge` lon/lat lookup arrays.
pub fn patchid_mesh_from_selected_domain(
    patchtypes_select: Vec<Vec<i32>>,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
) -> io::Result<PatchIdMesh> {
    let nlon = patchtypes_select.len();
    let nlat = matrix_width("patchtypes_select", &patchtypes_select)?;

    let mut lon_w = Vec::with_capacity(nlon);
    let mut lon_e = Vec::with_capacity(nlon);
    let mut longitude = Vec::with_capacity(nlon);
    for lon_offset in 0..nlon {
        let source_lon = usize_from_i32_nonnegative(
            minlon_dm_area
                + i32::try_from(lon_offset).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("longitude offset {lon_offset} does not fit i32"),
                    )
                })?,
            "Dmlons_source",
        )?;
        lon_w.push(lookup_f64(lon_vertex, source_lon, "lon_vertex")?);
        lon_e.push(lookup_f64(lon_vertex, source_lon + 1, "lon_vertex")?);
        longitude.push(lookup_f64(lon_i, source_lon, "lon_i")?);
    }

    let mut lat_n = Vec::with_capacity(nlat);
    let mut lat_s = Vec::with_capacity(nlat);
    let mut latitude = Vec::with_capacity(nlat);
    for lat_offset in 0..nlat {
        let source_lat = usize_from_i32_nonnegative(
            maxlat_dm_area
                - i32::try_from(lat_offset).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("latitude offset {lat_offset} does not fit i32"),
                    )
                })?,
            "Dmlats_source",
        )?;
        lat_n.push(lookup_f64(lat_vertex, source_lat, "lat_vertex")?);
        lat_s.push(lookup_f64(lat_vertex, source_lat + 1, "lat_vertex")?);
        latitude.push(lookup_f64(lat_i, source_lat, "lat_i")?);
    }

    Ok(PatchIdMesh {
        elmindex: patchtypes_select,
        lon_w,
        lon_e,
        lat_n,
        lat_s,
        longitude,
        latitude,
    })
}

/// Port of the repeated `mode_grid == 'tri'/'hex'` setup in
/// `MOD_mask_postproc.F90:mask_postproc_Earth/Lnd/Ocn`.
pub fn mask_postproc_layout_from_unstructured_mesh(
    mesh: &UnstructuredMesh,
    mode_grid: &str,
) -> io::Result<MaskPostprocLayout> {
    validate_unstructured_mesh(mesh)?;
    match mode_grid.trim() {
        "tri" => Ok(MaskPostprocLayout {
            ustr_points: mesh.m_points.len(),
            ustr_bounds: mesh.w_points.len(),
            center_points: mesh.m_points.clone(),
            vertex_points: mesh.w_points.clone(),
            center_neighbors: m_to_w_as_usize_rows(&mesh.m_to_w)?,
            vertex_neighbors: i32_rows_as_usize(&mesh.w_to_m, "itab_w%im")?,
            center_neighbor_counts: vec![3; mesh.m_points.len()],
            vertex_neighbor_counts: i32_counts_as_usize(&mesh.n_w_to_m, "n_ngrwm")?,
        }),
        "hex" => Ok(MaskPostprocLayout {
            ustr_points: mesh.w_points.len(),
            ustr_bounds: mesh.m_points.len(),
            center_points: mesh.w_points.clone(),
            vertex_points: mesh.m_points.clone(),
            center_neighbors: i32_rows_as_usize(&mesh.w_to_m, "itab_w%im")?,
            vertex_neighbors: m_to_w_as_usize_rows(&mesh.m_to_w)?,
            center_neighbor_counts: i32_counts_as_usize(&mesh.n_w_to_m, "n_ngrwm")?,
            vertex_neighbor_counts: vec![3; mesh.m_points.len()],
        }),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("mask_postproc layout supports tri or hex mode_grid only, got {other}"),
        )),
    }
}

/// Build the `Unstructured_Mesh_Save` payload used at the end of
/// `MOD_mask_postproc.F90:mask_postproc_*`.
///
/// For `tri`, the final center/vertex arrays are written directly.  For `hex`,
/// the Fortran call swaps center and vertex arguments so the legacy gridfile
/// still stores triangles in `m*` variables and polygons in `w*` variables.
pub fn unstructured_mesh_from_mask_postproc_final(
    final_data: &earthmesh_mesh::MaskPostprocFinalData,
    mode_grid: &str,
) -> io::Result<UnstructuredMesh> {
    match mode_grid.trim() {
        "tri" => Ok(UnstructuredMesh {
            m_points: lonlat_points_from_pairs(
                "center_coordinates_final",
                &final_data.center_coordinates_final,
                final_data.points_final,
            )?,
            w_points: lonlat_points_from_pairs(
                "vertex_coordinates_final",
                &final_data.vertex_coordinates_final,
                final_data.bounds_final,
            )?,
            m_to_w: rows_to_triangle_connectivity(
                "center_neighbors_final",
                &final_data.center_neighbors_final,
                final_data.points_final,
            )?,
            w_to_m: usize_rows_to_i32(
                "vertex_neighbors_final",
                &final_data.vertex_neighbors_final,
            )?,
            n_w_to_m: usize_values_to_i32(
                "vertex_neighbor_counts_final",
                &final_data.vertex_neighbor_counts_final,
            )?,
        }),
        "hex" => Ok(UnstructuredMesh {
            m_points: lonlat_points_from_pairs(
                "vertex_coordinates_final",
                &final_data.vertex_coordinates_final,
                final_data.bounds_final,
            )?,
            w_points: lonlat_points_from_pairs(
                "center_coordinates_final",
                &final_data.center_coordinates_final,
                final_data.points_final,
            )?,
            m_to_w: rows_to_triangle_connectivity(
                "vertex_neighbors_final",
                &final_data.vertex_neighbors_final,
                final_data.bounds_final,
            )?,
            w_to_m: usize_rows_to_i32(
                "center_neighbors_final",
                &final_data.center_neighbors_final,
            )?,
            n_w_to_m: usize_values_to_i32(
                "center_neighbor_counts_final",
                &final_data.center_neighbor_counts_final,
            )?,
        }),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("final mask_postproc gridfile supports tri or hex mode_grid only, got {other}"),
        )),
    }
}

/// Compose the Rust ports of the final `MOD_mask_postproc.F90:mask_postproc_*`
/// compaction steps into the gridfile payload written by `Unstructured_Mesh_Save`.
///
/// This intentionally starts after the domain-specific mask edits are already
/// represented in `IsInDmArea_ustr`; ocean-specific renewal, land patchtype
/// generation, and NetCDF I/O remain separate orchestration layers.
pub fn finalize_mask_postproc_layout_with_reindex_report(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<MaskPostprocFinalizationReport> {
    validate_mask_postproc_layout(layout)?;
    if is_in_domain_ustr.len() < layout.ustr_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInDmArea_ustr length {} must cover ustr_points {}",
                is_in_domain_ustr.len(),
                layout.ustr_points
            ),
        ));
    }

    let active_centers = is_in_domain_ustr
        .iter()
        .map(|&value| value == 1)
        .collect::<Vec<_>>();
    let center_coordinates = lonlat_pairs_from_points(&layout.center_points);
    let vertex_coordinates = lonlat_pairs_from_points(&layout.vertex_points);
    let mut final_data = earthmesh_mesh::finalize_mask_postproc_data_fortran_indexed(
        mode_grid,
        &active_centers,
        &center_coordinates,
        &vertex_coordinates,
        &layout.center_neighbors,
        &layout.center_neighbor_counts,
        layout.ustr_bounds.saturating_sub(1),
    )?;

    let unique_vertices = earthmesh_mesh::extract_unique_vertices_fortran_indexed(
        &final_data.center_neighbors_final,
        &final_data.center_neighbor_counts_final,
        layout.ustr_bounds.saturating_sub(1),
    )?;
    let vertex_reindex =
        earthmesh_mesh::sort_and_reindex_vertices(&unique_vertices, layout.ustr_bounds)?;
    final_data.center_neighbors_final =
        earthmesh_mesh::reindex_final_center_vertices_fortran_indexed(
            &final_data.center_neighbors_final,
            &final_data.center_neighbor_counts_final,
            &vertex_reindex.vertex_mapping,
        )?;

    let mesh = unstructured_mesh_from_mask_postproc_final(&final_data, mode_grid)?;
    Ok(MaskPostprocFinalizationReport {
        mesh,
        final_data,
        vertex_reindex,
    })
}

/// Compose the Rust ports of the final `MOD_mask_postproc.F90:mask_postproc_*`
/// compaction steps into the gridfile payload written by `Unstructured_Mesh_Save`.
///
/// Use `finalize_mask_postproc_layout_with_reindex_report` when downstream
/// writers need the original-vertex to final-vertex mapping, such as ocean OBC
/// boundary classification.
pub fn finalize_mask_postproc_layout_to_unstructured_mesh(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<UnstructuredMesh> {
    Ok(
        finalize_mask_postproc_layout_with_reindex_report(layout, is_in_domain_ustr, mode_grid)?
            .mesh,
    )
}

/// Compose the Earth branch role/refinement payload with the legacy
/// `result/earthmesh_info.nc4` output path.
pub fn write_mask_postproc_earth_info_netcdf(
    plan: &MaskPostprocDomainIoPlan,
    num_mp_step: &[usize],
    sjx_points: usize,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    seaorland_ustr: &[i32],
) -> io::Result<EarthmeshInfoWriteReport> {
    if plan.mesh_type != "earthmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "earthmesh_info output is only produced for earthmesh plans, got {}",
                plan.mesh_type
            ),
        ));
    }
    let info = build_earthmesh_info_fortran_indexed(
        &plan.mode_grid,
        num_mp_step,
        sjx_points,
        layout,
        is_in_domain_ustr,
        seaorland_ustr,
    )?;
    write_earthmesh_info_netcdf(plan.file_dir.join("result/earthmesh_info.nc4"), &info)
}

/// Compose `PatchID_Save` coordinate construction with the legacy patchtype
/// output path selected by `plan_mask_postproc_domain_io`.
pub fn write_mask_postproc_patchtype_netcdf(
    plan: &MaskPostprocDomainIoPlan,
    patchtypes_select: Vec<Vec<i32>>,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    lon_i: &[f64],
    lat_i: &[f64],
) -> io::Result<PatchIdWriteReport> {
    let output = plan.patchtype_output.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mask_postproc plan for {} has no patchtype_output",
                plan.mesh_type
            ),
        )
    })?;
    let patch = patchid_mesh_from_selected_domain(
        patchtypes_select,
        minlon_dm_area,
        maxlat_dm_area,
        lon_vertex,
        lat_vertex,
        lon_i,
        lat_i,
    )?;
    write_patchid_netcdf(output, &patch)
}

/// Compose final mask-postprocess grid construction with the legacy NetCDF
/// result path selected by `plan_mask_postproc_domain_io`.
pub fn write_mask_postproc_final_gridfile(
    plan: &MaskPostprocDomainIoPlan,
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = finalize_mask_postproc_layout_to_unstructured_mesh(
        layout,
        is_in_domain_ustr,
        &plan.mode_grid,
    )?;
    write_unstructured_mesh_netcdf(&plan.result_gridfile, &mesh)
}

/// Load the two NetCDF inputs common to `mask_postproc_Earth`,
/// `mask_postproc_Lnd`, and `mask_postproc_Ocn`: the source unstructured
/// gridfile and the contain-domain mask table.
pub fn read_mask_postproc_domain_inputs(
    plan: &MaskPostprocDomainIoPlan,
) -> io::Result<MaskPostprocDomainInputs> {
    let source_mesh = read_unstructured_mesh_netcdf(&plan.source_gridfile)?;
    let layout = mask_postproc_layout_from_unstructured_mesh(&source_mesh, &plan.mode_grid)?;
    let contain = read_contain_netcdf(&plan.contain_domain)?;
    let is_in_domain_ustr = contain.is_in_area_ustr.clone();

    Ok(MaskPostprocDomainInputs {
        layout,
        contain,
        is_in_domain_ustr,
    })
}

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Earth` branch.
///
/// This runner intentionally composes already-migrated pure/data helpers:
/// contain-domain reading, Earth land/sea patchtype classification, `PatchID`
/// output, final clipped gridfile writing, and `earthmesh_info.nc4` output.
pub fn run_mask_postproc_earth_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocEarthRunOptions<'_>,
) -> io::Result<MaskPostprocEarthDomainReport> {
    if plan.mesh_type != "earthmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "earth mask_postproc runner requires earthmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let patchtypes = build_earth_patchtypes_fortran_indexed(
        &inputs.contain,
        options.mask_sea_ratio,
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.nlons_dm_select,
        options.nlats_dm_select,
    )?;
    let patchtype = write_mask_postproc_patchtype_netcdf(
        plan,
        patchtypes.patchtypes_select.clone(),
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.lon_vertex,
        options.lat_vertex,
        options.lon_i,
        options.lat_i,
    )?;
    let final_gridfile =
        write_mask_postproc_final_gridfile(plan, &inputs.layout, &inputs.is_in_domain_ustr)?;
    let earthmesh_info = write_mask_postproc_earth_info_netcdf(
        plan,
        options.num_mp_step,
        options.sjx_points,
        &inputs.layout,
        &inputs.is_in_domain_ustr,
        &patchtypes.seaorland_ustr,
    )?;

    Ok(MaskPostprocEarthDomainReport {
        patchtypes,
        patchtype,
        final_gridfile,
        earthmesh_info,
    })
}

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Lnd` branch.
///
/// The source-grid clipping uses the contain-domain mask exactly like the
/// Fortran branch, while land-specific patchtype assignment is delegated to the
/// already-migrated pure `build_land_patchtypes_fortran_indexed` helper.
pub fn run_mask_postproc_land_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocLandRunOptions<'_>,
) -> io::Result<MaskPostprocLandDomainReport> {
    if plan.mesh_type != "landmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "land mask_postproc runner requires landmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let patchtypes = build_land_patchtypes_fortran_indexed(
        &inputs.contain,
        options.seaorland,
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.nlons_dm_select,
        options.nlats_dm_select,
    )?;
    let patchtype = write_mask_postproc_patchtype_netcdf(
        plan,
        patchtypes.patchtypes_select.clone(),
        options.minlon_dm_area,
        options.maxlat_dm_area,
        options.lon_vertex,
        options.lat_vertex,
        options.lon_i,
        options.lat_i,
    )?;
    let final_gridfile =
        write_mask_postproc_final_gridfile(plan, &inputs.layout, &inputs.is_in_domain_ustr)?;

    Ok(MaskPostprocLandDomainReport {
        patchtypes,
        patchtype,
        final_gridfile,
    })
}

/// File-backed composition of the migrated
/// `MOD_mask_postproc.F90:mask_postproc_Ocn` branch.
///
/// This runner composes contain-domain reading, the ocean sea-ratio mask
/// adjustment, tri/hex renewal, final gridfile writing, and tri-only boundary
/// outputs (`obc*.nc4`/`obcv2*.nc4`).
pub fn run_mask_postproc_ocean_domain(
    plan: &MaskPostprocDomainIoPlan,
    options: MaskPostprocOceanRunOptions,
) -> io::Result<MaskPostprocOceanDomainReport> {
    if plan.mesh_type != "oceanmesh" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ocean mask_postproc runner requires oceanmesh plan, got {}",
                plan.mesh_type
            ),
        ));
    }

    let inputs = read_mask_postproc_domain_inputs(plan)?;
    let ocean_mask = apply_ocean_mask_sea_ratio_fortran_indexed(
        &inputs.contain,
        options.num_vertex,
        options.mask_sea_ratio,
    )?;
    let renewal = renew_mask_postproc_ocean_domain_fortran_indexed(
        &inputs.layout,
        &ocean_mask,
        &plan.mode_grid,
    )?;
    let finalization = finalize_mask_postproc_layout_with_reindex_report(
        &inputs.layout,
        &renewal.is_in_domain_ustr,
        &plan.mode_grid,
    )?;
    let final_gridfile = write_unstructured_mesh_netcdf(&plan.result_gridfile, &finalization.mesh)?;

    let mut boundary_orders = None;
    let mut obc = None;
    let mut obcv2 = None;
    if plan.mode_grid == "tri" {
        let boundary = renewal.boundary.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean renewal did not produce boundary connection metadata",
            )
        })?;
        let isolated = renewal.isolated.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean renewal did not produce isolated-ocean metadata",
            )
        })?;
        let obcv2_output = plan.obcv2_output.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean plan is missing obcv2 output path",
            )
        })?;
        obcv2 = Some(write_obcv2_boundary_netcdf(obcv2_output, boundary)?);

        let orders = classify_boundary_orders_fortran_indexed(
            isolated.num_bdy_long,
            &isolated.bdy_long_order,
            &inputs.layout.vertex_neighbors,
            &inputs.layout.vertex_neighbor_counts,
            &finalization.vertex_reindex.vertex_mapping,
            &renewal.is_in_domain_ustr,
        )?;
        let obc_output = plan.obc_output.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "tri ocean plan is missing obc output path",
            )
        })?;
        obc = Some(write_obc_boundary_netcdf(obc_output, &orders)?);
        boundary_orders = Some(orders);
    }

    Ok(MaskPostprocOceanDomainReport {
        renewal,
        finalization,
        final_gridfile,
        boundary_orders,
        obc,
        obcv2,
    })
}

/// Legacy output path for `MOD_mask_postproc.F90:bdy_calculation`.
pub fn obc_boundary_output_path(file_dir: impl AsRef<Path>, mask_patch_on: bool) -> PathBuf {
    let filename = if mask_patch_on {
        "obc_patch.nc4"
    } else {
        "obc.nc4"
    };
    file_dir.as_ref().join("result").join(filename)
}

/// Legacy output path for `MOD_mask_postproc.F90:bdy_connection`.
pub fn obcv2_boundary_output_path(file_dir: impl AsRef<Path>, mask_patch_on: bool) -> PathBuf {
    let filename = if mask_patch_on {
        "obcv2_patch.nc4"
    } else {
        "obcv2.nc4"
    };
    file_dir.as_ref().join("result").join(filename)
}

/// Plan the legacy file names used by Earth/Lnd/Ocn mask post-processing.
///
/// This is the side-effect-free I/O contract around the migrated pure helpers:
/// source mesh and contain-domain inputs, final clipped mesh output, optional
/// land/earth `patchtype` output, and ocean-tri OBC outputs.
pub fn plan_mask_postproc_domain_io(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
    mesh_type: &str,
    mask_patch_on: bool,
) -> io::Result<MaskPostprocDomainIoPlan> {
    let file_dir = file_dir.as_ref();
    let mode_grid = mode_grid.trim();
    let mesh_type = mesh_type.trim();
    if !matches!(mode_grid, "tri" | "hex") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_postproc domain I/O supports tri or hex mode_grid only",
        ));
    }
    if !matches!(mesh_type, "earthmesh" | "landmesh" | "oceanmesh") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "domain mask_postproc I/O plan supports earthmesh, landmesh, or oceanmesh; atmosmesh uses the MPAS branch",
        ));
    }

    let nxpc = format!("{nxp:04}");
    let source_gridfile = file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxpc}_{mode_grid}.nc4"));
    let contain_domain = file_dir.join("contain").join(format!(
        "contain_{mesh_type}_domain_NXP{nxpc}_{mode_grid}.nc4"
    ));
    let result_suffix = if mask_patch_on { "_patch" } else { "" };
    let result_gridfile = file_dir.join("result").join(format!(
        "gridfile_NXP{nxpc}_{mode_grid}_{mesh_type}{result_suffix}.nc4"
    ));
    let patchtype_output = matches!(mesh_type, "earthmesh" | "landmesh").then(|| {
        file_dir
            .join("patchtype")
            .join(format!("patchtype_NXP{nxpc}_{mode_grid}.nc4"))
    });
    let writes_ocean_boundary = mesh_type == "oceanmesh" && mode_grid == "tri";
    let obc_output =
        writes_ocean_boundary.then(|| obc_boundary_output_path(file_dir, mask_patch_on));
    let obcv2_output =
        writes_ocean_boundary.then(|| obcv2_boundary_output_path(file_dir, mask_patch_on));

    Ok(MaskPostprocDomainIoPlan {
        file_dir: file_dir.to_path_buf(),
        mesh_type: mesh_type.to_string(),
        mode_grid: mode_grid.to_string(),
        source_gridfile,
        contain_domain,
        result_gridfile,
        patchtype_output,
        obc_output,
        obcv2_output,
    })
}

/// Write the `obc.nc4`/`obc_patch.nc4` schema produced by
/// `MOD_mask_postproc.F90:bdy_calculation`.
pub fn write_obc_boundary_netcdf(
    output: impl AsRef<Path>,
    orders: &BoundaryOrders,
) -> io::Result<ObcBoundaryWriteReport> {
    let bdy_num = orders.bdy_order.len();
    require_len("obc_order", orders.obc_order.len(), bdy_num)?;
    require_len("ibc_order", orders.ibc_order.len(), bdy_num)?;

    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bdy_num", bdy_num)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("bdy_order", &["bdy_num"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&usize_values_to_i32("bdy_order", &orders.bdy_order)?, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("obc_order", &["bdy_num"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&usize_values_to_i32("obc_order", &orders.obc_order)?, ..)
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("ibc_order", &["bdy_num"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&usize_values_to_i32("ibc_order", &orders.ibc_order)?, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(ObcBoundaryWriteReport {
        output: output.to_path_buf(),
        boundary_points: bdy_num,
    })
}

/// Write the `obcv2.nc4`/`obcv2_patch.nc4` schema produced by
/// `MOD_mask_postproc.F90:bdy_connection`.
pub fn write_obcv2_boundary_netcdf(
    output: impl AsRef<Path>,
    connection: &BoundaryConnection,
) -> io::Result<Obcv2BoundaryWriteReport> {
    let num1 = connection.curves.num_bdy_long[0];
    let num2 = connection.curves.num_closed_curve;
    if connection.curves.close_curves.len() < num2 + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "close_curves must include the placeholder plus num_closed_curve records",
        ));
    }
    if connection.curves.n_close_curve.len() < num2 + 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_close_curve must include the placeholder plus num_closed_curve records",
        ));
    }

    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }

    let close_curve_values =
        flatten_close_curves_for_netcdf(&connection.curves.close_curves, num1, num2)?;
    let n_close_curve_values =
        usize_values_to_i32("n_close_curve", &connection.curves.n_close_curve[1..=num2])?;

    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("num1", num1)
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("num2", num2)
        .map_err(netcdf_to_io_error)?;
    {
        let mut var = file
            .add_variable::<i32>("close_curve", &["num2", "num1"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&close_curve_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut var = file
            .add_variable::<i32>("n_close_curve", &["num2"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&n_close_curve_values, ..)
            .map_err(netcdf_to_io_error)?;
    }

    Ok(Obcv2BoundaryWriteReport {
        output: output.to_path_buf(),
        longest_curve_slots: num1,
        closed_curves: num2,
    })
}

/// Rust adapter for `mkgrd.F90:gridfile_write`: derive the unstructured
/// gridfile payload from grid state and write the legacy output path.
pub fn write_gridfile_from_state(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = gridfile_mesh_from_state(grid, tabs)?;
    let output = gridfile_output_path(file_dir, nxp, step, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

/// Rust adapter for `mkgrd.F90:gridfile_write` when upstream kernels keep
/// direct Fortran one-based arrays.
pub fn write_gridfile_from_fortran_indexed_state(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
    grid: &GridMemory,
    tabs: &IjTabs,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mesh = gridfile_mesh_from_fortran_indexed_state(grid, tabs)?;
    let output = gridfile_output_path(file_dir, nxp, step, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

/// Copy an existing EarthMesh-format mode file into the standard initial gridfile path.
pub fn copy_existing_earthmesh_mode_file(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let source = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let sjx_points = source
        .dimension("sjx_points")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing sjx_points"))?
        .len();
    let lbx_points = source
        .dimension("lbx_points")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing lbx_points"))?
        .len();
    let dimc = source
        .dimension("dimc")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mode_file missing dimc"))?
        .len();
    drop(source);

    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(mode_file, &output)?;
    Ok(UnstructuredMeshWriteReport {
        output,
        sjx_points,
        lbx_points,
        dimc,
    })
}

/// Convert an existing MPAS mesh NetCDF into the EarthMesh unstructured gridfile schema.
///
/// This ports `MOD_file_preprocess.F90:MPAS_Mesh_Read`: MPAS vertices become
/// EarthMesh M points, MPAS cells become W points, connectivity arrays are
/// shifted by one to preserve the legacy placeholder record, and longitudes are
/// converted from radians to degrees in the `[-180, 180]` range.
pub fn convert_mpas_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let n_vertices = required_dimension_len(&file, "nVertices")?;
    let n_cells = required_dimension_len(&file, "nCells")?;
    let max_edges = required_dimension_len(&file, "maxEdges")?;

    let lon_vertex = required_values_f64(&file, "lonVertex")?;
    let lat_vertex = required_values_f64(&file, "latVertex")?;
    let lon_cell = required_values_f64(&file, "lonCell")?;
    let lat_cell = required_values_f64(&file, "latCell")?;
    let cells_on_vertex = required_values_i32_2d(&file, "cellsOnVertex")?;
    let vertices_on_cell = required_values_i32_2d(&file, "verticesOnCell")?;
    let n_edges_on_cell = required_values_i32(&file, "nEdgesOnCell")?;

    require_len("lonVertex", lon_vertex.len(), n_vertices)?;
    require_len("latVertex", lat_vertex.len(), n_vertices)?;
    require_len("lonCell", lon_cell.len(), n_cells)?;
    require_len("latCell", lat_cell.len(), n_cells)?;
    require_len("cellsOnVertex", cells_on_vertex.len(), n_vertices * 3)?;
    require_len(
        "verticesOnCell",
        vertices_on_cell.len(),
        n_cells * max_edges,
    )?;
    require_len("nEdgesOnCell", n_edges_on_cell.len(), n_cells)?;

    let mut m_points = Vec::with_capacity(n_vertices + 1);
    m_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_vertices {
        m_points.push(LonLatPoint {
            lon: normalize_degrees(rad_to_deg(lon_vertex[idx])),
            lat: rad_to_deg(lat_vertex[idx]),
        });
    }

    let mut w_points = Vec::with_capacity(n_cells + 1);
    w_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_cells {
        w_points.push(LonLatPoint {
            lon: normalize_degrees(rad_to_deg(lon_cell[idx])),
            lat: rad_to_deg(lat_cell[idx]),
        });
    }

    let mut m_to_w = Vec::with_capacity(n_vertices + 1);
    m_to_w.push([1, 1, 1]);
    for vertex in 0..n_vertices {
        let base = vertex * 3;
        m_to_w.push([
            cells_on_vertex[base] + 1,
            cells_on_vertex[base + 1] + 1,
            cells_on_vertex[base + 2] + 1,
        ]);
    }

    let mut w_to_m = Vec::with_capacity(n_cells + 1);
    w_to_m.push(vec![1]);
    for cell in 0..n_cells {
        let base = cell * max_edges;
        w_to_m.push(
            vertices_on_cell[base..base + max_edges]
                .iter()
                .map(|value| value + 1)
                .collect(),
        );
    }

    let mut n_w_to_m = Vec::with_capacity(n_cells + 1);
    n_w_to_m.push(1);
    n_w_to_m.extend(n_edges_on_cell);

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

/// Convert an existing FVCOM mesh NetCDF into the EarthMesh unstructured gridfile schema.
///
/// This ports `MOD_file_preprocess.F90:FVCOM_Mesh_Read`: FVCOM elements become
/// EarthMesh M points, FVCOM nodes become W points, one placeholder record is
/// retained, connectivity arrays are shifted by one, and longitudes are wrapped
/// into the `[-180, 180]` range before writing the standard gridfile schema.
pub fn convert_fvcom_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let maxelem = required_dimension_len(&file, "maxelem")?;
    let n_nodes = required_dimension_len(&file, "node")?;
    let n_elements = required_dimension_len(&file, "nele")?;
    if maxelem < 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FVCOM maxelem must be at least 7 for EarthMesh dimc",
        ));
    }

    let lonc = required_values_f64(&file, "lonc")?;
    let latc = required_values_f64(&file, "latc")?;
    let lon = required_values_f64(&file, "lon")?;
    let lat = required_values_f64(&file, "lat")?;
    let nv = required_values_i32_matrix(&file, "nv", "nele", "three", n_elements, 3)?;
    let nbve = required_values_i32_matrix(&file, "nbve", "node", "maxelem", n_nodes, maxelem)?;
    let ntve = required_values_i32(&file, "ntve")?;

    require_len("lonc", lonc.len(), n_elements)?;
    require_len("latc", latc.len(), n_elements)?;
    require_len("lon", lon.len(), n_nodes)?;
    require_len("lat", lat.len(), n_nodes)?;
    require_len("ntve", ntve.len(), n_nodes)?;

    let mut m_points = Vec::with_capacity(n_elements + 1);
    m_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_elements {
        m_points.push(LonLatPoint {
            lon: normalize_degrees(lonc[idx]),
            lat: latc[idx],
        });
    }

    let mut w_points = Vec::with_capacity(n_nodes + 1);
    w_points.push(LonLatPoint { lon: 0.0, lat: 0.0 });
    for idx in 0..n_nodes {
        w_points.push(LonLatPoint {
            lon: normalize_degrees(lon[idx]),
            lat: lat[idx],
        });
    }

    let mut m_to_w = Vec::with_capacity(n_elements + 1);
    m_to_w.push([1, 1, 1]);
    for element in 0..n_elements {
        let base = element * 3;
        m_to_w.push([nv[base] + 1, nv[base + 1] + 1, nv[base + 2] + 1]);
    }

    let mut w_to_m = Vec::with_capacity(n_nodes + 1);
    w_to_m.push(vec![1; 7]);
    for node in 0..n_nodes {
        let base = node * maxelem;
        w_to_m.push(nbve[base..base + 7].iter().map(|value| value + 1).collect());
    }

    let mut n_w_to_m = Vec::with_capacity(n_nodes + 1);
    n_w_to_m.push(0);
    n_w_to_m.extend(ntve);

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

/// Convert an existing IAP-Ocean-style mesh NetCDF into the EarthMesh gridfile schema.
///
/// This ports the `mkgrd.F90` branch that calls
/// `MOD_grid_preprocess.F90:IAP_Mesh_make`: the source stores W-point
/// coordinates in radians plus M-to-W triangle connectivity.  The Rust path
/// rebuilds M-point spherical circumcenters, derives W-to-M adjacency, preserves
/// the legacy placeholder record, and writes `Unstructured_Mesh_Save` output.
pub fn convert_iap_ocean_mode_file_to_earthmesh(
    mode_file: impl AsRef<Path>,
    file_dir: impl AsRef<Path>,
    nxp: usize,
    mode_grid: &str,
) -> io::Result<UnstructuredMeshWriteReport> {
    let mode_file = mode_file.as_ref();
    let file = netcdf::open(mode_file).map_err(netcdf_to_io_error)?;
    let source_triangles = required_dimension_len(&file, "sjx_points")?;
    let source_vertices = required_dimension_len(&file, "lbx_points")?;
    let fortran_triangles = source_triangles + 1;
    let fortran_vertices = source_vertices + 1;

    let glonw = required_values_f64(&file, "GLONW")?;
    let glatw = required_values_f64(&file, "GLATW")?;
    let _triangles_on_triangle = required_values_i32_matrix(
        &file,
        "itab_m%im",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;
    let source_m_to_w = required_values_i32_matrix(
        &file,
        "itab_m%iw",
        "sjx_points",
        "dimb",
        source_triangles,
        3,
    )?;

    require_len("GLONW", glonw.len(), source_vertices)?;
    require_len("GLATW", glatw.len(), source_vertices)?;

    let mut w_points_fortran = vec![LonLatDegrees::new(0.0, 0.0); fortran_vertices + 1];
    for source_idx in 0..source_vertices {
        let fortran_idx = source_idx + 2;
        w_points_fortran[fortran_idx] = LonLatDegrees::new(
            normalize_degrees(rad_to_deg(glonw[source_idx])),
            rad_to_deg(glatw[source_idx]),
        );
    }

    let mut m_to_w_fortran = vec![[1_usize, 1, 1]; fortran_triangles + 1];
    for source_idx in 0..source_triangles {
        let fortran_idx = source_idx + 2;
        let base = source_idx * 3;
        m_to_w_fortran[fortran_idx] = [
            usize_from_i32_connectivity(source_m_to_w[base], "itab_m%iw")? + 1,
            usize_from_i32_connectivity(source_m_to_w[base + 1], "itab_m%iw")? + 1,
            usize_from_i32_connectivity(source_m_to_w[base + 2], "itab_m%iw")? + 1,
        ];
    }

    let centroids = centroid_spherical_mesh_fortran_indexed(&w_points_fortran, &m_to_w_fortran)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "IAP-Ocean triangle connectivity references missing W points",
            )
        })?;
    let mut centroid_xyz = lonlat_points_to_unit_xyz(&centroids);
    let mut vertex_xyz = lonlat_points_to_unit_xyz(&w_points_fortran);
    scale_cartesian_points_by_earth_radius(&mut centroid_xyz);
    scale_cartesian_points_by_earth_radius(&mut vertex_xyz);
    let circumcenters =
        circumcenter_spherical_mesh_fortran_indexed(&centroid_xyz, &vertex_xyz, &m_to_w_fortran)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "IAP-Ocean spherical circumcenter calculation failed",
                )
            })?;

    let mut m_points_fortran = vec![LonLatDegrees::new(0.0, 0.0); fortran_triangles + 1];
    for fortran_idx in 2..=fortran_triangles {
        m_points_fortran[fortran_idx] = xyz_to_lonlat_degrees(circumcenters[fortran_idx]);
    }

    let mut m_points = Vec::with_capacity(fortran_triangles);
    for fortran_idx in 1..=fortran_triangles {
        let lonlat = m_points_fortran[fortran_idx];
        m_points.push(LonLatPoint {
            lon: lonlat.lon_degrees,
            lat: lonlat.lat_degrees,
        });
    }

    let mut w_points = Vec::with_capacity(fortran_vertices);
    for point in w_points_fortran.iter().take(fortran_vertices + 1).skip(1) {
        w_points.push(LonLatPoint {
            lon: point.lon_degrees,
            lat: point.lat_degrees,
        });
    }

    let m_to_w = (1..=fortran_triangles)
        .map(|idx| {
            [
                m_to_w_fortran[idx][0] as i32,
                m_to_w_fortran[idx][1] as i32,
                m_to_w_fortran[idx][2] as i32,
            ]
        })
        .collect::<Vec<_>>();
    let (w_to_m, n_w_to_m) =
        derive_iap_w_to_m_fortran_indexed(fortran_vertices, &m_to_w_fortran, &m_points_fortran)?;

    let mesh = UnstructuredMesh {
        m_points,
        w_points,
        m_to_w,
        w_to_m,
        n_w_to_m,
    };
    let output = gridfile_output_path(file_dir, nxp, 1, mode_grid);
    write_unstructured_mesh_netcdf(output, &mesh)
}

fn required_dimension_len(file: &netcdf::File, name: &str) -> io::Result<usize> {
    file.dimension(name)
        .map(|dimension| dimension.len())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} dimension"),
            )
        })
}

fn required_values_f64(file: &netcdf::File, name: &str) -> io::Result<Vec<f64>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<f64, _>(..)
        .map_err(netcdf_to_io_error)
}

fn required_values_i32(file: &netcdf::File, name: &str) -> io::Result<Vec<i32>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<i32, _>(..)
        .map_err(netcdf_to_io_error)
}

fn required_values_i32_2d(file: &netcdf::File, name: &str) -> io::Result<Vec<i32>> {
    file.variable(name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing {name} variable"),
            )
        })?
        .get_values::<i32, _>((.., ..))
        .map_err(netcdf_to_io_error)
}

fn required_values_i32_matrix(
    file: &netcdf::File,
    name: &str,
    outer_dim: &str,
    inner_dim: &str,
    outer_len: usize,
    inner_len: usize,
) -> io::Result<Vec<i32>> {
    let variable = file.variable(name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("missing {name} variable"),
        )
    })?;
    let dimensions = variable.dimensions();
    let dimension_names = dimensions
        .iter()
        .map(|dimension| dimension.name())
        .collect::<Vec<_>>();
    let dimension_lengths = dimensions
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let values = variable
        .get_values::<i32, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    require_len(name, values.len(), outer_len * inner_len)?;

    if dimension_names == [outer_dim, inner_dim] || dimension_lengths == [outer_len, inner_len] {
        return Ok(values);
    }
    if dimension_names == [inner_dim, outer_dim] || dimension_lengths == [inner_len, outer_len] {
        let mut transposed = vec![0; outer_len * inner_len];
        for inner in 0..inner_len {
            for outer in 0..outer_len {
                transposed[outer * inner_len + inner] = values[inner * outer_len + outer];
            }
        }
        return Ok(transposed);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{name} dimensions {:?} with lengths {:?} do not match expected ({outer_dim}, {inner_dim})",
            dimension_names, dimension_lengths
        ),
    ))
}

fn usize_from_i32_connectivity(value: i32, name: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} contains negative connectivity value {value}"),
        )
    })
}

fn usize_from_i32_nonnegative(value: i32, name: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains negative value {value}"),
        )
    })
}

fn usize_from_i32_positive(value: i32, name: &str) -> io::Result<usize> {
    let converted = usize_from_i32_nonnegative(value, name)?;
    if converted == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be positive"),
        ));
    }
    Ok(converted)
}

fn patchtype_indices(
    lon_source: i32,
    lat_source: i32,
    minlon_dm_area: i32,
    maxlat_dm_area: i32,
    nlons_dm_select: usize,
    nlats_dm_select: usize,
) -> io::Result<(usize, usize)> {
    let lon_idx = lon_source - minlon_dm_area;
    let lat_idx = lat_source - maxlat_dm_area;
    if lon_idx < 0
        || lat_idx < 0
        || usize::try_from(lon_idx)
            .ok()
            .is_none_or(|idx| idx >= nlons_dm_select)
        || usize::try_from(lat_idx)
            .ok()
            .is_none_or(|idx| idx >= nlats_dm_select)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source pixel ({lon_source}, {lat_source}) is outside patchtype grid from minlon {minlon_dm_area}, maxlat {maxlat_dm_area}, shape {nlons_dm_select}x{nlats_dm_select}"
            ),
        ));
    }
    Ok((lon_idx as usize, lat_idx as usize))
}

fn lookup_f64(values: &[f64], index: usize, name: &str) -> io::Result<f64> {
    values.get(index).copied().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} does not cover source index {index}",
                values.len()
            ),
        )
    })
}

fn m_to_w_as_usize_rows(rows: &[[i32; 3]]) -> io::Result<Vec<Vec<usize>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|&value| usize_from_i32_connectivity(value, "itab_m%iw"))
                .collect()
        })
        .collect()
}

fn i32_rows_as_usize(rows: &[Vec<i32>], name: &str) -> io::Result<Vec<Vec<usize>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .map(|&value| usize_from_i32_connectivity(value, name))
                .collect()
        })
        .collect()
}

fn i32_counts_as_usize(values: &[i32], name: &str) -> io::Result<Vec<usize>> {
    values
        .iter()
        .map(|&value| usize_from_i32_connectivity(value, name))
        .collect()
}

fn validate_mask_postproc_layout(layout: &MaskPostprocLayout) -> io::Result<()> {
    for (name, actual, required) in [
        (
            "center_points",
            layout.center_points.len(),
            layout.ustr_points,
        ),
        (
            "center_neighbors",
            layout.center_neighbors.len(),
            layout.ustr_points,
        ),
        (
            "center_neighbor_counts",
            layout.center_neighbor_counts.len(),
            layout.ustr_points,
        ),
        (
            "vertex_points",
            layout.vertex_points.len(),
            layout.ustr_bounds,
        ),
        (
            "vertex_neighbors",
            layout.vertex_neighbors.len(),
            layout.ustr_bounds,
        ),
        (
            "vertex_neighbor_counts",
            layout.vertex_neighbor_counts.len(),
            layout.ustr_bounds,
        ),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    Ok(())
}

fn lonlat_pairs_from_points(points: &[LonLatPoint]) -> Vec<[f64; 2]> {
    points.iter().map(|point| [point.lon, point.lat]).collect()
}

fn lonlat_points_from_pairs(
    name: &str,
    values: &[[f64; 2]],
    expected_final_id: usize,
) -> io::Result<Vec<LonLatPoint>> {
    if values.len() <= expected_final_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must include final id {expected_final_id}",
                values.len()
            ),
        ));
    }
    Ok(values
        .iter()
        .map(|point| LonLatPoint {
            lon: point[0],
            lat: point[1],
        })
        .collect())
}

fn rows_to_triangle_connectivity(
    name: &str,
    rows: &[Vec<usize>],
    expected_final_id: usize,
) -> io::Result<Vec<[i32; 3]>> {
    if rows.len() <= expected_final_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must include final id {expected_final_id}",
                rows.len()
            ),
        ));
    }
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            if row.len() < 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} row {row_idx} must contain at least three connectivity slots"),
                ));
            }
            Ok([
                usize_to_i32(name, row[0])?,
                usize_to_i32(name, row[1])?,
                usize_to_i32(name, row[2])?,
            ])
        })
        .collect()
}

fn usize_rows_to_i32(name: &str, rows: &[Vec<usize>]) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .map(|row| usize_values_to_i32(name, row))
        .collect()
}

fn usize_to_i32(name: &str, value: usize) -> io::Result<i32> {
    i32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains value {value} that does not fit NetCDF INT"),
        )
    })
}

fn lonlat_degrees_from_points(points: &[LonLatPoint]) -> Vec<LonLatDegrees> {
    points
        .iter()
        .map(|point| LonLatDegrees {
            lon_degrees: point.lon,
            lat_degrees: point.lat,
        })
        .collect()
}

fn split_cartesian_components(points: &[CartesianPoint]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(points.len());
    let mut y = Vec::with_capacity(points.len());
    let mut z = Vec::with_capacity(points.len());
    for point in points {
        x.push(point.x);
        y.push(point.y);
        z.push(point.z);
    }
    (x, y, z)
}

fn scale_cartesian_points_by_earth_radius(points: &mut [CartesianPoint]) {
    for point in points {
        point.x *= earthmesh_core::EARTH_RADIUS_METERS;
        point.y *= earthmesh_core::EARTH_RADIUS_METERS;
        point.z *= earthmesh_core::EARTH_RADIUS_METERS;
    }
}

fn derive_iap_w_to_m_fortran_indexed(
    fortran_vertices: usize,
    m_to_w_fortran: &[[usize; 3]],
    m_points_fortran: &[LonLatDegrees],
) -> io::Result<(Vec<Vec<i32>>, Vec<i32>)> {
    let mut incident = vec![Vec::<usize>::new(); fortran_vertices + 1];
    for triangle_id in 2..m_to_w_fortran.len() {
        for &vertex_id in &m_to_w_fortran[triangle_id] {
            if vertex_id == 0 || vertex_id > fortran_vertices {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "IAP-Ocean triangle {triangle_id} references W point {vertex_id}, outside 1..={fortran_vertices}"
                    ),
                ));
            }
            incident[vertex_id].push(triangle_id);
        }
    }

    let maxnum = incident
        .iter()
        .take(fortran_vertices + 1)
        .skip(1)
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(7);
    let mut w_to_m = Vec::with_capacity(fortran_vertices);
    let mut n_w_to_m = Vec::with_capacity(fortran_vertices);
    for vertex_id in 1..=fortran_vertices {
        let sorted =
            sort_iap_incident_triangles(&incident[vertex_id], m_to_w_fortran, m_points_fortran)?;
        n_w_to_m.push(i32::try_from(incident[vertex_id].len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "IAP-Ocean W point has too many incident triangles",
            )
        })?);
        let mut row = vec![1; maxnum];
        for (slot, triangle_id) in sorted.iter().copied().enumerate() {
            row[slot] = i32::try_from(triangle_id).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "IAP-Ocean triangle id exceeds i32 range",
                )
            })?;
        }
        w_to_m.push(row);
    }
    Ok((w_to_m, n_w_to_m))
}

fn sort_iap_incident_triangles(
    incident: &[usize],
    m_to_w_fortran: &[[usize; 3]],
    m_points_fortran: &[LonLatDegrees],
) -> io::Result<Vec<usize>> {
    if incident.len() <= 1 {
        return Ok(incident.to_vec());
    }

    let mut neighbor_degree = vec![0; incident.len()];
    for (idx, &triangle_id) in incident.iter().enumerate() {
        for (other_idx, &other_triangle_id) in incident.iter().enumerate() {
            if idx == other_idx {
                continue;
            }
            if iap_triangles_are_neighbors(
                m_to_w_fortran[triangle_id],
                m_to_w_fortran[other_triangle_id],
            ) {
                neighbor_degree[idx] += 1;
            }
        }
    }

    let start_pos = neighbor_degree
        .iter()
        .position(|&degree| degree == 1)
        .unwrap_or(0);
    let mut used = vec![false; incident.len()];
    let mut ordered = Vec::with_capacity(incident.len());
    let mut ref_triangle = incident[start_pos];
    used[start_pos] = true;
    ordered.push(ref_triangle);

    while ordered.len() < incident.len() {
        let mut found_pos = None;
        for (idx, &candidate) in incident.iter().enumerate() {
            if used[idx] {
                continue;
            }
            if iap_triangles_are_neighbors(m_to_w_fortran[ref_triangle], m_to_w_fortran[candidate])
            {
                found_pos = Some(idx);
                break;
            }
        }
        if found_pos.is_none() {
            found_pos = used.iter().position(|is_used| !*is_used);
        }
        let Some(pos) = found_pos else {
            break;
        };
        ref_triangle = incident[pos];
        used[pos] = true;
        ordered.push(ref_triangle);
    }

    let area = robust_spherical_area_degrees(
        &ordered
            .iter()
            .map(|&triangle_id| {
                m_points_fortran.get(triangle_id).copied().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "IAP-Ocean sorted triangle id missing M point",
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?,
    );
    if area < 0.0 {
        ordered.reverse();
    }
    Ok(ordered)
}

fn iap_triangles_are_neighbors(a: [usize; 3], b: [usize; 3]) -> bool {
    let shared = a
        .iter()
        .filter(|&&vertex_id| b.contains(&vertex_id))
        .count();
    shared >= 2
}

fn robust_spherical_area_degrees(points: &[LonLatDegrees]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let mut area = 0.0;
    for idx in 0..points.len() {
        let next = (idx + 1) % points.len();
        let mut delta_lon = (points[next].lon_degrees - points[idx].lon_degrees).to_radians();
        if delta_lon > std::f64::consts::PI {
            delta_lon -= 2.0 * std::f64::consts::PI;
        } else if delta_lon < -std::f64::consts::PI {
            delta_lon += 2.0 * std::f64::consts::PI;
        }
        area += delta_lon
            * (2.0
                + points[idx].lat_degrees.to_radians().sin()
                + points[next].lat_degrees.to_radians().sin());
    }
    area / 2.0
}

fn rad_to_deg(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

fn normalize_degrees(mut degrees: f64) -> f64 {
    if degrees > 180.0 {
        degrees -= 360.0;
    }
    if degrees < -180.0 {
        degrees += 360.0;
    }
    degrees
}

/// Build the `gridfile_write` output path:
/// `file_dir/gridfile/gridfile_NXP####_##_<mode_grid>.nc4`.
pub fn gridfile_output_path(
    file_dir: impl AsRef<Path>,
    nxp: usize,
    step: usize,
    mode_grid: &str,
) -> PathBuf {
    file_dir.as_ref().join("gridfile").join(format!(
        "gridfile_NXP{nxp:04}_{step:02}_{}.nc4",
        mode_grid.trim()
    ))
}

fn require_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required {required}"),
        ));
    }
    Ok(())
}

fn validate_unstructured_mesh(mesh: &UnstructuredMesh) -> io::Result<()> {
    if mesh.m_to_w.len() != mesh.m_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "m_to_w length must match m_points length",
        ));
    }
    if mesh.w_to_m.len() != mesh.w_points.len() || mesh.n_w_to_m.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "w_to_m and n_w_to_m lengths must match w_points length",
        ));
    }
    if mesh.n_w_to_m.iter().any(|&n| n < 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_w_to_m values must be non-negative",
        ));
    }
    Ok(())
}

fn validate_contain_mesh(contain: &ContainMesh) -> io::Result<()> {
    matrix_width("ustr_id", &contain.ustr_id)?;
    matrix_width("ustr_ii", &contain.ustr_ii)?;
    if contain.is_in_area_ustr.len() != contain.ustr_id.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "IsInArea_ustr length {} must match num_ustr {}",
                contain.is_in_area_ustr.len(),
                contain.ustr_id.len()
            ),
        ));
    }
    Ok(())
}

fn validate_patchid_mesh(patch: &PatchIdMesh) -> io::Result<()> {
    let nlon = patch.elmindex.len();
    let nlat = matrix_width("elmindex", &patch.elmindex)?;
    for (name, actual, required) in [
        ("lon_w", patch.lon_w.len(), nlon),
        ("lon_e", patch.lon_e.len(), nlon),
        ("longitude", patch.longitude.len(), nlon),
        ("lat_n", patch.lat_n.len(), nlat),
        ("lat_s", patch.lat_s.len(), nlat),
        ("latitude", patch.latitude.len(), nlat),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    Ok(())
}

fn validate_earthmesh_info(info: &EarthmeshInfo) -> io::Result<()> {
    if info.refine_degree_f.len() != info.seaorland_ustr_f.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refine_degree_f and seaorland_ustr_f must have matching length: {} != {}",
                info.refine_degree_f.len(),
                info.seaorland_ustr_f.len()
            ),
        ));
    }
    Ok(())
}

fn validate_cellwidth_mesh(mesh: &CellwidthMesh) -> io::Result<()> {
    if mesh.cellwidth.len() != mesh.cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cellwidth length {} must match cell_points length {}",
                mesh.cellwidth.len(),
                mesh.cell_points.len()
            ),
        ));
    }
    Ok(())
}

fn validate_dists_on_edge_mesh(mesh: &DistsOnEdgeMesh) -> io::Result<()> {
    if mesh.dists_on_edge.len() != mesh.edge_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "dists_on_edge length {} must match edge_points length {}",
                mesh.dists_on_edge.len(),
                mesh.edge_points.len()
            ),
        ));
    }
    Ok(())
}

fn validate_global_quality_mesh(quality: &GlobalQualityMesh) -> io::Result<()> {
    validate_quality_class_metrics("sjx", &quality.sjx, 3)?;
    validate_quality_class_metrics("wbx", &quality.wbx, 5)?;
    validate_quality_class_metrics("lbx", &quality.lbx, 6)?;
    if let Some(qbx) = &quality.qbx {
        validate_quality_class_metrics("qbx", qbx, 7)?;
    }
    Ok(())
}

fn validate_quality_class_metrics(
    class_name: &str,
    metrics: &QualityClassMetrics,
    width: usize,
) -> io::Result<()> {
    let rows = metrics.length.len();
    if metrics.angle.len() != rows {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{class_name} angle row count {} must match length row count {rows}",
                metrics.angle.len()
            ),
        ));
    }
    for (name, actual) in [("less", metrics.less.len()), ("more", metrics.more.len())] {
        if actual != rows {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{class_name} {name} length {actual} must match row count {rows}"),
            ));
        }
    }
    for (idx, row) in metrics.length.iter().enumerate() {
        if row.len() != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{class_name} length row {idx} width {} must match required {width}",
                    row.len()
                ),
            ));
        }
    }
    for (idx, row) in metrics.angle.iter().enumerate() {
        if row.len() != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{class_name} angle row {idx} width {} must match required {width}",
                    row.len()
                ),
            ));
        }
    }
    Ok(())
}

fn quality_class_from_triangle_quality(
    output: &earthmesh_mesh::TriangleMeshQualityFortranOutput,
) -> QualityClassMetrics {
    QualityClassMetrics {
        length: output.length_cache.iter().map(|row| row.to_vec()).collect(),
        angle: output.angle_cache.iter().map(|row| row.to_vec()).collect(),
        extr: [
            output.extreme_angles_degrees.0,
            output.extreme_angles_degrees.1,
        ],
        eavg: [
            output.average_min_max_angles_degrees.0,
            output.average_min_max_angles_degrees.1,
        ],
        savg: output.angle_stddev_degrees,
        less: bool_flags_to_i32(&output.angle_less_flags),
        more: bool_flags_to_i32(&output.angle_more_flags),
    }
}

fn quality_class_from_polygon_quality(
    output: &earthmesh_mesh::PolygonMeshQualityFortranOutput,
) -> QualityClassMetrics {
    QualityClassMetrics {
        length: output.length_cache.clone(),
        angle: output.angle_cache.clone(),
        extr: [
            output.extreme_angles_degrees.0,
            output.extreme_angles_degrees.1,
        ],
        eavg: [
            output.average_min_max_angles_degrees.0,
            output.average_min_max_angles_degrees.1,
        ],
        savg: output.angle_stddev_degrees,
        less: bool_flags_to_i32(&output.angle_less_flags),
        more: bool_flags_to_i32(&output.angle_more_flags),
    }
}

fn empty_quality_class(_width: usize) -> QualityClassMetrics {
    QualityClassMetrics {
        length: Vec::new(),
        angle: Vec::new(),
        extr: [0.0, 0.0],
        eavg: [0.0, 0.0],
        savg: 0.0,
        less: Vec::new(),
        more: Vec::new(),
    }
}

fn bool_flags_to_i32(flags: &[bool]) -> Vec<i32> {
    flags.iter().map(|flag| i32::from(*flag)).collect()
}

fn lonlat_degrees_to_lonlat_point(point: LonLatDegrees) -> LonLatPoint {
    LonLatPoint {
        lon: point.lon_degrees,
        lat: point.lat_degrees,
    }
}

fn unstructured_mesh_from_springjustment_global(
    source: &UnstructuredMesh,
    output: &SpringjustmentGlobalCoreOutput,
) -> io::Result<UnstructuredMesh> {
    require_len(
        "Springjustment_global updated_triangle_lonlat",
        output.updated_triangle_lonlat.len(),
        source.m_points.len(),
    )?;
    require_len(
        "Springjustment_global updated_cell_lonlat",
        output.updated_cell_lonlat.len(),
        source.w_points.len(),
    )?;

    Ok(UnstructuredMesh {
        m_points: output
            .updated_triangle_lonlat
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        w_points: output
            .updated_cell_lonlat
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        m_to_w: source.m_to_w.clone(),
        w_to_m: source.w_to_m.clone(),
        n_w_to_m: source.n_w_to_m.clone(),
    })
}

fn unstructured_mesh_from_springjustment_regional(
    source: &UnstructuredMesh,
    output: &SpringjustmentRegionalCoreOutput,
) -> io::Result<UnstructuredMesh> {
    require_len(
        "Springjustment_regional_step updated_triangle_lonlat",
        output.updated_triangle_lonlat.len(),
        source.m_points.len(),
    )?;
    require_len(
        "Springjustment_regional_step updated_cell_lonlat",
        output.updated_cell_lonlat.len(),
        source.w_points.len(),
    )?;

    Ok(UnstructuredMesh {
        m_points: output
            .updated_triangle_lonlat
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        w_points: output
            .updated_cell_lonlat
            .iter()
            .copied()
            .map(lonlat_degrees_to_lonlat_point)
            .collect(),
        m_to_w: source.m_to_w.clone(),
        w_to_m: source.w_to_m.clone(),
        n_w_to_m: source.n_w_to_m.clone(),
    })
}

fn cells_on_triangle_fortran_indexed_from_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<Vec<[usize; 3]>> {
    mesh.m_to_w
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            let mut out = [0usize; 3];
            for (slot, value) in row.iter().copied().enumerate() {
                if value < 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("m_to_w row {row_idx} slot {slot} has negative cell id {value}"),
                    ));
                }
                let value = value as usize;
                if value >= mesh.w_points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "m_to_w row {row_idx} references cell id {value}, but only {} cell rows exist",
                            mesh.w_points.len()
                        ),
                    ));
                }
                out[slot] = value;
            }
            Ok(out)
        })
        .collect()
}

fn triangles_on_cell_fortran_indexed_from_mesh(
    mesh: &UnstructuredMesh,
) -> io::Result<Vec<Vec<usize>>> {
    let mut rows = Vec::with_capacity(mesh.w_to_m.len());
    for (row_idx, row) in mesh.w_to_m.iter().enumerate() {
        let mut out = Vec::with_capacity(row.len());
        for (slot, value) in row.iter().copied().enumerate() {
            if value < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("w_to_m row {row_idx} slot {slot} has negative triangle id {value}"),
                ));
            }
            let value = value as usize;
            if value >= mesh.m_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "w_to_m row {row_idx} references triangle id {value}, but only {} triangle rows exist",
                        mesh.m_points.len()
                    ),
                ));
            }
            out.push(value);
        }
        rows.push(out);
    }
    Ok(rows)
}

fn n_edges_on_cell_usize_from_mesh(mesh: &UnstructuredMesh) -> io::Result<Vec<usize>> {
    mesh.n_w_to_m
        .iter()
        .enumerate()
        .map(|(idx, value)| {
            if *value < 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("n_w_to_m row {idx} has negative edge count {value}"),
                ));
            }
            Ok(*value as usize)
        })
        .collect()
}

fn mpas_lat_lon_radians(points: &[LonLatDegrees]) -> (Vec<f64>, Vec<f64>) {
    let mut lat = Vec::with_capacity(points.len());
    let mut lon = Vec::with_capacity(points.len());
    for point in points {
        lat.push(earthmesh_core::deg_to_rad(point.lat_degrees));
        let mut lon_degrees = point.lon_degrees;
        if lon_degrees < 0.0 {
            lon_degrees += 360.0;
        }
        lon.push(earthmesh_core::deg_to_rad(lon_degrees));
    }
    (lat, lon)
}

fn zero_based_id(name: &str, value: usize) -> io::Result<i32> {
    if value == 0 {
        return Ok(0);
    }
    i32::try_from(value - 1).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} contains value {value} that does not fit NetCDF INT"),
        )
    })
}

fn zero_based_padded_rows(
    name: &str,
    rows: &[Vec<usize>],
    width: usize,
) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .enumerate()
        .map(|(row_idx, row)| {
            if row.len() > width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} row {row_idx} width {} exceeds {width}", row.len()),
                ));
            }
            let mut output = row
                .iter()
                .copied()
                .map(|value| zero_based_id(name, value))
                .collect::<io::Result<Vec<_>>>()?;
            output.resize(width, 0);
            Ok(output)
        })
        .collect()
}

fn zero_based_triplet_rows(name: &str, rows: &[[usize; 3]]) -> io::Result<Vec<Vec<i32>>> {
    rows.iter()
        .map(|row| {
            row.iter()
                .copied()
                .map(|value| zero_based_id(name, value))
                .collect()
        })
        .collect()
}

fn zero_based_pair_rows(name: &str, rows: &[[usize; 2]]) -> io::Result<Vec<[i32; 2]>> {
    rows.iter()
        .map(|row| Ok([zero_based_id(name, row[0])?, zero_based_id(name, row[1])?]))
        .collect()
}

fn pad_f64_rows(rows: &[Vec<f64>], width: usize) -> Vec<Vec<f64>> {
    rows.iter()
        .map(|row| {
            let mut output = row.clone();
            output.resize(width, 0.0);
            output
        })
        .collect()
}

fn validate_mpas_mesh(mesh: &MpasMesh) -> io::Result<()> {
    if mesh.x_cell.is_empty() || mesh.x_vertex.is_empty() || mesh.x_edge.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS mesh arrays must include the legacy placeholder row",
        ));
    }
    let n_cells = mesh.x_cell.len();
    let n_vertices = mesh.x_vertex.len();
    let n_edges = mesh.x_edge.len();
    for (name, actual, required) in [
        ("lat_cell", mesh.lat_cell.len(), n_cells),
        ("lon_cell", mesh.lon_cell.len(), n_cells),
        ("y_cell", mesh.y_cell.len(), n_cells),
        ("z_cell", mesh.z_cell.len(), n_cells),
        ("n_edges_on_cell", mesh.n_edges_on_cell.len(), n_cells),
        ("cells_on_cell", mesh.cells_on_cell.len(), n_cells),
        ("vertices_on_cell", mesh.vertices_on_cell.len(), n_cells),
        ("edges_on_cell", mesh.edges_on_cell.len(), n_cells),
        ("area_cell", mesh.area_cell.len(), n_cells),
        ("mesh_density", mesh.mesh_density.len(), n_cells),
        ("lat_vertex", mesh.lat_vertex.len(), n_vertices),
        ("lon_vertex", mesh.lon_vertex.len(), n_vertices),
        ("y_vertex", mesh.y_vertex.len(), n_vertices),
        ("z_vertex", mesh.z_vertex.len(), n_vertices),
        ("cells_on_vertex", mesh.cells_on_vertex.len(), n_vertices),
        ("edges_on_vertex", mesh.edges_on_vertex.len(), n_vertices),
        ("area_triangle", mesh.area_triangle.len(), n_vertices),
        (
            "kite_areas_on_vertex",
            mesh.kite_areas_on_vertex.len(),
            n_vertices,
        ),
        ("lat_edge", mesh.lat_edge.len(), n_edges),
        ("lon_edge", mesh.lon_edge.len(), n_edges),
        ("y_edge", mesh.y_edge.len(), n_edges),
        ("z_edge", mesh.z_edge.len(), n_edges),
        ("cells_on_edge", mesh.cells_on_edge.len(), n_edges),
        ("vertices_on_edge", mesh.vertices_on_edge.len(), n_edges),
        ("n_edges_on_edge", mesh.n_edges_on_edge.len(), n_edges),
        ("edges_on_edge", mesh.edges_on_edge.len(), n_edges),
        ("dv_edge", mesh.dv_edge.len(), n_edges),
        ("dc_edge", mesh.dc_edge.len(), n_edges),
        ("angle_edge", mesh.angle_edge.len(), n_edges),
        ("weights_on_edge", mesh.weights_on_edge.len(), n_edges),
        ("error_segment", mesh.error_segment.len(), n_edges),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    for (name, rows, width) in [
        ("cells_on_cell", &mesh.cells_on_cell, 10_usize),
        ("vertices_on_cell", &mesh.vertices_on_cell, 10_usize),
        ("edges_on_cell", &mesh.edges_on_cell, 10_usize),
        ("cells_on_vertex", &mesh.cells_on_vertex, 3_usize),
        ("edges_on_vertex", &mesh.edges_on_vertex, 3_usize),
        ("edges_on_edge", &mesh.edges_on_edge, 20_usize),
    ] {
        let actual = matrix_width(name, rows)?;
        if actual != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} width {actual} must match required {width}"),
            ));
        }
    }
    let kite_width = f64_matrix_width("kite_areas_on_vertex", &mesh.kite_areas_on_vertex)?;
    if kite_width != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("kite_areas_on_vertex width {kite_width} must match required 3"),
        ));
    }
    let weights_width = f64_matrix_width("weights_on_edge", &mesh.weights_on_edge)?;
    if weights_width != 20 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("weights_on_edge width {weights_width} must match required 20"),
        ));
    }
    Ok(())
}

fn validate_mpas_simple_mesh(mesh: &MpasSimpleMesh) -> io::Result<()> {
    if mesh.x_cell.is_empty() || mesh.x_vertex.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MPAS simple mesh arrays must include the legacy placeholder row",
        ));
    }
    for (name, actual, required) in [
        ("y_cell", mesh.y_cell.len(), mesh.x_cell.len()),
        ("z_cell", mesh.z_cell.len(), mesh.x_cell.len()),
        ("mesh_density", mesh.mesh_density.len(), mesh.x_cell.len()),
        ("y_vertex", mesh.y_vertex.len(), mesh.x_vertex.len()),
        ("z_vertex", mesh.z_vertex.len(), mesh.x_vertex.len()),
        (
            "cells_on_vertex",
            mesh.cells_on_vertex.len(),
            mesh.x_vertex.len(),
        ),
    ] {
        if actual != required {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{name} length {actual} must match required {required}"),
            ));
        }
    }
    let width = matrix_width("cells_on_vertex", &mesh.cells_on_vertex)?;
    if width != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("cells_on_vertex width {width} must match vertexDegree 3"),
        ));
    }
    Ok(())
}

fn f64_matrix_width(name: &str, rows: &[Vec<f64>]) -> io::Result<usize> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != width) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} rows must have uniform width"),
        ));
    }
    Ok(width)
}

fn matrix_width(name: &str, rows: &[Vec<i32>]) -> io::Result<usize> {
    let width = rows.first().map(Vec::len).unwrap_or(0);
    if rows.iter().any(|row| row.len() != width) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} rows must have uniform width"),
        ));
    }
    Ok(width)
}

fn flatten_i32_rows(rows: &[Vec<i32>]) -> Vec<i32> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

fn flatten_i32_pairs(rows: &[[i32; 2]]) -> Vec<i32> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

fn flatten_f64_rows(rows: &[Vec<f64>]) -> Vec<f64> {
    rows.iter().flat_map(|row| row.iter().copied()).collect()
}

fn write_i32_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[i32],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

fn write_i32_matrix_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<i32>],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_i32_rows(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

fn write_i32_pair_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[[i32; 2]],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_i32_pairs(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

fn write_f64_matrix_rows(
    file: &mut netcdf::FileMut,
    name: &str,
    dims: &[&str],
    rows: &[Vec<f64>],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, dims)
        .map_err(netcdf_to_io_error)?;
    var.put_values(&flatten_f64_rows(rows), (.., ..))
        .map_err(netcdf_to_io_error)
}

fn one_to_n_i32(n: usize, name: &str) -> io::Result<Vec<i32>> {
    (1..=n).map(|value| usize_to_i32(name, value)).collect()
}

fn write_f64_1d(
    file: &mut netcdf::FileMut,
    name: &str,
    dim: &str,
    values: &[f64],
) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[dim])
        .map_err(netcdf_to_io_error)?;
    var.put_values(values, ..).map_err(netcdf_to_io_error)
}

fn optional_values_i32_2d(file: &netcdf::File, name: &str) -> io::Result<Option<Vec<i32>>> {
    let Some(variable) = file.variable(name) else {
        return Ok(None);
    };
    variable
        .get_values::<i32, _>((.., ..))
        .map(Some)
        .map_err(netcdf_to_io_error)
}

fn required_scalar_i32(file: &netcdf::File, name: &str) -> io::Result<i32> {
    let values = required_values_i32(file, name)?;
    match values.as_slice() {
        [value] => Ok(*value),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} scalar must contain exactly one value, found {}",
                values.len()
            ),
        )),
    }
}

fn required_scalar_usize_i32(file: &netcdf::File, name: &str) -> io::Result<usize> {
    let value = required_scalar_i32(file, name)?;
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} value {value} must be non-negative"),
        )
    })
}

fn i32_matrix_from_flat(
    name: &str,
    values: Vec<i32>,
    rows: usize,
    width: usize,
) -> io::Result<Vec<Vec<i32>>> {
    let expected = rows.checked_mul(width).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} dimensions {rows}x{width} overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{name} contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    Ok(rows_from_flat_i32(&values, width))
}

fn write_i32_scalar(file: &mut netcdf::FileMut, name: &str, value: i32) -> io::Result<()> {
    let mut var = file
        .add_variable::<i32>(name, &[])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&[value], ..).map_err(netcdf_to_io_error)
}

fn write_f64_scalar(file: &mut netcdf::FileMut, name: &str, value: f64) -> io::Result<()> {
    let mut var = file
        .add_variable::<f64>(name, &[])
        .map_err(netcdf_to_io_error)?;
    var.put_values(&[value], ..).map_err(netcdf_to_io_error)
}

fn write_quality_class(
    file: &mut netcdf::FileMut,
    suffix: &str,
    row_dim: &str,
    width_dim: &str,
    metrics: &QualityClassMetrics,
) -> io::Result<()> {
    write_f64_matrix_rows(
        file,
        &format!("length_{suffix}"),
        &[row_dim, width_dim],
        &metrics.length,
    )?;
    write_f64_matrix_rows(
        file,
        &format!("angle_{suffix}"),
        &[row_dim, width_dim],
        &metrics.angle,
    )?;
    write_f64_1d(file, &format!("Extr_{suffix}"), "two", &metrics.extr)?;
    write_f64_1d(file, &format!("Eavg_{suffix}"), "two", &metrics.eavg)?;
    write_f64_scalar(file, &format!("Savg_{suffix}"), metrics.savg)?;
    write_i32_1d(file, &format!("less_{suffix}"), row_dim, &metrics.less)?;
    write_i32_1d(file, &format!("more_{suffix}"), row_dim, &metrics.more)?;
    Ok(())
}

fn rows_from_flat_i32(values: &[i32], width: usize) -> Vec<Vec<i32>> {
    if width == 0 {
        return Vec::new();
    }
    values.chunks_exact(width).map(|row| row.to_vec()).collect()
}

fn unstructured_dimc(mesh: &UnstructuredMesh) -> usize {
    mesh.n_w_to_m
        .iter()
        .filter_map(|&value| usize::try_from(value).ok())
        .chain(mesh.w_to_m.iter().map(Vec::len))
        .max()
        .unwrap_or(0)
        .max(7)
}

fn lon_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lon).collect()
}

fn lat_values(points: &[LonLatPoint]) -> Vec<f64> {
    points.iter().map(|point| point.lat).collect()
}

fn flatten_m_to_w(m_to_w: &[[i32; 3]]) -> Vec<i32> {
    let mut values = Vec::with_capacity(m_to_w.len() * 3);
    for row in m_to_w {
        values.extend_from_slice(row);
    }
    values
}

fn flatten_w_to_m(w_to_m: &[Vec<i32>], dimc: usize) -> Vec<i32> {
    let mut values = Vec::with_capacity(w_to_m.len() * dimc);
    for row in w_to_m {
        values.extend(row.iter().copied().take(dimc));
        values.resize(values.len() + dimc.saturating_sub(row.len().min(dimc)), 0);
    }
    values
}

fn trim_trailing_zero_connectivity(row: &[i32]) -> Vec<i32> {
    let end = row
        .iter()
        .rposition(|&value| value != 0)
        .map(|idx| idx + 1)
        .unwrap_or(row.len());
    row[..end].to_vec()
}

fn usize_values_to_i32(name: &str, values: &[usize]) -> io::Result<Vec<i32>> {
    values
        .iter()
        .map(|&value| {
            i32::try_from(value).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{name} contains value {value} that does not fit NetCDF INT"),
                )
            })
        })
        .collect()
}

fn flatten_close_curves_for_netcdf(
    close_curves: &[Vec<usize>],
    longest_curve_slots: usize,
    closed_curves: usize,
) -> io::Result<Vec<i32>> {
    let mut values = Vec::with_capacity(longest_curve_slots * closed_curves);
    for curve_id in 1..=closed_curves {
        let curve = &close_curves[curve_id];
        if curve.len() > longest_curve_slots {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "close_curve {curve_id} length {} exceeds num1 {longest_curve_slots}",
                    curve.len()
                ),
            ));
        }
        values.extend(usize_values_to_i32("close_curve", curve)?);
        values.resize(values.len() + longest_curve_slots - curve.len(), 1);
    }
    Ok(values)
}

/// Report for the migrated NetCDF branch of `mkgrd.F90:mode4mesh_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mode4MeshMakeReport {
    pub input: PathBuf,
    pub grid_select: String,
    pub output: PathBuf,
    pub bound_points: usize,
    pub mode_points: usize,
}

/// Execute the NetCDF-supported branch of `mode4mesh_make`.
///
/// This currently ports the active Lambert `.nc/.nc4` path. The legacy Fortran
/// lonlat `.nc` path and Lambert `.nml` path stop immediately, so they are
/// represented as `InvalidInput` until deliberately enabled with tests.
pub fn mode4mesh_make_netcdf(
    inputfile: impl AsRef<Path>,
    grid_select: &str,
    output: impl AsRef<Path>,
) -> io::Result<Mode4MeshMakeReport> {
    let inputfile = inputfile.as_ref();
    let output = output.as_ref();
    let extension = source_extension(inputfile);
    let grid_select_trimmed = grid_select.trim();

    match grid_select_trimmed {
        "lambert" => {
            if !matches!(extension.as_deref(), Some("nc") | Some("nc4")) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "lambert mode4mesh_make currently requires .nc or .nc4 input",
                ));
            }
            let vertices = read_lambert_vertices_netcdf(inputfile)?;
            let mesh = lambert_vertices_to_mode4_mesh(&vertices)?;
            write_mode4_mesh_netcdf(output, &mesh)?;
            Ok(Mode4MeshMakeReport {
                input: inputfile.to_path_buf(),
                grid_select: grid_select_trimmed.to_string(),
                output: output.to_path_buf(),
                bound_points: mesh.bound_points(),
                mode_points: mesh.mode_points(),
            })
        }
        "lonlat" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lonlat mode4mesh_make is not enabled by this NetCDF adapter",
        )),
        "cubical" => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cubical mode4mesh_make is not supported",
        )),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported grid_select {other}"),
        )),
    }
}

/// Combined report for workspace setup followed by planned mask operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceMaskApplyReport {
    pub workspace: WorkspaceApplyReport,
    pub mask_reports: Vec<MaskOperationReport>,
    pub mask_counts: MaskCountState,
}

/// Apply the Rust `read_nl` workspace plan and then execute every planned
/// `Mask_make` operation in order.
pub fn apply_workspace_and_mask_operations(
    plan: &MkgrdWorkspacePlan,
    namelist_source: &Path,
    workdir: &Path,
    max_iter_spc: usize,
    validate_refine_max_iter: bool,
) -> io::Result<WorkspaceMaskApplyReport> {
    let workspace = apply_read_nl_workspace_plan(plan, namelist_source, workdir)?;
    let mut mask_counts = MaskCountState::default();
    let mut mask_reports = Vec::with_capacity(plan.mask_operations.len());

    for operation in &plan.mask_operations {
        let report =
            apply_mask_operation(operation, &plan.file_dir, max_iter_spc, &mut mask_counts)?;
        mask_reports.push(report);
    }

    if validate_refine_max_iter {
        validate_mask_refine_reaches_max_iter_spc(&mask_counts, max_iter_spc)?;
    }

    Ok(WorkspaceMaskApplyReport {
        workspace,
        mask_reports,
        mask_counts,
    })
}

/// Evidence report from applying the filesystem subset of `mkgrd.F90:read_nl`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceApplyReport {
    pub removed_file_dir: Option<PathBuf>,
    pub removed_filelists: Vec<PathBuf>,
    pub created_directories: Vec<PathBuf>,
    pub copied_namelist_to: Option<PathBuf>,
}

/// Read `bbox_refine` from a bbox NetCDF source used by `bbox_mask_make`.
pub fn read_bbox_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let variable = file.variable("bbox_refine").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox NetCDF input is missing bbox_refine",
        )
    })?;
    let refine = variable
        .get_value::<i32, _>(())
        .map_err(netcdf_to_io_error)?;
    if refine < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bbox_refine must be non-negative",
        ));
    }
    Ok(refine as usize)
}

/// Read bbox mask points from the NetCDF schema produced by `bbox_mask_make`.
pub fn read_bbox_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<BBoxMask> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let bbox_num = required_dimension_len(&file, "bbox_num")?;
    let four = required_dimension_len(&file, "four")?;
    if four != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox four dimension {four} must equal 4"),
        ));
    }
    let refine_degree = read_bbox_refine_netcdf(inputfile)?;
    let values = required_values_f64(&file, "bbox_points")?;
    let expected = bbox_num.checked_mul(4).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bbox_points dimensions {bbox_num}x4 overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "bbox_points contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    let points = values
        .chunks_exact(4)
        .map(|row| BBoxPoint {
            west: row[0],
            east: row[1],
            north: row[2],
            south: row[3],
        })
        .collect::<Vec<_>>();
    let mask = BBoxMask {
        refine_degree,
        points,
    };
    validate_bbox_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid bbox mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

/// Write bbox mask points to a NetCDF file using the bbox schema consumed by
/// EarthMesh mask preprocessing.
pub fn write_bbox_mask_netcdf(output: impl AsRef<Path>, mask: &BBoxMask) -> io::Result<()> {
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bbox_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("four", 4).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("bbox_refine", &[])
            .map_err(netcdf_to_io_error)?;
        refine
            .put_value(mask.refine_degree as i32, ())
            .map_err(netcdf_to_io_error)?;
    }
    let mut values = Vec::with_capacity(mask.points.len() * 4);
    for point in &mask.points {
        values.extend([point.west, point.east, point.north, point.south]);
    }
    {
        let mut bbox_points = file
            .add_variable::<f64>("bbox_points", &["bbox_num", "four"])
            .map_err(netcdf_to_io_error)?;
        bbox_points
            .put_values(&values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

fn netcdf_to_io_error(err: netcdf::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, err.to_string())
}

/// Copy a circle NetCDF source into the Fortran tmpfile naming scheme.
pub fn copy_circle_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_circle_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

/// Copy a close NetCDF source into the Fortran tmpfile naming scheme.
pub fn copy_close_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_close_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

fn copy_mask_netcdf_with_output<F>(
    inputfile: impl AsRef<Path>,
    refine_degree: usize,
    max_iter_spc: usize,
    output_fn: F,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>>
where
    F: FnOnce(&mut MaskCountState, usize, &Path) -> io::Result<PathBuf>,
{
    let inputfile = inputfile.as_ref();
    let extension = inputfile.extension().and_then(|value| value.to_str());
    if !matches!(extension, Some("nc") | Some("nc4")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask NetCDF input must end with .nc or .nc4",
        ));
    }
    if refine_degree > max_iter_spc {
        return Ok(None);
    }
    let output = output_fn(counts, refine_degree, file_dir.as_ref())?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(inputfile, &output)?;
    Ok(Some(output))
}

/// Copy a bbox NetCDF source into the Fortran tmpfile naming scheme.
///
/// This covers the `bbox_mask_make` `.nc/.nc4` branch after the caller has
/// obtained `bbox_refine` from the NetCDF metadata. If `refine_degree` is above
/// `max_iter_spc`, the function returns `Ok(None)` and leaves counters/files
/// untouched, matching the Fortran early return.
pub fn copy_bbox_mask_netcdf_with_refine(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    refine_degree: usize,
    max_iter_spc: usize,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    copy_mask_netcdf_with_output(
        inputfile,
        refine_degree,
        max_iter_spc,
        |counts, refine_degree, file_dir| {
            counts.next_bbox_output(mask_select, refine_degree, file_dir)
        },
        file_dir,
        counts,
    )
}

/// Apply the non-mask filesystem side effects described by a Rust read_nl plan.
///
/// This intentionally does not execute `Mask_make`; callers can inspect
/// `plan.mask_operations` and run the appropriate mask adapter after the safe
/// workspace setup has completed.
pub fn apply_read_nl_workspace_plan(
    plan: &MkgrdWorkspacePlan,
    namelist_source: &Path,
    workdir: &Path,
) -> io::Result<WorkspaceApplyReport> {
    let mut report = WorkspaceApplyReport::default();
    let file_dir = PathBuf::from(&plan.file_dir);

    if plan.remove_existing_file_dir && file_dir.exists() {
        fs::remove_dir_all(&file_dir)?;
        report.removed_file_dir = Some(file_dir.clone());
    }

    if plan.remove_filelists && workdir.exists() {
        for entry in fs::read_dir(workdir)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };
            if name.ends_with("_filelist.txt") {
                fs::remove_file(&path)?;
                report.removed_filelists.push(path);
            }
        }
        report.removed_filelists.sort();
    }

    for directory in &plan.directories_to_create {
        let path = PathBuf::from(directory);
        fs::create_dir_all(&path)?;
        report.created_directories.push(path);
    }

    let namelist_save_path = PathBuf::from(&plan.namelist_save_path);
    if let Some(parent) = namelist_save_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(namelist_source, &namelist_save_path)?;
    report.copied_namelist_to = Some(namelist_save_path);

    Ok(report)
}

/// Result of applying one Rust `Mask_make` operation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaskOperationReport {
    pub sources: Vec<PathBuf>,
    pub outputs: Vec<PathBuf>,
}

/// Apply one `mkgrd.F90:Mask_make(mask_select, type_select, mask_fprefix)` call.
pub fn apply_mask_operation(
    operation: &MaskOperation,
    file_dir: impl AsRef<Path>,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<MaskOperationReport> {
    let discovery = discover_mask_sources(&operation.mask_fprefix)?;
    if discovery.files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no mask sources matched mask_fprefix",
        ));
    }

    let file_dir = file_dir.as_ref();
    let mut report = MaskOperationReport {
        sources: discovery.files.clone(),
        outputs: Vec::new(),
    };

    for source in discovery.files {
        let output = match operation.type_select.as_str() {
            "bbox" => apply_bbox_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "circle" => apply_circle_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "close" => apply_close_source(
                &source,
                &operation.mask_select,
                file_dir,
                max_iter_spc,
                counts,
            )?,
            "lambert" => Some(convert_lambert_mask_netcdf(
                &source,
                &operation.mask_select,
                file_dir,
                counts,
            )?),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported type_select {other}"),
                ));
            }
        };
        if let Some(output) = output {
            report.outputs.push(output);
        }
    }

    Ok(report)
}

/// Preserve the `read_nl` specified-refinement guard.
pub fn validate_mask_refine_reaches_max_iter_spc(
    counts: &MaskCountState,
    max_iter_spc: usize,
) -> io::Result<()> {
    if max_iter_spc >= counts.mask_refine_ndm.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max_iter_spc must fit mask_refine_ndm 0:9",
        ));
    }
    if counts.mask_refine_ndm[max_iter_spc] == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mask_refine_ndm(max_iter_spc) must be larger than zero",
        ));
    }
    Ok(())
}

fn apply_bbox_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_bbox_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_bbox_output(mask_select, mask.refine_degree, file_dir)?;
            write_bbox_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_bbox_refine_netcdf(source)?;
            copy_bbox_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}

fn apply_circle_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_circle_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_circle_output(mask_select, mask.refine_degree, file_dir)?;
            write_circle_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_circle_refine_netcdf(source)?;
            copy_circle_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}

fn apply_close_source(
    source: &Path,
    mask_select: &str,
    file_dir: &Path,
    max_iter_spc: usize,
    counts: &mut MaskCountState,
) -> io::Result<Option<PathBuf>> {
    match source_extension(source).as_deref() {
        Some("nml") => {
            let Some(mask) = parse_close_mask_nml(source, max_iter_spc)? else {
                return Ok(None);
            };
            let output = counts.next_close_output(mask_select, mask.refine_degree, file_dir)?;
            write_close_mask_netcdf(&output, &mask)?;
            Ok(Some(output))
        }
        Some("nc") | Some("nc4") => {
            let refine = read_close_refine_netcdf(source)?;
            copy_close_mask_netcdf_with_refine(
                source,
                mask_select,
                refine,
                max_iter_spc,
                file_dir,
                counts,
            )
        }
        _ => Err(unsupported_mask_source(source)),
    }
}

fn source_extension(source: &Path) -> Option<String> {
    source
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn unsupported_mask_source(source: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unsupported mask source extension for {}", source.display()),
    )
}

/// Prefix discovery result for the first shell-listing step in `Mask_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskSourceDiscovery {
    pub directory: PathBuf,
    pub file_prefix: String,
    pub files: Vec<PathBuf>,
}

/// Discover source mask files matching the Fortran `ls mask_fprefix*` behavior.
///
/// The Fortran routine first splits `mask_fprefix` at the last `/`, then lists
/// every file whose full path starts with that prefix. This Rust adapter keeps
/// the same prefix semantics while avoiding shell execution.
pub fn discover_mask_sources(mask_fprefix: impl AsRef<Path>) -> io::Result<MaskSourceDiscovery> {
    let mask_fprefix = mask_fprefix.as_ref();
    let Some(directory) = mask_fprefix
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_fprefix must include a parent directory like mkgrd.F90:Mask_make",
        ));
    };
    let Some(file_prefix) = mask_fprefix.file_name().and_then(|value| value.to_str()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mask_fprefix must include a file prefix",
        ));
    };

    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with(file_prefix) {
            files.push(path);
        }
    }
    files.sort();

    Ok(MaskSourceDiscovery {
        directory: directory.to_path_buf(),
        file_prefix: file_prefix.to_string(),
        files,
    })
}

/// One `bbox_points(i, :)` row: West, East, North, South.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BBoxPoint {
    pub west: f64,
    pub east: f64,
    pub north: f64,
    pub south: f64,
}

/// Parsed `.nml` input for `bbox_mask_make`.
#[derive(Debug, Clone, PartialEq)]
pub struct BBoxMask {
    pub refine_degree: usize,
    pub points: Vec<BBoxPoint>,
}

/// Parse the text `.nml` branch of `mkgrd.F90:bbox_mask_make`.
///
/// Returns `Ok(None)` when `refine_degree > max_iter_spc`, matching the Fortran
/// early return before any output/count update.
pub fn parse_bbox_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<BBoxMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let bbox_num_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing bbox_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing bbox_refine line"))?;
    let bbox_num = parse_value_after_equals::<usize>(bbox_num_line, "bbox_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "bbox_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(bbox_num);
    for index in 0..bbox_num {
        let line = lines.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing bbox point row {}", index + 1),
            )
        })?;
        let values = line
            .split_whitespace()
            .map(|value| {
                value.parse::<f64>().map_err(|err| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("invalid bbox coordinate {value}: {err}"),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if values.len() != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point row {} must contain 4 values", index + 1),
            ));
        }
        let point = BBoxPoint {
            west: values[0],
            east: values[1],
            north: values[2],
            south: values[3],
        };
        if point.west > point.east {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bbox west must be <= east like bbox_mask_make",
            ));
        }
        if point.north < point.south {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bbox north must be >= south like bbox_mask_make",
            ));
        }
        points.push(point);
    }

    Ok(Some(BBoxMask {
        refine_degree,
        points,
    }))
}

fn validate_close_mask(mask: &CloseMask) -> io::Result<()> {
    if mask.points.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "close mask must contain at least three points",
        ));
    }
    for (index, point) in mask.points.iter().enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("close point {} coordinates must be finite", index + 1),
            ));
        }
    }
    Ok(())
}

fn validate_mode4_mesh_for_area_judge(mesh: &Mode4Mesh) -> io::Result<()> {
    if mesh.bound_points() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 mesh must include a placeholder plus at least one boundary point",
        ));
    }
    if mesh.mode_points() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 mesh must include a placeholder plus at least one cell",
        ));
    }
    if mesh.ngr_bound.len() != mesh.n_ngr.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "mode4 ngr_bound and n_ngr lengths must match",
        ));
    }
    for (index, point) in mesh.lonlat_bound.iter().enumerate() {
        if index == 0 {
            continue;
        }
        if !point.lon.is_finite() || !point.lat.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mode4 boundary point {} coordinates must be finite",
                    index + 1
                ),
            ));
        }
    }
    for cell_index in 1..mesh.mode_points() {
        for &bound_index in &mesh.ngr_bound[cell_index] {
            if bound_index < 1 || bound_index as usize > mesh.bound_points() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("mode4 cell {cell_index} references out-of-range vertex {bound_index}"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_circle_mask(mask: &CircleMask) -> io::Result<()> {
    if mask.points.len() != mask.radius_km.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "circle points and radius arrays must have the same length",
        ));
    }
    for (index, (point, radius)) in mask.points.iter().zip(mask.radius_km.iter()).enumerate() {
        if !point.lon.is_finite() || !point.lat.is_finite() || !radius.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "circle point {} coordinates and radius must be finite",
                    index + 1
                ),
            ));
        }
        if *radius < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("circle point {} radius must be non-negative", index + 1),
            ));
        }
    }
    Ok(())
}

fn validate_bbox_mask(mask: &BBoxMask) -> io::Result<()> {
    for (index, point) in mask.points.iter().enumerate() {
        if point.west > point.east {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point {} west must be <= east", index + 1),
            ));
        }
        if point.north < point.south {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point {} north must be >= south", index + 1),
            ));
        }
    }
    Ok(())
}

fn parse_value_after_equals<T>(line: &str, field: &str) -> io::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let (_, value) = line.split_once('=').ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{field} line must contain '='"),
        )
    })?;
    value.trim().parse::<T>().map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid {field} value: {err}"),
        )
    })
}

/// Shared longitude/latitude row used by circle and close masks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatPoint {
    pub lon: f64,
    pub lat: f64,
}

/// Parsed circle mask input: lon, lat, and radius in kilometers.
#[derive(Debug, Clone, PartialEq)]
pub struct CircleMask {
    pub refine_degree: usize,
    pub points: Vec<LonLatPoint>,
    pub radius_km: Vec<f64>,
}

/// Parsed close-polygon mask input.
#[derive(Debug, Clone, PartialEq)]
pub struct CloseMask {
    pub refine_degree: usize,
    pub points: Vec<LonLatPoint>,
}

/// Parse the text `.nml` branch of `mkgrd.F90:circle_mask_make`.
pub fn parse_circle_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<CircleMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let count_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing circle_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing circle_refine line"))?;
    let circle_num = parse_value_after_equals::<usize>(count_line, "circle_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "circle_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(circle_num);
    let mut radius_km = Vec::with_capacity(circle_num);
    for index in 0..circle_num {
        let values = parse_float_row(
            lines.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing circle point row {}", index + 1),
                )
            })?,
            3,
            "circle point",
            index + 1,
        )?;
        points.push(LonLatPoint {
            lon: values[0],
            lat: values[1],
        });
        radius_km.push(values[2]);
    }
    Ok(Some(CircleMask {
        refine_degree,
        points,
        radius_km,
    }))
}

/// Parse the text `.nml` branch of `mkgrd.F90:close_mask_make`.
pub fn parse_close_mask_nml(
    inputfile: impl AsRef<Path>,
    max_iter_spc: usize,
) -> io::Result<Option<CloseMask>> {
    let content = fs::read_to_string(inputfile)?;
    let mut lines = content.lines();
    let count_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing close_num line"))?;
    let refine_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing close_refine line"))?;
    let close_num = parse_value_after_equals::<usize>(count_line, "close_num")?;
    let refine_degree = parse_value_after_equals::<usize>(refine_line, "close_refine")?;
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let mut points = Vec::with_capacity(close_num);
    for index in 0..close_num {
        let values = parse_float_row(
            lines.next().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing close point row {}", index + 1),
                )
            })?,
            2,
            "close point",
            index + 1,
        )?;
        points.push(LonLatPoint {
            lon: values[0],
            lat: values[1],
        });
    }
    Ok(Some(CloseMask {
        refine_degree,
        points,
    }))
}

pub fn read_circle_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    read_nonnegative_refine_netcdf(inputfile, "circle_refine")
}

pub fn read_circle_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<CircleMask> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let circle_num = required_dimension_len(&file, "circle_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle two dimension {two} must equal 2"),
        ));
    }
    let refine_degree = read_circle_refine_netcdf(inputfile)?;
    let point_values = required_values_f64(&file, "circle_points")?;
    let expected_points = circle_num.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("circle_points dimensions {circle_num}x2 overflow"),
        )
    })?;
    if point_values.len() != expected_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_points contains {} values, expected {expected_points}",
                point_values.len()
            ),
        ));
    }
    let radius_km = required_values_f64(&file, "circle_radius")?;
    if radius_km.len() != circle_num {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "circle_radius contains {} values, expected {circle_num}",
                radius_km.len()
            ),
        ));
    }
    let points = point_values
        .chunks_exact(2)
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();
    let mask = CircleMask {
        refine_degree,
        points,
        radius_km,
    };
    validate_circle_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid circle mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

pub fn read_close_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    read_nonnegative_refine_netcdf(inputfile, "close_refine")
}

pub fn read_close_mask_netcdf(inputfile: impl AsRef<Path>) -> io::Result<CloseMask> {
    let inputfile = inputfile.as_ref();
    let file = netcdf::open(inputfile).map_err(netcdf_to_io_error)?;
    let close_num = required_dimension_len(&file, "close_num")?;
    let two = required_dimension_len(&file, "two")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close two dimension {two} must equal 2"),
        ));
    }
    let refine_degree = read_close_refine_netcdf(inputfile)?;
    let values = required_values_f64(&file, "close_points")?;
    let expected = close_num.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close_points dimensions {close_num}x2 overflow"),
        )
    })?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "close_points contains {} values, expected {expected}",
                values.len()
            ),
        ));
    }
    let points = values
        .chunks_exact(2)
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();
    let mask = CloseMask {
        refine_degree,
        points,
    };
    validate_close_mask(&mask).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid close mask NetCDF: {err}"),
        )
    })?;
    Ok(mask)
}

pub fn write_circle_mask_netcdf(output: impl AsRef<Path>, mask: &CircleMask) -> io::Result<()> {
    if mask.points.len() != mask.radius_km.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "circle points and radius arrays must have the same length",
        ));
    }
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("circle_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("circle_refine", &[])
            .map_err(netcdf_to_io_error)?;
        refine
            .put_value(mask.refine_degree as i32, ())
            .map_err(netcdf_to_io_error)?;
    }
    let mut point_values = Vec::with_capacity(mask.points.len() * 2);
    for point in &mask.points {
        point_values.extend([point.lon, point.lat]);
    }
    {
        let mut points = file
            .add_variable::<f64>("circle_points", &["circle_num", "two"])
            .map_err(netcdf_to_io_error)?;
        points
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut radius = file
            .add_variable::<f64>("circle_radius", &["circle_num"])
            .map_err(netcdf_to_io_error)?;
        radius
            .put_values(&mask.radius_km, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

pub fn write_close_mask_netcdf(output: impl AsRef<Path>, mask: &CloseMask) -> io::Result<()> {
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("close_num", mask.points.len())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    {
        let mut refine = file
            .add_variable::<i32>("close_refine", &[])
            .map_err(netcdf_to_io_error)?;
        refine
            .put_value(mask.refine_degree as i32, ())
            .map_err(netcdf_to_io_error)?;
    }
    let mut point_values = Vec::with_capacity(mask.points.len() * 2);
    for point in &mask.points {
        point_values.extend([point.lon, point.lat]);
    }
    {
        let mut points = file
            .add_variable::<f64>("close_points", &["close_num", "two"])
            .map_err(netcdf_to_io_error)?;
        points
            .put_values(&point_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

fn read_nonnegative_refine_netcdf(
    inputfile: impl AsRef<Path>,
    var_name: &str,
) -> io::Result<usize> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let variable = file.variable(var_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("NetCDF input is missing {var_name}"),
        )
    })?;
    let refine = variable
        .get_value::<i32, _>(())
        .map_err(netcdf_to_io_error)?;
    if refine < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{var_name} must be non-negative"),
        ));
    }
    Ok(refine as usize)
}

fn parse_float_row(line: &str, expected: usize, label: &str, row: usize) -> io::Result<Vec<f64>> {
    let values = line
        .split_whitespace()
        .map(|value| {
            value.parse::<f64>().map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid {label} coordinate {value}: {err}"),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    if values.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} row {row} must contain {expected} values"),
        ));
    }
    Ok(values)
}

/// Rectilinear Lambert-style vertex grid consumed by `lamb_mask_make`.
#[derive(Debug, Clone, PartialEq)]
pub struct LambertVertices {
    pub xi_vert: usize,
    pub eta_vert: usize,
    pub lon_vert: Vec<f64>,
    pub lat_vert: Vec<f64>,
}

/// Mode4 mesh payload written by `Mode4_Mesh_Save`.
#[derive(Debug, Clone, PartialEq)]
pub struct Mode4Mesh {
    pub lonlat_bound: Vec<LonLatPoint>,
    pub ngr_bound: Vec<[i32; 4]>,
    pub n_ngr: Vec<i32>,
}

impl Mode4Mesh {
    pub fn bound_points(&self) -> usize {
        self.lonlat_bound.len()
    }

    pub fn mode_points(&self) -> usize {
        self.ngr_bound.len()
    }
}

/// Read `xi_vert`/`eta_vert`, `lon_vert`, and `lat_vert` from a Lambert source.
pub fn read_lambert_vertices_netcdf(inputfile: impl AsRef<Path>) -> io::Result<LambertVertices> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let xi_vert = file
        .dimension("xi_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing xi_vert dimension"))?
        .len();
    let eta_vert = file
        .dimension("eta_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing eta_vert dimension"))?
        .len();
    let lon_vert = file
        .variable("lon_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing lon_vert variable"))?
        .get_values::<f64, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    let lat_vert = file
        .variable("lat_vert")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing lat_vert variable"))?
        .get_values::<f64, _>((.., ..))
        .map_err(netcdf_to_io_error)?;
    Ok(LambertVertices {
        xi_vert,
        eta_vert,
        lon_vert,
        lat_vert,
    })
}

/// Convert Lambert vertex arrays into the Fortran-indexed mode4 mesh payload.
pub fn lambert_vertices_to_mode4_mesh(vertices: &LambertVertices) -> io::Result<Mode4Mesh> {
    if vertices.xi_vert < 2 || vertices.eta_vert < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lambert xi_vert and eta_vert must both be at least 2",
        ));
    }
    let expected = vertices.xi_vert * vertices.eta_vert;
    if vertices.lon_vert.len() != expected || vertices.lat_vert.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "lambert lon_vert/lat_vert lengths must match xi_vert * eta_vert",
        ));
    }

    let lon_points = vertices.xi_vert - 1;
    let lat_points = vertices.eta_vert - 1;
    let bound_points = (lon_points + 1) * (lat_points + 1) + 1;
    let mode_points = lon_points * lat_points + 1;

    let mut lonlat_bound = vec![
        LonLatPoint {
            lon: -999.0,
            lat: -999.0
        };
        bound_points
    ];
    let mut out_idx = 1;
    for j in 0..vertices.eta_vert {
        for i in 0..vertices.xi_vert {
            let source_idx = i + j * vertices.xi_vert;
            let mut lon = vertices.lon_vert[source_idx];
            if lon > 180.0 {
                lon -= 360.0;
            }
            lonlat_bound[out_idx] = LonLatPoint {
                lon,
                lat: vertices.lat_vert[source_idx],
            };
            out_idx += 1;
        }
    }

    let mut ngr_bound = vec![[1_i32; 4]; mode_points];
    let mut cell_idx = 1;
    for j in 0..lat_points {
        for i in 0..lon_points {
            let lower_left = i + j * vertices.xi_vert + 2;
            ngr_bound[cell_idx] = [
                lower_left as i32,
                (lower_left + 1) as i32,
                (lower_left + vertices.xi_vert + 1) as i32,
                (lower_left + vertices.xi_vert) as i32,
            ];
            cell_idx += 1;
        }
    }

    Ok(Mode4Mesh {
        lonlat_bound,
        ngr_bound,
        n_ngr: vec![4; mode_points],
    })
}

pub fn read_mode4_mesh_netcdf(inputfile: impl AsRef<Path>) -> io::Result<Mode4Mesh> {
    let file = netcdf::open(inputfile.as_ref()).map_err(netcdf_to_io_error)?;
    let bound_points = required_dimension_len(&file, "bound_points")?;
    let mode_points = required_dimension_len(&file, "mode_points")?;
    let two = required_dimension_len(&file, "two")?;
    let four = required_dimension_len(&file, "four")?;
    if two != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 two dimension {two} must equal 2"),
        ));
    }
    if four != 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 four dimension {four} must equal 4"),
        ));
    }

    let lonlat_values = required_values_f64(&file, "lonlat_bound")?;
    let expected_lonlat = bound_points.checked_mul(2).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 lonlat_bound dimensions {bound_points}x2 overflow"),
        )
    })?;
    if lonlat_values.len() != expected_lonlat {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "lonlat_bound contains {} values, expected {expected_lonlat}",
                lonlat_values.len()
            ),
        ));
    }
    let lonlat_bound = lonlat_values
        .chunks_exact(2)
        .map(|row| LonLatPoint {
            lon: row[0],
            lat: row[1],
        })
        .collect::<Vec<_>>();

    let ngr_values = required_values_i32_2d(&file, "ngr_bound")?;
    let expected_ngr = mode_points.checked_mul(4).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("mode4 ngr_bound dimensions {mode_points}x4 overflow"),
        )
    })?;
    if ngr_values.len() != expected_ngr {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ngr_bound contains {} values, expected {expected_ngr}",
                ngr_values.len()
            ),
        ));
    }
    let ngr_bound = ngr_values
        .chunks_exact(4)
        .map(|row| [row[0], row[1], row[2], row[3]])
        .collect::<Vec<_>>();

    let n_ngr = required_values_i32(&file, "n_ngr")?;
    if n_ngr.len() != mode_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "n_ngr contains {} values, expected {mode_points}",
                n_ngr.len()
            ),
        ));
    }

    let mesh = Mode4Mesh {
        lonlat_bound,
        ngr_bound,
        n_ngr,
    };
    validate_mode4_mesh_for_area_judge(&mesh).map_err(|err| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid mode4 mesh NetCDF: {err}"),
        )
    })?;
    Ok(mesh)
}

pub fn write_mode4_mesh_netcdf(output: impl AsRef<Path>, mesh: &Mode4Mesh) -> io::Result<()> {
    if mesh.ngr_bound.len() != mesh.n_ngr.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mode4 ngr_bound and n_ngr lengths must match",
        ));
    }
    let output = output.as_ref();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = netcdf::create(output).map_err(netcdf_to_io_error)?;
    file.add_dimension("bound_points", mesh.bound_points())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("mode_points", mesh.mode_points())
        .map_err(netcdf_to_io_error)?;
    file.add_dimension("two", 2).map_err(netcdf_to_io_error)?;
    file.add_dimension("four", 4).map_err(netcdf_to_io_error)?;

    let mut lonlat_values = Vec::with_capacity(mesh.bound_points() * 2);
    for point in &mesh.lonlat_bound {
        lonlat_values.extend([point.lon, point.lat]);
    }
    {
        let mut lonlat_bound = file
            .add_variable::<f64>("lonlat_bound", &["bound_points", "two"])
            .map_err(netcdf_to_io_error)?;
        lonlat_bound
            .put_values(&lonlat_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }

    let mut ngr_values: Vec<i32> = Vec::with_capacity(mesh.mode_points() * 4);
    for row in &mesh.ngr_bound {
        ngr_values.extend_from_slice(row);
    }
    {
        let mut ngr_bound = file
            .add_variable::<i32>("ngr_bound", &["mode_points", "four"])
            .map_err(netcdf_to_io_error)?;
        ngr_bound
            .put_values(&ngr_values, (.., ..))
            .map_err(netcdf_to_io_error)?;
    }
    {
        let mut n_ngr = file
            .add_variable::<i32>("n_ngr", &["mode_points"])
            .map_err(netcdf_to_io_error)?;
        n_ngr
            .put_values(&mesh.n_ngr, ..)
            .map_err(netcdf_to_io_error)?;
    }
    Ok(())
}

pub fn convert_lambert_mask_netcdf(
    inputfile: impl AsRef<Path>,
    mask_select: &str,
    file_dir: impl AsRef<Path>,
    counts: &mut MaskCountState,
) -> io::Result<PathBuf> {
    let vertices = read_lambert_vertices_netcdf(inputfile)?;
    let mesh = lambert_vertices_to_mode4_mesh(&vertices)?;
    let output = counts.next_lambert_output(mask_select, 0, file_dir)?;
    write_mode4_mesh_netcdf(&output, &mesh)?;
    Ok(output)
}

/// Rust-owned mask counters matching `mask_domain_ndm`, `mask_refine_ndm`, and
/// `mask_patch_ndm` updates in `bbox_mask_make`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaskCountState {
    pub mask_domain_ndm: usize,
    pub mask_refine_ndm: [usize; 10],
    pub mask_patch_ndm: [usize; 10],
}

impl MaskCountState {
    /// Advance counters and return the Fortran bbox output filename.
    pub fn next_bbox_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "bbox", refine_degree, file_dir, 2)
    }

    /// Advance counters and return the Fortran circle output filename.
    pub fn next_circle_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "circle", refine_degree, file_dir, 2)
    }

    /// Advance counters and return the Fortran close output filename.
    pub fn next_close_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "close", refine_degree, file_dir, 3)
    }

    /// Advance counters and return the Fortran lambert output filename.
    pub fn next_lambert_output(
        &mut self,
        mask_select: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
    ) -> io::Result<PathBuf> {
        self.next_mask_output(mask_select, "lambert", refine_degree, file_dir, 2)
    }

    fn next_mask_output(
        &mut self,
        mask_select: &str,
        mask_type: &str,
        refine_degree: usize,
        file_dir: impl AsRef<Path>,
        count_width: usize,
    ) -> io::Result<PathBuf> {
        if refine_degree >= self.mask_refine_ndm.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refine_degree must fit mask counter arrays 0:9",
            ));
        }
        let count = match mask_select {
            "mask_domain" => {
                self.mask_domain_ndm += 1;
                self.mask_domain_ndm
            }
            "mask_refine" => {
                self.mask_refine_ndm[refine_degree] += 1;
                self.mask_refine_ndm[refine_degree]
            }
            "mask_patch" => {
                self.mask_patch_ndm[refine_degree] += 1;
                self.mask_patch_ndm[refine_degree]
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unsupported mask_select {other}"),
                ));
            }
        };
        Ok(file_dir.as_ref().join("tmpfile").join(format!(
            "{mask_select}_{mask_type}_{refine_degree}_{count:0count_width$}.nc4"
        )))
    }
}
