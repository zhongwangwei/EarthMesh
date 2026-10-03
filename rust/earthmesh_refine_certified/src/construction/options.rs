//! CMRC's run options: which construction, which delivery, which angle
//! contract, and its resource bounds. The CLI reads them from `&certified`
//! (`earthmesh_cli::certified_options`); the construction takes them as they
//! are.

use crate::AngleContractId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedMode {
    #[default]
    SafeMotherOnly,
    ReverseCoarsening,
    /// A coarser certified mother stretched toward the demand, falling back
    /// to the safe mother when no stretch passes (guide 11.87).
    StretchedMother,
    /// The coarsest certified mother whose moved vertices serve the demand
    /// -- the Schmidt stretch or the equidistribution, whichever needs fewer
    /// cells -- falling back to the safe mother (guide 11.88).
    EquidistributedMother,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedDelivery {
    Tri,
    Hex,
    #[default]
    Coupled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedRunOptions {
    pub mode: CertifiedMode,
    pub delivery: CertifiedDelivery,
    pub angle_contract: AngleContractId,
    pub maximum_level: usize,
    pub maximum_cells: usize,
    pub gradation_rings_per_level: usize,
    pub search_budget: usize,
}

impl Default for CertifiedRunOptions {
    fn default() -> Self {
        Self {
            mode: CertifiedMode::SafeMotherOnly,
            delivery: CertifiedDelivery::Coupled,
            // The projects' contract (`CertifiedAngleContract::default`): a
            // namelist that names none gets what a project would. Under the
            // strict 40-80 window reverse coarsening finds no legal state on
            // some mixed demands at all (guide 11.102).
            angle_contract: AngleContractId::DomainQuality38To82V1,
            maximum_level: 8,
            maximum_cells: 10_000_000,
            gradation_rings_per_level: 3,
            search_budget: 100_000,
        }
    }
}
