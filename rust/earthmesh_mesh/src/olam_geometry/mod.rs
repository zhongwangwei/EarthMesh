use std::{collections::BTreeMap, io};

use super::{IcosahedronWFace, LonLatDegrees};

pub(crate) fn validate_lonlat(point: LonLatDegrees) -> io::Result<()> {
    if !point.lon_degrees.is_finite()
        || !point.lat_degrees.is_finite()
        || point.lat_degrees < -90.0
        || point.lat_degrees > 90.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid lon/lat point {:?}", point),
        ));
    }
    Ok(())
}

pub(crate) fn validate_positive_distance(name: &str, value: f64) -> io::Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be positive and finite"),
        ));
    }
    Ok(())
}

pub(crate) fn face_following_two_vertices(
    face: IcosahedronWFace,
    im: usize,
    iw: usize,
) -> io::Result<(usize, usize)> {
    if face.im[0] == im {
        Ok((face.im[1], face.im[2]))
    } else if face.im[1] == im {
        Ok((face.im[2], face.im[0]))
    } else if face.im[2] == im {
        Ok((face.im[0], face.im[1]))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} vertices {:?} do not contain M point {im}",
                face.im
            ),
        ))
    }
}

pub(crate) fn face_following_vertex(
    face: IcosahedronWFace,
    im: usize,
    iw: usize,
) -> io::Result<usize> {
    if face.im[0] == im {
        Ok(face.im[1])
    } else if face.im[1] == im {
        Ok(face.im[2])
    } else if face.im[2] == im {
        Ok(face.im[0])
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} vertices {:?} do not contain M point {im}",
                face.im
            ),
        ))
    }
}

pub(crate) fn olam_edge_key(im1: usize, im2: usize) -> (usize, usize) {
    if im1 <= im2 {
        (im1, im2)
    } else {
        (im2, im1)
    }
}

pub(crate) fn lookup_olam_midpoint(
    midpoint_by_edge: &BTreeMap<(usize, usize), usize>,
    im1: usize,
    im2: usize,
    owner_iw: usize,
) -> io::Result<usize> {
    midpoint_by_edge
        .get(&olam_edge_key(im1, im2))
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("W face {owner_iw} references edge [{im1}, {im2}] without a midpoint"),
            )
        })
}

pub(crate) fn lookup_olam_thirds(
    thirds_by_edge: &BTreeMap<(usize, usize), [usize; 2]>,
    im1: usize,
    im2: usize,
    owner_iw: usize,
) -> io::Result<[usize; 2]> {
    let points_from_low_to_high = thirds_by_edge
        .get(&olam_edge_key(im1, im2))
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("W face {owner_iw} references edge [{im1}, {im2}] without thirds"),
            )
        })?;
    if im1 <= im2 {
        Ok(points_from_low_to_high)
    } else {
        Ok([points_from_low_to_high[1], points_from_low_to_high[0]])
    }
}
