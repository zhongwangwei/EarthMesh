use std::{collections::BTreeSet, io};

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn try_shrink_method_c_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
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
            let Ok(trial_perimeter) =
                self.method_c_perimeter_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            let removed = selected_count - trial_count;
            let remainder = trial_perimeter.len() % 3;
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
