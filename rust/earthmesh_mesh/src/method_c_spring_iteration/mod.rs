use super::*;

/// Reusable buffers + loop-invariant pentagon mask for
/// [`method_c_global_spring_iteration_into`].
///
/// The spring driver runs thousands of Jacobi iterations; allocating these
/// once per spring call instead of once per iteration removes the dominant
/// `alloc_zeroed`/`memcpy` traffic the profiler attributed to the spring pass.
pub(crate) struct MethodCGlobalSpringScratch {
    is_pentagon: Vec<bool>,
    edge_vectors: Vec<CartesianPoint>,
    edge_distances: Vec<f64>,
    edge_displacements: Vec<CartesianPoint>,
}

impl MethodCGlobalSpringScratch {
    pub(crate) fn new(point_count: usize, edge_count: usize, impent: &[usize; 12]) -> Self {
        // Bitmask replaces the former `impent.contains(&im)` linear scan in the
        // per-point hot loop. An out-of-range pentagon id could never match an
        // in-range `im`, exactly like `contains`, so it is simply skipped here.
        let mut is_pentagon = vec![false; point_count];
        for &im in impent {
            if im < point_count {
                is_pentagon[im] = true;
            }
        }
        Self {
            is_pentagon,
            edge_vectors: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
            edge_distances: vec![0.0_f64; edge_count],
            edge_displacements: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
        }
    }
}

/// One Method-C global-spring Jacobi iteration, writing into `updated_m_points`.
///
/// Bit-identical to the historical per-iteration-allocating version: every
/// edge slot `2..` is fully rewritten here before it is read, slots `0..2`
/// keep their zero placeholders from scratch creation, and pentagon/dummy
/// point slots are never written (the driver keeps both point buffers
/// initialized to the input positions, which those slots retain forever).
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
        is_pentagon,
        edge_vectors,
        edge_distances,
        edge_displacements,
    } = scratch;
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
        || is_pentagon.len() != m_points.len()
        || edge_vectors.len() != topology.edge_m_points.len()
        || edge_distances.len() != topology.edge_m_points.len()
        || edge_displacements.len() != topology.edge_m_points.len()
        || updated_m_points.len() != m_points.len()
    {
        return None;
    }

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

    for im in 2..m_points.len() {
        if is_pentagon[im] {
            continue;
        }

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
            updated_m_points[im] = CartesianPoint::new(
                point.x * expansion,
                point.y * expansion,
                point.z * expansion,
            );
        } else {
            updated_m_points[im] = point;
        }
    }

    Some(())
}
