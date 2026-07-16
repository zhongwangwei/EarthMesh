use crate::cama_binary_io::{CamaLonLatBbox, CamaReachClassificationThresholds};
use crate::cama_reach_inventory::{
    classify_cama_reach_record, read_cama_reach_inventory_from_map_dir,
};
use crate::unstructured_mesh_support::mesh_points_have_two_placeholder_rows;
use crate::{
    classify_area_judge_landtype_one_based, read_unstructured_mesh_netcdf,
    sample_landtype_values_for_points_one_based, AreaJudgeLandtypeClass, ColmSurfaceCounts,
    LonLatPoint, UnstructuredMesh,
};
use earthmesh_geometry::{haversine_km, spherical_polygon_area_km2, Point};
use std::{fs, io, path::Path};

#[derive(Debug, Clone, Copy)]
pub struct CouplingCsvOptions<'a> {
    pub fraction_method: &'a str,
    pub identify_coastline: bool,
    pub identify_river_mouth: bool,
    pub cama_root: Option<&'a Path>,
    pub target_dx_km: f64,
}

impl Default for CouplingCsvOptions<'_> {
    fn default() -> Self {
        Self {
            fraction_method: "point_sample",
            identify_coastline: false,
            identify_river_mouth: false,
            cama_root: None,
            target_dx_km: 100.0,
        }
    }
}

#[derive(Debug, Clone)]
struct RiverMouthSignal {
    lon: f64,
    lat: f64,
    class: String,
    fraction: f64,
    area_m2: f64,
}

/// CoLM's coupling contract defines `coastal_fraction` as the ocean share of a
/// mixed cell, so `land_fraction = 1 - coastal_fraction` remains valid for both
/// land-dominant and ocean-dominant coast cells.
fn colm_coastal_fraction(has_coast: bool, ocean_fraction: f64) -> f64 {
    if has_coast {
        ocean_fraction
    } else {
        0.0
    }
}

pub fn write_colm_coupling_csv_from_mesh(
    gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    case_name: &str,
    mode_grid: &str,
    output_csv: impl AsRef<Path>,
) -> io::Result<ColmSurfaceCounts> {
    write_colm_coupling_csv_from_mesh_with_options(
        gridfile,
        landtype_file,
        gridnum_perdegree,
        case_name,
        mode_grid,
        output_csv,
        CouplingCsvOptions::default(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn write_colm_coupling_csv_from_mesh_with_options(
    gridfile: impl AsRef<Path>,
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    case_name: &str,
    mode_grid: &str,
    output_csv: impl AsRef<Path>,
    options: CouplingCsvOptions<'_>,
) -> io::Result<ColmSurfaceCounts> {
    if !matches!(
        options.fraction_method,
        "point_sample" | "conservative_overlay"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "coupling_fraction_method must be point_sample or conservative_overlay, got {}",
                options.fraction_method
            ),
        ));
    }
    if options.identify_river_mouth && options.cama_root.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "river-mouth coupling requires coupling_cama_root",
        ));
    }

    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let cells = coupling_cells(&mesh, mode_grid)?;
    let landtype_file = landtype_file.as_ref();
    let cell_center_values = sample_landtype_values_for_points_one_based(
        landtype_file,
        gridnum_perdegree,
        &cells.iter().map(|cell| cell.center).collect::<Vec<_>>(),
    )?;
    let dlon = 1.0 / gridnum_perdegree as f64;
    let dlat = dlon;

    let mouths = if options.identify_river_mouth {
        load_river_mouth_signals(
            options.cama_root.expect("validated CaMa root"),
            &cells,
            options.target_dx_km,
        )?
    } else {
        Vec::new()
    };

    let mut counts = ColmSurfaceCounts::default();
    let mut out = String::from(
        "cell_id,cell_index,center_lon,center_lat,surface_class,has_river,river_class,river_fraction,estimated_river_area_m2,has_coast,coast_class,coastal_fraction,normalized_cell_area_m2,source_areaCell\n",
    );
    for (dense_index, cell) in cells.iter().enumerate() {
        let land_fraction = if options.fraction_method == "conservative_overlay" {
            conservative_land_fraction(cell.center, &cell.vertices, dlon, dlat, &|points| {
                sample_landtype_values_for_points_one_based(
                    landtype_file,
                    gridnum_perdegree,
                    points,
                )
            })?
        } else {
            f64::from(matches!(
                classify_area_judge_landtype_one_based(cell_center_values[dense_index]),
                AreaJudgeLandtypeClass::Land
            ))
        };
        let ocean_fraction = 1.0 - land_fraction;
        let mixed = land_fraction > 1.0e-9 && ocean_fraction > 1.0e-9;
        let has_coast = options.identify_coastline && mixed;
        let surface = if has_coast {
            counts.coast += 1;
            "COAST"
        } else if land_fraction >= ocean_fraction {
            counts.land += 1;
            "LAND"
        } else {
            counts.ocean += 1;
            "OCEAN"
        };
        let mouth = mouths
            .iter()
            .filter_map(|signal| {
                let distance = haversine_km(
                    Point::new(cell.center.lon, cell.center.lat),
                    Point::new(signal.lon, signal.lat),
                );
                (distance <= options.target_dx_km.max(5.0) * 1.5).then_some((distance, signal))
            })
            .min_by(|left, right| left.0.total_cmp(&right.0));
        let (has_river, river_class, river_fraction, river_area) = match mouth {
            Some((_, signal)) => (true, signal.class.as_str(), signal.fraction, signal.area_m2),
            None => (false, "none", 0.0, 0.0),
        };
        let cell_area_m2 = spherical_polygon_area_km2(
            &cell
                .vertices
                .iter()
                .map(|point| Point::new(point.lon, point.lat))
                .collect::<Vec<_>>(),
        ) * 1.0e6;
        let cell_index = dense_index + 1;
        out.push_str(&format!(
            "{case_name}_{cell_index},{cell_index},{lon:.6},{lat:.6},{surface},{has_river},{river_class},{river_fraction:.10},{river_area:.6},{has_coast},{coast_class},{coastal_fraction:.10},{cell_area_m2:.6},{cell_area_m2:.6}\n",
            lon = cell.center.lon,
            lat = cell.center.lat,
            coast_class = if has_coast { "COAST" } else { "none" },
            coastal_fraction = colm_coastal_fraction(has_coast, ocean_fraction),
        ));
    }
    crate::ensure_parent_dir(output_csv.as_ref())?;
    fs::write(output_csv, out)?;
    Ok(counts)
}

#[derive(Debug, Clone)]
struct CouplingCell {
    center: LonLatPoint,
    vertices: Vec<LonLatPoint>,
}

fn coupling_cells(mesh: &UnstructuredMesh, mode_grid: &str) -> io::Result<Vec<CouplingCell>> {
    let m_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let w_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let mut cells = Vec::new();
    match mode_grid.trim() {
        "tri" => {
            for (index, center) in mesh.m_points.iter().copied().enumerate() {
                if index == 0 {
                    continue;
                }
                if crate::unstructured_mesh_support::mesh_canonical_id_for_row(
                    index,
                    m_has_two_placeholders,
                )
                .is_none()
                {
                    continue;
                }
                let Some(ids) = mesh.m_to_w.get(index) else {
                    continue;
                };
                let vertices = ids
                    .iter()
                    .filter_map(|&id| {
                        crate::unstructured_mesh_support::mesh_row_for_canonical_id(
                            id,
                            mesh.w_points.len(),
                            w_has_two_placeholders,
                        )
                    })
                    .map(|row| mesh.w_points[row])
                    .collect::<Vec<_>>();
                if vertices.len() >= 3 {
                    cells.push(CouplingCell { center, vertices });
                }
            }
        }
        "hex" => {
            for (index, center) in mesh.w_points.iter().copied().enumerate() {
                if index == 0 {
                    continue;
                }
                if crate::unstructured_mesh_support::mesh_canonical_id_for_row(
                    index,
                    w_has_two_placeholders,
                )
                .is_none()
                {
                    continue;
                }
                let Some(ids) = mesh.w_to_m.get(index) else {
                    continue;
                };
                let count = mesh
                    .n_w_to_m
                    .get(index)
                    .and_then(|count| usize::try_from(*count).ok())
                    .unwrap_or(ids.len())
                    .min(ids.len());
                let vertices = ids[..count]
                    .iter()
                    .filter_map(|&id| {
                        crate::unstructured_mesh_support::mesh_row_for_canonical_id(
                            id,
                            mesh.m_points.len(),
                            m_has_two_placeholders,
                        )
                    })
                    .map(|row| mesh.m_points[row])
                    .collect::<Vec<_>>();
                if vertices.len() >= 3 {
                    cells.push(CouplingCell { center, vertices });
                }
            }
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("coupling CSV requires tri or hex mode_grid, got {other}"),
            ));
        }
    }
    Ok(cells)
}

fn conservative_land_fraction(
    center: LonLatPoint,
    vertices: &[LonLatPoint],
    dlon: f64,
    dlat: f64,
    sample_landtypes: &impl Fn(&[LonLatPoint]) -> io::Result<Vec<i32>>,
) -> io::Result<f64> {
    let polygon = vertices
        .iter()
        .map(|point| Point::new(unwrap_lon(point.lon, center.lon), point.lat))
        .collect::<Vec<_>>();
    let min_lon = polygon
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_lon = polygon
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_lat = polygon
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_lat = polygon
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    // ponytail: bounded 64×64 raster quadrature; replace with spatial-indexed
    // exact pixel clipping only if coupling benchmarks require sub-pixel coasts.
    let lon_step = dlon.abs().max((max_lon - min_lon) / 64.0).max(1.0e-6);
    let lat_step = dlat.abs().max((max_lat - min_lat) / 64.0).max(1.0e-6);
    let mut samples = Vec::new();
    let mut weights = Vec::new();
    let mut lat = min_lat + lat_step * 0.5;
    while lat <= max_lat {
        let mut lon = min_lon + lon_step * 0.5;
        while lon <= max_lon {
            if point_in_polygon(&polygon, Point::new(lon, lat)) {
                let weight = lat.to_radians().cos().abs();
                samples.push(LonLatPoint {
                    lon: normalize_lon(lon),
                    lat,
                });
                weights.push(weight);
            }
            lon += lon_step;
        }
        lat += lat_step;
    }
    if samples.is_empty() {
        samples.push(center);
        weights.push(1.0);
    }
    let values = sample_landtypes(&samples)?;
    let mut land_weight = 0.0;
    let mut total_weight = 0.0;
    for (value, weight) in values.into_iter().zip(weights) {
        total_weight += weight;
        if matches!(
            classify_area_judge_landtype_one_based(value),
            AreaJudgeLandtypeClass::Land
        ) {
            land_weight += weight;
        }
    }
    if total_weight == 0.0 {
        Ok(0.0)
    } else {
        Ok((land_weight / total_weight).clamp(0.0, 1.0))
    }
}

fn point_in_polygon(polygon: &[Point], point: Point) -> bool {
    let mut inside = false;
    for index in 0..polygon.len() {
        let a = polygon[index];
        let b = polygon[(index + 1) % polygon.len()];
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}

fn unwrap_lon(lon: f64, center: f64) -> f64 {
    let mut value = lon;
    while value - center > 180.0 {
        value -= 360.0;
    }
    while value - center < -180.0 {
        value += 360.0;
    }
    value
}

fn normalize_lon(lon: f64) -> f64 {
    (lon + 180.0).rem_euclid(360.0) - 180.0
}

fn load_river_mouth_signals(
    cama_root: &Path,
    cells: &[CouplingCell],
    target_dx_km: f64,
) -> io::Result<Vec<RiverMouthSignal>> {
    let bbox = CamaLonLatBbox {
        west: cells
            .iter()
            .map(|cell| cell.center.lon)
            .fold(180.0, f64::min),
        east: cells
            .iter()
            .map(|cell| cell.center.lon)
            .fold(-180.0, f64::max),
        south: cells
            .iter()
            .map(|cell| cell.center.lat)
            .fold(90.0, f64::min),
        north: cells
            .iter()
            .map(|cell| cell.center.lat)
            .fold(-90.0, f64::max),
    };
    let inventory =
        read_cama_reach_inventory_from_map_dir(cama_root, bbox, target_dx_km, 1.0e-6, true)?;
    let thresholds = CamaReachClassificationThresholds::default();
    inventory
        .records
        .iter()
        .filter(|record| record.is_estuary)
        .map(|record| {
            let class = classify_cama_reach_record(record, thresholds)?;
            let area_m2 = class.effective_width_m * record.river_length_m.max(0.0);
            let cell_scale_m2 = (target_dx_km * 1000.0).powi(2).max(1.0);
            Ok(RiverMouthSignal {
                lon: record.lon,
                lat: record.lat,
                class: class.river_class,
                fraction: (area_m2 / cell_scale_m2).clamp(0.0, 1.0),
                area_m2,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{colm_coastal_fraction, conservative_land_fraction, point_in_polygon};
    use crate::LonLatPoint;
    use earthmesh_geometry::Point;

    #[test]
    fn conservative_overlay_resolves_a_half_land_cell_and_conserves_fraction() {
        let vertices = vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 2.0, lat: 0.0 },
            LonLatPoint { lon: 2.0, lat: 2.0 },
            LonLatPoint { lon: 0.0, lat: 2.0 },
        ];
        let land = conservative_land_fraction(
            LonLatPoint { lon: 1.0, lat: 1.0 },
            &vertices,
            0.05,
            0.05,
            &|points| {
                Ok(points
                    .iter()
                    .map(|point| if point.lon < 1.0 { 1 } else { 0 })
                    .collect())
            },
        )
        .unwrap();
        assert!((land - 0.5).abs() < 0.03, "land fraction {land}");
        assert!(((land + (1.0 - land)) - 1.0).abs() < f64::EPSILON);
        assert!(point_in_polygon(
            &[
                Point::new(0.0, 0.0),
                Point::new(2.0, 0.0),
                Point::new(2.0, 2.0),
                Point::new(0.0, 2.0)
            ],
            Point::new(1.0, 1.0)
        ));
    }

    #[test]
    fn colm_coastal_fraction_preserves_land_fraction_for_either_dominance() {
        let land_dominant_ocean_fraction = colm_coastal_fraction(true, 0.2);
        let ocean_dominant_ocean_fraction = colm_coastal_fraction(true, 0.8);

        assert!((1.0 - land_dominant_ocean_fraction - 0.8).abs() < f64::EPSILON);
        assert!((1.0 - ocean_dominant_ocean_fraction - 0.2).abs() < f64::EPSILON);
        assert_eq!(colm_coastal_fraction(false, 0.8), 0.0);
    }
}
