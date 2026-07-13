mod area;
mod bounds;
mod patch;
mod paths;

pub use area::build_area_judge_area_sources_one_based;
pub(crate) use bounds::merge_area_judge_source_bounds;
pub use patch::apply_area_judge_patch_sources_one_based;
