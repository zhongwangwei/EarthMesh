//! The finishing passes on a refined triangulation: an angle-safe Lawson
//! polish, and the repair into the published angle window.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use crate::RedGreenMesh;

/// Result of angle-protected Lawson polishing, not an angle or Delaunay certificate.
#[derive(Debug, PartialEq, Eq)]
pub struct RedGreenPolishReport {
    pub flipped_edges: usize,
    pub forced_flips: usize,
    pub remaining_illegal_edges: usize,
}

/// Compatibility entry point for angle-safe polishing. The returned flip count
/// does not imply Delaunay certification; use `polish_redgreen_mesh` for the audit.
pub fn legalize_redgreen_mesh(mesh: &mut RedGreenMesh) -> io::Result<usize> {
    Ok(polish_redgreen_mesh(mesh)?.flipped_edges)
}

/// Publication cannot leave illegal diagonals that create an overfull W ring.
/// Try the quality guard first, then use ordinary Lawson only where necessary.
pub fn finalize_redgreen_mesh(mesh: &mut RedGreenMesh) -> io::Result<RedGreenPolishReport> {
    polish_redgreen_mesh_impl(mesh, true)
}

const MAX_ADJACENT_RESOLUTION_RATIO: f64 = 2.0;

#[derive(Default)]
struct LocalResolutionStats {
    max_ratio: f64,
    over_limit_count: usize,
    by_edge: BTreeMap<(usize, usize), f64>,
}

fn triangle_edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn triangle_edges(corners: [usize; 3]) -> [(usize, usize); 3] {
    [
        triangle_edge(corners[0], corners[1]),
        triangle_edge(corners[1], corners[2]),
        triangle_edge(corners[2], corners[0]),
    ]
}

fn shared_triangle_edge(left: [usize; 3], right: [usize; 3]) -> io::Result<(usize, usize)> {
    let shared = left
        .into_iter()
        .filter(|vertex| right.contains(vertex))
        .collect::<Vec<_>>();
    if shared.len() == 2 {
        Ok(triangle_edge(shared[0], shared[1]))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green adjacency does not share exactly one edge",
        ))
    }
}

fn triangle_resolution_scale(
    corners: [usize; 3],
    vertices: &[earthmesh_mesh::CartesianPoint],
) -> io::Result<f64> {
    let point_at = |vertex: usize| {
        vertices.get(vertex).copied().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("red-green triangle references missing vertex {vertex}"),
            )
        })
    };
    let area = earthmesh_mesh::spherical_triangle_area_unit([
        point_at(corners[0])?,
        point_at(corners[1])?,
        point_at(corners[2])?,
    ]);
    if area.is_finite() && area > 0.0 {
        Ok(area.sqrt())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green triangle has non-positive spherical area",
        ))
    }
}

fn record_resolution_ratio(stats: &mut LocalResolutionStats, edge: (usize, usize), ratio: f64) {
    stats.max_ratio = stats.max_ratio.max(ratio);
    if ratio > MAX_ADJACENT_RESOLUTION_RATIO {
        stats.over_limit_count += 1;
    }
    stats.by_edge.insert(edge, ratio);
}

fn current_local_resolution_stats(
    state: &earthmesh_mesh::MeshState,
    faces: [usize; 2],
) -> io::Result<LocalResolutionStats> {
    local_resolution_stats_for_pair(
        state,
        faces[0],
        faces[1],
        [state.triangles()[faces[0]], state.triangles()[faces[1]]],
    )
}

fn external_neighbours_by_edge(
    state: &earthmesh_mesh::MeshState,
    faces: [usize; 2],
) -> io::Result<BTreeMap<(usize, usize), usize>> {
    let mut external = BTreeMap::new();
    for face in faces {
        let here = state.triangles()[face];
        for &neighbour in &state.neighbours()[face] {
            if !state.is_triangle_live(neighbour) || faces.contains(&neighbour) {
                continue;
            }
            let edge = shared_triangle_edge(here, state.triangles()[neighbour])?;
            if external.insert(edge, neighbour).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "red-green local edge has more than one external neighbour",
                ));
            }
        }
    }
    Ok(external)
}

fn local_resolution_stats_for_pair(
    state: &earthmesh_mesh::MeshState,
    triangle: usize,
    neighbour: usize,
    candidate: [[usize; 3]; 2],
) -> io::Result<LocalResolutionStats> {
    let external = external_neighbours_by_edge(state, [triangle, neighbour])?;
    let scales = [
        triangle_resolution_scale(candidate[0], state.vertices())?,
        triangle_resolution_scale(candidate[1], state.vertices())?,
    ];
    let internal = shared_triangle_edge(candidate[0], candidate[1])?;
    let mut stats = LocalResolutionStats::default();
    let ratio = scales[0].max(scales[1]) / scales[0].min(scales[1]);
    record_resolution_ratio(&mut stats, internal, ratio);

    for (index, corners) in candidate.into_iter().enumerate() {
        for edge in triangle_edges(corners) {
            if edge == internal {
                continue;
            }
            let Some(&external_face) = external.get(&edge) else {
                continue;
            };
            let external_scale =
                triangle_resolution_scale(state.triangles()[external_face], state.vertices())?;
            let ratio = scales[index].max(external_scale) / scales[index].min(external_scale);
            record_resolution_ratio(&mut stats, edge, ratio);
        }
    }
    Ok(stats)
}

fn resolution_safe_after_flip(
    state: &earthmesh_mesh::MeshState,
    triangle: usize,
    neighbour: usize,
    candidate: [[usize; 3]; 2],
) -> io::Result<bool> {
    let before = current_local_resolution_stats(state, [triangle, neighbour])?;
    let after = local_resolution_stats_for_pair(state, triangle, neighbour, candidate)?;
    if after.over_limit_count > before.over_limit_count
        || (before.max_ratio > MAX_ADJACENT_RESOLUTION_RATIO
            && after.max_ratio > before.max_ratio + 1.0e-12)
    {
        return Ok(false);
    }
    for (edge, before_ratio) in before.by_edge {
        if before_ratio <= MAX_ADJACENT_RESOLUTION_RATIO
            && after
                .by_edge
                .get(&edge)
                .is_some_and(|after_ratio| *after_ratio > MAX_ADJACENT_RESOLUTION_RATIO)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Improve admissible edges independently; never discard a safe flip merely
/// because another quadrilateral needs a different geometric repair.
/// Per-face depth after edge flips: a face the flips rewrote takes the deepest
/// depth among the rewritten faces whose old triangle shared an edge with it,
/// which are the faces it was cut from. Faces the flips did not touch keep
/// theirs. Returns no depths when none were tracked on the way in.
fn flipped_refinement_levels(
    old: &[[usize; 3]],
    new: &[[usize; 3]],
    levels: &[usize],
) -> Vec<usize> {
    if levels.len() != old.len() || new.len() != old.len() {
        return Vec::new();
    }
    let sorted = |t: [usize; 3]| {
        let mut t = t;
        t.sort_unstable();
        t
    };
    let edges = |t: [usize; 3]| [(t[0], t[1]), (t[1], t[2]), (t[0], t[2])];
    let changed: Vec<usize> = (0..old.len())
        .filter(|&face| sorted(old[face]) != sorted(new[face]))
        .collect();
    let mut old_faces_on_edge = std::collections::HashMap::<(usize, usize), Vec<usize>>::new();
    for &face in &changed {
        let corners = sorted(old[face]);
        if corners[0] <= 1 {
            continue;
        }
        for edge in edges(corners) {
            old_faces_on_edge.entry(edge).or_default().push(face);
        }
    }
    let mut result = levels.to_vec();
    for &face in &changed {
        let corners = sorted(new[face]);
        if corners[0] <= 1 {
            continue;
        }
        let mut depth = levels[face];
        for edge in edges(corners) {
            for &source in old_faces_on_edge
                .get(&edge)
                .map(Vec::as_slice)
                .unwrap_or(&[])
            {
                depth = depth.max(levels[source]);
            }
        }
        result[face] = depth;
    }
    result
}

pub fn polish_redgreen_mesh(mesh: &mut RedGreenMesh) -> io::Result<RedGreenPolishReport> {
    polish_redgreen_mesh_impl(mesh, false)
}

fn polish_redgreen_mesh_impl(
    mesh: &mut RedGreenMesh,
    finish_delaunay: bool,
) -> io::Result<RedGreenPolishReport> {
    let vertices = mesh
        .cell_points
        .iter()
        .copied()
        .map(earthmesh_mesh::lonlat_degrees_to_unit_xyz)
        .collect();
    let mut state = earthmesh_mesh::MeshState::from_parts(vertices, mesh.cells_on_triangle.clone())
        .map_err(|errors| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                errors
                    .into_iter()
                    .take(4)
                    .map(|error| error.to_string())
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        })?;
    let faces = state.active_triangle_slots().collect::<BTreeSet<_>>();
    let mut shape_error = None;
    let safe_flips = state
        .legalize_around_if(&faces, |state, triangle, corner| {
            let neighbor = state.neighbours()[triangle][corner];
            let before = [state.triangles()[triangle], state.triangles()[neighbor]];
            let admissible = (|| {
                let candidate = crate::checked_lop_edge_flip(
                    triangle,
                    neighbor,
                    before[0],
                    before[1],
                    &mesh.cell_points,
                )?;
                let old = crate::triangle_pair_angle_range(before, &mesh.cell_points)?;
                let new = crate::triangle_pair_angle_range(candidate.triangles, &mesh.cell_points)?;
                if !(new.0 >= old.0 - 1.0e-9 && new.1 <= old.1 + 1.0e-9) {
                    return Ok(false);
                }
                resolution_safe_after_flip(state, triangle, neighbor, candidate.triangles)
            })();
            match admissible {
                Ok(accept) => accept,
                Err(error) => {
                    shape_error = Some(error);
                    false
                }
            }
        })
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if let Some(error) = shape_error {
        return Err(error);
    }
    let count_illegal = |state: &earthmesh_mesh::MeshState| -> io::Result<usize> {
        let mut count = 0;
        for &triangle in &faces {
            for corner in 0..3 {
                if triangle < state.neighbours()[triangle][corner]
                    && state.edge_is_illegal(triangle, corner).map_err(|error| {
                        io::Error::new(io::ErrorKind::InvalidData, error.to_string())
                    })?
                {
                    count += 1;
                }
            }
        }
        Ok(count)
    };
    let mut remaining_illegal_edges = count_illegal(&state)?;
    let forced_flips = if finish_delaunay && remaining_illegal_edges > 0 {
        let forced = state
            .legalize_around(&faces)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
        remaining_illegal_edges = count_illegal(&state)?;
        forced
    } else {
        0
    };
    let flips = safe_flips + forced_flips;
    let report = RedGreenPolishReport {
        flipped_edges: flips,
        forced_flips,
        remaining_illegal_edges,
    };
    if flips == 0 {
        return Ok(report);
    }

    // This is a terminal triangulation step. Flips invalidate green ancestry;
    // reject any later attempt to refine this finalized mesh as a red hierarchy.
    mesh.green_parents.clear();
    // The per-face depth is still what the output reports as each cell's
    // refinement level, so carry it across the flips rather than dropping it:
    // a flipped face covers parts of the old faces it was cut from, and takes
    // the deepest of them. Balance keeps neighbouring depths within one, so
    // this can only round a transition face up by at most one level.
    mesh.refinement_levels = flipped_refinement_levels(
        &mesh.cells_on_triangle,
        state.triangles(),
        &mesh.refinement_levels,
    );
    mesh.cells_on_triangle = state.triangles().to_vec();
    rebuild_redgreen_derived_tables(mesh)?;
    Ok(report)
}

/// Triangle centres and the ordered triangle ring of every cell, recomputed
/// from the corner table and cell positions after either has changed.
fn rebuild_redgreen_derived_tables(mesh: &mut RedGreenMesh) -> io::Result<()> {
    mesh.triangle_points.resize(
        mesh.cells_on_triangle.len(),
        earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
    );
    for triangle in 2..mesh.cells_on_triangle.len() {
        let corners = mesh.cells_on_triangle[triangle];
        mesh.triangle_points[triangle] = earthmesh_mesh::spherical_centroid_degrees(&[
            mesh.cell_points[corners[0]],
            mesh.cell_points[corners[1]],
            mesh.cell_points[corners[2]],
        ])
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Red-Green triangle {triangle} has no centroid"),
            )
        })?;
    }
    mesh.triangles_on_cell = vec![Vec::new(); mesh.cell_points.len()];
    for (triangle, corners) in mesh.cells_on_triangle.iter().enumerate().skip(2) {
        for &cell in corners {
            mesh.triangles_on_cell[cell].push(triangle);
        }
    }
    mesh.n_triangles_on_cell = mesh.triangles_on_cell.iter().map(Vec::len).collect();
    crate::get_sort_new_one_based(
        mesh.cell_count(),
        &mesh.n_triangles_on_cell,
        &mesh.cells_on_triangle,
        &mesh.triangle_points,
        &mut mesh.triangles_on_cell,
    )?;
    Ok(())
}

/// The output contract's triangle angle window (guide 11.72), aimed at with a
/// small margin so the coordinates' rounding on the way to the file cannot
/// carry an angle back out.
const ANGLE_WINDOW_MARGIN_DEG: f64 = 0.25;

/// Bring red-green's triangles into the enforced angle window.
///
/// The bisection closure leaves ~30/60/90 triangles at every level change, and
/// refinement cannot remove them -- it makes them again one level down. This
/// is a terminal step on the finished triangulation: valence-balancing flips,
/// removal of refined vertices of valence 3 or 4, and angle-driven vertex
/// moves (`earthmesh_mesh::repair_triangle_angle_window`). Base-mesh cells
/// keep their ids; refined ones above a removed vertex shift down by one.
pub fn repair_redgreen_angle_window(
    mesh: &mut RedGreenMesh,
    degree_cap: Option<usize>,
) -> io::Result<earthmesh_mesh::AngleWindowReport> {
    let (lo, hi) = earthmesh_quality::TRIANGLE_ANGLE_WINDOW_DEG;
    let mut options = earthmesh_mesh::AngleWindowOptions::new((
        lo + ANGLE_WINDOW_MARGIN_DEG,
        hi - ANGLE_WINDOW_MARGIN_DEG,
    ));
    options.first_vertex = 2;
    options.first_face = 2;
    options.removable_from = mesh.num_center.max(2);
    // Into the window only: pushing angles toward 60 is the pipeline's
    // backend-neutral angle contract, which runs on every backend's mesh.
    options.equilateral_rounds = 0;
    // Never widen the widest cell: the dual and the mask post-process are
    // built for the degrees the mesh already has. A hex grid is held to its
    // cap instead, and a cell above it is brought down: capped at the widest
    // cell, a degree-8 cell left by a refinement level set its own limit.
    options.max_valence = degree_cap.unwrap_or_else(|| {
        (2..mesh.n_triangles_on_cell.len())
            .map(|cell| mesh.n_triangles_on_cell[cell])
            .max()
            .unwrap_or(0)
            .max(7)
    });
    let mut points: Vec<[f64; 3]> = mesh
        .cell_points
        .iter()
        .map(|&p| {
            let xyz = earthmesh_mesh::lonlat_degrees_to_unit_xyz(p);
            [xyz.x, xyz.y, xyz.z]
        })
        .collect();
    let mut faces = mesh.cells_on_triangle.clone();
    let mut levels = if mesh.refinement_levels.len() == faces.len() {
        mesh.refinement_levels.clone()
    } else {
        Vec::new()
    };
    let report =
        earthmesh_mesh::repair_triangle_angle_window(&mut points, &mut faces, &mut levels, options);
    if report.flips + report.moves + report.removed_vertices == 0 {
        return Ok(report);
    }
    mesh.green_parents.clear();
    mesh.cell_points = points
        .iter()
        .map(|&[x, y, z]| {
            earthmesh_mesh::xyz_to_lonlat_degrees(earthmesh_mesh::CartesianPoint::new(x, y, z))
        })
        .collect();
    mesh.cells_on_triangle = faces;
    mesh.refinement_levels = levels;
    rebuild_redgreen_derived_tables(mesh)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flipped_faces_take_the_deepest_face_they_were_cut_from() {
        // Quad 2-3-4-5 split along 2-4 into depths 1 and 2, flipped onto 3-5;
        // face 3 is elsewhere and untouched.
        let old = [[1, 1, 1], [2, 3, 4], [2, 4, 5], [6, 7, 8]];
        let new = [[1, 1, 1], [2, 3, 5], [3, 4, 5], [6, 7, 8]];
        let levels = [0, 1, 2, 1];
        assert_eq!(
            flipped_refinement_levels(&old, &new, &levels),
            vec![0, 2, 2, 1]
        );
        // Nothing flipped: depths pass through untouched.
        assert_eq!(
            flipped_refinement_levels(&old, &old, &levels),
            levels.to_vec()
        );
        // Depth that was never tracked stays absent.
        assert!(flipped_refinement_levels(&old, &new, &[]).is_empty());
    }

    #[test]
    fn final_legalization_replaces_an_illegal_diagonal() {
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 4],
            cell_points: vec![
                earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(3.0, 0.0),
                earthmesh_mesh::LonLatDegrees::new(4.0, 1.0),
                earthmesh_mesh::LonLatDegrees::new(0.0, 3.0),
            ],
            cells_on_triangle: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 4, 5]],
            triangles_on_cell: vec![Vec::new(); 6],
            n_triangles_on_cell: vec![0; 6],
            green_parents: Vec::new(),
            refinement_levels: Vec::new(),
        };

        let flips = legalize_redgreen_mesh(&mut mesh).expect("legalize final point set");

        assert_eq!(flips, 1);
        assert!(mesh.cells_on_triangle[2].contains(&3));
        assert!(mesh.cells_on_triangle[2].contains(&5));
        assert!(mesh.cells_on_triangle[3].contains(&3));
        assert!(mesh.cells_on_triangle[3].contains(&5));
    }

    #[test]
    fn final_lawson_refuses_flip_that_breaks_external_two_to_one_ratio() {
        use earthmesh_mesh::LonLatDegrees as Point;
        // Current illegal edge B-C is angle-safe to flip, but that would move the
        // large A-B-D triangle onto the already-valid tiny neighbour across A-B.
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![Point::new(0.0, 0.0); 5],
            cell_points: vec![
                Point::new(0.0, 0.0),
                Point::new(0.0, 0.0),
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.3687017802959149, 0.15753136583659297),
                Point::new(0.9502750133197551, -0.8704885354647786),
                Point::new(0.5, -0.05),
            ],
            cells_on_triangle: vec![[1; 3], [1; 3], [2, 3, 4], [5, 4, 3], [6, 3, 2]],
            triangles_on_cell: vec![Vec::new(); 7],
            n_triangles_on_cell: vec![0; 7],
            green_parents: Vec::new(),
            refinement_levels: vec![0; 5],
        };

        let before = mesh.cells_on_triangle.clone();
        let report = polish_redgreen_mesh(&mut mesh).unwrap();

        assert_eq!(report.flipped_edges, 0);
        assert!(report.remaining_illegal_edges >= 1);
        assert_eq!(mesh.cells_on_triangle, before);
    }
    #[test]
    fn final_lawson_keeps_safe_flips_without_accepting_the_bad_pair() {
        use earthmesh_mesh::LonLatDegrees as Point;
        // Two real NXP80 quadrilaterals. The first flip raises max angle
        // 97.159 -> 113.033; the second improves 90.809 -> 89.197.
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![Point::new(0.0, 0.0); 6],
            cell_points: vec![
                Point::new(0.0, 0.0),
                Point::new(0.0, 0.0),
                Point::new(1.5904483212864065, -73.3537167014725),
                Point::new(2.4385950022378506, -73.69405763645796),
                Point::new(4.774389118700588, -73.31695417755233),
                Point::new(2.3384087199332564, -73.00411890583615),
                Point::new(13.87039566655922, -5.3910828568021305),
                Point::new(14.080406690793641, -5.732818572761572),
                Point::new(14.538690578907662, -4.97067681285334),
                Point::new(14.748917229045073, -5.312203227305401),
            ],
            cells_on_triangle: vec![[1; 3], [1; 3], [2, 3, 4], [5, 2, 4], [6, 7, 8], [9, 8, 7]],
            triangles_on_cell: vec![Vec::new(); 10],
            n_triangles_on_cell: vec![0; 10],
            green_parents: Vec::new(),
            refinement_levels: vec![0; 6],
        };
        let before = mesh.cells_on_triangle.clone();
        let flips = polish_redgreen_mesh(&mut mesh).unwrap();
        assert_eq!(
            flips.flipped_edges, 1,
            "a bad candidate must not discard the independent safe flip"
        );
        assert_eq!(flips.remaining_illegal_edges, 1);
        assert_eq!(&mesh.cells_on_triangle[2..4], &before[2..4]);
        assert_ne!(&mesh.cells_on_triangle[4..6], &before[4..6]);
    }
}
