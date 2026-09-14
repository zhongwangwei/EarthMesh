use super::*;

pub(crate) struct LeppInsertionBatch {
    pub result: Result<Vec<LeppInsertionReport>, LeppInsertionError>,
    /// Includes attempted closure insertions and insertions rolled back with the batch.
    pub attempted: usize,
}

/// Commit a single insertion, or close its transient degree-four fan with one
/// neighbouring LEPP insertion. The quality postcondition sees only the complete
/// admissible batch. Both vertices count against the caller's remaining budget.
pub(crate) fn insert_lepp_terminal_batch(
    mesh: &mut MeshState,
    mut segments: Option<&mut SegmentList>,
    start: usize,
    config: &LeppSearchConfig,
    gates: &LeppInsertionGates,
    remaining: usize,
    postcondition: impl Fn(&MeshState) -> bool,
) -> LeppInsertionBatch {
    let mut attempted = 0;
    let result = (|| {
        if remaining == 0 {
            return Err(LeppInsertionError::InvalidGates {
                message: "no remaining insertion budget".to_string(),
            });
        }
        if gates.minimum_vertex_degree == 0 {
            attempted += 1;
            let report = if let Some(segments) = segments.as_deref_mut() {
                insert_lepp_terminal_midpoint_constrained_with_postcondition(
                    mesh,
                    segments,
                    start,
                    config,
                    gates,
                    |state, _| postcondition(state),
                )
            } else {
                insert_lepp_terminal_midpoint_with_postcondition(
                    mesh,
                    start,
                    config,
                    gates,
                    |state, _| postcondition(state),
                )
            }?;
            return Ok(vec![report]);
        }
        if gates.minimum_vertex_degree > gates.maximum_vertex_degree {
            return Err(LeppInsertionError::InvalidGates {
                message: "minimum_vertex_degree exceeds maximum_vertex_degree".to_string(),
            });
        }
        let before_segments = segments.as_deref().cloned();
        let intermediate_gates = LeppInsertionGates {
            minimum_vertex_degree: 0,
            ..gates.clone()
        };
        attempted += 1;
        let (first, undo) = if let Some(segments) = segments.as_deref_mut() {
            insert_terminal_midpoint_constrained_staged(
                mesh,
                segments,
                start,
                config,
                &intermediate_gates,
                true,
                |_, _| true,
            )
        } else {
            insert_terminal_midpoint_staged(
                mesh,
                start,
                config,
                &intermediate_gates,
                true,
                |_, _| true,
            )
        }?;
        let undo = undo.expect("staged first insertion retains its undo patch");
        let mesh_has_open_edges = mesh.open_edge_count() != 0;
        let changed: BTreeSet<_> = first.insertion.created.iter().copied().collect();
        let outcome = (|| {
            let failure = match check_degrees(mesh, &changed, gates, mesh_has_open_edges) {
                Ok(()) => {
                    return if postcondition(mesh) {
                        Ok(vec![first])
                    } else {
                        Err(LeppInsertionError::Transaction(
                            InsertionTransactionError::Rejected,
                        ))
                    };
                }
                Err(error) => error,
            };
            if remaining < 2
                || !matches!(
                    failure,
                    LeppInsertionError::DegreeLimit {
                        degree: 4,
                        minimum: 5,
                        ..
                    }
                )
            {
                return Err(failure);
            }
            let mut starts = changed.clone();
            for &face in &changed {
                starts.extend(mesh.neighbours()[face]);
            }
            let mut terminals = BTreeSet::new();
            // ponytail: bounded local pair only; larger closure patches need separate evidence.
            for start in starts {
                if !mesh.is_triangle_live(start) {
                    continue;
                }
                let path = find_lepp(mesh, start, config)?;
                let edge = match path.terminal {
                    LeppTerminal::InteriorPair { edge, .. }
                    | LeppTerminal::Boundary { edge, .. } => edge,
                };
                if !terminals.insert(edge) {
                    continue;
                }
                let accepts = |state: &MeshState, report: &InsertionReport| {
                    let mut both = changed.clone();
                    both.extend(report.created.iter().copied());
                    check_degrees(state, &both, gates, mesh_has_open_edges).is_ok()
                        && postcondition(state)
                };
                attempted += 1;
                let second = if let Some(segments) = segments.as_deref_mut() {
                    insert_lepp_terminal_midpoint_constrained_with_postcondition(
                        mesh, segments, start, config, gates, accepts,
                    )
                } else {
                    insert_lepp_terminal_midpoint_with_postcondition(
                        mesh, start, config, gates, accepts,
                    )
                };
                match second {
                    Ok(second) => return Ok(vec![first, second]),
                    Err(error) if expected_rejection(&error) => {}
                    Err(error) => return Err(error),
                }
            }
            Err(failure)
        })();
        if outcome.is_err() {
            if let (Some(segments), Some(before)) = (segments, before_segments) {
                *segments = before;
            }
            mesh.restore_patch(undo).map_err(|error| {
                LeppInsertionError::Transaction(InsertionTransactionError::Rollback(error))
            })?;
        }
        outcome
    })();
    LeppInsertionBatch { result, attempted }
}

fn expected_rejection(error: &LeppInsertionError) -> bool {
    matches!(
        error,
        LeppInsertionError::DegreeLimit { .. }
            | LeppInsertionError::ProtectedVertexDegreeWouldChange { .. }
            | LeppInsertionError::Transaction(InsertionTransactionError::Rejected)
    )
}

fn check_degrees(
    mesh: &MeshState,
    changed: &BTreeSet<usize>,
    gates: &LeppInsertionGates,
    mesh_has_open_edges: bool,
) -> Result<(), LeppInsertionError> {
    for (vertex, seed) in mesh.sites_touching(changed) {
        let degree = measured_degree(mesh, vertex, seed, mesh_has_open_edges)
            .ok_or(LeppInsertionError::UnmeasurableVertexDegree { vertex })?;
        if degree < gates.minimum_vertex_degree || degree > gates.maximum_vertex_degree {
            return Err(LeppInsertionError::DegreeLimit {
                vertex,
                degree,
                minimum: gates.minimum_vertex_degree,
                maximum: gates.maximum_vertex_degree,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_search_only_skips_expected_constraint_rejections() {
        assert!(expected_rejection(&LeppInsertionError::Transaction(
            InsertionTransactionError::Rejected,
        )));
        for error in [
            LeppInsertionError::Transaction(InsertionTransactionError::Topology(Vec::new())),
            LeppInsertionError::StaleProtectedSegment {
                edge: LeppEdgeId::new(2, 3),
            },
            LeppInsertionError::UnmeasurableVertexDegree { vertex: 2 },
        ] {
            assert!(!expected_rejection(&error));
        }
    }
}
