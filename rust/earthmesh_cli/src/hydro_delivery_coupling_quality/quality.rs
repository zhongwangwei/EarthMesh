use std::fs;
use std::io;
use std::path::Path;

use crate::*;

/// R7 coupling-quality validator (`earthmesh_quality::coupling`) fed by the mesh +
/// land-type land/ocean signal.
///
/// `land_fraction[i]` in 0..1 (ocean = 1 - land) and 0-based `neighbors[i]` describe
/// each cell. Cells straddling the coast (0 < land < 1) classify as MixedCoast and drive
/// the coastline / orphan / mass-conservation checks — i.e. a meaningful report, not the
/// all-pure degenerate case. River/estuary signals are absent here (land-type only), so
/// the river-outlet checks stay neutral. Returns the full `CouplingQualityReport`.
pub fn landtype_coupling_quality(
    land_fraction: &[f64],
    neighbors: &[Vec<usize>],
) -> earthmesh_quality::coupling::CouplingQualityReport {
    use earthmesh_quality::coupling::{
        build_coupling_map, build_coupling_quality, classify_all, CoupledCellFractions,
        CoupledCellInput, CoupledThresholds,
    };
    let cells: Vec<CoupledCellInput> = land_fraction
        .iter()
        .enumerate()
        .map(|(i, &lf)| {
            let lf = lf.clamp(0.0, 1.0);
            CoupledCellInput {
                fractions: CoupledCellFractions {
                    land_fraction: lf,
                    ocean_fraction: 1.0 - lf,
                    river_fraction: 0.0,
                    wetland_fraction: 0.0,
                    estuary_fraction: 0.0,
                    source_features: vec!["landtype".to_string()],
                    quality_flags: Vec::new(),
                },
                neighbors: neighbors.get(i).cloned().unwrap_or_default(),
                is_estuary: false,
                is_river_mouth: false,
                outlet_ocean_cell: None,
            }
        })
        .collect();
    let th = CoupledThresholds::default();
    let classes = classify_all(&cells, &th);
    let maps = build_coupling_map(&cells, &classes);
    build_coupling_quality(&cells, &classes, &maps, &th)
}

/// Read an EarthMesh gridfile + a global land-type NetCDF, derive each W cell's land
/// fraction (its centre plus its M-corner points sampled against the land-type grid) and
/// its neighbours (W cells that share an M dual triangle), then run
/// [`landtype_coupling_quality`] and write `coupling_quality.json`. The mesh+land-type
/// counterpart of the hydro coupling QA — closes R7 onto the real coupling signal.
pub fn write_coupling_quality_from_gridfile(
    gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    output_json: impl AsRef<Path>,
) -> io::Result<earthmesh_quality::coupling::CouplingQualityReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
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
    let sample_land = |lon: f64, lat: f64| -> bool {
        let li = (((lon - lon0) / dlon).round() as i64).rem_euclid(nlon as i64);
        let lj = (((lat - lat0) / dlat).round() as i64).clamp(0, nlat as i64 - 1);
        matches!(
            classify_area_judge_landtype_fortran_indexed(
                lt.landtypes_global[(li + 1) as usize][(lj + 1) as usize]
            ),
            AreaJudgeLandtypeClass::Land
        )
    };
    let is_marker = |p: &LonLatPoint| p.lon == 0.0 && p.lat == 0.0;

    let mut dense_of: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut wi_list: Vec<usize> = Vec::new();
    for (wi, c) in mesh.w_points.iter().enumerate() {
        if wi == 0 || is_marker(c) {
            continue;
        }
        dense_of.insert(wi, wi_list.len());
        wi_list.push(wi);
    }

    let mut land_fraction = Vec::with_capacity(wi_list.len());
    for &wi in &wi_list {
        let center = &mesh.w_points[wi];
        let mut land = usize::from(sample_land(center.lon, center.lat));
        let mut total = 1usize;
        if let Some(corners) = mesh.w_to_m.get(wi) {
            for &mi in corners {
                if mi >= 1 && (mi as usize) < mesh.m_points.len() {
                    let p = &mesh.m_points[mi as usize];
                    if is_marker(p) {
                        continue;
                    }
                    land += usize::from(sample_land(p.lon, p.lat));
                    total += 1;
                }
            }
        }
        land_fraction.push(land as f64 / total as f64);
    }

    let mut nbset: Vec<std::collections::BTreeSet<usize>> =
        vec![std::collections::BTreeSet::new(); wi_list.len()];
    for tri in &mesh.m_to_w {
        let dense: Vec<usize> = tri
            .iter()
            .filter_map(|&w| dense_of.get(&(w as usize)).copied())
            .collect();
        for &a in &dense {
            for &b in &dense {
                if a != b {
                    nbset[a].insert(b);
                }
            }
        }
    }
    let neighbors: Vec<Vec<usize>> = nbset.into_iter().map(|s| s.into_iter().collect()).collect();

    let report = landtype_coupling_quality(&land_fraction, &neighbors);
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(
        output_json,
        earthmesh_quality::coupling::to_coupling_quality_json(&report),
    )?;
    Ok(report)
}
