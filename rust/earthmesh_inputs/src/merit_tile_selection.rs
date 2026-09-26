use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Lon/lat bounding box used to select native MERIT-Hydro NetCDF windows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeritLonLatBbox {
    pub west: f64,
    pub east: f64,
    pub south: f64,
    pub north: f64,
}

/// Parse a MERIT-Hydro tile filename such as `n10e100.nc` into its 5-degree bounds.
pub fn merit_tile_bounds_from_name(name: impl AsRef<str>) -> io::Result<MeritLonLatBbox> {
    let raw = Path::new(name.as_ref())
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid MERIT tile name"))?;
    let stem = raw.strip_suffix(".nc").ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a MERIT-Hydro tile name: {raw}"),
        )
    })?;
    if stem.len() != 7 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a MERIT-Hydro tile name: {raw}"),
        ));
    }
    let lat_sign = &stem[0..1];
    let lat_value: f64 = stem[1..3].parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a MERIT-Hydro tile name: {raw}"),
        )
    })?;
    let lon_sign = &stem[3..4];
    let lon_value: f64 = stem[4..7].parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("not a MERIT-Hydro tile name: {raw}"),
        )
    })?;
    let south = match lat_sign {
        "n" => lat_value,
        "s" => -lat_value,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a MERIT-Hydro tile name: {raw}"),
            ))
        }
    };
    let west = match lon_sign {
        "e" => lon_value,
        "w" => -lon_value,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a MERIT-Hydro tile name: {raw}"),
            ))
        }
    };
    let bounds = MeritLonLatBbox {
        west,
        south,
        east: west + 5.0,
        north: south + 5.0,
    };
    if bounds.west < -180.0 || bounds.east > 180.0 || bounds.south < -90.0 || bounds.north > 90.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("MERIT-Hydro tile bounds are outside the Earth: {raw}"),
        ));
    }
    Ok(bounds)
}

/// Validate and split a MERIT query into ordinary, non-wrapping longitude windows.
///
/// A query whose `west > east` crosses the antimeridian and becomes at most two
/// windows, one on either side of +/-180 degrees.
pub fn split_merit_query_bbox(bbox: MeritLonLatBbox) -> io::Result<Vec<MeritLonLatBbox>> {
    validate_query_bbox(bbox)?;
    if bbox.west < bbox.east {
        return Ok(vec![bbox]);
    }

    let mut windows = Vec::with_capacity(2);
    if bbox.west < 180.0 {
        windows.push(MeritLonLatBbox {
            east: 180.0,
            ..bbox
        });
    }
    if bbox.east > -180.0 {
        windows.push(MeritLonLatBbox {
            west: -180.0,
            ..bbox
        });
    }
    if windows.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MERIT-Hydro bbox has no positive-width longitude interval",
        ));
    }
    Ok(windows)
}

/// Clip a non-wrapping query window to one MERIT tile.
pub fn clip_merit_bbox_to_tile(
    query: MeritLonLatBbox,
    tile: MeritLonLatBbox,
) -> io::Result<Option<MeritLonLatBbox>> {
    validate_non_wrapping_bbox(query, "MERIT-Hydro query window")?;
    validate_non_wrapping_bbox(tile, "MERIT-Hydro tile")?;
    let clipped = MeritLonLatBbox {
        west: query.west.max(tile.west),
        east: query.east.min(tile.east),
        south: query.south.max(tile.south),
        north: query.north.min(tile.north),
    };
    Ok((clipped.west < clipped.east && clipped.south < clipped.north).then_some(clipped))
}

/// Select MERIT-Hydro NetCDF tiles under a root directory whose 5-degree bounds intersect a bbox.
pub fn select_merit_hydro_tiles(
    root: impl AsRef<Path>,
    bbox: MeritLonLatBbox,
) -> io::Result<Vec<PathBuf>> {
    validate_query_bbox(bbox)?;
    let mut selected = Vec::new();
    for entry in fs::read_dir(root.as_ref())? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        let Ok(bounds) = merit_tile_bounds_from_name(name) else {
            continue;
        };
        if merit_bbox_intersects(bounds, bbox) {
            selected.push(path);
        }
    }
    selected.sort();
    Ok(selected)
}

fn validate_query_bbox(bbox: MeritLonLatBbox) -> io::Result<()> {
    validate_finite_and_ranges(bbox, "MERIT-Hydro bbox")?;
    if bbox.west == bbox.east {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "MERIT-Hydro bbox longitude interval must have positive width",
        ));
    }
    Ok(())
}

fn validate_non_wrapping_bbox(bbox: MeritLonLatBbox, label: &str) -> io::Result<()> {
    validate_finite_and_ranges(bbox, label)?;
    if bbox.west >= bbox.east {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must not cross the antimeridian"),
        ));
    }
    Ok(())
}

fn validate_finite_and_ranges(bbox: MeritLonLatBbox, label: &str) -> io::Result<()> {
    if ![bbox.west, bbox.east, bbox.south, bbox.north]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} coordinates must be finite"),
        ));
    }
    if bbox.west < -180.0 || bbox.west > 180.0 || bbox.east < -180.0 || bbox.east > 180.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} longitudes must be within [-180, 180] degrees"),
        ));
    }
    if bbox.south < -90.0 || bbox.north > 90.0 || bbox.south >= bbox.north {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} latitude interval must be ordered within [-90, 90] degrees"),
        ));
    }
    Ok(())
}

/// Does tile `a` (a 5 deg MERIT tile, never spans the antimeridian) intersect query
/// `b`? A query whose `west > east` is interpreted as crossing the antimeridian.
fn merit_bbox_intersects(a: MeritLonLatBbox, b: MeritLonLatBbox) -> bool {
    let lat_overlap = a.south < b.north && a.north > b.south;
    if !lat_overlap {
        return false;
    }
    if b.west <= b.east {
        a.west < b.east && a.east > b.west
    } else {
        (a.east > b.west && a.west < 180.0) || (a.west < b.east && a.east > -180.0)
    }
}
