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
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io;
use std::path::Path;

const MAX_DENSE_LANDTYPE_CELLS: usize = 16 * 1024 * 1024;
const LANDTYPE_FALLBACK_TILE: usize = 1024;
const LANDTYPE_MAX_STORAGE_CHUNK_BYTES: usize = 64 * 1024 * 1024;
const LANDTYPE_TILE_CACHE_BYTES: usize = 256 * 1024 * 1024;
const LANDTYPE_WHOLE_READ_LIMIT_BYTES: usize = 256 * 1024 * 1024;

struct SourceTileCache {
    values: HashMap<(usize, usize), Vec<i8>>,
    order: VecDeque<(usize, usize)>,
    capacity: usize,
}

impl SourceTileCache {
    fn new(capacity: usize) -> Self {
        Self {
            values: HashMap::new(),
            order: VecDeque::new(),
            capacity,
        }
    }

    fn insert(&mut self, key: (usize, usize), value: Vec<i8>) {
        if self.values.contains_key(&key) {
            return;
        }
        if self.values.len() == self.capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.values.remove(&oldest);
            }
        }
        self.values.insert(key, value);
        self.order.push_back(key);
    }
}

/// Reusable landtype point sampler with one frozen NetCDF source handle.
///
/// Small rasters are read once at construction. Large rasters keep the file
/// open, read its real storage chunks, and cache only requested source values.
pub struct FrozenLandtypeSampler {
    file: netcdf::File,
    nlons_source: usize,
    nlats_source: usize,
    lon_lat_order: bool,
    lon0: f64,
    lat0: f64,
    dlon: f64,
    dlat: f64,
    tile_lon: usize,
    tile_lat: usize,
    whole_variable: Option<Vec<i8>>,
    source_tile_cache: RefCell<SourceTileCache>,
    #[cfg(test)]
    tile_read_count: std::cell::Cell<usize>,
}

impl FrozenLandtypeSampler {
    pub fn open(landtype_file: impl AsRef<Path>, gridnum_perdegree: usize) -> io::Result<Self> {
        Self::open_with_whole_read_limit(
            landtype_file,
            gridnum_perdegree,
            LANDTYPE_WHOLE_READ_LIMIT_BYTES,
        )
    }

    fn open_with_whole_read_limit(
        landtype_file: impl AsRef<Path>,
        gridnum_perdegree: usize,
        whole_read_limit_bytes: usize,
    ) -> io::Result<Self> {
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
            build_global_source_axes_one_based(gridnum_perdegree, nlons_source, nlats_source)?;
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
        let source_cell_count = nlons_source.checked_mul(nlats_source).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "landtype source cell count overflows usize",
            )
        })?;

        let (lon_lat_order, whole_variable, tile_lon, tile_lat) = {
            let variable = file.variable("landtype").ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "missing landtype variable")
            })?;
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
            let chunking = variable.chunking().ok().flatten();
            let (tile_lon, tile_lat) = match chunking.as_deref() {
                Some([first, second]) if *first > 0 && *second > 0 => {
                    validate_landtype_storage_chunk(*first, *second)?;
                    if lon_lat_order {
                        (*first, *second)
                    } else {
                        (*second, *first)
                    }
                }
                _ => (
                    LANDTYPE_FALLBACK_TILE.min(nlons_source),
                    LANDTYPE_FALLBACK_TILE.min(nlats_source),
                ),
            };
            let whole_variable = if source_cell_count <= whole_read_limit_bytes {
                Some(
                    variable
                        .get_values::<i8, _>(..)
                        .map_err(netcdf_to_io_error)?,
                )
            } else {
                None
            };
            (lon_lat_order, whole_variable, tile_lon, tile_lat)
        };

        Ok(Self {
            file,
            nlons_source,
            nlats_source,
            lon_lat_order,
            lon0: axes.lon_i[1],
            lat0: axes.lat_i[1],
            dlon: axes.lon_i[2] - axes.lon_i[1],
            dlat: axes.lat_i[2] - axes.lat_i[1],
            tile_lon,
            tile_lat,
            whole_variable,
            source_tile_cache: RefCell::new(SourceTileCache::new(
                (LANDTYPE_TILE_CACHE_BYTES / (tile_lon * tile_lat)).max(1),
            )),
            #[cfg(test)]
            tile_read_count: std::cell::Cell::new(0),
        })
    }

    pub fn sample_values(&self, points: &[LonLatPoint]) -> io::Result<Vec<i32>> {
        if let Some(all_values) = &self.whole_variable {
            return Ok(points
                .iter()
                .map(|point| {
                    let (lon_index, lat_index) = self.source_indices(point);
                    let offset = if self.lon_lat_order {
                        lon_index * self.nlats_source + lat_index
                    } else {
                        lat_index * self.nlons_source + lon_index
                    };
                    i32::from(all_values[offset])
                })
                .collect());
        }
        let mut requests = BTreeMap::<(usize, usize), Vec<(usize, usize, usize)>>::new();
        let mut sampled = vec![0; points.len()];
        for (output_index, point) in points.iter().enumerate() {
            let (lon_index, lat_index) = self.source_indices(point);
            requests
                .entry((lon_index / self.tile_lon, lat_index / self.tile_lat))
                .or_default()
                .push((output_index, lon_index, lat_index));
        }
        for (tile_key, tile_requests) in requests {
            if !self
                .source_tile_cache
                .borrow()
                .values
                .contains_key(&tile_key)
            {
                let tile = self.read_raw_tile(tile_key)?;
                self.source_tile_cache.borrow_mut().insert(tile_key, tile);
            }
            let (lon_start, lon_len) = tile_bounds(tile_key.0, self.nlons_source, self.tile_lon);
            let (lat_start, lat_len) = tile_bounds(tile_key.1, self.nlats_source, self.tile_lat);
            let cache = self.source_tile_cache.borrow();
            let tile = &cache.values[&tile_key];
            for (output_index, lon_index, lat_index) in tile_requests {
                let offset = if self.lon_lat_order {
                    (lon_index - lon_start) * lat_len + (lat_index - lat_start)
                } else {
                    (lat_index - lat_start) * lon_len + (lon_index - lon_start)
                };
                sampled[output_index] = i32::from(tile[offset]);
            }
        }
        Ok(sampled)
    }

    pub fn sample_land_flags(&self, points: &[LonLatPoint]) -> io::Result<Vec<bool>> {
        self.sample_values(points).map(|values| {
            values
                .into_iter()
                .map(|value| {
                    matches!(
                        classify_area_judge_landtype_one_based(value),
                        AreaJudgeLandtypeClass::Land
                    )
                })
                .collect()
        })
    }

    fn source_indices(&self, point: &LonLatPoint) -> (usize, usize) {
        let lon_index = (((point.lon - self.lon0) / self.dlon).round() as i64)
            .rem_euclid(self.nlons_source as i64) as usize;
        let lat_index = (((point.lat - self.lat0) / self.dlat).round() as i64)
            .clamp(0, self.nlats_source as i64 - 1) as usize;
        (lon_index, lat_index)
    }

    fn read_raw_tile(&self, tile_key: (usize, usize)) -> io::Result<Vec<i8>> {
        #[cfg(test)]
        self.tile_read_count.set(self.tile_read_count.get() + 1);
        let (lon_start, lon_len) = tile_bounds(tile_key.0, self.nlons_source, self.tile_lon);
        let (lat_start, lat_len) = tile_bounds(tile_key.1, self.nlats_source, self.tile_lat);
        let variable = self.file.variable("landtype").ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing landtype variable")
        })?;
        if self.lon_lat_order {
            variable
                .get_values::<i8, _>((
                    lon_start..lon_start + lon_len,
                    lat_start..lat_start + lat_len,
                ))
                .map_err(netcdf_to_io_error)
        } else {
            variable
                .get_values::<i8, _>((
                    lat_start..lat_start + lat_len,
                    lon_start..lon_start + lon_len,
                ))
                .map_err(netcdf_to_io_error)
        }
    }
}

fn validate_landtype_storage_chunk(first: usize, second: usize) -> io::Result<()> {
    let chunk_bytes = first
        .checked_mul(second)
        .and_then(|cells| cells.checked_mul(std::mem::size_of::<i8>()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "landtype storage chunk size overflows usize",
            )
        })?;
    if chunk_bytes > LANDTYPE_MAX_STORAGE_CHUNK_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "landtype storage chunk is {chunk_bytes} bytes, above the safe {}-byte limit; rechunk the landtype variable",
                LANDTYPE_MAX_STORAGE_CHUNK_BYTES
            ),
        ));
    }
    Ok(())
}

fn tile_bounds(tile: usize, limit: usize, tile_size: usize) -> (usize, usize) {
    let start = tile * tile_size;
    (start, tile_size.min(limit - start))
}

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
    FrozenLandtypeSampler::open(landtype_file, gridnum_perdegree)?.sample_values(points)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn assert_sparse_chunked_sampler(lon_lat_order: bool) {
        let gridnum_perdegree = 1;
        let nlons = gridnum_perdegree * 360;
        let nlats = gridnum_perdegree * 180;
        let path = std::env::temp_dir().join(format!(
            "earthmesh_land_mask_chunk_cache_{}_{}.nc",
            std::process::id(),
            if lon_lat_order { "lon_lat" } else { "lat_lon" }
        ));
        let _ = fs::remove_file(&path);
        {
            let mut file = crate::create_netcdf_quiet(&path).expect("create landtype fixture");
            file.add_dimension("latitude", nlats).unwrap();
            file.add_dimension("longitude", nlons).unwrap();
            if lon_lat_order {
                let mut landtype = file
                    .add_variable::<i8>("landtype", &["longitude", "latitude"])
                    .unwrap();
                landtype.set_chunking(&[36, 18]).unwrap();
                landtype.put_value(1, (1, 0)).unwrap();
                landtype.put_value(1, (2, 0)).unwrap();
                landtype.put_value(0, (37, 0)).unwrap();
                landtype.put_value(0, (38, 0)).unwrap();
            } else {
                let mut landtype = file
                    .add_variable::<i8>("landtype", &["latitude", "longitude"])
                    .unwrap();
                landtype.set_chunking(&[18, 36]).unwrap();
                landtype.put_value(1, (0, 1)).unwrap();
                landtype.put_value(1, (0, 2)).unwrap();
                landtype.put_value(0, (0, 37)).unwrap();
                landtype.put_value(0, (0, 38)).unwrap();
            }
        }

        let sampler =
            FrozenLandtypeSampler::open_with_whole_read_limit(&path, gridnum_perdegree, 0)
                .expect("open sparse sampler");
        assert_eq!((sampler.tile_lon, sampler.tile_lat), (36, 18));
        assert!(sampler.whole_variable.is_none());
        let land = LonLatPoint {
            lon: sampler.lon0 + sampler.dlon,
            lat: sampler.lat0,
        };
        let ocean = LonLatPoint {
            lon: sampler.lon0 + 37.0 * sampler.dlon,
            lat: sampler.lat0,
        };

        assert_eq!(
            sampler.sample_land_flags(&[land, ocean, land]).unwrap(),
            vec![true, false, true]
        );
        assert_eq!(sampler.source_tile_cache.borrow().values.len(), 2);
        assert_eq!(sampler.source_tile_cache.borrow().order.len(), 2);
        assert_eq!(sampler.tile_read_count.get(), 2);
        let adjacent_land = LonLatPoint {
            lon: sampler.lon0 + 2.0 * sampler.dlon,
            lat: sampler.lat0,
        };
        let adjacent_ocean = LonLatPoint {
            lon: sampler.lon0 + 38.0 * sampler.dlon,
            lat: sampler.lat0,
        };
        assert_eq!(
            sampler
                .sample_land_flags(&[adjacent_ocean, adjacent_land])
                .unwrap(),
            vec![false, true]
        );
        assert_eq!(sampler.source_tile_cache.borrow().values.len(), 2);
        assert_eq!(sampler.source_tile_cache.borrow().order.len(), 2);
        assert_eq!(sampler.tile_read_count.get(), 2);

        drop(sampler);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn land_mask_sampler_reuses_real_chunks_in_lat_lon_order() {
        assert_sparse_chunked_sampler(false);
    }

    #[test]
    fn land_mask_sampler_reuses_real_chunks_in_lon_lat_order() {
        assert_sparse_chunked_sampler(true);
    }

    #[test]
    fn landtype_storage_chunk_size_is_checked_and_bounded() {
        validate_landtype_storage_chunk(2_880, 5_760).unwrap();
        let error = validate_landtype_storage_chunk(8_193, 8_192).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("rechunk"));
        assert!(validate_landtype_storage_chunk(usize::MAX, 2).is_err());
    }

    #[test]
    fn source_tile_cache_is_bounded_fifo_without_duplicate_queue_entries() {
        let mut cache = SourceTileCache::new(2);
        cache.insert((1, 0), vec![4]);
        cache.insert((2, 0), vec![5]);
        cache.insert((1, 0), vec![9]);
        cache.insert((3, 0), vec![6]);

        assert_eq!(cache.values.len(), 2);
        assert_eq!(cache.order, VecDeque::from([(2, 0), (3, 0)]));
        assert_eq!(cache.values.get(&(1, 0)), None);
        assert_eq!(cache.values.get(&(2, 0)), Some(&vec![5]));
        assert_eq!(cache.values.get(&(3, 0)), Some(&vec![6]));
    }
}
