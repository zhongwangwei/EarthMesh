#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryMode {
    Triangular,
    Voronoi,
    Coupled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertifiedConfig {
    pub mother_subdivision: usize,
    pub delivery: DeliveryMode,
    pub max_cells: Option<usize>,
    pub max_level: usize,
    pub grading_ring_width: usize,
}

impl CertifiedConfig {
    pub fn mother_only(mother_subdivision: usize) -> Self {
        Self {
            mother_subdivision,
            delivery: DeliveryMode::Coupled,
            max_cells: None,
            max_level: 0,
            grading_ring_width: 1,
        }
    }
}
