//! A data layer's pixels inside a longitude-latitude window, for CMRC's
//! merge criteria (design H3, `docs/certified_mesh/heterogeneity_merge.md`):
//! only the window is read, at the file's own resolution, and missing values
//! are skipped. A layer is a NetCDF file with one-dimensional latitude and
//! longitude coordinates, a global lattice without them (rows north to
//! south, `nlon / 360` per degree, as `dem.nc` is laid out), or a directory
//! of 5-degree tiles named like MERIT-Hydro's (`n20e100.nc` holds latitudes
//! 20 to 25 north, longitudes 100 to 105 east).

use std::io;
use std::path::Path;

use crate::area_judge_threshold_inputs::numeric_missing_values;

/// A window of longitudes `west..=east` (degrees, `west < east`; a window
/// across the dateline is two windows) and latitudes `south..=north`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatWindow {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// The pixel centres and values of `variable` inside `window`: longitude,
/// latitude, value.
pub fn read_window_samples(
    path: &Path,
    variable: &str,
    window: LonLatWindow,
) -> io::Result<Vec<(f64, f64, f64)>> {
    if !(window.west < window.east && window.south <= window.north) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("window {window:?} is empty or crosses the dateline"),
        ));
    }
    if !path.is_dir() {
        return read_file_window(path, variable, window);
    }
    let mut samples = Vec::new();
    let tile = |degrees: f64| (degrees / 5.0).floor() as i32 * 5;
    for lat0 in (tile(window.south)..=tile(window.north.min(89.999))).step_by(5) {
        for lon0 in (tile(window.west)..=tile(window.east.min(179.999))).step_by(5) {
            let name = format!(
                "{}{:02}{}{:03}.nc",
                if lat0 >= 0 { 'n' } else { 's' },
                lat0.abs(),
                if lon0 >= 0 { 'e' } else { 'w' },
                lon0.abs()
            );
            let file = path.join(name);
            // Ocean tiles are absent from MERIT-Hydro.
            if !file.is_file() {
                continue;
            }
            let clipped = LonLatWindow {
                west: window.west.max(lon0 as f64),
                east: window.east.min(lon0 as f64 + 5.0),
                south: window.south.max(lat0 as f64),
                north: window.north.min(lat0 as f64 + 5.0),
            };
            samples.extend(read_file_window(&file, variable, clipped)?);
        }
    }
    Ok(samples)
}

/// The contiguous indices of `values` within `low..=high`.
fn within(values: &[f64], low: f64, high: f64) -> Option<(usize, usize)> {
    let first = values
        .iter()
        .position(|&value| value >= low && value <= high)?;
    let count = values[first..]
        .iter()
        .take_while(|&&value| value >= low && value <= high)
        .count();
    Some((first, count))
}

fn coordinate(file: &netcdf::File, names: &[&str]) -> io::Result<Option<(String, Vec<f64>)>> {
    for name in names {
        if let Some(variable) = file.variable(name) {
            if variable.dimensions().len() == 1 {
                let values = variable
                    .get_values::<f64, _>(..)
                    .or_else(|_| {
                        variable
                            .get_values::<f32, _>(..)
                            .map(|values| values.into_iter().map(f64::from).collect())
                    })
                    .map_err(crate::netcdf_to_io_error)?;
                let dimension = variable.dimensions()[0].name();
                return Ok(Some((dimension, values)));
            }
        }
    }
    Ok(None)
}

fn read_values(
    variable: &netcdf::Variable<'_>,
    start: [usize; 2],
    count: [usize; 2],
) -> io::Result<Vec<f64>> {
    let extents = (start, count);
    if let Ok(values) = variable.get_values::<f64, _>(extents) {
        return Ok(values);
    }
    if let Ok(values) = variable.get_values::<f32, _>(extents) {
        return Ok(values.into_iter().map(f64::from).collect());
    }
    if let Ok(values) = variable.get_values::<i32, _>(extents) {
        return Ok(values.into_iter().map(f64::from).collect());
    }
    if let Ok(values) = variable.get_values::<i16, _>(extents) {
        return Ok(values.into_iter().map(f64::from).collect());
    }
    if let Ok(values) = variable.get_values::<i8, _>(extents) {
        return Ok(values.into_iter().map(f64::from).collect());
    }
    variable
        .get_values::<u8, _>(extents)
        .map(|values| values.into_iter().map(f64::from).collect())
        .map_err(crate::netcdf_to_io_error)
}

fn read_file_window(
    path: &Path,
    variable_name: &str,
    window: LonLatWindow,
) -> io::Result<Vec<(f64, f64, f64)>> {
    let file = crate::open_netcdf(path).map_err(crate::netcdf_to_io_error)?;
    let variable = file.variable(variable_name).ok_or_else(|| {
        invalid(format!(
            "{} has no variable {variable_name}",
            path.display()
        ))
    })?;
    let dimensions = variable
        .dimensions()
        .iter()
        .map(|dimension| (dimension.name(), dimension.len()))
        .collect::<Vec<_>>();
    if dimensions.len() != 2 {
        return Err(invalid(format!(
            "{variable_name} in {} is not two-dimensional",
            path.display()
        )));
    }
    // Pixel centres along each axis, and which dimension is latitude.
    let (lats, lons, latitude_first) = match (
        coordinate(&file, &["lat", "latitude"])?,
        coordinate(&file, &["lon", "longitude"])?,
    ) {
        (Some((lat_dim, lats)), Some((lon_dim, lons))) => {
            let latitude_first = if dimensions[0].0 == lat_dim && dimensions[1].0 == lon_dim {
                true
            } else if dimensions[0].0 == lon_dim && dimensions[1].0 == lat_dim {
                false
            } else {
                return Err(invalid(format!(
                    "{variable_name} in {} is not laid out on its coordinates",
                    path.display()
                )));
            };
            (lats, lons, latitude_first)
        }
        _ => {
            // A global lattice: latitude rows first, north to south.
            let (nlat, nlon) = (dimensions[0].1, dimensions[1].1);
            if nlon != 2 * nlat || !nlon.is_multiple_of(360) {
                return Err(invalid(format!(
                    "{} has no coordinates and is not a global {nlon}x{nlat} lattice",
                    path.display()
                )));
            }
            let per_degree = (nlon / 360) as f64;
            let lats = (0..nlat)
                .map(|row| 90.0 - (row as f64 + 0.5) / per_degree)
                .collect::<Vec<_>>();
            let lons = (0..nlon)
                .map(|column| -180.0 + (column as f64 + 0.5) / per_degree)
                .collect::<Vec<_>>();
            (lats, lons, true)
        }
    };
    // Longitudes from 0 to 360: the window in that convention, split at 0.
    let pieces = if lons.iter().all(|&lon| lon <= 180.0) || window.west >= 0.0 {
        vec![(window.west, window.east)]
    } else if window.east <= 0.0 {
        vec![(window.west + 360.0, window.east + 360.0)]
    } else {
        vec![(window.west + 360.0, 360.0), (0.0, window.east)]
    };
    let missing = numeric_missing_values(&variable)?;
    let mut samples = Vec::new();
    for (west, east) in pieces {
        let (Some((lat_start, lat_count)), Some((lon_start, lon_count))) = (
            within(&lats, window.south, window.north),
            within(&lons, west, east),
        ) else {
            continue;
        };
        let (start, count) = if latitude_first {
            ([lat_start, lon_start], [lat_count, lon_count])
        } else {
            ([lon_start, lat_start], [lon_count, lat_count])
        };
        let values = read_values(&variable, start, count)?;
        samples.reserve(values.len());
        for (index, value) in values.into_iter().enumerate() {
            if !value.is_finite() || value.abs() >= 1.0e30 || missing.contains(&value) {
                continue;
            }
            let (row, column) = (index / count[1], index % count[1]);
            let (lat, lon) = if latitude_first {
                (lats[lat_start + row], lons[lon_start + column])
            } else {
                (lats[lat_start + column], lons[lon_start + row])
            };
            samples.push((if lon > 180.0 { lon - 360.0 } else { lon }, lat, value));
        }
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_merge_samples_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        let _ = std::fs::remove_file(&path);
        path
    }

    /// A small file with coordinates, laid out (lon, lat) as MERIT tiles
    /// are, with a fill value: the window's pixels come back with their
    /// centres, the fills left out.
    #[test]
    fn a_coordinate_file_is_read_in_its_window() {
        let directory = temp("tiles");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("n20e100.nc");
        let (nlon, nlat) = (50usize, 40usize);
        let mut file = crate::create_netcdf(&path).unwrap();
        file.add_dimension("longitude", nlon).unwrap();
        file.add_dimension("latitude", nlat).unwrap();
        let lons = (0..nlon)
            .map(|i| 100.0 + (i as f64 + 0.5) * 0.1)
            .collect::<Vec<_>>();
        let lats = (0..nlat)
            .map(|j| 25.0 - (j as f64 + 0.5) * 0.125)
            .collect::<Vec<_>>();
        file.add_variable::<f64>("longitude", &["longitude"])
            .unwrap()
            .put_values(&lons, ..)
            .unwrap();
        file.add_variable::<f64>("latitude", &["latitude"])
            .unwrap()
            .put_values(&lats, ..)
            .unwrap();
        let mut values = vec![0f32; nlon * nlat];
        for i in 0..nlon {
            for j in 0..nlat {
                values[i * nlat + j] = if (i + j) % 7 == 0 {
                    -9999.0
                } else {
                    (i * 100 + j) as f32
                };
            }
        }
        let mut elevation = file
            .add_variable::<f32>("elv", &["longitude", "latitude"])
            .unwrap();
        elevation.set_fill_value(-9999.0f32).unwrap();
        elevation.put_values(&values, (.., ..)).unwrap();
        drop(file);

        let window = LonLatWindow {
            west: 101.0,
            east: 102.0,
            south: 22.0,
            north: 23.0,
        };
        for source in [path.clone(), directory.clone()] {
            let samples = read_window_samples(&source, "elv", window).unwrap();
            let mut expected = Vec::new();
            for (i, &lon) in lons.iter().enumerate() {
                for (j, &lat) in lats.iter().enumerate() {
                    if (101.0..=102.0).contains(&lon)
                        && (22.0..=23.0).contains(&lat)
                        && (i + j) % 7 != 0
                    {
                        expected.push((lon, lat, (i * 100 + j) as f64));
                    }
                }
            }
            let mut got = samples.clone();
            got.sort_by(|a, b| a.partial_cmp(b).unwrap());
            expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(got, expected, "{}", source.display());
        }
        std::fs::remove_dir_all(&directory).unwrap();
    }

    /// Longitudes from 0 to 360: a window across Greenwich is read from
    /// both ends of the file and given back from -180 to 180.
    #[test]
    fn a_file_from_0_to_360_is_read_across_greenwich() {
        let path = temp("east.nc");
        let (nlon, nlat) = (360usize, 4usize);
        let mut file = crate::create_netcdf(&path).unwrap();
        file.add_dimension("lat", nlat).unwrap();
        file.add_dimension("lon", nlon).unwrap();
        let lons = (0..nlon).map(|i| i as f64 + 0.5).collect::<Vec<_>>();
        let lats = (0..nlat).map(|j| -1.5 + j as f64).collect::<Vec<_>>();
        file.add_variable::<f64>("lon", &["lon"])
            .unwrap()
            .put_values(&lons, ..)
            .unwrap();
        file.add_variable::<f64>("lat", &["lat"])
            .unwrap()
            .put_values(&lats, ..)
            .unwrap();
        let values = (0..nlat * nlon)
            .map(|index| (index % nlon) as i32)
            .collect::<Vec<_>>();
        file.add_variable::<i32>("class", &["lat", "lon"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);
        let window = |west, east| LonLatWindow {
            west,
            east,
            south: -1.0,
            north: 1.0,
        };
        let mut got = read_window_samples(&path, "class", window(-2.0, 2.0)).unwrap();
        got.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            got,
            vec![
                (-1.5, -0.5, 358.0),
                (-1.5, 0.5, 358.0),
                (-0.5, -0.5, 359.0),
                (-0.5, 0.5, 359.0),
                (0.5, -0.5, 0.0),
                (0.5, 0.5, 0.0),
                (1.5, -0.5, 1.0),
                (1.5, 0.5, 1.0),
            ]
        );
        let west = read_window_samples(&path, "class", window(-11.0, -9.0)).unwrap();
        assert_eq!(west.len(), 4);
        assert!(west
            .iter()
            .all(|&(lon, _, value)| lon < 0.0 && value == lon + 360.0 - 0.5));
        std::fs::remove_file(&path).unwrap();
    }

    /// A global lattice without coordinates -- rows north to south, as
    /// `dem.nc` -- is read by its implied pixel centres.
    #[test]
    fn a_global_lattice_is_read_in_its_window() {
        let path = temp("global.nc");
        let (nlon, nlat) = (720usize, 360usize);
        let mut file = crate::create_netcdf(&path).unwrap();
        file.add_dimension("nlat", nlat).unwrap();
        file.add_dimension("nlon", nlon).unwrap();
        let values = (0..nlat * nlon)
            .map(|index| index as f32)
            .collect::<Vec<_>>();
        file.add_variable::<f32>("topo", &["nlat", "nlon"])
            .unwrap()
            .put_values(&values, (.., ..))
            .unwrap();
        drop(file);
        let samples = read_window_samples(
            &path,
            "topo",
            LonLatWindow {
                west: 10.0,
                east: 11.0,
                south: 45.0,
                north: 46.0,
            },
        )
        .unwrap();
        // Half-degree pixels: centres 10.25, 10.75 by 45.25, 45.75.
        assert_eq!(samples.len(), 4);
        for &(lon, lat, value) in &samples {
            let row = ((90.0 - lat) * 2.0 - 0.5).round() as usize;
            let column = ((lon + 180.0) * 2.0 - 0.5).round() as usize;
            assert_eq!(value, (row * nlon + column) as f64);
        }
        std::fs::remove_file(&path).unwrap();
    }
}
