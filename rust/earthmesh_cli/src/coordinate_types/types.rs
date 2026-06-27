/// Shared longitude/latitude row used by circle and close masks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatPoint {
    pub lon: f64,
    pub lat: f64,
}

/// A geographic region used to carve a gridfile down to an area of interest.
#[derive(Debug, Clone)]
pub enum GridRegion {
    Bbox {
        west: f64,
        east: f64,
        north: f64,
        south: f64,
    },
    Circle {
        lon: f64,
        lat: f64,
        radius_km: f64,
    },
    Close {
        points: Vec<LonLatPoint>,
    },
    Any(Vec<GridRegion>),
}

impl GridRegion {
    pub(crate) fn contains(&self, lon: f64, lat: f64) -> bool {
        let norm = |x: f64| ((x + 180.0).rem_euclid(360.0)) - 180.0;
        match self {
            GridRegion::Bbox {
                west,
                east,
                north,
                south,
            } => {
                let (s, n) = ((*south).min(*north), (*south).max(*north));
                let (w, e) = (norm(*west), norm(*east));
                let lon = norm(lon);
                lat >= s && lat <= n && lon >= w.min(e) && lon <= w.max(e)
            }
            GridRegion::Circle {
                lon: clon,
                lat: clat,
                radius_km,
            } => {
                let r = 6371.0_f64;
                let (la1, la2) = (clat.to_radians(), lat.to_radians());
                let dlat = (lat - *clat).to_radians();
                let dlon = (norm(lon) - norm(*clon)).to_radians();
                let a =
                    (dlat / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlon / 2.0).sin().powi(2);
                2.0 * r * a.sqrt().asin() <= *radius_km
            }
            GridRegion::Close { points } => point_in_close_region(points, lon, lat),
            GridRegion::Any(regions) => regions.iter().any(|region| region.contains(lon, lat)),
        }
    }
}

fn point_in_close_region(points: &[LonLatPoint], lon: f64, lat: f64) -> bool {
    if points.len() < 3 || !lon.is_finite() || !lat.is_finite() {
        return false;
    }
    let norm = |x: f64| ((x + 180.0).rem_euclid(360.0)) - 180.0;
    let lon0 = norm(lon);
    let unwrap = |x: f64| {
        let mut y = norm(x);
        if y - lon0 > 180.0 {
            y -= 360.0;
        } else if y - lon0 < -180.0 {
            y += 360.0;
        }
        y
    };
    let mut inside = false;
    let eps = 1.0e-12;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        if !a.lon.is_finite() || !a.lat.is_finite() || !b.lon.is_finite() || !b.lat.is_finite() {
            return false;
        }
        let ax = unwrap(a.lon);
        let bx = unwrap(b.lon);
        let ay = a.lat;
        let by = b.lat;
        let cross = (lon0 - ax) * (by - ay) - (lat - ay) * (bx - ax);
        let on_segment = cross.abs() <= eps
            && lon0 >= ax.min(bx) - eps
            && lon0 <= ax.max(bx) + eps
            && lat >= ay.min(by) - eps
            && lat <= ay.max(by) + eps;
        if on_segment {
            return true;
        }
        if (ay > lat) != (by > lat) {
            let x_at_lat = ax + (lat - ay) * (bx - ax) / (by - ay);
            if lon0 < x_at_lat {
                inside = !inside;
            }
        }
    }
    inside
}
