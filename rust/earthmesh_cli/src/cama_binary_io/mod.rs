mod geometry;
mod reports;
mod validation;

pub use geometry::{CamaBinaryGridSpec, CamaBinaryWindow};
pub use reports::{
    CamaElevtnSurfaceWindowReport, CamaLonLatBbox, CamaMetricKind, CamaMetricWindowReport,
    CamaNextxyWindowReport, CamaReachClassification, CamaReachClassificationThresholds,
    CamaReachInventoryGeoJsonWriteReport, CamaReachInventoryJsonlWriteReport,
    CamaReachInventoryReport, CamaReachRecord, CamaSurfaceClass,
};
pub(crate) use validation::validate_cama_binary_window;
