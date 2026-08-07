use super::*;
use rayon::prelude::*;

/// Reusable buffers for [`method_c_global_spring_iteration_into`].
///
/// The spring driver runs thousands of Jacobi iterations; allocating these
/// once per spring call instead of once per iteration removes the dominant
/// `alloc_zeroed`/`memcpy` traffic the profiler attributed to the spring pass.
pub(crate) struct MethodCGlobalSpringScratch {
    edge_vectors: Vec<CartesianPoint>,
    edge_distances: Vec<f64>,
    edge_displacements: Vec<CartesianPoint>,
}

impl MethodCGlobalSpringScratch {
    pub(crate) fn new(edge_count: usize) -> Self {
        Self {
            edge_vectors: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
            edge_distances: vec![0.0_f64; edge_count],
            edge_displacements: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
        }
    }
}

/// One Method-C global-spring Jacobi iteration, writing into `updated_m_points`.
///
/// Every edge slot `2..` is fully rewritten before it is read, slots `0..2`
/// keep their zero placeholders from scratch creation, and dummy point slots
/// are never written. Point accumulation stays in `f64`, while edge deltas
/// retain the Canonical default-`real` (`f32`) rounding points used by the
/// Fortran implementation; the driver performs the final storage cast once.
pub(crate) fn method_c_global_spring_iteration_into(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    scratch: &mut MethodCGlobalSpringScratch,
    dist00: f64,
    radius: Option<f64>,
    updated_m_points: &mut [CartesianPoint],
) -> Option<()> {
    // Destructure once: routing every hot-loop slice access through
    // `scratch.field` projections degraded inlining/alias analysis (profiles
    // showed `SliceIndex::get` surfacing as a real call); independent local
    // bindings restore the codegen of the former stack-local buffers.
    let MethodCGlobalSpringScratch {
        edge_vectors,
        edge_distances,
        edge_displacements,
    } = scratch;
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
        || edge_vectors.len() != topology.edge_m_points.len()
        || edge_distances.len() != topology.edge_m_points.len()
        || edge_displacements.len() != topology.edge_m_points.len()
        || updated_m_points.len() != m_points.len()
    {
        return None;
    }

    edge_vectors
        .par_iter_mut()
        .zip(edge_distances.par_iter_mut())
        .enumerate()
        .skip(2)
        .try_for_each(|(edge_id, (edge_vector, edge_distance))| {
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
            *edge_vector = CartesianPoint::new(dx as f64, dy as f64, dz as f64);
            *edge_distance = distance as f64;
            Some(())
        })?;

    let dist00 = dist00 as f32;
    edge_displacements
        .par_iter_mut()
        .enumerate()
        .skip(2)
        .try_for_each(|(edge_id, edge_displacement)| {
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
            let target_distance = dist00 / 1.2 * ratio;
            let frac_change = (target_distance - dist) / dist;
            let edge_vector = edge_vectors[edge_id];
            *edge_displacement = CartesianPoint::new(
                (edge_vector.x as f32 * frac_change) as f64,
                (edge_vector.y as f32 * frac_change) as f64,
                (edge_vector.z as f32 * frac_change) as f64,
            );
            Some(())
        })?;

    updated_m_points
        .par_iter_mut()
        .enumerate()
        .skip(2)
        .try_for_each(|(im, updated_point)| {
            // Fortran's optional `impent` freeze is commented out: all active
            // points, including the twelve pentagons, participate.
            let npoly = topology.m_npoly[im];
            if npoly > 7 {
                return None;
            }
            // Reading the input buffer is identical to the historical read of the
            // freshly cloned output slot (they held the same value at this point).
            let mut point = m_points[im];
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
                *updated_point = CartesianPoint::new(
                    point.x * expansion,
                    point.y * expansion,
                    point.z * expansion,
                );
            } else {
                *updated_point = point;
            }
            Some(())
        })?;

    Some(())
}
