use std::io;

use crate::*;

/// Concatenate land, ocean, and atmosphere threshold reports in the same order
/// used by the top-level `MOD_GetRef:GetRef` LOCmesh aggregation.
pub fn aggregate_getref_threshold_reports_fortran_indexed(
    num_vertex: usize,
    land: Option<&GetRefLandThresholdReport>,
    ocean: Option<&GetRefOceanThresholdReport>,
    atmos: Option<&GetRefAtmosThresholdReport>,
) -> io::Result<GetRefThresholdAggregationReport> {
    let mut components: Vec<(&str, &[String], &[Vec<i32>], usize)> = Vec::new();
    if let Some(report) = land {
        validate_getref_land_threshold_report_for_aggregation(report)?;
        components.push((
            "land",
            &report.column_names,
            &report.ref_th_land,
            report.ref_colnum,
        ));
    }
    if let Some(report) = ocean {
        validate_getref_ocean_threshold_report_for_aggregation(report)?;
        components.push((
            "ocean",
            &report.column_names,
            &report.ref_th,
            report.ref_colnum,
        ));
    }
    if let Some(report) = atmos {
        validate_getref_atmos_threshold_report_for_aggregation(report)?;
        components.push((
            "atmos",
            &report.column_names,
            &report.ref_th,
            report.ref_colnum,
        ));
    }
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "GetRef aggregation requires at least one component threshold report",
        ));
    }

    let sjx_points = components[0].2.len().checked_sub(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "component threshold matrix must include a Fortran placeholder row",
        )
    })?;
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    for (source, _, ref_th, _) in &components {
        let component_sjx_points = ref_th.len().checked_sub(1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{source} threshold matrix must include a Fortran placeholder row"),
            )
        })?;
        if component_sjx_points != sjx_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{source} sjx_points {component_sjx_points} must match aggregation sjx_points {sjx_points}"
                ),
            ));
        }
    }

    let ref_colnum = components
        .iter()
        .map(|(_, _, _, ref_colnum)| *ref_colnum)
        .sum::<usize>();
    let mut column_sources = Vec::with_capacity(ref_colnum);
    let mut column_names = Vec::with_capacity(ref_colnum);
    let mut ref_th = vec![vec![0; ref_colnum + 1]; sjx_points + 1];
    let mut target_col = 0usize;
    for (source, names, source_ref_th, source_colnum) in components {
        for source_col in 1..=source_colnum {
            target_col += 1;
            column_sources.push(source.to_string());
            column_names.push(names[source_col - 1].clone());
            copy_getref_threshold_column(source_ref_th, source_col, &mut ref_th, target_col)?;
        }
    }

    let ref_sjx = aggregate_getref_ref_sjx(&ref_th, num_vertex, ref_colnum)?;
    Ok(GetRefThresholdAggregationReport {
        ref_colnum,
        column_sources,
        column_names,
        ref_th,
        ref_sjx,
    })
}

pub(crate) fn empty_getref_threshold_aggregation_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
) -> io::Result<GetRefThresholdAggregationReport> {
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    let ref_th = vec![vec![0]; sjx_points + 1];
    let ref_sjx = aggregate_getref_ref_sjx(&ref_th, num_vertex, 0)?;
    Ok(GetRefThresholdAggregationReport {
        ref_colnum: 0,
        column_sources: Vec::new(),
        column_names: Vec::new(),
        ref_th,
        ref_sjx,
    })
}
