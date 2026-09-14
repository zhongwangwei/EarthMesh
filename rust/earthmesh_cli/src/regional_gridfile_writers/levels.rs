use std::io;
use std::path::Path;

use crate::{
    read_gridfile_cell_lineages, read_gridfile_mesh_points, MaskPostprocFinalizationReport,
    MethodCGridfileMetadataSlices,
};

pub(crate) struct OptionalRefineLevelVectors {
    pub mpas: Option<crate::mpas_gridfile_context::MpasGridfileContext>,
    pub hfield: Option<crate::hfield_gridfile_context::HfieldGridfileContext>,
    pub m_lineage: Vec<i64>,
    pub m: Vec<i32>,
    pub m_orig: Vec<i32>,
    pub m_ngr: Vec<i32>,
    pub w_lineage: Vec<i64>,
    pub w: Vec<i32>,
    pub w_orig: Vec<i32>,
    pub w_ngr: Vec<i32>,
}

pub(crate) struct FinalRefineLevelVectors {
    pub mpas: Option<crate::mpas_gridfile_context::MpasGridfileContext>,
    pub hfield: Option<crate::hfield_gridfile_context::HfieldGridfileContext>,
    pub m_lineage: Option<Vec<i64>>,
    pub m: Option<Vec<i32>>,
    pub m_orig: Option<Vec<i32>>,
    pub m_ngr: Option<Vec<i32>>,
    pub w_lineage: Option<Vec<i64>>,
    pub w: Option<Vec<i32>>,
    pub w_orig: Option<Vec<i32>>,
    pub w_ngr: Option<Vec<i32>>,
}

impl FinalRefineLevelVectors {
    pub(crate) fn slices(&self) -> MethodCGridfileMetadataSlices<'_> {
        MethodCGridfileMetadataSlices {
            mpas: self.mpas.as_ref(),
            hfield: self.hfield.as_ref(),
            m_lineage: self.m_lineage.as_deref(),
            m_refine_level: self.m.as_deref(),
            m_refine_level_orig: self.m_orig.as_deref(),
            m_ngr: self.m_ngr.as_deref(),
            w_lineage: self.w_lineage.as_deref(),
            w_refine_level: self.w.as_deref(),
            w_refine_level_orig: self.w_orig.as_deref(),
            w_ngr: self.w_ngr.as_deref(),
        }
    }

    pub(crate) fn duplicate_w_vertices(&mut self, source_rows: &[usize]) -> io::Result<()> {
        if let Some(context) = &mut self.mpas {
            duplicate_metadata_rows(&mut context.cellwidth_km, source_rows, "MPAS cellwidth")?;
        }
        if let Some(values) = &mut self.w_lineage {
            duplicate_metadata_rows(values, source_rows, "lineage")?;
        }
        for values in [&mut self.w, &mut self.w_orig, &mut self.w_ngr]
            .into_iter()
            .flatten()
        {
            duplicate_metadata_rows(values, source_rows, "metadata")?;
        }
        Ok(())
    }
}

fn duplicate_metadata_rows<T: Copy>(
    values: &mut Vec<T>,
    source_rows: &[usize],
    role: &str,
) -> io::Result<()> {
    for &source_row in source_rows {
        let value = *values.get(source_row).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("split W vertex source row {source_row} has no {role}"),
            )
        })?;
        values.push(value);
    }
    Ok(())
}

pub(crate) fn refine_levels_from_gridfile(
    gridfile: impl AsRef<Path>,
) -> io::Result<OptionalRefineLevelVectors> {
    let gridfile = gridfile.as_ref();
    let mesh = read_gridfile_mesh_points(gridfile)?;
    let lineages = read_gridfile_cell_lineages(gridfile)?;
    Ok(OptionalRefineLevelVectors {
        mpas: crate::mpas_gridfile_context::read_mpas_gridfile_context(gridfile)?,
        hfield: crate::hfield_gridfile_context::read_hfield_gridfile_context(gridfile)?,
        m_lineage: lineages.m,
        m: mesh.m_refine_level,
        m_orig: mesh.m_refine_level_orig,
        m_ngr: mesh.m_ngr,
        w_lineage: lineages.w,
        w: mesh.w_refine_level,
        w_orig: mesh.w_refine_level_orig,
        w_ngr: mesh.w_ngr,
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
                ..empty_final_metadata()
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
                ..empty_final_metadata()
            })
        }
        _ => Ok(empty_final_metadata()),
    }
}

fn empty_final_metadata() -> FinalRefineLevelVectors {
    FinalRefineLevelVectors {
        mpas: None,
        hfield: None,
        m_lineage: None,
        m: None,
        m_orig: None,
        m_ngr: None,
        w_lineage: None,
        w: None,
        w_orig: None,
        w_ngr: None,
    }
}

pub(crate) fn final_method_c_metadata_for_mask_postproc(
    mode_grid: &str,
    report: &MaskPostprocFinalizationReport,
    is_in_domain: &[i32],
    ustr_points: usize,
    source: &OptionalRefineLevelVectors,
) -> io::Result<FinalRefineLevelVectors> {
    let current = final_refine_levels_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source.m,
        &source.w,
    )?;
    let original = final_refine_levels_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source.m_orig,
        &source.w_orig,
    )?;
    let ngr = final_refine_levels_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source.m_ngr,
        &source.w_ngr,
    )?;
    let lineages = final_lineages_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source.m_lineage,
        &source.w_lineage,
    )?;
    Ok(FinalRefineLevelVectors {
        hfield: source.hfield.clone(),
        mpas: compact_mpas_context(
            mode_grid,
            report,
            is_in_domain,
            ustr_points,
            source.mpas.as_ref(),
        )?,
        m_lineage: lineages.m_lineage,
        m: current.m,
        m_orig: original.m,
        m_ngr: ngr.m,
        w_lineage: lineages.w_lineage,
        w: current.w,
        w_orig: original.w,
        w_ngr: ngr.w,
    })
}

fn compact_mpas_context(
    mode_grid: &str,
    report: &MaskPostprocFinalizationReport,
    is_in_domain: &[i32],
    ustr_points: usize,
    source: Option<&crate::mpas_gridfile_context::MpasGridfileContext>,
) -> io::Result<Option<crate::mpas_gridfile_context::MpasGridfileContext>> {
    let Some(source) = source else {
        return Ok(None);
    };
    let expected = match mode_grid.trim() {
        "hex" => ustr_points,
        "tri" => report.vertex_reindex.vertex_mapping.len().saturating_sub(1),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "MPAS width compaction requires tri or hex",
            ))
        }
    };
    let widths = values_for_layout("MPAS W cellwidth", &source.cellwidth_km, expected)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "MPAS width context is empty"))?;
    let mut compacted = if mode_grid.trim() == "hex" {
        compact_center_values(
            &widths,
            is_in_domain,
            ustr_points,
            report.mesh.w_points.len(),
        )
    } else {
        compact_vertex_values(
            &widths,
            &report.vertex_reindex.sorted_vertices,
            report.mesh.w_points.len(),
        )
    }
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "MPAS W cellwidth compaction lost row alignment",
        )
    })?;
    // Compaction inserts sentinel slots. They have no physical scale and must
    // not lower the original producer's density normalization.
    // finalize_mask_postproc_data_one_based always emits slots 0 and 1;
    // do not infer these known slots from their coordinates or padded rings.
    compacted
        .get_mut(..2)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "MPAS compact mesh lacks placeholder slots",
            )
        })?
        .fill(source.cellwidth_km[0]);
    let result = crate::mpas_gridfile_context::MpasGridfileContext {
        cellwidth_km: compacted,
        base_nxp: source.base_nxp,
        step: source.step,
        density_reference_width_km: source.density_reference_width_km,
        source: source.source.clone(),
    };
    result.validate(report.mesh.w_points.len())?;
    Ok(Some(result))
}

struct FinalLineageVectors {
    m_lineage: Option<Vec<i64>>,
    w_lineage: Option<Vec<i64>>,
}

fn final_lineages_for_mask_postproc(
    mode_grid: &str,
    report: &MaskPostprocFinalizationReport,
    is_in_domain: &[i32],
    ustr_points: usize,
    source_m_lineages: &[i64],
    source_w_lineages: &[i64],
) -> io::Result<FinalLineageVectors> {
    match mode_grid.trim() {
        "tri" => {
            let center_lineages =
                values_for_layout("earthmesh_m_lineage", source_m_lineages, ustr_points)?;
            let vertex_lineages = values_for_layout(
                "earthmesh_w_lineage",
                source_w_lineages,
                report.vertex_reindex.vertex_mapping.len().saturating_sub(1),
            )?;
            Ok(FinalLineageVectors {
                m_lineage: compact_center_values(
                    center_lineages.as_deref().unwrap_or(&[]),
                    is_in_domain,
                    ustr_points,
                    report.mesh.m_points.len(),
                ),
                w_lineage: compact_vertex_values(
                    vertex_lineages.as_deref().unwrap_or(&[]),
                    &report.vertex_reindex.sorted_vertices,
                    report.mesh.w_points.len(),
                ),
            })
        }
        "hex" => {
            let center_lineages =
                values_for_layout("earthmesh_w_lineage", source_w_lineages, ustr_points)?;
            let vertex_lineages = values_for_layout(
                "earthmesh_m_lineage",
                source_m_lineages,
                report.vertex_reindex.vertex_mapping.len().saturating_sub(1),
            )?;
            Ok(FinalLineageVectors {
                m_lineage: compact_vertex_values(
                    vertex_lineages.as_deref().unwrap_or(&[]),
                    &report.vertex_reindex.sorted_vertices,
                    report.mesh.m_points.len(),
                ),
                w_lineage: compact_center_values(
                    center_lineages.as_deref().unwrap_or(&[]),
                    is_in_domain,
                    ustr_points,
                    report.mesh.w_points.len(),
                ),
            })
        }
        _ => Ok(FinalLineageVectors {
            m_lineage: None,
            w_lineage: None,
        }),
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
    final_method_c_metadata_for_mask_postproc(
        mode_grid,
        report,
        is_in_domain,
        ustr_points,
        &source_levels,
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

fn values_for_layout<T: Copy + Default>(
    name: &str,
    source_lineages: &[T],
    expected_len: usize,
) -> io::Result<Option<Vec<T>>> {
    if source_lineages.is_empty() {
        Ok(None)
    } else if source_lineages.len() == expected_len {
        Ok(Some(source_lineages.to_vec()))
    } else if source_lineages.len() + 1 == expected_len {
        let mut lineages = Vec::with_capacity(expected_len);
        lineages.push(T::default());
        lineages.extend_from_slice(source_lineages);
        Ok(Some(lineages))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} length {} must equal mask-postproc layout length {expected_len} (or be exactly one shorter when a placeholder row is inserted)",
                source_lineages.len()
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

fn compact_center_values<T: Copy + Default>(
    source_lineages: &[T],
    is_in_domain: &[i32],
    ustr_points: usize,
    output_len: usize,
) -> Option<Vec<T>> {
    if source_lineages.len() < ustr_points || is_in_domain.len() < ustr_points {
        return None;
    }
    let mut lineages = vec![T::default(); output_len];
    if output_len > 1 && source_lineages.len() > 1 {
        lineages[1] = source_lineages[1];
    }
    let mut compact_center_id = 1usize;
    for source_center_id in 2..ustr_points {
        if is_in_domain[source_center_id] != 1 {
            continue;
        }
        compact_center_id += 1;
        if compact_center_id >= lineages.len() {
            return None;
        }
        lineages[compact_center_id] = source_lineages[source_center_id];
    }
    Some(lineages)
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

fn compact_vertex_values<T: Copy + Default>(
    source_lineages: &[T],
    sorted_vertices: &[usize],
    output_len: usize,
) -> Option<Vec<T>> {
    if sorted_vertices
        .iter()
        .any(|&source_vertex_id| source_vertex_id >= source_lineages.len())
    {
        return None;
    }
    let mut lineages = vec![T::default(); output_len];
    for (offset, &source_vertex_id) in sorted_vertices.iter().enumerate() {
        let final_vertex_id = offset + 1;
        if final_vertex_id >= lineages.len() {
            return None;
        }
        lineages[final_vertex_id] = source_lineages[source_vertex_id];
    }
    Some(lineages)
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

    #[test]
    fn layout_lineages_preserve_tokens_and_insert_only_placeholder() {
        assert_eq!(
            values_for_layout("lineages", &[1, 101], 3).unwrap(),
            Some(vec![0, 1, 101])
        );
        assert_eq!(
            values_for_layout("lineages", &[0, 1, 101], 3).unwrap(),
            Some(vec![0, 1, 101])
        );
        assert!(values_for_layout("lineages", &[1, 2, 3, 4], 2).is_err());
    }
    #[test]
    fn mpas_widths_follow_tri_hex_compaction_and_split_rows_without_renormalizing() {
        use crate::mpas_gridfile_context::MpasGridfileContext;
        use crate::{LonLatPoint, MaskPostprocLayout};
        for mode in ["tri", "hex"] {
            let corners = if mode == "tri" { 3 } else { 5 };
            let mut points = vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 2];
            for center in [110.0, 130.0] {
                for j in 0..corners {
                    let a = j as f64 * std::f64::consts::TAU / corners as f64;
                    points.push(LonLatPoint {
                        lon: center + a.cos(),
                        lat: 20.0 + a.sin(),
                    });
                }
            }
            let layout = MaskPostprocLayout {
                ustr_points: 4,
                ustr_bounds: points.len(),
                center_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 4],
                vertex_points: points,
                center_neighbors: vec![
                    vec![1],
                    vec![1],
                    (2..2 + corners).collect(),
                    (2 + corners..2 + 2 * corners).collect(),
                ],
                center_neighbor_counts: vec![1, 1, corners, corners],
                vertex_neighbors: (0..2 + 2 * corners)
                    .map(|i| {
                        vec![if i < 2 {
                            1
                        } else if i < 2 + corners {
                            2
                        } else {
                            3
                        }]
                    })
                    .collect(),
                vertex_neighbor_counts: vec![1; 2 + 2 * corners],
            };
            let selected = [0, 0, 1, -1];
            let report =
                crate::finalize_mask_postproc_layout_with_reindex_report(&layout, &selected, mode)
                    .unwrap();
            for placeholders in [1, 2] {
                let physical = if mode == "hex" {
                    vec![200.0, 50.0]
                } else {
                    (0..2 * corners)
                        .map(|i| if i < corners { 100.0 + i as f64 } else { 50.0 })
                        .collect()
                };
                let mut widths = vec![900.0; placeholders];
                widths.extend(physical);
                let context = MpasGridfileContext {
                    cellwidth_km: widths,
                    base_nxp: 12,
                    step: 3,
                    density_reference_width_km: 50.0,
                    source: "test_producer".into(),
                };
                let mut result = empty_final_metadata();
                result.mpas =
                    compact_mpas_context(mode, &report, &selected, 4, Some(&context)).unwrap();
                let delivered = result.mpas.as_ref().unwrap();
                assert_eq!(&delivered.cellwidth_km[..2], &[900.0, 900.0]);
                let expected = if mode == "hex" {
                    vec![200.0]
                } else {
                    (0..corners).map(|i| 100.0 + i as f64).collect()
                };
                assert_eq!(&delivered.cellwidth_km[2..], expected);
                assert_eq!(delivered.density_reference_width_km, 50.0);
                assert!(delivered.cellwidth_km.iter().all(|&w| w > 50.0));
                let original = delivered.cellwidth_km.clone();
                result.duplicate_w_vertices(&[2]).unwrap();
                let delivered = result.mpas.unwrap();
                assert_eq!(&delivered.cellwidth_km[..original.len()], original);
                assert_eq!(delivered.cellwidth_km.last(), Some(&original[2]));
                assert_eq!(delivered.density_reference_width_km, 50.0);
            }
        }
    }
}
