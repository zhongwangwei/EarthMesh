pub(super) fn signed_ring_area(coordinates: &[(f64, f64)]) -> f64 {
    if coordinates.len() < 3 {
        return 0.0;
    }
    let mut twice_area = 0.0;
    for (index, (lon1, lat1)) in coordinates.iter().enumerate() {
        let (lon2, lat2) = coordinates[(index + 1) % coordinates.len()];
        twice_area += lon1 * lat2 - lon2 * lat1;
    }
    twice_area / 2.0
}

pub(crate) fn ring_area(coordinates: &[(f64, f64)]) -> f64 {
    signed_ring_area(coordinates).abs()
}
