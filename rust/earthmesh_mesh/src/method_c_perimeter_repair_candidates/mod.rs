use std::{
    collections::BTreeSet,
    io,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

use rayon::prelude::*;

use super::*;

fn method_c_fill_boundary_key(
    exact_accepted: bool,
    added: usize,
    remainder: usize,
    perimeter_len: usize,
    candidate: usize,
) -> (bool, usize, usize, usize, usize) {
    (!exact_accepted, added, remainder, perimeter_len, candidate)
}

impl MethodCDelaunayMesh {
    pub(crate) fn method_c_vertex_only_perimeter_contacts(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_method_c_len("selected_faces", selected.len(), self.nwd + 1)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let Some(parent_mrlw) = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, &is_selected)| is_selected.then_some(self.w_faces[iw].mrlw))
        else {
            return Ok(Vec::new());
        };

        let mut contacts = Vec::new();
        for im in 2..=self.nmd {
            let neighbors = m_neighbors[im];
            let mut active = Vec::with_capacity(neighbors.npoly);
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C vertex-contact W face", iw, self.nwd)?;
                active.push(selected[iw]);
            }
            let active_count = active.iter().filter(|&&item| item).count();
            if active_count > 1
                && active_count < neighbors.npoly
                && crate::mask_postproc_waterway::cyclic_active_runs(&active) > 1
                && neighbors
                    .iw
                    .iter()
                    .take(neighbors.npoly)
                    .all(|&iw| self.w_faces[iw].mrlw == parent_mrlw)
            {
                contacts.push(im);
            }
        }
        Ok(contacts)
    }

    pub(crate) fn fill_method_c_vertex_only_perimeter_contacts(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<bool> {
        let contacts = self.method_c_vertex_only_perimeter_contacts(selected, m_neighbors)?;
        if std::env::var_os("EARTHMESH_M0_REPAIR_TRACE").is_some() {
            eprintln!("earthmesh_mesh: method_c vertex-only contacts={contacts:?}");
        }
        if contacts.len() < 2 {
            return Ok(false);
        }
        let fills = self.method_c_vertex_only_perimeter_contact_fill_faces(
            selected,
            m_neighbors,
            &contacts,
        )?;

        let mut changed = false;
        for iw in fills {
            changed |= !selected[iw];
            selected[iw] = true;
        }
        if changed {
            self.close_method_c_concavities_for_level_with_neighbors(selected, m_neighbors)?;
            self.ensure_method_c_selected_faces_share_parent_mrlw(selected, child_level)?;
        }
        Ok(changed)
    }

    pub(crate) fn method_c_vertex_only_perimeter_contact_fill_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        contacts: &[usize],
    ) -> io::Result<BTreeSet<usize>> {
        let mut fills = BTreeSet::new();
        for &im in contacts {
            require_method_c_id("Method-C vertex-contact M point", im, self.nmd)?;
            let neighbors = m_neighbors[im];
            let active = neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .map(|&iw| selected[iw])
                .collect::<Vec<_>>();
            let mut kept_gap = vec![false; neighbors.npoly];
            let mut longest_gap = (0usize, 0usize);
            for start in 0..neighbors.npoly {
                if active[start] || !active[(start + neighbors.npoly - 1) % neighbors.npoly] {
                    continue;
                }
                let mut length = 0;
                while length < neighbors.npoly && !active[(start + length) % neighbors.npoly] {
                    length += 1;
                }
                if length >= longest_gap.1 {
                    longest_gap = (start, length);
                }
            }
            for offset in 0..longest_gap.1 {
                kept_gap[(longest_gap.0 + offset) % neighbors.npoly] = true;
            }
            for (slot, &iw) in neighbors.iw.iter().take(neighbors.npoly).enumerate() {
                if !active[slot] && !kept_gap[slot] {
                    fills.insert(iw);
                }
            }
        }
        Ok(fills)
    }

    pub(crate) fn try_fill_method_c_specific_m_point(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        im: usize,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        require_method_c_id("Method-C valence repair M point", im, self.nmd)?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut trial = selected.to_vec();
        self.mark_fill_rad3_faces_with_neighbors(im, &mut trial, m_neighbors)?;
        self.close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
        if trial.iter().filter(|&&item| item).count() == selected_count {
            return Ok(None);
        }
        if self
            .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
            .is_err()
        {
            return Ok(None);
        }
        let Ok(trial_perimeters) =
            self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
        else {
            return Ok(None);
        };
        let trial_perimeter = trial_perimeters.into_iter().flatten().collect();
        Ok(Some((trial, trial_perimeter)))
    }

    pub(crate) fn try_fill_method_c_perimeter_boundary(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        dependency_faces: Option<&[usize]>,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let mut boundary_m = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            for point in perimeter {
                boundary_m.insert(point.im);
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C repair boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m > 0 && selected_count_at_m < neighbors.npoly {
                    boundary_m.insert(im);
                }
            }
        }
        if boundary_m.is_empty() {
            return Ok(None);
        }

        let selected_count = selected.iter().filter(|&&item| item).count();
        let candidates = boundary_m.into_iter().collect::<Vec<_>>();
        let candidate_count = candidates.len();
        let detailed_trace = std::env::var("EARTHMESH_M0_REPAIR_TRACE")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true"));
        let exact_scan = std::env::var_os("EARTHMESH_M0_EXACT_CANDIDATE_SCAN").is_some();
        let dependency_changed = detailed_trace.then(AtomicUsize::default);
        let dependency_outcomes =
            detailed_trace.then(|| Mutex::new(Vec::<(usize, &'static str)>::new()));
        let exact_scan_outcomes =
            exact_scan.then(|| Mutex::new(Vec::<(usize, &'static str)>::new()));
        let exact_outcome = |trial: &[bool]| match self.emit_method_c_selected_faces(
            trial,
            m_neighbors,
            child_level,
            max_mrows,
            project_to_radius,
        ) {
            Ok(_) => "ok",
            Err(error) => match method_c_repairable_payload(&error).map(|payload| payload.kind) {
                Some(MethodCRepairableKind::Valence) => "valence",
                Some(MethodCRepairableKind::TransitionPatch) => "transition-patch",
                Some(MethodCRepairableKind::NonTripletPerimeter) => "non-triplet",
                None => "other",
            },
        };
        let best = candidates
            .into_par_iter()
            .map(
                |im| -> io::Result<
                    Option<(
                        bool,
                        usize,
                        usize,
                        usize,
                        usize,
                        Vec<bool>,
                        Vec<MethodCPerimeterPoint>,
                    )>,
                > {
                    let mut trial = selected.to_vec();
                    self.mark_fill_rad3_faces_with_neighbors(im, &mut trial, m_neighbors)?;
                    self.close_method_c_concavities_for_level_with_neighbors(
                        &mut trial,
                        m_neighbors,
                    )?;
                    let added = trial.iter().filter(|&&item| item).count() - selected_count;
                    if added == 0
                        || self
                            .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                            .is_err()
                    {
                        return Ok(None);
                    }
                    let Ok(trial_perimeters) =
                        self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
                    else {
                        return Ok(None);
                    };
                    let remainder = Self::method_c_perimeter_remainder_score(&trial_perimeters);
                    let trial_perimeter =
                        trial_perimeters.into_iter().flatten().collect::<Vec<_>>();
                    let changes_dependency = dependency_faces.is_some_and(|faces| {
                        faces.iter().any(|&iw| selected.get(iw) != trial.get(iw))
                    });
                    if changes_dependency {
                        if let Some(changed) = dependency_changed.as_ref() {
                            changed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    let mut exact_accepted = false;
                    if changes_dependency || exact_scan {
                        let outcome = exact_outcome(&trial);
                        exact_accepted = changes_dependency && outcome == "ok";
                        if changes_dependency {
                            if let Some(outcomes) = dependency_outcomes.as_ref() {
                                outcomes
                                    .lock()
                                    .expect("Method-C dependency outcome lock")
                                    .push((im, outcome));
                            }
                        }
                        if let Some(outcomes) = exact_scan_outcomes.as_ref() {
                            outcomes
                                .lock()
                                .expect("Method-C exact-scan outcome lock")
                                .push((im, outcome));
                        }
                    }
                    Ok(Some((
                        exact_accepted,
                        added,
                        remainder,
                        trial_perimeter.len(),
                        im,
                        trial,
                        trial_perimeter,
                    )))
                },
            )
            .try_reduce(
                || None,
                |left, right| {
                    Ok(match (left, right) {
                        (Some(left), Some(right)) => {
                            if method_c_fill_boundary_key(
                                right.0, right.1, right.2, right.3, right.4,
                            ) < method_c_fill_boundary_key(
                                left.0, left.1, left.2, left.3, left.4,
                            ) {
                                Some(right)
                            } else {
                                Some(left)
                            }
                        }
                        (Some(score), None) | (None, Some(score)) => Some(score),
                        (None, None) => None,
                    })
                },
            )?;

        if detailed_trace {
            let dependency_changed = dependency_changed
                .as_ref()
                .map_or(0, |count| count.load(Ordering::Relaxed));
            let mut dependency_outcomes = dependency_outcomes
                .as_ref()
                .map(|outcomes| {
                    outcomes
                        .lock()
                        .expect("Method-C dependency outcome lock")
                        .clone()
                })
                .unwrap_or_default();
            dependency_outcomes.sort_unstable();
            if let Some((exact_accepted, added, remainder, perimeter_len, candidate, _, _)) =
                best.as_ref()
            {
                eprintln!(
                    "earthmesh_mesh: method_c fill-boundary candidates={} dependency_faces={} dependency_changed_candidates={} dependency_candidate_outcomes={:?} chosen_m={} exact_accepted={} added={} remainder={} perimeter_len={}",
                    candidate_count,
                    dependency_faces.map_or(0, <[usize]>::len),
                    dependency_changed,
                    dependency_outcomes,
                    candidate,
                    exact_accepted,
                    added,
                    remainder,
                    perimeter_len,
                );
            } else {
                eprintln!(
                    "earthmesh_mesh: method_c fill-boundary candidates={} dependency_faces={} dependency_changed_candidates={} dependency_candidate_outcomes={:?} chosen_m=none",
                    candidate_count,
                    dependency_faces.map_or(0, <[usize]>::len),
                    dependency_changed,
                    dependency_outcomes,
                );
            }
        }
        if let Some(outcomes) = exact_scan_outcomes {
            let outcomes = outcomes
                .into_inner()
                .expect("Method-C exact-scan outcome lock");
            let mut counts = std::collections::BTreeMap::new();
            for &(_, outcome) in &outcomes {
                *counts.entry(outcome).or_insert(0usize) += 1;
            }
            let successful = outcomes
                .iter()
                .filter_map(|&(im, outcome)| (outcome == "ok").then_some(im))
                .take(10)
                .collect::<Vec<_>>();
            eprintln!(
                "earthmesh_mesh: method_c exact candidate scan outcomes={counts:?} first_ok_candidates={successful:?}"
            );
        }
        Ok(best.map(|(_, _, _, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }
}

#[cfg(test)]
mod tests {
    use super::method_c_fill_boundary_key;

    #[test]
    fn exact_fill_boundary_candidate_outranks_cheaper_known_failure() {
        assert!(
            method_c_fill_boundary_key(true, 20, 0, 60, 10)
                < method_c_fill_boundary_key(false, 1, 0, 18, 2)
        );
    }
}
