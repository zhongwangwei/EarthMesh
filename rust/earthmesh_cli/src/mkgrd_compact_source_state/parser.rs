use std::io;

use crate::{validate_i32_matrix_shape, AreaJudgeSourceBounds, GetContainMeshKind};

use super::types::{MkgrdCompactSourceState, MkgrdCompactSourceStateFinalPostproc};

pub fn parse_mkgrd_compact_source_state(contents: &str) -> io::Result<MkgrdCompactSourceState> {
    let mut gridnum_perdegree = None;
    let mut nlons_source = None;
    let mut nlats_source = None;
    let mut first_triangle_id = Some(1usize);
    let mut num_vertex = None;
    let mut maxlc = None;
    let mut final_domain_contain = None;
    let mut final_domain_postproc = None;
    let mut calculated_minlon_source = None;
    let mut calculated_maxlon_source = None;
    let mut calculated_maxlat_source = None;
    let mut calculated_minlat_source = None;
    let mut section = "";
    let mut calculated_refine = Vec::new();
    let mut is_in_domain = Vec::new();
    let mut seaorland = Vec::new();
    let mut landtypes_global = Vec::new();

    for (line_number, raw_line) in contents.lines().enumerate() {
        let line_number = line_number + 1;
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            match section {
                "calculated_refine" | "is_in_domain" | "seaorland" | "landtypes_global" => {}
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown source-state section [{other}] at line {line_number}"),
                    ));
                }
            }
            continue;
        }

        if section.is_empty() {
            let (key, value) = line.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("source-state metadata line {line_number} must use key=value"),
                )
            })?;
            let value = value.trim();
            match key.trim() {
                "gridnum_perdegree" => {
                    gridnum_perdegree = Some(parse_source_state_positive_usize(
                        "gridnum_perdegree",
                        value,
                    )?)
                }
                "nlons" => nlons_source = Some(parse_source_state_positive_usize("nlons", value)?),
                "nlats" => nlats_source = Some(parse_source_state_positive_usize("nlats", value)?),
                "first_triangle_id" => {
                    first_triangle_id = Some(parse_source_state_positive_usize(
                        "first_triangle_id",
                        value,
                    )?)
                }
                "num_vertex" => {
                    num_vertex = Some(parse_source_state_positive_usize("num_vertex", value)?)
                }
                "calculated_minlon_source" => {
                    calculated_minlon_source = Some(parse_source_state_positive_usize(
                        "calculated_minlon_source",
                        value,
                    )?)
                }
                "calculated_maxlon_source" => {
                    calculated_maxlon_source = Some(parse_source_state_positive_usize(
                        "calculated_maxlon_source",
                        value,
                    )?)
                }
                "calculated_maxlat_source" => {
                    calculated_maxlat_source = Some(parse_source_state_positive_usize(
                        "calculated_maxlat_source",
                        value,
                    )?)
                }
                "calculated_minlat_source" => {
                    calculated_minlat_source = Some(parse_source_state_positive_usize(
                        "calculated_minlat_source",
                        value,
                    )?)
                }
                "maxlc" => {
                    maxlc = Some(value.parse::<i32>().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidInput, "maxlc must be an integer")
                    })?);
                }
                "final_domain_contain" => {
                    final_domain_contain = Some(parse_compact_source_state_contain_kind(value)?)
                }
                "final_domain_postproc" => {
                    final_domain_postproc = Some(parse_compact_source_state_postproc_kind(value)?)
                }
                other => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("unknown source-state metadata key {other} at line {line_number}"),
                    ));
                }
            }
            continue;
        }

        let row = line
            .split_whitespace()
            .map(|value| {
                value.parse::<i32>().map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "source-state section [{section}] line {line_number} has non-integer value {value}"
                        ),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if row.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source-state section [{section}] line {line_number} must not be empty"),
            ));
        }
        match section {
            "calculated_refine" => calculated_refine.push(row),
            "is_in_domain" => is_in_domain.push(row),
            "seaorland" => seaorland.push(row),
            "landtypes_global" => landtypes_global.push(row),
            _ => unreachable!(),
        }
    }

    let gridnum_perdegree = require_source_state_metadata(gridnum_perdegree, "gridnum_perdegree")?;
    let nlons_source = require_source_state_metadata(nlons_source, "nlons")?;
    let nlats_source = require_source_state_metadata(nlats_source, "nlats")?;
    let first_triangle_id = require_source_state_metadata(first_triangle_id, "first_triangle_id")?;
    let num_vertex = require_source_state_metadata(num_vertex, "num_vertex")?;
    let maxlc = require_source_state_metadata(maxlc, "maxlc")?;
    let expected_lons = nlons_source + 1;
    let expected_lats = nlats_source + 1;
    validate_i32_matrix_shape("is_in_domain", &is_in_domain, expected_lons, expected_lats)?;
    validate_i32_matrix_shape("seaorland", &seaorland, expected_lons, expected_lats)?;
    validate_i32_matrix_shape(
        "landtypes_global",
        &landtypes_global,
        expected_lons,
        expected_lats,
    )?;
    let calculated_bounds = if calculated_refine.is_empty() {
        if calculated_minlon_source.is_some()
            || calculated_maxlon_source.is_some()
            || calculated_maxlat_source.is_some()
            || calculated_minlat_source.is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "source-state calculated bounds require a [calculated_refine] section",
            ));
        }
        None
    } else {
        validate_i32_matrix_shape(
            "calculated_refine",
            &calculated_refine,
            expected_lons,
            expected_lats,
        )?;
        Some(AreaJudgeSourceBounds {
            minlon_source: require_source_state_metadata(
                calculated_minlon_source,
                "calculated_minlon_source",
            )?,
            maxlon_source: require_source_state_metadata(
                calculated_maxlon_source,
                "calculated_maxlon_source",
            )?,
            maxlat_source: require_source_state_metadata(
                calculated_maxlat_source,
                "calculated_maxlat_source",
            )?,
            minlat_source: require_source_state_metadata(
                calculated_minlat_source,
                "calculated_minlat_source",
            )?,
        })
    };

    Ok(MkgrdCompactSourceState {
        gridnum_perdegree,
        nlons_source,
        nlats_source,
        first_triangle_id,
        num_vertex,
        maxlc,
        final_domain_contain,
        final_domain_postproc,
        calculated_refine: (!calculated_refine.is_empty()).then_some(calculated_refine),
        calculated_bounds,
        is_in_domain,
        seaorland,
        landtypes_global,
    })
}

fn parse_source_state_positive_usize(name: &str, value: &str) -> io::Result<usize> {
    let parsed = value.parse::<usize>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be a positive integer"),
        )
    })?;
    if parsed == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be a positive integer"),
        ));
    }
    Ok(parsed)
}

fn require_source_state_metadata<T>(value: Option<T>, name: &str) -> io::Result<T> {
    value.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("source-state missing {name}"),
        )
    })
}

fn parse_compact_source_state_contain_kind(value: &str) -> io::Result<GetContainMeshKind> {
    match value {
        "land" | "landmesh" => Ok(GetContainMeshKind::Land),
        "ocean" | "oceanmesh" => Ok(GetContainMeshKind::Ocean),
        "atmos" | "atmosmesh" => Ok(GetContainMeshKind::Atmos),
        "loc" | "LOCmesh" | "earthmesh" => Ok(GetContainMeshKind::Loc),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source-state final_domain_contain must be one of land/ocean/atmos/loc, got {other}"
            ),
        )),
    }
}

fn parse_compact_source_state_postproc_kind(
    value: &str,
) -> io::Result<MkgrdCompactSourceStateFinalPostproc> {
    match value {
        "land" | "landmesh" => Ok(MkgrdCompactSourceStateFinalPostproc::Land),
        "ocean" | "oceanmesh" => Ok(MkgrdCompactSourceStateFinalPostproc::Ocean),
        "atmos" | "atmosmesh" => Ok(MkgrdCompactSourceStateFinalPostproc::Atmos),
        "earth" | "earthmesh" => Ok(MkgrdCompactSourceStateFinalPostproc::Earth),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "source-state final_domain_postproc must be one of land/ocean/atmos/earth, got {other}"
            ),
        )),
    }
}
