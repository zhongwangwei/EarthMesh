//! Exact whole-cell binding to an explicit parent; no proximity or filename inference.
use crate::grid_quality_inputs::{
    quality_input_from_gridfile_hex_native, read_gridfile_cell_lineages,
};
use crate::{gridfile_m_row_layout, gridfile_w_row_layout, GridfileMeshPoints};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

// Refinement siblings share an ancestor. Exact site + ancestry binds a row;
// neither ancestry alone nor a nearest-neighbour coordinate search can do so.
fn lineage_site(id: i64, lon: f64, lat: f64) -> io::Result<(i64, u64, u64)> {
    if id <= 0 || !lon.is_finite() || !lat.is_finite() {
        return Err(invalid(
            "whole-cell identity requires positive ancestry and finite coordinates",
        ));
    }
    let bits = |value: f64| if value == 0.0 { 0 } else { value.to_bits() };
    Ok((id, bits(lon), bits(lat)))
}

/// Return parent MPAS cell indices in final native W order. Native placeholder
/// layouts and lineage values may differ from canonical connectivity indices.
pub(crate) fn verify_whole_cell_lineage(
    source: &Path,
    output: &Path,
    grid: &GridfileMeshPoints,
) -> io::Result<Vec<usize>> {
    let original = crate::read_gridfile_mesh_points(source)?;
    quality_input_from_gridfile_hex_native(&original)?;
    quality_input_from_gridfile_hex_native(grid)?;
    let lineage = read_gridfile_cell_lineages(output)?;
    let parent_lineage = read_gridfile_cell_lineages(source)?;
    let m_layout = gridfile_m_row_layout(grid);
    let w_layout = gridfile_w_row_layout(grid);
    let source_m = gridfile_m_row_layout(&original);
    let source_w = gridfile_w_row_layout(&original);
    let mut matched = Vec::new();
    for (
        lon,
        lat,
        levels,
        ids,
        layout,
        source_lon,
        source_lat,
        source_levels,
        source_ids,
        source_layout,
    ) in [
        (
            &grid.m_lon,
            &grid.m_lat,
            &grid.m_refine_level,
            &lineage.m,
            m_layout,
            &original.m_lon,
            &original.m_lat,
            &original.m_refine_level,
            &parent_lineage.m,
            source_m,
        ),
        (
            &grid.w_lon,
            &grid.w_lat,
            &grid.w_refine_level,
            &lineage.w,
            w_layout,
            &original.w_lon,
            &original.w_lat,
            &original.w_refine_level,
            &parent_lineage.w,
            source_w,
        ),
    ] {
        if ids.len() != lon.len() || source_ids.len() != source_lon.len() {
            return Err(invalid(
                "whole-cell delivery requires complete M/W parent lineage",
            ));
        }
        let has_levels = !levels.is_empty() || !source_levels.is_empty();
        if has_levels && (levels.len() != lon.len() || source_levels.len() != source_lon.len()) {
            return Err(invalid(
                "whole-cell delivery requires matching complete refinement levels",
            ));
        }
        let mut parent_rows = HashMap::new();
        for row in source_layout.first_physical_row..source_lon.len() {
            let key = lineage_site(source_ids[row], source_lon[row], source_lat[row])?;
            if parent_rows.insert(key, row).is_some() {
                return Err(invalid(
                    "parent M/W ancestry and exact coordinates are ambiguous",
                ));
            }
        }
        let mut used = HashSet::new();
        let mut rows = Vec::new();
        for row in layout.first_physical_row..lon.len() {
            let key = lineage_site(ids[row], lon[row], lat[row])?;
            let &parent = parent_rows
                .get(&key)
                .ok_or_else(|| invalid("whole-cell lineage points outside explicit parent"))?;
            if !used.insert(parent) || (has_levels && levels[row] != source_levels[parent]) {
                return Err(invalid(
                    "whole-cell delivery changed parent coordinates, levels or identity",
                ));
            }
            rows.push(parent);
        }
        matched.push(rows);
    }
    for (index, &parent_row) in matched[1].iter().enumerate() {
        let row = w_layout.first_physical_row + index;
        let count = grid.n_w[row] as usize; // native adapters validated count/indices
        if original.n_w[parent_row] != grid.n_w[row] {
            return Err(invalid(
                "whole-cell delivery changed a parent's corner count",
            ));
        }
        let source_start = parent_row * original.w_to_m_width;
        let parent_ring = original.w_to_m[source_start..source_start + count]
            .iter()
            .map(|&id| {
                let row = source_m
                    .physical_row_for_canonical_id(id, original.m_lon.len())
                    .ok_or_else(|| invalid("parent corner index is invalid"))?;
                Ok(row as i64)
            })
            .collect::<io::Result<Vec<_>>>()?;
        let ring = grid.w_to_m[row * grid.w_to_m_width..row * grid.w_to_m_width + count]
            .iter()
            .map(|&id| {
                let row = m_layout
                    .physical_row_for_canonical_id(id, grid.m_lon.len())
                    .ok_or_else(|| invalid("selected corner index is invalid"))?;
                Ok(matched[0][row - m_layout.first_physical_row] as i64)
            })
            .collect::<io::Result<Vec<_>>>()?;
        if !same_cycle(&ring, &parent_ring) {
            return Err(invalid(
                "whole-cell delivery changed a parent's cyclic boundary",
            ));
        }
    }
    Ok(matched
        .pop()
        .unwrap()
        .into_iter()
        .map(|row| row - source_w.first_physical_row + 1)
        .collect())
}

pub(crate) fn same_cycle(a: &[i64], b: &[i64]) -> bool {
    if a.is_empty() || a.len() != b.len() {
        return false;
    }
    let Some(start) = b.iter().position(|&id| id == a[0]) else {
        return false;
    };
    // Reversal is the established outward-winding normalization, not a geometry edit.
    (0..a.len()).all(|i| a[i] == b[(start + i) % b.len()])
        || (0..a.len()).all(|i| a[i] == b[(start + b.len() - i) % b.len()])
}

#[cfg(test)]
mod tests {
    use super::lineage_site;

    #[test]
    fn exact_lineage_site_preserves_zero_equivalence_and_rejects_invalid_identity() {
        assert_eq!(
            lineage_site(1, -0.0, 0.0).unwrap(),
            lineage_site(1, 0.0, -0.0).unwrap()
        );
        assert_ne!(
            lineage_site(1, 10.0, 20.0).unwrap(),
            lineage_site(1, 10.0, 20.000000000001).unwrap()
        );
        for (id, lon, lat) in [
            (0, 0.0, 0.0),
            (-1, 0.0, 0.0),
            (1, f64::NAN, 0.0),
            (1, 0.0, f64::INFINITY),
        ] {
            assert!(lineage_site(id, lon, lat).is_err());
        }
    }
}
