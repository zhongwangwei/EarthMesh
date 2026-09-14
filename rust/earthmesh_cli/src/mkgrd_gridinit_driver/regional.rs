use crate::fvcom_mesh_2dm_output_path;
use crate::read_method_c_domain_region;
use crate::read_obc_order_netcdf;
use crate::read_unstructured_mesh_netcdf;
use crate::regional_gridfile_writers::write_regional_gridfile;
use crate::unstructured_mesh_write_report_from_file;
use crate::write_clean_regional_ocean_gridfile;
use crate::write_fvcom_2dm_from_carved;
use crate::write_landtype_masked_gridfile_with_refine_levels;
use crate::GridRegion;
use crate::LonLatPoint;
use crate::MkgrdGridinitRunReport;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

use super::global::run_mkgrd_gridinit_global_namelist;
use super::landtype::landtype_gridnum_perdegree;

/// From-scratch regional **clip** with no refinement.
///
/// A `mask_domain_global=.false.` run names a containment region
/// (`mask_domain_type` + `mask_domain_fprefix`). The Method-C refine path only
/// reaches the clip step when a refinement region is *also* active, but a plain
/// regional grid should subset to its domain regardless. This generates the
/// global base mesh ([`run_mkgrd_gridinit_global_namelist`]) and then keeps only
/// the in-domain cells via the shared `write_regional_gridfile` writer — the
/// exact clip the Method-C path performs, minus any spawn/refine. It works for every
/// mesh type. The returned report's `gridfile` is rewritten to the clipped
/// result, so existing `gridfile=` consumers (CLI print, GUI) pick up the subset
/// mesh with no extra plumbing.
pub fn run_mkgrd_regional_clip_base_namelist(
    namelist_source: impl AsRef<Path>,
    workdir: impl AsRef<Path>,
    max_tris: usize,
) -> io::Result<MkgrdGridinitRunReport> {
    run_mkgrd_regional_clip_base(namelist_source.as_ref(), workdir.as_ref(), max_tris, false)
}

pub(crate) fn run_mkgrd_regional_clip_base(
    namelist_source: &Path,
    workdir: &Path,
    max_tris: usize,
    final_delivery: bool,
) -> io::Result<MkgrdGridinitRunReport> {
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let mesh_type = config.mesh_type.trim().to_string();
    let landtype = config.landtype_file.trim().to_string();
    let carve_landtype = matches!(mesh_type.as_str(), "landmesh" | "oceanmesh")
        && !landtype.is_empty()
        && landtype != "none"
        && landtype != "/tmp";
    if final_delivery && !config.mask_patch_on {
        return super::regional_delivery::run_final_base(
            namelist_source,
            workdir,
            max_tris,
            &config,
            carve_landtype,
        );
    }
    let region = read_method_c_domain_region(&config)?; // None ⇒ global (no geometric clip)
    if region.is_none() && !carve_landtype {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "base clip/carve needs a non-global mask domain (mask_domain_global=.false. + \
             mask_domain_fprefix) or a land/ocean landcover file (landtype_file)",
        ));
    }

    let landtype_gpd = carve_landtype
        .then(|| landtype_gridnum_perdegree(Path::new(&landtype)))
        .transpose()?;

    let mut gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let file_dir = PathBuf::from(config.file_dir());
    let mode_grid = config.mode_grid.trim();
    if let Some(close_points) =
        clean_regional_ocean_close_points(region.as_ref(), &mesh_type, mode_grid, carve_landtype)
    {
        let gpd = landtype_gpd.expect("carve landtype preflight should provide grid resolution");
        let plan = write_clean_regional_ocean_gridfile(
            &gridinit.gridfile.output,
            close_points,
            Path::new(&landtype),
            nxp,
            gpd,
            config.mask_sea_ratio,
            &file_dir,
        )?;
        if !config.defer_model_exports {
            let carved = read_unstructured_mesh_netcdf(&plan.result_gridfile)?;
            let obc_order = match &plan.obc_output {
                Some(path) if path.exists() => read_obc_order_netcdf(path)?,
                _ => Vec::new(),
            };
            gridinit.fvcom_2dm = Some(write_fvcom_2dm_from_carved(
                &carved,
                &obc_order,
                &fvcom_mesh_2dm_output_path(&file_dir),
            )?);
        }
        gridinit.raw_output = Some(gridinit.gridfile.clone());
        gridinit.gridfile = unstructured_mesh_write_report_from_file(&plan.result_gridfile)?;
        return Ok(gridinit);
    }

    apply_base_clip_and_carve(&mut gridinit, region.as_ref(), landtype_gpd, &file_dir)?;
    Ok(gridinit)
}

/// The same clip/carve kernel serves raw callers and staged final handoffs.
pub(super) fn apply_base_clip_and_carve(
    gridinit: &mut MkgrdGridinitRunReport,
    region: Option<&GridRegion>,
    landtype_gpd: Option<usize>,
    file_dir: &Path,
) -> io::Result<()> {
    let config = &gridinit.config;
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let mode_grid = config.mode_grid.trim();
    let mesh_type = config.mesh_type.trim();
    let landtype = config.landtype_file.trim();
    // 1) Optional geometric CLIP to the domain (regional bbox/circle/close): keep
    // only the in-region cells. Retain the exact full parent and its metadata
    // before the shared regional writer replaces the selected result file.
    if let Some(region) = region {
        let raw_path = base_raw_parent_path(file_dir, nxp, mode_grid);
        crate::ensure_parent_dir(&raw_path)?;
        let output_path = gridinit.gridfile.output.clone();
        fs::copy(&output_path, &raw_path)?;
        let kept = write_regional_gridfile(&raw_path, &output_path, region, mode_grid)?;
        if kept == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C domain mask kept no cells",
            ));
        }
        gridinit.raw_output = Some(unstructured_mesh_write_report_from_file(&raw_path)?);
        gridinit.gridfile = unstructured_mesh_write_report_from_file(&output_path)?;
    }

    // 2) Optional landcover CARVE: keep land cells (landmesh) / ocean cells
    // (oceanmesh) by sampling each cell centre against the landtype file — the
    // same land/sea masking the compatibility egui did. Runs on the current result
    // gridfile (post-clip when regional). The shared writer rejects an empty carve.
    if let Some(gpd) = landtype_gpd {
        // Sample resolution must equal the landcover file's own grid, NOT
        // NL%gridnum_perdegree (which need not match it).
        if gpd > 0 {
            let masked = base_carve_path(file_dir, nxp, mode_grid, mesh_type);
            let kept = write_landtype_masked_gridfile_with_refine_levels(
                &gridinit.gridfile.output,
                &masked,
                landtype,
                gpd,
                mode_grid,
                mesh_type,
                None,
                None,
                config.isolated_ocean,
                // Gridinit runs before any refinement, so nothing has been
                // named yet.
                None,
            )?;
            if kept > 0 {
                if gridinit.raw_output.is_none() {
                    gridinit.raw_output = Some(gridinit.gridfile.clone());
                }
                gridinit.gridfile = unstructured_mesh_write_report_from_file(&masked)?;
            }
        }
    }
    Ok(())
}

pub(super) fn base_raw_parent_path(file_dir: &Path, nxp: usize, mode_grid: &str) -> PathBuf {
    file_dir
        .join("tmpfile")
        .join(format!("gridfile_NXP{nxp:04}_clip_raw_{mode_grid}.nc4"))
}

pub(super) fn base_carve_path(
    file_dir: &Path,
    nxp: usize,
    mode_grid: &str,
    mesh_type: &str,
) -> PathBuf {
    file_dir
        .join("result")
        .join(format!("gridfile_NXP{nxp:04}_{mode_grid}_{mesh_type}.nc4"))
}

pub(super) fn clean_regional_ocean_close_points<'a>(
    region: Option<&'a GridRegion>,
    mesh_type: &str,
    mode_grid: &str,
    carve_landtype: bool,
) -> Option<&'a [LonLatPoint]> {
    if !carve_landtype || mesh_type != "oceanmesh" || mode_grid != "tri" {
        return None;
    }
    match region {
        Some(GridRegion::Close { points }) => Some(points),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_ocean_path_only_handles_tri_close_ocean_with_landtype() {
        let region = GridRegion::Close {
            points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }],
        };
        assert!(
            clean_regional_ocean_close_points(Some(&region), "oceanmesh", "tri", true).is_some()
        );
        assert!(
            clean_regional_ocean_close_points(Some(&region), "oceanmesh", "hex", true).is_none()
        );
        assert!(
            clean_regional_ocean_close_points(Some(&region), "landmesh", "tri", true).is_none()
        );
        assert!(
            clean_regional_ocean_close_points(Some(&region), "oceanmesh", "tri", false).is_none()
        );
    }
}
