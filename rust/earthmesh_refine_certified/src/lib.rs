mod api;
pub mod certificate;
pub mod coarsen;
mod config;
mod fingerprint;
pub mod mother_grid;
mod outcome;
pub mod remap;
pub mod requirement;

pub use api::{
    certify_geometry, certify_mother_grid, finalize_geometry_certified_mother,
    generate_certified_mother_grid, geometry_certified_mother_grid, safe_mother_final_evidence,
    safe_mother_only,
};
pub use certificate::{
    BalanceCertificate, Certificate, CertificateError, CertificateReport, FinalCertificateReport,
    GeometryCertificateReport, GeometryRegionCertificateReport, PhysicalCertificate,
};
pub use config::{CertifiedConfig, DeliveryMode};
pub use mother_grid::{MotherGrid, TriangleAddress, TriangleOrientation, VertexAddress};
pub use outcome::{
    classify_adaptivity_delivery, AdaptivityFulfillmentReport, AdaptivityIncompleteReason,
    CertifiedMeshOutcome, CertifiedPrimalDualMesh, CompressionIncompleteReason,
    FinalCertificationEvidence, GeometryCertifiedMotherGrid, SafeFallbackReason,
};

pub use requirement::{
    certify_final_cell_requirements, certify_final_cell_requirements_from_raster,
    certify_final_cell_requirements_from_raster_global_bound,
    certify_final_cell_requirements_with_remap, FinalCellRequirementCertificate,
    FinalCellRequirementError, FinalCellRequirementReport, FinalCellRequirementResiduals,
    RasterLevelField, RequirementWitness, SourceLevelField, TargetLevelField,
};
