use super::*;
use crate::method_c_nest_spring_iteration::method_c_nest_mrow_distance_multiplier;

/// Read-only diagnostics for one Method-C nest-spring generation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MethodCNestSpringDiagnostics {
    pub ngr: usize,
    pub generation_m_points: usize,
    pub movable_m_points: usize,
    pub movable_edges: usize,
    pub shaped_movable_edges: usize,
    pub movable_adjacent_hex_cells: usize,
    /// Stable Voronoi W-cell lineages for movable M points and their direct
    /// M-neighbors. Populated only for opt-in measurement runs.
    pub movable_adjacent_hex_cell_lineages: Vec<i64>,
}

#[derive(Clone, Debug)]
pub struct MethodCHfieldNestSpringFailure {
    pub iteration: usize,
    pub niter: usize,
    pub ngr: usize,
    pub preserve_mrow: bool,
    pub reason: String,
    pub edge_id: Option<usize>,
    pub adjacent_area_squared: Option<[f32; 2]>,
    pub target_min_distance: f32,
    pub min_area_squared: f32,
}

impl std::fmt::Display for MethodCHfieldNestSpringFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "failed to run Method-C h-field nest spring iteration {}/{} \
             for NGR {} preserve_mrow={}: {}; dmin={} min_area_squared={}",
            self.iteration,
            self.niter,
            self.ngr,
            self.preserve_mrow,
            self.reason,
            self.target_min_distance,
            self.min_area_squared
        )
    }
}

impl std::error::Error for MethodCHfieldNestSpringFailure {}

#[derive(Clone, Debug)]
pub struct MethodCHfieldSpringTrace {
    pub failure_iteration: usize,
    pub failure_edge_id: usize,
    pub triangle_side: usize,
    pub triangle_edge_ids: [usize; 3],
    pub triangle_m_point_ids: [usize; 3],
    pub samples: Vec<MethodCHfieldSpringTraceSample>,
}

#[derive(Clone, Debug)]
pub struct MethodCHfieldSpringTraceSample {
    /// Geometry entering this Jacobi iteration.
    pub iteration: usize,
    pub heron_area_squared: f32,
    /// Accepted point movement produced by this iteration. The failing
    /// iteration has no accepted step.
    pub applied_vertex_step_m: Option<[f64; 3]>,
    pub edges: [MethodCHfieldSpringTraceEdge; 3],
}

#[derive(Clone, Debug)]
pub struct MethodCHfieldSpringTraceEdge {
    pub edge_id: usize,
    pub mrlu: usize,
    pub mrow: [isize; 2],
    pub mrow_multiplier: f64,
    pub raw_target_m: f64,
    pub nominal_target_m: f64,
    pub current_length_m: f64,
    pub target_over_nominal: f64,
    pub current_over_target: f64,
    pub angle_ratio: f64,
    pub adjacent_area_squared: [f64; 2],
    pub min_area_over_floor: f64,
    pub area_ratio: Option<f64>,
    pub solver_target_before_area_m: f64,
    pub solver_target_m: Option<f64>,
    pub current_over_solver_target: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct MethodCNestSpringStepGuardDiagnostics {
    pub backtracked_iterations: usize,
    pub total_halvings: usize,
    pub max_halvings: usize,
}

impl MethodCDelaunayMesh {
    pub fn nest_spring_diagnostics(
        &self,
        ngr: usize,
        move_interior: bool,
        include_lineages: bool,
    ) -> io::Result<MethodCNestSpringDiagnostics> {
        self.validate_topology()?;
        let movable = method_c_nest_movable_m_points(self, ngr, move_interior)?;
        let generation_m_points = (2..=self.nmd)
            .filter(|&im| self.m_metadata[im].ngr == ngr)
            .count();
        let movable_m_points = movable.iter().skip(2).filter(|&&value| value).count();
        let mut movable_edges = 0usize;
        let mut shaped_movable_edges = 0usize;
        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            if !movable[edge.im[0]] && !movable[edge.im[1]] {
                continue;
            }
            movable_edges += 1;
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if method_c_nest_mrow_distance_multiplier(
                self.w_faces[iw1].mrow,
                self.w_faces[iw2].mrow,
            ) != 1.0
            {
                shaped_movable_edges += 1;
            }
        }

        let mut adjacent = std::collections::BTreeSet::new();
        for im in 2..=self.nmd {
            if !movable[im] {
                continue;
            }
            adjacent.insert(im);
            let neighbors = self.m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C nest spring adjacent U edge", iu, self.nud)?;
                let edge = self.u_edges[iu];
                let neighbor = if edge.im[0] == im {
                    edge.im[1]
                } else {
                    edge.im[0]
                };
                require_method_c_id("Method-C nest spring adjacent M point", neighbor, self.nmd)?;
                adjacent.insert(neighbor);
            }
        }
        let movable_adjacent_hex_cell_lineages = if include_lineages {
            adjacent
                .iter()
                .map(|&im| {
                    i64::try_from(self.m_lineage[im]).map_err(|_| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Method-C M lineage does not fit i64",
                        )
                    })
                })
                .collect::<io::Result<Vec<_>>>()?
        } else {
            Vec::new()
        };

        Ok(MethodCNestSpringDiagnostics {
            ngr,
            generation_m_points,
            movable_m_points,
            movable_edges,
            shaped_movable_edges,
            movable_adjacent_hex_cells: adjacent.len(),
            movable_adjacent_hex_cell_lineages,
        })
    }

    /// Apply the core Method-C `spring_dynamics_nest` relaxation to a refined nest.
    ///
    /// With `move_interior=false` this mirrors Method-C's atmospheric nest call:
    /// only M points adjacent to transition-row faces with nonzero `mrow` move. With
    /// `move_interior=true`, M points adjacent to faces on `ngr` are also moved.
    pub fn spring_nest(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
    ) -> io::Result<Self> {
        self.spring_nest_with_radius_projection(nxp, niter, ngr, move_interior, true, None)
    }

    pub(crate) fn spring_nest_with_radius_projection(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        dist00_override: Option<f64>,
    ) -> io::Result<Self> {
        self.spring_nest_with_radius_projection_impl(
            nxp,
            niter,
            ngr,
            move_interior,
            project_to_radius,
            dist00_override,
            false,
        )
        .map(|(mesh, _)| mesh)
    }

    pub fn spring_nest_guarded(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
    ) -> io::Result<(Self, MethodCNestSpringStepGuardDiagnostics)> {
        self.spring_nest_with_radius_projection_impl(
            nxp,
            niter,
            ngr,
            move_interior,
            true,
            None,
            true,
        )
    }

    fn spring_nest_with_radius_projection_impl(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        dist00_override: Option<f64>,
        guard_steps: bool,
    ) -> io::Result<(Self, MethodCNestSpringStepGuardDiagnostics)> {
        if nxp == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C nest spring requires positive NXP",
            ));
        }
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C nest spring NGR must be greater than one",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok((
                self.clone(),
                MethodCNestSpringStepGuardDiagnostics::default(),
            ));
        }

        let movable_m_points = method_c_nest_movable_m_points(self, ngr, move_interior)?;
        if movable_m_points.iter().skip(2).all(|movable| !*movable) {
            return Ok((
                self.clone(),
                MethodCNestSpringStepGuardDiagnostics::default(),
            ));
        }

        let radius = active_mesh_radius(self)?;
        let topology = icosahedron_spring_topology_canonical(
            self.nmd,
            &self.u_edges,
            &self.m_neighbors,
            0.035,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to build Method-C nest spring topology",
            )
        })?;
        let dist00 = dist00_override.unwrap_or(method_c_canonical_global_dist00(1.0, radius, nxp));
        // Loop-invariant masks/targets + reusable buffers, hoisted out of the
        // per-iteration hot path (bit-identical; see MethodCNestSpringScratch).
        let mut scratch = MethodCNestSpringScratch::new(
            self,
            &topology,
            &movable_m_points,
            dist00,
            project_to_radius,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to prepare Method-C nest spring buffers",
            )
        })?;
        // Double buffering: unmovable/dummy slots are never written by the
        // iteration, so both buffers keep their initial positions there
        // forever, exactly like the historical clone-per-iteration version.
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();
        let guard_faces = guard_steps
            .then(|| method_c_nest_guard_faces(&topology, &scratch.moveu))
            .transpose()?;
        let mut guard_diagnostics = MethodCNestSpringStepGuardDiagnostics::default();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("method_c-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Method-C nest spring was cancelled",
                ));
            }
            if let Err(failure) = method_c_nest_spring_iteration_into(
                &m_points,
                &topology,
                &movable_m_points,
                &mut scratch,
                &mut next_m_points,
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "failed to run Method-C nest spring iteration {iteration}/{niter} \
                         for NGR {ngr}: {failure}; dmin={} min_area_squared={}",
                        scratch.target_min_distance, scratch.min_area_squared
                    ),
                ));
            }
            if let Some(faces) = &guard_faces {
                let halvings = method_c_guard_nest_spring_step(
                    &m_points,
                    &mut next_m_points,
                    &movable_m_points,
                    faces,
                    scratch.radius,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "Method-C guarded nest spring failed at iteration \
                             {iteration}/{niter} for NGR {ngr}: {error}"
                        ),
                    )
                })?;
                if halvings > 0 {
                    guard_diagnostics.backtracked_iterations += 1;
                    guard_diagnostics.total_halvings += halvings;
                    guard_diagnostics.max_halvings = guard_diagnostics.max_halvings.max(halvings);
                }
            }
            std::mem::swap(&mut m_points, &mut next_m_points);
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            m_lineage: self.m_lineage.clone(),
            next_m_lineage: self.next_m_lineage,
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            w_lineage: self.w_lineage.clone(),
            next_w_lineage: self.next_w_lineage,
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok((adjusted, guard_diagnostics))
    }

    /// H-field variant of [`Self::spring_nest`]: per-edge target lengths come
    /// from the caller (typically sampled from an `earthmesh_hfield` cell-width
    /// field via [`method_c_edge_target_lengths_from_field`]) instead of the
    /// level/mrow-derived spacing. Movable-point selection, the Jacobi
    /// iteration structure, the trailing default-real rounding, and topology
    /// validation are identical to the standard path, which stays untouched as
    /// the compat default.
    pub fn spring_nest_with_edge_targets(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
    ) -> io::Result<Self> {
        self.spring_nest_with_edge_targets_mrow(
            niter,
            ngr,
            move_interior,
            project_to_radius,
            edge_targets_m,
            false,
        )
    }

    /// Diagnostic H-field variant that keeps Method-C's transition-row
    /// multiplier separate from the continuous edge target.
    pub fn spring_nest_with_edge_targets_preserving_mrow(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
    ) -> io::Result<Self> {
        self.spring_nest_with_edge_targets_mrow(
            niter,
            ngr,
            move_interior,
            project_to_radius,
            edge_targets_m,
            true,
        )
    }

    fn spring_nest_with_edge_targets_mrow(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
        preserve_mrow: bool,
    ) -> io::Result<Self> {
        self.spring_nest_with_edge_targets_mrow_impl(
            niter,
            ngr,
            move_interior,
            project_to_radius,
            edge_targets_m,
            preserve_mrow,
            false,
        )
        .map(|(mesh, _)| mesh)
    }

    pub fn spring_nest_with_edge_targets_guarded(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
        preserve_mrow: bool,
    ) -> io::Result<(Self, MethodCNestSpringStepGuardDiagnostics)> {
        self.spring_nest_with_edge_targets_mrow_impl(
            niter,
            ngr,
            move_interior,
            project_to_radius,
            edge_targets_m,
            preserve_mrow,
            true,
        )
    }

    fn spring_nest_with_edge_targets_mrow_impl(
        &self,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
        preserve_mrow: bool,
        guard_steps: bool,
    ) -> io::Result<(Self, MethodCNestSpringStepGuardDiagnostics)> {
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C nest spring NGR must be greater than one",
            ));
        }
        if edge_targets_m.len() < self.nud + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "edge target lengths ({}) must cover Canonical U ids 0..={}",
                    edge_targets_m.len(),
                    self.nud
                ),
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok((
                self.clone(),
                MethodCNestSpringStepGuardDiagnostics::default(),
            ));
        }

        let movable_m_points = method_c_nest_movable_m_points(self, ngr, move_interior)?;
        if movable_m_points.iter().skip(2).all(|movable| !*movable) {
            return Ok((
                self.clone(),
                MethodCNestSpringStepGuardDiagnostics::default(),
            ));
        }

        let topology = icosahedron_spring_topology_canonical(
            self.nmd,
            &self.u_edges,
            &self.m_neighbors,
            0.035,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to build Method-C nest spring topology",
            )
        })?;
        let mut scratch = MethodCNestSpringScratch::with_edge_target_lengths(
            self,
            &topology,
            &movable_m_points,
            edge_targets_m,
            project_to_radius,
            preserve_mrow,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to prepare Method-C h-field nest spring buffers",
            )
        })?;
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();
        let guard_faces = guard_steps
            .then(|| method_c_nest_guard_faces(&topology, &scratch.moveu))
            .transpose()?;
        let mut guard_diagnostics = MethodCNestSpringStepGuardDiagnostics::default();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("method_c-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Method-C nest spring was cancelled",
                ));
            }
            if let Err(failure) = method_c_nest_spring_iteration_into(
                &m_points,
                &topology,
                &movable_m_points,
                &mut scratch,
                &mut next_m_points,
            ) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    MethodCHfieldNestSpringFailure {
                        iteration,
                        niter,
                        ngr,
                        preserve_mrow,
                        reason: failure.to_string(),
                        edge_id: failure.edge_id(),
                        adjacent_area_squared: failure.adjacent_area_squared(),
                        target_min_distance: scratch.target_min_distance,
                        min_area_squared: scratch.min_area_squared,
                    },
                ));
            }
            if let Some(faces) = &guard_faces {
                let halvings = method_c_guard_nest_spring_step(
                    &m_points,
                    &mut next_m_points,
                    &movable_m_points,
                    faces,
                    scratch.radius,
                )
                .map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "Method-C guarded h-field nest spring failed at iteration \
                             {iteration}/{niter} for NGR {ngr}: {error}"
                        ),
                    )
                })?;
                if halvings > 0 {
                    guard_diagnostics.backtracked_iterations += 1;
                    guard_diagnostics.total_halvings += halvings;
                    guard_diagnostics.max_halvings = guard_diagnostics.max_halvings.max(halvings);
                }
            }
            std::mem::swap(&mut m_points, &mut next_m_points);
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            m_lineage: self.m_lineage.clone(),
            next_m_lineage: self.next_m_lineage,
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            w_lineage: self.w_lineage.clone(),
            next_w_lineage: self.next_w_lineage,
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok((adjusted, guard_diagnostics))
    }

    /// Replay a known H-field spring failure and retain only the requested
    /// failing triangle's final input states.
    pub fn trace_hfield_nest_spring_failure(
        &self,
        failure: &MethodCHfieldNestSpringFailure,
        move_interior: bool,
        project_to_radius: bool,
        edge_targets_m: &[f64],
        base_m: f64,
        window: usize,
    ) -> io::Result<MethodCHfieldSpringTrace> {
        let failure_edge_id = failure.edge_id.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "H-field spring failure has no U edge to trace",
            )
        })?;
        let adjacent_area_squared = failure.adjacent_area_squared.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "H-field spring failure has no adjacent triangle areas to trace",
            )
        })?;
        if window == 0 || failure.iteration == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "H-field spring trace requires a positive window and failure iteration",
            ));
        }
        self.validate_topology()?;
        let movable_m_points = method_c_nest_movable_m_points(self, failure.ngr, move_interior)?;
        let topology = icosahedron_spring_topology_canonical(
            self.nmd,
            &self.u_edges,
            &self.m_neighbors,
            0.035,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to build Method-C nest spring topology for failure trace",
            )
        })?;
        let mut scratch = MethodCNestSpringScratch::with_edge_target_lengths(
            self,
            &topology,
            &movable_m_points,
            edge_targets_m,
            project_to_radius,
            failure.preserve_mrow,
        )
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to prepare Method-C h-field failure trace buffers",
            )
        })?;
        let triangle_side = usize::from(adjacent_area_squared[1] < adjacent_area_squared[0]);
        let [iu1, iu2, iu3, iu4] =
            *topology
                .edge_neighbor_u
                .get(failure_edge_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "H-field spring failure edge has no triangle neighbors",
                    )
                })?;
        let triangle_edge_ids = if triangle_side == 0 {
            [failure_edge_id, iu1, iu2]
        } else {
            [failure_edge_id, iu3, iu4]
        };
        let mut triangle_m_point_ids = triangle_edge_ids
            .iter()
            .flat_map(|&edge_id| topology.edge_m_points[edge_id])
            .collect::<Vec<_>>();
        triangle_m_point_ids.sort_unstable();
        triangle_m_point_ids.dedup();
        let triangle_m_point_ids: [usize; 3] =
            triangle_m_point_ids.try_into().map_err(|ids: Vec<usize>| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "H-field spring failure triangle has {} M points instead of 3",
                        ids.len()
                    ),
                )
            })?;
        let first_sample = failure.iteration.saturating_sub(window - 1).max(1);
        let mut samples = Vec::with_capacity(failure.iteration - first_sample + 1);
        let mut m_points = self.m_points.clone();
        let mut next_m_points = self.m_points.clone();

        for iteration in 1..=failure.iteration {
            if iteration >= first_sample {
                samples.push(method_c_hfield_trace_sample(
                    self,
                    &m_points,
                    &topology,
                    &scratch,
                    triangle_edge_ids,
                    edge_targets_m,
                    base_m,
                    iteration,
                )?);
            }
            match method_c_nest_spring_iteration_into(
                &m_points,
                &topology,
                &movable_m_points,
                &mut scratch,
                &mut next_m_points,
            ) {
                Ok(()) => {
                    if iteration >= first_sample {
                        samples
                            .last_mut()
                            .expect("trace sample was just appended")
                            .applied_vertex_step_m = Some(triangle_m_point_ids.map(|im| {
                            let before = m_points[im];
                            let after = next_m_points[im];
                            let dx = after.x - before.x;
                            let dy = after.y - before.y;
                            let dz = after.z - before.z;
                            (dx * dx + dy * dy + dz * dz).sqrt()
                        }));
                    }
                    std::mem::swap(&mut m_points, &mut next_m_points);
                }
                Err(replayed) => {
                    if iteration != failure.iteration || replayed.edge_id() != Some(failure_edge_id)
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "H-field spring replay diverged at iteration {iteration}: \
                                 {replayed}"
                            ),
                        ));
                    }
                    return Ok(MethodCHfieldSpringTrace {
                        failure_iteration: failure.iteration,
                        failure_edge_id,
                        triangle_side,
                        triangle_edge_ids,
                        triangle_m_point_ids,
                        samples,
                    });
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "H-field spring replay did not reproduce the recorded failure",
        ))
    }
}

fn method_c_nest_guard_faces(
    topology: &IcosahedronSpringTopology,
    moveu: &[bool],
) -> io::Result<Vec<[usize; 3]>> {
    let mut faces = std::collections::BTreeSet::new();
    for (edge_id, &movable) in moveu.iter().enumerate().skip(2) {
        if !movable {
            continue;
        }
        let [iu1, iu2, iu3, iu4] = *topology.edge_neighbor_u.get(edge_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Method-C spring guard is missing neighbors for U edge {edge_id}"),
            )
        })?;
        for triangle_edges in [[edge_id, iu1, iu2], [edge_id, iu3, iu4]] {
            let mut triangle = triangle_edges
                .iter()
                .flat_map(|&iu| topology.edge_m_points.get(iu).copied())
                .flatten()
                .collect::<Vec<_>>();
            triangle.sort_unstable();
            triangle.dedup();
            let triangle: [usize; 3] = triangle.try_into().map_err(|points: Vec<usize>| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C spring guard triangle for U edge {edge_id} has {} M points",
                        points.len()
                    ),
                )
            })?;
            faces.insert(triangle);
        }
    }
    Ok(faces.into_iter().collect())
}

fn method_c_triangle_area_squared(
    m_points: &[CartesianPoint],
    [im1, im2, im3]: [usize; 3],
) -> Option<f32> {
    let distance = |a: usize, b: usize| {
        let point1 = *m_points.get(a)?;
        let point2 = *m_points.get(b)?;
        let dx = (point2.x - point1.x) as f32;
        let dy = (point2.y - point1.y) as f32;
        let dz = (point2.z - point1.z) as f32;
        Some((dx * dx + dy * dy + dz * dz).sqrt())
    };
    let [a, b, c] = [
        distance(im1, im2)?,
        distance(im2, im3)?,
        distance(im3, im1)?,
    ];
    let s = 0.5 * (a + b + c);
    Some(s * (s - a) * (s - b) * (s - c))
}

fn method_c_guard_nest_spring_step(
    current: &[CartesianPoint],
    candidate: &mut [CartesianPoint],
    movable_m_points: &[bool],
    faces: &[[usize; 3]],
    radius: Option<f64>,
) -> io::Result<usize> {
    let valid = |points: &[CartesianPoint]| {
        faces.iter().all(|&face| {
            let Some(candidate_area) = method_c_triangle_area_squared(points, face) else {
                return false;
            };
            candidate_area.is_finite() && candidate_area > 0.0
        })
    };
    if valid(candidate) {
        return Ok(0);
    }

    let proposed = candidate.to_vec();
    for halvings in 1usize..=16 {
        let fraction = 0.5_f64.powi(halvings as i32);
        for im in 2..candidate.len() {
            if !movable_m_points[im] {
                continue;
            }
            let before = current[im];
            let after = proposed[im];
            let mut point = CartesianPoint::new(
                before.x + fraction * (after.x - before.x),
                before.y + fraction * (after.y - before.y),
                before.z + fraction * (after.z - before.z),
            );
            if let Some(radius) = radius {
                let norm = magnitude(point);
                if norm == 0.0 || !norm.is_finite() {
                    continue;
                }
                let expansion = radius / norm;
                point = CartesianPoint::new(
                    point.x * expansion,
                    point.y * expansion,
                    point.z * expansion,
                );
            }
            candidate[im] = point;
        }
        if valid(candidate) {
            return Ok(halvings);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "Method-C nest spring step guard could not preserve positive triangle area",
    ))
}

fn method_c_hfield_trace_sample(
    mesh: &MethodCDelaunayMesh,
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    scratch: &MethodCNestSpringScratch,
    triangle_edge_ids: [usize; 3],
    edge_targets_m: &[f64],
    base_m: f64,
    iteration: usize,
) -> io::Result<MethodCHfieldSpringTraceSample> {
    let edge = |edge_id: usize| -> io::Result<MethodCHfieldSpringTraceEdge> {
        let edge = *mesh.u_edges.get(edge_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace is missing U edge {edge_id}"),
            )
        })?;
        let point1 = *m_points.get(edge.im[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no first M point"),
            )
        })?;
        let point2 = *m_points.get(edge.im[1]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no second M point"),
            )
        })?;
        let dx = (point2.x - point1.x) as f32;
        let dy = (point2.y - point1.y) as f32;
        let dz = (point2.z - point1.z) as f32;
        let current_length_m = (dx * dx + dy * dy + dz * dz).sqrt() as f64;
        let raw_target_m = *edge_targets_m.get(edge_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no target"),
            )
        })?;
        let face1 = *mesh.w_faces.get(edge.iw[0]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no first W face"),
            )
        })?;
        let face2 = *mesh.w_faces.get(edge.iw[1]).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no second W face"),
            )
        })?;
        let mrow_multiplier = scratch.target_mrow_multiplier[edge_id] as f64;
        let nominal_target_m = base_m / 2.0_f64.powi(edge.mrlu.max(1) as i32 - 1);
        let [iu1, iu2, iu3, iu4] = *topology.edge_neighbor_u.get(edge_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("H-field spring trace edge {edge_id} has no neighbor edges"),
            )
        })?;
        let distance = |neighbor_edge_id: usize| -> io::Result<f32> {
            let [im1, im2] = *topology
                .edge_m_points
                .get(neighbor_edge_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "H-field spring trace is missing neighbor U edge {neighbor_edge_id}"
                        ),
                    )
                })?;
            let point1 = m_points[im1];
            let point2 = m_points[im2];
            let dx = (point2.x - point1.x) as f32;
            let dy = (point2.y - point1.y) as f32;
            let dz = (point2.z - point1.z) as f32;
            Ok((dx * dx + dy * dy + dz * dz).sqrt())
        };
        let dist = current_length_m as f32;
        let [dist1, dist2, dist3, dist4] = [
            distance(iu1)?,
            distance(iu2)?,
            distance(iu3)?,
            distance(iu4)?,
        ];
        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let angle_ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        let s1 = 0.5 * (dist + dist1 + dist2);
        let s2 = 0.5 * (dist + dist3 + dist4);
        let area1_squared = s1 * (s1 - dist) * (s1 - dist1) * (s1 - dist2);
        let area2_squared = s2 * (s2 - dist) * (s2 - dist3) * (s2 - dist4);
        let min_local_area_squared = area1_squared.min(area2_squared);
        let min_area_over_floor = min_local_area_squared as f64 / scratch.min_area_squared as f64;
        let area_ratio = (min_local_area_squared > 0.0 && min_local_area_squared.is_finite())
            .then(|| (scratch.min_area_squared / min_local_area_squared).max(1.0) as f64);
        let solver_target_before_area_m = (scratch.target_level_base[edge_id]
            * angle_ratio
            * scratch.target_mrow_multiplier[edge_id])
            as f64;
        let solver_target_m = area_ratio.map(|ratio| solver_target_before_area_m * ratio);
        Ok(MethodCHfieldSpringTraceEdge {
            edge_id,
            mrlu: edge.mrlu,
            mrow: [face1.mrow, face2.mrow],
            mrow_multiplier,
            raw_target_m,
            nominal_target_m,
            current_length_m,
            target_over_nominal: raw_target_m / nominal_target_m,
            current_over_target: current_length_m / raw_target_m,
            angle_ratio: angle_ratio as f64,
            adjacent_area_squared: [area1_squared as f64, area2_squared as f64],
            min_area_over_floor,
            area_ratio,
            solver_target_before_area_m,
            solver_target_m,
            current_over_solver_target: solver_target_m.map(|target| current_length_m / target),
        })
    };
    let edges = [
        edge(triangle_edge_ids[0])?,
        edge(triangle_edge_ids[1])?,
        edge(triangle_edge_ids[2])?,
    ];
    let [a, b, c] = [
        edges[0].current_length_m as f32,
        edges[1].current_length_m as f32,
        edges[2].current_length_m as f32,
    ];
    let s = 0.5 * (a + b + c);
    Ok(MethodCHfieldSpringTraceSample {
        iteration,
        heron_area_squared: s * (s - a) * (s - b) * (s - c),
        applied_vertex_step_m: None,
        edges,
    })
}

/// Sample per-edge target lengths for
/// [`MethodCDelaunayMesh::spring_nest_with_edge_targets`] from a
/// `(lon_degrees, lat_degrees) -> meters` closure, evaluated at each active U
/// edge's chordal midpoint (dateline-safe by construction: the midpoint is
/// averaged in Cartesian space before converting to lon/lat). Inactive
/// placeholder edges keep a `0.0` target, which is fine because only movable
/// edges are ever read and validated by the scratch builder.
pub fn method_c_edge_target_lengths_from_field<F: Fn(f64, f64) -> f64>(
    mesh: &MethodCDelaunayMesh,
    target_m: F,
) -> io::Result<Vec<f64>> {
    let mut targets = vec![0.0_f64; mesh.nud + 1];
    for iu in 2..=mesh.nud {
        let edge = mesh.u_edges[iu];
        let [im1, im2] = edge.im;
        if im1 <= 1 || im2 <= 1 || im1 > mesh.nmd || im2 > mesh.nmd {
            continue;
        }
        let p1 = mesh.m_points[im1];
        let p2 = mesh.m_points[im2];
        let midpoint = CartesianPoint::new(
            0.5 * (p1.x + p2.x),
            0.5 * (p1.y + p2.y),
            0.5 * (p1.z + p2.z),
        );
        let lonlat = xyz_to_lonlat_degrees(midpoint);
        let target = target_m(lonlat.lon_degrees, lonlat.lat_degrees);
        if !target.is_finite() || target <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("edge {iu} target length {target} must be positive and finite"),
            ));
        }
        targets[iu] = target;
    }
    Ok(targets)
}

pub(crate) fn method_c_nest_movable_m_points(
    mesh: &MethodCDelaunayMesh,
    ngr: usize,
    move_interior: bool,
) -> io::Result<Vec<bool>> {
    let mut movable = vec![false; mesh.nmd + 1];

    for im in 2..=mesh.nmd {
        if mesh.m_metadata[im].ngr != ngr {
            continue;
        }

        if move_interior {
            movable[im] = true;
            continue;
        }

        let neighbors = mesh.m_neighbors[im];
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C nest spring movable W face", iw, mesh.nwd)?;
            if mesh.w_faces[iw].mrow != 0 {
                movable[im] = true;
                break;
            }
        }
    }

    Ok(movable)
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    #[test]
    fn guard_uses_the_same_two_triangles_as_the_spring_stencil() {
        let mut topology = IcosahedronSpringTopology {
            edge_m_points: vec![[1, 1], [1, 1], [2, 3], [2, 4], [3, 4], [2, 5], [3, 5]],
            edge_neighbor_u: vec![[1, 1, 1, 1]; 7],
            m_npoly: Vec::new(),
            m_u_edges: Vec::new(),
            directions: Vec::new(),
        };
        topology.edge_neighbor_u[2] = [3, 4, 5, 6];
        let mut moveu = vec![false; 7];
        moveu[2] = true;

        assert_eq!(
            method_c_nest_guard_faces(&topology, &moveu).unwrap(),
            vec![[2, 3, 4], [2, 3, 5]]
        );
    }
}
