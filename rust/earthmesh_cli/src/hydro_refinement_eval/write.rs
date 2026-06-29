use std::fs;
use std::io;
use std::path::Path;

use crate::{read_text_maybe_gzip, JsonParser};

use super::json::build_refinement_eval_json;
use super::log::parse_refinement_log;
use super::summary::{
    summarize_background_cells, summarize_coast_intersections, summarize_river_intersections,
};

/// Faithful port of `refinement_eval.py::write_refinement_eval_json`.
pub fn write_refinement_eval_json(
    background_geojson: impl AsRef<Path>,
    intersections_geojson: impl AsRef<Path>,
    output_json: impl AsRef<Path>,
    coast_intersections_geojson: Option<&Path>,
    log_path: Option<&Path>,
    unit_sphere_area: bool,
) -> io::Result<()> {
    let bg_root = JsonParser::new(&read_text_maybe_gzip(background_geojson.as_ref())?).parse()?;
    let river_root =
        JsonParser::new(&read_text_maybe_gzip(intersections_geojson.as_ref())?).parse()?;
    let background = summarize_background_cells(&bg_root, unit_sphere_area);
    let river = summarize_river_intersections(&river_root);
    let coast = match coast_intersections_geojson {
        Some(p) => Some(summarize_coast_intersections(
            &JsonParser::new(&read_text_maybe_gzip(p)?).parse()?,
        )),
        None => None,
    };
    let log = match log_path {
        Some(p) => Some(parse_refinement_log(&fs::read_to_string(p)?)),
        None => None,
    };
    let json = build_refinement_eval_json(&background, &river, coast.as_ref(), log.as_ref());
    crate::ensure_parent_dir(output_json.as_ref())?;
    fs::write(output_json, json)
}
