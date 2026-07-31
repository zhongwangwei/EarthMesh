use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        method_c_repairable_payload(error).is_some()
    }

    pub(crate) fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let payload = method_c_repairable_payload(error)?;
        (payload.kind == MethodCRepairableKind::Valence)
            .then_some(payload.m_point)
            .flatten()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        const MAX_REPAIR_PASSES: usize = 12;

        let mut last_error = None;
        let mut repair_attempts = 0;
        let detailed_trace = std::env::var("EARTHMESH_M0_REPAIR_TRACE")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true"));
        let report = |phase, done| {
            if !detailed_trace || earthmesh_core::progress::report(phase, done, MAX_REPAIR_PASSES) {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    format!("Method-C {phase} cancelled"),
                ))
            }
        };
        for attempt in 0..MAX_REPAIR_PASSES {
            report("method_c-non-triplet-attempt-start", attempt + 1)?;
            let perimeter =
                match self.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                    Ok(perimeters) if Self::method_c_perimeters_are_triplets(&perimeters) => {
                        report("method_c-non-triplet-attempt-end", attempt + 1)?;
                        return Ok(perimeters.into_iter().flatten().collect());
                    }
                    Ok(perimeters) => {
                        // A prior vertex-contact error no longer describes
                        // this mask. If growth is exhausted, report the
                        // current non-triplet perimeter so the outer repair
                        // loop can continue with its other operators.
                        last_error = None;
                        Some(perimeters.into_iter().flatten().collect::<Vec<_>>())
                    }
                    Err(error) => {
                        last_error = Some(error);
                        report("method_c-vertex-contact-fill-start", attempt + 1)?;
                        let changed = self.fill_method_c_vertex_only_perimeter_contacts(
                            selected,
                            m_neighbors,
                            child_level,
                        )?;
                        report("method_c-vertex-contact-fill-end", attempt + 1)?;
                        if changed {
                            repair_attempts += 1;
                            report("method_c-non-triplet-attempt-end", attempt + 1)?;
                            continue;
                        }
                        None
                    }
                };
            report("method_c-non-triplet-grow-start", attempt + 1)?;
            let repaired = self.try_grow_method_c_non_triplet_perimeter_once(
                selected,
                m_neighbors,
                child_level,
                perimeter.as_deref(),
                None,
            )?;
            report("method_c-non-triplet-grow-end", attempt + 1)?;
            let Some((repaired, _)) = repaired else {
                report("method_c-non-triplet-attempt-end", attempt + 1)?;
                break;
            };
            repair_attempts += 1;
            selected.clone_from_slice(&repaired);
            match self.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                Ok(repaired_perimeters)
                    if Self::method_c_perimeters_are_triplets(&repaired_perimeters) =>
                {
                    report("method_c-non-triplet-attempt-end", attempt + 1)?;
                    return Ok(repaired_perimeters.into_iter().flatten().collect());
                }
                Ok(_) => {}
                Err(error) => last_error = Some(error),
            }
            report("method_c-non-triplet-attempt-end", attempt + 1)?;
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        let perimeters = self.method_c_perimeters_from_selected_faces(selected, m_neighbors)?;
        let perimeter_lengths = perimeters.iter().map(Vec::len).collect::<Vec<_>>();
        Err(method_c_repairable_perimeter_error(
            MethodCRepairableKind::NonTripletPerimeter,
            perimeter_lengths.clone(),
            repair_attempts,
            format!(
                "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples without crossing the parent boundary",
                perimeter_lengths
            ),
        ))
    }
}
