use earthmesh_mesh::LonLatDegrees;

pub(crate) fn area_judge_close_crosses_dateline(points: &[LonLatDegrees]) -> bool {
    if points.len() < 2 {
        return false;
    }
    let norm = |lon: f64| ((lon + 180.0).rem_euclid(360.0)) - 180.0;
    let (west, east) = points
        .iter()
        .filter_map(|point| {
            point
                .lon_degrees
                .is_finite()
                .then_some(norm(point.lon_degrees))
        })
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(west, east), lon| {
            (west.min(lon), east.max(lon))
        });
    east - west > 180.0
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

#[cfg(test)]
mod tests {
    use super::*;

    fn p(lon: f64, lat: f64) -> LonLatDegrees {
        LonLatDegrees {
            lon_degrees: lon,
            lat_degrees: lat,
        }
    }

    #[test]
    fn dateline_predicate_detects_closed_antimeridian_box() {
        let ring = [p(179.0, 0.0), p(-179.0, 0.0), p(-179.0, 1.0), p(179.0, 1.0)];
        assert!(area_judge_close_crosses_dateline(&ring));
    }

    #[test]
    fn dateline_predicate_ignores_normal_box() {
        let ring = [p(10.0, 0.0), p(12.0, 0.0), p(12.0, 1.0), p(10.0, 1.0)];
        assert!(!area_judge_close_crosses_dateline(&ring));
    }
}
