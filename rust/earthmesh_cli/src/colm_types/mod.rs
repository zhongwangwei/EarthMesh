use std::path::PathBuf;

/// Summary from writing a CoLM2024/CoLM20XX coupling metadata NetCDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColmCouplingNetcdfWriteReport {
    pub output: PathBuf,
    pub rows: usize,
}

/// Summary from writing a CoLM2024/CoLM20XX restart-template NetCDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColmRestartTemplateNetcdfWriteReport {
    pub output: PathBuf,
    pub rows: usize,
}

/// Summary from writing a CoLM2024/CoLM20XX forcing-template NetCDF.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColmForcingTemplateNetcdfWriteReport {
    pub output: PathBuf,
    pub rows: usize,
}

/// Per-surface-class cell tally from [`write_colm_coupling_csv_from_mesh`](crate::write_colm_coupling_csv_from_mesh).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ColmSurfaceCounts {
    pub land: usize,
    pub ocean: usize,
    pub coast: usize,
}

/// A CoLM coupling surface class tied to one mesh-cell centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColmSurfaceClassPoint {
    pub lon: f64,
    pub lat: f64,
    /// `surface_class_code`: 0=UNKNOWN, 1=LAND, 2=OCEAN, 3=COAST.
    pub code: i8,
}
