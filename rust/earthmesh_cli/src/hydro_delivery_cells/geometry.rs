pub(super) fn unwrap_ring_lon(ring: &mut [(f64, f64)], ref_lon: f64) {
    for p in ring.iter_mut() {
        while p.0 - ref_lon > 180.0 {
            p.0 -= 360.0;
        }
        while p.0 - ref_lon < -180.0 {
            p.0 += 360.0;
        }
    }
}

pub(super) fn preview_cell_too_large(
    corners: &[(f64, f64)],
    clon: f64,
    clat: f64,
    max_deg: f64,
) -> bool {
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
    if corners.iter().any(|&c| gc_deg((clon, clat), c) > max_deg) {
        return true;
    }
    let n = corners.len();
    (0..n).any(|i| gc_deg(corners[i], corners[(i + 1) % n]) > max_deg)
}

pub(super) fn convex_hull_ccw(mut pts: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    pts.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);
    if pts.len() < 3 {
        return pts;
    }
    let cross = |o: &(f64, f64), a: &(f64, f64), b: &(f64, f64)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64)> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(&lower[lower.len() - 2], &lower[lower.len() - 1], &p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64)> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(&upper[upper.len() - 2], &upper[upper.len() - 1], &p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

pub(crate) fn convex_hull_order_indices(mut pts: Vec<(f64, f64, usize)>) -> Vec<usize> {
    pts.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });
    pts.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12);
    if pts.len() < 3 {
        return pts.iter().map(|p| p.2).collect();
    }
    let cross = |o: &(f64, f64, usize), a: &(f64, f64, usize), b: &(f64, f64, usize)| {
        (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0)
    };
    let mut lower: Vec<(f64, f64, usize)> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(&lower[lower.len() - 2], &lower[lower.len() - 1], &p) <= 0.0
        {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<(f64, f64, usize)> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(&upper[upper.len() - 2], &upper[upper.len() - 1], &p) <= 0.0
        {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower.iter().map(|p| p.2).collect()
}
