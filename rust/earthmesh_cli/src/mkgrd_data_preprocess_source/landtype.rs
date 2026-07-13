use crate::build_global_source_axes_one_based;
use crate::build_v3_data_source_descriptor;
use crate::classify_area_judge_landtype_one_based;
use crate::first_existing_dimension_len;
use crate::netcdf_to_io_error;
use crate::required_values_i8_matrix;
use crate::AreaJudgeLandtypeClass;
use crate::LandtypeDataPreprocessReport;
use crate::LonLatPoint;
use crate::V3DataSourceKind;
use earthmesh_mesh::AreaJudgeSourceBounds;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

const MAX_DENSE_LANDTYPE_CELLS: usize = 16 * 1024 * 1024;

/// A longitude-major landtype hyperslab whose bounds retain Canonical global
/// one-based source indices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandtypeWindow {
    pub bounds: AreaJudgeSourceBounds,
    pub nlons: usize,
    pub nlats: usize,
    pub values: Vec<i8>,
}

impl LandtypeWindow {
    pub fn value_at_global(&self, lon_index: usize, lat_index: usize) -> Option<i8> {
        if lon_index < self.bounds.minlon_source
            || lon_index > self.bounds.maxlon_source
            || lat_index < self.bounds.maxlat_source
            || lat_index > self.bounds.minlat_source
        {
            return None;
        }
        let lon_offset = lon_index - self.bounds.minlon_source;
        let lat_offset = lat_index - self.bounds.maxlat_source;
        self.values
            .get(lon_offset * self.nlats + lat_offset)
            .copied()
    }
}

/// Dense compatibility reader matching
/// `MOD_data_preprocess.F90:data_preprocess` one-based arrays.
///
/// Production-resolution global sources must use
/// [`read_landtype_bbox_window_one_based`] or point sampling instead. This
/// function rejects rasters whose dense `Vec<Vec<i32>>` representation could
/// exhaust process memory.
pub fn read_landtype_data_preprocess_one_based(
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
    let dense_cells = nlons_source.checked_mul(nlats_source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "landtype dense source cell count overflows usize",
        )
    })?;
    if dense_cells > MAX_DENSE_LANDTYPE_CELLS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "dense landtype compatibility reader would materialize {dense_cells} source cells; limit is {MAX_DENSE_LANDTYPE_CELLS}; use read_landtype_bbox_window_one_based or point sampling for production-resolution data"
            ),
        ));
    }

    let axes = build_global_source_axes_one_based(gridnum_perdegree, nlons_source, nlats_source)?;
    let file = crate::open_netcdf(landtype_file.as_ref()).map_err(netcdf_to_io_error)?;
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

/// Read one Canonical source-index bounding box without materialising the
/// global raster. Returned values are longitude-major for both NetCDF
/// `[longitude, latitude]` and `[latitude, longitude]` variable orders.
pub fn read_landtype_bbox_window_one_based(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    bounds: AreaJudgeSourceBounds,
) -> io::Result<LandtypeWindow> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive for landtype window reads",
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
    if bounds.minlon_source == 0
        || bounds.maxlat_source == 0
        || bounds.maxlon_source < bounds.minlon_source
        || bounds.minlat_source < bounds.maxlat_source
        || bounds.maxlon_source > nlons_source
        || bounds.minlat_source > nlats_source
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "landtype window bounds lon {}..{} lat {}..{} are outside one-based source dimensions {nlons_source}x{nlats_source}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }

    let file = crate::open_netcdf(landtype_file.as_ref()).map_err(netcdf_to_io_error)?;
    let lon_dim = first_existing_dimension_len(&file, &["lon", "longitude"])?;
    let lat_dim = first_existing_dimension_len(&file, &["lat", "latitude"])?;
    if lon_dim != nlons_source || lat_dim != nlats_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "landtype_file dimensions {lon_dim}x{lat_dim} do not match gridnum_perdegree source dimensions {nlons_source}x{nlats_source}"
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
                "landtype dimensions {:?} do not match expected longitude x latitude",
                dimension_lengths
            ),
        ));
    };

    let start_lon = bounds.minlon_source - 1;
    let start_lat = bounds.maxlat_source - 1;
    let nlons = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats = bounds.minlat_source - bounds.maxlat_source + 1;
    let raw = if lon_lat_order {
        variable
            .get_values::<i8, _>((start_lon..start_lon + nlons, start_lat..start_lat + nlats))
            .map_err(netcdf_to_io_error)?
    } else {
        variable
            .get_values::<i8, _>((start_lat..start_lat + nlats, start_lon..start_lon + nlons))
            .map_err(netcdf_to_io_error)?
    };
    let expected = nlons.checked_mul(nlats).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "landtype window cell count overflows usize",
        )
    })?;
    if raw.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "landtype window returned {} values, expected {expected}",
                raw.len()
            ),
        ));
    }
    let values = if lon_lat_order {
        raw
    } else {
        let mut transposed = vec![0_i8; expected];
        for lat_offset in 0..nlats {
            for lon_offset in 0..nlons {
                transposed[lon_offset * nlats + lat_offset] = raw[lat_offset * nlons + lon_offset];
            }
        }
        transposed
    };

    Ok(LandtypeWindow {
        bounds,
        nlons,
        nlats,
        values,
    })
}

/// Sample `landtype` values at mesh/grid cell centres without materialising the
/// full global source raster.
pub fn sample_landtype_values_for_points_one_based(
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
    let axes = build_global_source_axes_one_based(gridnum_perdegree, nlons_source, nlats_source)?;
    if axes.lon_i.len() < 3 || axes.lat_i.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "land-type axes too short to derive sampling step",
        ));
    }

    let file = crate::open_netcdf(landtype_file.as_ref()).map_err(netcdf_to_io_error)?;
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

    // Grouped block reads instead of one 1-element NetCDF read per sampled cell.
    // Requests are grouped by 1024x1024 tile, each tile is read once and dropped
    // before the next tile, so global point sets do not retain the full raster.
    const LANDTYPE_TILE: usize = 1024;
    // Small variables are cheaper to read once. Larger rasters stay on the
    // grouped tile path, which bounds retained application memory to one tile
    // plus the request/output vectors.
    const LANDTYPE_WHOLE_READ_LIMIT_BYTES: usize = 256 * 1024 * 1024;
    let whole_variable = if nlons_source
        .checked_mul(nlats_source)
        .is_some_and(|total| total <= LANDTYPE_WHOLE_READ_LIMIT_BYTES)
    {
        Some(
            variable
                .get_values::<i8, _>(..)
                .map_err(netcdf_to_io_error)?,
        )
    } else {
        None
    };
    if let Some(all_values) = whole_variable {
        let mut sampled = Vec::with_capacity(points.len());
        for point in points {
            let lon_index = (((point.lon - lon0) / dlon).round() as i64)
                .rem_euclid(nlons_source as i64) as usize;
            let lat_index = (((point.lat - lat0) / dlat).round() as i64)
                .clamp(0, nlats_source as i64 - 1) as usize;
            let offset = if lon_lat_order {
                lon_index * nlats_source + lat_index
            } else {
                lat_index * nlons_source + lon_index
            };
            sampled.push(i32::from(all_values[offset]));
        }
        return Ok(sampled);
    }

    let tile_bounds = |tile: usize, limit: usize| {
        let start = tile * LANDTYPE_TILE;
        (start, LANDTYPE_TILE.min(limit - start))
    };
    let mut requests = BTreeMap::<(usize, usize), Vec<(usize, usize, usize)>>::new();
    for (output_index, point) in points.iter().enumerate() {
        let lon_index =
            (((point.lon - lon0) / dlon).round() as i64).rem_euclid(nlons_source as i64) as usize;
        let lat_index =
            (((point.lat - lat0) / dlat).round() as i64).clamp(0, nlats_source as i64 - 1) as usize;
        let tile_key = (lon_index / LANDTYPE_TILE, lat_index / LANDTYPE_TILE);
        requests
            .entry(tile_key)
            .or_default()
            .push((output_index, lon_index, lat_index));
    }
    let mut sampled = vec![0; points.len()];
    for (tile_key, tile_requests) in requests {
        let (lon_start, lon_len) = tile_bounds(tile_key.0, nlons_source);
        let (lat_start, lat_len) = tile_bounds(tile_key.1, nlats_source);
        let tile = if lon_lat_order {
            variable
                .get_values::<i8, _>((
                    lon_start..lon_start + lon_len,
                    lat_start..lat_start + lat_len,
                ))
                .map_err(netcdf_to_io_error)?
        } else {
            variable
                .get_values::<i8, _>((
                    lat_start..lat_start + lat_len,
                    lon_start..lon_start + lon_len,
                ))
                .map_err(netcdf_to_io_error)?
        };
        for (output_index, lon_index, lat_index) in tile_requests {
            // `get_values` returns C order: the first extent varies slowest.
            let offset = if lon_lat_order {
                (lon_index - lon_start) * lat_len + (lat_index - lat_start)
            } else {
                (lat_index - lat_start) * lon_len + (lon_index - lon_start)
            };
            sampled[output_index] = i32::from(tile[offset]);
        }
    }
    Ok(sampled)
}

/// Sample land-type values at points and return preview/coupling surface class
/// codes: 1=LAND, 2=OCEAN. Coast classes require separate hydro/coast data.
pub fn sample_landtype_surface_class_codes_for_points_one_based(
    landtype_file: impl AsRef<Path>,
    gridnum_perdegree: usize,
    points: &[LonLatPoint],
) -> io::Result<Vec<i8>> {
    sample_landtype_values_for_points_one_based(landtype_file, gridnum_perdegree, points).map(
        |values| {
            values
                .into_iter()
                .map(
                    |value| match classify_area_judge_landtype_one_based(value) {
                        AreaJudgeLandtypeClass::Land => 1,
                        AreaJudgeLandtypeClass::Ocean => 2,
                    },
                )
                .collect()
        },
    )
}
