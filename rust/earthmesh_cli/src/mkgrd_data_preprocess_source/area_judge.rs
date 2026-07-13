use crate::build_area_judge_base_state_one_based;
use crate::build_area_judge_seaorland_one_based;
use crate::validate_i32_matrix_shape;
use crate::AreaJudgeBaseStateReport;
use crate::DataPreprocessAreaJudgeSourceReport;
use std::io;
use std::path::Path;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::read_landtype_data_preprocess_one_based;

/// Read the `data_preprocess` landtype source and immediately build the
/// `Area_judge` sea/land source mask used by containment/refinement.
pub fn read_data_preprocess_area_judge_source_one_based(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    is_in_domain: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
    mesh_type: &str,
    refine: bool,
) -> io::Result<DataPreprocessAreaJudgeSourceReport> {
    let preprocess = read_landtype_data_preprocess_one_based(landtype_file, gridnum_perdegree)?;
    let expected_lon = preprocess.nlons_source + 1;
    let expected_lat = preprocess.nlats_source + 1;
    validate_i32_matrix_shape("IsInDmArea_grid", is_in_domain, expected_lon, expected_lat)?;
    let seaorland = build_area_judge_seaorland_one_based(
        is_in_domain,
        &preprocess.landtypes_global,
        bounds,
        mesh_type,
        refine,
    )?;
    Ok(DataPreprocessAreaJudgeSourceReport {
        preprocess,
        seaorland,
    })
}

/// Read `data_preprocess` landtype data and build the non-restart
/// `Area_judge` base state from file-level inputs, matching the Canonical
/// `data_preprocess(); Area_judge()` handoff without module globals.
pub fn read_data_preprocess_area_judge_base_state_one_based(
    file_dir: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    mask_domain_global: bool,
    mask_domain_type: &str,
    mask_domain_ndm: usize,
    mesh_type: &str,
    refine: bool,
) -> io::Result<AreaJudgeBaseStateReport> {
    let preprocess = read_landtype_data_preprocess_one_based(landtype_file, gridnum_perdegree)?;
    build_area_judge_base_state_one_based(
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
    )
}
