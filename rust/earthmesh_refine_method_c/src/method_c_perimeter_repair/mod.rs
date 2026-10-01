use std::io;

use super::*;

/// `EARTHMESH_METHOD_C_REPAIR_TRACE=1`: say what each repair pass does.
pub(crate) fn repair_trace() -> bool {
    std::env::var("EARTHMESH_METHOD_C_REPAIR_TRACE").as_deref() == Ok("1")
}

/// Blocks short of a triple above which they are grown together.
pub(crate) const BATCH_ABOVE_BLOCKS: usize = 8;

impl MethodCMesh {
    /// One growth per block short of a triple, chosen as the grower would
    /// choose it near that block alone, kept only if it lowers the blocks'
    /// remainder, and applied together where the changes touch no common
    /// point. `None` when no block has such a growth.
    fn grow_non_triplet_blocks_together(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        off: &[Vec<MethodCPerimeterPoint>],
    ) -> io::Result<Option<Vec<bool>>> {
        use crate::method_c_perimeter_incremental::{IncrementalSelection, Trial};
        use std::collections::BTreeSet;
        let Some(mut incremental) = IncrementalSelection::new(self, selected, m_neighbors)? else {
            return Ok(None);
        };
        let parent_mrlw = (2..=self.nwd)
            .find(|&iw| selected[iw])
            .map(|iw| self.w_faces[iw].mrlw);
        let base_remainder = incremental.remainder();
        let mut chosen: Vec<Vec<usize>> = Vec::new();
        for perimeter in off {
            let mut candidates = BTreeSet::new();
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in &neighbors.iw[..neighbors.npoly] {
                    if !selected[iw] && Some(self.w_faces[iw].mrlw) == parent_mrlw {
                        candidates.insert(iw);
                    }
                }
            }
            let mut best: Option<((usize, usize, usize), Vec<usize>)> = None;
            for candidate in candidates {
                let Trial::Scored(score) = incremental.trial(None, &[candidate])? else {
                    continue;
                };
                if score.remainder >= base_remainder {
                    continue;
                }
                let key = (score.added.len(), score.remainder, score.length);
                if best.as_ref().is_none_or(|(current, _)| key < *current) {
                    best = Some((key, score.added));
                }
            }
            if let Some((_, added)) = best {
                chosen.push(added);
            }
        }
        let mut touched_so_far = BTreeSet::new();
        let mut grown = selected.to_vec();
        let mut applied = 0usize;
        for added in chosen {
            let touched: BTreeSet<usize> =
                added.iter().flat_map(|&iw| self.w_faces[iw].im).collect();
            if !touched.is_disjoint(&touched_so_far) {
                continue;
            }
            touched_so_far.extend(touched);
            for iw in added {
                grown[iw] = true;
            }
            applied += 1;
        }
        if repair_trace() {
            eprintln!("earthmesh_cli: method-c repair: {applied} growths applied together");
        }
        if applied == 0 {
            return Ok(None);
        }
        self.close_method_c_concavities_for_level_with_neighbors(&mut grown, m_neighbors)?;
        if self
            .ensure_method_c_selected_faces_share_parent_mrlw(&grown, child_level)
            .is_err()
        {
            return Ok(None);
        }
        Ok(Some(grown))
    }

    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        method_c_repairable_payload(error).is_some()
    }

    pub(crate) fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let payload = method_c_repairable_payload(error)?;
        (payload.kind == RepairableKind::Valence)
            .then_some(payload.m_point)
            .flatten()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        // Twelve passes per offending block, not twelve for the whole mesh. A
        // global coastal case came out of selection with 27 refined blocks, of
        // which three had a perimeter one short of a multiple of three; the
        // budget was spent long before the search reached them.
        const MAX_REPAIR_PASSES_PER_BLOCK: usize = 12;

        let mut last_error = None;
        let block_count = self
            .method_c_perimeters_from_selected_faces(selected, m_neighbors)
            .map(|perimeters| perimeters.len())
            .unwrap_or(1)
            .max(1);
        let max_passes = MAX_REPAIR_PASSES_PER_BLOCK.saturating_mul(block_count);
        for _ in 0..max_passes {
            // Search around the blocks that are not yet a multiple of three.
            // Handing the grower every perimeter buries the ones that need work:
            // the scoring is global, so a pass keeps picking candidates near a
            // block that is already fine.
            let perimeter = match self
                .method_c_perimeters_from_selected_faces(selected, m_neighbors)
            {
                Ok(perimeters) if Self::method_c_perimeters_are_triplets(&perimeters) => {
                    return Ok(perimeters.into_iter().flatten().collect());
                }
                Ok(perimeters) => {
                    let off: Vec<Vec<MethodCPerimeterPoint>> = perimeters
                        .into_iter()
                        .filter(|perimeter| !perimeter.len().is_multiple_of(3))
                        .collect();
                    // Many blocks short of a triple are grown together, one
                    // candidate each, where their changes do not meet: one at
                    // a time, a global 12 km slope selection spent hours in
                    // whole-mesh passes, one block per pass.
                    if off.len() > BATCH_ABOVE_BLOCKS {
                        let grown = self.grow_non_triplet_blocks_together(
                            selected,
                            m_neighbors,
                            child_level,
                            &off,
                        )?;
                        if repair_trace() {
                            eprintln!(
                                "earthmesh_cli: method-c repair: {} blocks short of a triple, \
                                 grown together: {}",
                                off.len(),
                                grown.is_some()
                            );
                        }
                        if let Some(grown) = grown {
                            selected.clone_from_slice(&grown);
                            continue;
                        }
                    } else if repair_trace() {
                        eprintln!(
                            "earthmesh_cli: method-c repair: {} blocks short of a triple",
                            off.len()
                        );
                    }
                    Some(off.into_iter().flatten().collect::<Vec<_>>())
                }
                Err(error) => {
                    // A pinch -- the selection meeting itself at a point --
                    // stops the perimeter walk. Filling one at a time never
                    // lets a selection with many walk (a global slope
                    // selection at 30 km has them wherever steep blocks meet
                    // corner to corner), so all of them are filled at once.
                    if let Some(filled) =
                        self.fill_method_c_selection_pinches(selected, m_neighbors, child_level)?
                    {
                        selected.clone_from_slice(&filled);
                        last_error = Some(error);
                        continue;
                    }
                    last_error = Some(error);
                    None
                }
            };
            let Some((repaired, _)) = self.try_grow_method_c_non_triplet_perimeter_once(
                selected,
                m_neighbors,
                child_level,
                perimeter.as_deref(),
            )?
            else {
                break;
            };
            selected.clone_from_slice(&repaired);
            // A growth can still leave a pinch elsewhere; the next pass fills
            // it rather than giving up on the whole level here.
            match self.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                Ok(repaired_perimeters)
                    if Self::method_c_perimeters_are_triplets(&repaired_perimeters) =>
                {
                    return Ok(repaired_perimeters.into_iter().flatten().collect());
                }
                Ok(_) => {}
                Err(error) => last_error = Some(error),
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        let perimeters = self.method_c_perimeters_from_selected_faces(selected, m_neighbors)?;
        // A point on the first block still short of a triple, so the failure
        // can be put down to that block.
        let short = perimeters
            .iter()
            .find(|perimeter| !perimeter.len().is_multiple_of(3))
            .and_then(|perimeter| perimeter.first())
            .map(|point| point.im);
        Err(repairable_error(
            RepairableKind::NonTripletPerimeter,
            short,
            format!(
                "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples without crossing the parent boundary",
                perimeters.iter().map(Vec::len).collect::<Vec<_>>()
            ),
        ))
    }
}

impl MethodCMesh {
    /// The selection with every pinch filled: each M point whose selected
    /// faces form two or more separate fans gets all of its faces. `None`
    /// when there are fewer than two, or when filling them would mix parent
    /// levels.
    fn fill_method_c_selection_pinches(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Option<Vec<bool>>> {
        let mut trial = selected.to_vec();
        let mut pinches = 0usize;
        for im in 2..=self.nmd {
            if self.selected_fans_around(im, selected, m_neighbors)? >= 2 {
                // The faces around the point only: a radius-3 fill here broke
                // the valence of a single pinch the grower otherwise repairs.
                let neighbors = m_neighbors[im];
                for &iw in &neighbors.iw[..neighbors.npoly] {
                    trial[iw] = true;
                }
                pinches += 1;
            }
        }
        // One pinch is the grower's: it closes it with the growth the level
        // needs anyway (two touching discs, a vertex-only contact), where a
        // plain fill led on to a valence refusal. Many are beyond it -- it moves
        // one place per pass.
        if pinches < 2 {
            return Ok(None);
        }
        self.close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
        if trial == selected
            || self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                .is_err()
        {
            return Ok(None);
        }
        Ok(Some(trial))
    }

    /// How many separate fans the selected faces around `im` form, joined
    /// across the U edges at `im` whose two faces are both selected.
    fn selected_fans_around(
        &self,
        im: usize,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<usize> {
        let neighbors = m_neighbors[im];
        let faces = &neighbors.iw[..neighbors.npoly];
        let mut fan = [usize::MAX; 7];
        let find = |fan: &[usize; 7], mut k: usize| {
            while fan[k] != k {
                k = fan[k];
            }
            k
        };
        for (k, &iw) in faces.iter().enumerate() {
            require_method_c_id("Method-C pinch W face", iw, self.nwd)?;
            if selected[iw] {
                fan[k] = k;
            }
        }
        for &iu in &neighbors.iu[..neighbors.npoly] {
            require_method_c_id("Method-C pinch U edge", iu, self.nud)?;
            let edge = self.u_edges[iu];
            let (iw1, iw2) = (edge.iw[0], edge.iw[1]);
            if !(selected[iw1] && selected[iw2]) {
                continue;
            }
            let (Some(a), Some(b)) = (
                faces.iter().position(|&iw| iw == iw1),
                faces.iter().position(|&iw| iw == iw2),
            ) else {
                continue;
            };
            let (ra, rb) = (find(&fan, a), find(&fan, b));
            if ra != rb {
                fan[ra] = rb;
            }
        }
        Ok((0..faces.len())
            .filter(|&k| fan[k] != usize::MAX && find(&fan, k) == k)
            .count())
    }
}
