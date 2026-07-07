use std::io;
use std::path::Path;

use crate::{read_gridfile_mesh_points, MaskPostprocFinalizationReport};

pub(crate) struct OptionalRefineLevelVectors {
    pub m: Vec<i32>,
    pub w: Vec<i32>,
}

pub(crate) struct FinalRefineLevelVectors {
    pub m: Option<Vec<i32>>,
    pub w: Option<Vec<i32>>,
}

pub(crate) fn refine_levels_from_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<OptionalRefineLevelVectors> {
    let mesh = read_gridfile_mesh_points(gridfile)?;
    Ok(OptionalRefineLevelVectors {
        m: mesh.m_refine_level,
        w: mesh.w_refine_level,
    })
}

pub(crate) fn final_refine_levels_for_mask_postproc(
    mode_grid: &str,
    report: &MaskPostprocFinalizationReport,
    is_in_domain: &[i32],
    ustr_points: usize,
    source_m_levels: &[i32],
    source_w_levels: &[i32],
) -> io::Result<FinalRefineLevelVectors> {
    match mode_grid.trim() {
        "tri" => {
            let center_levels =
                levels_for_layout("earthmesh_m_refine_level", source_m_levels, ustr_points)?;
            let vertex_levels = levels_for_layout(
                "earthmesh_w_refine_level",
                source_w_levels,
                report.vertex_reindex.vertex_mapping.len().saturating_sub(1),
            )?;
            Ok(FinalRefineLevelVectors {
                m: compact_center_levels(
                    center_levels.as_deref().unwrap_or(&[]),
                    is_in_domain,
                    ustr_points,
                    report.mesh.m_points.len(),
                ),
                w: compact_vertex_levels(
                    vertex_levels.as_deref().unwrap_or(&[]),
                    &report.vertex_reindex.sorted_vertices,
                    report.mesh.w_points.len(),
                ),
            })
        }
        "hex" => {
            let center_levels =
                levels_for_layout("earthmesh_w_refine_level", source_w_levels, ustr_points)?;
            let vertex_levels = levels_for_layout(
                "earthmesh_m_refine_level",
                source_m_levels,
                report.vertex_reindex.vertex_mapping.len().saturating_sub(1),
            )?;
            Ok(FinalRefineLevelVectors {
                m: compact_vertex_levels(
                    vertex_levels.as_deref().unwrap_or(&[]),
                    &report.vertex_reindex.sorted_vertices,
                    report.mesh.m_points.len(),
                ),
                w: compact_center_levels(
                    center_levels.as_deref().unwrap_or(&[]),
                    is_in_domain,
                    ustr_points,
                    report.mesh.w_points.len(),
                ),
            })
        }
        _ => Ok(FinalRefineLevelVectors { m: None, w: None }),
    }
}

pub(crate) fn final_refine_levels_from_gridfile_for_mask_postproc(
    mode_grid: &str,
    source_gridfile: impl AsRef<Path>,
    report: &MaskPostprocFinalizationReport,
    is_in_domain: &[i32],
    ustr_points: usize,
) -> io::Result<FinalRefineLevelVectors> {
    let source_levels = refine_levels_from_gridfile(source_gridfile)?;
    final_refine_levels_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source_levels.m,
        &source_levels.w,
    )
}

fn levels_for_layout(
    name: &str,
    source_levels: &[i32],
    expected_len: usize,
) -> io::Result<Option<Vec<i32>>> {
    if source_levels.is_empty() {
        Ok(None)
    } else if source_levels.len() == expected_len {
        Ok(Some(
            source_levels.iter().map(|level| (*level).max(0)).collect(),
        ))
    } else if source_levels.len() + 1 == expected_len {
        let mut levels = Vec::with_capacity(expected_len);
        levels.push(0);
        levels.extend(source_levels.iter().map(|level| (*level).max(0)));
        Ok(Some(levels))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must equal mask-postproc layout length {expected_len} (or be exactly one shorter when a placeholder row is inserted)",
                source_levels.len()
            ),
        ))
    }
}

fn compact_center_levels(
    source_levels: &[i32],
    is_in_domain: &[i32],
    ustr_points: usize,
    output_len: usize,
) -> Option<Vec<i32>> {
    if source_levels.len() < ustr_points || is_in_domain.len() < ustr_points {
        return None;
    }
    let mut levels = vec![0; output_len];
    if output_len > 1 && source_levels.len() > 1 {
        levels[1] = source_levels[1].max(0);
    }
    let mut compact_center_id = 1usize;
    for source_center_id in 2..ustr_points {
        if is_in_domain[source_center_id] != 1 {
            continue;
        }
        compact_center_id += 1;
        if compact_center_id >= levels.len() {
            return None;
        }
        levels[compact_center_id] = source_levels[source_center_id].max(0);
    }
    Some(levels)
}

fn compact_vertex_levels(
    source_levels: &[i32],
    sorted_vertices: &[usize],
    output_len: usize,
) -> Option<Vec<i32>> {
    if sorted_vertices
        .iter()
        .any(|&source_vertex_id| source_vertex_id >= source_levels.len())
    {
        return None;
    }
    let mut levels = vec![0; output_len];
    for (offset, &source_vertex_id) in sorted_vertices.iter().enumerate() {
        let final_vertex_id = offset + 1;
        if final_vertex_id >= levels.len() {
            return None;
        }
        levels[final_vertex_id] = source_levels[source_vertex_id].max(0);
    }
    Some(levels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_levels_accept_inserted_mask_postproc_placeholder() {
        assert_eq!(
            levels_for_layout("levels", &[0, 4], 3).unwrap(),
            Some(vec![0, 0, 4])
        );
        assert_eq!(
            levels_for_layout("levels", &[0, 0, 4], 3).unwrap(),
            Some(vec![0, 0, 4])
        );
        assert!(levels_for_layout("levels", &[0, 1, 2, 3], 2).is_err());
    }
}
