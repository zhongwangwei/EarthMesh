use crate::fvcom_mesh_2dm_output_path;
use crate::read_method_c_domain_region;
use crate::read_obc_order_netcdf;
use crate::read_unstructured_mesh_netcdf;
use crate::unstructured_mesh_write_report_from_file;
use crate::write_clean_regional_ocean_gridfile;
use crate::write_fvcom_2dm_from_carved;
use crate::write_landtype_masked_gridfile_with_refine_levels;
use crate::write_method_c_mesh_with_optional_domain;
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
    let namelist_source = namelist_source.as_ref();
    let workdir = workdir.as_ref();
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let region = read_method_c_domain_region(&config)?; // None ⇒ global (no geometric clip)
    let mesh_type = config.mesh_type.trim().to_string();
    let landtype = config.landtype_file.trim().to_string();
    let carve_landtype = matches!(mesh_type.as_str(), "landmesh" | "oceanmesh")
        && !landtype.is_empty()
        && landtype != "none"
        && landtype != "/tmp"
        && Path::new(&landtype).is_file();
    if region.is_none() && !carve_landtype {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "base clip/carve needs a non-global mask domain (mask_domain_global=.false. + \
             mask_domain_fprefix) or a land/ocean landcover file (landtype_file)",
        ));
    }

    let mut gridinit = run_mkgrd_gridinit_global_namelist(namelist_source, workdir, max_tris)?;
    let nxp = usize::try_from(config.nxp)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "NXP must fit usize"))?;
    let file_dir = PathBuf::from(config.file_dir());
    let mode_grid = config.mode_grid.trim();
    if let Some(close_points) =
        clean_regional_ocean_close_points(region.as_ref(), &mesh_type, mode_grid, carve_landtype)
    {
        let gpd = landtype_gridnum_perdegree(Path::new(&landtype))?;
        let plan = write_clean_regional_ocean_gridfile(
            &gridinit.gridfile.output,
            close_points,
            Path::new(&landtype),
            nxp,
            gpd,
            config.mask_sea_ratio,
            &file_dir,
        )?;
        let carved = read_unstructured_mesh_netcdf(&plan.result_gridfile)?;
        let obc_order = match &plan.obc_output {
            Some(path) if path.exists() => read_obc_order_netcdf(path)?,
            _ => Vec::new(),
        };
        let fvcom_2dm = write_fvcom_2dm_from_carved(
            &carved,
            &obc_order,
            &fvcom_mesh_2dm_output_path(&file_dir),
        )?;
        gridinit.gridfile = unstructured_mesh_write_report_from_file(&plan.result_gridfile)?;
        gridinit.fvcom_2dm = Some(fvcom_2dm);
        return Ok(gridinit);
    }

    // 1) Optional geometric CLIP to the domain (regional bbox/circle/close): keep
    // only the in-region cells. `mesh` is read into memory, so overwriting its
    // result file with the subset is safe.
    if let Some(region) = &region {
        let mesh = read_unstructured_mesh_netcdf(&gridinit.gridfile.output)?;
        let raw_path = file_dir
            .join("tmpfile")
            .join(format!("gridfile_NXP{nxp:04}_clip_raw_{mode_grid}.nc4"));
        crate::ensure_parent_dir(&raw_path)?;
        let output_path = gridinit.gridfile.output.clone();
        let (_, clipped) = write_method_c_mesh_with_optional_domain(
            &mesh,
            &raw_path,
            &output_path,
            Some(region),
            mode_grid,
        )?;
        gridinit.gridfile = clipped;
    }

    // 2) Optional landcover CARVE: keep land cells (landmesh) / ocean cells
    // (oceanmesh) by sampling each cell centre against the landtype file — the
    // same land/sea masking the compatibility egui did. Runs on the current result
    // gridfile (post-clip when regional). Kept==0 leaves the mesh untouched.
    if carve_landtype {
        // Sample resolution must equal the landcover file's own grid, NOT
        // NL%gridnum_perdegree (which need not match it).
        let gpd = landtype_gridnum_perdegree(Path::new(&landtype))?;
        if gpd > 0 {
            let masked = file_dir
                .join("result")
                .join(format!("gridfile_NXP{nxp:04}_{mode_grid}_{mesh_type}.nc4"));
            let kept = write_landtype_masked_gridfile_with_refine_levels(
                &gridinit.gridfile.output,
                &masked,
                &landtype,
                gpd,
                mode_grid,
                &mesh_type,
                None,
                None,
                config.isolated_ocean,
            )?;
            if kept > 0 {
                gridinit.gridfile = unstructured_mesh_write_report_from_file(&masked)?;
            }
        }
    }
    Ok(gridinit)
}

fn clean_regional_ocean_close_points<'a>(
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
