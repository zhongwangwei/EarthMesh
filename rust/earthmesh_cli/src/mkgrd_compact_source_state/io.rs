use std::{fs, io, path::Path};

use super::parser::parse_mkgrd_compact_source_state;
use super::types::{MkgrdCompactRestartRefineSourceState, MkgrdCompactSourceState};

pub fn read_mkgrd_compact_source_state(
    path: impl AsRef<Path>,
) -> io::Result<MkgrdCompactSourceState> {
    let contents = fs::read_to_string(path.as_ref())?;
    parse_mkgrd_compact_source_state(&contents)
}

pub fn read_mkgrd_compact_restart_refine_source_state(
    path: impl AsRef<Path>,
) -> io::Result<MkgrdCompactRestartRefineSourceState> {
    let source_state = read_mkgrd_compact_source_state(path)?;
    let axes = source_state.build_global_source_axes()?;
    Ok(MkgrdCompactRestartRefineSourceState { source_state, axes })
}
