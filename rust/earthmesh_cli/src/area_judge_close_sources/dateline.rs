use earthmesh_mesh::LonLatDegrees;

pub(crate) fn area_judge_close_crosses_dateline(points: &[LonLatDegrees]) -> bool {
    if points.len() < 2 {
        return false;
    }
    let edgew = points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::INFINITY, f64::min);
    let edgee = points
        .iter()
        .map(|point| point.lon_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let widest_edge = points
        .windows(2)
        .map(|pair| (pair[1].lon_degrees - pair[0].lon_degrees).abs())
        .fold(0.0, f64::max);
    widest_edge > (edgee - edgew).abs()
}

pub(crate) fn area_judge_check_crossing(points: &mut [LonLatDegrees]) {
    for point in points {
        if point.lon_degrees < 0.0 {
            point.lon_degrees += 180.0;
        } else {
            point.lon_degrees -= 180.0;
        }
    }
}
