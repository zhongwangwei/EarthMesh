use super::*;

pub(crate) fn olam_global_spring_iteration(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    impent: &[usize; 12],
    dist00: f64,
    radius: Option<f64>,
) -> Option<Vec<CartesianPoint>> {
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
    {
        return None;
    }

    let mut edge_vectors = vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut edge_distances = vec![0.0_f64; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [im1, im2] = topology.edge_m_points[edge_id];
        let point1 = *m_points.get(im1)?;
        let point2 = *m_points.get(im2)?;
        let dx = (point2.x - point1.x) as f32;
        let dy = (point2.y - point1.y) as f32;
        let dz = (point2.z - point1.z) as f32;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance == 0.0 || !distance.is_finite() {
            return None;
        }
        edge_vectors[edge_id] = CartesianPoint::new(dx as f64, dy as f64, dz as f64);
        edge_distances[edge_id] = distance as f64;
    }

    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let dist00_f32 = dist00 as f32;
    for edge_id in 2..topology.edge_m_points.len() {
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = edge_distances[edge_id] as f32;
        let dist1 = *edge_distances.get(iu1)? as f32;
        let dist2 = *edge_distances.get(iu2)? as f32;
        let dist3 = *edge_distances.get(iu3)? as f32;
        let dist4 = *edge_distances.get(iu4)? as f32;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        if !ratio.is_finite() {
            return None;
        }
        let target_distance = dist00_f32 / 1.2 * ratio;
        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            (edge_vector.x as f32 * frac_change) as f64,
            (edge_vector.y as f32 * frac_change) as f64,
            (edge_vector.z as f32 * frac_change) as f64,
        );
    }

    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        if impent.contains(&im) {
            continue;
        }

        let npoly = topology.m_npoly[im];
        if npoly > 7 {
            return None;
        }
        let mut point = updated_m_points[im];
        for j in 0..npoly {
            let edge_id = topology.m_u_edges[im][j];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = topology.directions[im][j] as f32;
            point.x += (direction * displacement.x as f32) as f64;
            point.y += (direction * displacement.y as f32) as f64;
            point.z += (direction * displacement.z as f32) as f64;
        }

        if let Some(radius) = radius {
            let norm = magnitude(point);
            if norm == 0.0 || !norm.is_finite() {
                return None;
            }
            let expansion = radius / norm;
            updated_m_points[im] = CartesianPoint::new(
                point.x * expansion,
                point.y * expansion,
                point.z * expansion,
            );
        } else {
            updated_m_points[im] = point;
        }
    }

    Some(updated_m_points)
}
