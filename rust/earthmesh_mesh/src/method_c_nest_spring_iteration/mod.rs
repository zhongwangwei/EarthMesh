use super::*;
use rayon::prelude::*;

pub(crate) fn method_c_nest_mrow_distance_multiplier(mrow1: isize, mrow2: isize) -> f64 {
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

/// `moveu` (edge has a movable endpoint) and `compu` (edge participates in a
/// movable edge's twocosphi stencil) masks shared by both scratch builders.
fn build_nest_move_masks(
    topology: &IcosahedronSpringTopology,
    movable_m_points: &[bool],
) -> Option<(Vec<bool>, Vec<bool>)> {
    if topology.edge_neighbor_u.len() != topology.edge_m_points.len() {
        return None;
    }
    let edge_count = topology.edge_m_points.len();
    let mut moveu = vec![false; edge_count];
    let mut compu = vec![false; edge_count];
    for edge_id in 2..edge_count {
        let [im1, im2] = topology.edge_m_points[edge_id];
        moveu[edge_id] = movable_m_points[im1] || movable_m_points[im2];
        let [iu1, _, iu3, _] = topology.edge_neighbor_u[edge_id];
        let [iu1_im1, iu1_im2] = *topology.edge_m_points.get(iu1)?;
        let im3 = if iu1_im1 == im1 { iu1_im2 } else { iu1_im1 };
        let [iu3_im1, iu3_im2] = *topology.edge_m_points.get(iu3)?;
        let im4 = if iu3_im1 == im1 { iu3_im2 } else { iu3_im1 };
        compu[edge_id] = moveu[edge_id] || movable_m_points[im3] || movable_m_points[im4];
    }
    Some((moveu, compu))
}

/// Reusable buffers + loop-invariant lookups for
/// [`method_c_nest_spring_iteration_into`].
///
/// `moveu`/`compu`, the max-mrlu spacing floor, the per-edge refinement-level
/// target base, the per-edge mrow multiplier, and the projection radius depend
/// only on the (fixed) mesh/topology/mask, so the historical version recomputed
/// identical values on each of the thousands of iterations. Hoisting them here
/// is bit-identical; the float that mixes with per-iteration values
/// (`target_level_base * angle_ratio`, then `*= mrow multiplier`) preserves the
/// original multiplication order exactly.
pub(crate) struct MethodCNestSpringScratch {
    pub(crate) moveu: Vec<bool>,
    pub(crate) compu: Vec<bool>,
    pub(crate) target_level_base: Vec<f32>,
    pub(crate) target_mrow_multiplier: Vec<f32>,
    pub(crate) min_area_squared: f32,
    pub(crate) radius: Option<f64>,
    pub(crate) edge_vectors: Vec<CartesianPoint>,
    pub(crate) edge_distances: Vec<f64>,
    pub(crate) edge_displacements: Vec<CartesianPoint>,
}

impl MethodCNestSpringScratch {
    pub(crate) fn new(
        mesh: &MethodCDelaunayMesh,
        topology: &IcosahedronSpringTopology,
        movable_m_points: &[bool],
        dist00: f64,
        project_to_radius: bool,
    ) -> Option<Self> {
        let (moveu, compu) = build_nest_move_masks(topology, movable_m_points)?;
        let edge_count = topology.edge_m_points.len();

        let max_mrlu = (2..edge_count)
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

        let mut target_level_base = vec![0.0_f32; edge_count];
        let mut target_mrow_multiplier = vec![1.0_f32; edge_count];
        for edge_id in 2..edge_count {
            if !moveu[edge_id] {
                continue;
            }
            let edge = *mesh.u_edges.get(edge_id)?;
            let edge_level = edge.mrlu.max(1);
            target_level_base[edge_id] =
                (dist00_f32 / 1.2) / 2.0_f32.powi(edge_level.saturating_sub(1) as i32);
            let face1 = *mesh.w_faces.get(edge.iw[0])?;
            let face2 = *mesh.w_faces.get(edge.iw[1])?;
            target_mrow_multiplier[edge_id] =
                method_c_nest_mrow_distance_multiplier(face1.mrow, face2.mrow) as f32;
        }

        let radius = if project_to_radius {
            Some(active_mesh_radius(mesh).ok()?)
        } else {
            None
        };

        Some(Self {
            moveu,
            compu,
            target_level_base,
            target_mrow_multiplier,
            min_area_squared,
            radius,
            edge_vectors: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
            edge_distances: vec![0.0_f64; edge_count],
            edge_displacements: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
        })
    }

    /// H-field variant: per-edge target lengths (meters, Canonical-indexed by U
    /// id, slots `0..2` unused) are supplied directly, e.g. sampled from an
    /// `earthmesh_hfield` cell-width field at edge midpoints. The mrow
    /// multiplier is identity because a gradient-limited field already encodes
    /// the transition shaping continuously; the anti-degeneracy area floor
    /// uses the minimum target over movable edges, mirroring the compatibility
    /// `dist00 / 2^(max_mrlu - 1)` finest-level spacing.
    ///
    /// Parity scope (deliberate, documented): with targets built as
    /// `dist00 / 2^(mrlu - 1)` the target base reproduces the compatibility value
    /// bitwise (power-of-two scaling commutes exactly with f32 rounding) on
    /// every edge whose compatibility mrow multiplier is `1.0`. On mrow-shaped
    /// transition edges the standard path applies the multiplier as a separate
    /// f32 multiply after the angle ratio, so folding it into the field
    /// changes rounding order by ULPs -- near-identical, not bit-identical.
    pub(crate) fn with_edge_target_lengths(
        mesh: &MethodCDelaunayMesh,
        topology: &IcosahedronSpringTopology,
        movable_m_points: &[bool],
        edge_targets_m: &[f64],
        project_to_radius: bool,
    ) -> Option<Self> {
        let (moveu, compu) = build_nest_move_masks(topology, movable_m_points)?;
        let edge_count = topology.edge_m_points.len();
        if edge_targets_m.len() < edge_count {
            return None;
        }

        let mut dmin = f32::INFINITY;
        for edge_id in 2..edge_count {
            if moveu[edge_id] {
                let target = edge_targets_m[edge_id];
                if !target.is_finite() || target <= 0.0 {
                    return None;
                }
                let target_f32 = target as f32;
                if target_f32 < dmin {
                    dmin = target_f32;
                }
            }
        }
        if !dmin.is_finite() {
            // No movable edges: the driver returns early in that case, but keep
            // the floor well-defined.
            dmin = 1.0;
        }
        let min_area_squared = 0.1875_f32 * dmin.powi(4);

        let mut target_level_base = vec![0.0_f32; edge_count];
        let target_mrow_multiplier = vec![1.0_f32; edge_count];
        for edge_id in 2..edge_count {
            if !moveu[edge_id] {
                continue;
            }
            target_level_base[edge_id] = (edge_targets_m[edge_id] as f32) / 1.2;
        }

        let radius = if project_to_radius {
            Some(active_mesh_radius(mesh).ok()?)
        } else {
            None
        };

        Some(Self {
            moveu,
            compu,
            target_level_base,
            target_mrow_multiplier,
            min_area_squared,
            radius,
            edge_vectors: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
            edge_distances: vec![0.0_f64; edge_count],
            edge_displacements: vec![CartesianPoint::new(0.0, 0.0, 0.0); edge_count],
        })
    }
}

/// One Method-C nest-spring Jacobi iteration, writing into `updated_m_points`.
///
/// `compu` edge slots are fully rewritten before being read each iteration,
/// non-`compu` slots retain their unused zero placeholders, and
/// unmovable/dummy point slots are never written.
pub(crate) fn method_c_nest_spring_iteration_into(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    movable_m_points: &[bool],
    scratch: &mut MethodCNestSpringScratch,
    updated_m_points: &mut [CartesianPoint],
) -> Option<()> {
    // Destructure once: routing every hot-loop slice access through
    // `scratch.field` projections degraded inlining/alias analysis (profiles
    // showed `SliceIndex::get` surfacing as a real call); independent local
    // bindings restore the codegen of the former stack-local buffers.
    let MethodCNestSpringScratch {
        moveu,
        compu,
        target_level_base,
        target_mrow_multiplier,
        min_area_squared,
        radius,
        edge_vectors,
        edge_distances,
        edge_displacements,
    } = scratch;
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
        || movable_m_points.len() != m_points.len()
        || moveu.len() != topology.edge_m_points.len()
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
            if !compu[edge_id] {
                return Some(());
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
            *edge_vector = CartesianPoint::new(dx as f64, dy as f64, dz as f64);
            *edge_distance = distance as f64;
            Some(())
        })?;

    edge_displacements
        .par_iter_mut()
        .enumerate()
        .skip(2)
        .try_for_each(|(edge_id, edge_displacement)| {
            if !moveu[edge_id] {
                return Some(());
            }
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

            let mut target_distance = target_level_base[edge_id] * angle_ratio;
            target_distance *= target_mrow_multiplier[edge_id];

            let s1 = 0.5 * (dist + dist1 + dist2);
            let s2 = 0.5 * (dist + dist3 + dist4);
            let area1_squared = s1 * (s1 - dist) * (s1 - dist1) * (s1 - dist2);
            let area2_squared = s2 * (s2 - dist) * (s2 - dist3) * (s2 - dist4);
            let min_local_area_squared = area1_squared.min(area2_squared);
            if min_local_area_squared <= 0.0 || !min_local_area_squared.is_finite() {
                return None;
            }
            let area_ratio = (*min_area_squared / min_local_area_squared).max(1.0);
            target_distance *= area_ratio;

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
            if !movable_m_points[im] {
                return Some(());
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

            if let Some(radius) = *radius {
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
