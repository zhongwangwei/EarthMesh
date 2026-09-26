use crate::AreaJudgeDomainInitializationReport;
use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

/// Initialize the global-domain branch of `MOD_Area_judge.F90:Area_judge`.
pub fn initialize_area_judge_global_domain_one_based(
    nlons_source: usize,
    nlats_source: usize,
) -> io::Result<AreaJudgeDomainInitializationReport> {
    if nlons_source == 0 || nlats_source == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "global domain source dimensions must be positive",
        ));
    }

    let mut is_in_domain = vec![vec![false; nlats_source + 1]; nlons_source + 1];
    for row in is_in_domain.iter_mut().take(nlons_source + 1).skip(1) {
        for value in row.iter_mut().take(nlats_source + 1).skip(1) {
            *value = true;
        }
    }

    Ok(AreaJudgeDomainInitializationReport {
        is_in_domain,
        bounds: AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: nlons_source,
            maxlat_source: 1,
            minlat_source: nlats_source,
        },
        numpatch: nlons_source * nlats_source,
        nlons_select: nlons_source,
        nlats_select: nlats_source,
    })
}
