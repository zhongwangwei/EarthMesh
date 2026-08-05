//! Experimental native per-face Landcover refinement retained outside the production HField path.

use std::{collections::BTreeMap, io};

use earthmesh_core::RefineConfig;
use earthmesh_geometry::{
    is_point_in_convex_polygon, shift_longitudes_for_dateline_crossing, Point,
};
use earthmesh_mesh::{xyz_to_lonlat_degrees, MethodCDelaunayMesh};

use crate::{
    getcontain_geometry::{
        getcontain_axis_candidate_range, getcontain_restore_dateline_source_index,
        getcontain_south_pole_scan_polygons,
    },
    global_source_axes::GlobalSourceAxes,
    hfield_refine::{has_land_thresholds, has_ocean_thresholds},
    mkgrd_data_preprocess_source::FrozenLandtypeSampler,
    GridRegion, LonLatPoint,
};

pub(crate) fn native_landcover_face_demands(
    mesh: &MethodCDelaunayMesh,
    sampler: &FrozenLandtypeSampler,
    axes: &GlobalSourceAxes,
    refine: &RefineConfig,
    mesh_type: &str,
    maxlc: i32,
    pass: usize,
    domain: Option<&GridRegion>,
) -> io::Result<Vec<bool>> {
    let global_min_lat = mesh
        .m_points
        .iter()
        .skip(2)
        .map(|point| xyz_to_lonlat_degrees(*point).lat_degrees)
        .fold(f64::INFINITY, f64::min);
    let mut demand = vec![false; mesh.nwd + 1];
    for (iw, demanded) in demand.iter_mut().enumerate().take(mesh.nwd + 1).skip(2) {
        if mesh.w_faces[iw].mrlw != pass {
            continue;
        }
        let polygon = mesh.w_faces[iw]
            .im
            .map(|im| xyz_to_lonlat_degrees(mesh.m_points[im]))
            .map(|point| Point::new(point.lon_degrees, point.lat_degrees));
        let mut source_indices = Vec::new();
        for mut scan_polygon in getcontain_south_pole_scan_polygons(&polygon, global_min_lat) {
            let (raw_min_lon, raw_max_lon) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lon, max_lon), point| (min_lon.min(point.x), max_lon.max(point.x)),
            );
            let crosses_dateline = raw_max_lon - raw_min_lon > 180.0;
            if crosses_dateline {
                scan_polygon = shift_longitudes_for_dateline_crossing(&scan_polygon);
            }
            let (min_lon, max_lon) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lon, max_lon), point| (min_lon.min(point.x), max_lon.max(point.x)),
            );
            let (min_lat, max_lat) = scan_polygon.iter().fold(
                (f64::INFINITY, f64::NEG_INFINITY),
                |(min_lat, max_lat), point| (min_lat.min(point.y), max_lat.max(point.y)),
            );
            let Some(lon_range) = getcontain_axis_candidate_range(&axes.lon_i, min_lon, max_lon)
            else {
                continue;
            };
            let Some(lat_range) = getcontain_axis_candidate_range(&axes.lat_i, min_lat, max_lat)
            else {
                continue;
            };
            for i in lon_range {
                let source_i = if crosses_dateline {
                    getcontain_restore_dateline_source_index(i, axes.nlons_source)?
                } else {
                    i
                };
                for j in lat_range.clone() {
                    if !is_point_in_convex_polygon(
                        &scan_polygon,
                        Point::new(axes.lon_i[i], axes.lat_i[j]),
                    ) {
                        continue;
                    }
                    let lon = axes.lon_i[source_i];
                    let lat = axes.lat_i[j];
                    if domain.is_none_or(|domain| domain.contains(lon, lat)) {
                        source_indices.push((source_i, j));
                    }
                }
            }
        }
        source_indices.sort_unstable();
        source_indices.dedup();
        if source_indices.is_empty() {
            continue;
        }
        let points = source_indices
            .iter()
            .map(|&(i, j)| LonLatPoint {
                lon: axes.lon_i[i],
                lat: axes.lat_i[j],
            })
            .collect::<Vec<_>>();
        let values = sampler.sample_values(&points)?;
        *demanded = landcover_values_require_refinement(&values, refine, mesh_type, maxlc)?;
    }
    Ok(demand)
}

fn landcover_values_require_refinement(
    values: &[i32],
    refine: &RefineConfig,
    mesh_type: &str,
    maxlc: i32,
) -> io::Result<bool> {
    let mut ocean = 0usize;
    let mut land = 0usize;
    let mut class_counts = BTreeMap::<i32, usize>::new();
    for &value in values {
        if value < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("landtype value {value} must be non-negative"),
            ));
        }
        if value == 0 {
            ocean += 1;
        } else {
            land += 1;
            if value != maxlc {
                *class_counts.entry(value).or_default() += 1;
            }
        }
    }
    let land_refines = has_land_thresholds(refine, mesh_type)
        && ((refine.refine_num_landtypes && class_counts.len() as i32 > refine.th_num_landtypes)
            || (refine.refine_area_mainland
                && land > 0
                && class_counts.values().copied().max().unwrap_or(0) as f64 / (land as f64)
                    < refine.th_area_mainland));
    let total = ocean + land;
    let ocean_refines = has_ocean_thresholds(refine, mesh_type) && total > 0 && {
        let ratio = ocean as f64 / total as f64;
        ratio > refine.th_sea_ratio[0] && ratio < refine.th_sea_ratio[1]
    };
    Ok(land_refines || ocean_refines)
}

#[cfg(test)]
mod tests {
    use super::{landcover_values_require_refinement, native_landcover_face_demands};
    use crate::{
        build_global_source_axes_one_based, create_netcdf_quiet,
        mkgrd_data_preprocess_source::FrozenLandtypeSampler,
    };
    use earthmesh_core::RefineConfig;
    use earthmesh_mesh::{voronoi_grid_from_method_c_delaunay_mesh, MethodCDelaunayMesh};
    use std::fs;

    #[test]
    fn native_landcover_thresholds_use_every_source_value() {
        let refine = RefineConfig {
            refine_num_landtypes: true,
            th_num_landtypes: 2,
            ..RefineConfig::default()
        };
        assert!(
            landcover_values_require_refinement(&[1, 2, 3, 3], &refine, "atmosmesh", 9,).unwrap()
        );
        assert!(
            !landcover_values_require_refinement(&[1, 2, 9, 9], &refine, "atmosmesh", 9,).unwrap()
        );
    }

    #[test]
    fn native_landcover_face_demand_materializes_without_raster_downsampling() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh-native-landcover-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("landtype.nc");
        let (nlon, nlat) = (360, 180);
        let mut values = vec![1_i8; nlon * nlat];
        values[295 * nlat + 64] = 2; // 115.5 E, 25.5 N
        values[nlat - 1] = 9; // excluded maxlc sentinel
        let mut file = create_netcdf_quiet(&path).unwrap();
        file.add_dimension("longitude", nlon).unwrap();
        file.add_dimension("latitude", nlat).unwrap();
        file.add_variable::<i8>("landtype", &["longitude", "latitude"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);

        let axes = build_global_source_axes_one_based(1, nlon, nlat).unwrap();
        let sampler = FrozenLandtypeSampler::open(&path, 1).unwrap();
        let mesh = MethodCDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).unwrap();
        let refine = RefineConfig {
            refine_num_landtypes: true,
            th_num_landtypes: 1,
            ..RefineConfig::default()
        };
        let demand =
            native_landcover_face_demands(&mesh, &sampler, &axes, &refine, "atmosmesh", 9, 1, None)
                .unwrap();
        assert!(demand.iter().skip(2).any(|demanded| *demanded));

        let refined = mesh
            .spawn_nest_pass_from_face_demands(
                &demand,
                1,
                2,
                MethodCDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            )
            .unwrap()
            .expect("one native source pixel must drive one Method-C pass");
        refined.validate_topology().unwrap();
        assert!(refined.nwd > mesh.nwd);
        let hex =
            voronoi_grid_from_method_c_delaunay_mesh(&refined, earthmesh_core::EARTH_RADIUS_METERS)
                .unwrap();
        assert_eq!(hex.grid.nma, refined.nwd);
        assert_eq!(hex.grid.nwa, refined.nmd);
        fs::remove_dir_all(root).unwrap();
    }
}
