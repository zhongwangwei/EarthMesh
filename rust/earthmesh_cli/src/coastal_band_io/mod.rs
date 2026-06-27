mod cama;
mod cells;
mod writer;

pub use cama::write_coastal_band_geojson_from_cama;
pub use cells::{coastal_band_cells, coastal_land_mask_from_elevation};
pub use writer::write_coastal_band_dissolve_geojson;
