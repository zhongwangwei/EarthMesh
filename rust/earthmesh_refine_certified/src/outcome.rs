use crate::certificate::{AngleContractId, FinalCertificateReport, GeometryCertificateReport};
use earthmesh_mesh::MeshState;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct AdaptivityFulfillmentReport {
    pub requested_level_min: usize,
    pub requested_level_max: usize,
    pub requested_histogram: BTreeMap<usize, usize>,
    pub delivered_level_min: usize,
    pub delivered_level_max: usize,
    pub delivered_histogram: BTreeMap<usize, usize>,
    pub mixed_levels_requested: bool,
    pub mixed_levels_delivered: bool,
    pub initial_faces: usize,
    pub final_faces: usize,
    pub compression_ratio: f64,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub search_complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdaptivityIncompleteReason {
    MixedLevelsRequestedButUniformDelivered,
    NoCertifiedCoarseningCommit,
}

pub type SafeFallbackReason = AdaptivityIncompleteReason;
pub type CompressionIncompleteReason = AdaptivityIncompleteReason;

impl std::fmt::Display for AdaptivityIncompleteReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MixedLevelsRequestedButUniformDelivered => {
                f.write_str("mixed levels were requested but the delivered mesh is uniform")
            }
            Self::NoCertifiedCoarseningCommit => {
                f.write_str("no certified coarsening component was committed")
            }
        }
    }
}

impl AdaptivityFulfillmentReport {
    #[allow(clippy::too_many_arguments)]
    pub fn from_levels(
        requested_levels: impl IntoIterator<Item = usize>,
        delivered_levels: impl IntoIterator<Item = usize>,
        initial_faces: usize,
        final_faces: usize,
        components_total: usize,
        components_committed: usize,
        components_promoted: usize,
        components_exhausted: usize,
        search_complete: bool,
    ) -> Self {
        let requested_histogram = histogram(requested_levels);
        let delivered_histogram = histogram(delivered_levels);
        let (requested_level_min, requested_level_max) = min_max(&requested_histogram);
        let (delivered_level_min, delivered_level_max) = min_max(&delivered_histogram);
        Self {
            requested_level_min,
            requested_level_max,
            mixed_levels_requested: requested_histogram.len() > 1,
            requested_histogram,
            delivered_level_min,
            delivered_level_max,
            mixed_levels_delivered: delivered_histogram.len() > 1,
            delivered_histogram,
            initial_faces,
            final_faces,
            compression_ratio: if final_faces == 0 {
                0.0
            } else {
                initial_faces as f64 / final_faces as f64
            },
            components_total,
            components_committed,
            components_promoted,
            components_exhausted,
            search_complete,
        }
    }

    pub fn compression_incomplete_reason(&self) -> Option<CompressionIncompleteReason> {
        if self.mixed_levels_requested && !self.mixed_levels_delivered {
            Some(AdaptivityIncompleteReason::MixedLevelsRequestedButUniformDelivered)
        } else if self.components_total > 0
            && self.components_committed == 0
            && self.final_faces >= self.initial_faces
            && self.delivered_level_min > self.requested_level_max
        {
            Some(AdaptivityIncompleteReason::NoCertifiedCoarseningCommit)
        } else {
            None
        }
    }
}

fn histogram(levels: impl IntoIterator<Item = usize>) -> BTreeMap<usize, usize> {
    let mut out = BTreeMap::new();
    for level in levels {
        *out.entry(level).or_insert(0) += 1;
    }
    out
}

fn min_max(histogram: &BTreeMap<usize, usize>) -> (usize, usize) {
    match (histogram.keys().next(), histogram.keys().next_back()) {
        (Some(min), Some(max)) => (*min, *max),
        _ => (0, 0),
    }
}

#[derive(Debug, Clone)]
pub struct GeometryCertifiedMotherGrid {
    primal: MeshState,
    certificate: GeometryCertificateReport,
}

impl GeometryCertifiedMotherGrid {
    pub(crate) fn new(primal: MeshState, certificate: GeometryCertificateReport) -> Self {
        Self {
            primal,
            certificate,
        }
    }

    pub fn primal(&self) -> &MeshState {
        &self.primal
    }

    pub fn certificate(&self) -> &GeometryCertificateReport {
        &self.certificate
    }

    pub fn angle_contract_id(&self) -> AngleContractId {
        self.certificate.angle_contract_id
    }

    pub(crate) fn into_parts(self) -> (MeshState, GeometryCertificateReport) {
        (self.primal, self.certificate)
    }
}

#[derive(Debug, Clone)]
pub struct FinalCertificationEvidence {
    pub(crate) physical_residuals: usize,
    pub(crate) balance_residuals: usize,
    pub(crate) remap_closure_errors: usize,
    pub(crate) remap_rows: usize,
    pub(crate) target_fingerprint: u64,
}

impl FinalCertificationEvidence {
    pub fn from_final_cells(
        requirements: &crate::requirement::FinalCellRequirementReport,
        remap: crate::remap::RemapCertificate,
    ) -> Result<Self, String> {
        let target_fingerprint = requirements.target_fingerprint();
        if remap.target_fingerprint() != Some(target_fingerprint) {
            return Err("remap and final-cell evidence target different meshes".into());
        }
        let physical = crate::certificate::PhysicalCertificate::from_final_cells(requirements)
            .map_err(|error| error.to_string())?;
        let balance = crate::certificate::BalanceCertificate::from_final_cells(requirements)
            .map_err(|error| error.to_string())?;
        Self::from_certificates(physical, balance, remap, target_fingerprint)
    }

    pub(crate) fn from_certificates(
        physical: crate::certificate::PhysicalCertificate,
        balance: crate::certificate::BalanceCertificate,
        remap: crate::remap::RemapCertificate,
        target_fingerprint: u64,
    ) -> Result<Self, String> {
        if remap.negative_weights() + remap.bad_row_sums() + remap.bad_lineage_rows() != 0
            || remap.global_area_closure_error() > remap.closure_tolerance()
        {
            return Err("remap certificate has residuals".into());
        }
        Ok(Self {
            physical_residuals: physical.residuals(),
            balance_residuals: balance.residuals(),
            remap_closure_errors: usize::from(
                remap.constant_closure_error() > remap.closure_tolerance()
                    || remap.global_area_closure_error() > remap.closure_tolerance(),
            ),
            remap_rows: remap.rows(),
            target_fingerprint,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CertifiedPrimalDualMesh {
    primal: MeshState,
    certificate: FinalCertificateReport,
}

impl CertifiedPrimalDualMesh {
    pub(crate) fn new(primal: MeshState, certificate: FinalCertificateReport) -> Self {
        Self {
            primal,
            certificate,
        }
    }

    pub fn primal(&self) -> &MeshState {
        &self.primal
    }

    pub fn certificate(&self) -> &FinalCertificateReport {
        &self.certificate
    }

    pub fn angle_contract_id(&self) -> AngleContractId {
        self.certificate.geometry.angle_contract_id
    }
}

#[derive(Debug, Clone)]
pub enum CertifiedMeshOutcome {
    GeometryCertified(Box<GeometryCertifiedMotherGrid>),
    Certified(Box<CertifiedPrimalDualMesh>),
    CertifiedAdaptive {
        mesh: Box<CertifiedPrimalDualMesh>,
        fulfillment: Box<AdaptivityFulfillmentReport>,
    },
    CertifiedSafeFallback {
        mesh: Box<CertifiedPrimalDualMesh>,
        fulfillment: Box<AdaptivityFulfillmentReport>,
        reason: SafeFallbackReason,
    },
    CompressionIncomplete {
        safe_mesh: Box<CertifiedPrimalDualMesh>,
        fulfillment: Box<AdaptivityFulfillmentReport>,
        reason: CompressionIncompleteReason,
    },
    CellBudgetInsufficient {
        required_cells: usize,
        budget: usize,
    },
    MaximumLevelReached {
        requested_level: usize,
        max_level: usize,
    },
    CriterionNotCertifiable {
        reason: String,
    },
    PhysicalCriterionUnsatisfiable {
        reason: String,
    },
    UnsupportedBoundaryConstraint {
        reason: String,
    },
    SearchBudgetExhausted {
        attempted_patches: usize,
    },
    InternalCertificationFailure {
        reason: String,
    },
}

pub fn classify_adaptivity_delivery(
    mesh: CertifiedPrimalDualMesh,
    fulfillment: AdaptivityFulfillmentReport,
    allow_safe_fallback: bool,
) -> CertifiedMeshOutcome {
    match (
        fulfillment.compression_incomplete_reason(),
        allow_safe_fallback,
    ) {
        (None, _) => CertifiedMeshOutcome::CertifiedAdaptive {
            mesh: Box::new(mesh),
            fulfillment: Box::new(fulfillment),
        },
        (Some(reason), true) => CertifiedMeshOutcome::CertifiedSafeFallback {
            mesh: Box::new(mesh),
            fulfillment: Box::new(fulfillment),
            reason,
        },
        (Some(reason), false) => CertifiedMeshOutcome::CompressionIncomplete {
            safe_mesh: Box::new(mesh),
            fulfillment: Box::new(fulfillment),
            reason,
        },
    }
}
