//! Rust orchestration adapters for replacing `mkgrd.x` side effects.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::MkgrdWorkspacePlan;

/// Evidence report from applying the filesystem subset of `mkgrd.F90:read_nl`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceApplyReport {
    pub removed_file_dir: Option<PathBuf>,
    pub removed_filelists: Vec<PathBuf>,
    pub created_directories: Vec<PathBuf>,
    pub copied_namelist_to: Option<PathBuf>,
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
    let inputfile = inputfile.as_ref();
    let extension = inputfile.extension().and_then(|value| value.to_str());
    if !matches!(extension, Some("nc") | Some("nc4")) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bbox NetCDF input must end with .nc or .nc4",
        ));
    }
    if refine_degree > max_iter_spc {
        return Ok(None);
    }

    let output = counts.next_bbox_output(mask_select, refine_degree, file_dir)?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(inputfile, &output)?;
    Ok(Some(output))
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
        Ok(file_dir
            .as_ref()
            .join("tmpfile")
            .join(format!("{mask_select}_bbox_{refine_degree}_{count:02}.nc4")))
    }
}
