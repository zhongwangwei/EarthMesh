//! The coarsest certified mother whose moved vertices serve the demand, by
//! whichever move needs fewer cells (guide 11.88): the Schmidt stretch
//! (conformal, one focus) or the equidistribution (any demand, but little
//! contrast). Neither changes the connectivity, so no heptagon appears.

use crate::{
    certificate::AngleContractId,
    equidistributed_mother::equidistributed_certified_mother,
    outcome::GeometryCertifiedMotherGrid,
    requirement::{FinalCellRequirementCertificate, RasterLevelField},
    stretched_mother::stretched_certified_mother,
};

/// A moved mother that passed every final certificate.
pub struct AdaptedMother {
    pub geometry: GeometryCertifiedMotherGrid,
    pub subdivision: usize,
    /// `schmidt_stretch` or `equidistribution`.
    pub strategy: &'static str,
    /// One line on how it was obtained, for the log.
    pub summary: String,
    pub delivered_levels: Vec<usize>,
    pub final_requirements: FinalCellRequirementCertificate,
    /// Why the candidates not delivered were set aside.
    pub rejected: Vec<String>,
}

pub fn adapted_certified_mother(
    base_subdivision: usize,
    chosen_level: usize,
    raster: &RasterLevelField,
    angle_contract: AngleContractId,
    max_cells: usize,
) -> Result<AdaptedMother, Vec<String>> {
    let safe = base_subdivision
        .checked_shl(chosen_level as u32)
        .unwrap_or(usize::MAX);
    let schmidt = stretched_certified_mother(
        base_subdivision,
        chosen_level,
        raster,
        angle_contract,
        max_cells,
    );
    // Only a mother coarser than the stretch's is worth equidistributing.
    let below = schmidt
        .as_ref()
        .map_or(safe, |stretched| stretched.subdivision);
    let equidistributed = equidistributed_certified_mother(
        base_subdivision,
        raster,
        angle_contract,
        max_cells,
        below,
    );
    match (equidistributed, schmidt) {
        (Ok(e), schmidt) => {
            let mut rejected = match schmidt {
                Ok(s) => vec![format!(
                    "schmidt stretch: n={} (factor {:.2}) needs more cells",
                    s.subdivision, s.factor
                )],
                Err(reasons) => reasons
                    .into_iter()
                    .map(|r| format!("schmidt stretch: {r}"))
                    .collect(),
            };
            rejected.extend(
                e.rejected
                    .into_iter()
                    .map(|r| format!("equidistribution: {r}")),
            );
            Ok(AdaptedMother {
                summary: format!(
                    "equidistributed mother n={} (contrast {:.2}, targets at {:.2} of the required size)",
                    e.subdivision, e.contrast, e.margin
                ),
                geometry: e.geometry,
                subdivision: e.subdivision,
                strategy: "equidistribution",
                delivered_levels: e.delivered_levels,
                final_requirements: e.final_requirements,
                rejected,
            })
        }
        (Err(reasons), Ok(s)) => {
            let mut rejected = s
                .rejected
                .into_iter()
                .map(|r| format!("schmidt stretch: {r}"))
                .collect::<Vec<_>>();
            rejected.extend(
                reasons
                    .into_iter()
                    .map(|r| format!("equidistribution: {r}")),
            );
            Ok(AdaptedMother {
                summary: format!(
                    "stretched mother n={} (factor {:.2} toward {:.3}E {:.3}N)",
                    s.subdivision, s.factor, s.focus_lonlat.0, s.focus_lonlat.1
                ),
                geometry: s.geometry,
                subdivision: s.subdivision,
                strategy: "schmidt_stretch",
                delivered_levels: s.delivered_levels,
                final_requirements: s.final_requirements,
                rejected,
            })
        }
        (Err(equidistribution), Err(stretch)) => Err(stretch
            .into_iter()
            .map(|r| format!("schmidt stretch: {r}"))
            .chain(
                equidistribution
                    .into_iter()
                    .map(|r| format!("equidistribution: {r}")),
            )
            .collect()),
    }
}
