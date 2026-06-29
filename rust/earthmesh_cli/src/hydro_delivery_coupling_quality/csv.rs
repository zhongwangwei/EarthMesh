use std::fs;
use std::io;
use std::path::Path;

use crate::*;

pub fn write_colm_coupling_csv_from_mesh(
    gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    case_name: &str,
    mode_grid: &str,
    output_csv: impl AsRef<Path>,
) -> io::Result<ColmSurfaceCounts> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let centers = match mode_grid.trim() {
        "tri" => &mesh.m_points,
        _ => &mesh.w_points,
    };
    let lt = read_landtype_data_preprocess_fortran_indexed(landtype_file, gridnum_perdegree)?;
    if lt.lon_i.len() < 3 || lt.lat_i.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "land-type axes too short to derive sampling step",
        ));
    }
    let (nlon, nlat) = (lt.nlons_source, lt.nlats_source);
    let lon0 = lt.lon_i[1];
    let lat0 = lt.lat_i[1];
    let dlon = lt.lon_i[2] - lt.lon_i[1];
    let dlat = lt.lat_i[2] - lt.lat_i[1];

    let sample = |lon: f64, lat: f64| -> i32 {
        let li = (((lon - lon0) / dlon).round() as i64).rem_euclid(nlon as i64);
        let lj = (((lat - lat0) / dlat).round() as i64).clamp(0, nlat as i64 - 1);
        lt.landtypes_global[(li + 1) as usize][(lj + 1) as usize]
    };

    let is_marker = |p: &LonLatPoint| p.lon == 0.0 && p.lat == 0.0;
    let mut counts = ColmSurfaceCounts::default();
    let mut out = String::from(
        "cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell\n",
    );
    let mut idx = 0i32;
    for (i, c) in centers.iter().enumerate() {
        if i == 0 || is_marker(c) {
            continue;
        }
        idx += 1;
        let surface = match classify_area_judge_landtype_fortran_indexed(sample(c.lon, c.lat)) {
            AreaJudgeLandtypeClass::Ocean => {
                counts.ocean += 1;
                "OCEAN"
            }
            AreaJudgeLandtypeClass::Land => {
                counts.land += 1;
                "LAND"
            }
        };
        // river/coast columns are [需数据] placeholders until hydro assignment.
        out.push_str(&format!(
            "{case_name}_{idx},{idx},{lon:.6},{lat:.6},{surface},false,none,0.0,0.0,false,none,0.0,0.0,0.0\n",
            lon = c.lon,
            lat = c.lat,
        ));
    }
    crate::ensure_parent_dir(output_csv.as_ref())?;
    fs::write(output_csv, out)?;
    Ok(counts)
}
