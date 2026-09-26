use std::path::PathBuf;

/// Evidence report from writing `MOD_mask_postproc.F90:bdy_calculation` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObcBoundaryWriteReport {
    pub output: PathBuf,
    pub boundary_points: usize,
}

/// Evidence report from writing `MOD_mask_postproc.F90:bdy_connection` output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obcv2BoundaryWriteReport {
    pub output: PathBuf,
    pub longest_curve_slots: usize,
    pub closed_curves: usize,
}
