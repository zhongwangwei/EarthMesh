use std::{collections::BTreeSet, io};

use super::*;

impl MethodCMesh {
    pub(crate) fn try_shrink_method_c_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        // Every candidate along the perimeter: searched near the failing point
        // only, a global 30 km slope run chose a shrink that left a
        // one-face-thick mask and failed. The scoring is incremental instead.
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut candidates = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C shrink candidate W face", iw, self.nwd)?;
                    if selected[iw] {
                        candidates.insert(iw);
                    }
                }
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_method_c_id("Method-C shrink boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m > 0 && selected_count_at_m < neighbors.npoly {
                    for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                        if selected[iw] {
                            candidates.insert(iw);
                        }
                    }
                }
            }
        }

        // Scored from what each removal changes (`method_c_perimeter_incremental`).
        let incremental = if crate::method_c_perimeter_incremental::whole_mesh_only() {
            None
        } else {
            crate::method_c_perimeter_incremental::IncrementalSelection::new(
                self,
                selected,
                m_neighbors,
            )?
        };
        if let Some(mut incremental) = incremental {
            use crate::method_c_perimeter_incremental::Trial;
            let mut best: Option<(usize, usize, usize, Option<usize>, Vec<usize>)> = None;
            for candidate in candidates {
                let Trial::Scored(score) = incremental.trial_without(candidate)? else {
                    continue;
                };
                // The removed face is off, and back among `added` if the
                // closure put it back.
                let trial_count = selected_count - 1 + score.added.len();
                if trial_count == 0 || trial_count >= selected_count {
                    continue;
                }
                let key = (selected_count - trial_count, score.remainder, score.length);
                if best
                    .as_ref()
                    .is_none_or(|current| key < (current.0, current.1, current.2))
                {
                    best = Some((key.0, key.1, key.2, score.removed, score.added));
                }
            }
            return match best {
                None => Ok(None),
                Some((_, _, _, removed, added)) => {
                    let trial = incremental.with_changes(removed, &added);
                    let perimeters =
                        self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)?;
                    Ok(Some((trial, perimeters.concat())))
                }
            };
        }

        let mut best: Option<(usize, usize, usize, Vec<bool>, Vec<MethodCPerimeterPoint>)> = None;
        for candidate in candidates {
            let mut trial = selected.to_vec();
            trial[candidate] = false;
            self.close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
            let trial_count = trial.iter().filter(|&&item| item).count();
            if trial_count == 0 || trial_count >= selected_count {
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
            let removed = selected_count - trial_count;
            let remainder = Self::method_c_perimeter_remainder_score(&trial_perimeters);
            let score = (
                removed,
                remainder,
                trial_perimeter.len(),
                trial,
                trial_perimeter,
            );
            if best.as_ref().is_none_or(|current| {
                (score.0, score.1, score.2) < (current.0, current.1, current.2)
            }) {
                best = Some(score);
            }
        }

        Ok(best.map(|(_, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }
}
