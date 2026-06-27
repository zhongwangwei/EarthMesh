use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use crate::*;

/// Read the landtype source and construct one-based source-grid arrays like
/// `MOD_data_preprocess.F90:data_preprocess`.
pub fn read_landtype_data_preprocess_fortran_indexed(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
) -> io::Result<LandtypeDataPreprocessReport> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive for data_preprocess",
        ));
    }
    let nlons_source = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 360 overflows usize",
        )
    })?;
    let nlats_source = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 180 overflows usize",
        )
    })?;

    let axes =
        build_global_source_axes_fortran_indexed(gridnum_perdegree, nlons_source, nlats_source)?;
    let file = netcdf::open(landtype_file.as_ref()).map_err(netcdf_to_io_error)?;
    let lon_dim = first_existing_dimension_len(&file, &["lon", "longitude"])?;
    let lat_dim = first_existing_dimension_len(&file, &["lat", "latitude"])?;
    if lon_dim != nlons_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "nlons_source from landtype_file {lon_dim} != gridnum_perdegree * 360 {nlons_source}"
            ),
        ));
    }
    if lat_dim != nlats_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "nlats_source from landtype_file {lat_dim} != gridnum_perdegree * 180 {nlats_source}"
            ),
        ));
    }
    let values = required_values_i8_matrix(
        &file,
        "landtype",
        "landtype_file longitude",
        "landtype_file latitude",
        nlons_source,
        nlats_source,
    )?;
    let mut landtypes_global = vec![vec![0_i32; nlats_source + 1]; nlons_source + 1];
    let mut maxlc = 0_i32;
    for lon_offset in 0..nlons_source {
        for lat_offset in 0..nlats_source {
            let value = i32::from(values[lon_offset * nlats_source + lat_offset]);
            landtypes_global[lon_offset + 1][lat_offset + 1] = value;
            maxlc = maxlc.max(value);
        }
    }

    Ok(LandtypeDataPreprocessReport {
        source: build_v3_data_source_descriptor(V3DataSourceKind::Landtype, landtype_file)?,
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        lon_i: axes.lon_i,
        lat_i: axes.lat_i,
        lon_vertex: axes.lon_vertex,
        lat_vertex: axes.lat_vertex,
        landtypes_global,
        maxlc,
    })
}

/// Sample `landtype` values at mesh/grid cell centres without materialising the
/// full global source raster.
pub fn sample_landtype_values_for_points_fortran_indexed(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    points: &[LonLatPoint],
) -> io::Result<Vec<i32>> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive for landtype sampling",
        ));
    }
    let nlons_source = gridnum_perdegree.checked_mul(360).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 360 overflows usize",
        )
    })?;
    let nlats_source = gridnum_perdegree.checked_mul(180).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree * 180 overflows usize",
        )
    })?;
    let axes =
        build_global_source_axes_fortran_indexed(gridnum_perdegree, nlons_source, nlats_source)?;
    if axes.lon_i.len() < 3 || axes.lat_i.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "land-type axes too short to derive sampling step",
        ));
    }

    let file = netcdf::open(landtype_file.as_ref()).map_err(netcdf_to_io_error)?;
    let lon_dim = first_existing_dimension_len(&file, &["lon", "longitude"])?;
    let lat_dim = first_existing_dimension_len(&file, &["lat", "latitude"])?;
    if lon_dim != nlons_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "nlons_source from landtype_file {lon_dim} != gridnum_perdegree * 360 {nlons_source}"
            ),
        ));
    }
    if lat_dim != nlats_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "nlats_source from landtype_file {lat_dim} != gridnum_perdegree * 180 {nlats_source}"
            ),
        ));
    }

    let variable = file
        .variable("landtype")
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing landtype variable"))?;
    let dimensions = variable.dimensions();
    if dimensions.len() != 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "landtype must be 2-D over longitude and latitude",
        ));
    }
    let dimension_names = dimensions
        .iter()
        .map(|dimension| dimension.name())
        .collect::<Vec<_>>();
    let dimension_lengths = dimensions
        .iter()
        .map(|dimension| dimension.len())
        .collect::<Vec<_>>();
    let lon_lat_order = if dimension_lengths == [nlons_source, nlats_source] {
        true
    } else if dimension_lengths == [nlats_source, nlons_source] {
        false
    } else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "landtype dimensions {:?} with lengths {:?} do not match expected longitude x latitude",
                dimension_names, dimension_lengths
            ),
        ));
    };

    let lon0 = axes.lon_i[1];
    let lat0 = axes.lat_i[1];
    let dlon = axes.lon_i[2] - axes.lon_i[1];
    let dlat = axes.lat_i[2] - axes.lat_i[1];
    let mut cache = BTreeMap::<(usize, usize), i32>::new();
    let mut sampled = Vec::with_capacity(points.len());
    for point in points {
        let lon_index =
            (((point.lon - lon0) / dlon).round() as i64).rem_euclid(nlons_source as i64) as usize;
        let lat_index =
            (((point.lat - lat0) / dlat).round() as i64).clamp(0, nlats_source as i64 - 1) as usize;
        let value = if let Some(value) = cache.get(&(lon_index, lat_index)) {
            *value
        } else {
            let raw = if lon_lat_order {
                variable
                    .get_value::<i8, _>((lon_index, lat_index))
                    .map_err(netcdf_to_io_error)?
            } else {
                variable
                    .get_value::<i8, _>((lat_index, lon_index))
                    .map_err(netcdf_to_io_error)?
            };
            let value = i32::from(raw);
            cache.insert((lon_index, lat_index), value);
            value
        };
        sampled.push(value);
    }
    Ok(sampled)
}

/// Sample land-type values at points and return preview/coupling surface class
/// codes: 1=LAND, 2=OCEAN. Coast classes require separate hydro/coast data.
pub fn sample_landtype_surface_class_codes_for_points_fortran_indexed(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    points: &[LonLatPoint],
) -> io::Result<Vec<i8>> {
    sample_landtype_values_for_points_fortran_indexed(landtype_file, gridnum_perdegree, points).map(
        |values| {
            values
                .into_iter()
                .map(
                    |value| match classify_area_judge_landtype_fortran_indexed(value) {
                        AreaJudgeLandtypeClass::Land => 1,
                        AreaJudgeLandtypeClass::Ocean => 2,
                    },
                )
                .collect()
        },
    )
}
