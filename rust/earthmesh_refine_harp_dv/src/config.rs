use crate::error::{HarpDvError, Result};

/// What the run is allowed to do, and where it must stop.
///
/// Validated rather than trusted. A NaN scale or a zero budget reaching the
/// driver would surface much later as a mesh nobody asked for, so every field
/// is checked once, here, before any work begins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HarpDvConfig {
    /// How many times the mesh may be re-evaluated and adapted.
    pub max_cycles: u32,
    /// The finest cell the run may produce. A demand below it stops satisfied
    /// as far as this run is concerned, and says so.
    pub minimum_cell_width_m: f64,
    /// The cell count the run may not exceed.
    pub maximum_cells: usize,
    /// Rings of neighbours a patch takes around its seed.
    pub patch_ring_depth: usize,
    /// The largest patch a single transaction may touch.
    pub maximum_patch_cells: usize,
    /// The widest ratio allowed between the effective scales of two adjacent
    /// cells before the coarser one is forced to refine.
    pub maximum_neighbor_scale_ratio: f64,
    /// Single-threaded, fixed traversal order, fixed tie-breaks.
    ///
    /// False is not implemented and is refused, rather than accepted and
    /// ignored. The first version is deterministic by construction.
    pub deterministic: bool,
}

impl Default for HarpDvConfig {
    fn default() -> Self {
        Self {
            max_cycles: 20,
            minimum_cell_width_m: 1_000.0,
            maximum_cells: 5_000_000,
            patch_ring_depth: 2,
            maximum_patch_cells: 10_000,
            maximum_neighbor_scale_ratio: 1.75,
            deterministic: true,
        }
    }
}

impl HarpDvConfig {
    /// Check every field, and say which one is wrong.
    pub fn validate(&self) -> Result<()> {
        if self.max_cycles == 0 {
            return Err(HarpDvError::InvalidConfig(
                "max_cycles must be at least one; zero cycles is a run that cannot do anything"
                    .to_string(),
            ));
        }
        if !self.minimum_cell_width_m.is_finite() || self.minimum_cell_width_m <= 0.0 {
            return Err(HarpDvError::InvalidConfig(format!(
                "minimum_cell_width_m must be a positive finite length, got {}",
                self.minimum_cell_width_m
            )));
        }
        if self.maximum_cells == 0 {
            return Err(HarpDvError::InvalidConfig(
                "maximum_cells must be at least one".to_string(),
            ));
        }
        if self.maximum_patch_cells == 0 {
            return Err(HarpDvError::InvalidConfig(
                "maximum_patch_cells must be at least one".to_string(),
            ));
        }
        if self.maximum_patch_cells > self.maximum_cells {
            return Err(HarpDvError::InvalidConfig(format!(
                "maximum_patch_cells {} exceeds maximum_cells {}; a patch cannot be larger than \
                 the mesh it sits in",
                self.maximum_patch_cells, self.maximum_cells
            )));
        }
        if !self.maximum_neighbor_scale_ratio.is_finite()
            || self.maximum_neighbor_scale_ratio <= 1.0
        {
            return Err(HarpDvError::InvalidConfig(format!(
                "maximum_neighbor_scale_ratio must be greater than one, got {}; a ratio of one \
                 would demand every cell be the same size",
                self.maximum_neighbor_scale_ratio
            )));
        }
        if !self.deterministic {
            return Err(HarpDvError::InvalidConfig(
                "deterministic = false is not implemented; the first version is single threaded \
                 with fixed tie-breaks, and accepting the flag without honouring it would be a \
                 promise nothing keeps"
                    .to_string(),
            ));
        }
        Ok(())
    }
}
