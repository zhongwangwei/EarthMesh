use std::time::Instant;

use crate::method_c_perimeter_repair::method_c_parent_support_error;

use super::*;

impl MethodCDelaunayMesh {
    fn method_c_parent_support_for_mask(
        &self,
        selected: &[bool],
        perimeter: &[MethodCPerimeterPoint],
    ) -> io::Result<BTreeSet<usize>> {
        self.required_parent_support_lineages_from_selected_and_perimeter(selected, perimeter)?
            .into_iter()
            .map(|lineage| {
                usize::try_from(lineage).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C parent support lineage {lineage} is negative"),
                    )
                })
            })
            .collect()
    }

    fn method_c_parent_support_for_current_mask(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<BTreeSet<usize>> {
        let Ok(perimeters) = self.method_c_perimeters_from_selected_faces(selected, m_neighbors)
        else {
            return Ok(BTreeSet::new());
        };
        self.method_c_parent_support_for_mask(
            selected,
            &perimeters.into_iter().flatten().collect::<Vec<_>>(),
        )
    }

    pub(crate) fn method_c_parent_u_dependency_faces(
        &self,
        parent_u: usize,
        method_c_m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> Vec<usize> {
        let Some(edge) = self.u_edges.get(parent_u) else {
            return Vec::new();
        };
        let mut faces = std::collections::BTreeSet::new();
        let mut add_m_ring = |im: usize| {
            if let Some(neighbors) = method_c_m_neighbors.get(im) {
                faces.extend(
                    neighbors
                        .iw
                        .iter()
                        .take(neighbors.npoly)
                        .copied()
                        .filter(|&iw| (2..=self.nwd).contains(&iw)),
                );
            }
        };
        for im in edge.im {
            add_m_ring(im);
        }
        faces.extend(
            edge.iw
                .into_iter()
                .filter(|&iw| (2..=self.nwd).contains(&iw)),
        );
        for iu in edge.iu {
            if let Some(neighbor) = self.u_edges.get(iu) {
                faces.extend(
                    neighbor
                        .iw
                        .into_iter()
                        .filter(|&iw| (2..=self.nwd).contains(&iw)),
                );
            }
        }
        faces.into_iter().collect()
    }

    pub(crate) fn method_c_repair_witness_dependency_faces(
        &self,
        error: &io::Error,
        method_c_m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> Vec<usize> {
        let Some(payload) = method_c_repairable_payload(error) else {
            return Vec::new();
        };
        let mut faces = std::collections::BTreeSet::new();
        let mut add_m_ring = |im: usize| {
            if let Some(neighbors) = method_c_m_neighbors.get(im) {
                faces.extend(
                    neighbors
                        .iw
                        .iter()
                        .take(neighbors.npoly)
                        .copied()
                        .filter(|&iw| (2..=self.nwd).contains(&iw)),
                );
            }
        };
        if let Some(im) = payload.parent_m_point {
            add_m_ring(im);
        }
        for &im in &payload.parent_m_valence_witnesses {
            add_m_ring(im);
        }
        if let Some(parent_u) = payload.parent_u_edge {
            faces.extend(self.method_c_parent_u_dependency_faces(parent_u, method_c_m_neighbors));
        }
        faces.into_iter().collect()
    }

    pub(crate) fn method_c_repair_candidate_preserves_coverage(
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
        candidate: &[bool],
    ) -> bool {
        coverage.is_none_or(|coverage| coverage.validate(candidate).is_ok())
    }

    pub(crate) fn emit_method_c_selected_faces(
        &self,
        selected: &[bool],
        method_c_m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        let perimeters =
            self.method_c_perimeters_from_selected_faces(selected, method_c_m_neighbors)?;
        if !Self::method_c_perimeters_are_triplets(&perimeters) {
            let perimeter_lengths = perimeters.iter().map(Vec::len).collect::<Vec<_>>();
            self.dump_method_c_unrepaired_mask(
                "no-repair-attempted",
                selected,
                &perimeter_lengths,
                child_level,
            );
            return Err(method_c_repairable_perimeter_error(
                MethodCRepairableKind::NonTripletPerimeter,
                perimeter_lengths.clone(),
                0,
                format!(
                    "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples",
                    perimeter_lengths
                ),
            ));
        }
        self.emit_method_c_selected_faces_with_perimeters(
            selected,
            method_c_m_neighbors,
            perimeters,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }

    fn emit_method_c_selected_faces_with_perimeters(
        &self,
        selected: &[bool],
        method_c_m_neighbors: &[IcosahedronMPointNeighbors],
        perimeters: Vec<Vec<MethodCPerimeterPoint>>,
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        let perimeter = perimeters.into_iter().flatten().collect::<Vec<_>>();
        let mut nest_wd =
            self.method_c_nest_wd_from_selected_and_perimeter(selected, &perimeter)?;
        self.emit_method_c_tables(
            &perimeter,
            method_c_m_neighbors,
            &mut nest_wd,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }

    /// Diagnostic-only exact materialization with an independently selected
    /// transition-triple start for each closed perimeter component.
    #[doc(hidden)]
    pub fn spawn_nest_pass_method_c_with_perimeter_component_offsets_for_diagnostics(
        &self,
        prepared_selected_faces: &[bool],
        component_offsets: &[usize],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_method_c_len(
            "prepared_selected_faces",
            prepared_selected_faces.len(),
            self.nwd + 1,
        )?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C child level must be greater than one",
            ));
        }
        self.ensure_method_c_selected_faces_share_parent_mrlw(
            prepared_selected_faces,
            child_level,
        )?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut perimeters = self.method_c_perimeters_from_selected_faces(
            prepared_selected_faces,
            &method_c_m_neighbors,
        )?;
        if !Self::method_c_perimeters_are_triplets(&perimeters) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C diagnostic perimeter components are not triplets",
            ));
        }
        if component_offsets.len() != perimeters.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Method-C diagnostic received {} perimeter offsets for {} components",
                    component_offsets.len(),
                    perimeters.len()
                ),
            ));
        }
        for (perimeter, &offset) in perimeters.iter_mut().zip(component_offsets) {
            if offset >= 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Method-C perimeter component offset must be 0, 1, or 2, got {offset}"),
                ));
            }
            perimeter.rotate_left(offset);
        }
        self.emit_method_c_selected_faces_with_perimeters(
            prepared_selected_faces,
            &method_c_m_neighbors,
            perimeters,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }

    pub(crate) fn spawn_nest_pass_with_max_mrows(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.spawn_nest_pass_method_c(selected_faces, child_level, max_mrows, project_to_radius)
    }

    pub(crate) fn spawn_nest_pass_method_c(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.spawn_nest_pass_method_c_repairing(
            selected_faces,
            child_level,
            max_mrows,
            project_to_radius,
            None,
        )
    }

    fn spawn_nest_pass_method_c_repairing(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C child level must be greater than one",
            ));
        }
        if let Some(coverage) = coverage {
            coverage.validate(selected_faces)?;
        }

        let mut selected = selected_faces.to_vec();
        let cross_level_support = std::env::var_os("EARTHMESH_M0_CROSS_LEVEL_SUPPORT").is_some();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        // Keep this pass clear of anything an earlier pass conceded. A conceded
        // region stays a generation behind for good, so a transition band that
        // reaches into it can never be satisfied by parent support.
        // `perim_fill3` consumes faces just outside the selection, hence the
        // two-ring margin.
        self.clear_method_c_conceded_margin(&mut selected, &method_c_m_neighbors, 2)?;
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        if let Some(coverage) = coverage {
            coverage.validate(&selected)?;
        }
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;

        let mut last_repairable_error = None;
        let mut attempted_masks = std::collections::HashSet::new();
        let detailed_trace = std::env::var("EARTHMESH_M0_REPAIR_TRACE")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true"));
        let max_repair_attempts = method_c_repair_attempt_limit(
            std::env::var("EARTHMESH_M0_REPAIR_MAX_ATTEMPTS")
                .ok()
                .as_deref(),
        );
        let report = |phase, done, total| {
            if earthmesh_core::progress::report(phase, done, total) {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    format!("Method-C {phase} cancelled"),
                ))
            }
        };
        let trace = |phase, done, total| {
            if detailed_trace {
                report(phase, done, total)
            } else {
                Ok(())
            }
        };
        let start_stage = || detailed_trace.then(Instant::now);
        let report_stage = |stage: &str, attempt: usize, started: Option<Instant>| {
            if let Some(started) = started {
                eprintln!(
                    "earthmesh_mesh: method_c repair stage={stage} attempt={} seconds={:.6}",
                    attempt + 1,
                    started.elapsed().as_secs_f64()
                );
            }
        };
        for attempt in 0..max_repair_attempts {
            report("method_c-mask-repair", attempt + 1, max_repair_attempts)?;
            trace(
                "method_c-mask-repair-attempt-start",
                attempt + 1,
                max_repair_attempts,
            )?;
            trace(
                "method_c-mask-repair-selected-faces",
                selected.iter().filter(|&&item| item).count(),
                self.nwd.saturating_sub(1),
            )?;
            trace(
                "method_c-mask-repair-non-triplet-start",
                attempt + 1,
                max_repair_attempts,
            )?;
            let stage_started = start_stage();
            let perimeter = match self.repair_method_c_non_triplet_perimeter_tracking_support(
                &mut selected,
                &method_c_m_neighbors,
                child_level,
                Some((max_mrows, project_to_radius)),
            ) {
                Ok(perimeter) => perimeter,
                Err(error) => {
                    let can_request_support = method_c_repairable_payload(&error)
                        .is_some_and(|payload| payload.kind != MethodCRepairableKind::Valence);
                    if cross_level_support && can_request_support {
                        let support = self.method_c_parent_support_for_current_mask(
                            &selected,
                            &method_c_m_neighbors,
                        )?;
                        if !support.is_empty() {
                            return Err(method_c_parent_support_error(support));
                        }
                    }
                    return Err(error);
                }
            };
            report_stage("non-triplet", attempt, stage_started);
            trace(
                "method_c-mask-repair-non-triplet-end",
                attempt + 1,
                max_repair_attempts,
            )?;
            if let Some(coverage) = coverage {
                coverage.validate(&selected)?;
            }
            if !attempted_masks.insert(selected.clone()) {
                if cross_level_support
                    && last_repairable_error.as_ref().is_some_and(|error| {
                        method_c_repairable_payload(error)
                            .is_some_and(|payload| payload.kind != MethodCRepairableKind::Valence)
                    })
                {
                    let support = self.method_c_parent_support_for_mask(&selected, &perimeter)?;
                    if !support.is_empty() {
                        return Err(method_c_parent_support_error(support));
                    }
                }
                if let Some(error) = last_repairable_error.as_ref() {
                    if let Some(im) = Self::method_c_valence_error_parent_m_point(error) {
                        if let Some((repaired, _)) = self.try_fill_method_c_specific_m_point(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            im,
                        )? {
                            if !attempted_masks.contains(&repaired) {
                                selected = repaired;
                                continue;
                            }
                        }
                    }
                }
                return Err(last_repairable_error.unwrap_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Method-C automatic perimeter repair repeated an unchanged mask",
                    )
                }));
            }
            let mut nest_wd =
                self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
            if detailed_trace {
                let witnesses =
                    self.method_c_transition_self_loop_witnesses(&perimeter, &nest_wd)?;
                let boundary_witnesses = self.method_c_transition_parent_boundary_witnesses(
                    &perimeter,
                    &nest_wd,
                    child_level - 1,
                )?;
                let boundary_details = boundary_witnesses
                    .iter()
                    .take(5)
                    .map(|(triple, faces)| {
                        (
                            *triple,
                            faces
                                .iter()
                                .map(|&iw| (iw, self.w_faces[iw].mrlw, nest_wd[iw].flag()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();
                eprintln!(
                    "earthmesh_mesh: method_c transition self-loop predictions={witnesses:?} parent-boundary predictions={boundary_witnesses:?} parent-boundary details={boundary_details:?}"
                );
            }
            trace(
                "method_c-mask-repair-emit-start",
                attempt + 1,
                max_repair_attempts,
            )?;
            let stage_started = start_stage();
            let emitted = self.emit_method_c_tables(
                &perimeter,
                &method_c_m_neighbors,
                &mut nest_wd,
                child_level,
                max_mrows,
                project_to_radius,
            );
            report_stage("emit", attempt, stage_started);
            trace(
                "method_c-mask-repair-emit-end",
                attempt + 1,
                max_repair_attempts,
            )?;
            match emitted {
                Ok(mesh) => {
                    trace(
                        "method_c-mask-repair-attempt-end",
                        attempt + 1,
                        max_repair_attempts,
                    )?;
                    if crate::method_c_spawn_hfield::coverage_relaxation_enabled() {
                        if let Some(coverage) = coverage {
                            let conceded = coverage.uncovered_anchors(&selected);
                            eprintln!(
                                "earthmesh_mesh: method_c coverage relaxation child_level={child_level} \
                                 attempt={} anchors={} conceded={} first={:?}",
                                attempt + 1,
                                coverage.anchor_count(),
                                conceded.len(),
                                conceded.iter().take(16).collect::<Vec<_>>()
                            );
                        }
                    }
                    return Ok(mesh);
                }
                Err(error) if Self::is_repairable_method_c_transition_error(&error) => {
                    if cross_level_support
                        && method_c_repairable_payload(&error).is_some_and(|payload| {
                            payload.kind == MethodCRepairableKind::TransitionPatch
                        })
                    {
                        let support =
                            self.method_c_parent_support_for_mask(&selected, &perimeter)?;
                        if !support.is_empty() {
                            return Err(method_c_parent_support_error(support));
                        }
                    }
                    let dependency_faces = detailed_trace.then(|| {
                        self.method_c_repair_witness_dependency_faces(&error, &method_c_m_neighbors)
                    });
                    if detailed_trace {
                        if let Some(payload) = method_c_repairable_payload(&error) {
                            let parent_u =
                                payload.parent_u_edge.and_then(|iu| self.u_edges.get(iu));
                            let parent_u_rings = parent_u
                                .into_iter()
                                .flat_map(|edge| edge.im)
                                .filter_map(|im| {
                                    method_c_m_neighbors.get(im).map(|neighbors| {
                                        let active = neighbors
                                            .iw
                                            .iter()
                                            .take(neighbors.npoly)
                                            .map(|&iw| selected.get(iw).copied().unwrap_or(false))
                                            .collect::<Vec<_>>();
                                        (
                                            im,
                                            active.iter().filter(|&&item| item).count(),
                                            crate::mask_postproc_waterway::cyclic_active_runs(
                                                &active,
                                            ),
                                            active,
                                        )
                                    })
                                })
                                .collect::<Vec<_>>();
                            let parent_u_face_flags = parent_u.map(|edge| {
                                edge.iw
                                    .map(|iw| nest_wd.get(iw).map_or(0, |face| face.flag()))
                            });
                            let parent_u_neighbor_edge_flags = parent_u
                                .into_iter()
                                .flat_map(|edge| edge.iu)
                                .filter_map(|iu| {
                                    self.u_edges.get(iu).map(|edge| {
                                        (
                                            iu,
                                            edge.iw[0..2]
                                                .iter()
                                                .map(|&iw| {
                                                    nest_wd.get(iw).map_or(0, |face| face.flag())
                                                })
                                                .collect::<Vec<_>>(),
                                        )
                                    })
                                })
                                .collect::<Vec<_>>();
                            let parent_u_perimeter_triples = payload
                                .parent_u_edge
                                .map(|parent_iu| {
                                    perimeter
                                        .chunks_exact(3)
                                        .enumerate()
                                        .filter(|(_, triple)| {
                                            triple.iter().any(|point| point.iu == parent_iu)
                                        })
                                        .map(|(index, triple)| (index, triple.to_vec()))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            eprintln!(
                                "earthmesh_mesh: method_c repair witness attempt={} kind={:?} child_m={:?} parent_m={:?} parent_u={:?} parent_m_valence_witnesses={:?} parent_u_m_points={:?} parent_u_faces={:?} parent_u_rings={:?} parent_u_face_flags={:?} parent_u_neighbor_edge_flags={:?} parent_u_perimeter_triples={:?}",
                                attempt + 1,
                                payload.kind,
                                payload.m_point,
                                payload.parent_m_point,
                                payload.parent_u_edge,
                                payload.parent_m_valence_witnesses,
                                parent_u.map(|edge| edge.im),
                                parent_u.map(|edge| edge.iw),
                                parent_u_rings,
                                parent_u_face_flags,
                                parent_u_neighbor_edge_flags,
                                parent_u_perimeter_triples,
                            );
                        }
                    }
                    let is_valence = method_c_repairable_payload(&error)
                        .is_some_and(|payload| payload.kind == MethodCRepairableKind::Valence);
                    let valence_m = Self::method_c_valence_error_parent_m_point(&error);
                    let mut repaired = if is_valence {
                        trace(
                            "method_c-mask-repair-shrink-start",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                        let stage_started = start_stage();
                        let repaired = self.try_shrink_method_c_perimeter_once(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                            coverage,
                        )?;
                        report_stage("shrink", attempt, stage_started);
                        trace(
                            "method_c-mask-repair-shrink-end",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                        repaired
                    } else {
                        None
                    };
                    if repaired.is_none() {
                        repaired = if let Some(im) = valence_m {
                            if im <= self.nmd {
                                trace(
                                    "method_c-mask-repair-fill-m-start",
                                    attempt + 1,
                                    max_repair_attempts,
                                )?;
                                let stage_started = start_stage();
                                let repaired = self.try_fill_method_c_specific_m_point(
                                    &selected,
                                    &method_c_m_neighbors,
                                    child_level,
                                    im,
                                )?;
                                report_stage("fill-m", attempt, stage_started);
                                trace(
                                    "method_c-mask-repair-fill-m-end",
                                    attempt + 1,
                                    max_repair_attempts,
                                )?;
                                repaired
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                    }
                    if repaired.is_none() {
                        trace(
                            "method_c-mask-repair-fill-boundary-start",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                        let stage_started = start_stage();
                        repaired = self.try_fill_method_c_perimeter_boundary(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                            dependency_faces.as_deref(),
                            max_mrows,
                            project_to_radius,
                        )?;
                        report_stage("fill-boundary", attempt, stage_started);
                        trace(
                            "method_c-mask-repair-fill-boundary-end",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                    }
                    if repaired.is_none() {
                        trace(
                            "method_c-mask-repair-grow-start",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                        let stage_started = start_stage();
                        repaired = self.try_grow_method_c_non_triplet_perimeter(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                            coverage,
                            Some((max_mrows, project_to_radius)),
                        )?;
                        report_stage("grow", attempt, stage_started);
                        trace(
                            "method_c-mask-repair-grow-end",
                            attempt + 1,
                            max_repair_attempts,
                        )?;
                    }
                    let Some((repaired, _)) = repaired else {
                        return Err(error);
                    };
                    selected.clone_from_slice(&repaired);
                    last_repairable_error = Some(error);
                    trace(
                        "method_c-mask-repair-attempt-end",
                        attempt + 1,
                        max_repair_attempts,
                    )?;
                }
                Err(error) => return Err(error),
            }
        }

        if cross_level_support
            && last_repairable_error.as_ref().is_some_and(|error| {
                method_c_repairable_payload(error)
                    .is_some_and(|payload| payload.kind != MethodCRepairableKind::Valence)
            })
        {
            let support =
                self.method_c_parent_support_for_current_mask(&selected, &method_c_m_neighbors)?;
            if !support.is_empty() {
                return Err(method_c_parent_support_error(support));
            }
        }
        Err(last_repairable_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C automatic perimeter repair exceeded its iteration limit",
            )
        }))
    }

    pub(crate) fn spawn_nest_pass_method_c_preserving_demands(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
        coverage: &crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage,
    ) -> io::Result<Self> {
        self.spawn_nest_pass_method_c_repairing(
            selected_faces,
            child_level,
            max_mrows,
            project_to_radius,
            Some(coverage),
        )
    }

    pub(crate) fn spawn_nest_pass_method_c_without_mask_repair(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_method_c_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C child level must be greater than one",
            ));
        }

        let mut selected = selected_faces.to_vec();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;
        self.emit_method_c_selected_faces(
            &selected,
            &method_c_m_neighbors,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }
}

/// How many outer repair rounds one pass may run.
///
/// A request may only lower the ceiling, never raise it, so shortening the
/// search stays a diagnostic choice and no setting can buy a pass more attempts
/// than production allows. Anything unparseable or out of range keeps the
/// ceiling. This used to be read only when `EARTHMESH_M0_REPAIR_TRACE` was on,
/// which left the documented knob silently inert on every ordinary run.
fn method_c_repair_attempt_limit(requested: Option<&str>) -> usize {
    requested
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| (1..=METHOD_C_REPAIR_ATTEMPT_CEILING).contains(value))
        .unwrap_or(METHOD_C_REPAIR_ATTEMPT_CEILING)
}

const METHOD_C_REPAIR_ATTEMPT_CEILING: usize = 64;

#[cfg(test)]
mod repair_attempt_limit_tests {
    use super::{method_c_repair_attempt_limit, METHOD_C_REPAIR_ATTEMPT_CEILING};

    #[test]
    fn repair_attempt_limit_defaults_to_the_ceiling() {
        assert_eq!(
            method_c_repair_attempt_limit(None),
            METHOD_C_REPAIR_ATTEMPT_CEILING
        );
    }

    #[test]
    fn repair_attempt_limit_honours_a_lower_request() {
        assert_eq!(method_c_repair_attempt_limit(Some("1")), 1);
    }

    #[test]
    fn repair_attempt_limit_rejects_a_request_above_the_ceiling() {
        assert_eq!(
            method_c_repair_attempt_limit(Some("65")),
            METHOD_C_REPAIR_ATTEMPT_CEILING
        );
    }

    #[test]
    fn repair_attempt_limit_rejects_an_unparseable_request() {
        assert_eq!(
            method_c_repair_attempt_limit(Some("many")),
            METHOD_C_REPAIR_ATTEMPT_CEILING
        );
    }
}
