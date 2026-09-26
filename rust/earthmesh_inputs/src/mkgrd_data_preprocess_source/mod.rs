mod area_judge;
mod landtype;
mod state;

pub use area_judge::{
    read_data_preprocess_area_judge_base_state_one_based,
    read_data_preprocess_area_judge_source_one_based,
};
pub use landtype::{
    read_landtype_bbox_window_one_based, read_landtype_data_preprocess_one_based,
    sample_landtype_surface_class_codes_for_points_one_based,
    sample_landtype_values_for_points_one_based, LandtypeWindow,
};
pub use state::{
    build_mkgrd_data_preprocess_source_state_from_config_one_based,
    build_mkgrd_data_preprocess_source_state_one_based,
};

/// Whether a namelist names a land-cover file at all (comments ignored).
pub fn namelist_sets_landtype_file(contents: &str) -> bool {
    contents
        .lines()
        .map(|line| line.split('!').next().unwrap_or(""))
        .any(|line| line.to_ascii_lowercase().contains("landtype_file"))
}

/// Whether a configured land-cover path points at data rather than a
/// placeholder (`none`, empty, or the legacy `/tmp`).
pub fn landtype_file_is_real(landtype_file: &str) -> bool {
    let trimmed = landtype_file.trim();
    !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") && trimmed != "/tmp"
}
