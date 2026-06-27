#[derive(Debug, Clone)]
pub(crate) struct ColmCouplingCsvRow {
    pub(crate) cell_id: String,
    pub(crate) cell_index: i32,
    pub(crate) center_lon: f64,
    pub(crate) center_lat: f64,
    pub(crate) surface_class: String,
    pub(crate) has_river: bool,
    pub(crate) river_class: String,
    pub(crate) river_fraction: f64,
    pub(crate) estimated_river_area_m2: f64,
    pub(crate) has_coast: bool,
    pub(crate) coast_class: String,
    pub(crate) coastal_fraction: f64,
    pub(crate) normalized_cell_area_m2: f64,
    pub(crate) source_area_cell: f64,
}
