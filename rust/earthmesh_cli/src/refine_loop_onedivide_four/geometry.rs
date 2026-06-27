use std::io;

use crate::LonLatPoint;

pub(super) fn midpoint(a: LonLatPoint, b: LonLatPoint) -> LonLatPoint {
    LonLatPoint {
        lon: (a.lon + b.lon) / 2.0,
        lat: (a.lat + b.lat) / 2.0,
    }
}

pub(super) fn centroid3(a: LonLatPoint, b: LonLatPoint, c: LonLatPoint) -> LonLatPoint {
    LonLatPoint {
        lon: (a.lon + b.lon + c.lon) / 3.0,
        lat: (a.lat + b.lat + c.lat) / 3.0,
    }
}

pub(super) fn check_crossing_fortran_points(points: &mut [LonLatPoint]) {
    for point in points {
        if point.lon < 0.0 {
            point.lon += 180.0;
        } else {
            point.lon -= 180.0;
        }
    }
}

pub(super) fn crossline_check_fortran_points(
    iter: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    mp_new: &mut [LonLatPoint],
    wp_new: &mut [LonLatPoint],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    if num_mp[iter] >= mp_new.len() || num_wp[iter] >= wp_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crossline_check range exceeds point storage",
        ));
    }
    for point in mp_new
        .iter_mut()
        .take(num_mp[iter] + 1)
        .skip(num_mp[iter - 1] + 1)
    {
        if point.lon == -180.0 {
            point.lon = 180.0;
        }
    }
    for point in wp_new
        .iter_mut()
        .take(num_wp[iter] + 1)
        .skip(num_wp[iter - 1] + 1)
    {
        if point.lon == -180.0 {
            point.lon = 180.0;
        }
    }
    Ok(())
}
