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

/// Where reverse coarsening builds the finest mother (design B1, guide
/// 11.106): over the whole sphere, or only where the requirement reaches and
/// a regional run delivers cells, the rest settled by construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedMaterialization {
    #[default]
    Whole,
    /// Built on demand, then put back together into the whole sphere, which
    /// is certified and published as the whole route publishes it.
    OnDemand,
    /// Built on demand and published as the region: the delivered domain
    /// cut from the built region's final mesh, the parent being that mesh --
    /// no sphere is assembled, so the base may be as fine as a 30 m run needs
    /// (guide 11.109).
    Regional,
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
    pub materialization: CertifiedMaterialization,
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
            materialization: CertifiedMaterialization::Whole,
        }
    }
}
