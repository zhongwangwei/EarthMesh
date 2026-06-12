//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, GridMemory, IjTabs, MaskOperation, MkgrdWorkspacePlan};
use earthmesh_mesh::{
    centroid_spherical_mesh_fortran_indexed, circumcenter_spherical_mesh_fortran_indexed,
    lonlat_points_to_unit_xyz, xyz_to_lonlat_degrees, BoundaryConnection, BoundaryOrders,
    CartesianPoint, LonLatDegrees,
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
pub fn finalize_mask_postproc_layout_to_unstructured_mesh(
    layout: &MaskPostprocLayout,
    is_in_domain_ustr: &[i32],
    mode_grid: &str,
) -> io::Result<UnstructuredMesh> {
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
    let reindex = earthmesh_mesh::sort_and_reindex_vertices(&unique_vertices, layout.ustr_bounds)?;
    final_data.center_neighbors_final =
        earthmesh_mesh::reindex_final_center_vertices_fortran_indexed(
            &final_data.center_neighbors_final,
            &final_data.center_neighbor_counts_final,
            &reindex.vertex_mapping,
        )?;

    unstructured_mesh_from_mask_postproc_final(&final_data, mode_grid)
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

pub fn read_close_refine_netcdf(inputfile: impl AsRef<Path>) -> io::Result<usize> {
    read_nonnegative_refine_netcdf(inputfile, "close_refine")
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
