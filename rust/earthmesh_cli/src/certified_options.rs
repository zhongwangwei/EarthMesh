//! Namelist controls for the CMRC peer backend.

use std::io;

use crate::namelist_reader::{namelist_assignments, namelist_has_section};
use earthmesh_refine_certified::requirement::heterogeneity::Statistic;
use earthmesh_refine_certified::AngleContractId;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedMode {
    #[default]
    SafeMotherOnly,
    ReverseCoarsening,
    /// A coarser certified mother stretched toward the demand, falling back
    /// to the safe mother when no stretch passes (guide 11.87).
    StretchedMother,
    /// The coarsest certified mother whose moved vertices serve the demand
    /// -- the Schmidt stretch or the equidistribution, whichever needs fewer
    /// cells -- falling back to the safe mother (guide 11.88).
    EquidistributedMother,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedDelivery {
    Tri,
    Hex,
    #[default]
    Coupled,
}

/// Where reverse coarsening builds the finest mother (design B1, guide
/// 11.106): over the whole sphere, or only where the requirement reaches and
/// a regional run delivers cells, the rest settled by construction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CertifiedMaterialization {
    #[default]
    Whole,
    /// Built on demand, then put back together into the whole sphere, which
    /// is certified and published as the whole route publishes it.
    OnDemand,
    /// Built on demand and published as the region: the delivered domain
    /// cut from the built region's final mesh, the parent being that mesh --
    /// no sphere is assembled, so the base may be as fine as a 30 m run needs
    /// (guide 11.109).
    Regional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CertifiedRunOptions {
    pub mode: CertifiedMode,
    pub delivery: CertifiedDelivery,
    pub angle_contract: AngleContractId,
    pub maximum_level: usize,
    pub maximum_cells: usize,
    pub gradation_rings_per_level: usize,
    pub search_budget: usize,
    pub materialization: CertifiedMaterialization,
}

impl Default for CertifiedRunOptions {
    fn default() -> Self {
        Self {
            mode: CertifiedMode::SafeMotherOnly,
            delivery: CertifiedDelivery::Coupled,
            // The projects' contract (`CertifiedAngleContract::default`): a
            // namelist that names none gets what a project would. Under the
            // strict 40-80 window reverse coarsening finds no legal state on
            // some mixed demands at all (guide 11.102).
            angle_contract: AngleContractId::DomainQuality38To82V1,
            maximum_level: 8,
            maximum_cells: 10_000_000,
            gradation_rings_per_level: 3,
            search_budget: 100_000,
            materialization: CertifiedMaterialization::Whole,
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
                    "stretched_mother" => CertifiedMode::StretchedMother,
                    "equidistributed_mother" => CertifiedMode::EquidistributedMother,
                    other => {
                        return Err(invalid(format!(
                    "certified mode must be safe_mother_only, reverse_coarsening, stretched_mother or equidistributed_mother, got {other}"
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
            "angle_contract" => {
                options.angle_contract = match assignment.value.to_ascii_lowercase().as_str() {
                    "legacy_strict_40_to_80" => AngleContractId::LegacyStrict40To80,
                    "domain_quality_38_to_82_v1" => AngleContractId::DomainQuality38To82V1,
                    other => {
                        return Err(invalid(format!(
                            "certified angle_contract must be legacy_strict_40_to_80 or domain_quality_38_to_82_v1, got {other}"
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
            "materialization" => {
                options.materialization = match assignment.value.to_ascii_lowercase().as_str() {
                    "whole" => CertifiedMaterialization::Whole,
                    "on_demand" => CertifiedMaterialization::OnDemand,
                    "regional" => CertifiedMaterialization::Regional,
                    other => {
                        return Err(invalid(format!(
                            "certified materialization must be whole, on_demand or regional, got {other}"
                        )))
                    }
                }
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

/// How fine the merge requirement starts: a number of levels below the base
/// (NXP), or a cell size the levels are snapped to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CertifiedFinest {
    Levels(usize),
    Metres(f64),
}

/// One merge criterion: a data layer -- a NetCDF file, or a directory of
/// 5-degree tiles -- its variable, a statistic over the samples a lattice
/// face holds, and the threshold it must meet for the face to merge.
#[derive(Clone, Debug, PartialEq)]
pub struct CertifiedMergeCriterion {
    pub file: std::path::PathBuf,
    pub variable: String,
    pub statistic: Statistic,
    pub threshold: f64,
}

/// CMRC's merge-if-homogeneous requirement (`&certified_merge`, design H3,
/// `docs/certified_mesh/heterogeneity_merge.md`): reverse coarsening starts
/// at the finest level and forms a parent only where every criterion passes
/// on the data it covers, stopping at the base (NXP). It replaces the
/// h-field requirement of `&mkrefine`.
#[derive(Clone, Debug, PartialEq)]
pub struct CertifiedMergeOptions {
    pub finest: CertifiedFinest,
    pub criteria: Vec<CertifiedMergeCriterion>,
    /// Fewer samples of a layer than this in a face give no evidence
    /// against merging it.
    pub minimum_samples: u64,
}

impl CertifiedMergeOptions {
    /// Levels below a base of `base_nxp`: as given, or the power of two
    /// nearest the ratio of the base cell to `finest_m`, cells measured as
    /// the h-field measures them (2 pi R / 5 NXP).
    pub fn levels(&self, base_nxp: usize) -> io::Result<usize> {
        match self.finest {
            CertifiedFinest::Levels(levels) => Ok(levels),
            CertifiedFinest::Metres(metres) => {
                let base_m = Self::base_cell_m(base_nxp);
                let levels = (base_m / metres).log2().round();
                if levels < 1.0 {
                    return Err(invalid(format!(
                        "certified_merge finest_m={metres} is not finer than the base cell \
                         ({base_m:.0} m at NXP={base_nxp})"
                    )));
                }
                Ok(levels as usize)
            }
        }
    }

    /// The base cell size the levels count down from, metres.
    pub fn base_cell_m(base_nxp: usize) -> f64 {
        2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS / (5.0 * base_nxp as f64)
    }
}

/// `&certified_merge`, if the namelist has one. Criteria are numbered:
///
/// ```text
/// &certified_merge
///   NL%finest_m = 30.            ! or NL%levels = 5
///   NL%minimum_samples = 4
///   NL%layer_file(1) = '/data/merit_hydro'   NL%layer_variable(1) = 'elv'
///   NL%statistic(1) = 'std'                  NL%threshold(1) = 5.
/// /
/// ```
///
/// Statistics are `std` (standard deviation, at most the threshold), `cv`
/// (coefficient of variation, at most) and `purity` (the most frequent
/// class's share of a categorical layer, at least).
pub fn read_certified_merge_options(contents: &str) -> io::Result<Option<CertifiedMergeOptions>> {
    if !namelist_has_section(contents, "certified_merge") {
        return Ok(None);
    }
    #[derive(Default)]
    struct Partial {
        file: Option<String>,
        variable: Option<String>,
        statistic: Option<Statistic>,
        threshold: Option<f64>,
    }
    let mut finest_m = None;
    let mut levels = None;
    let mut minimum_samples = 4u64;
    let mut partial = std::collections::BTreeMap::<usize, Partial>::new();
    for assignment in namelist_assignments(contents, "certified_merge")? {
        let value = assignment.value.trim();
        let field = assignment.field.as_str();
        let indexed = matches!(
            field,
            "layer_file" | "layer_variable" | "statistic" | "threshold"
        );
        if indexed != (assignment.indices.len() == 1) {
            return Err(invalid(format!(
                "&certified_merge {field} {}",
                if indexed {
                    "takes one criterion index, as statistic(1)"
                } else {
                    "takes no index"
                }
            )));
        }
        let criterion = || partial_entry(&assignment.indices);
        match field {
            "finest_m" => {
                let metres = value.replace(['d', 'D'], "e").parse::<f64>().ok();
                finest_m = Some(
                    metres
                        .filter(|m| m.is_finite() && *m > 0.0)
                        .ok_or_else(|| {
                            invalid(format!(
                                "certified_merge finest_m must be positive, got {value}"
                            ))
                        })?,
                )
            }
            "levels" => levels = Some(parse_usize(field, value)?),
            "minimum_samples" => minimum_samples = parse_usize(field, value)? as u64,
            "layer_file" => partial.entry(criterion()?).or_default().file = Some(value.to_owned()),
            "layer_variable" => {
                partial.entry(criterion()?).or_default().variable = Some(value.to_owned())
            }
            "statistic" => {
                partial.entry(criterion()?).or_default().statistic =
                    Some(match value.to_ascii_lowercase().as_str() {
                        "std" | "standard_deviation" => Statistic::StandardDeviation,
                        "cv" | "coefficient_of_variation" => Statistic::CoefficientOfVariation,
                        "purity" => Statistic::Purity,
                        other => {
                            return Err(invalid(format!(
                                "certified_merge statistic must be std, cv or purity, got {other}"
                            )))
                        }
                    })
            }
            "threshold" => {
                let threshold = value.replace(['d', 'D'], "e").parse::<f64>().ok();
                partial.entry(criterion()?).or_default().threshold = Some(
                    threshold
                        .filter(|t| t.is_finite() && *t >= 0.0)
                        .ok_or_else(|| {
                            invalid(format!(
                            "certified_merge threshold must be a non-negative number, got {value}"
                        ))
                        })?,
                )
            }
            other => return Err(invalid(format!("unknown &certified_merge field '{other}'"))),
        }
    }
    let finest = match (finest_m, levels) {
        (Some(metres), None) => CertifiedFinest::Metres(metres),
        (None, Some(levels)) if levels > 0 => CertifiedFinest::Levels(levels),
        (None, Some(_)) => return Err(invalid("certified_merge levels must be positive")),
        _ => {
            return Err(invalid(
                "&certified_merge needs exactly one of finest_m and levels",
            ))
        }
    };
    let mut criteria = Vec::new();
    for (index, entry) in partial {
        let (Some(file), Some(variable), Some(statistic), Some(threshold)) =
            (entry.file, entry.variable, entry.statistic, entry.threshold)
        else {
            return Err(invalid(format!(
                "certified_merge criterion {index} needs layer_file, layer_variable, statistic \
                 and threshold"
            )));
        };
        if statistic == Statistic::Purity && threshold > 1.0 {
            return Err(invalid(format!(
                "certified_merge criterion {index}: a purity threshold is a share, at most 1"
            )));
        }
        criteria.push(CertifiedMergeCriterion {
            file: file.into(),
            variable,
            statistic,
            threshold,
        });
    }
    if criteria.is_empty() {
        return Err(invalid("&certified_merge names no criterion"));
    }
    Ok(Some(CertifiedMergeOptions {
        finest,
        criteria,
        minimum_samples,
    }))
}

fn partial_entry(indices: &[usize]) -> io::Result<usize> {
    match indices {
        [index] if *index > 0 => Ok(*index),
        _ => Err(invalid("certified_merge criteria are numbered from 1")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_merge_criteria_and_rejects_incomplete_ones() {
        let options = read_certified_merge_options(
            "&certified_merge\n NL%finest_m=30.\n NL%minimum_samples=6\n \
             NL%layer_file(1)='/data/merit'\n NL%layer_variable(1)='elv'\n \
             NL%statistic(1)='std'\n NL%threshold(1)=5.\n \
             NL%layer_file(2)='/data/lc.nc'\n NL%layer_variable(2)='landtype'\n \
             NL%statistic(2)='purity'\n NL%threshold(2)=0.8\n/",
        )
        .unwrap()
        .unwrap();
        assert_eq!(options.finest, CertifiedFinest::Metres(30.0));
        assert_eq!(options.minimum_samples, 6);
        assert_eq!(
            options.criteria,
            vec![
                CertifiedMergeCriterion {
                    file: "/data/merit".into(),
                    variable: "elv".into(),
                    statistic: Statistic::StandardDeviation,
                    threshold: 5.0,
                },
                CertifiedMergeCriterion {
                    file: "/data/lc.nc".into(),
                    variable: "landtype".into(),
                    statistic: Statistic::Purity,
                    threshold: 0.8,
                },
            ]
        );
        // 30 m from a 1 km base: five levels, 31 m.
        let nxp = (CertifiedMergeOptions::base_cell_m(1) / 1000.0).round() as usize;
        assert_eq!(options.levels(nxp).unwrap(), 5);
        assert!(read_certified_merge_options("&mkgrd\n/").unwrap().is_none());

        let criterion = " NL%layer_file(1)='a.nc'\n NL%layer_variable(1)='v'\n \
                         NL%statistic(1)='cv'\n NL%threshold(1)=0.1\n";
        let with =
            |extra: &str| read_certified_merge_options(&format!("&certified_merge\n{extra}/"));
        assert_eq!(
            with(&format!(" NL%levels=2\n{criterion}"))
                .unwrap()
                .unwrap()
                .finest,
            CertifiedFinest::Levels(2)
        );
        // Neither or both of finest_m and levels; no criterion; a criterion
        // without its threshold; a purity over 1; an unknown statistic.
        assert!(with(criterion).is_err());
        assert!(with(&format!(" NL%levels=2\n NL%finest_m=30.\n{criterion}")).is_err());
        assert!(with(" NL%levels=2\n").is_err());
        assert!(with(
            " NL%levels=2\n NL%layer_file(1)='a.nc'\n NL%layer_variable(1)='v'\n \
                       NL%statistic(1)='std'\n"
        )
        .is_err());
        assert!(with(&format!(
            " NL%levels=2\n{}",
            criterion.replace("'cv'", "'purity'").replace("0.1", "1.5")
        ))
        .is_err());
        assert!(with(&format!(
            " NL%levels=2\n{}",
            criterion.replace("'cv'", "'median'")
        ))
        .is_err());
        assert!(with(&format!(" NL%levels=2\n NL%statistic=3\n{criterion}")).is_err());
        // A cell no finer than the base.
        let coarse = with(&format!(" NL%finest_m=900000.\n{criterion}"))
            .unwrap()
            .unwrap();
        assert!(coarse.levels(16).is_err());
    }

    #[test]
    fn parses_every_certified_option_and_rejects_unknown_values() {
        let options = read_certified_options(
            "&certified\n NL%mode='reverse_coarsening'\n NL%delivery='tri'\n \
             NL%maximum_level=5\n NL%maximum_cells=9000\n \
             NL%angle_contract='domain_quality_38_to_82_v1'\n \
             NL%gradation_rings_per_level=4\n NL%search_budget=700\n/",
        )
        .unwrap();
        assert_eq!(options.mode, CertifiedMode::ReverseCoarsening);
        assert_eq!(options.delivery, CertifiedDelivery::Tri);
        assert_eq!(
            options.angle_contract,
            AngleContractId::DomainQuality38To82V1
        );
        assert_eq!(options.maximum_level, 5);
        assert_eq!(options.maximum_cells, 9000);
        assert_eq!(options.gradation_rings_per_level, 4);
        assert_eq!(options.search_budget, 700);

        for (value, expected) in [
            ("whole", CertifiedMaterialization::Whole),
            ("on_demand", CertifiedMaterialization::OnDemand),
            ("regional", CertifiedMaterialization::Regional),
        ] {
            let options =
                read_certified_options(&format!("&certified\n NL%materialization='{value}'\n/"))
                    .unwrap();
            assert_eq!(options.materialization, expected);
        }
        assert!(read_certified_options("&certified\n NL%materialization='typo'\n/").is_err());
        assert!(read_certified_options("&certified\n NL%mode='typo'\n/").is_err());
        assert!(read_certified_options("&certified\n NL%angle_contract='typo'\n/").is_err());
        assert!(read_certified_options("&certified\n NL%maximum_cells=0\n/").is_err());
        assert!(read_certified_options("&certified\n NL%extra=1\n/").is_err());
    }
}
