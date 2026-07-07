use std::io;
use std::path::Path;

use super::levels::{final_refine_levels_for_mask_postproc, refine_levels_from_gridfile};
use crate::*;

/// Carve a global gridfile down to `region` and write the regional gridfile, in
/// pure Rust. Reuses the engine's mask-postproc compaction/re-index: cells whose
/// centre is outside the region are dropped and the mesh is renumbered. The
/// gridfile's leading placeholder (Fortran id 1 / array index 0) is preserved.
/// Returns the number of cells kept. `mode_grid` selects the primal cells
/// (`hex` -> hexagons / W cells, `tri` -> triangles / M cells).
pub fn write_regional_gridfile(
    global_gridfile: impl AsRef<Path>,
    regional_gridfile: impl AsRef<Path>,
    region: &GridRegion,
    mode_grid: &str,
) -> io::Result<usize> {
    write_regional_gridfile_with_refine_levels(
        global_gridfile,
        regional_gridfile,
        region,
        mode_grid,
        None,
        None,
    )
}

pub fn write_regional_gridfile_with_refine_levels(
    global_gridfile: impl AsRef<Path>,
    regional_gridfile: impl AsRef<Path>,
    region: &GridRegion,
    mode_grid: &str,
    m_refine_level: Option<&[i32]>,
    w_refine_level: Option<&[i32]>,
) -> io::Result<usize> {
    let global_gridfile = global_gridfile.as_ref();
    let mesh = read_unstructured_mesh_netcdf(global_gridfile)?;
    let raw_layout = mask_postproc_layout_from_unstructured_mesh(&mesh, mode_grid)?;
    let layout = ensure_leading_mask_postproc_placeholder(raw_layout);
    let mut is_in_domain = vec![-1i32; layout.ustr_points];
    let mut kept = 0usize;
    if !is_in_domain.is_empty() {
        is_in_domain[0] = 0;
    }
    if is_in_domain.len() > 1 {
        is_in_domain[1] = 0;
    }
    for i in 2..layout.ustr_points {
        if regional_cell_inside(&layout, i, region, mode_grid) {
            is_in_domain[i] = 1;
            kept += 1;
        }
    }
    let report =
        finalize_mask_postproc_layout_with_reindex_report(&layout, &is_in_domain, mode_grid)?;
    let source_levels = if m_refine_level.is_none() || w_refine_level.is_none() {
        Some(refine_levels_from_gridfile(global_gridfile)?)
    } else {
        None
    };
    let source_m_levels = m_refine_level
        .or_else(|| source_levels.as_ref().map(|levels| levels.m.as_slice()))
        .unwrap_or(&[]);
    let source_w_levels = w_refine_level
        .or_else(|| source_levels.as_ref().map(|levels| levels.w.as_slice()))
        .unwrap_or(&[]);
    let final_levels = final_refine_levels_for_mask_postproc(
        mode_grid,
        &report,
        &is_in_domain,
        layout.ustr_points,
        source_m_levels,
        source_w_levels,
    )?;
    write_unstructured_mesh_netcdf_with_refine_levels(
        regional_gridfile,
        &report.mesh,
        final_levels.m.as_deref(),
        final_levels.w.as_deref(),
    )?;
    Ok(kept)
}

fn regional_cell_inside(
    layout: &MaskPostprocLayout,
    cell: usize,
    region: &GridRegion,
    mode_grid: &str,
) -> bool {
    let Some(center) = layout.center_points.get(cell) else {
        return false;
    };
    if !region.contains(center.lon, center.lat) {
        return false;
    }
    if mode_grid.trim() != "tri" {
        return true;
    }
    let Some(vertices) = layout.center_neighbors.get(cell) else {
        return false;
    };
    vertices.iter().all(|&vertex_id| {
        mesh_row_for_fortran_id(vertex_id as i32, layout.vertex_points.len(), true)
            .and_then(|row| layout.vertex_points.get(row))
            .is_some_and(|point| region.contains(point.lon, point.lat))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close_region() -> GridRegion {
        GridRegion::Close {
            points: vec![
                LonLatPoint {
                    lon: 100.0,
                    lat: 10.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 10.0,
                },
                LonLatPoint {
                    lon: 130.0,
                    lat: 40.0,
                },
                LonLatPoint {
                    lon: 100.0,
                    lat: 40.0,
                },
            ],
        }
    }

    fn layout_with_vertices(vertices: [(f64, f64); 3]) -> MaskPostprocLayout {
        let mut vertex_points = vec![
            LonLatPoint { lon: 0.0, lat: 0.0 },
            LonLatPoint { lon: 0.0, lat: 0.0 },
        ];
        vertex_points.extend(
            vertices
                .into_iter()
                .map(|(lon, lat)| LonLatPoint { lon, lat }),
        );
        MaskPostprocLayout {
            ustr_points: 3,
            ustr_bounds: vertex_points.len(),
            center_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint {
                    lon: 115.0,
                    lat: 25.0,
                },
            ],
            vertex_points,
            center_neighbors: vec![vec![], vec![], vec![2, 3, 4]],
            vertex_neighbors: vec![],
            center_neighbor_counts: vec![0, 0, 3],
            vertex_neighbor_counts: vec![],
        }
    }

    #[test]
    fn tri_regional_clip_rejects_far_vertices_even_when_center_is_inside() {
        let layout = layout_with_vertices([(110.0, 20.0), (120.0, 20.0), (-70.0, 20.0)]);
        assert!(!regional_cell_inside(&layout, 2, &close_region(), "tri"));
    }

    #[test]
    fn tri_regional_clip_keeps_cells_with_center_and_vertices_inside() {
        let layout = layout_with_vertices([(110.0, 20.0), (120.0, 20.0), (115.0, 30.0)]);
        assert!(regional_cell_inside(&layout, 2, &close_region(), "tri"));
    }
}
