use std::{collections::BTreeSet, io};

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn try_grow_method_c_non_triplet_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[OlamMethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<OlamMethodCPerimeterPoint>)>> {
        let parent_mrlw = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, is_selected)| is_selected.then_some(self.w_faces[iw].mrlw))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "OLAM Method-C cannot repair an empty selected face mask",
                )
            })?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut candidates = BTreeSet::new();

        if let Some(perimeter) = perimeter {
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C repair candidate W face", iw, self.nwd)?;
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
                    require_olam_id("OLAM Method-C repair boundary W face", iw, self.nwd)?;
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

        let mut best: Option<(
            usize,
            usize,
            usize,
            Vec<bool>,
            Vec<OlamMethodCPerimeterPoint>,
        )> = None;
        for candidate in candidates {
            let mut trial = selected.to_vec();
            trial[candidate] = true;
            self.close_olam_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
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
            let added = trial.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
                continue;
            }
            let remainder = trial_perimeter.len() % 3;
            if remainder == 0 {
                return Ok(Some((trial, trial_perimeter)));
            }
            let score = (
                added,
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
