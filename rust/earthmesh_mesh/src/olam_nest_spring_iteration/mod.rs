use super::*;

pub(crate) fn olam_nest_mrow_distance_multiplier(mrow1: isize, mrow2: isize) -> f64 {
    let mrmax = mrow1.max(mrow2);
    let mrmin = mrow1.min(mrow2);
    match (mrmax, mrmin) {
        (-2, -2) => 7.0 / 6.0,
        (-1, -2) => 8.0 / 6.0,
        (-1, -1) => 9.0 / 6.0,
        (1, -1) => 10.0 / 6.0,
        (1, 1) => 11.0 / 12.0,
        _ => 1.0,
    }
}

pub(crate) fn olam_nest_spring_iteration(
    m_points: &[CartesianPoint],
    mesh: &OlamDelaunayMesh,
    topology: &IcosahedronSpringTopology,
    movable_m_points: &[bool],
    dist00: f64,
    project_to_radius: bool,
) -> Option<Vec<CartesianPoint>> {
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
        || movable_m_points.len() != m_points.len()
    {
        return None;
    }

    let mut moveu = vec![false; topology.edge_m_points.len()];
    let mut compu = vec![false; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [im1, im2] = topology.edge_m_points[edge_id];
        moveu[edge_id] = movable_m_points[im1] || movable_m_points[im2];
        let [iu1, _, iu3, _] = topology.edge_neighbor_u[edge_id];
        let [iu1_im1, iu1_im2] = *topology.edge_m_points.get(iu1)?;
        let im3 = if iu1_im1 == im1 { iu1_im2 } else { iu1_im1 };
        let [iu3_im1, iu3_im2] = *topology.edge_m_points.get(iu3)?;
        let im4 = if iu3_im1 == im1 { iu3_im2 } else { iu3_im1 };
        compu[edge_id] = moveu[edge_id] || movable_m_points[im3] || movable_m_points[im4];
    }

    let mut edge_vectors = vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut edge_distances = vec![0.0_f64; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        if !compu[edge_id] {
            continue;
        }
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

    let max_mrlu = (2..topology.edge_m_points.len())
        .filter_map(|edge_id| {
            if moveu[edge_id] {
                Some(mesh.u_edges.get(edge_id)?.mrlu.max(1))
            } else {
                None
            }
        })
        .max()
        .unwrap_or(1);
    let dist00_f32 = dist00 as f32;
    let dmin = dist00_f32 / 2.0_f32.powi(max_mrlu.saturating_sub(1) as i32);
    let min_area_squared = 0.1875_f32 * dmin.powi(4);
    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];

    for edge_id in 2..topology.edge_m_points.len() {
        if !moveu[edge_id] {
            continue;
        }
        let edge = *mesh.u_edges.get(edge_id)?;
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = *edge_distances.get(edge_id)? as f32;
        let dist1 = *edge_distances.get(iu1)? as f32;
        let dist2 = *edge_distances.get(iu2)? as f32;
        let dist3 = *edge_distances.get(iu3)? as f32;
        let dist4 = *edge_distances.get(iu4)? as f32;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let angle_ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        if !angle_ratio.is_finite() {
            return None;
        }

        let edge_level = edge.mrlu.max(1);
        let mut target_distance =
            (dist00_f32 / 1.2) / 2.0_f32.powi(edge_level.saturating_sub(1) as i32) * angle_ratio;
        let face1 = *mesh.w_faces.get(edge.iw[0])?;
        let face2 = *mesh.w_faces.get(edge.iw[1])?;
        target_distance *= olam_nest_mrow_distance_multiplier(face1.mrow, face2.mrow) as f32;

        let s1 = 0.5 * (dist + dist1 + dist2);
        let s2 = 0.5 * (dist + dist3 + dist4);
        let area1_squared = s1 * (s1 - dist) * (s1 - dist1) * (s1 - dist2);
        let area2_squared = s2 * (s2 - dist) * (s2 - dist3) * (s2 - dist4);
        let min_local_area_squared = area1_squared.min(area2_squared);
        if min_local_area_squared <= 0.0 || !min_local_area_squared.is_finite() {
            return None;
        }
        let area_ratio = (min_area_squared / min_local_area_squared).max(1.0);
        target_distance *= area_ratio;

        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            (edge_vector.x as f32 * frac_change) as f64,
            (edge_vector.y as f32 * frac_change) as f64,
            (edge_vector.z as f32 * frac_change) as f64,
        );
    }

    let radius = if project_to_radius {
        Some(active_mesh_radius(mesh).ok()?)
    } else {
        None
    };
    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        if !movable_m_points[im] {
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
