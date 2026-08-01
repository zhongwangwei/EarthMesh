use std::{collections::BTreeSet, io};

use super::*;

/// One candidate face addition, scored.
///
/// `remainder` is zero exactly when every perimeter decomposes into transition
/// triples. `unsupported` is only meaningful once it does — counting the parent
/// faces a transition would consume costs a full `nest_wd` build, so the search
/// pays for it only on masks it might actually accept.
struct MethodCRepairTrial {
    added: usize,
    remainder: usize,
    unsupported: usize,
    mask: Vec<bool>,
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
    fn method_c_unsupported_witness_count(
        &self,
        selected: &[bool],
        perimeter: &[MethodCPerimeterPoint],
        parent_level: usize,
    ) -> io::Result<usize> {
        let nest_wd = self.method_c_nest_wd_from_selected_and_perimeter(selected, perimeter)?;
        Ok(self
            .method_c_transition_parent_boundary_witnesses(perimeter, &nest_wd, parent_level)?
            .into_iter()
            .flat_map(|(_, faces)| faces)
            .filter(|&iw| self.w_faces[iw].mrlw < parent_level && !nest_wd[iw].is_subdivided())
            .count())
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

    /// Evaluate every single-face addition to `selected`.
    ///
    /// With `support_aware` off this stops at the first decomposable mask, which
    /// is what the greedy walk has always done. With it on, a decomposable mask
    /// that the parent cannot support is not an answer — it stays in the running
    /// so the search can keep adding faces instead of committing to a perimeter
    /// that will fail materialization.
    fn method_c_scored_repair_trials(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
        support_aware: bool,
    ) -> io::Result<(Option<MethodCRepairTrial>, Vec<MethodCRepairTrial>)> {
        let parent_mrlw = self.method_c_repair_parent_mrlw(selected)?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let candidates =
            self.method_c_repair_candidate_faces(selected, m_neighbors, perimeter, parent_mrlw)?;

        let mut scored = Vec::new();
        for candidate in candidates {
            let mut trial = selected.to_vec();
            trial[candidate] = true;
            self.close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
            if !Self::method_c_repair_candidate_preserves_coverage(coverage, &trial) {
                continue;
            }
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeters) =
                self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            let trial_perimeter = trial_perimeters
                .iter()
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            let added = trial.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
                continue;
            }
            let decomposes = Self::method_c_perimeters_are_triplets(&trial_perimeters);
            if decomposes && !support_aware {
                return Ok((
                    Some(MethodCRepairTrial {
                        added,
                        remainder: 0,
                        unsupported: 0,
                        mask: trial,
                        perimeter: trial_perimeter,
                    }),
                    scored,
                ));
            }
            let unsupported = if decomposes {
                self.method_c_unsupported_witness_count(&trial, &trial_perimeter, parent_mrlw)?
            } else {
                0
            };
            let scored_trial = MethodCRepairTrial {
                added,
                remainder: Self::method_c_perimeter_remainder_score(&trial_perimeters),
                unsupported,
                mask: trial,
                perimeter: trial_perimeter,
            };
            if decomposes && unsupported == 0 {
                return Ok((Some(scored_trial), scored));
            }
            scored.push(scored_trial);
        }
        Ok((None, scored))
    }

    pub(crate) fn try_grow_method_c_non_triplet_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        let (solved, mut scored) = self.method_c_scored_repair_trials(
            selected,
            m_neighbors,
            child_level,
            perimeter,
            coverage,
            false,
        )?;
        if let Some(solved) = solved {
            return Ok(Some((solved.mask, solved.perimeter)));
        }
        scored.sort_by_key(MethodCRepairTrial::greedy_key);
        Ok(scored
            .into_iter()
            .next()
            .map(|trial| (trial.mask, trial.perimeter)))
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
        let mut frontier = vec![(selected.to_vec(), perimeter.map(<[_]>::to_vec))];
        let mut decomposable_fallback: Option<MethodCRepairTrial> = None;
        let mut first_step_fallback: Option<MethodCRepairTrial> = None;

        for _ in 0..depth {
            let mut scored = Vec::new();
            for (mask, mask_perimeter) in &frontier {
                let (solved, trials) = self.method_c_scored_repair_trials(
                    mask,
                    m_neighbors,
                    child_level,
                    mask_perimeter.as_deref(),
                    coverage,
                    true,
                )?;
                if let Some(solved) = solved {
                    return Ok(Some((solved.mask, solved.perimeter)));
                }
                scored.extend(trials);
            }
            if scored.is_empty() {
                break;
            }
            scored.sort_by_key(MethodCRepairTrial::beam_key);
            if decomposable_fallback.is_none() {
                if let Some(position) = scored.iter().position(|trial| trial.remainder == 0) {
                    let trial = &scored[position];
                    decomposable_fallback = Some(MethodCRepairTrial {
                        added: trial.added,
                        remainder: trial.remainder,
                        unsupported: trial.unsupported,
                        mask: trial.mask.clone(),
                        perimeter: trial.perimeter.clone(),
                    });
                }
            }
            if first_step_fallback.is_none() {
                let best = scored
                    .iter()
                    .min_by_key(|trial| trial.greedy_key())
                    .expect("scored is non-empty");
                first_step_fallback = Some(MethodCRepairTrial {
                    added: best.added,
                    remainder: best.remainder,
                    unsupported: best.unsupported,
                    mask: best.mask.clone(),
                    perimeter: best.perimeter.clone(),
                });
            }
            // Face count and perimeter length together stand in for the shape,
            // so the frontier does not fill up with the same trial reached by
            // different orders of the same two additions.
            let mut seen = BTreeSet::new();
            frontier = scored
                .into_iter()
                .filter(|trial| {
                    seen.insert((
                        trial.mask.iter().filter(|&&item| item).count(),
                        trial.perimeter.len(),
                    ))
                })
                .take(width)
                .map(|trial| (trial.mask, Some(trial.perimeter)))
                .collect();
        }

        Ok(decomposable_fallback
            .or(first_step_fallback)
            .map(|trial| (trial.mask, trial.perimeter)))
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
