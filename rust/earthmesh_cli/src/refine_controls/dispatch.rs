use std::io;

use earthmesh_core::EarthmeshConfig;

use crate::{
    native_grid_refinement_requested, native_grid_surface_global_expansion_requested,
    read_native_grid_mdomain,
};

pub(crate) fn refine_pipeline_refine_dispatch_requested(
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
        native_grid_surface_global_expansion_requested(contents, config.mesh_type.trim())?;
    let native_refine_regions_requested =
        native_grid_refinement_requested(contents, config.mesh_type.trim())?;
    let native_mdomain = read_native_grid_mdomain(contents)?;
    let compatibility_specified_refine = config.refine;
    // An explicitly selected certified backend also owns the uniform mother
    // grid. Disabled refinement must not silently select Method-C instead.
    let certified_mother = config
        .refine_backend
        .trim()
        .eq_ignore_ascii_case("certified");
    Ok(native_mdomain.is_some()
        || native_surface_global_expansion
        || native_refine_regions_requested
        || compatibility_specified_refine
        || certified_mother)
}
