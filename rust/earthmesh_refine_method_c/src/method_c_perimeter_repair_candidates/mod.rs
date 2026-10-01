use std::{collections::BTreeSet, io};

use super::*;

impl MethodCMesh {
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
        focus: Option<usize>,
    ) -> io::Result<Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>> {
        // Each candidate is a whole-mesh trial (a copy, a concavity sweep, the
        // perimeters again), so trying every point of a long perimeter is
        // quadratic: a global slope selection at 30 km spent most of its run
        // here. A long perimeter with a known failing point is searched near
        // that point only; a short one, or one without, as before.
        const LONG_PERIMETER: usize = 96;
        const FOCUS_REACH: usize = 24;
        let mut boundary_m = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            let near = focus
                .filter(|_| perimeter.len() > LONG_PERIMETER)
                .map(|im| {
                    perimeter
                        .iter()
                        .enumerate()
                        .filter(|(_, point)| point.im == im)
                        .map(|(k, _)| k)
                        .collect::<Vec<_>>()
                })
                .filter(|at| !at.is_empty());
            for (k, point) in perimeter.iter().enumerate() {
                let reached = near.as_ref().is_none_or(|at| {
                    at.iter().any(|&a| {
                        let d = a.abs_diff(k);
                        d.min(perimeter.len() - d) <= FOCUS_REACH
                    })
                });
                if reached {
                    boundary_m.insert(point.im);
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
                if selected_count_at_m > 0 && selected_count_at_m < neighbors.npoly {
                    boundary_m.insert(im);
                }
            }
        }
        if boundary_m.is_empty() {
            return Ok(None);
        }

        // Scored from what each fill changes (`method_c_perimeter_incremental`);
        // the whole-mesh trial below stays for a base it cannot take.
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
            let mut best: Option<(usize, usize, usize, Vec<usize>)> = None;
            for im in boundary_m {
                let Trial::Scored(score) = incremental.trial(Some(im), &[])? else {
                    continue;
                };
                let key = (score.added.len(), score.remainder, score.length);
                if best
                    .as_ref()
                    .is_none_or(|current| key < (current.0, current.1, current.2))
                {
                    best = Some((key.0, key.1, key.2, score.added));
                }
            }
            return match best {
                None => Ok(None),
                Some((_, _, _, added)) => {
                    let trial = incremental.with(&added);
                    let perimeters =
                        self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)?;
                    Ok(Some((trial, perimeters.concat())))
                }
            };
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
