use std::io;

use crate::{spherical_centroid_degrees, LonLatDegrees};

pub fn midpoint_lonlat(a: LonLatDegrees, b: LonLatDegrees) -> Option<LonLatDegrees> {
    spherical_centroid_degrees(&[a, b])
}

pub fn average_lonlat3(
    a: LonLatDegrees,
    b: LonLatDegrees,
    c: LonLatDegrees,
) -> Option<LonLatDegrees> {
    spherical_centroid_degrees(&[a, b, c])
}

pub fn check_crossing_canonical_lonlat(points: &mut [LonLatDegrees]) {
    for point in points {
        if point.lon_degrees < 0.0 {
            point.lon_degrees += 180.0;
        } else {
            point.lon_degrees -= 180.0;
        }
    }
}

pub fn crossline_check_canonical(
    iter: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crossline_check range exceeds point storage",
        ));
    }
    for point in triangle_points
        .iter_mut()
        .take(num_mp[iter] + 1)
        .skip(num_mp[iter - 1] + 1)
    {
        if point.lon_degrees == -180.0 {
            point.lon_degrees = 180.0;
        }
    }
    for point in cell_points
        .iter_mut()
        .take(num_wp[iter] + 1)
        .skip(num_wp[iter - 1] + 1)
    {
        if point.lon_degrees == -180.0 {
            point.lon_degrees = 180.0;
        }
    }
    Ok(())
}
