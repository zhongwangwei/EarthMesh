//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{MaskOperation, MkgrdWorkspacePlan};

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
