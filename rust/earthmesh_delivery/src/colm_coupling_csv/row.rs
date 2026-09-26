#[derive(Debug, Clone)]
pub struct ColmCouplingCsvRow {
    pub cell_id: String,
    pub cell_index: i32,
    pub center_lon: f64,
    pub center_lat: f64,
    pub surface_class: String,
    pub has_river: bool,
    pub river_class: String,
    pub river_fraction: f64,
    pub estimated_river_area_m2: f64,
    pub has_coast: bool,
    pub coast_class: String,
    pub coastal_fraction: f64,
    pub normalized_cell_area_m2: f64,
    pub source_area_cell: f64,
}
