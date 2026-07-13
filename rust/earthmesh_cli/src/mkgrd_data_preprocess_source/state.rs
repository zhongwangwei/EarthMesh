use crate::build_area_judge_base_state_one_based;
use crate::MkgrdDataPreprocessSourceState;
use std::io;
use std::path::Path;

use earthmesh_core::EarthmeshConfig;

use super::read_landtype_data_preprocess_one_based;

/// Build the owned source-state bundle consumed by current `mkgrd` refine
/// executors from `data_preprocess` landtype data plus `Area_judge` domain
/// classification.
pub fn build_mkgrd_data_preprocess_source_state_one_based(
    file_dir: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mask_domain_global: bool,
    mask_domain_type: &str,
    mask_domain_ndm: usize,
    mesh_type: &str,
    refine: bool,
    num_vertex: usize,
    first_triangle_id: usize,
) -> io::Result<MkgrdDataPreprocessSourceState> {
    let preprocess = read_landtype_data_preprocess_one_based(landtype_file, gridnum_perdegree)?;
    let base = build_area_judge_base_state_one_based(
        file_dir,
        mask_domain_global,
        mask_domain_type,
        mask_domain_ndm,
        &preprocess.landtypes_global,
        mesh_type,
        refine,
        &preprocess.lon_vertex,
        &preprocess.lat_vertex,
        &preprocess.lon_i,
        &preprocess.lat_i,
        gridnum_perdegree,
        preprocess.nlons_source,
        preprocess.nlats_source,
    )?;

    Ok(MkgrdDataPreprocessSourceState {
        lon_vertex: preprocess.lon_vertex,
        lat_vertex: preprocess.lat_vertex,
        lon_i: preprocess.lon_i,
        lat_i: preprocess.lat_i,
        gridnum_perdegree,
        nlons_source: preprocess.nlons_source,
        nlats_source: preprocess.nlats_source,
        first_triangle_id,
        num_vertex,
        sources: vec![preprocess.source.clone()],
        is_in_domain: base.domain.is_in_domain,
        seaorland: base.seaorland.seaorland,
        landtypes_global: preprocess.landtypes_global,
        maxlc: preprocess.maxlc,
    })
}

/// Build the owned `data_preprocess` source-state bundle from parsed mkgrd
/// namelist config plus CLI/source-state overrides.
///
/// This centralizes the direct `mkgrd` front-end expansion of `NL%landtype_file`,
/// `NL%gridnum_perdegree`, `NL%mode_grid`, domain flags, and source first-id
/// values before the current refine executors consume typed Rust state.
pub fn build_mkgrd_data_preprocess_source_state_from_config_one_based(
    file_dir: impl AsRef<Path>,
    config: &EarthmeshConfig,
    source_gridnum_perdegree: Option<usize>,
    source_first_triangle_id: usize,
) -> io::Result<MkgrdDataPreprocessSourceState> {
    let gridnum_perdegree = match source_gridnum_perdegree {
        Some(value) => value,
        None => usize::try_from(config.gridnum_perdegree).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "NL%gridnum_perdegree must be positive for data_preprocess source state, got {}",
                    config.gridnum_perdegree
                ),
            )
        })?,
    };
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "NL%gridnum_perdegree must be positive for data_preprocess source state, got {}",
                config.gridnum_perdegree
            ),
        ));
    }
    build_mkgrd_data_preprocess_source_state_one_based(
        file_dir,
        Path::new(config.landtype_file.trim()),
        gridnum_perdegree,
        config.mask_domain_global,
        config.mask_domain_type.trim(),
        1,
        config.mesh_type.trim(),
        config.refine,
        1,
        source_first_triangle_id,
    )
}
