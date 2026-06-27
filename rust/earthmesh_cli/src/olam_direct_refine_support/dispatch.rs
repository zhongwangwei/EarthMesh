use std::io;

use earthmesh_core::EarthmeshConfig;

use crate::*;

pub(crate) fn olam_direct_refine_dispatch_requested(
    contents: &str,
    config: &EarthmeshConfig,
) -> io::Result<bool> {
    if !matches!(
        config.mesh_type.trim(),
        "atmos" | "atmosmesh" | "landmesh" | "oceanmesh" | "LOCmesh" | "earthmesh"
    ) {
        return Ok(false);
    }
    let native_surface_global_expansion =
        olam_native_surface_global_expansion_requested(contents, config.mesh_type.trim())?;
    let native_olam_regions_requested =
        olam_native_refinement_requested(contents, config.mesh_type.trim())?;
    let native_mdomain = read_olam_native_mdomain(contents)?;
    let legacy_specified_refine = config.refine;
    Ok(native_mdomain.is_some()
        || native_surface_global_expansion
        || native_olam_regions_requested
        || legacy_specified_refine)
}
