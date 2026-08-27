use crate::gridfile_m_row_layout;
use crate::gridfile_w_row_layout;
use crate::netcdf_to_io_error;
use crate::required_dimension_len;
use crate::required_values_f64;
use crate::required_values_i32;
use crate::required_values_i32_matrix;
use crate::GridfileMeshPoints;
use crate::GridfileRowLayout;
use std::collections::HashMap;
use std::io;
use std::path::Path;

/// Build an engine-agnostic [`earthmesh_quality::QualityMeshInput`] from a gridfile's
/// triangle (M->W) view.
pub fn quality_input_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> io::Result<earthmesh_quality::QualityMeshInput> {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
    validate_coordinate_pairs(mesh)?;
    let vertices: Vec<Point> = mesh
        .w_lon
        .iter()
        .zip(&mesh.w_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect();
    let cells = tri_quality_cells_from_gridfile(mesh)?
        .into_iter()
        .map(|(mi, vertices)| QualityCell {
            vertices,
            refine_level: refine_level_at(&mesh.m_refine_level, mi),
            neighbors: Vec::new(),
        })
        .collect::<Vec<_>>();
    if cells.is_empty() {
        return Err(invalid("triangle quality input contains no physical cells"));
    }
    let mut cells = cells;
    derive_shared_edge_neighbors(&mut cells);
    Ok(QualityMeshInput { vertices, cells })
}

/// Build quality input from the HEXAGON (W-cell) view.
pub fn quality_input_from_gridfile_hex(
    mesh: &GridfileMeshPoints,
) -> io::Result<earthmesh_quality::QualityMeshInput> {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
    validate_coordinate_pairs(mesh)?;
    let vertices: Vec<Point> = mesh
        .m_lon
        .iter()
        .zip(&mesh.m_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect();
    let cells = hex_quality_cells_from_gridfile(mesh)?
        .into_iter()
        .map(|(wi, vertices)| QualityCell {
            vertices,
            refine_level: refine_level_at(&mesh.w_refine_level, wi),
            neighbors: Vec::new(),
        })
        .collect::<Vec<_>>();
    if cells.is_empty() {
        return Err(invalid("hex quality input contains no physical cells"));
    }
    let mut cells = cells;
    derive_shared_edge_neighbors(&mut cells);
    Ok(QualityMeshInput { vertices, cells })
}

pub(crate) fn tri_quality_cells_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> io::Result<Vec<(usize, Vec<usize>)>> {
    validate_coordinate_pairs(mesh)?;
    let wn = mesh.w_lon.len();
    let mn = mesh.m_lon.len();
    let expected = mn
        .checked_mul(3)
        .ok_or_else(|| invalid("M connectivity size overflow"))?;
    if mesh.m_to_w.len() != expected {
        return Err(invalid(format!(
            "M coordinate rows {mn} require {expected} triangle connectivity values, found {}",
            mesh.m_to_w.len()
        )));
    }
    let m_layout = gridfile_m_row_layout(mesh);
    let w_layout = gridfile_w_row_layout(mesh);
    let mut cells = Vec::new();
    for (mi, tri) in mesh.m_to_w.as_chunks::<3>().0.iter().enumerate() {
        if !m_layout.is_physical_row(mi) {
            continue;
        }
        let idx = tri
            .iter()
            .map(|&id| {
                w_layout
                    .physical_row_for_canonical_id(id, wn)
                    .ok_or_else(|| {
                        invalid(format!("M cell row {mi} contains invalid W vertex id {id}"))
                    })
            })
            .collect::<io::Result<Vec<_>>>()?;
        if idx[0] == idx[1] || idx[1] == idx[2] || idx[0] == idx[2] {
            return Err(invalid(format!(
                "M cell row {mi} contains duplicate W vertex ids {tri:?}"
            )));
        }
        cells.push((mi, idx));
    }
    Ok(cells)
}

pub(crate) fn hex_quality_cells_from_gridfile(
    mesh: &GridfileMeshPoints,
) -> io::Result<Vec<(usize, Vec<usize>)>> {
    validate_coordinate_pairs(mesh)?;
    let mn = mesh.m_lon.len();
    let wn = mesh.w_lon.len();
    let m_layout = gridfile_m_row_layout(mesh);
    let w_layout = gridfile_w_row_layout(mesh);
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); wn];
    if !mesh.m_to_w.is_empty() {
        let expected = mn
            .checked_mul(3)
            .ok_or_else(|| invalid("M connectivity size overflow"))?;
        if mesh.m_to_w.len() != expected {
            return Err(invalid(format!(
                "M coordinate rows {mn} require {expected} triangle connectivity values, found {}",
                mesh.m_to_w.len()
            )));
        }
    }
    for (mi, tri) in mesh.m_to_w.as_chunks::<3>().0.iter().enumerate() {
        if !m_layout.is_physical_row(mi) {
            continue;
        }
        for &v in tri {
            let w_row = w_layout
                .physical_row_for_canonical_id(v, wn)
                .ok_or_else(|| {
                    invalid(format!("M cell row {mi} contains invalid W vertex id {v}"))
                })?;
            incident[w_row].push(mi);
        }
    }

    let mut cells = Vec::new();
    for (wi, incident_corners) in incident.iter().enumerate().take(wn) {
        if !w_layout.is_physical_row(wi) {
            continue;
        }
        // `itab_w%im` + `n_ngrwm` is the gridfile's authoritative W-cell ring.
        // Prefer it over reconstructing incidence from M triangles, but retain the
        // inverse-connectivity path for legacy gridfiles that do not carry it.
        let corners = authoritative_w_corners(mesh, wi, mn, m_layout)?.unwrap_or_else(|| {
            let mut corners = incident_corners
                .iter()
                .copied()
                .filter(|&mi| mi < mn && m_layout.is_physical_row(mi))
                .collect::<Vec<_>>();
            corners.sort_unstable();
            corners.dedup();
            corners
        });
        if corners.len() < 3 {
            return Err(invalid(format!(
                "W cell row {wi} has only {} valid M corners",
                corners.len()
            )));
        }
        let ordered = order_corners_on_sphere(mesh, wi, corners);
        if ordered.len() < 3 {
            return Err(invalid(format!(
                "W cell row {wi} cannot form a valid polygon"
            )));
        }
        cells.push((wi, ordered));
    }
    Ok(cells)
}

fn authoritative_w_corners(
    mesh: &GridfileMeshPoints,
    wi: usize,
    mn: usize,
    m_layout: GridfileRowLayout,
) -> io::Result<Option<Vec<usize>>> {
    let width = mesh.w_to_m_width;
    if width == 0 && mesh.w_to_m.is_empty() && mesh.n_w.is_empty() {
        return Ok(None);
    }
    if width == 0 {
        return Err(invalid("authoritative W connectivity has zero row width"));
    }
    let expected = mesh
        .w_lon
        .len()
        .checked_mul(width)
        .ok_or_else(|| invalid("W connectivity size overflow"))?;
    if mesh.w_to_m.len() != expected || mesh.n_w.len() != mesh.w_lon.len() {
        return Err(invalid(format!(
            "authoritative W connectivity requires {expected} values and {} counts, found {} values and {} counts",
            mesh.w_lon.len(),
            mesh.w_to_m.len(),
            mesh.n_w.len()
        )));
    }
    let count = usize::try_from(mesh.n_w[wi]).map_err(|_| {
        invalid(format!(
            "W cell row {wi} has negative corner count {}",
            mesh.n_w[wi]
        ))
    })?;
    if !(3..=width).contains(&count) {
        return Err(invalid(format!(
            "W cell row {wi} corner count {count} must be between 3 and {width}"
        )));
    }
    let start = wi
        .checked_mul(width)
        .ok_or_else(|| invalid("W connectivity row offset overflow"))?;
    let row = &mesh.w_to_m[start..start + count];
    let mut corners = Vec::with_capacity(count);
    for &canonical_id in row {
        let corner = m_layout
            .physical_row_for_canonical_id(canonical_id, mn)
            .ok_or_else(|| {
                invalid(format!(
                    "W cell row {wi} contains invalid M corner id {canonical_id}"
                ))
            })?;
        if corners.contains(&corner) {
            return Err(invalid(format!(
                "W cell row {wi} contains duplicate M corner id {canonical_id}"
            )));
        }
        corners.push(corner);
    }
    Ok(Some(corners))
}

fn validate_coordinate_pairs(mesh: &GridfileMeshPoints) -> io::Result<()> {
    if mesh.m_lon.len() != mesh.m_lat.len() {
        return Err(invalid(format!(
            "M longitude/latitude lengths differ: {} vs {}",
            mesh.m_lon.len(),
            mesh.m_lat.len()
        )));
    }
    if mesh.w_lon.len() != mesh.w_lat.len() {
        return Err(invalid(format!(
            "W longitude/latitude lengths differ: {} vs {}",
            mesh.w_lon.len(),
            mesh.w_lat.len()
        )));
    }
    for (kind, lon, lat) in [
        ("M", mesh.m_lon.as_slice(), mesh.m_lat.as_slice()),
        ("W", mesh.w_lon.as_slice(), mesh.w_lat.as_slice()),
    ] {
        for (row, (&lon, &lat)) in lon.iter().zip(lat).enumerate() {
            if !lon.is_finite() || !lat.is_finite() {
                return Err(invalid(format!(
                    "{kind} coordinate row {row} must be finite"
                )));
            }
            if !(-90.0..=90.0).contains(&lat) {
                return Err(invalid(format!(
                    "{kind} coordinate row {row} latitude must be within [-90, 90] degrees"
                )));
            }
        }
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Sort W-cell corners by azimuth in the local spherical tangent plane centered
/// on the W point. Unlike a raw longitude-plane hull, this remains cyclic across
/// the antimeridian and near the poles without discarding valid cells.
fn order_corners_on_sphere(
    mesh: &GridfileMeshPoints,
    wi: usize,
    mut corners: Vec<usize>,
) -> Vec<usize> {
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
    if m_lon.len() != m_lat.len() {
        return Err(invalid(format!(
            "GLONM/GLATM lengths differ: {} vs {}",
            m_lon.len(),
            m_lat.len()
        )));
    }
    if w_lon.len() != w_lat.len() {
        return Err(invalid(format!(
            "GLONW/GLATW lengths differ: {} vs {}",
            w_lon.len(),
            w_lat.len()
        )));
    }
    let m_to_w =
        required_values_i32_matrix(&file, "itab_m%iw", "sjx_points", "dimb", m_lon.len(), 3)?;
    let m_refine_level = optional_values_i32_exact(&file, "earthmesh_m_refine_level", m_lon.len())?;
    let m_refine_level_orig =
        optional_values_i32_exact(&file, "earthmesh_m_refine_level_orig", m_lon.len())?;
    let m_ngr = optional_values_i32_exact(&file, "earthmesh_m_ngr", m_lon.len())?;
    let has_w_to_m = file.variable("itab_w%im").is_some();
    let has_n_w = file.variable("n_ngrwm").is_some();
    if has_w_to_m != has_n_w {
        return Err(invalid(
            "gridfile must provide both itab_w%im and n_ngrwm, or neither",
        ));
    }
    let w_to_m_width = file.dimension("dimc").map(|d| d.len()).unwrap_or(0);
    let (w_to_m, n_w) = if has_w_to_m {
        if w_to_m_width == 0 {
            return Err(invalid("gridfile itab_w%im requires a positive dimc"));
        }
        let im = required_values_i32_matrix(
            &file,
            "itab_w%im",
            "lbx_points",
            "dimc",
            w_lon.len(),
            w_to_m_width,
        )?;
        let n = required_values_i32(&file, "n_ngrwm")?;
        if n.len() != w_lon.len() {
            return Err(invalid(format!(
                "n_ngrwm length {} must equal W coordinate rows {}",
                n.len(),
                w_lon.len()
            )));
        }
        (im, n)
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

pub fn read_gridfile_cell_lineages(
    path: impl AsRef<Path>,
) -> io::Result<crate::MethodCGridfileLineages> {
    let file = crate::open_netcdf(path.as_ref()).map_err(netcdf_to_io_error)?;
    let m_rows = required_dimension_len(&file, "sjx_points")?;
    let w_rows = required_dimension_len(&file, "lbx_points")?;
    Ok(crate::MethodCGridfileLineages {
        m: optional_values_i64_exact(&file, "earthmesh_m_lineage", m_rows)?,
        w: optional_values_i64_exact(&file, "earthmesh_w_lineage", w_rows)?,
    })
}

fn optional_values_i64_exact(
    file: &netcdf::File,
    name: &str,
    expected_len: usize,
) -> io::Result<Vec<i64>> {
    let Some(variable) = file.variable(name) else {
        return Ok(Vec::new());
    };
    let values = variable
        .get_values::<i64, _>(..)
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

        let error = quality_input_from_gridfile(&mesh).unwrap_err();

        assert!(error.to_string().contains("M coordinate rows"));
    }

    #[test]
    fn tri_quality_rejects_invalid_connectivity_instead_of_dropping_the_cell() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0],
            m_lat: vec![0.0],
            w_lon: vec![0.0, 1.0, 0.0],
            w_lat: vec![0.0, 0.0, 1.0],
            m_to_w: vec![1, 2, 99],
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

        let error = quality_input_from_gridfile(&mesh).unwrap_err();
        assert!(error.to_string().contains("invalid W vertex id 99"));
    }

    #[test]
    fn tri_quality_skips_single_compact_sentinel_row() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 10.33],
            m_lat: vec![0.0, 20.33],
            w_lon: vec![0.0, 10.0, 11.0, 10.0],
            w_lat: vec![0.0, 20.0, 20.0, 21.0],
            m_to_w: vec![1, 1, 1, 2, 3, 4],
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

        let cells = tri_quality_cells_from_gridfile(&mesh).unwrap();

        assert_eq!(cells, vec![(1, vec![1, 2, 3])]);
    }

    #[test]
    fn tri_quality_keeps_first_physical_w_vertex_at_origin_after_single_sentinel() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 0.33],
            m_lat: vec![0.0, 0.33],
            w_lon: vec![0.0, 0.0, 1.0, 0.0],
            w_lat: vec![0.0, 0.0, 0.0, 1.0],
            m_to_w: vec![1, 1, 1, 2, 3, 4],
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

        let cells = tri_quality_cells_from_gridfile(&mesh).unwrap();

        assert_eq!(cells, vec![(1, vec![1, 2, 3])]);
    }

    #[test]
    fn hex_quality_rejects_malformed_authoritative_ring_instead_of_falling_back() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 1.0, 0.0],
            m_lat: vec![0.0, 0.0, 1.0],
            w_lon: vec![0.25],
            w_lat: vec![0.25],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 2, 99],
            w_to_m_width: 3,
            n_w: vec![3],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let error = quality_input_from_gridfile_hex(&mesh).unwrap_err();
        assert!(error.to_string().contains("invalid M corner id 99"));
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

        let cells = tri_quality_cells_from_gridfile(&mesh).unwrap();

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

        let cells = hex_quality_cells_from_gridfile(&mesh).unwrap();

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

        let tri = quality_input_from_gridfile(&mesh).unwrap();
        assert_eq!(tri.cells.len(), 4);
        assert_eq!(tri.cells[1].refine_level, Some(1));

        let hex = quality_input_from_gridfile_hex(&mesh).unwrap();
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

        let quality = quality_input_from_gridfile_hex(&mesh).unwrap();

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
    fn hex_quality_skips_single_compact_sentinel_and_keeps_physical_polar_cell() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 0.0, 72.0, 144.0, -144.0, -72.0],
            m_lat: vec![0.0, -88.0, -88.0, -88.0, -88.0, -88.0],
            w_lon: vec![0.0, 0.0],
            w_lat: vec![0.0, -90.0],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 1, 1, 1, 1, 2, 3, 4, 5, 6],
            w_to_m_width: 5,
            n_w: vec![1, 5],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let cells = hex_quality_cells_from_gridfile(&mesh).unwrap();

        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].0, 1);
        let mut corners = cells[0].1.clone();
        corners.sort_unstable();
        assert_eq!(corners, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn hex_quality_keeps_first_physical_w_cell_at_origin_after_single_sentinel() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, -1.0, 1.0, 1.0, -1.0],
            m_lat: vec![0.0, -1.0, -1.0, 1.0, 1.0],
            w_lon: vec![0.0, 0.0],
            w_lat: vec![0.0, 0.0],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 1, 1, 1, 2, 3, 4, 5],
            w_to_m_width: 4,
            n_w: vec![1, 4],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let cells = hex_quality_cells_from_gridfile(&mesh).unwrap();

        assert_eq!(cells.len(), 1);
        assert_eq!(cells[0].0, 1);
    }

    #[test]
    fn hex_quality_keeps_first_physical_m_corner_at_origin_without_m_triangles() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 0.0, 1.0, 1.0, 0.0],
            m_lat: vec![0.0, 0.0, 0.0, 1.0, 1.0],
            w_lon: vec![0.0, 0.5],
            w_lat: vec![0.0, 0.5],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 1, 1, 1, 2, 3, 4, 5],
            w_to_m_width: 4,
            n_w: vec![1, 4],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let cells = hex_quality_cells_from_gridfile(&mesh).unwrap();

        let mut corners = cells[0].1.clone();
        corners.sort_unstable();
        assert_eq!(corners, vec![1, 2, 3, 4]);
    }

    #[test]
    fn hex_quality_rejects_non_sentinel_origin_cell_with_one_corner() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, 1.0, 0.0],
            m_lat: vec![0.0, 0.0, 1.0],
            w_lon: vec![0.0],
            w_lat: vec![0.0],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 2, 3],
            w_to_m_width: 3,
            n_w: vec![1],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let error = hex_quality_cells_from_gridfile(&mesh).unwrap_err();

        assert!(error
            .to_string()
            .contains("corner count 1 must be between 3 and 3"));
    }

    #[test]
    #[ignore = "requires EARTHMESH_REAL_IGBP_NXP80_GRIDFILE"]
    fn real_igbp_nxp80_mesh_certification() {
        use std::collections::BTreeMap;

        use earthmesh_mesh::{
            lonlat_degrees_to_unit_xyz, CartesianPoint, LonLatDegrees, MeshState,
        };
        use earthmesh_refine_harp_dv::{certify_mesh, AdaptiveMesh};

        let Some(path) =
            std::env::var_os("EARTHMESH_REAL_IGBP_NXP80_GRIDFILE").map(std::path::PathBuf::from)
        else {
            eprintln!("skipped: set EARTHMESH_REAL_IGBP_NXP80_GRIDFILE");
            return;
        };
        let source = read_gridfile_mesh_points(&path).expect("read real IGBP NXP80 gridfile");
        let w_layout = gridfile_w_row_layout(&source);
        let mut vertex_for_w_row = vec![None; source.w_lon.len()];
        let mut w_row_for_vertex = vec![None, None];
        let mut vertices = vec![CartesianPoint::new(0.0, 0.0, 0.0); 2];
        for row in 0..source.w_lon.len() {
            if !w_layout.is_physical_row(row) {
                continue;
            }
            vertex_for_w_row[row] = Some(vertices.len());
            w_row_for_vertex.push(Some(row));
            vertices.push(lonlat_degrees_to_unit_xyz(LonLatDegrees::new(
                source.w_lon[row],
                source.w_lat[row],
            )));
        }
        let mut triangles = vec![[1usize; 3]; 2];
        for (_, corners) in
            tri_quality_cells_from_gridfile(&source).expect("read real IGBP triangles")
        {
            let corners: [usize; 3] = corners.try_into().expect("triangles have three corners");
            triangles.push(corners.map(|row| {
                vertex_for_w_row[row].expect("triangle corner maps to a physical W row")
            }));
        }
        let state = MeshState::from_parts(vertices, triangles).unwrap_or_else(|errors| {
            panic!(
                "real IGBP gridfile must form a mesh: {}",
                errors
                    .iter()
                    .take(4)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        });
        let mesh = AdaptiveMesh::from_mesh_state(state).expect("adapt real IGBP mesh read-only");
        let mut report = certify_mesh(&mesh, &[]);
        // `from_mesh_state` marks every input site as inherited. Replace that
        // placeholder lineage with the gridfile metadata before attribution.
        for violation in &mut report.violations {
            let source_row = w_row_for_vertex[violation.corner_vertex]
                .expect("certified vertex maps back to a physical W row");
            violation.refinement_depth = source
                .w_refine_level
                .get(source_row)
                .and_then(|&depth| u16::try_from(depth).ok());
            violation.birth_cycle = None;
        }
        let refinement_level_histogram = report.violations.iter().fold(
            BTreeMap::<Option<u16>, usize>::new(),
            |mut histogram, violation| {
                *histogram.entry(violation.refinement_depth).or_default() += 1;
                histogram
            },
        );
        let min = report.min_angle_deg.expect("measurable minimum angle");
        let max = report.max_angle_deg.expect("measurable maximum angle");
        let violation_count = report.below_40_count + report.above_80_count;
        let violation_percent =
            100.0 * violation_count as f64 / report.measurable_angle_count as f64;

        eprintln!("fixture_kind=real");
        eprintln!("source_mesh=real_igbp_nxp80 path={}", path.display());
        eprintln!("criterion=igbp");
        eprintln!(
            "angles_deg min={min:.6} p1={:.6} p99={:.6} max={max:.6}",
            report.p1_angle_deg.expect("measurable p1"),
            report.p99_angle_deg.expect("measurable p99")
        );
        eprintln!(
            "violations below_40={} above_80={} total={} percent={violation_percent:.6}",
            report.below_40_count, report.above_80_count, violation_count
        );
        eprintln!("degree_histogram={:?}", report.degree_histogram);
        eprintln!(
            "degree_attribution le_4={} ge_5={}",
            report.violating_angles_at_degree_le_4, report.violating_angles_at_degree_ge_5
        );
        eprintln!(
            "topology euler={} charge={} open_edges={} errors={}",
            report.euler_characteristic,
            report.euler_degree_charge,
            report.open_edge_count,
            report.topology_error_count
        );
        eprintln!(
            "violation_refinement_depth_source=earthmesh_w_refine_level histogram={refinement_level_histogram:?}"
        );
        eprintln!("birth_cycle=unavailable_from_gridfile");
        eprintln!("target_scale_ratio=unavailable_without_criterion");

        assert!((min - 28.22).abs() < 0.01, "unexpected minimum angle {min}");
        assert!(
            (max - 108.11).abs() < 0.01,
            "unexpected maximum angle {max}"
        );
        assert_eq!(report.measurable_angle_count, 610_500);
        assert_eq!(report.below_40_count, 11_022);
        assert_eq!(report.above_80_count, 17_297);
        assert_eq!(report.violating_angles_at_degree_le_4, 402);
        assert_eq!(report.violating_angles_at_degree_ge_5, 27_917);
        assert!(report.violations.iter().all(|violation| {
            violation.refinement_depth.is_some()
                && violation.birth_cycle.is_none()
                && violation.realized_to_target_scale_ratio.is_none()
        }));
        assert_eq!(report.euler_characteristic, 2);
        assert_eq!(report.euler_degree_charge, 12);
        assert_eq!(report.open_edge_count, 0);
        assert_eq!(report.topology_error_count, 0);
        assert_eq!(report.degree_sum, report.twice_edge_count);
        assert_eq!(
            report.violating_angles_at_degree_le_4 + report.violating_angles_at_degree_ge_5,
            violation_count
        );
    }
}
