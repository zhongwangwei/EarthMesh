mod adapted_mother;
mod api;
pub mod certificate;
pub mod coarsen;
mod config;
mod equidistributed_mother;
mod fingerprint;
mod mother_geometry;
pub mod mother_grid;
pub mod on_demand;
mod outcome;
pub mod remap;
pub mod requirement;
mod stretched_mother;

pub use api::{
    certify_geometry, certify_geometry_with_contract, certify_mother_grid,
    certify_mother_grid_with_contract, finalize_geometry_certified_mother,
    finalize_region_geometry, generate_certified_mother_grid, geometry_certified_mother_grid,
    geometry_certified_mother_grid_with_contract, safe_mother_final_evidence, safe_mother_only,
};
pub use certificate::{
    AngleContract, AngleContractId, AngleWindow, BalanceCertificate, Certificate, CertificateError,
    CertificateReport, FinalCertificateReport, GeometryCertificateReport,
    GeometryRegionCertificateReport, PhysicalCertificate, CERTIFICATE_SCHEMA_VERSION,
};
pub use config::{CertifiedConfig, DeliveryMode};
pub use fingerprint::mesh_fingerprint;
pub use mother_grid::{MotherGrid, TriangleAddress, TriangleOrientation, VertexAddress};
pub use outcome::{
    classify_adaptivity_delivery, AdaptivityFulfillmentReport, AdaptivityIncompleteReason,
    CertifiedMeshOutcome, CertifiedPrimalDualMesh, CompressionIncompleteReason,
    FinalCertificationEvidence, GeometryCertifiedMotherGrid, SafeFallbackReason,
};

pub use adapted_mother::{adapted_certified_mother, AdaptedMother};
pub use equidistributed_mother::{equidistributed_certified_mother, EquidistributedMother};
pub use requirement::{
    certify_final_cell_requirements, certify_final_cell_requirements_from_raster,
    certify_final_cell_requirements_from_raster_global_bound,
    certify_final_cell_requirements_with_remap, region_required_levels_from_raster,
    FinalCellRequirementCertificate, FinalCellRequirementError, FinalCellRequirementReport,
    FinalCellRequirementResiduals, RasterLevelField, RequirementWitness, SourceLevelField,
    TargetLevelField,
};
pub use stretched_mother::{stretched_certified_mother, StretchedMother};
