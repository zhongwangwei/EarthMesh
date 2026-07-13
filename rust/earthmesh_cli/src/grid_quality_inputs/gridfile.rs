use crate::convex_hull_order_indices;
use crate::gridfile_lonlat_has_two_placeholders;
use crate::mesh_row_for_canonical_id;
use crate::netcdf_to_io_error;
use crate::required_values_f64;
use crate::required_values_i32;
use crate::required_values_i32_matrix;
use crate::GridfileMeshPoints;
use std::collections::HashMap;
use std::io;
use std::path::Path;

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
            .filter_map(|&v| mesh_row_for_canonical_id(v, wn, w_has_two_placeholders))
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
    let mn = mesh.m_lon.len().min(mesh.m_lat.len());
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
            if let Some(w_row) = mesh_row_for_canonical_id(v, wn, w_has_two_placeholders) {
                incident[w_row].push(mi);
            }
        }
    }

    let mut cells = Vec::new();
    for (wi, incident_corners) in incident.iter().enumerate().take(wn) {
        if w_has_two_placeholders && wi < 2 {
            continue;
        }
        // `itab_w%im` + `n_ngrwm` is the gridfile's authoritative W-cell ring.
        // Prefer it over reconstructing incidence from M triangles, but retain the
        // inverse-connectivity path for legacy gridfiles that do not carry it.
        let corners =
            authoritative_w_corners(mesh, wi, mn, m_has_two_placeholders).unwrap_or_else(|| {
                let mut corners = incident_corners
                    .iter()
                    .copied()
                    .filter(|&mi| mi < mn && !(m_has_two_placeholders && mi < 2))
                    .collect::<Vec<_>>();
                corners.sort_unstable();
                corners.dedup();
                corners
            });
        if corners.len() < 3 {
            continue;
        }
        let ordered = order_corners_on_sphere(mesh, wi, corners);
        if ordered.len() >= 3 {
            cells.push((wi, ordered));
        }
    }
    cells
}

fn authoritative_w_corners(
    mesh: &GridfileMeshPoints,
    wi: usize,
    mn: usize,
    m_has_two_placeholders: bool,
) -> Option<Vec<usize>> {
    let width = mesh.w_to_m_width;
    if width == 0 || mesh.w_to_m.len() < mesh.w_lon.len().checked_mul(width)? {
        return None;
    }
    let count = usize::try_from(*mesh.n_w.get(wi)?).ok()?;
    if count > width {
        return None;
    }
    let start = wi.checked_mul(width)?;
    let row = mesh.w_to_m.get(start..start.checked_add(count)?)?;
    let mut corners = Vec::with_capacity(count);
    for &canonical_id in row {
        let corner = mesh_row_for_canonical_id(canonical_id, mn, m_has_two_placeholders)?;
        if corners.contains(&corner) {
            return None;
        }
        corners.push(corner);
    }
    Some(corners)
}

/// Sort W-cell corners by azimuth in the local spherical tangent plane centered
/// on the W point. Unlike a raw longitude-plane hull, this remains cyclic across
/// the antimeridian and near the poles without discarding valid cells.
fn order_corners_on_sphere(
    mesh: &GridfileMeshPoints,
    wi: usize,
    mut corners: Vec<usize>,
) -> Vec<usize> {
    if !mesh.w_lon[wi].is_finite()
        || !mesh.w_lat[wi].is_finite()
        || corners
            .iter()
            .any(|&mi| !mesh.m_lon[mi].is_finite() || !mesh.m_lat[mi].is_finite())
    {
        return convex_hull_order_indices(
            corners
                .into_iter()
                .map(|mi| (mesh.m_lon[mi], mesh.m_lat[mi], mi))
                .collect(),
        );
    }
    let lon0 = mesh.w_lon[wi].to_radians();
    let lat0 = mesh.w_lat[wi].to_radians();
    let tangent = |mi: usize| {
        let lon = mesh.m_lon[mi].to_radians();
        let lat = mesh.m_lat[mi].to_radians();
        let dlon = lon - lon0;
        let east = lat.cos() * dlon.sin();
        let north = lat0.cos() * lat.sin() - lat0.sin() * lat.cos() * dlon.cos();
        (east, north)
    };
    corners.sort_by(|&a, &b| {
        let (east_a, north_a) = tangent(a);
        let (east_b, north_b) = tangent(b);
        north_a
            .atan2(east_a)
            .partial_cmp(&north_b.atan2(east_b))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.cmp(&b))
    });
    let signed_twice_area = (0..corners.len())
        .map(|k| {
            let (x0, y0) = tangent(corners[k]);
            let (x1, y1) = tangent(corners[(k + 1) % corners.len()]);
            x0 * y1 - y0 * x1
        })
        .sum::<f64>();
    if signed_twice_area < 0.0 {
        corners.reverse();
    }
    corners
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
    let m_refine_level_orig =
        optional_values_i32_exact(&file, "earthmesh_m_refine_level_orig", m_lon.len())?;
    let m_ngr = optional_values_i32_exact(&file, "earthmesh_m_ngr", m_lon.len())?;
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
    let w_refine_level_orig =
        optional_values_i32_exact(&file, "earthmesh_w_refine_level_orig", w_lon.len())?;
    let w_ngr = optional_values_i32_exact(&file, "earthmesh_w_ngr", w_lon.len())?;
    Ok(GridfileMeshPoints {
        m_lon,
        m_lat,
        w_lon,
        w_lat,
        m_to_w,
        m_refine_level,
        m_refine_level_orig,
        m_ngr,
        w_to_m,
        w_to_m_width,
        n_w,
        w_refine_level,
        w_refine_level_orig,
        w_ngr,
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
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
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
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
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
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
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
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: Vec::new(),
            w_to_m_width: 0,
            n_w: Vec::new(),
            w_refine_level: vec![3, 2, 1, 0],
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let tri = quality_input_from_gridfile(&mesh);
        assert_eq!(tri.cells.len(), 4);
        assert_eq!(tri.cells[1].refine_level, Some(1));

        let hex = quality_input_from_gridfile_hex(&mesh);
        assert!(hex.cells.iter().any(|cell| cell.refine_level == Some(2)));
    }

    #[test]
    fn hex_quality_keeps_antimeridian_cell_and_closed_sphere_topology_from_w_ring() {
        // Four dual triangular W cells form a closed tetrahedral sphere. Three
        // rings, including W row 0, cross the antimeridian in raw longitude
        // coordinates and were previously dropped by the `hi - lo > 180` guard.
        // Deliberately omit M->W so this also proves the authoritative W->M ring
        // is consumed rather than ignored.
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 179.0, -179.0, 180.0],
            m_lat: vec![-30.0, -5.0, 5.0, 30.0],
            w_lon: vec![180.0, -60.0, 60.0, 0.0],
            w_lat: vec![0.0, 20.0, 20.0, -60.0],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![2, 3, 4, 1, 3, 4, 1, 2, 4, 1, 2, 3],
            w_to_m_width: 3,
            n_w: vec![3, 3, 3, 3],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let quality = quality_input_from_gridfile_hex(&mesh);

        assert_eq!(quality.cells.len(), 4);
        assert!(quality.cells[0].vertices.contains(&1));
        assert!(quality.cells[0].vertices.contains(&2));
        assert!(quality.cells[0].vertices.contains(&3));
        assert!(quality.cells.iter().all(|cell| cell.neighbors.len() == 3));

        let mut edges = std::collections::HashSet::new();
        for cell in &quality.cells {
            for k in 0..cell.vertices.len() {
                let a = cell.vertices[k];
                let b = cell.vertices[(k + 1) % cell.vertices.len()];
                edges.insert(if a < b { (a, b) } else { (b, a) });
            }
        }
        assert_eq!(
            quality.vertices.len() + quality.cells.len() - edges.len(),
            2
        );
    }

    #[test]
    fn hex_quality_does_not_resurrect_single_placeholder_from_inverse_connectivity() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 1.0, 2.0],
            m_lat: vec![0.0, 1.0, 0.0],
            w_lon: vec![0.0],
            w_lat: vec![0.0],
            m_to_w: vec![1, 1, 1],
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 1, 1],
            w_to_m_width: 3,
            n_w: vec![1],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        assert!(hex_quality_cells_from_gridfile(&mesh).is_empty());
    }
}
