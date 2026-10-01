mod adaptive;
use earthmesh_delivery::gridfile_quality_input as gridfile;
mod hfield;

use std::io;

use earthmesh_mesh::LonLatDegrees;
use earthmesh_refine::TargetLevelField;

use crate::GridfileMeshPoints;

pub use adaptive::attach_adaptive_diagnostics_from_gridfile_path;
pub(crate) use gridfile::tri_quality_cells_from_gridfile;
#[allow(unused_imports)]
pub use gridfile::{
    quality_input_from_gridfile, quality_input_from_gridfile_hex,
    quality_input_from_gridfile_hex_native, read_gridfile_cell_lineages, read_gridfile_mesh_points,
};
pub use hfield::{
    attach_hfield_diagnostics_from_gridfile_namelist, attach_hfield_diagnostics_from_namelist,
    attach_hfield_diagnostics_from_namelist_with_g,
};

/// Where a hex cell reads its target. A tri cell always reads it at its
/// centre, the M point.
#[derive(Clone, Copy)]
enum HexSample {
    /// The deepest target over the cell's corners, which suits a field that
    /// varies smoothly.
    Corners,
    /// The cell centre, the W point: a demand with a hard edge, where a corner
    /// can sit inside while the centre, which decides the marking, does not.
    Centre,
}

/// Each quality cell's target level, read through the demand interface only
/// -- whichever form the project stated the demand in.
fn target_levels_for_quality_cells(
    mesh: &GridfileMeshPoints,
    kind: &str,
    hex_sample: HexSample,
    targets: &dyn TargetLevelField,
    route: &str,
) -> io::Result<Vec<u32>> {
    let at = |lon: f64, lat: f64| -> io::Result<u32> {
        Ok(targets.target_level(LonLatDegrees::new(lon, lat))? as u32)
    };
    match kind.trim() {
        "tri" => gridfile::tri_quality_cells_from_gridfile(mesh)?
            .into_iter()
            .map(|(mi, _)| at(mesh.m_lon[mi], mesh.m_lat[mi]))
            .collect(),
        "hex" => gridfile::hex_quality_cells_from_gridfile(mesh)?
            .into_iter()
            .map(|(wi, corners)| match hex_sample {
                HexSample::Centre => at(mesh.w_lon[wi], mesh.w_lat[wi]),
                HexSample::Corners => corners
                    .iter()
                    .filter_map(|&mi| Some((*mesh.m_lon.get(mi)?, *mesh.m_lat.get(mi)?)))
                    .map(|(lon, lat)| at(lon, lat))
                    .try_fold(0, |deepest, level| Ok(deepest.max(level?))),
            })
            .collect(),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{route} diagnostics support tri or hex view, got {other}"),
        )),
    }
}
