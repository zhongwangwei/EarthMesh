use std::{collections::BTreeSet, io};

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
    unsupported_lineages: BTreeSet<usize>,
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
        scratch: &mut Vec<bool>,
    ) -> io::Result<()> {
        scratch.clear();
        scratch.extend_from_slice(selected);
        scratch[candidate] = true;
        self.close_method_c_concavities_for_level_with_neighbors(scratch, m_neighbors)
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
        scratch: &mut Vec<bool>,
    ) -> io::Result<(Option<MethodCRepairTrial>, Vec<MethodCRepairTrial>)> {
        let parent_mrlw = self.method_c_repair_parent_mrlw(selected)?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let candidates =
            self.method_c_repair_candidate_faces(selected, m_neighbors, perimeter, parent_mrlw)?;

        let mut scored = Vec::new();
        for candidate in candidates {
            self.method_c_apply_repair_candidate(selected, candidate, m_neighbors, scratch)?;
            if !Self::method_c_repair_candidate_preserves_coverage(coverage, scratch) {
                continue;
            }
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(scratch, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeters) =
                self.method_c_perimeters_from_selected_faces(scratch, m_neighbors)
            else {
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
                        unsupported_lineages: BTreeSet::new(),
                        perimeter: trial_perimeter,
                    }),
                    scored,
                ));
            }
            let unsupported_lineages = if decomposes {
                self.method_c_unsupported_witness_lineages(scratch, &trial_perimeter, parent_mrlw)?
            } else {
                BTreeSet::new()
            };
            let unsupported = unsupported_lineages.len();
            let scored_trial = MethodCRepairTrial {
                candidate,
                added,
                remainder: Self::method_c_perimeter_remainder_score(&trial_perimeters),
                unsupported,
                unsupported_lineages,
                perimeter: trial_perimeter,
            };
            if decomposes && unsupported == 0 {
                return Ok((Some(scored_trial), scored));
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
        self.method_c_apply_repair_candidate(selected, candidate, m_neighbors, &mut mask)?;
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
        let (solved, mut scored) = self.method_c_scored_repair_trials(
            selected,
            m_neighbors,
            child_level,
            perimeter,
            coverage,
            false,
            &mut scratch,
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
    /// unsupported witnesses addresses the second.
    ///
    /// Returns a mask that both decomposes and is supported when one is found.
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
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let mut scratch = Vec::with_capacity(selected.len());
        let mut frontier = vec![(selected.to_vec(), perimeter.map(<[_]>::to_vec))];
        let mut decomposable_fallback: Option<(Vec<bool>, Vec<MethodCPerimeterPoint>, BTreeSet<usize>)> =
            None;
        let mut first_step_fallback: Option<(Vec<bool>, Vec<MethodCPerimeterPoint>, BTreeSet<usize>)> =
            None;

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
                    &mut scratch,
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
                if let Some((parent_index, trial)) =
                    scored.iter().find(|(_, trial)| trial.remainder == 0)
                {
                    let parent = &frontier[*parent_index].0;
                    decomposable_fallback = Some((
                        self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?,
                        trial.perimeter.clone(),
                        trial.unsupported_lineages.clone(),
                    ));
                }
            }
            if first_step_fallback.is_none() {
                let (parent_index, trial) = scored
                    .iter()
                    .min_by_key(|(_, trial)| trial.greedy_key())
                    .expect("scored is non-empty");
                let parent = &frontier[*parent_index].0;
                first_step_fallback = Some((
                    self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?,
                    trial.perimeter.clone(),
                    trial.unsupported_lineages.clone(),
                ));
            }
            // Face count and perimeter length together stand in for the shape,
            // so the frontier does not fill up with the same trial reached by
            // different orders of the same two additions. Only survivors get a
            // mask built for them.
            let parent_face_counts = frontier
                .iter()
                .map(|(mask, _)| mask.iter().filter(|&&item| item).count())
                .collect::<Vec<_>>();
            let mut seen = BTreeSet::new();
            let mut next = Vec::with_capacity(width);
            for (parent_index, trial) in scored {
                if next.len() >= width {
                    break;
                }
                let faces = parent_face_counts[parent_index] + trial.added;
                if !seen.insert((faces, trial.perimeter.len())) {
                    continue;
                }
                let parent = &frontier[parent_index].0;
                let mask = self.method_c_repair_trial_mask(parent, trial.candidate, m_neighbors)?;
                next.push((mask, Some(trial.perimeter)));
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }

        // A decomposable perimeter the parent cannot support is not a dead
        // end, it is a request. The outer support loop rewinds to the parent
        // pass and refines these faces, so hand them over rather than emitting
        // a mask that is known to fail materialization.
        Ok(decomposable_fallback
            .or(first_step_fallback)
            .map(|(mask, trial_perimeter, unsupported_lineages)| {
                if !unsupported_lineages.is_empty() {
                    crate::method_c_perimeter_repair::record_post_drop_support(
                        unsupported_lineages,
                    );
                }
                (mask, trial_perimeter)
            }))
    }

    /// Repair entry point: greedy by default, widened when configured.
    pub(crate) fn try_grow_method_c_non_triplet_perimeter(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
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
        )
    }
}
