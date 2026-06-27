use super::*;

/// Port of one main-loop iteration in `icosahedron.F90:spring_dynamics1`.
///
/// `dist00` is the coarse target segment length computed by Fortran as
/// `beta * pi2_r8 * erad8 / (5 * nxp)`. The routine applies the OLAM-6.4
/// `dist00 / 1.2` target scaling, opposite-angle ratio clamp, per-M-point
/// direction signs from `IcosahedronSpringTopology`, and radius normalization.
pub fn icosahedron_spring_iteration_fortran(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    dist00: f64,
    radius: f64,
) -> Option<IcosahedronSpringIterationOutput> {
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
        let edge_vector = vector_between(point1, point2);
        let distance = magnitude(edge_vector);
        if distance == 0.0 {
            return None;
        }
        edge_vectors[edge_id] = edge_vector;
        edge_distances[edge_id] = distance;
    }

    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = edge_distances[edge_id];
        let dist1 = *edge_distances.get(iu1)?;
        let dist2 = *edge_distances.get(iu2)?;
        let dist3 = *edge_distances.get(iu3)?;
        let dist4 = *edge_distances.get(iu4)?;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        let target_distance = dist00 / 1.2 * ratio;
        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            edge_vector.x * frac_change,
            edge_vector.y * frac_change,
            edge_vector.z * frac_change,
        );
    }

    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        let npoly = topology.m_npoly[im];
        if npoly > 7 {
            return None;
        }
        let mut point = updated_m_points[im];
        for j in 0..npoly {
            let edge_id = topology.m_u_edges[im][j];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = topology.directions[im][j];
            point.x += direction * displacement.x;
            point.y += direction * displacement.y;
            point.z += direction * displacement.z;
        }

        let norm = magnitude(point);
        if norm == 0.0 {
            return None;
        }
        let expansion = radius / norm;
        updated_m_points[im] = CartesianPoint::new(
            point.x * expansion,
            point.y * expansion,
            point.z * expansion,
        );
    }

    Some(IcosahedronSpringIterationOutput {
        updated_m_points,
        edge_displacements,
        edge_distances,
    })
}

/// Multi-iteration wrapper for `icosahedron.F90:spring_dynamics1`.
///
/// It repeatedly applies `icosahedron_spring_iteration_fortran` and records the
/// Fortran-style periodic Max-DS diagnostic for `iter == 1` or
/// `iter % diagnostic_every == 0`, comparing each diagnostic iteration against
/// the coordinates at the start of that same iteration.
pub fn icosahedron_spring_dynamics1_fortran(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    niter: usize,
    dist00: f64,
    radius: f64,
    diagnostic_every: usize,
) -> Option<IcosahedronSpringDynamicsOutput> {
    if diagnostic_every == 0 {
        return None;
    }

    let mut current_m_points = m_points.to_vec();
    let mut last_edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter {
        if (iteration == 1 || iteration == niter || iteration % 20 == 0)
            && !earthmesh_core::progress::report("spring", iteration, niter)
        {
            return None;
        }
        let record_diagnostic = iteration == 1 || iteration % diagnostic_every == 0;
        let diagnostic_reference = if record_diagnostic {
            Some(current_m_points.clone())
        } else {
            None
        };

        let iteration_output =
            icosahedron_spring_iteration_fortran(&current_m_points, topology, dist00, radius)?;
        current_m_points = iteration_output.updated_m_points;
        last_edge_displacements = iteration_output.edge_displacements;

        if let Some(reference) = diagnostic_reference {
            let mut max_displacement = 0.0_f64;
            for im in 2..current_m_points.len() {
                let displacement = magnitude(vector_between(reference[im], current_m_points[im]));
                max_displacement = max_displacement.max(displacement);
            }
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
    }

    Some(IcosahedronSpringDynamicsOutput {
        updated_m_points: current_m_points,
        last_edge_displacements,
        diagnostic_max_displacements,
    })
}
