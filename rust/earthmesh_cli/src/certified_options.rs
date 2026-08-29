//! Namelist controls for the CMRC peer backend.

use std::io;

use crate::namelist_reader::{namelist_assignments, namelist_has_section};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedMode {
    #[default]
    SafeMotherOnly,
    ReverseCoarsening,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedDelivery {
    Tri,
    Hex,
    #[default]
    Coupled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedRunOptions {
    pub mode: CertifiedMode,
    pub delivery: CertifiedDelivery,
    pub maximum_level: usize,
    pub maximum_cells: usize,
    pub gradation_rings_per_level: usize,
    pub search_budget: usize,
}

impl Default for CertifiedRunOptions {
    fn default() -> Self {
        Self {
            mode: CertifiedMode::SafeMotherOnly,
            delivery: CertifiedDelivery::Coupled,
            maximum_level: 8,
            maximum_cells: 5_000_000,
            gradation_rings_per_level: 3,
            search_budget: 100_000,
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

pub fn read_certified_options(contents: &str) -> io::Result<CertifiedRunOptions> {
    if !namelist_has_section(contents, "certified") {
        return Ok(CertifiedRunOptions::default());
    }
    let mut options = CertifiedRunOptions::default();
    for assignment in namelist_assignments(contents, "certified")? {
        match assignment.field.as_str() {
            "mode" => {
                options.mode = match assignment.value.to_ascii_lowercase().as_str() {
                    "safe_mother_only" => CertifiedMode::SafeMotherOnly,
                    "reverse_coarsening" => CertifiedMode::ReverseCoarsening,
                    other => {
                        return Err(invalid(format!(
                    "certified mode must be safe_mother_only or reverse_coarsening, got {other}"
                )))
                    }
                }
            }
            "delivery" => {
                options.delivery = match assignment.value.to_ascii_lowercase().as_str() {
                    "tri" => CertifiedDelivery::Tri,
                    "hex" => CertifiedDelivery::Hex,
                    "coupled" => CertifiedDelivery::Coupled,
                    other => {
                        return Err(invalid(format!(
                            "certified delivery must be tri, hex, or coupled, got {other}"
                        )))
                    }
                }
            }
            "maximum_level" => {
                options.maximum_level = parse_usize(&assignment.field, &assignment.value)?
            }
            "maximum_cells" => {
                options.maximum_cells = parse_usize(&assignment.field, &assignment.value)?
            }
            "gradation_rings_per_level" => {
                options.gradation_rings_per_level =
                    parse_usize(&assignment.field, &assignment.value)?
            }
            "search_budget" => {
                options.search_budget = parse_usize(&assignment.field, &assignment.value)?
            }
            other => return Err(invalid(format!("unknown &certified field '{other}'"))),
        }
    }
    if options.maximum_cells == 0
        || options.gradation_rings_per_level == 0
        || options.search_budget == 0
    {
        return Err(invalid(
            "certified maximum_cells, gradation_rings_per_level, and search_budget must be positive",
        ));
    }
    Ok(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_certified_option_and_rejects_unknown_values() {
        let options = read_certified_options(
            "&certified\n NL%mode='reverse_coarsening'\n NL%delivery='tri'\n \
             NL%maximum_level=5\n NL%maximum_cells=9000\n \
             NL%gradation_rings_per_level=4\n NL%search_budget=700\n/",
        )
        .unwrap();
        assert_eq!(options.mode, CertifiedMode::ReverseCoarsening);
        assert_eq!(options.delivery, CertifiedDelivery::Tri);
        assert_eq!(options.maximum_level, 5);
        assert_eq!(options.maximum_cells, 9000);
        assert_eq!(options.gradation_rings_per_level, 4);
        assert_eq!(options.search_budget, 700);

        assert!(read_certified_options("&certified\n NL%mode='typo'\n/").is_err());
        assert!(read_certified_options("&certified\n NL%maximum_cells=0\n/").is_err());
        assert!(read_certified_options("&certified\n NL%extra=1\n/").is_err());
    }
}
