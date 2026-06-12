//! Rust mesh kernels migrated from EarthMesh Fortran.

use earthmesh_core::{deg_to_rad, rad_to_deg};

/// Earth-centered Cartesian point using the same axis convention as `mkgrd.F90`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CartesianPoint {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Longitude/latitude pair in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatDegrees {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
}

impl LonLatDegrees {
    pub const fn new(lon_degrees: f64, lat_degrees: f64) -> Self {
        Self {
            lon_degrees,
            lat_degrees,
        }
    }
}

/// Port of `MOD_grid_preprocess:lonlat2xyz` for a single unit-sphere point.
///
/// The Fortran routine intentionally returns unit vectors; callers multiply by
/// `erad8` when Earth-radius-scaled coordinates are required.
pub fn lonlat_degrees_to_unit_xyz(lonlat: LonLatDegrees) -> CartesianPoint {
    let lon_rad = deg_to_rad(lonlat.lon_degrees);
    let lat_rad = deg_to_rad(lonlat.lat_degrees);
    CartesianPoint::new(
        lat_rad.cos() * lon_rad.cos(),
        lat_rad.cos() * lon_rad.sin(),
        lat_rad.sin(),
    )
}

/// Batch port of `MOD_grid_preprocess:lonlat2xyz`, preserving input order.
pub fn lonlat_points_to_unit_xyz(points: &[LonLatDegrees]) -> Vec<CartesianPoint> {
    points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect()
}

/// Convert Earth-centered Cartesian coordinates to lon/lat degrees.
///
/// This ports the scalar formula used by `mkgrd.F90:grid_xyz2lonlat`:
///
/// - `raxis = sqrt(x ** 2 + y ** 2)`
/// - `lat = atan2(z, raxis) * piu180`
/// - `lon = atan2(y, x) * piu180`
#[inline]
pub fn xyz_to_lonlat_degrees(point: CartesianPoint) -> LonLatDegrees {
    let raxis = point.x.hypot(point.y);
    LonLatDegrees {
        lon_degrees: rad_to_deg(point.y.atan2(point.x)),
        lat_degrees: rad_to_deg(point.z.atan2(raxis)),
    }
}

/// Convert a slice of Earth-centered Cartesian points to lon/lat degrees while
/// preserving point order.
pub fn xyz_points_to_lonlat_degrees(points: &[CartesianPoint]) -> Vec<LonLatDegrees> {
    points.iter().copied().map(xyz_to_lonlat_degrees).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_conversion_preserves_order() {
        let points = [
            CartesianPoint::new(1.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 1.0, 0.0),
        ];
        let lonlat = xyz_points_to_lonlat_degrees(&points);
        assert_eq!(lonlat.len(), 2);
        assert_eq!(lonlat[0].lon_degrees, 0.0);
        assert_eq!(lonlat[1].lon_degrees, 90.0);
    }
}

/// Precomputed sine/cosine basis for an icosahedron polar stereographic pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleBasis {
    pub cos_lat: f64,
    pub sin_lat: f64,
    pub cos_lon: f64,
    pub sin_lon: f64,
}

impl PoleBasis {
    pub fn from_lonlat_radians(lon_radians: f64, lat_radians: f64) -> Self {
        Self {
            cos_lat: lat_radians.cos(),
            sin_lat: lat_radians.sin(),
            cos_lon: lon_radians.cos(),
            sin_lon: lon_radians.sin(),
        }
    }
}

/// Point on the polar stereographic plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePoint {
    pub x: f64,
    pub y: f64,
}

impl PlanePoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Port of `icosahedron.F90:de_ps_r8`.
pub fn project_to_polar_stereographic(point: CartesianPoint, pole: PoleBasis) -> PlanePoint {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq =
        pole.cos_lat * point.z - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq =
        pole.sin_lat * point.z + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePoint::new(xq * t, yq * t)
}

/// Port of `icosahedron.F90:ps_de_r8`.
pub fn unproject_from_polar_stereographic(point: PlanePoint, pole: PoleBasis) -> CartesianPoint {
    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let earth_diameter_sq = earth_diameter * earth_diameter;
    let t = earth_diameter_sq / (point.x * point.x + point.y * point.y + earth_diameter_sq);

    let xq = point.x * t;
    let yq = point.y * t;
    let zq = earth_diameter * (t - 1.0);

    CartesianPoint::new(
        -pole.sin_lon * xq + pole.cos_lon * (-pole.sin_lat * yq + pole.cos_lat * zq),
        pole.cos_lon * xq - pole.sin_lon * (pole.sin_lat * yq - pole.cos_lat * zq),
        pole.cos_lat * yq + pole.sin_lat * zq,
    )
}

/// Port of `MOD_grid_preprocess:centroid_spherical_single`.
///
/// Converts lon/lat vertices to unit Cartesian vectors, averages components,
/// then converts the averaged vector back to lon/lat degrees.
pub fn spherical_centroid_degrees(points: &[LonLatDegrees]) -> Option<LonLatDegrees> {
    if points.is_empty() {
        return None;
    }

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sz = 0.0;
    for point in points {
        let xyz = lonlat_degrees_to_unit_xyz(*point);
        sx += xyz.x;
        sy += xyz.y;
        sz += xyz.z;
    }
    let n = points.len() as f64;
    let centroid = CartesianPoint::new(sx / n, sy / n, sz / n);
    Some(xyz_to_lonlat_degrees(centroid))
}

/// Batch port of `MOD_grid_preprocess:centroid_spherical_calculation`.
///
/// Preserves the Fortran workflow where triangle ids start at `2`; slots `0`
/// and `1` remain initialized to `(0, 0)` just like an unwritten `mp` scratch
/// array in the migrated Rust call boundary.
pub fn centroid_spherical_mesh_fortran_indexed(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<LonLatDegrees>> {
    let mut centroids = vec![LonLatDegrees::new(0.0, 0.0); cells_on_triangle.len()];

    for triangle_id in 2..cells_on_triangle.len() {
        let cell_ids = cells_on_triangle[triangle_id];
        let triangle_points = [
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ];
        centroids[triangle_id] = spherical_centroid_degrees(&triangle_points)?;
    }

    Some(centroids)
}

/// Port of one iteration of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// `barycenter` and `vertices` are Earth-radius-scaled Cartesian coordinates.
/// The algorithm mirrors the Fortran global-domain branch: build a local polar
/// stereographic plane at the spherical barycenter, solve the 2-D circumcenter,
/// unproject it back to an Earth displacement, then renormalize to the Earth
/// radius.
pub fn spherical_circumcenter_from_barycenter(
    barycenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> Option<CartesianPoint> {
    let earth_radius = earthmesh_core::EARTH_RADIUS_METERS;
    let raxis = barycenter.x.hypot(barycenter.y);
    if raxis == 0.0 {
        return None;
    }

    let pole = PoleBasis {
        cos_lat: raxis / earth_radius,
        sin_lat: barycenter.z / earth_radius,
        cos_lon: barycenter.x / raxis,
        sin_lon: barycenter.y / raxis,
    };

    let mut projected = [PlanePoint::new(0.0, 0.0); 3];
    for (slot, vertex) in projected.iter_mut().zip(vertices) {
        let displacement = CartesianPoint::new(
            vertex.x - barycenter.x,
            vertex.y - barycenter.y,
            vertex.z - barycenter.z,
        );
        *slot = project_to_polar_stereographic(displacement, pole);
    }

    let [p1, p2, p3] = projected;
    let dx12 = p2.x - p1.x;
    let dx13 = p3.x - p1.x;
    let dx23 = p3.x - p2.x;
    let s1 = p1.x * p1.x + p1.y * p1.y;
    let s2 = p2.x * p2.x + p2.y * p2.y;
    let s3 = p3.x * p3.x + p3.y * p3.y;

    let y_denom = dx13 * p2.y - dx12 * p3.y - dx23 * p1.y;
    if y_denom == 0.0 {
        return None;
    }
    let ycc = 0.5 * (dx13 * s2 - dx12 * s3 - dx23 * s1) / y_denom;

    let xcc = if dx12.abs() > dx13.abs() {
        if dx12 == 0.0 {
            return None;
        }
        (s2 - s1 - ycc * 2.0 * (p2.y - p1.y)) / (2.0 * dx12)
    } else {
        if dx13 == 0.0 {
            return None;
        }
        (s3 - s1 - ycc * 2.0 * (p3.y - p1.y)) / (2.0 * dx13)
    };

    let displacement = unproject_from_polar_stereographic(PlanePoint::new(xcc, ycc), pole);
    let mut circumcenter = CartesianPoint::new(
        displacement.x + barycenter.x,
        displacement.y + barycenter.y,
        displacement.z + barycenter.z,
    );

    let radius = magnitude(circumcenter);
    if radius == 0.0 {
        return None;
    }
    let expansion = earth_radius / radius;
    circumcenter.x *= expansion;
    circumcenter.y *= expansion;
    circumcenter.z *= expansion;
    Some(circumcenter)
}

/// Batch port of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// Returns a copy of the incoming M-point Cartesian centers with triangle ids
/// `2..len` replaced by spherical circumcenters, preserving the Fortran inout
/// behavior for slots not visited by the loop.
pub fn circumcenter_spherical_mesh_fortran_indexed(
    initial_centers: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<CartesianPoint>> {
    if cells_on_triangle.len() > initial_centers.len() {
        return None;
    }

    let mut centers = initial_centers.to_vec();
    for triangle_id in 2..cells_on_triangle.len() {
        let vertex_ids = cells_on_triangle[triangle_id];
        let vertices = [
            *vertex_points.get(vertex_ids[0])?,
            *vertex_points.get(vertex_ids[1])?,
            *vertex_points.get(vertex_ids[2])?,
        ];
        centers[triangle_id] =
            spherical_circumcenter_from_barycenter(centers[triangle_id], vertices)?;
    }

    Some(centers)
}

/// Result of `MOD_grid_preprocess:find_frac_index`.
///
/// `index` is intentionally 1-based to preserve the Fortran caller contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FortranFracIndex {
    pub index: usize,
    pub frac: f64,
}

/// Port of `MOD_grid_preprocess:find_frac_index` with explicit failure.
///
/// The Fortran subroutine supports monotonic ascending longitude grids and
/// monotonic descending latitude grids. The original error path is unreachable
/// after `return`; this Rust port returns `None` when the point is outside the
/// provided bounds or a zero-width cell is encountered.
pub fn find_frac_index_fortran(grid: &[f64], point: f64) -> Option<FortranFracIndex> {
    if grid.len() < 2 {
        return None;
    }

    let ascending = grid[0] < *grid.last()?;
    for i in 0..(grid.len() - 1) {
        let in_cell = if ascending {
            point >= grid[i] && point <= grid[i + 1]
        } else {
            point <= grid[i] && point >= grid[i + 1]
        };
        if !in_cell {
            continue;
        }

        let dx = grid[i + 1] - grid[i];
        if dx == 0.0 {
            return None;
        }
        let frac = ((point - grid[i]) / dx).clamp(0.0, 1.0);
        return Some(FortranFracIndex { index: i + 1, frac });
    }

    None
}

/// Rust representation of `refine_vars:set_dis_type` choices used by
/// `MOD_grid_preprocess:dist_layers_make`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceLayerSpacing {
    Linear,
    Power,
    Exponential,
    Logarithmic,
}

/// Port of `MOD_grid_preprocess:dist_layers_make`.
pub fn distance_layers(
    dist_len: usize,
    dist_select: f64,
    spacing: DistanceLayerSpacing,
) -> Option<Vec<f64>> {
    if dist_len == 0 {
        return None;
    }

    let mindist_select = dist_select / 2.0;
    let dist_len_f = dist_len as f64;
    let mut layers = Vec::with_capacity(dist_len);

    match spacing {
        DistanceLayerSpacing::Linear => {
            let a = mindist_select / dist_len_f;
            let b = mindist_select - a;
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0) + b);
            }
        }
        DistanceLayerSpacing::Power => {
            let a = mindist_select;
            let b = 2.0_f64.ln() / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).powf(b));
            }
        }
        DistanceLayerSpacing::Exponential => {
            let b = 2.0_f64.powf(1.0 / dist_len_f);
            let a = mindist_select / b;
            for i in 1..=dist_len {
                layers.push(a * b.powf(i as f64 + 1.0));
            }
        }
        DistanceLayerSpacing::Logarithmic => {
            let b = mindist_select;
            let a = b / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).ln() + b);
            }
        }
    }

    Some(layers)
}

fn boundary_cells_from_triangle_flags(
    num_center_in: usize,
    triangles_on_cell: &[Vec<usize>],
    triangle_flags: &[bool],
) -> Option<Vec<bool>> {
    let mut boundary = vec![false; triangles_on_cell.len()];
    for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
        let triangles = &triangles_on_cell[cell_id];
        if triangles.is_empty() {
            continue;
        }
        let mut flagged = 0usize;
        for &triangle_id in triangles {
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != triangles.len();
    }
    Some(boundary)
}

/// Port of the edge-length update rule in
/// `MOD_grid_preprocess:distsOnEdge_layers_make`.
///
/// The arrays preserve migrated Fortran indexing: slots `0` and `1` are
/// placeholders, triangle ids and edge ids are used directly, and the caller
/// provides `num_vertex_in`/`num_center_in` from `num_mp_step(iter)` and
/// `num_wp_step(iter)`.
pub fn dists_on_edge_layers_fortran_indexed(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    triangles_on_cell: &[Vec<usize>],
    edges_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_dists_on_edge: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < 2 * dist_len
        || refinement_flags.len() > edges_on_vertex.len()
        || initial_dists_on_edge.len() > cells_on_edge.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut dists_on_edge = initial_dists_on_edge.to_vec();
    let mut edge_moved = vec![false; initial_dists_on_edge.len()];
    let mindist00 = dist_layers[2 * dist_len - 1] / 2.0;

    for _ in 0..num_rc {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    triangle_flags[triangle_id] = false;
                }
            }
        }
    }

    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &edge_id in edges_on_vertex.get(triangle_id)? {
            if edge_id == 0 {
                continue;
            }
            *dists_on_edge.get_mut(edge_id)? = mindist00;
            *edge_moved.get_mut(edge_id)? = true;
        }
    }

    for layer_id in 0..=dist_len {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        if layer_id == dist_len {
            break;
        }

        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    continue;
                }
                *triangle_flags.get_mut(triangle_id)? = true;
                *triangle_in.get_mut(triangle_id)? = true;
            }
        }

        for triangle_id in 2..triangle_in.len() {
            if !triangle_in[triangle_id] {
                continue;
            }
            for &edge_id in edges_on_vertex.get(triangle_id)? {
                if edge_id == 0 || *edge_moved.get(edge_id)? {
                    continue;
                }
                let cells = *cells_on_edge.get(edge_id)?;
                let boundary_sum =
                    usize::from(*boundary.get(cells[0])?) + usize::from(*boundary.get(cells[1])?);
                let layer_index = if boundary_sum == 1 {
                    2 * layer_id
                } else {
                    2 * layer_id + 1
                };
                *dists_on_edge.get_mut(edge_id)? = *dist_layers.get(layer_index)?;
                *edge_moved.get_mut(edge_id)? = true;
            }
        }
        triangle_in.fill(false);
    }

    Some(dists_on_edge)
}

/// Port of the cell-width update rule in
/// `MOD_grid_preprocess:cellwidth_layers_make`.
///
/// `cells_on_triangle` corresponds to Fortran `ngrmw(:, i)` for triangle `i`,
/// while `triangles_on_cell` corresponds to `ngrwm(:, k)` for cell `k`.
pub fn cellwidth_layers_fortran_indexed(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_cellwidth: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < dist_len
        || refinement_flags.len() > cells_on_triangle.len()
        || initial_cellwidth.len() < triangles_on_cell.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut cellwidth = initial_cellwidth.to_vec();

    for _ in 0..num_rc {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    triangle_flags[triangle_id] = false;
                }
            }
        }
    }

    let inner_cellwidth = dist_layers[dist_len - 1] / 2.0;
    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &cell_id in cells_on_triangle.get(triangle_id)? {
            if cell_id == 0 {
                continue;
            }
            *cellwidth.get_mut(cell_id)? = inner_cellwidth;
        }
    }

    for layer_id in 0..=dist_len {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        if layer_id == dist_len {
            break;
        }

        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    continue;
                }
                *triangle_flags.get_mut(triangle_id)? = true;
                *triangle_in.get_mut(triangle_id)? = true;
            }
        }

        for triangle_id in 2..triangle_in.len() {
            if !triangle_in[triangle_id] {
                continue;
            }
            for &cell_id in cells_on_triangle.get(triangle_id)? {
                if cell_id == 0 || *boundary.get(cell_id)? {
                    continue;
                }
                *cellwidth.get_mut(cell_id)? = *dist_layers.get(layer_id)?;
            }
        }
        triangle_in.fill(false);
    }

    Some(cellwidth)
}

/// Port of `MOD_grid_preprocess:CheckLon`.
///
/// The Fortran routine performs a single +/-360 adjustment rather than a full
/// modulo normalization. Preserve that behavior for parity.
pub fn normalize_lon_m180_180(lon_degrees: f64) -> f64 {
    if lon_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

/// Port of the swap predicate in `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Fortran compares the 2-D cross product between the cell-center edge vector
/// and the current vertex-edge vector. If `res > 0`, ordering is kept; otherwise
/// `verticesOnEdge(1:2, i)` is swapped.
pub fn should_swap_vertices_on_edge(
    cell1: LonLatDegrees,
    cell2: LonLatDegrees,
    vertex1: LonLatDegrees,
    vertex2: LonLatDegrees,
) -> bool {
    let cell_delta_lon = normalize_lon_m180_180(cell2.lon_degrees - cell1.lon_degrees);
    let cell_delta_lat = cell2.lat_degrees - cell1.lat_degrees;
    let vertex_delta_lon = normalize_lon_m180_180(vertex2.lon_degrees - vertex1.lon_degrees);
    let vertex_delta_lat = vertex2.lat_degrees - vertex1.lat_degrees;

    let cross = cell_delta_lon * vertex_delta_lat - cell_delta_lat * vertex_delta_lon;
    cross <= 0.0
}

/// Port of `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Returns a sorted copy of `verticesOnEdge`, preserving the Fortran convention
/// that edge ids start at `2`. Each edge is swapped when the migrated
/// cross-product predicate indicates Fortran would exchange
/// `verticesOnEdge(1:2, i)`.
pub fn order_vertices_on_edge_fortran_indexed(
    point_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
) -> Option<Vec<[usize; 2]>> {
    if cells_on_edge.len() != vertices_on_edge.len() {
        return None;
    }

    let mut ordered = vertices_on_edge.to_vec();
    for edge_id in 2..vertices_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let vertices = ordered[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let cell2 = *cell_lonlat.get(cells[1])?;
        let vertex1 = *point_lonlat.get(vertices[0])?;
        let vertex2 = *point_lonlat.get(vertices[1])?;

        if should_swap_vertices_on_edge(cell1, cell2, vertex1, vertex2) {
            ordered[edge_id].swap(0, 1);
        }
    }

    Some(ordered)
}

/// Port of one-vertex rotation logic from `MOD_grid_preprocess:normalizeRotation`.
///
/// The minimum positive cell id is rotated into slot 0, and the edge slots are
/// rotated in lockstep. If no positive cell id exists, arrays are unchanged.
pub fn normalize_vertex_rotation(
    cells_on_vertex: [usize; 3],
    edges_on_vertex: [usize; 3],
) -> ([usize; 3], [usize; 3]) {
    let mut min_cell = cells_on_vertex[0];
    let mut min_pos = 0usize;

    for pos in 1..3 {
        let cell = cells_on_vertex[pos];
        if cell > 0 && (min_cell == 0 || cell < min_cell) {
            min_cell = cell;
            min_pos = pos;
        }
    }

    if min_pos == 1 && min_cell > 0 {
        (
            [cells_on_vertex[1], cells_on_vertex[2], cells_on_vertex[0]],
            [edges_on_vertex[1], edges_on_vertex[2], edges_on_vertex[0]],
        )
    } else if min_pos == 2 && min_cell > 0 {
        (
            [cells_on_vertex[2], cells_on_vertex[0], cells_on_vertex[1]],
            [edges_on_vertex[2], edges_on_vertex[0], edges_on_vertex[1]],
        )
    } else {
        (cells_on_vertex, edges_on_vertex)
    }
}

/// Port of `MOD_grid_preprocess:standardizeVerticesOnCellRotation`.
///
/// Cell ids preserve the migrated Fortran indexing convention: slot `1` is
/// skipped and valid cells are visited from id `2`. Only the first
/// `n_edges_on_cell[cell_id]` entries are rotated; any storage tail is kept in
/// place, matching Fortran's fixed-width `verticesOnCell(:, i)` arrays.
pub fn standardize_vertices_on_cell_rotation_fortran_indexed(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        return None;
    }

    let mut standardized = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if standardized[cell_id].len() < ne {
            return None;
        }

        let mut min_vertex_id = usize::MAX;
        let mut min_pos = 0usize;
        for pos in 0..ne {
            let vertex_id = standardized[cell_id][pos];
            if vertex_id > 0 && vertex_id < min_vertex_id {
                min_vertex_id = vertex_id;
                min_pos = pos;
            }
        }

        if min_vertex_id != usize::MAX && min_pos != 0 {
            let current = standardized[cell_id][0..ne].to_vec();
            let rotated = current[min_pos..]
                .iter()
                .chain(current[..min_pos].iter())
                .copied()
                .collect::<Vec<_>>();
            standardized[cell_id][0..ne].copy_from_slice(&rotated);
        }
    }

    Some(standardized)
}

/// Output of `MOD_grid_preprocess:Get_ConnectOnCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellConnectivityOnCell {
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
}

/// Port of `MOD_grid_preprocess:Get_ConnectOnCell`.
///
/// The input `vertices_on_cell` must already be ordered around each cell. For
/// each consecutive vertex pair, this finds the shared edge from the two
/// `edgesOnVertex` triplets, then maps that edge to the neighboring cell via
/// `cellsOnEdge`.
pub fn connect_on_cell_fortran_indexed(
    n_edges_on_cell: &[usize],
    cells_on_edge: &[[usize; 2]],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_cell: &[Vec<usize>],
) -> Option<CellConnectivityOnCell> {
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        return None;
    }

    let mut edges_on_cell = vec![Vec::new(); vertices_on_cell.len()];
    let mut cells_on_cell = vec![Vec::new(); vertices_on_cell.len()];

    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if vertices_on_cell[cell_id].len() < ne {
            return None;
        }

        let mut cell_edges = Vec::with_capacity(ne);
        let mut neighbor_cells = Vec::with_capacity(ne);
        for vertex_slot in 0..ne {
            let vertex1 = vertices_on_cell[cell_id][vertex_slot];
            let vertex2 = vertices_on_cell[cell_id][(vertex_slot + 1) % ne];
            let edges_vertex1 = *edges_on_vertex.get(vertex1)?;
            let edges_vertex2 = *edges_on_vertex.get(vertex2)?;
            let edge_id = edges_vertex1
                .iter()
                .copied()
                .find(|edge| *edge > 0 && edges_vertex2.contains(edge))?;
            let cells = *cells_on_edge.get(edge_id)?;
            let neighbor = if cells[0] == cell_id {
                cells[1]
            } else if cells[1] == cell_id {
                cells[0]
            } else {
                return None;
            };
            cell_edges.push(edge_id);
            neighbor_cells.push(neighbor);
        }
        edges_on_cell[cell_id] = cell_edges;
        cells_on_cell[cell_id] = neighbor_cells;
    }

    Some(CellConnectivityOnCell {
        edges_on_cell,
        cells_on_cell,
    })
}

/// Port of `MOD_grid_preprocess:orderVerticesOnCell`.
///
/// Preserves the Fortran selection-sort approach: for each fixed vertex slot,
/// choose the remaining vertex with positive `cross(vec1, vec2) · normal` and
/// the smallest angle to the current reference vector.
pub fn order_vertices_on_cell_fortran_indexed(
    cell_points: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() || cell_points.len() < vertices_on_cell.len()
    {
        return None;
    }

    let mut ordered = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if ordered[cell_id].len() < ne {
            return None;
        }

        let cell_center = *cell_points.get(cell_id)?;
        let normal_mag = magnitude(cell_center);
        if normal_mag == 0.0 {
            return None;
        }
        let normal = CartesianPoint::new(
            cell_center.x / normal_mag,
            cell_center.y / normal_mag,
            cell_center.z / normal_mag,
        );

        for slot in 0..(ne - 1) {
            let vertex1_id = ordered[cell_id][slot];
            if vertex1_id == 0 {
                continue;
            }
            let vertex1 = *vertex_points.get(vertex1_id)?;
            let vec1 = vector_between(cell_center, vertex1);
            let mag1 = magnitude(vec1);
            if mag1 == 0.0 {
                continue;
            }

            let mut min_angle = std::f64::consts::PI * 2.0;
            let mut swap_slot = None;
            for candidate_slot in (slot + 1)..ne {
                let vertex2_id = ordered[cell_id][candidate_slot];
                if vertex2_id == 0 {
                    continue;
                }
                let vertex2 = *vertex_points.get(vertex2_id)?;
                let vec2 = vector_between(cell_center, vertex2);
                let mag2 = magnitude(vec2);
                if mag2 == 0.0 {
                    continue;
                }

                let cross_product = cross(vec1, vec2);
                if dot(cross_product, normal) <= 0.0 {
                    continue;
                }
                let angle = (dot(vec1, vec2) / (mag1 * mag2)).clamp(-1.0, 1.0).acos();
                if angle < min_angle {
                    min_angle = angle;
                    swap_slot = Some(candidate_slot);
                }
            }

            if let Some(candidate_slot) = swap_slot {
                if candidate_slot != slot + 1 {
                    ordered[cell_id].swap(slot + 1, candidate_slot);
                }
            }
        }
    }

    Some(ordered)
}

/// Port of `MOD_grid_preprocess:planeAngle`.
pub fn plane_angle_signed(
    point_a: CartesianPoint,
    point_b: CartesianPoint,
    point_c: CartesianPoint,
    normal: CartesianPoint,
) -> Option<f64> {
    let ab = vector_between(point_a, point_b);
    let ac = vector_between(point_a, point_c);
    let mab = magnitude(ab);
    let mac = magnitude(ac);
    if mab == 0.0 || mac == 0.0 {
        return None;
    }

    let cos_angle = (dot(ab, ac) / (mab * mac)).clamp(-1.0, 1.0);
    let signed = if dot(cross(ab, ac), normal) >= 0.0 {
        cos_angle.acos()
    } else {
        -cos_angle.acos()
    };
    Some(signed)
}

/// Output of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDistanceAngleOutput {
    pub dc_edge: Vec<f64>,
    pub dv_edge: Vec<f64>,
    pub angle_edge: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
pub fn edge_distance_angle_fortran_indexed(
    vertices: &[CartesianPoint],
    cells: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    lat_vertex_degrees: &[f64],
    lon_edge_degrees: &[f64],
    lat_edge_degrees: &[f64],
) -> Option<EdgeDistanceAngleOutput> {
    if cells_on_edge.len() != vertices_on_edge.len()
        || edge_points.len() < vertices_on_edge.len()
        || lon_edge_degrees.len() < vertices_on_edge.len()
        || lat_edge_degrees.len() < vertices_on_edge.len()
    {
        return None;
    }

    let mut dc_edge = vec![0.0; vertices_on_edge.len()];
    let mut dv_edge = vec![0.0; vertices_on_edge.len()];
    let mut angle_edge = vec![0.0; vertices_on_edge.len()];
    let pi = std::f64::consts::PI;

    for edge_id in 2..vertices_on_edge.len() {
        let vertex_ids = vertices_on_edge[edge_id];
        let cell_ids = cells_on_edge[edge_id];
        let vertex1 = *vertices.get(vertex_ids[0])?;
        let vertex2 = *vertices.get(vertex_ids[1])?;
        let cell1 = *cells.get(cell_ids[0])?;
        let cell2 = *cells.get(cell_ids[1])?;

        dv_edge[edge_id] = arc_length_unit_sphere(vertex1, vertex2);
        dc_edge[edge_id] = arc_length_unit_sphere(cell1, cell2);
        if dv_edge[edge_id] == 0.0 {
            return None;
        }

        let mut angle = (deg_to_rad(*lat_vertex_degrees.get(vertex_ids[1])?)
            - deg_to_rad(*lat_vertex_degrees.get(vertex_ids[0])?))
            / dv_edge[edge_id];
        angle = angle.clamp(-1.0, 1.0).acos();

        let edge_point = *edge_points.get(edge_id)?;
        let lon_north = deg_to_rad(lon_edge_degrees[edge_id]);
        let lat_north = deg_to_rad(lat_edge_degrees[edge_id] + 0.05);
        let north_point = CartesianPoint::new(
            lat_north.cos() * lon_north.cos(),
            lat_north.cos() * lon_north.sin(),
            lat_north.sin(),
        );
        let mut sign = plane_angle_signed(edge_point, north_point, vertex2, edge_point)?;
        if sign.abs() > 1.0e-14 {
            sign /= sign.abs();
        } else {
            sign = 1.0;
        }

        angle *= sign;
        if angle > pi {
            angle -= 2.0 * pi;
        }
        if angle < -pi {
            angle += 2.0 * pi;
        }
        angle_edge[edge_id] = angle;
    }

    Some(EdgeDistanceAngleOutput {
        dc_edge,
        dv_edge,
        angle_edge,
    })
}

/// Output of `MOD_grid_preprocess:edgeIDSort`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeIdSortOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Port of `MOD_grid_preprocess:edgeIDSort`.
///
/// Edges from the current mesh are reordered to match
/// `cells_on_edge_reference`; `edges_on_vertex` is then rebuilt from the sorted
/// `vertices_on_edge` arrays.
pub fn edge_id_sort_fortran_indexed(
    num_vertices: usize,
    cells_on_edge_reference: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
    edge_points: &[LonLatDegrees],
) -> Option<EdgeIdSortOutput> {
    let num_edges = cells_on_edge_reference.len();
    if cells_on_edge.len() != num_edges
        || vertices_on_edge.len() != num_edges
        || edge_points.len() != num_edges
    {
        return None;
    }

    let mut sorted_cells_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_vertices_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_edge_points = vec![LonLatDegrees::new(0.0, 0.0); num_edges];

    for target_edge_id in 2..num_edges {
        let reference_cells = cells_on_edge_reference[target_edge_id];
        let source_edge_id = (2..num_edges).find(|&candidate| {
            cells_on_edge[candidate][0] == reference_cells[0]
                && cells_on_edge[candidate][1] == reference_cells[1]
        })?;
        sorted_cells_on_edge[target_edge_id] = cells_on_edge[source_edge_id];
        sorted_vertices_on_edge[target_edge_id] = vertices_on_edge[source_edge_id];
        sorted_edge_points[target_edge_id] = edge_points[source_edge_id];
    }

    let mut edges_on_vertex = vec![[0usize; 3]; num_vertices];
    let mut edge_counts = vec![0usize; num_vertices];
    for edge_id in 2..num_edges {
        for &vertex_id in &sorted_vertices_on_edge[edge_id] {
            if vertex_id == 0 {
                continue;
            }
            let count = edge_counts.get_mut(vertex_id)?;
            if *count >= 3 {
                return None;
            }
            edges_on_vertex.get_mut(vertex_id)?[*count] = edge_id;
            *count += 1;
        }
    }

    Some(EdgeIdSortOutput {
        cells_on_edge: sorted_cells_on_edge,
        vertices_on_edge: sorted_vertices_on_edge,
        edges_on_vertex,
        edge_points: sorted_edge_points,
    })
}

/// Output of `MOD_grid_preprocess:set_weightsOnEdge`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightsOnEdgeOutput {
    pub weights_on_edge: Vec<Vec<f64>>,
    pub edges_on_edge: Vec<Vec<usize>>,
    pub n_edges_on_edge: Vec<usize>,
    pub error_segment: Vec<f64>,
}

fn find_index_in_prefix(index: usize, indices: &[usize], n_indices: usize) -> Option<usize> {
    indices
        .iter()
        .take(n_indices)
        .position(|candidate| *candidate == index)
}

/// Port of `MOD_grid_preprocess:set_weightsOnEdge`.
///
/// The routine computes MPAS-compatible edge stencils and reconstruction
/// weights for Fortran-indexed mesh arrays. Weight rows are stored compactly per
/// edge rather than in a fixed `maxEdges2 x num_edge` matrix.
pub fn set_weights_on_edge_fortran_indexed(
    area_cell: &[f64],
    angle_edge: &[f64],
    dc_edge: &[f64],
    dv_edge: &[f64],
    kite_areas_on_vertex: &[[f64; 3]],
    edges_on_cell: &[Vec<usize>],
    cells_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_cell: &[Vec<usize>],
    vertices_on_edge: &[[usize; 2]],
    n_edges_on_cell: &[usize],
) -> Option<WeightsOnEdgeOutput> {
    let num_edges = cells_on_edge.len();
    if vertices_on_edge.len() != num_edges
        || angle_edge.len() < num_edges
        || dc_edge.len() < num_edges
        || dv_edge.len() < num_edges
    {
        return None;
    }

    let mut weights_on_edge = vec![Vec::new(); num_edges];
    let mut edges_on_edge = vec![Vec::new(); num_edges];
    let mut n_edges_on_edge = vec![0usize; num_edges];
    let mut error_segment = vec![0.0; num_edges];

    for edge_id in 2..num_edges {
        let [cell1, cell2] = cells_on_edge[edge_id];
        let edge_vertices = vertices_on_edge[edge_id];
        if cell1 == 0
            || cell2 == 0
            || edge_vertices[0] == 0
            || edge_vertices[1] == 0
            || cell1 >= n_edges_on_cell.len()
            || cell2 >= n_edges_on_cell.len()
        {
            continue;
        }
        let mut nw1 = 0usize;

        for side in 0..2 {
            let (cell_id, vertex_start, tev2) = if side == 0 {
                (cell1, vertices_on_edge[edge_id][1], -1.0)
            } else {
                (cell2, vertices_on_edge[edge_id][0], 1.0)
            };
            let ne = *n_edges_on_cell.get(cell_id)?;
            if ne == 0
                || vertices_on_cell.get(cell_id)?.len() < ne
                || edges_on_cell.get(cell_id)?.len() < ne
            {
                return None;
            }
            let area = *area_cell.get(cell_id)?;
            if area == 0.0 {
                return None;
            }

            let mut riv_cell = Vec::with_capacity(ne);
            for vertex_id in vertices_on_cell[cell_id].iter().copied().take(ne) {
                let cells_for_vertex = *cells_on_vertex.get(vertex_id)?;
                let kite_slot = cells_for_vertex
                    .iter()
                    .position(|candidate| *candidate == cell_id)?;
                riv_cell.push(kite_areas_on_vertex.get(vertex_id)?[kite_slot] / area);
            }

            let vertex_index = find_index_in_prefix(vertex_start, &vertices_on_cell[cell_id], ne)?;
            let mut riv_wrap = riv_cell.clone();
            riv_wrap.extend_from_slice(&riv_cell);

            for wrapped_index in vertex_index..=(vertex_index + ne - 2) {
                let mut kahan_sum = 0.0;
                let mut kahan_c = 0.0;
                for value in &riv_wrap[vertex_index..=wrapped_index] {
                    let kahan_y = *value - kahan_c;
                    let kahan_t = kahan_sum + kahan_y;
                    kahan_c = (kahan_t - kahan_sum) - kahan_y;
                    kahan_sum = kahan_t;
                }
                weights_on_edge[edge_id].push((kahan_sum - 0.5) * tev2);
            }

            let edge_index_cell = find_index_in_prefix(edge_id, &edges_on_cell[cell_id], ne)?;
            let mut edge_index = edges_on_cell[cell_id][0..ne].to_vec();
            edge_index.extend_from_within(0..ne);
            for local_edge_slot in 0..(ne - 1) {
                let output_slot = nw1 + local_edge_slot;
                let contributing_edge_id = edge_index[edge_index_cell + local_edge_slot + 1];
                edges_on_edge[edge_id].push(contributing_edge_id);
                let factor = *dv_edge.get(contributing_edge_id)? / *dc_edge.get(edge_id)?;
                let mut weight = *weights_on_edge[edge_id].get(output_slot)? * factor;
                if cells_on_edge.get(contributing_edge_id)?[1] == cell_id {
                    weight = -weight;
                }
                weights_on_edge[edge_id][output_slot] = weight;
            }

            nw1 = ne - 1;
            n_edges_on_edge[edge_id] += nw1;
        }
    }

    for edge_id in 2..num_edges {
        let mut v_edge = 0.0;
        for (contributing_edge_id, weight) in edges_on_edge[edge_id]
            .iter()
            .copied()
            .zip(weights_on_edge[edge_id].iter().copied())
        {
            v_edge += angle_edge.get(contributing_edge_id)?.cos() * weight;
        }
        let ve = -angle_edge[edge_id].sin();
        error_segment[edge_id] = (v_edge - ve).abs();
    }

    Some(WeightsOnEdgeOutput {
        weights_on_edge,
        edges_on_edge,
        n_edges_on_edge,
        error_segment,
    })
}

fn vector_between(from: CartesianPoint, to: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

fn dot(a: CartesianPoint, b: CartesianPoint) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: CartesianPoint, b: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn magnitude(a: CartesianPoint) -> f64 {
    (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
}

/// Port of the candidate-selection core in `MOD_grid_preprocess:orderVertexArrays`.
///
/// From one reference edge vector, choose the candidate edge with positive CCW
/// orientation around the vertex normal and the smallest angle to the reference
/// vector. The returned index is the zero-based slot in `candidate_edges`.
pub fn next_ccw_edge_candidate_slot(
    vertex: CartesianPoint,
    reference_edge: CartesianPoint,
    candidate_edges: &[CartesianPoint],
) -> Option<usize> {
    let normal = vertex;
    let normal_mag = magnitude(normal);
    let reference_vec = vector_between(vertex, reference_edge);
    let reference_mag = magnitude(reference_vec);
    let mut min_angle = std::f64::consts::PI * 2.0;
    let mut best_slot = None;

    for (slot, candidate_edge) in candidate_edges.iter().copied().enumerate() {
        let candidate_vec = vector_between(vertex, candidate_edge);
        let candidate_mag = magnitude(candidate_vec);
        let cross_prod = cross(reference_vec, candidate_vec);
        let cross_mag = magnitude(cross_prod);

        if cross_mag > 1.0e-15 && normal_mag > 1.0e-15 {
            let dot_val = dot(cross_prod, normal) / (cross_mag * normal_mag);
            if dot_val > 0.0 {
                let denom = reference_mag * candidate_mag;
                if denom == 0.0 {
                    continue;
                }
                let cos_angle = (dot(reference_vec, candidate_vec) / denom).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                if angle < min_angle {
                    min_angle = angle;
                    best_slot = Some(slot);
                }
            }
        }
    }

    best_slot
}

/// Single-vertex output from `orderVertexArrays`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderedVertexArrays {
    pub edges_on_vertex: [usize; 3],
    pub cells_on_vertex: [usize; 3],
}

/// Array-level output from the Fortran-indexed `orderVertexArrays` port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedVertexArraysOutput {
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
}

/// Port of the per-vertex mutation/rebuild workflow in `MOD_grid_preprocess:orderVertexArrays`.
///
/// This preserves the Fortran algorithm: mutate `edgesOnVertex` by repeatedly
/// swapping the next smallest positive-CCW edge into the following slot, then
/// rebuild `cellsOnVertex` from `verticesOnEdge` and `cellsOnEdge`.
pub fn order_vertex_arrays_for_vertex(
    vertex_id: usize,
    vertex: CartesianPoint,
    edges_on_vertex: [usize; 3],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArrays> {
    let mut ordered_edges = edges_on_vertex;

    for j in 0..3 {
        let edge1 = ordered_edges[j];
        if edge1 == 0 {
            continue;
        }
        let reference_edge = *edge_points.get(edge1)?;
        let candidate_slots = ((j + 1)..3)
            .filter(|slot| ordered_edges[*slot] > 0)
            .collect::<Vec<_>>();
        let candidate_points = candidate_slots
            .iter()
            .map(|slot| edge_points.get(ordered_edges[*slot]).copied())
            .collect::<Option<Vec<_>>>()?;
        let Some(relative_slot) =
            next_ccw_edge_candidate_slot(vertex, reference_edge, &candidate_points)
        else {
            continue;
        };
        let swap_slot = candidate_slots[relative_slot];
        if swap_slot != j + 1 {
            ordered_edges.swap(j + 1, swap_slot);
        }
    }

    let mut ordered_cells = [0usize; 3];
    for j in 0..3 {
        let edge = ordered_edges[j];
        if edge == 0 {
            continue;
        }
        let vertices = *vertices_on_edge.get(edge)?;
        let cells = *cells_on_edge.get(edge)?;
        ordered_cells[j] = if vertex_id == vertices[0] {
            cells[0]
        } else {
            cells[1]
        };
    }

    Some(OrderedVertexArrays {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}

/// Fortran-indexed array wrapper for `MOD_grid_preprocess:orderVertexArrays`.
///
/// Indices `0` and `1` are preserved/skipped so existing Fortran-style ids can
/// be used directly while the rest of the mesh workflow is migrated.
pub fn order_vertex_arrays_fortran_indexed(
    vertex_points: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArraysOutput> {
    if edges_on_vertex.len() < vertex_points.len() {
        return None;
    }

    let mut ordered_edges = edges_on_vertex.to_vec();
    let mut ordered_cells = vec![[0usize; 3]; vertex_points.len()];

    for vertex_id in 2..vertex_points.len() {
        let ordered = order_vertex_arrays_for_vertex(
            vertex_id,
            vertex_points[vertex_id],
            ordered_edges[vertex_id],
            edge_points,
            vertices_on_edge,
            cells_on_edge,
        )?;
        ordered_edges[vertex_id] = ordered.edges_on_vertex;
        ordered_cells[vertex_id] = ordered.cells_on_vertex;
    }

    Some(OrderedVertexArraysOutput {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}

/// Port of `MOD_grid_preprocess:arc_length`.
///
/// Computes spherical arc length from Cartesian coordinates using the same
/// haversine form and float32 squaring emulation described in the Fortran code.
pub fn arc_length_unit_sphere(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let r_a = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let r_b = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();

    let lon_a = a.y.atan2(a.x);
    let lat_a = (a.z / r_a).asin();
    let lon_b = b.y.atan2(b.x);
    let lat_b = (b.z / r_b).asin();

    let dlat_half = 0.5 * (lat_a - lat_b);
    let dlon_half = 0.5 * (lon_a - lon_b);

    let sin_dlat_half_f32 = dlat_half.sin() as f32;
    let sin_dlon_half_f32 = dlon_half.sin() as f32;
    let term1 = (sin_dlat_half_f32 * sin_dlat_half_f32) as f64;
    let term2 = lat_b.cos() * lat_a.cos() * (sin_dlon_half_f32 * sin_dlon_half_f32) as f64;

    let arg = (term1 + term2).sqrt();
    r_a * 2.0 * arg.asin()
}

/// Port of `MOD_grid_preprocess:triangle_signed_area_sphere`.
///
/// Despite the Fortran name, the l'Huilier implementation returns a
/// non-negative spherical excess for the three input points. It deliberately
/// reuses `arc_length_unit_sphere` so the same mixed-precision haversine
/// behavior is preserved.
pub fn spherical_triangle_area_unit(points: [CartesianPoint; 3]) -> f64 {
    let a = arc_length_unit_sphere(points[2], points[1]);
    let b = arc_length_unit_sphere(points[2], points[0]);
    let c = arc_length_unit_sphere(points[0], points[1]);
    let semiperimeter = (a + b + c) / 2.0;
    let tan_quarter_excess = (semiperimeter / 2.0).tan()
        * ((semiperimeter - a) / 2.0).tan()
        * ((semiperimeter - b) / 2.0).tan()
        * ((semiperimeter - c) / 2.0).tan();

    4.0 * tan_quarter_excess.max(0.0).sqrt().atan()
}

/// Port of the MPAS kite area primitive inside `MOD_grid_preprocess:GetArea`.
///
/// For one vertex/cell pair, Fortran computes the kite as the absolute area of
/// triangle `(vertex, edge1, cell)` plus triangle `(vertex, edge2, cell)`.
pub fn spherical_kite_area_unit(
    vertex: CartesianPoint,
    edge1: CartesianPoint,
    edge2: CartesianPoint,
    cell: CartesianPoint,
) -> f64 {
    spherical_triangle_area_unit([vertex, edge1, cell]).abs()
        + spherical_triangle_area_unit([vertex, edge2, cell]).abs()
}

/// Port of the `areaCell` fan triangulation inside `MOD_grid_preprocess:GetArea`.
///
/// Fortran pins `verticesOnCell(1, i)` and sums triangles
/// `(v1, vj+1, vj+2)` for `j = 1..num_edges-2`.
pub fn spherical_cell_area_from_vertices_unit(vertices: &[CartesianPoint]) -> Option<f64> {
    if vertices.len() < 3 {
        return None;
    }

    let anchor = vertices[0];
    let mut area = 0.0;
    for j in 0..(vertices.len() - 2) {
        area += spherical_triangle_area_unit([anchor, vertices[j + 1], vertices[j + 2]]);
    }
    Some(area)
}

/// Port of the shared-cell lookup in `MOD_grid_preprocess:GetArea`.
///
/// Fortran checks all four combinations from `cellsOnEdge(:, edge1)` and
/// `cellsOnEdge(:, edge2)` and keeps the maximum matching positive cell id.
/// Zero is the no-cell sentinel and is returned as `None`.
pub fn shared_cell_for_edge_pair(
    edge1_cells: [usize; 2],
    edge2_cells: [usize; 2],
) -> Option<usize> {
    let mut shared_cell = 0usize;
    for cell1 in edge1_cells {
        for cell2 in edge2_cells {
            if cell1 == cell2 {
                shared_cell = shared_cell.max(cell1);
            }
        }
    }

    (shared_cell > 0).then_some(shared_cell)
}

/// Port of the `cellsOnVertex(:, i)` scan in `MOD_grid_preprocess:GetArea`.
///
/// Returns a zero-based Rust index for the matching Fortran `icv` slot.
pub fn vertex_cell_position(cells_on_vertex: [usize; 3], cell: usize) -> Option<usize> {
    cells_on_vertex
        .iter()
        .position(|candidate| *candidate == cell)
}

/// Port of `MOD_grid_preprocess:IsNgrmm`.
///
/// Returns the one-based Fortran code for the vertex in `a` opposite the shared
/// edge with `b`: `1`, `2`, or `3`. Non-neighbor triangles return `None`
/// instead of Fortran's `0` sentinel.
pub fn is_ngrmm(a: [usize; 3], b: [usize; 3]) -> Option<usize> {
    if b.contains(&a[0]) {
        if b.contains(&a[1]) {
            Some(3)
        } else if b.contains(&a[2]) {
            Some(2)
        } else {
            None
        }
    } else if b.contains(&a[1]) && b.contains(&a[2]) {
        Some(1)
    } else {
        None
    }
}

/// Port of the `GetEdge` `cellsOnEdge(:, k)` mapping after `IsNgrmm`.
///
/// The two shared polygon-cell ids are selected from `a` according to the
/// Fortran opposite-vertex code and sorted ascending before return.
pub fn cells_on_edge_from_neighbor_cells(a: [usize; 3], b: [usize; 3]) -> Option<[usize; 2]> {
    let mut cells = match is_ngrmm(a, b)? {
        1 => [a[1], a[2]],
        2 => [a[2], a[0]],
        3 => [a[0], a[1]],
        _ => return None,
    };
    if cells[0] > cells[1] {
        cells.swap(0, 1);
    }
    Some(cells)
}

/// Output from the core connectivity part of `MOD_grid_preprocess:GetEdge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEdgeConnectivity {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
}

/// Production-facing `GetEdge` output after the same post-processing sequence
/// used by the global mesh workflow.
#[derive(Debug, Clone, PartialEq)]
pub struct GetEdgeProductionOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Port of the core connectivity loop in `MOD_grid_preprocess:GetEdge`.
///
/// The optional midpoint calculation is intentionally separate; this function
/// ports edge-id creation/reuse, `verticesOnEdge`, `cellsOnEdge`, and
/// `edgesOnVertex` for Fortran-indexed arrays.
pub fn get_edge_connectivity_fortran_indexed(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
) -> Option<GetEdgeConnectivity> {
    if cells_on_vertex.len() != triangle_neighbors.len() || triangle_neighbors.len() < 2 {
        return None;
    }

    let mut edges_on_vertex = vec![[0usize; 3]; triangle_neighbors.len()];
    let mut cells_on_edge = vec![[0usize; 2]; 2];
    let mut vertices_on_edge = vec![[0usize; 2]; 2];
    let mut triangle_used = vec![false; triangle_neighbors.len()];
    let mut edge_id = 1usize;

    for triangle_id in 2..triangle_neighbors.len() {
        for neighbor_slot in 0..3 {
            let neighbor_id = triangle_neighbors[triangle_id][neighbor_slot];
            if neighbor_id == 0 {
                continue;
            }
            if neighbor_id >= triangle_neighbors.len() {
                return None;
            }

            if triangle_used[neighbor_id] {
                let reuse_slot = triangle_neighbors[neighbor_id]
                    .iter()
                    .position(|candidate| *candidate == triangle_id)?;
                edges_on_vertex[triangle_id][neighbor_slot] =
                    edges_on_vertex[neighbor_id][reuse_slot];
                continue;
            }

            edge_id += 1;
            if cells_on_edge.len() <= edge_id {
                cells_on_edge.resize(edge_id + 1, [0usize; 2]);
                vertices_on_edge.resize(edge_id + 1, [0usize; 2]);
            }

            edges_on_vertex[triangle_id][neighbor_slot] = edge_id;
            vertices_on_edge[edge_id] = [triangle_id, neighbor_id];
            cells_on_edge[edge_id] = cells_on_edge_from_neighbor_cells(
                cells_on_vertex[triangle_id],
                cells_on_vertex[neighbor_id],
            )?;
        }
        triangle_used[triangle_id] = true;
    }

    Some(GetEdgeConnectivity {
        cells_on_edge,
        vertices_on_edge,
        edges_on_vertex,
    })
}

/// Production wrapper for `MOD_grid_preprocess:GetEdge` plus the immediate
/// post-processing used before MPAS-style mesh outputs are consumed.
///
/// The sequence matches the migrated workflow surfaces:
/// `GetEdge`, `GetSort_verticesOnEdge`, optional `vp` midpoint generation, and
/// `orderVertexArrays`.
pub fn get_edge_production_fortran_indexed(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
    triangle_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
) -> Option<GetEdgeProductionOutput> {
    let connectivity = get_edge_connectivity_fortran_indexed(triangle_neighbors, cells_on_vertex)?;
    let vertices_on_edge = order_vertices_on_edge_fortran_indexed(
        triangle_lonlat,
        cell_lonlat,
        &connectivity.cells_on_edge,
        &connectivity.vertices_on_edge,
    )?;
    let edge_points =
        edge_midpoints_from_cells_fortran_indexed(&connectivity.cells_on_edge, cell_lonlat)?;
    let triangle_points = triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let edge_points_cartesian = edge_points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let ordered_vertex_arrays = order_vertex_arrays_fortran_indexed(
        &triangle_points,
        &edge_points_cartesian,
        &connectivity.edges_on_vertex,
        &vertices_on_edge,
        &connectivity.cells_on_edge,
    )?;

    Some(GetEdgeProductionOutput {
        cells_on_edge: connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: ordered_vertex_arrays.edges_on_vertex,
        cells_on_vertex: ordered_vertex_arrays.cells_on_vertex,
        edge_points,
    })
}

/// Port of the optional `vp` midpoint calculation in `MOD_grid_preprocess:GetEdge`.
///
/// For each Fortran-indexed edge id from `2..`, the edge point is the spherical
/// centroid of the two neighboring polygon cell centers `wp(cellsOnEdge(:, k), :)`.
pub fn edge_midpoints_from_cells_fortran_indexed(
    cells_on_edge: &[[usize; 2]],
    cell_lonlat: &[LonLatDegrees],
) -> Option<Vec<LonLatDegrees>> {
    let mut midpoints = vec![LonLatDegrees::new(0.0, 0.0); cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let cell2 = *cell_lonlat.get(cells[1])?;
        midpoints[edge_id] = spherical_centroid_degrees(&[cell1, cell2])?;
    }
    Some(midpoints)
}

/// Borrowed inputs for the Fortran-indexed subset of `MOD_grid_preprocess:GetArea`.
///
/// Index `0` is unused and index `1` is skipped to mirror the Fortran loops
/// that run from `2` through the allocated counts. Positive connectivity ids
/// are therefore used directly as Rust vector indices.
#[derive(Debug, Clone, Copy)]
pub struct GetAreaUnitInput<'a> {
    pub vertices: &'a [CartesianPoint],
    pub edge_points: &'a [CartesianPoint],
    pub cell_points: &'a [CartesianPoint],
    pub cells_on_vertex: &'a [[usize; 3]],
    pub edges_on_vertex: &'a [[usize; 3]],
    pub cells_on_edge: &'a [[usize; 2]],
    pub vertices_on_cell: &'a [Vec<usize>],
}

/// Unit-sphere area outputs from the Fortran-indexed `GetArea` subset.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaUnitOutput {
    pub kite_areas_on_vertex: Vec<[f64; 3]>,
    pub area_triangle: Vec<f64>,
    pub area_cell: Vec<f64>,
}

/// Relative reconstruction error summary printed by `MOD_grid_preprocess:GetArea`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaTriangleReconstructionError {
    pub max_relative: f64,
    pub avg_relative: f64,
}

/// Port of the core array workflow in `MOD_grid_preprocess:GetArea`.
///
/// This keeps the Fortran indexing convention and computes:
///
/// - `kiteAreasOnVertex(icv, i)` from consecutive edge pairs around a vertex.
/// - `areaTriangle(i)` as the sum of the three kite slots for each vertex.
/// - `areaCell(i)` by fan-triangulating `verticesOnCell(:, i)`.
pub fn get_area_unit_fortran_indexed(input: GetAreaUnitInput<'_>) -> Option<GetAreaUnitOutput> {
    if input.cells_on_vertex.len() < input.vertices.len()
        || input.edges_on_vertex.len() < input.vertices.len()
    {
        return None;
    }

    let mut kite_areas_on_vertex = vec![[0.0; 3]; input.vertices.len()];
    let mut area_triangle = vec![0.0; input.vertices.len()];
    let mut area_cell = vec![0.0; input.cell_points.len()];

    for vertex_id in 2..input.vertices.len() {
        let vertex = input.vertices[vertex_id];
        let cells_on_vertex = input.cells_on_vertex[vertex_id];
        let edges_on_vertex = input.edges_on_vertex[vertex_id];

        for edge_slot in 0..3 {
            let next_edge_slot = (edge_slot + 1) % 3;
            let edge1 = edges_on_vertex[edge_slot];
            let edge2 = edges_on_vertex[next_edge_slot];
            if edge1 == 0 || edge2 == 0 {
                continue;
            }

            let edge1_cells = *input.cells_on_edge.get(edge1)?;
            let edge2_cells = *input.cells_on_edge.get(edge2)?;
            let Some(cell_id) = shared_cell_for_edge_pair(edge1_cells, edge2_cells) else {
                continue;
            };
            let Some(vertex_cell_slot) = vertex_cell_position(cells_on_vertex, cell_id) else {
                continue;
            };

            let edge1_point = *input.edge_points.get(edge1)?;
            let edge2_point = *input.edge_points.get(edge2)?;
            let cell_point = *input.cell_points.get(cell_id)?;
            kite_areas_on_vertex[vertex_id][vertex_cell_slot] =
                spherical_kite_area_unit(vertex, edge1_point, edge2_point, cell_point);
        }
    }

    for vertex_id in 2..input.vertices.len() {
        area_triangle[vertex_id] = kite_areas_on_vertex[vertex_id].iter().sum();
    }

    for cell_id in 2..input.cell_points.len() {
        let Some(vertex_ids) = input.vertices_on_cell.get(cell_id) else {
            continue;
        };
        if vertex_ids.len() < 3 {
            continue;
        }
        let vertices = vertex_ids
            .iter()
            .map(|vertex_id| input.vertices.get(*vertex_id).copied())
            .collect::<Option<Vec<_>>>()?;
        area_cell[cell_id] = spherical_cell_area_from_vertices_unit(&vertices)?;
    }

    Some(GetAreaUnitOutput {
        kite_areas_on_vertex,
        area_triangle,
        area_cell,
    })
}

/// Production-facing `GetArea` output with the diagnostic summary printed by
/// the Fortran routine.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaProductionOutput {
    pub unit: GetAreaUnitOutput,
    pub reconstruction_error: AreaTriangleReconstructionError,
}

/// Production wrapper for `MOD_grid_preprocess:GetArea`.
///
/// This combines the migrated unit-sphere area workflow with the reconstruction
/// relative-error diagnostic that the Fortran routine prints after computing
/// `areaTriangle`.
pub fn get_area_production_fortran_indexed(
    input: GetAreaUnitInput<'_>,
) -> Option<GetAreaProductionOutput> {
    let unit = get_area_unit_fortran_indexed(input)?;
    let reconstruction_error = area_triangle_reconstruction_error_fortran_indexed(
        &unit.area_triangle,
        input.cell_points,
        input.cells_on_vertex,
    )?;

    Some(GetAreaProductionOutput {
        unit,
        reconstruction_error,
    })
}

/// Port of the `GetArea` area-triangle reconstruction error summary.
///
/// For each Fortran-indexed vertex id from `2..`, the routine recomputes the
/// triangle area from `cellsOnVertex(:, i)` cell centers and compares it with
/// the reconstructed `areaTriangle(i)`.
pub fn area_triangle_reconstruction_error_fortran_indexed(
    area_triangle: &[f64],
    cell_points: &[CartesianPoint],
    cells_on_vertex: &[[usize; 3]],
) -> Option<AreaTriangleReconstructionError> {
    if area_triangle.len() < 3 || cells_on_vertex.len() < area_triangle.len() {
        return None;
    }

    let mut max_relative = 0.0;
    let mut sum_relative = 0.0;
    let mut count = 0usize;

    for vertex_id in 2..area_triangle.len() {
        let cell_ids = cells_on_vertex[vertex_id];
        if cell_ids.contains(&0) {
            return None;
        }
        let exact = spherical_triangle_area_unit([
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ]);
        if exact == 0.0 {
            return None;
        }
        let relative = (area_triangle[vertex_id] - exact).abs() / exact;
        max_relative = f64::max(max_relative, relative);
        sum_relative += relative;
        count += 1;
    }

    Some(AreaTriangleReconstructionError {
        max_relative,
        avg_relative: sum_relative / count as f64,
    })
}

/// Output of `MOD_grid_preprocess:Get_Length_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonLengthAngleMetrics {
    pub angles_degrees: Vec<f64>,
    pub edge_lengths_meters: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Length_Angle`.
///
/// For each polygon vertex, this builds the same `(previous, current, next)`
/// triplet as the Fortran cyclic buffer, computes the spherical angle using the
/// half-angle formula, and records the current-to-next edge length scaled by
/// `erad8`.
pub fn polygon_length_angle_metrics(points: &[LonLatDegrees]) -> Option<PolygonLengthAngleMetrics> {
    let num_edges = points.len();
    if num_edges < 3 {
        return None;
    }

    let mut angles_degrees = Vec::with_capacity(num_edges);
    let mut edge_lengths_meters = Vec::with_capacity(num_edges);

    for i in 0..num_edges {
        let previous = points[(i + num_edges - 1) % num_edges];
        let current = points[i];
        let next = points[(i + 1) % num_edges];

        let previous_xyz = lonlat_degrees_to_unit_xyz(previous);
        let current_xyz = lonlat_degrees_to_unit_xyz(current);
        let next_xyz = lonlat_degrees_to_unit_xyz(next);

        let length1 = arc_length_unit_sphere(next_xyz, current_xyz);
        let length2 = arc_length_unit_sphere(next_xyz, previous_xyz);
        let length3 = arc_length_unit_sphere(previous_xyz, current_xyz);
        let semiperimeter = 0.5 * (length1 + length2 + length3);
        let angle_arg = ((semiperimeter - length1).sin() * (semiperimeter - length3).sin()
            / (length1.sin() * length3.sin()))
        .sqrt();
        angles_degrees.push(rad_to_deg(2.0 * angle_arg.asin()));
        edge_lengths_meters.push(length1 * earthmesh_core::EARTH_RADIUS_METERS);
    }

    Some(PolygonLengthAngleMetrics {
        angles_degrees,
        edge_lengths_meters,
    })
}

/// Mesh-quality aggregate produced by Fortran `TriMeshQuality`/`PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualitySummary {
    pub cell_metrics: Vec<PolygonLengthAngleMetrics>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

fn polygon_quality_summary(
    cells: &[Vec<LonLatDegrees>],
    regular_angle_degrees: f64,
    lower_threshold_degrees: f64,
    upper_threshold_degrees: f64,
) -> Option<MeshQualitySummary> {
    if cells.is_empty() {
        return None;
    }

    let mut cell_metrics = Vec::with_capacity(cells.len());
    let mut angle_less_flags = Vec::with_capacity(cells.len());
    let mut angle_more_flags = Vec::with_capacity(cells.len());
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut angle_count = 0usize;

    for cell in cells {
        let metrics = polygon_length_angle_metrics(cell)?;
        let cell_min = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let cell_max = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        global_min = global_min.min(cell_min);
        global_max = global_max.max(cell_max);
        sum_min += cell_min;
        sum_max += cell_max;
        sum_squared += metrics
            .angles_degrees
            .iter()
            .map(|angle| (angle - regular_angle_degrees).powi(2))
            .sum::<f64>();
        angle_count += metrics.angles_degrees.len();
        angle_less_flags.push(cell_min < lower_threshold_degrees);
        angle_more_flags.push(cell_max > upper_threshold_degrees);
        cell_metrics.push(metrics);
    }

    let cell_count = cells.len() as f64;
    Some(MeshQualitySummary {
        cell_metrics,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / cell_count, sum_max / cell_count),
        angle_stddev_degrees: (sum_squared / angle_count as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:TriMeshQuality`.
pub fn triangle_mesh_quality(triangles: &[[LonLatDegrees; 3]]) -> Option<MeshQualitySummary> {
    let cells: Vec<Vec<LonLatDegrees>> =
        triangles.iter().map(|triangle| triangle.to_vec()).collect();
    polygon_quality_summary(&cells, 60.0, 45.0, 75.0)
}

/// Fortran-style cache/update output for `MOD_grid_preprocess:TriMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleMeshQualityFortranOutput {
    pub length_cache: Vec<[f64; 3]>,
    pub angle_cache: Vec<[f64; 3]>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:TriMeshQuality`.
///
/// Inputs use the repository's Rust convention for migrated Fortran-indexed
/// arrays: slots `0` and `1` are placeholders and triangle ids start at `2`.
/// Adjusted triangles are recalculated from `cell_points`/`cells_on_triangle`;
/// unadjusted triangles reuse the provided angle/length caches.
pub fn triangle_mesh_quality_fortran_indexed(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    adjust_flags: &[bool],
    length_cache: &[[f64; 3]],
    angle_cache: &[[f64; 3]],
) -> Option<TriangleMeshQualityFortranOutput> {
    let len = cells_on_triangle.len();
    if len < 3 || adjust_flags.len() != len || length_cache.len() != len || angle_cache.len() != len
    {
        return None;
    }

    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; len];
    let mut angle_more_flags = vec![false; len];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut count = 0usize;

    for triangle_id in 2..len {
        if adjust_flags[triangle_id] {
            let cell_ids = cells_on_triangle[triangle_id];
            let triangle = [
                *cell_points.get(cell_ids[0])?,
                *cell_points.get(cell_ids[1])?,
                *cell_points.get(cell_ids[2])?,
            ];
            let metrics = polygon_length_angle_metrics(&triangle)?;
            updated_angles[triangle_id] = [
                metrics.angles_degrees[0],
                metrics.angles_degrees[1],
                metrics.angles_degrees[2],
            ];
            updated_lengths[triangle_id] = [
                metrics.edge_lengths_meters[0],
                metrics.edge_lengths_meters[1],
                metrics.edge_lengths_meters[2],
            ];
        }

        let angles = updated_angles[triangle_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - 60.0).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[triangle_id] = min_angle < 45.0;
        angle_more_flags[triangle_id] = max_angle > 75.0;
        count += 1;
    }

    Some(TriangleMeshQualityFortranOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / count as f64, sum_max / count as f64),
        angle_stddev_degrees: (sum_squared / (3 * count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:PolyMeshQuality`.
///
/// All cells in the input should have the same edge count, matching each
/// Fortran call for pentagons, hexagons, or heptagons. The regular angle is
/// `(num_edges - 2) * 180 / num_edges`, with 0.9/1.1 threshold bands.
pub fn polygon_mesh_quality(cells: &[Vec<LonLatDegrees>]) -> Option<MeshQualitySummary> {
    let first = cells.first()?;
    let num_edges = first.len();
    if num_edges < 3 || cells.iter().any(|cell| cell.len() != num_edges) {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    polygon_quality_summary(
        cells,
        regular_angle,
        regular_angle * 0.9,
        regular_angle * 1.1,
    )
}

/// Fortran-style compact cache/update output for `MOD_grid_preprocess:PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonMeshQualityFortranOutput {
    pub length_cache: Vec<Vec<f64>>,
    pub angle_cache: Vec<Vec<f64>>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:PolyMeshQuality`.
///
/// Fortran iterates over cell ids from `2`, skips cells whose `n_ngrwm` does not
/// match `num_edges`, and stores quality caches in a compact `j` index for only
/// the matching cells. This Rust port preserves that compact-cache contract.
pub fn polygon_mesh_quality_fortran_indexed(
    num_edges: usize,
    cell_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
    length_cache: &[Vec<f64>],
    angle_cache: &[Vec<f64>],
) -> Option<PolygonMeshQualityFortranOutput> {
    let len = cells_on_polygon.len();
    if num_edges < 3 || len < 3 || polygon_edge_counts.len() != len || adjust_flags.len() != len {
        return None;
    }

    let matching_count = (2..len)
        .filter(|&cell_id| polygon_edge_counts[cell_id] == num_edges)
        .count();
    if matching_count == 0
        || length_cache.len() != matching_count
        || angle_cache.len() != matching_count
        || length_cache.iter().any(|row| row.len() != num_edges)
        || angle_cache.iter().any(|row| row.len() != num_edges)
    {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    let angle_regularless = regular_angle * 0.9;
    let angle_regularmore = regular_angle * 1.1;
    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; matching_count];
    let mut angle_more_flags = vec![false; matching_count];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut compact_id = 0usize;

    for cell_id in 2..len {
        if polygon_edge_counts[cell_id] != num_edges {
            continue;
        }

        if adjust_flags[cell_id] {
            let polygon_indices = cells_on_polygon.get(cell_id)?;
            if polygon_indices.len() < num_edges {
                return None;
            }
            let mut polygon = Vec::with_capacity(num_edges);
            for &point_id in polygon_indices.iter().take(num_edges) {
                polygon.push(*cell_points.get(point_id)?);
            }
            let metrics = polygon_length_angle_metrics(&polygon)?;
            updated_angles[compact_id] = metrics.angles_degrees;
            updated_lengths[compact_id] = metrics.edge_lengths_meters;
        }

        let angles = &updated_angles[compact_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - regular_angle).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[compact_id] = min_angle < angle_regularless;
        angle_more_flags[compact_id] = max_angle > angle_regularmore;
        compact_id += 1;
    }

    Some(PolygonMeshQualityFortranOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (
            sum_min / matching_count as f64,
            sum_max / matching_count as f64,
        ),
        angle_stddev_degrees: (sum_squared / (num_edges * matching_count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Polygon edge-count classes reported by
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolygonEdgeClassCounts {
    pub pentagons: usize,
    pub hexagons: usize,
    pub heptagons: usize,
    pub less_than_five: usize,
    pub greater_than_seven: usize,
}

/// Quality summaries produced by the Rust orchestration wrapper for
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, PartialEq)]
pub struct GridQualityGlobalOutput {
    pub edge_class_counts: PolygonEdgeClassCounts,
    pub triangle: TriangleMeshQualityFortranOutput,
    pub pentagon: Option<PolygonMeshQualityFortranOutput>,
    pub hexagon: Option<PolygonMeshQualityFortranOutput>,
    pub heptagon: Option<PolygonMeshQualityFortranOutput>,
}

fn polygon_edge_class_counts_fortran_indexed(
    polygon_edge_counts: &[usize],
) -> PolygonEdgeClassCounts {
    let mut counts = PolygonEdgeClassCounts {
        pentagons: 0,
        hexagons: 0,
        heptagons: 0,
        less_than_five: 0,
        greater_than_seven: 0,
    };

    for edge_count in polygon_edge_counts.iter().copied().skip(2) {
        match edge_count {
            5 => counts.pentagons += 1,
            6 => counts.hexagons += 1,
            7 => counts.heptagons += 1,
            count if count < 5 => counts.less_than_five += 1,
            _ => counts.greater_than_seven += 1,
        }
    }

    counts
}

fn polygon_quality_or_none_fortran_indexed(
    num_edges: usize,
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
) -> Option<Option<PolygonMeshQualityFortranOutput>> {
    let matching_count = polygon_edge_counts
        .iter()
        .copied()
        .skip(2)
        .filter(|edge_count| *edge_count == num_edges)
        .count();

    if matching_count == 0 {
        return Some(None);
    }

    let length_cache = vec![vec![0.0; num_edges]; matching_count];
    let angle_cache = vec![vec![0.0; num_edges]; matching_count];
    polygon_mesh_quality_fortran_indexed(
        num_edges,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .map(Some)
}

/// Rust orchestration wrapper for `MOD_grid_preprocess:Grid_Quality_Check_Global`.
///
/// This ports the calculation side of the Fortran routine: polygon edge-class
/// counting, all-true initial adjust flags, triangle quality, and 5/6/7-sided
/// polygon quality groups. The NetCDF `quality_save_global` side effect remains
/// an adapter/output-layer responsibility.
pub fn grid_quality_check_global_fortran_indexed(
    triangle_cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
) -> Option<GridQualityGlobalOutput> {
    if cells_on_polygon.len() != polygon_edge_counts.len() {
        return None;
    }

    let edge_class_counts = polygon_edge_class_counts_fortran_indexed(polygon_edge_counts);
    let triangle_adjust_flags = vec![true; cells_on_triangle.len()];
    let triangle_length_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle_angle_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle = triangle_mesh_quality_fortran_indexed(
        triangle_cell_points,
        cells_on_triangle,
        &triangle_adjust_flags,
        &triangle_length_cache,
        &triangle_angle_cache,
    )?;

    let polygon_adjust_flags = vec![true; cells_on_polygon.len()];
    let pentagon = polygon_quality_or_none_fortran_indexed(
        5,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let hexagon = polygon_quality_or_none_fortran_indexed(
        6,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let heptagon = polygon_quality_or_none_fortran_indexed(
        7,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;

    Some(GridQualityGlobalOutput {
        edge_class_counts,
        triangle,
        pentagon,
        hexagon,
        heptagon,
    })
}

/// Port of `MOD_grid_preprocess:robust_spherical_area`.
///
/// Returns signed area on the unit sphere. The caller can multiply by radius²
/// when physical area is needed. The formula preserves Fortran's dateline-aware
/// `delta_lon` adjustment and does not take an absolute value.
pub fn robust_spherical_area_unit(points: &[LonLatDegrees]) -> Option<f64> {
    let num_inter = points.len();
    if num_inter < 3 {
        return None;
    }

    let mut area = 0.0;
    for i in 0..num_inter {
        let j = (i + 1) % num_inter;
        let lon_i = deg_to_rad(points[i].lon_degrees);
        let lon_j = deg_to_rad(points[j].lon_degrees);
        let lat_i = deg_to_rad(points[i].lat_degrees);
        let lat_j = deg_to_rad(points[j].lat_degrees);
        let mut delta_lon = lon_j - lon_i;
        if delta_lon > std::f64::consts::PI {
            delta_lon -= 2.0 * std::f64::consts::PI;
        } else if delta_lon < -std::f64::consts::PI {
            delta_lon += 2.0 * std::f64::consts::PI;
        }
        area += delta_lon * (2.0 + lat_i.sin() + lat_j.sin());
    }

    Some(area / 2.0)
}
