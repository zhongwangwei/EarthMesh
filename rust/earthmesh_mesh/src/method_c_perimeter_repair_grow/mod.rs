use std::{collections::BTreeSet, io};

use crate::method_c_perimeter_selection::MethodCPerimeterProbe;

use super::*;

/// One candidate face addition, scored.
///
/// `remainder` is zero exactly when every perimeter decomposes into transition
/// triples. `unsupported` is only meaningful once it does — counting the parent
/// faces a transition would consume costs a full `nest_wd` build, so the search
/// pays for it only on masks it might actually accept.
///
/// The mask itself is deliberately absent. Scoring a candidate needs one, but
/// keeping one per candidate meant holding a `nwd`-sized vector for every face
/// the search merely looked at. Only the survivors get a mask, rebuilt from the
/// face id, so allocation tracks the beam width rather than the fan-out.
struct MethodCRepairTrial {
    candidate: usize,
    added: usize,
    remainder: usize,
    unsupported: usize,
    perimeter: Vec<MethodCPerimeterPoint>,
}

impl MethodCRepairTrial {
    /// The greedy walk's ranking: cheapest edit first.
    fn greedy_key(&self) -> (usize, usize, usize) {
        (self.added, self.remainder, self.perimeter.len())
    }

    /// The widened search's ranking: decomposable masks first, then the ones
    /// closest to being materializable, and only then the cheapest edit.
    fn beam_key(&self) -> (usize, usize, usize, usize) {
        (
            self.remainder,
            self.unsupported,
            self.added,
            self.perimeter.len(),
        )
    }
}

/// How many trials the repair search carries forward per step.
///
/// A perimeter of remainder one cannot reach a multiple of three by adding a
/// single face — measured on the NXP=243 base, single additions to Case 9's
/// bad components reach only lengths 22, 23 and 26. Two faces do reach 24. A
/// search that keeps just the locally best trial can therefore commit to a
/// first face the fix does not lie behind. Width one is the greedy walk this
/// has always done, and stays the default.
fn repair_beam_width() -> usize {
    std::env::var("EARTHMESH_M0_REPAIR_BEAM_WIDTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1)
        .max(1)
}

/// How many faces the widened search may add before giving up.
fn repair_beam_depth() -> usize {
    std::env::var("EARTHMESH_M0_REPAIR_BEAM_DEPTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(3)
        .max(1)
}

impl MethodCDelaunayMesh {
    /// The parent level every selected face shares.
    ///
    /// Reading the first selected face is only sound because the caller has
    /// already established that they all agree:
    /// `spawn_nest_pass_method_c_repairing` runs
    /// `ensure_method_c_selected_faces_share_parent_mrlw` before the repair
    /// loop, `method_c_scored_repair_trials` re-runs it on every trial mask,
    /// and candidate enumeration admits only faces whose `mrlw` already equals
    /// the level returned here. Call this on a mask of mixed levels and it will
    /// answer for whichever face happens to come first.
    fn method_c_repair_parent_mrlw(&self, selected: &[bool]) -> io::Result<usize> {
        selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, is_selected)| is_selected.then_some(self.w_faces[iw].mrlw))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Method-C cannot repair an empty selected face mask",
                )
            })
    }

    /// Faces the transition would consume that the parent has not refined.
    ///
    /// A decomposable perimeter is not yet a materializable one: `perim_fill3`
    /// reads parent faces around each triple, and if the parent left one coarse
    /// the nested grid crosses its boundary. Counting them lets the search
    /// prefer a mask that satisfies both constraints rather than trading one
    /// for the other.
    fn method_c_unsupported_witness_lineages(
        &self,
        selected: &[bool],
        perimeter: &[MethodCPerimeterPoint],
        parent_level: usize,
    ) -> io::Result<BTreeSet<usize>> {
        let nest_wd = self.method_c_nest_wd_from_selected_and_perimeter(selected, perimeter)?;
        Ok(self
            .method_c_transition_parent_boundary_witnesses(perimeter, &nest_wd, parent_level)?
            .into_iter()
            .flat_map(|(_, faces)| faces)
            .filter(|&iw| self.w_faces[iw].mrlw < parent_level && !nest_wd[iw].is_subdivided())
            .map(|iw| self.w_lineage[iw])
            .collect())
    }

    /// Faces that may be added next: those touching the perimeter when one is
    /// known, otherwise those touching any M point the mask only partly covers.
    fn method_c_repair_candidate_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        perimeter: Option<&[MethodCPerimeterPoint]>,
        parent_mrlw: usize,
    ) -> io::Result<BTreeSet<usize>> {
        let mut candidates = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C repair candidate W face", iw, self.nwd)?;
                    if !selected[iw] && self.w_faces[iw].mrlw == parent_mrlw {
                        candidates.insert(iw);
                    }
                }
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C repair boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m == 0 || selected_count_at_m == neighbors.npoly {
                    continue;
                }
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    if !selected[iw] && self.w_faces[iw].mrlw == parent_mrlw {
                        candidates.insert(iw);
                    }
                }
            }
        }
        Ok(candidates)
    }

    /// Apply one candidate face to `scratch` and close the concavities it opens.
    ///
    /// `scratch` is overwritten, never read on entry, so the caller can hand the
    /// same buffer back for every candidate instead of allocating per trial.
    fn method_c_apply_repair_candidate(
        &self,
        selected: &[bool],
        candidate: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
        parent_mrlw: usize,
        scratch: &mut Vec<bool>,
    ) -> io::Result<()> {
        scratch.clear();
        scratch.extend_from_slice(selected);
        scratch[candidate] = true;
        let mut pending = BTreeSet::new();
        for im in self.w_faces[candidate].im {
            if (2..=self.nmd).contains(&im) {
                pending.insert(im);
            }
        }
        while !pending.is_empty() {
            let mut next_pass = BTreeSet::new();
            while let Some(im) = pending.pop_first() {
                let neighbors = m_neighbors[im];
                let mut selected_count = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C local concavity W face", iw, self.nwd)?;
                    selected_count += usize::from(scratch[iw]);
                }
                if selected_count == 0 || selected_count != neighbors.npoly.saturating_sub(1) {
                    continue;
                }
                let footprint = self.method_c_rad3_faces_with_neighbors(im, m_neighbors)?;
                if footprint
                    .iter()
                    .any(|&iw| iw >= 2 && self.w_faces[iw].mrlw != parent_mrlw)
                {
                    continue;
                }
                for iw in footprint {
                    if scratch[iw] {
                        continue;
                    }
                    scratch[iw] = true;
                    for next_im in self.w_faces[iw].im {
                        if !(2..=self.nmd).contains(&next_im) {
                            continue;
                        }
                        if next_im > im {
                            pending.insert(next_im);
                        } else {
                            next_pass.insert(next_im);
                        }
                    }
                }
            }
            pending = next_pass;
        }
        Ok(())
    }

    /// Evaluate every single-face addition to `selected`.
    ///
    /// With `support_aware` off this stops at the first decomposable mask, which
    /// is what the greedy walk has always done. With it on, a decomposable mask
    /// that the parent cannot support is not an answer — it stays in the running
    /// so the search can keep adding faces instead of committing to a perimeter
    /// that will fail materialization.
    ///
    /// The returned trials carry face ids, not masks; `scratch` holds whichever
    /// one was evaluated last and is not meaningful to the caller.
    fn method_c_scored_repair_trials(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
        support_aware: bool,
        exact_materialization: Option<(usize, bool)>,
        scratch: &mut Vec<bool>,
        perimeter_probe: &mut MethodCPerimeterProbe,
    ) -> io::Result<(Option<MethodCRepairTrial>, Vec<MethodCRepairTrial>)> {
        let parent_mrlw = self.method_c_repair_parent_mrlw(selected)?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let candidates =
            self.method_c_repair_candidate_faces(selected, m_neighbors, perimeter, parent_mrlw)?;

        let mut scored = Vec::new();
        for candidate in candidates {
            self.method_c_apply_repair_candidate(
                selected,
                candidate,
                m_neighbors,
                parent_mrlw,
                scratch,
            )?;
            if !Self::method_c_repair_candidate_preserves_coverage(coverage, scratch) {
                continue;
            }
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(scratch, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeters) = self.method_c_perimeters_from_selected_faces_with_probe(
                scratch,
                m_neighbors,
                perimeter_probe,
            ) else {
                continue;
            };
            let trial_perimeter = trial_perimeters
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let added = scratch.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
                continue;
            }
            let decomposes = Self::method_c_perimeters_are_triplets(&trial_perimeters);
            if decomposes && !support_aware {
                return Ok((
                    Some(MethodCRepairTrial {
                        candidate,
                        added,
                        remainder: 0,
                        unsupported: 0,
                        perimeter: trial_perimeter,
                    }),
                    scored,
                ));
            }
            // Only the count survives: it is what `beam_key` ranks on. The
            // lineages themselves have no consumer -- the parent-support path
            // recomputes them from the mask it finally accepts -- so keeping a
            // set per trial would just be a set the search never reads.
            let unsupported = if decomposes {
                self.method_c_unsupported_witness_lineages(scratch, &trial_perimeter, parent_mrlw)?
                    .len()
            } else {
                0
            };
            let scored_trial = MethodCRepairTrial {
                candidate,
                added,
                remainder: Self::method_c_perimeter_remainder_score(&trial_perimeters),
                unsupported,
                perimeter: trial_perimeter,
            };
            if decomposes && unsupported == 0 {
                let exact = exact_materialization.is_none_or(|(max_mrows, project_to_radius)| {
                    self.emit_method_c_selected_faces(
                        scratch,
                        m_neighbors,
                        child_level,
                        max_mrows,
                        project_to_radius,
                    )
                    .is_ok()
                });
                if exact {
                    return Ok((Some(scored_trial), scored));
                }
            }
            scored.push(scored_trial);
        }
        Ok((None, scored))
    }

    /// Rebuild the mask a scored trial stands for.
    fn method_c_repair_trial_mask(
        &self,
        selected: &[bool],
        candidate: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<bool>> {
        let mut mask = Vec::with_capacity(selected.len());
        let parent_mrlw = self.method_c_repair_parent_mrlw(selected)?;
        self.method_c_apply_repair_candidate(
            selected,
            candidate,
            m_neighbors,
            parent_mrlw,
            &mut mask,
        )?;
        Ok(mask)
    }

    pub(crate) fn try_grow_method_c_non_triplet_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let mut scratch = Vec::with_capacity(selected.len());
        let mut perimeter_probe = MethodCPerimeterProbe::default();
        let (solved, mut scored) = self.method_c_scored_repair_trials(
            selected,
            m_neighbors,
            child_level,
            perimeter,
            coverage,
            false,
            None,
            &mut scratch,
            &mut perimeter_probe,
        )?;
        let chosen = match solved {
            Some(solved) => Some(solved),
            None => {
                scored.sort_by_key(MethodCRepairTrial::greedy_key);
                scored.into_iter().next()
            }
        };
        let Some(chosen) = chosen else {
            return Ok(None);
        };
        let mask = self.method_c_repair_trial_mask(selected, chosen.candidate, m_neighbors)?;
        Ok(Some((mask, chosen.perimeter)))
    }

    /// Widened, support-aware repair: keep the best `width` trials at each step
    /// rather than committing to one, and walk up to `depth` faces deep.
    ///
    /// Two things defeat the greedy walk. It minimises faces added before it
    /// minimises the remainder, so it prefers a step that leaves the perimeter
    /// undecomposable over one that costs a face more but sits next to the fix;
    /// and it accepts the first decomposable perimeter it sees even when the
    /// parent has not refined the faces that perimeter's transition needs.
    /// Carrying several trials forward addresses the first, and scoring
    /// unsupported witnesses addresses the second. When exact materialization
    /// is requested, a merely decomposable and supported mask stays in the
    /// beam unless the canonical emitter accepts it.
    ///
    /// Returns a mask that decomposes, is supported, and passes the exact
    /// emitter when that final gate is enabled.
    /// Failing that it falls back to the best decomposable mask, and failing
    /// that to the best first step, so the caller's outer loop still advances.
    pub(crate) fn try_grow_method_c_non_triplet_perimeter_beam(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
        width: usize,
        depth: usize,
        exact_materialization: Option<(usize, bool)>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let mut scratch = Vec::with_capacity(selected.len());
        let mut perimeter_probe = MethodCPerimeterProbe::default();
        let mut frontier = vec![(selected.to_vec(), perimeter.map(<[_]>::to_vec))];
        let mut decomposable_fallback: Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)> = None;
        let mut first_step_fallback: Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)> = None;

        for _ in 0..depth {
            let mut scored = Vec::new();
            for (parent_index, (mask, mask_perimeter)) in frontier.iter().enumerate() {
                let (solved, trials) = self.method_c_scored_repair_trials(
                    mask,
                    m_neighbors,
                    child_level,
                    mask_perimeter.as_deref(),
                    coverage,
                    true,
                    exact_materialization,
                    &mut scratch,
                    &mut perimeter_probe,
                )?;
                if let Some(solved) = solved {
                    let repaired =
                        self.method_c_repair_trial_mask(mask, solved.candidate, m_neighbors)?;
                    return Ok(Some((repaired, solved.perimeter)));
                }
                scored.extend(trials.into_iter().map(|trial| (parent_index, trial)));
            }
            if scored.is_empty() {
                break;
            }
            scored.sort_by_key(|(_, trial)| trial.beam_key());
            if decomposable_fallback.is_none() {
                if let Some((parent_index, trial)) = scored.iter().find(|(_, trial)| {
                    trial.remainder == 0
                        && (trial.unsupported > 0 || exact_materialization.is_none())
                }) {
                    let parent = &frontier[*parent_index].0;
                    decomposable_fallback = Some((
                        self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?,
                        trial.perimeter.clone(),
                    ));
                }
            }
            if first_step_fallback.is_none() {
                if let Some((parent_index, trial)) = scored
                    .iter()
                    .filter(|(_, trial)| {
                        exact_materialization.is_none()
                            || trial.remainder != 0
                            || trial.unsupported > 0
                    })
                    .min_by_key(|(_, trial)| trial.greedy_key())
                {
                    let parent = &frontier[*parent_index].0;
                    first_step_fallback = Some((
                        self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?,
                        trial.perimeter.clone(),
                    ));
                }
            }
            // Different shapes with the same face/perimeter counts are not
            // equivalent. Drop only masks that are actually identical.
            let mut next: Vec<(Vec<bool>, Option<Vec<MethodCPerimeterPoint>>)> =
                Vec::with_capacity(width);
            for (parent_index, trial) in scored {
                if next.len() >= width {
                    break;
                }
                let parent = &frontier[parent_index].0;
                let mask = self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?;
                if next.iter().any(|(existing, _)| existing == &mask) {
                    continue;
                }
                next.push((mask, Some(trial.perimeter)));
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }

        Ok(decomposable_fallback.or(first_step_fallback))
    }

    /// Repair entry point: greedy by default, widened when configured.
    pub(crate) fn try_grow_method_c_non_triplet_perimeter(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
        exact_materialization: Option<(usize, bool)>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let width = repair_beam_width();
        if width <= 1 {
            return self.try_grow_method_c_non_triplet_perimeter_once(
                selected,
                m_neighbors,
                child_level,
                perimeter,
                coverage,
            );
        }
        self.try_grow_method_c_non_triplet_perimeter_beam(
            selected,
            m_neighbors,
            child_level,
            perimeter,
            coverage,
            width,
            repair_beam_depth(),
            exact_materialization,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn local_candidate_closure_matches_full_closure() {
        for nxp in [6, 7] {
            let mesh = MethodCDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0)
                .expect("build canonical mesh");
            let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
            let mut selected = vec![false; mesh.nwd + 1];
            for seed in [3, mesh.nmd / 2, mesh.nmd.saturating_sub(2)] {
                if let Ok(footprint) = mesh.method_c_rad3_faces_with_neighbors(seed, &m_neighbors) {
                    for iw in footprint {
                        if iw >= 2 && iw <= mesh.nwd {
                            selected[iw] = true;
                        }
                    }
                }
            }
            mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
                .expect("close base mask");
            let parent_mrlw = mesh
                .method_c_repair_parent_mrlw(&selected)
                .expect("parent level");

            let candidates = mesh
                .method_c_repair_candidate_faces(&selected, &m_neighbors, None, parent_mrlw)
                .expect("repair candidates");
            for candidate in candidates {
                let mut local = Vec::new();
                mesh.method_c_apply_repair_candidate(
                    &selected,
                    candidate,
                    &m_neighbors,
                    parent_mrlw,
                    &mut local,
                )
                .expect("local closure");

                let mut full = selected.clone();
                full[candidate] = true;
                mesh.close_method_c_concavities_for_level_with_neighbors(&mut full, &m_neighbors)
                    .expect("full closure");
                if local != full {
                    let differences = local
                        .iter()
                        .zip(&full)
                        .enumerate()
                        .filter_map(|(iw, (local, full))| (local != full).then_some(iw))
                        .collect::<Vec<_>>();
                    let incident_m = (2..=mesh.nmd)
                        .filter(|&im| {
                            let neighbors = m_neighbors[im];
                            neighbors
                                .iw
                                .iter()
                                .take(neighbors.npoly)
                                .any(|&iw| iw == candidate)
                        })
                        .collect::<Vec<_>>();
                    panic!(
                        "nxp={nxp} candidate={candidate} face={:?} incident_m={incident_m:?} differences={differences:?}",
                        mesh.w_faces[candidate].im,
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "timing measurement, run explicitly"]
    fn local_candidate_closure_cost() {
        const REPS: usize = 100;
        let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
            .expect("build canonical mesh");
        let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
        let mut selected = vec![false; mesh.nwd + 1];
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(mesh.nmd / 2, &m_neighbors)
            .expect("seed footprint")
        {
            selected[iw] = true;
        }
        mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
            .expect("close base mask");
        let parent_mrlw = mesh
            .method_c_repair_parent_mrlw(&selected)
            .expect("parent level");
        let candidate = *mesh
            .method_c_repair_candidate_faces(&selected, &m_neighbors, None, parent_mrlw)
            .expect("repair candidates")
            .first()
            .expect("candidate");

        let mut local = Vec::new();
        let started = Instant::now();
        for _ in 0..REPS {
            mesh.method_c_apply_repair_candidate(
                &selected,
                candidate,
                &m_neighbors,
                parent_mrlw,
                &mut local,
            )
            .expect("local closure");
        }
        let local_time = started.elapsed();

        let started = Instant::now();
        let mut full = Vec::new();
        for _ in 0..REPS {
            full.clone_from(&selected);
            full[candidate] = true;
            mesh.close_method_c_concavities_for_level_with_neighbors(&mut full, &m_neighbors)
                .expect("full closure");
        }
        let full_time = started.elapsed();

        assert_eq!(local, full);
        eprintln!(
            "candidate closure ms: local={:.3} full={:.3}",
            local_time.as_secs_f64() * 1000.0 / REPS as f64,
            full_time.as_secs_f64() * 1000.0 / REPS as f64,
        );
    }
}
