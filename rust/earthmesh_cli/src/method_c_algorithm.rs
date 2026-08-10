//! Explicit selection and bounds for Method-C's LEPP-Delaunay algorithm.

use std::io;

use crate::namelist_reader::{namelist_assignments, namelist_has_section};
use earthmesh_core::{
    DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO, DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
    DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES, DEFAULT_METHOD_C_LEPP_MAX_CYCLES,
    DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES,
    DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION, DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MethodCAlgorithm {
    #[default]
    Canonical,
    LeppDelaunay,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MethodCAlgorithmOptions {
    pub algorithm: MethodCAlgorithm,
    pub max_cycles: usize,
    pub target_size_tolerance: f64,
    pub maximum_neighbor_size_ratio: f64,
    pub maximum_vertices: usize,
    pub maximum_insertions_per_cycle: usize,
    pub maximum_path_length: usize,
    pub stop_at_source_resolution: bool,
    pub minimum_triangle_angle_deg: f64,
}

impl Default for MethodCAlgorithmOptions {
    fn default() -> Self {
        Self {
            algorithm: MethodCAlgorithm::Canonical,
            max_cycles: DEFAULT_METHOD_C_LEPP_MAX_CYCLES,
            target_size_tolerance: DEFAULT_METHOD_C_LEPP_TARGET_SIZE_TOLERANCE,
            maximum_neighbor_size_ratio: DEFAULT_METHOD_C_LEPP_MAXIMUM_NEIGHBOR_SIZE_RATIO,
            maximum_vertices: DEFAULT_METHOD_C_LEPP_MAXIMUM_VERTICES,
            maximum_insertions_per_cycle: DEFAULT_METHOD_C_LEPP_MAXIMUM_INSERTIONS_PER_CYCLE,
            maximum_path_length: DEFAULT_METHOD_C_LEPP_MAXIMUM_PATH_LENGTH,
            stop_at_source_resolution: DEFAULT_METHOD_C_LEPP_STOP_AT_SOURCE_RESOLUTION,
            minimum_triangle_angle_deg: DEFAULT_METHOD_C_LEPP_MINIMUM_TRIANGLE_ANGLE_DEGREES,
        }
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn parse_usize(field: &str, value: &str) -> io::Result<usize> {
    value.trim().parse().map_err(|_| {
        invalid(format!(
            "{field} must be a non-negative integer, got {value}"
        ))
    })
}

fn parse_f64(field: &str, value: &str) -> io::Result<f64> {
    value
        .trim()
        .parse()
        .map_err(|_| invalid(format!("{field} must be a number, got {value}")))
}

fn parse_bool(field: &str, value: &str) -> io::Result<bool> {
    match value.trim().trim_matches('.').to_ascii_lowercase().as_str() {
        "true" | "t" => Ok(true),
        "false" | "f" => Ok(false),
        _ => Err(invalid(format!(
            "{field} must be .true. or .false., got {value}"
        ))),
    }
}

pub fn read_method_c_algorithm_options(contents: &str) -> io::Result<MethodCAlgorithmOptions> {
    if !namelist_has_section(contents, "method_c") {
        return Ok(MethodCAlgorithmOptions::default());
    }
    let mut options = MethodCAlgorithmOptions::default();
    for assignment in namelist_assignments(contents, "method_c")? {
        match assignment.field.as_str() {
            "algorithm" => {
                options.algorithm = match assignment
                    .value
                    .trim()
                    .trim_matches(['\'', '"'])
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "canonical" => MethodCAlgorithm::Canonical,
                    "lepp_delaunay" => MethodCAlgorithm::LeppDelaunay,
                    other => {
                        return Err(invalid(format!(
                            "method_c algorithm must be canonical or lepp_delaunay, got {other}"
                        )))
                    }
                }
            }
            "max_cycles" => options.max_cycles = parse_usize(&assignment.field, &assignment.value)?,
            "target_size_tolerance" => {
                options.target_size_tolerance = parse_f64(&assignment.field, &assignment.value)?
            }
            "maximum_neighbor_size_ratio" => {
                options.maximum_neighbor_size_ratio =
                    parse_f64(&assignment.field, &assignment.value)?
            }
            "maximum_vertices" => {
                options.maximum_vertices = parse_usize(&assignment.field, &assignment.value)?
            }
            "maximum_insertions_per_cycle" => {
                options.maximum_insertions_per_cycle =
                    parse_usize(&assignment.field, &assignment.value)?
            }
            "maximum_path_length" => {
                options.maximum_path_length = parse_usize(&assignment.field, &assignment.value)?
            }
            "stop_at_source_resolution" => {
                options.stop_at_source_resolution =
                    parse_bool(&assignment.field, &assignment.value)?
            }
            "minimum_triangle_angle_deg" => {
                options.minimum_triangle_angle_deg =
                    parse_f64(&assignment.field, &assignment.value)?
            }
            other => return Err(invalid(format!("unknown &method_c field '{other}'"))),
        }
    }
    if options.max_cycles == 0
        || options.maximum_vertices == 0
        || options.maximum_insertions_per_cycle == 0
        || options.maximum_path_length == 0
    {
        return Err(invalid(
            "Method-C LEPP integer limits must be greater than zero",
        ));
    }
    if !options.target_size_tolerance.is_finite() || options.target_size_tolerance < 1.0 {
        return Err(invalid(
            "target_size_tolerance must be finite and at least one",
        ));
    }
    if !options.maximum_neighbor_size_ratio.is_finite()
        || options.maximum_neighbor_size_ratio <= 1.0
    {
        return Err(invalid(
            "maximum_neighbor_size_ratio must be finite and greater than one",
        ));
    }
    if !options.minimum_triangle_angle_deg.is_finite()
        || !(0.0..60.0).contains(&options.minimum_triangle_angle_deg)
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
    fn absent_group_is_canonical() {
        assert_eq!(
            read_method_c_algorithm_options("&mkgrd\n/\n").unwrap(),
            MethodCAlgorithmOptions::default()
        );
    }

    #[test]
    fn lepp_settings_are_parsed_and_validated() {
        let options = read_method_c_algorithm_options(
            "&method_c\n NL%algorithm='lepp_delaunay'\n NL%max_cycles=3\n \
             NL%maximum_insertions_per_cycle=9\n NL%stop_at_source_resolution=.false.\n \
             NL%minimum_triangle_angle_deg=20.0\n/\n",
        )
        .unwrap();
        assert_eq!(options.algorithm, MethodCAlgorithm::LeppDelaunay);
        assert_eq!(options.max_cycles, 3);
        assert_eq!(options.maximum_insertions_per_cycle, 9);
        assert!(!options.stop_at_source_resolution);
        assert_eq!(options.minimum_triangle_angle_deg, 20.0);

        assert!(read_method_c_algorithm_options("&method_c\n NL%algorithm='typo'\n/\n").is_err());
        assert!(read_method_c_algorithm_options("&method_c\n NL%max_cycles=0\n/\n").is_err());
        assert!(read_method_c_algorithm_options("&method_c\n NL%extra=1\n/\n").is_err());
        assert!(read_method_c_algorithm_options(
            "&method_c\n NL%minimum_triangle_angle_deg=60.0\n/\n"
        )
        .is_err());
    }
}
