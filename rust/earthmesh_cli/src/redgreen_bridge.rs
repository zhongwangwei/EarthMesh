//! Between the red-green refinement tables and the gridfile's mesh.
//!
//! They are the same five tables under different names -- the writer's
//! `m_to_w`/`w_to_m` are the pipeline's `ngrmw`/`ngrwm` -- so this is a
//! conversion rather than a translation. What it does have to do is police the
//! width: ids are `usize` on one side and `i32` on the other, and a mesh too
//! large to address in `i32` has to say so here rather than wrap silently into
//! a gridfile that reads as valid.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use earthmesh_refine_redgreen::RedGreenMesh;
use rayon::prelude::*;

use crate::coordinate_types::LonLatPoint;
use crate::unstructured_mesh_support::UnstructuredMesh;

fn narrow(id: usize, role: &str) -> io::Result<i32> {
    i32::try_from(id).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{role} id {id} exceeds what a gridfile can address"),
        )
    })
}

/// The refined mesh, in the shape the gridfile writer takes.
pub fn unstructured_mesh_from_redgreen(mesh: &RedGreenMesh) -> io::Result<UnstructuredMesh> {
    let point = |p: &earthmesh_mesh::LonLatDegrees| LonLatPoint {
        lon: p.lon_degrees,
        lat: p.lat_degrees,
    };
    let mut m_to_w = Vec::with_capacity(mesh.cells_on_triangle.len());
    for corners in &mesh.cells_on_triangle {
        m_to_w.push([
            narrow(corners[0], "cell")?,
            narrow(corners[1], "cell")?,
            narrow(corners[2], "cell")?,
        ]);
    }
    let mut w_to_m = Vec::with_capacity(mesh.triangles_on_cell.len());
    for row in &mesh.triangles_on_cell {
        let mut converted = Vec::with_capacity(row.len());
        for &triangle in row {
            converted.push(narrow(triangle, "triangle")?);
        }
        w_to_m.push(converted);
    }
    let mut n_w_to_m = Vec::with_capacity(mesh.n_triangles_on_cell.len());
    for &count in &mesh.n_triangles_on_cell {
        n_w_to_m.push(narrow(count, "triangle count")?);
    }

    Ok(UnstructuredMesh {
        m_points: mesh.triangle_points.iter().map(point).collect(),
        w_points: mesh.cell_points.iter().map(point).collect(),
        m_to_w,
        w_to_m,
        n_w_to_m,
    })
}

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
// Geometry contract, independent of the configurable warn/fail quality policy.
const TRIANGLE_SHAPE_FLOOR_DEG: f64 = 25.0;

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
                let candidate = earthmesh_refine_redgreen::checked_lop_edge_flip(
                    triangle,
                    neighbor,
                    before[0],
                    before[1],
                    &mesh.cell_points,
                )?;
                let old = earthmesh_refine_redgreen::triangle_pair_angle_range(
                    before,
                    &mesh.cell_points,
                )?;
                let new = earthmesh_refine_redgreen::triangle_pair_angle_range(
                    candidate.triangles,
                    &mesh.cell_points,
                )?;
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
    mesh.refinement_levels.clear();
    mesh.cells_on_triangle = state.triangles().to_vec();
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
                format!("legalized Red-Green triangle {triangle} has no centroid"),
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
    earthmesh_refine_redgreen::get_sort_new_one_based(
        mesh.cell_count(),
        &mesh.n_triangles_on_cell,
        &mesh.cells_on_triangle,
        &mesh.triangle_points,
        &mut mesh.triangles_on_cell,
    )?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_refined_mesh_arrives_with_every_table_intact() {
        // The tables are the same five under different names, so the test that
        // matters is that none of them loses a row or a slot on the way.
        let mesh =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        let redgreen = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&mesh, &neighbors)
            .expect("bridge in");

        let written = unstructured_mesh_from_redgreen(&redgreen).expect("bridge out");

        assert_eq!(written.m_points.len(), redgreen.triangle_points.len());
        assert_eq!(written.w_points.len(), redgreen.cell_points.len());
        assert_eq!(written.m_to_w.len(), redgreen.cells_on_triangle.len());
        assert_eq!(written.w_to_m.len(), redgreen.triangles_on_cell.len());
        assert_eq!(
            written.m_to_w[2],
            redgreen.cells_on_triangle[2].map(|id| id as i32),
            "a triangle's corners survive the narrowing"
        );
        assert_eq!(
            written.n_w_to_m[2] as usize,
            redgreen.n_triangles_on_cell[2]
        );
    }

    #[test]
    fn an_id_too_wide_for_the_gridfile_is_refused_rather_than_wrapped() {
        // Silently wrapping would produce a gridfile that reads as valid and
        // points at the wrong cells.
        let mut mesh = RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cell_points: vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 3],
            cells_on_triangle: vec![[1, 1, 1]; 3],
            triangles_on_cell: vec![Vec::new(); 3],
            n_triangles_on_cell: vec![0; 3],
            green_parents: Vec::new(),
            refinement_levels: Vec::new(),
        };
        mesh.cells_on_triangle[2] = [1, 1, i32::MAX as usize + 1];

        let error = unstructured_mesh_from_redgreen(&mesh)
            .expect_err("an unaddressable id must not wrap into a gridfile");
        assert!(
            error.to_string().contains("exceeds what a gridfile"),
            "{error}"
        );
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

/// The engine's refinement settings, as this level's red-green run reads them.
///
/// `halo` and `max_transition_row` are per-level in the namelist -- v2's
/// `HALO = 3, 3, 3` -- so the level picks its own entry. A level past the end of
/// the array reuses the last one that was given rather than silently falling
/// back to a default: the array is how the user said "these levels", and
/// running a deeper level on a number nobody wrote would be inventing one.
pub fn redgreen_settings_for_level(
    refine: &earthmesh_core::RefineConfig,
    level: usize,
) -> earthmesh_refine_redgreen::RedGreenSettings {
    let defaults = earthmesh_refine_redgreen::RedGreenSettings::default();
    let at_level = |values: &[i32; 10], fallback: usize| -> usize {
        let index = level.max(1).min(values.len() - 1);
        (values[index] > 0)
            .then_some(values[index])
            .or_else(|| {
                values[1..index]
                    .iter()
                    .rev()
                    .find(|&&value| value > 0)
                    .copied()
            })
            .map(|value| value as usize)
            .unwrap_or(fallback)
    };
    earthmesh_refine_redgreen::RedGreenSettings {
        max_transition_row: at_level(&refine.max_transition_row, defaults.max_transition_row),
        build_transition_rows: refine.is_transition,
        eliminate_weak_concavity: refine.weak_concav_eliminate,
        halo: at_level(&refine.halo, defaults.halo),
        protect_triangle_quality: false,
        min_triangle_angle_deg: defaults.min_triangle_angle_deg,
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn each_level_reads_its_own_halo_and_transition_width() {
        // The namelist gives these per level -- v2's HALO = 3, 3, 3 -- so a
        // three-level run that narrows the band as it deepens has to be read
        // that way, not collapsed to one number.
        let refine = earthmesh_core::RefineConfig {
            halo: [0, 4, 3, 2, 0, 0, 0, 0, 0, 0],
            max_transition_row: [0, 3, 2, 1, 0, 0, 0, 0, 0, 0],
            ..earthmesh_core::RefineConfig::default()
        };

        assert_eq!(redgreen_settings_for_level(&refine, 1).halo, 4);
        assert_eq!(redgreen_settings_for_level(&refine, 2).halo, 3);
        assert_eq!(redgreen_settings_for_level(&refine, 3).halo, 2);
        assert_eq!(
            redgreen_settings_for_level(&refine, 3).max_transition_row,
            1
        );
    }

    #[test]
    fn omitted_redgreen_bands_use_the_algorithm_defaults() {
        let refine = earthmesh_core::RefineConfig::default();
        let settings = redgreen_settings_for_level(&refine, 1);
        assert_eq!(settings.halo, 3);
        assert_eq!(settings.max_transition_row, 3);
    }

    #[test]
    fn levels_past_the_configured_prefix_reuse_the_last_value() {
        let refine = earthmesh_core::RefineConfig {
            halo: [0, 4, 2, 0, 0, 0, 0, 0, 0, 0],
            max_transition_row: [0, 5, 3, 0, 0, 0, 0, 0, 0, 0],
            ..earthmesh_core::RefineConfig::default()
        };

        let settings = redgreen_settings_for_level(&refine, 4);
        assert_eq!(settings.halo, 2);
        assert_eq!(settings.max_transition_row, 3);
    }
}

/// Which triangles a level's regions ask for, one entry per triangle.
///
/// The marking is the whole interface between "what the project wants" and
/// "what red-green builds": any set of triangles is legal input, and the judge
/// chain grows it until the triangulation closes. That is why this can be a
/// containment test and nothing more -- there is no shape to satisfy.
///
/// A triangle is asked for when its own centre falls inside a region. Centre
/// sampling is the same rule the ocean carve uses, so a cell is refined and
/// kept, or neither, rather than refined and then carved away.
pub fn redgreen_marking_from_regions(
    mesh: &earthmesh_refine_redgreen::RedGreenMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    level: usize,
) -> Vec<i32> {
    let mut marking = vec![0i32; mesh.triangle_count() + 1];
    if regions.is_empty() {
        return marking;
    }
    let region_index = earthmesh_mesh::RefinementRegionIndex::new(regions);
    marking
        .par_iter_mut()
        .enumerate()
        .skip(mesh.num_vertex + 1)
        .for_each(|(triangle, mark)| {
            let centre = mesh.triangle_points[triangle];
            *mark = i32::from(region_index.contains_lonlat_canonical(centre, level));
        });
    marking
}

#[cfg(test)]
mod marking_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    fn base() -> earthmesh_refine_redgreen::RedGreenMesh {
        let mesh =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = mesh.m_neighbors.clone();
        earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&mesh, &neighbors).expect("bridge")
    }

    #[test]
    fn a_circle_marks_the_triangles_whose_centres_it_holds() {
        let mesh = base();
        let marking = redgreen_marking_from_regions(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            }],
            1,
        );

        let marked = marking.iter().filter(|&&value| value == 1).count();
        assert!(marked > 0, "a circle this size must hold some triangle");
        assert!(
            marked < mesh.triangle_count(),
            "and must not hold the whole globe: {marked} of {}",
            mesh.triangle_count()
        );
        assert_eq!(marking[0], 0, "slot 0 is not a triangle");
        assert_eq!(
            marking[1], 0,
            "slot 1 is the canonical placeholder and is never asked for"
        );
    }

    #[test]
    fn a_region_shallower_than_this_level_asks_for_nothing_here() {
        // A level-1 circle is served by level 1 and must not reappear at level
        // 2, or every level would refine everything the one above it did.
        let mesh = base();
        let regions = [RefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 2_000_000.0,
            level: 1,
        }];

        assert!(redgreen_marking_from_regions(&mesh, &regions, 1).contains(&1));
        assert!(redgreen_marking_from_regions(&mesh, &regions, 2)
            .iter()
            .all(|&value| value == 0));
    }

    #[test]
    fn marking_is_identical_across_thread_counts() {
        let mesh = base();
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(179.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(-45.0, 80.0),
                radius_meters: 1_000_000.0,
                level: 1,
            },
        ];
        let run = |threads| {
            rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap()
                .install(|| redgreen_marking_from_regions(&mesh, &regions, 1))
        };

        assert_eq!(run(1), run(4));
    }
}

/// One red-green level, from the regions that asked for it to a mesh the
/// gridfile writer takes.
///
/// `previous_level_marks` is the level above's settled red interior **in this
/// mesh's numbering**. Transition children are deliberately excluded so a
/// deeper round cannot split them again.
pub fn refine_redgreen_level(
    mesh: &earthmesh_refine_redgreen::RedGreenMesh,
    regions: &[earthmesh_mesh::RefinementRegion],
    refine: &earthmesh_core::RefineConfig,
    level: usize,
    previous_level_marks: Option<&[i32]>,
    preserve_locality: bool,
) -> io::Result<(UnstructuredMesh, earthmesh_refine_redgreen::RedGreenOutcome)> {
    let marking = redgreen_marking_from_regions(mesh, regions, level);
    let mut settings = redgreen_settings_for_level(refine, level);
    // TRI can publish variable-width W fans; HEX still needs the canonical dual.
    settings.protect_triangle_quality = preserve_locality;
    if preserve_locality {
        settings.min_triangle_angle_deg = TRIANGLE_SHAPE_FLOOR_DEG;
    }
    let outcome = earthmesh_refine_redgreen::refine_redgreen_round_inside(
        mesh,
        &marking,
        &settings,
        previous_level_marks,
    )?;
    let written = unstructured_mesh_from_redgreen(&outcome.mesh)?;
    Ok((written, outcome))
}

#[cfg(test)]
mod level_tests {
    use super::*;
    use earthmesh_mesh::{LonLatDegrees, RefinementRegion};

    #[test]
    fn a_named_circle_refines_and_arrives_as_a_writable_mesh() {
        // The chain end to end: regions -> marking -> round -> gridfile tables.
        // Every link has its own test; this is the one that says they compose.
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let before = mesh.triangle_count();

        let (written, outcome) = refine_redgreen_level(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }],
            &earthmesh_core::RefineConfig::default(),
            1,
            None,
            false,
        )
        .expect("one red-green level");

        assert!(
            outcome.refined_triangle_count > 0,
            "the circle asked for triangles: {outcome:?}"
        );
        assert!(
            outcome.mesh.triangle_count() > before,
            "a refined mesh has more triangles: {} vs {before}",
            outcome.mesh.triangle_count()
        );
        assert_eq!(
            written.m_to_w.len(),
            outcome.mesh.cells_on_triangle.len(),
            "and arrives whole"
        );
    }

    /// A refined mesh has to reach the writer with both arrays on the same row
    /// layout.
    ///
    /// A gridfile reader picks between the compact layout (id = row + 1) and the
    /// two-placeholder one (id = row) by whether rows 0 and 1 sit at the origin
    /// -- and it picks per array. The renewal left slot 0 of the cell points at
    /// its 9999 "no vertex here yet" sentinel, so the cell array read as compact
    /// while the triangle array read as two-placeholder. The file opened, passed
    /// its checks, and resolved every connectivity id one row off.
    #[test]
    fn a_refined_mesh_keeps_both_arrays_on_the_same_row_layout() {
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");

        let (written, _) = refine_redgreen_level(
            &mesh,
            &[RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            }],
            // With the transition rows off the round leaves hanging nodes by
            // design -- only the hexagonal dual reads such a mesh -- so the
            // triangular view is only a mesh to check when they are on.
            &earthmesh_core::RefineConfig {
                is_transition: true,
                ..earthmesh_core::RefineConfig::default()
            },
            1,
            None,
            false,
        )
        .expect("one red-green level");

        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
        assert!(
            topology.is_consistent(),
            "a refined red-green mesh must reach the writer as one mesh: {:?}",
            &topology.violations[..topology.violations.len().min(4)]
        );
    }

    /// A deeper level must stay inside the red children actually produced by
    /// its parent. Re-testing region geometry would also include green
    /// transition children and let later levels split them into slivers.
    #[test]
    fn a_deeper_level_is_held_inside_the_one_above_it() {
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(9, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        // Both levels ask for the same disc, so level 2 reaches all the way out
        // to level 1's boundary and the halo has something to cancel.
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(0.0, 0.0),
                radius_meters: 3_000_000.0,
                level: 2,
            },
        ];
        // The transition rows have to be built for a level to be chainable at
        // all -- without them the round leaves hanging nodes and the next one
        // cannot even derive the triangle neighbours -- and the halo is what
        // holds the deeper level inside.
        let refine = earthmesh_core::RefineConfig {
            is_transition: true,
            halo: [3; 10],
            max_transition_row: [3; 10],
            ..earthmesh_core::RefineConfig::default()
        };

        let (_, first) =
            refine_redgreen_level(&mesh, &regions, &refine, 1, None, false).expect("level one");
        let previous = first.interior_marks.clone();
        assert_eq!(
            previous.len(),
            first.mesh.triangle_count() + 1,
            "the carried interior must use the mesh numbering the next level sees"
        );
        assert_ne!(
            previous.len(),
            first.cell_renumbering.len(),
            "and cell_renumbering is not that mapping -- it is per cell"
        );

        let (_, held) =
            refine_redgreen_level(&first.mesh, &regions, &refine, 2, Some(&previous), false)
                .expect("level two, held inside level one");
        let (_, free) = refine_redgreen_level(&first.mesh, &regions, &refine, 2, None, false)
            .expect("level two, free");

        assert!(
            held.halo_cancelled_count > 0,
            "a level reaching its parent's boundary must be pulled back inside: {held:?}"
        );
        assert_eq!(free.halo_cancelled_count, 0, "and only when asked to be");
        assert!(
            held.refined_triangle_count < free.refined_triangle_count,
            "so it refines less: {} vs {}",
            held.refined_triangle_count,
            free.refined_triangle_count
        );
    }
}
