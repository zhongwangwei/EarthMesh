use std::path::{Path, PathBuf};

pub(super) fn area_judge_threshold_path(threshold_dir: &Path, name: &str) -> PathBuf {
    threshold_dir.join(format!("{name}.nc"))
}
