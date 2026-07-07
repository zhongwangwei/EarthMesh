use std::collections::HashMap;
use std::io;
use std::path::Path;

use crate::*;

/// Build an engine-agnostic [`earthmesh_quality::QualityMeshInput`] from a gridfile's
/// triangle (M->W) view.
pub fn quality_input_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> earthmesh_quality::QualityMeshInput {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
    let vertices: Vec<Point> = mesh
        .w_lon
        .iter()
        .zip(&mesh.w_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect();
    let cells = tri_quality_cells_from_gridfile(mesh)
        .into_iter()
        .map(|(mi, vertices)| QualityCell {
            vertices,
            refine_level: refine_level_at(&mesh.m_refine_level, mi),
            neighbors: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut cells = cells;
    derive_shared_edge_neighbors(&mut cells);
    QualityMeshInput { vertices, cells }
}

/// Build quality input from the HEXAGON (W-cell) view.
pub fn quality_input_from_gridfile_hex(
    mesh: &GridfileMeshPoints,
) -> earthmesh_quality::QualityMeshInput {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
    let vertices: Vec<Point> = mesh
        .m_lon
        .iter()
        .zip(&mesh.m_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect();
    let cells = hex_quality_cells_from_gridfile(mesh)
        .into_iter()
        .map(|(wi, vertices)| QualityCell {
            vertices,
            refine_level: refine_level_at(&mesh.w_refine_level, wi),
            neighbors: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut cells = cells;
    derive_shared_edge_neighbors(&mut cells);
    QualityMeshInput { vertices, cells }
}

pub(crate) fn tri_quality_cells_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> Vec<(usize, Vec<usize>)> {
    let wn = mesh.w_lon.len();
    let w_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.w_lon, &mesh.w_lat);
    let mut cells = Vec::new();
    for (mi, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
        let idx: Vec<usize> = tri
            .iter()
            .filter_map(|&v| mesh_row_for_fortran_id(v, wn, w_has_two_placeholders))
            .collect();
        if idx.len() == 3 && idx[0] != idx[1] && idx[1] != idx[2] && idx[0] != idx[2] {
            cells.push((mi, idx));
        }
    }
    cells
}

pub(crate) fn hex_quality_cells_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> Vec<(usize, Vec<usize>)> {
    let mn = mesh.m_lon.len();
    let wn = mesh.w_lon.len();
    let m_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.m_lon, &mesh.m_lat);
    let w_has_two_placeholders = gridfile_lonlat_has_two_placeholders(&mesh.w_lon, &mesh.w_lat);
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); wn];
    for (mi, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
        if mi >= mn {
            break;
        }
        if m_has_two_placeholders && mi < 2 {
            continue;
        }
        for &v in tri {
            if let Some(w_row) = mesh_row_for_fortran_id(v, wn, w_has_two_placeholders) {
                incident[w_row].push(mi);
            }
        }
    }

    let mut cells = Vec::new();
    for (wi, corners_for_w) in incident.iter().enumerate().take(wn) {
        if w_has_two_placeholders && wi < 2 {
            continue;
        }
        let corners: Vec<usize> = corners_for_w
            .iter()
            .copied()
            .filter(|&mi| mi < mn && !(m_has_two_placeholders && mi < 2))
            .collect();
        if corners.len() < 3 {
            continue;
        }
        let (lo, hi) = corners.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &mi| {
            (lo.min(mesh.m_lon[mi]), hi.max(mesh.m_lon[mi]))
        });
        if hi - lo > 180.0 {
            continue;
        }
        let pts: Vec<(f64, f64, usize)> = corners
            .iter()
            .map(|&mi| (mesh.m_lon[mi], mesh.m_lat[mi], mi))
            .collect();
        let ordered = convex_hull_order_indices(pts);
        if ordered.len() >= 3 {
            cells.push((wi, ordered));
        }
    }
    cells
}

fn refine_level_at(levels: &[i32], index: usize) -> Option<u32> {
    levels
        .get(index)
        .copied()
        .filter(|level| *level >= 0)
        .map(|level| level as u32)
}

fn derive_shared_edge_neighbors(cells: &mut [earthmesh_quality::QualityCell]) {
    let mut edge_to_cells: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (ci, cell) in cells.iter().enumerate() {
        let v = &cell.vertices;
        for k in 0..v.len() {
            let (a, b) = (v[k], v[(k + 1) % v.len()]);
            let key = if a < b { (a, b) } else { (b, a) };
            edge_to_cells.entry(key).or_default().push(ci);
        }
    }
    for shared in edge_to_cells.values() {
        if shared.len() == 2 {
            let (x, y) = (shared[0], shared[1]);
            cells[x].neighbors.push(y);
            cells[y].neighbors.push(x);
        }
    }
    for cell in cells {
        cell.neighbors.sort_unstable();
        cell.neighbors.dedup();
    }
}

/// Read the M-point (cell-centre) and W-point (vertex) lon/lat arrays plus the
/// triangle-to-vertex connectivity from an EarthMesh gridfile.
pub fn read_gridfile_mesh_points(path: impl AsRef<Path>) -> io::Result<GridfileMeshPoints> {
    let file = crate::open_netcdf(path.as_ref()).map_err(netcdf_to_io_error)?;
    let m_lon = required_values_f64(&file, "GLONM")?;
    let m_lat = required_values_f64(&file, "GLATM")?;
    let w_lon = required_values_f64(&file, "GLONW")?;
    let w_lat = required_values_f64(&file, "GLATW")?;
    let m_to_w =
        required_values_i32_matrix(&file, "itab_m%iw", "sjx_points", "dimb", m_lon.len(), 3)?;
    let m_refine_level = optional_values_i32_exact(&file, "earthmesh_m_refine_level", m_lon.len())?;
    let w_to_m_width = file.dimension("dimc").map(|d| d.len()).unwrap_or(0);
    let (w_to_m, n_w) = if w_to_m_width > 0 {
        let im = required_values_i32_matrix(
            &file,
            "itab_w%im",
            "lbx_points",
            "dimc",
            w_lon.len(),
            w_to_m_width,
        )
        .unwrap_or_default();
        let n = required_values_i32(&file, "n_ngrwm").unwrap_or_default();
        if im.len() == w_lon.len() * w_to_m_width && n.len() == w_lon.len() {
            (im, n)
        } else {
            (Vec::new(), Vec::new())
        }
    } else {
        (Vec::new(), Vec::new())
    };
    let w_to_m_width = if w_to_m.is_empty() { 0 } else { w_to_m_width };
    let w_refine_level = optional_values_i32_exact(&file, "earthmesh_w_refine_level", w_lon.len())?;
    Ok(GridfileMeshPoints {
        m_lon,
        m_lat,
        w_lon,
        w_lat,
        m_to_w,
        m_refine_level,
        w_to_m,
        w_to_m_width,
        n_w,
        w_refine_level,
    })
}

fn optional_values_i32_exact(
    file: &netcdf::File,
    name: &str,
    expected_len: usize,
) -> io::Result<Vec<i32>> {
    let Some(variable) = file.variable(name) else {
        return Ok(Vec::new());
    };
    let values = variable
        .get_values::<i32, _>(..)
        .map_err(netcdf_to_io_error)?;
    if values.len() == expected_len {
        Ok(values)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} length {} must equal {expected_len}", values.len()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tri_quality_skips_cells_with_placeholder_zero_vertex() {
        let mesh = GridfileMeshPoints {
            m_lon: Vec::new(),
            m_lat: Vec::new(),
            w_lon: vec![0.0, 0.0, 110.0, 120.0, 115.0, 0.0],
            w_lat: vec![0.0, 0.0, 20.0, 20.0, 30.0, 0.0],
            m_to_w: vec![2, 3, 4, 2, 4, 1],
            m_refine_level: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
        };

        let quality = quality_input_from_gridfile(&mesh);

        assert_eq!(quality.cells.len(), 1);
        assert_eq!(quality.cells[0].vertices, vec![2, 3, 4]);
    }

    #[test]
    fn tri_quality_keeps_valid_zero_zero_vertex_by_row_identity() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![10.0],
            m_lat: vec![10.0],
            w_lon: vec![0.0, 0.0, 0.0, 1.0, 0.0],
            w_lat: vec![0.0, 0.0, 0.0, 0.0, 1.0],
            m_to_w: vec![2, 3, 4],
            m_refine_level: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
        };

        let cells = tri_quality_cells_from_gridfile(&mesh);

        assert_eq!(cells, vec![(0, vec![2, 3, 4])]);
    }

    #[test]
    fn hex_quality_keeps_valid_zero_zero_center_and_corner_by_row_identity() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 0.0, 0.0, 1.0, 0.0],
            m_lat: vec![0.0, 0.0, 0.0, 0.0, 1.0],
            w_lon: vec![0.0, 2.0, 3.0],
            w_lat: vec![0.0, 0.0, 1.0],
            m_to_w: vec![
                1, 2, 3, // placeholder M row 0; ignored by two-placeholder row identity
                1, 2, 3, // placeholder M row 1; ignored by two-placeholder row identity
                1, 2, 3, // real corner at (0,0)
                1, 2, 3, 1, 2, 3,
            ],
            m_refine_level: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
        };

        let cells = hex_quality_cells_from_gridfile(&mesh);

        assert!(cells
            .iter()
            .any(|(wi, corners)| *wi == 0 && corners.contains(&2)));
    }

    #[test]
    fn tri_and_hex_quality_copy_optional_refine_levels() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.1, 1.0, 0.1, 1.0],
            m_lat: vec![0.1, 0.1, 1.0, 1.0],
            w_lon: vec![0.5, 1.0, 1.0, 0.5],
            w_lat: vec![0.5, 0.5, 1.0, 1.0],
            m_to_w: vec![1, 2, 3, 1, 3, 4, 1, 2, 4, 2, 3, 4],
            m_refine_level: vec![0, 1, 2, 3],
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: vec![3, 2, 1, 0],
        };

        let tri = quality_input_from_gridfile(&mesh);
        assert_eq!(tri.cells.len(), 4);
        assert_eq!(tri.cells[1].refine_level, Some(1));

        let hex = quality_input_from_gridfile_hex(&mesh);
        assert!(hex.cells.iter().any(|cell| cell.refine_level == Some(2)));
    }
}
