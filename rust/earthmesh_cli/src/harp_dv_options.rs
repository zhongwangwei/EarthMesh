//! Project/namelist controls for the production HARP-DV backend.

use std::io;

use crate::namelist_reader::{namelist_assignments, namelist_has_section};
use earthmesh_refine_harp_dv::{
    CandidatePolicy, HardGates, HarpDvConfig, GRIDFILE_MAX_VERTEX_DEGREE,
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HarpDvRunOptions {
    pub config: HarpDvConfig,
    pub candidate_policy: CandidatePolicy,
    pub gates: HardGates,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn parse<T: std::str::FromStr>(field: &str, value: &str) -> io::Result<T> {
    value
        .trim()
        .parse()
        .map_err(|_| invalid(format!("{field} has an invalid value: {value}")))
}

pub fn read_harp_dv_options(contents: &str) -> io::Result<HarpDvRunOptions> {
    if !namelist_has_section(contents, "harp_dv") {
        return Ok(HarpDvRunOptions::default());
    }
    let mut options = HarpDvRunOptions::default();
    for assignment in namelist_assignments(contents, "harp_dv")? {
        match assignment.field.as_str() {
            "max_cycles" => {
                options.config.max_cycles = parse(&assignment.field, &assignment.value)?
            }
            "minimum_cell_width_m" => {
                options.config.minimum_cell_width_m = parse(&assignment.field, &assignment.value)?
            }
            "maximum_cells" => {
                options.config.maximum_cells = parse(&assignment.field, &assignment.value)?
            }
            "maximum_patch_cells" => {
                options.config.maximum_patch_cells = parse(&assignment.field, &assignment.value)?
            }
            "maximum_neighbor_scale_ratio" => {
                options.config.maximum_neighbor_scale_ratio =
                    parse(&assignment.field, &assignment.value)?
            }
            "minimum_candidate_separation_m" => {
                options.candidate_policy.min_separation_m =
                    parse(&assignment.field, &assignment.value)?
            }
            "maximum_vertex_degree" => {
                options.gates.max_vertex_degree = parse(&assignment.field, &assignment.value)?
            }
            "minimum_triangle_angle_deg" => {
                options.gates.min_triangle_angle_deg = parse(&assignment.field, &assignment.value)?
            }
            other => return Err(invalid(format!("unknown &harp_dv field '{other}'"))),
        }
    }
    options
        .config
        .validate()
        .map_err(|error| invalid(error.to_string()))?;
    if !options.candidate_policy.min_separation_m.is_finite()
        || options.candidate_policy.min_separation_m <= 0.0
    {
        return Err(invalid(
            "minimum_candidate_separation_m must be positive and finite",
        ));
    }
    if !(3..=GRIDFILE_MAX_VERTEX_DEGREE).contains(&options.gates.max_vertex_degree) {
        return Err(invalid(format!(
            "maximum_vertex_degree must be in 3..={GRIDFILE_MAX_VERTEX_DEGREE}"
        )));
    }
    if !options.gates.min_triangle_angle_deg.is_finite()
        || !(0.0..60.0).contains(&options.gates.min_triangle_angle_deg)
    {
        return Err(invalid(
            "minimum_triangle_angle_deg must be finite and in [0, 60)",
        ));
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_harp_dv_option_is_parsed_and_validated() {
        let options = read_harp_dv_options(
            "&harp_dv\n NL%max_cycles=3\n NL%minimum_cell_width_m=2000\n \
             NL%maximum_cells=9000\n NL%maximum_patch_cells=800\n \
             NL%maximum_neighbor_scale_ratio=1.5\n \
             NL%minimum_candidate_separation_m=2\n NL%maximum_vertex_degree=6\n \
             NL%minimum_triangle_angle_deg=25\n/",
        )
        .expect("options");
        assert_eq!(options.config.max_cycles, 3);
        assert_eq!(options.config.minimum_cell_width_m, 2000.0);
        assert_eq!(options.config.maximum_cells, 9000);
        assert_eq!(options.config.maximum_patch_cells, 800);
        assert_eq!(options.config.maximum_neighbor_scale_ratio, 1.5);
        assert_eq!(options.candidate_policy.min_separation_m, 2.0);
        assert_eq!(options.gates.max_vertex_degree, 6);
        assert_eq!(options.gates.min_triangle_angle_deg, 25.0);

        assert!(read_harp_dv_options("&harp_dv\n NL%maximum_vertex_degree=8\n/").is_err());
        assert!(read_harp_dv_options("&harp_dv\n NL%extra=1\n/").is_err());
    }
}
