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

        // Diagnostic: the triplet test is all-or-nothing, so one bad component
        // fails the whole pass even when the rest decompose cleanly. Dropping
        // just the offending components measures how much refinement Method-C
        // could still legalize, which is what a finer-granularity stage would
        // have to make up. Off by default; production still fails the pass.
        if component_triplet_drop_enabled() {
            if let Some(perimeter) =
                self.drop_non_triplet_components_for_diagnostics(selected, m_neighbors)?
            {
                return Ok(perimeter);
            }
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

    /// Deselect the connected components of `selected` whose own perimeter does
    /// not decompose into transition triples, and return the perimeter of what
    /// remains if that is now legal.
    ///
    /// Components are taken over shared W-face edges of the selection, which is
    /// the same adjacency the perimeter walk follows, so each component's
    /// boundary is independent of the others'. Returns `None` when dropping the
    /// offenders leaves nothing selected or still fails, so the caller falls
    /// back to reporting the original failure.
    fn drop_non_triplet_components_for_diagnostics(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Option<Vec<MethodCPerimeterPoint>>> {
        let components = self.method_c_selected_face_components(selected, m_neighbors)?;
        let total_selected = selected.iter().filter(|&&item| item).count();
        let mut dropped_components = 0usize;
        let mut dropped_faces = 0usize;
        let mut kept = vec![false; selected.len()];

        for component in &components {
            let mut mask = vec![false; selected.len()];
            for &iw in component {
                mask[iw] = true;
            }
            let perimeters = self.method_c_perimeters_from_selected_faces(&mask, m_neighbors)?;
            if Self::method_c_perimeters_are_triplets(&perimeters) {
                for &iw in component {
                    kept[iw] = true;
                }
            } else {
                dropped_components += 1;
                dropped_faces += component.len();
            }
        }

        let kept_faces = kept.iter().filter(|&&item| item).count();
        eprintln!(
            "earthmesh_mesh: method_c component triplet drop components={} dropped_components={} \
             selected_faces={total_selected} dropped_faces={dropped_faces} kept_faces={kept_faces}",
            components.len(),
            dropped_components,
        );
        if kept_faces == 0 || dropped_components == 0 {
            return Ok(None);
        }

        let perimeters = self.method_c_perimeters_from_selected_faces(&kept, m_neighbors)?;
        if !Self::method_c_perimeters_are_triplets(&perimeters) {
            return Ok(None);
        }
        selected.clone_from_slice(&kept);
        Ok(Some(perimeters.into_iter().flatten().collect()))
    }

    /// Connected components of the selected faces under shared-edge adjacency.
    fn method_c_selected_face_components(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<Vec<usize>>> {
        let mut seen = vec![false; selected.len()];
        let mut components = Vec::new();
        // Faces meeting at an M point are edge- or vertex-adjacent; walking the
        // M rings is the same traversal the perimeter builder uses.
        let mut faces_at_m = vec![Vec::new(); self.nmd + 1];
        for (iw, &is_selected) in selected.iter().enumerate() {
            if !is_selected || iw < 2 {
                continue;
            }
            for im in self.w_faces[iw].im {
                if im >= 2 && im <= self.nmd {
                    faces_at_m[im].push(iw);
                }
            }
        }
        for (start, &is_selected) in selected.iter().enumerate() {
            if !is_selected || start < 2 || seen[start] {
                continue;
            }
            let mut component = Vec::new();
            let mut stack = vec![start];
            seen[start] = true;
            while let Some(iw) = stack.pop() {
                component.push(iw);
                for im in self.w_faces[iw].im {
                    if im < 2 || im > self.nmd {
                        continue;
                    }
                    let _ = m_neighbors.get(im);
                    for &neighbor in &faces_at_m[im] {
                        if !seen[neighbor] {
                            seen[neighbor] = true;
                            stack.push(neighbor);
                        }
                    }
                }
            }
            component.sort_unstable();
            components.push(component);
        }
        Ok(components)
    }
}

/// Whether a pass may drop the selection components whose perimeter cannot be
/// decomposed, instead of failing outright.
fn component_triplet_drop_enabled() -> bool {
    std::env::var_os("EARTHMESH_M0_COMPONENT_TRIPLET_DROP").is_some()
}
