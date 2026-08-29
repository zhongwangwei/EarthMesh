use crate::certificate::{FinalCertificateReport, GeometryCertificateReport};
use earthmesh_mesh::MeshState;

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
}

#[derive(Debug, Clone)]
pub enum CertifiedMeshOutcome {
    GeometryCertified(Box<GeometryCertifiedMotherGrid>),
    Certified(Box<CertifiedPrimalDualMesh>),
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
