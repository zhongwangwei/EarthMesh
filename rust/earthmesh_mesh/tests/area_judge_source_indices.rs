use earthmesh_mesh::{
    area_judge_minmax_range_make_fortran_indexed, area_judge_source_find_fortran_indexed,
    AreaJudgeAxis, AreaJudgeSourceBounds,
};

fn one_degree_lon_vertices_fortran_indexed() -> Vec<f64> {
    let mut vertices = Vec::with_capacity(362);
    vertices.push(f64::NAN);
    for lon in -180..=180 {
        vertices.push(lon as f64);
    }
    vertices
}

fn one_degree_lat_vertices_fortran_indexed() -> Vec<f64> {
    let mut vertices = Vec::with_capacity(182);
    vertices.push(f64::NAN);
    for lat in (-90..=90).rev() {
        vertices.push(lat as f64);
    }
    vertices
}

#[test]
fn source_find_uses_fortran_one_based_window_for_lon_and_lat_vertices() {
    let lon_vertices = one_degree_lon_vertices_fortran_indexed();
    let lat_vertices = one_degree_lat_vertices_fortran_indexed();

    assert_eq!(
        area_judge_source_find_fortran_indexed(
            -180.0,
            &lon_vertices,
            AreaJudgeAxis::Longitude,
            1,
            360,
        ),
        Some(1)
    );
    assert_eq!(
        area_judge_source_find_fortran_indexed(
            -179.2,
            &lon_vertices,
            AreaJudgeAxis::Longitude,
            1,
            360,
        ),
        Some(2)
    );
    assert_eq!(
        area_judge_source_find_fortran_indexed(
            180.0,
            &lon_vertices,
            AreaJudgeAxis::Longitude,
            1,
            360,
        ),
        Some(361)
    );

    assert_eq!(
        area_judge_source_find_fortran_indexed(
            90.0,
            &lat_vertices,
            AreaJudgeAxis::Latitude,
            1,
            180,
        ),
        Some(1)
    );
    assert_eq!(
        area_judge_source_find_fortran_indexed(
            89.2,
            &lat_vertices,
            AreaJudgeAxis::Latitude,
            1,
            180,
        ),
        Some(2)
    );
    assert_eq!(
        area_judge_source_find_fortran_indexed(
            -90.0,
            &lat_vertices,
            AreaJudgeAxis::Latitude,
            1,
            180,
        ),
        Some(181)
    );
}

#[test]
fn minmax_range_make_returns_fortran_adjusted_cell_bounds() {
    let lon_vertices = one_degree_lon_vertices_fortran_indexed();
    let lat_vertices = one_degree_lat_vertices_fortran_indexed();

    let bounds = area_judge_minmax_range_make_fortran_indexed(
        -179.2,
        -177.2,
        89.2,
        87.2,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
    );

    assert_eq!(
        bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 2,
            maxlon_source: 2,
            maxlat_source: 2,
            minlat_source: 2,
        })
    );
}

#[test]
fn minmax_range_make_preserves_fortran_eastern_and_southern_edge_adjustments() {
    let lon_vertices = one_degree_lon_vertices_fortran_indexed();
    let lat_vertices = one_degree_lat_vertices_fortran_indexed();

    let bounds = area_judge_minmax_range_make_fortran_indexed(
        178.2,
        179.2,
        -88.2,
        -89.2,
        &lon_vertices,
        &lat_vertices,
        1,
        360,
        180,
    );

    assert_eq!(
        bounds,
        Some(AreaJudgeSourceBounds {
            minlon_source: 360,
            maxlon_source: 360,
            maxlat_source: 180,
            minlat_source: 180,
        })
    );
}
