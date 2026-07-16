use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::MethodCDelaunayMesh;

pub(crate) fn final_quality_non_negative_usize(value: i32, message: &str) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, message))
}

pub(crate) fn method_c_spring_iterations(
    refine: &RefineConfig,
    is_atmosmesh: bool,
) -> io::Result<usize> {
    if refine.spring_global_type != 1 && refine.spring_regional_type <= 0 {
        return Ok(0);
    }
    if refine.niter_refine_specified {
        return final_quality_non_negative_usize(
            refine.niter_refine,
            "Method-C specified refine niter_refine must be non-negative",
        );
    }
    Ok(if is_atmosmesh { 5000 } else { 2000 })
}

pub(crate) fn native_spawn_spring_iterations(
    _refine: &RefineConfig,
    is_atmosmesh: bool,
    runtype: &str,
) -> io::Result<usize> {
    Ok(if runtype.trim() == "MAKEGRID_PLOT" {
        100
    } else if is_atmosmesh {
        5000
    } else {
        2000
    })
}

pub(crate) fn native_spawn_uses_cartesian_xy(
    native_mdomain: Option<usize>,
    mask_domain_global: bool,
    native_only_spawn: bool,
) -> bool {
    native_only_spawn && native_mdomain.map_or(!mask_domain_global, |mdomain| mdomain == 5)
}

pub(crate) fn validate_native_spawn_mdomain(native_mdomain: Option<usize>) -> io::Result<()> {
    match native_mdomain {
        Some(mdomain @ 1..=4) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "native Method-C spawn_nest supports mdomain 0 or 5; mdomain={} uses a different Canonical gridinit path",
                mdomain
            ),
        )),
        _ => Ok(()),
    }
}

pub(crate) fn native_initial_delaunay_mesh(
    nxp: usize,
    native_mdomain: Option<usize>,
    native_deltax: f64,
) -> io::Result<Option<MethodCDelaunayMesh>> {
    if native_mdomain == Some(5) {
        return MethodCDelaunayMesh::from_cart_hex(nxp, native_deltax).map(Some);
    }
    Ok(None)
}
