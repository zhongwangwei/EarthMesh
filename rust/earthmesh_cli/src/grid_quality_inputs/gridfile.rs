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
    quality_input_from_gridfile_hex_with_source_rows(mesh).map(|(input, _)| input)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HexDelaunayRowCounts {
    pub placeholder_rows: usize,
    pub interior_triangle_rows: usize,
    pub boundary_dual_rows: usize,
}

/// Build the interior Delaunay-triangle view of a hex gridfile.
///
/// Open hex grids retain boundary dual vertices as physical M rows with one or
/// two distinct W references. They are polygon corners, not triangle cells.
/// The authoritative W rings distinguish them from interior M rows without
/// weakening the strict triangle-product reader.
pub fn quality_input_from_gridfile_hex_delaunay_interior(
    mesh: &GridfileMeshPoints,
) -> io::Result<(earthmesh_quality::QualityMeshInput, HexDelaunayRowCounts)> {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
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
    let mut reference_count = vec![0usize; mn];
    for wi in 0..wn {
        if !w_layout.is_physical_row(wi) {
            continue;
        }
        let corners = authoritative_w_corners(mesh, wi, mn, m_layout)?.ok_or_else(|| {
            invalid("hex Delaunay classification requires authoritative W connectivity")
        })?;
        for mi in corners {
            reference_count[mi] += 1;
        }
    }

    let vertices = mesh
        .w_lon
        .iter()
        .zip(&mesh.w_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect::<Vec<_>>();
    let mut cells = Vec::new();
    let mut counts = HexDelaunayRowCounts {
        placeholder_rows: 0,
        interior_triangle_rows: 0,
        boundary_dual_rows: 0,
    };
    for (mi, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
        if !m_layout.is_physical_row(mi) {
            counts.placeholder_rows += 1;
            continue;
        }
        let mut mapped = Vec::with_capacity(3);
        let mut distinct = Vec::with_capacity(3);
        for &id in tri {
            let row = w_layout
                .physical_row_for_canonical_id(id, wn)
                .ok_or_else(|| invalid(format!("M row {mi} contains invalid W vertex id {id}")))?;
            mapped.push(row);
            if !distinct.contains(&row) {
                distinct.push(row);
            }
        }
        match (distinct.len(), reference_count[mi]) {
            (3, 3) => {
                counts.interior_triangle_rows += 1;
                cells.push(QualityCell {
                    vertices: mapped,
                    refine_level: refine_level_at(&mesh.m_refine_level, mi),
                    neighbors: Vec::new(),
                });
            }
            (1, 1) | (2, 2) => counts.boundary_dual_rows += 1,
            (distinct_vertices, references) => {
                return Err(invalid(format!(
                    "M row {mi} has {distinct_vertices} distinct W vertices but is referenced by {references} W cells"
                )));
            }
        }
    }
    if cells.is_empty() {
        return Err(invalid(
            "hex Delaunay quality input contains no interior triangles",
        ));
    }
    derive_shared_edge_neighbors(&mut cells);
    Ok((QualityMeshInput { vertices, cells }, counts))
}

/// Build HEX quality input and retain each cell's source W row.
pub fn quality_input_from_gridfile_hex_with_source_rows(
    mesh: &GridfileMeshPoints,
) -> io::Result<(earthmesh_quality::QualityMeshInput, Vec<usize>)> {
    use earthmesh_geometry::Point;
    use earthmesh_quality::{QualityCell, QualityMeshInput};
    validate_coordinate_pairs(mesh)?;
    let vertices: Vec<Point> = mesh
        .m_lon
        .iter()
        .zip(&mesh.m_lat)
        .map(|(&lon, &lat)| Point::new(lon, lat))
        .collect();
    let source_cells = hex_quality_cells_from_gridfile(mesh)?;
    let source_rows = source_cells.iter().map(|(wi, _)| *wi).collect();
    let cells = source_cells
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
    Ok((QualityMeshInput { vertices, cells }, source_rows))
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
    for (mi, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
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
    for (mi, tri) in mesh.m_to_w.chunks_exact(3).enumerate() {
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
        let corners = authoritative_w_corners(mesh, wi, mn, m_layout)?;
        let corner_count = corners
            .as_ref()
            .map_or_else(|| incident_corners.len(), Vec::len);
        if corner_count < 3 {
            return Err(invalid(format!(
                "W cell row {wi} has only {} valid M corners",
                corner_count
            )));
        }
        let ordered = corners.unwrap_or_else(|| {
            let mut corners = incident_corners
                .iter()
                .copied()
                .filter(|&mi| mi < mn && m_layout.is_physical_row(mi))
                .collect::<Vec<_>>();
            corners.sort_unstable();
            corners.dedup();
            order_corners_on_sphere(mesh, wi, corners)
        });
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

    fn open_hex_dual_mesh() -> GridfileMeshPoints {
        GridfileMeshPoints {
            m_lon: vec![0.0, 0.0, 10.0, 10.2, 9.8, 10.4, 10.1, 9.9, 10.3],
            m_lat: vec![0.0, 0.0, 20.0, 20.2, 19.8, 20.4, 20.1, 19.9, 20.3],
            w_lon: vec![0.0, 0.0, 10.0, 11.0, 11.0, 10.0],
            w_lat: vec![0.0, 0.0, 20.0, 20.0, 21.0, 21.0],
            m_to_w: vec![
                1, 1, 1, // placeholders
                1, 1, 1, //
                2, 3, 4, // interior triangles
                2, 4, 5, //
                2, 2, 2, // boundary dual vertices
                3, 3, 3, //
                3, 4, 3, //
                4, 5, 4, //
                5, 5, 5, //
            ],
            m_refine_level: vec![0; 9],
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![
                1, 1, 1, 1, // placeholders
                1, 1, 1, 1, //
                2, 3, 4, 1, // authoritative W rings
                2, 5, 6, 1, //
                2, 3, 6, 7, //
                3, 7, 8, 1, //
            ],
            w_to_m_width: 4,
            n_w: vec![1, 1, 3, 3, 4, 3],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        }
    }

    #[test]
    fn hex_delaunay_classifies_open_boundary_dual_rows_without_weakening_tri_quality() {
        let mesh = open_hex_dual_mesh();

        let strict_tri_error = quality_input_from_gridfile(&mesh).unwrap_err();
        assert!(strict_tri_error
            .to_string()
            .contains("duplicate W vertex ids"));

        let (input, counts) = quality_input_from_gridfile_hex_delaunay_interior(&mesh).unwrap();
        assert_eq!(input.cells.len(), 2);
        assert_eq!(
            counts,
            HexDelaunayRowCounts {
                placeholder_rows: 2,
                interior_triangle_rows: 2,
                boundary_dual_rows: 5,
            }
        );
    }

    #[test]
    fn hex_delaunay_rejects_row_whose_shape_and_reverse_degree_disagree() {
        let mut mesh = open_hex_dual_mesh();
        mesh.w_to_m[4 * mesh.w_to_m_width..5 * mesh.w_to_m_width].copy_from_slice(&[2, 3, 7, 1]);
        mesh.n_w[4] = 3;

        let error = quality_input_from_gridfile_hex_delaunay_interior(&mesh).unwrap_err();
        assert!(error
            .to_string()
            .contains("M row 6 has 2 distinct W vertices but is referenced by 1 W cells"));
    }

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
        let report =
            earthmesh_quality::compute(&quality, &earthmesh_quality::QualityThresholds::default());
        assert_eq!(report.topology.boundary_loop_count, 0);
        assert_eq!(report.topology.expected_euler_characteristic, Some(2));
        assert_eq!(report.topology.euler_characteristic_mismatch_count, 0);
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
            w_to_m: vec![1, 0, 0, 0, 0, 2, 3, 4, 5, 6],
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

        let (quality, source_rows) =
            quality_input_from_gridfile_hex_with_source_rows(&mesh).unwrap();
        assert_eq!(source_rows, vec![1]);
        assert_eq!(quality.cells.len(), 1);
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
    fn hex_quality_preserves_authoritative_w_ring_order() {
        let mesh = GridfileMeshPoints {
            m_lon: vec![0.0, -1.0, 1.0, 1.0, -1.0],
            m_lat: vec![0.0, -1.0, -1.0, 1.0, 1.0],
            w_lon: vec![0.0, 0.0],
            w_lat: vec![0.0, 0.0],
            m_to_w: Vec::new(),
            m_refine_level: Vec::new(),
            m_refine_level_orig: Vec::new(),
            m_ngr: Vec::new(),
            w_to_m: vec![1, 1, 1, 1, 2, 4, 3, 5],
            w_to_m_width: 4,
            n_w: vec![1, 4],
            w_refine_level: Vec::new(),
            w_refine_level_orig: Vec::new(),
            w_ngr: Vec::new(),
        };

        let cells = hex_quality_cells_from_gridfile(&mesh).unwrap();

        assert_eq!(cells[0].1, vec![1, 3, 2, 4]);
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
}
