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
                // The parent-support oracle ran against the mask as it stood
                // before this drop, so its answer no longer describes the
                // perimeter about to be materialized: removing a component
                // moves the boundary and therefore changes which parent faces
                // perim_fill3 consumes. Report the faces the new perimeter
                // needs but the parent has not refined, so the caller can tell
                // a stale support answer from a genuine one.
                let parent_level = selected
                    .iter()
                    .enumerate()
                    .skip(2)
                    .find_map(|(iw, &is_selected)| is_selected.then_some(self.w_faces[iw].mrlw));
                if let Some(parent_level) = parent_level {
                    let nest_wd =
                        self.method_c_nest_wd_from_selected_and_perimeter(selected, &perimeter)?;
                    let stale = self
                        .method_c_transition_parent_boundary_witnesses(
                            &perimeter,
                            &nest_wd,
                            parent_level,
                        )?
                        .into_iter()
                        .flat_map(|(_, faces)| faces)
                        .filter(|&iw| {
                            self.w_faces[iw].mrlw < parent_level && !nest_wd[iw].is_subdivided()
                        })
                        .map(|iw| self.w_lineage[iw])
                        .collect::<BTreeSet<_>>();
                    eprintln!(
                        "earthmesh_mesh: method_c post-drop support recheck parent_level={parent_level} \
                         unsupported_witness_faces={}",
                        stale.len()
                    );
                    record_post_drop_support(stale);
                }
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
                // Face ids are rebuilt every pass; the stable lineage is not.
                // Record the concession against lineage so later passes can
                // recognise the same region after the parent is re-materialized.
                record_conceded_lineages(component.iter().map(|&iw| self.w_lineage[iw]));
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

    /// Deselect anything within `rings` M-point hops of a conceded region.
    ///
    /// Conceded faces keep the lineage they had when they were given up, so a
    /// later pass recognises them even after the parent is re-materialized and
    /// face ids change. No-op when nothing has been conceded.
    pub(crate) fn clear_method_c_conceded_margin(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        rings: usize,
    ) -> io::Result<usize> {
        let conceded = conceded_lineage_snapshot();
        if conceded.is_empty() {
            return Ok(0);
        }
        let mut blocked = vec![false; selected.len()];
        for iw in 2..selected.len().min(self.nwd + 1) {
            if conceded.contains(&self.w_lineage[iw]) {
                blocked[iw] = true;
            }
        }
        if !blocked.iter().any(|&item| item) {
            return Ok(0);
        }
        for _ in 0..rings {
            let mut grown = blocked.clone();
            for iw in 2..blocked.len().min(self.nwd + 1) {
                if !blocked[iw] {
                    continue;
                }
                for im in self.w_faces[iw].im {
                    if im < 2 || im > self.nmd {
                        continue;
                    }
                    let neighbors = m_neighbors[im];
                    for &near in neighbors.iw.iter().take(neighbors.npoly) {
                        if near >= 2 && near <= self.nwd {
                            grown[near] = true;
                        }
                    }
                }
            }
            blocked = grown;
        }
        let mut cleared = 0usize;
        for (iw, is_selected) in selected.iter_mut().enumerate() {
            if *is_selected && blocked.get(iw).copied().unwrap_or(false) {
                *is_selected = false;
                cleared += 1;
            }
        }
        if cleared > 0 {
            eprintln!(
                "earthmesh_mesh: method_c conceded margin cleared={cleared} rings={rings} \
                 conceded_lineages={}",
                conceded.len()
            );
        }
        Ok(cleared)
    }
}

/// Whether a pass may drop the selection components whose perimeter cannot be
/// decomposed, instead of failing outright.
fn component_triplet_drop_enabled() -> bool {
    std::env::var_os("EARTHMESH_M0_COMPONENT_TRIPLET_DROP").is_some()
}

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

/// Regions a pass gave up on, keyed by stable W-face lineage.
///
/// A conceded component can never be refined by a later pass: selection only
/// admits faces of the current generation, so the concession stays a generation
/// behind for good. Later passes therefore have to keep their transition bands
/// clear of it rather than request parent support that can never arrive.
fn conceded_lineages() -> &'static Mutex<BTreeSet<usize>> {
    static CONCEDED: OnceLock<Mutex<BTreeSet<usize>>> = OnceLock::new();
    CONCEDED.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn record_conceded_lineages(lineages: impl IntoIterator<Item = usize>) {
    if let Ok(mut set) = conceded_lineages().lock() {
        set.extend(lineages);
    }
}

/// Stable lineages conceded so far in this process.
pub(crate) fn conceded_lineage_snapshot() -> BTreeSet<usize> {
    conceded_lineages()
        .lock()
        .map(|set| set.clone())
        .unwrap_or_default()
}

/// Parent faces a concession left unsupported, keyed by stable W-face lineage.
///
/// The support oracle answers before a concession happens, so removing a
/// component invalidates that answer: the boundary moves and `perim_fill3`
/// consumes a different set of parent faces. Recording the difference lets the
/// outer support loop request it instead of proceeding to an emit that will
/// fail on it.
fn post_drop_support() -> &'static Mutex<BTreeSet<usize>> {
    static PENDING: OnceLock<Mutex<BTreeSet<usize>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn record_post_drop_support(lineages: BTreeSet<usize>) {
    if let Ok(mut set) = post_drop_support().lock() {
        set.extend(lineages);
    }
}

/// Take the pending post-concession support requirements, clearing them.
pub fn take_post_drop_support_lineages() -> Vec<usize> {
    post_drop_support()
        .lock()
        .map(|mut set| std::mem::take(&mut *set).into_iter().collect())
        .unwrap_or_default()
}
