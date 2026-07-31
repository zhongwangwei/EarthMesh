use std::sync::atomic::{AtomicUsize, Ordering};
use std::{collections::BTreeSet, io};

use rayon::prelude::*;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn try_shrink_method_c_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[MethodCPerimeterPoint]>,
        coverage: Option<&crate::method_c_spawn_hfield::MethodCHfieldDemandCoverage>,
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

        let candidates = candidates.into_iter().collect::<Vec<_>>();
        let candidate_count = candidates.len();
        let detailed_trace = std::env::var("EARTHMESH_M0_REPAIR_TRACE")
            .is_ok_and(|value| matches!(value.as_str(), "1" | "on" | "true"));
        let coverage_rejected = detailed_trace.then(AtomicUsize::default);
        let best = candidates
            .into_par_iter()
            .map(
                |candidate| -> io::Result<
                    Option<(
                        usize,
                        usize,
                        usize,
                        usize,
                        Vec<bool>,
                        Vec<MethodCPerimeterPoint>,
                    )>,
                > {
                    let mut trial = selected.to_vec();
                    trial[candidate] = false;
                    self.close_method_c_concavities_for_level_with_neighbors(
                        &mut trial,
                        m_neighbors,
                    )?;
                    let trial_count = trial.iter().filter(|&&item| item).count();
                    if trial_count == 0
                        || trial_count >= selected_count
                        || self
                            .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                            .is_err()
                    {
                        return Ok(None);
                    }
                    if !Self::method_c_repair_candidate_preserves_coverage(coverage, &trial) {
                        if let Some(rejected) = coverage_rejected.as_ref() {
                            rejected.fetch_add(1, Ordering::Relaxed);
                        }
                        return Ok(None);
                    }
                    let Ok(trial_perimeters) =
                        self.method_c_perimeters_from_selected_faces(&trial, m_neighbors)
                    else {
                        return Ok(None);
                    };
                    let trial_perimeter = trial_perimeters
                        .iter()
                        .flatten()
                        .copied()
                        .collect::<Vec<_>>();
                    let removed = selected_count - trial_count;
                    let remainder = Self::method_c_perimeter_remainder_score(&trial_perimeters);
                    Ok(Some((
                        removed,
                        remainder,
                        trial_perimeter.len(),
                        candidate,
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
                            if (right.0, right.1, right.2, right.3)
                                < (left.0, left.1, left.2, left.3)
                            {
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
            let rejected = coverage_rejected
                .as_ref()
                .map_or(0, |count| count.load(Ordering::Relaxed));
            if let Some((removed, remainder, perimeter_len, candidate, _, _)) = best.as_ref() {
                eprintln!(
                    "earthmesh_mesh: method_c shrink candidates={} coverage_rejected={} chosen_w={} removed={} remainder={} perimeter_len={}",
                    candidate_count, rejected, candidate, removed, remainder, perimeter_len,
                );
            } else {
                eprintln!(
                    "earthmesh_mesh: method_c shrink candidates={} coverage_rejected={} chosen_w=none",
                    candidate_count, rejected,
                );
            }
        }
        Ok(best.map(|(_, _, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }
}
