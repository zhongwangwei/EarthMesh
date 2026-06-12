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
