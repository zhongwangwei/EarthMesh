use std::io;
use std::path::Path;

use crate::*;

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
            "domain mask_postproc I/O plan supports earthmesh, landmesh, or oceanmesh; atmos/atmosmesh uses the MPAS branch",
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
