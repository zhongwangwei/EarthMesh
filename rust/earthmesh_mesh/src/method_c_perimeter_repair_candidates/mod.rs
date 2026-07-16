use std::{collections::BTreeSet, io};

use super::*;

impl MethodCDelaunayMesh {
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
        let mut best: Option<(usize, usize, usize, Vec<bool>, Vec<MethodCPerimeterPoint>)> = None;
        for im in boundary_m {
            let mut trial = selected.to_vec();
            self.mark_fill_rad3_faces_with_neighbors(im, &mut trial, m_neighbors)?;
            self.close_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
            let added = trial.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
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
            let remainder = Self::method_c_perimeter_remainder_score(&trial_perimeters);
            let trial_perimeter = trial_perimeters.into_iter().flatten().collect::<Vec<_>>();
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
