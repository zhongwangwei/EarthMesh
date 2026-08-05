use std::io;

use super::*;

#[derive(Debug, Clone)]
pub struct MethodCParentSupportRequest {
    pub lineages: Vec<usize>,
}

impl std::fmt::Display for MethodCParentSupportRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Method-C parent support required for {} stable W-face lineages",
            self.lineages.len()
        )
    }
}

impl std::error::Error for MethodCParentSupportRequest {}

pub fn method_c_parent_support_request(error: &io::Error) -> Option<&MethodCParentSupportRequest> {
    error
        .get_ref()?
        .downcast_ref::<MethodCParentSupportRequest>()
}

pub(crate) fn method_c_parent_support_error(lineages: BTreeSet<usize>) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        MethodCParentSupportRequest {
            lineages: lineages.into_iter().collect(),
        },
    )
}

fn method_c_non_triplet_perimeter_points(
    perimeters: Vec<Vec<MethodCPerimeterPoint>>,
) -> Vec<MethodCPerimeterPoint> {
    perimeters
        .into_iter()
        .filter(|perimeter| !perimeter.len().is_multiple_of(3))
        .flatten()
        .collect()
}

impl MethodCDelaunayMesh {
    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        method_c_repairable_payload(error).is_some()
    }

    pub(crate) fn method_c_valence_error_parent_m_point(error: &io::Error) -> Option<usize> {
        let payload = method_c_repairable_payload(error)?;
        (payload.kind == MethodCRepairableKind::Valence)
            .then_some(payload.parent_m_point)
            .flatten()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        self.repair_method_c_non_triplet_perimeter_tracking_support(
            selected,
            m_neighbors,
            child_level,
            None,
        )
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter_tracking_support(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        exact_materialization: Option<(usize, bool)>,
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
                        Some(method_c_non_triplet_perimeter_points(perimeters))
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
            let repaired = self.try_grow_method_c_non_triplet_perimeter(
                selected,
                m_neighbors,
                child_level,
                perimeter.as_deref(),
                None,
                exact_materialization,
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
                    if !stale.is_empty() {
                        return Err(method_c_parent_support_error(stale));
                    }
                }
                return Ok(perimeter);
            }
        }

        let perimeters = self.method_c_perimeters_from_selected_faces(selected, m_neighbors)?;
        let perimeter_lengths = perimeters.iter().map(Vec::len).collect::<Vec<_>>();
        self.dump_method_c_unrepaired_mask(
            "repair-exhausted",
            selected,
            &perimeter_lengths,
            child_level,
        );
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

    /// Write the mask repair gave up on, so its shape can be studied offline.
    ///
    /// Answering "how many faces would make this perimeter decomposable" takes
    /// milliseconds against a saved mask and a quarter of an hour against a
    /// pipeline run. Off unless `EARTHMESH_M0_UNREPAIRED_MASK_DUMP_DIR` names a
    /// directory; failures to write are swallowed because a diagnostic must not
    /// change which error the caller sees.
    pub(crate) fn dump_method_c_unrepaired_mask(
        &self,
        label: &str,
        selected: &[bool],
        perimeter_lengths: &[usize],
        child_level: usize,
    ) {
        let Some(dir) = std::env::var_os("EARTHMESH_M0_UNREPAIRED_MASK_DUMP_DIR") else {
            return;
        };
        let dir = std::path::PathBuf::from(dir);
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let faces = selected
            .iter()
            .enumerate()
            .filter_map(|(iw, &is_selected)| is_selected.then_some(iw))
            .collect::<Vec<_>>();
        let undecomposable = perimeter_lengths
            .iter()
            .filter(|length| *length % 3 != 0)
            .copied()
            .collect::<Vec<_>>();
        let body = format!(
            "{{\"kind\":\"earthmesh_method_c_unrepaired_mask\",\"label\":\"{label}\",\
             \"child_level\":{child_level},\
             \"nmd\":{},\"nwd\":{},\"selected_face_count\":{},\
             \"perimeter_lengths\":{perimeter_lengths:?},\"undecomposable_lengths\":{undecomposable:?},\
             \"selected_faces\":{faces:?}}}",
            self.nmd,
            self.nwd,
            faces.len(),
        );
        let path = dir.join(format!("unrepaired-mask-{label}-level{child_level}.json"));
        if std::fs::write(&path, body).is_ok() {
            eprintln!(
                "earthmesh_mesh: wrote unrepaired mask {label} child_level={child_level} faces={} \
                 undecomposable={undecomposable:?} -> {}",
                faces.len(),
                path.display()
            );
        }
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
    pub(crate) fn method_c_selected_face_components(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn perimeter_point(im: usize) -> MethodCPerimeterPoint {
        MethodCPerimeterPoint {
            im,
            iu: im,
            npoly: 6,
            nwdiv: 2,
            near_pentagon: false,
        }
    }

    #[test]
    fn repair_search_ignores_already_triplet_perimeters() {
        let points = method_c_non_triplet_perimeter_points(vec![
            (1..=6).map(perimeter_point).collect(),
            (10..=14).map(perimeter_point).collect(),
            (20..=22).map(perimeter_point).collect(),
            (30..=36).map(perimeter_point).collect(),
        ]);

        assert_eq!(
            points.iter().map(|point| point.im).collect::<Vec<_>>(),
            vec![10, 11, 12, 13, 14, 30, 31, 32, 33, 34, 35, 36]
        );
    }

    #[test]
    fn parent_support_requests_are_typed_and_error_local() {
        let first = method_c_parent_support_error(BTreeSet::from([3, 7]));
        let second = method_c_parent_support_error(BTreeSet::from([11]));

        assert_eq!(
            method_c_parent_support_request(&first)
                .expect("typed first request")
                .lineages,
            vec![3, 7]
        );
        assert_eq!(
            method_c_parent_support_request(&second)
                .expect("typed second request")
                .lineages,
            vec![11]
        );
        assert!(method_c_parent_support_request(&io::Error::new(
            io::ErrorKind::InvalidData,
            "unrelated Method-C failure",
        ))
        .is_none());
    }

    #[test]
    fn valence_repair_never_treats_a_child_m_id_as_a_parent_id() {
        let child_only = method_c_repairable_error(
            MethodCRepairableKind::Valence,
            Some(42),
            "child-space witness",
        );
        assert_eq!(
            MethodCDelaunayMesh::method_c_valence_error_parent_m_point(&child_only),
            None
        );

        let mapped = crate::method_c_table_helpers::method_c_repairable_error_with_parent_origin(
            child_only,
            Some(7),
            None,
        );
        assert_eq!(
            MethodCDelaunayMesh::method_c_valence_error_parent_m_point(&mapped),
            Some(7)
        );
    }
}
