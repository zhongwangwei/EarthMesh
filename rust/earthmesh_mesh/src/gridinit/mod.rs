/// Result of Method-C `gridinit:get_factors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodCGridinitFactors {
    pub base_nxp: usize,
    pub expansion_factor: usize,
}

/// Port of `method_c_grid.f90:get_factors`.
///
/// Method-C does not always build the initial icosahedron at the requested `NXP`.
/// It may choose a coarser base grid and later call `expand_delaunay_mesh`.
/// The selection rule tries 3-first and 2-first reductions down to
/// `nxpmin = 24`, then selects the largest candidate below `46` when more than
/// one such candidate exists; otherwise it selects the minimum candidate.
pub fn method_c_gridinit_factorization_canonical(nxp: usize) -> Option<MethodCGridinitFactors> {
    if nxp == 0 {
        return None;
    }

    const NXP_MIN: usize = 24;
    let mut candidates = [MethodCGridinitFactors {
        base_nxp: nxp,
        expansion_factor: 1,
    }; 4];

    reduce_gridinit_candidate(&mut candidates[0], 3, NXP_MIN);
    reduce_gridinit_candidate(&mut candidates[0], 2, NXP_MIN);

    reduce_gridinit_candidate(&mut candidates[1], 2, NXP_MIN);
    reduce_gridinit_candidate(&mut candidates[1], 3, NXP_MIN);

    let threshold = (NXP_MIN - 1) * 2;
    let under_threshold = candidates
        .iter()
        .filter(|candidate| candidate.base_nxp < threshold)
        .count();

    let mut selected = candidates[0];
    if under_threshold > 1 {
        for candidate in candidates.iter().copied().skip(1) {
            if candidate.base_nxp < threshold && candidate.base_nxp > selected.base_nxp {
                selected = candidate;
            }
        }
    } else {
        for candidate in candidates.iter().copied().skip(1) {
            if candidate.base_nxp < selected.base_nxp {
                selected = candidate;
            }
        }
    }

    Some(selected)
}

fn reduce_gridinit_candidate(
    candidate: &mut MethodCGridinitFactors,
    factor: usize,
    nxp_min: usize,
) {
    while candidate.base_nxp.is_multiple_of(factor) && candidate.base_nxp / factor >= nxp_min {
        candidate.base_nxp /= factor;
        candidate.expansion_factor *= factor;
    }
}
