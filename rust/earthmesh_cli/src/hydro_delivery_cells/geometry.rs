fn unwrap_ring_lon(ring: &mut [(f64, f64)], ref_lon: f64) {
    let mut previous = ref_lon;
    for point in ring {
        while point.0 - previous > 180.0 {
            point.0 -= 360.0;
        }
        while point.0 - previous < -180.0 {
            point.0 += 360.0;
        }
        previous = point.0;
    }
}

/// Reject only mesh edges too large for the minor-arc overlay. The source cell
/// center is not authoritative for GeoJSON geometry and can wrap independently.
pub(super) fn cell_has_unsupported_edge_arc(corners: &[(f64, f64)], max_deg: f64) -> bool {
    fn gc_deg(a: (f64, f64), b: (f64, f64)) -> f64 {
        let xyz = |lon: f64, lat: f64| {
            let (lo, la) = (lon.to_radians(), lat.to_radians());
            [la.cos() * lo.cos(), la.cos() * lo.sin(), la.sin()]
        };
        let (va, vb) = (xyz(a.0, a.1), xyz(b.0, b.1));
        (va[0] * vb[0] + va[1] * vb[1] + va[2] * vb[2])
            .clamp(-1.0, 1.0)
            .acos()
            .to_degrees()
    }
    let n = corners.len();
    (0..n).any(|i| gc_deg(corners[i], corners[(i + 1) % n]) > max_deg)
}

fn clip_ring_at_longitude(
    ring: &[(f64, f64)],
    boundary: f64,
    keep_greater: bool,
) -> Vec<(f64, f64)> {
    let inside = |point: (f64, f64)| {
        if keep_greater {
            point.0 >= boundary
        } else {
            point.0 <= boundary
        }
    };
    let intersection = |left: (f64, f64), right: (f64, f64)| {
        let t = (boundary - left.0) / (right.0 - left.0);
        (boundary, left.1 + t * (right.1 - left.1))
    };
    let Some(&last) = ring.last() else {
        return Vec::new();
    };
    let mut output = Vec::new();
    let mut previous = last;
    let mut previous_inside = inside(previous);
    for &current in ring {
        let current_inside = inside(current);
        if current_inside != previous_inside {
            output.push(intersection(previous, current));
        }
        if current_inside {
            output.push(current);
        }
        previous = current;
        previous_inside = current_inside;
    }
    output.dedup_by(|left, right| {
        (left.0 - right.0).abs() <= 1.0e-12 && (left.1 - right.1).abs() <= 1.0e-12
    });
    if output.len() > 1
        && (output[0].0 - output[output.len() - 1].0).abs() <= 1.0e-12
        && (output[0].1 - output[output.len() - 1].1).abs() <= 1.0e-12
    {
        output.pop();
    }
    output
}

/// Split one ordered, non-polar spherical cell at ±180° into bounded GeoJSON
/// rings. Each returned ring is closed and every longitude is in [-180, 180].
pub(super) fn split_ring_at_antimeridian(
    corners: &[(f64, f64)],
    ref_lon: f64,
) -> Vec<Vec<(f64, f64)>> {
    if corners.len() < 3 {
        return Vec::new();
    }
    let mut unwrapped = corners.to_vec();
    unwrap_ring_lon(&mut unwrapped, ref_lon);
    let lon_min = unwrapped
        .iter()
        .map(|point| point.0)
        .fold(f64::INFINITY, f64::min);
    let lon_max = unwrapped
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max);
    let first_slab = ((lon_min + 180.0) / 360.0).floor() as i32;
    let last_slab = ((lon_max + 180.0) / 360.0).floor() as i32;
    let mut rings = Vec::new();
    for slab in first_slab..=last_slab {
        let west = -180.0 + f64::from(slab) * 360.0;
        let east = west + 360.0;
        let clipped = clip_ring_at_longitude(&unwrapped, west, true);
        let mut clipped = clip_ring_at_longitude(&clipped, east, false);
        if clipped.len() < 3 {
            continue;
        }
        let clipped_lon_span = clipped
            .iter()
            .map(|point| point.0)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), lon| {
                (lo.min(lon), hi.max(lon))
            });
        if clipped_lon_span.1 - clipped_lon_span.0 <= 1.0e-12 {
            continue;
        }
        let shift = f64::from(slab) * 360.0;
        for point in &mut clipped {
            point.0 = (point.0 - shift).clamp(-180.0, 180.0);
        }
        clipped.push(clipped[0]);
        rings.push(clipped);
    }
    rings
}

pub(super) fn ring_intersects_directed_bbox(ring: &[(f64, f64)], bbox: Option<[f64; 4]>) -> bool {
    let Some([west, south, east, north]) = bbox else {
        return true;
    };
    let raw_span = east - west;
    let span = if (raw_span.abs() - 360.0).abs() <= 1.0e-12 {
        360.0
    } else {
        raw_span.rem_euclid(360.0)
    };
    let bbox_lo = west;
    let bbox_hi = west + span;
    let bbox_mid = bbox_lo + 0.5 * span;
    if ring.is_empty() {
        return false;
    }
    // Unwrap the compact cell as one coherent ring first, then translate the
    // whole interval toward the directed bbox. Wrapping each vertex around
    // `bbox_mid` independently splits cells that straddle the bbox's antipodal
    // meridian into a spurious ~360° envelope (for example, a cell near -66.6°
    // against a bbox near 113.4°).
    let anchor = ring[0].0;
    let mut unwrapped_lons = Vec::with_capacity(ring.len());
    for &(lon, _) in ring {
        let mut lon = lon;
        while lon - anchor > 180.0 {
            lon -= 360.0;
        }
        while lon - anchor < -180.0 {
            lon += 360.0;
        }
        unwrapped_lons.push(lon);
    }
    let ring_lo = unwrapped_lons.iter().copied().fold(f64::INFINITY, f64::min);
    let ring_hi = unwrapped_lons
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let shift = 360.0 * ((bbox_mid - 0.5 * (ring_lo + ring_hi)) / 360.0).round();
    let mut lon_lo = f64::INFINITY;
    let mut lon_hi = f64::NEG_INFINITY;
    let mut lat_lo = f64::INFINITY;
    let mut lat_hi = f64::NEG_INFINITY;
    for ((_, lat), lon) in ring
        .iter()
        .zip(unwrapped_lons)
        .map(|(point, lon)| (point, lon + shift))
    {
        lon_lo = lon_lo.min(lon);
        lon_hi = lon_hi.max(lon);
        lat_lo = lat_lo.min(*lat);
        lat_hi = lat_hi.max(*lat);
    }
    lon_lo <= bbox_hi && lon_hi >= bbox_lo && lat_lo <= north && lat_hi >= south
}

/// Order spherical cell corners in the local east/north tangent plane. This
/// avoids longitude-plane convex hull failures near the poles and dateline.
pub(super) fn order_around_spherical_center(
    mut corners: Vec<(f64, f64)>,
    clon: f64,
    clat: f64,
) -> Vec<(f64, f64)> {
    let (lon0, lat0) = (clon.to_radians(), clat.to_radians());
    corners.sort_by(|a, b| {
        let angle = |&(lon, lat): &(f64, f64)| {
            let (lon, lat) = (lon.to_radians(), lat.to_radians());
            let dlon = lon - lon0;
            let east = lat.cos() * dlon.sin();
            let north = lat.cos() * lat0.sin() * dlon.cos() - lat.sin() * lat0.cos();
            east.atan2(north)
        };
        angle(a)
            .partial_cmp(&angle(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    corners.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);
    corners
}

#[cfg(test)]
mod bbox_tests {
    use super::ring_intersects_directed_bbox;

    #[test]
    fn antipodal_cell_does_not_form_a_ghost_360_degree_envelope() {
        let ghost = [(-67.48, 21.7), (-66.9, 22.5), (-65.54, 22.2), (-66.0, 21.8)];
        assert!(!ring_intersects_directed_bbox(
            &ghost,
            Some([113.25, 22.0, 113.5, 22.25]),
        ));
    }

    #[test]
    fn compact_dateline_cell_intersects_a_directed_dateline_bbox() {
        let cell = [(179.2, -1.0), (-179.4, -1.0), (-179.4, 1.0), (179.2, 1.0)];
        assert!(ring_intersects_directed_bbox(
            &cell,
            Some([170.0, -2.0, -170.0, 2.0]),
        ));
        assert!(!ring_intersects_directed_bbox(
            &cell,
            Some([10.0, -2.0, 20.0, 2.0]),
        ));
    }
}
