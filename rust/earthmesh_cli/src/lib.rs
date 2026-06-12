//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{EarthmeshConfig, GridMemory, IjTabs, MaskOperation, MkgrdWorkspacePlan};
use earthmesh_mesh::{
    centroid_spherical_mesh_fortran_indexed, circumcenter_spherical_mesh_fortran_indexed,
    lonlat_points_to_unit_xyz, xyz_to_lonlat_degrees, CartesianPoint, LonLatDegrees,
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

/// Evidence report from writing an unstructured gridfile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstructuredMeshWriteReport {
    pub output: PathBuf,
    pub sjx_points: usize,
    pub lbx_points: usize,
    pub dimc: usize,
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
