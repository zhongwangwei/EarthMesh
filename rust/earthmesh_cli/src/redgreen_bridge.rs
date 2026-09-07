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

/// Restore the final Red-Green point set to a spherical Delaunay triangulation.
///
/// The ported transition LOP only visits its boundary-segment candidates. A
/// multi-component adaptive run can leave other illegal diagonals behind, so
/// the shared Lawson implementation must finish the job before publication.
pub fn legalize_redgreen_mesh(mesh: &mut RedGreenMesh) -> io::Result<usize> {
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
    let flips = state
        .legalize_around(&faces)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    if flips == 0 {
        return Ok(0);
    }

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
    Ok(flips)
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
        };

        let flips = legalize_redgreen_mesh(&mut mesh).expect("legalize final point set");

        assert_eq!(flips, 1);
        assert!(mesh.cells_on_triangle[2].contains(&3));
        assert!(mesh.cells_on_triangle[2].contains(&5));
        assert!(mesh.cells_on_triangle[3].contains(&3));
        assert!(mesh.cells_on_triangle[3].contains(&5));
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

/// Restore only holes whose entire boundary decomposes into triangular faces.
///
/// The weak-concavity transition can omit a face where two carried transition
/// rows meet. Filling an arbitrary boundary would hide a broken mesh, so this
/// accepts only edge-disjoint three-edge cycles and leaves every other shape as
/// an error for the caller's topology gate.
fn is_hanging_edge_cycle(cycle: [usize; 3], points: &[earthmesh_mesh::LonLatDegrees]) -> bool {
    if cycle.iter().any(|&cell| cell >= points.len()) {
        return false;
    }
    let xyz = cycle.map(|cell| earthmesh_mesh::lonlat_degrees_to_unit_xyz(points[cell]));
    let mut edges = [
        earthmesh_mesh::arc_length_unit_sphere(xyz[0], xyz[1]),
        earthmesh_mesh::arc_length_unit_sphere(xyz[1], xyz[2]),
        earthmesh_mesh::arc_length_unit_sphere(xyz[0], xyz[2]),
    ];
    edges.sort_by(f64::total_cmp);
    (edges[2] - edges[0] - edges[1]).abs() <= 1.0e-10 * edges[2].max(1.0)
}

fn close_triangular_transition_holes(mesh: &mut RedGreenMesh) -> io::Result<usize> {
    let edge_counts = |mesh: &RedGreenMesh| -> io::Result<BTreeMap<(usize, usize), usize>> {
        let mut counts = BTreeMap::new();
        for (triangle, corners) in mesh
            .cells_on_triangle
            .iter()
            .enumerate()
            .skip(mesh.num_vertex + 1)
        {
            if corners
                .iter()
                .any(|&cell| cell == 0 || cell >= mesh.cell_points.len())
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("red-green triangle {triangle} names a missing cell"),
                ));
            }
            for (a, b) in [
                (corners[0], corners[1]),
                (corners[1], corners[2]),
                (corners[2], corners[0]),
            ] {
                *counts.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        Ok(counts)
    };

    let counts = edge_counts(mesh)?;
    if let Some((&edge, &owners)) = counts.iter().find(|(_, owners)| **owners > 2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("red-green edge {edge:?} has {owners} owners"),
        ));
    }
    let boundary = counts
        .iter()
        .filter_map(|(&edge, &owners)| (owners == 1).then_some(edge))
        .collect::<BTreeSet<_>>();
    if boundary.is_empty() {
        return Ok(0);
    }

    let mut graph = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &(a, b) in &boundary {
        graph.entry(a).or_default().insert(b);
        graph.entry(b).or_default().insert(a);
    }
    let mut holes = BTreeSet::<[usize; 3]>::new();
    for &(a, b) in &boundary {
        let common = graph[&a]
            .intersection(&graph[&b])
            .copied()
            .filter(|&middle| is_hanging_edge_cycle([a, b, middle], &mesh.cell_points))
            .collect::<Vec<_>>();
        if common.len() != 1 {
            let reason = if common.is_empty() {
                "contains a true missing face, not a hanging edge".to_string()
            } else {
                format!("belongs to {} hanging-edge cycles", common.len())
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("red-green transition boundary edge {a}/{b} {reason}"),
            ));
        }
        let mut face = [a, b, common[0]];
        face.sort_unstable();
        holes.insert(face);
    }
    let mut covered = BTreeMap::<(usize, usize), usize>::new();
    for [a, b, c] in &holes {
        for edge in [(*a, *b), (*b, *c), (*a, *c)] {
            *covered.entry(edge).or_default() += 1;
        }
    }
    if covered.len() != boundary.len()
        || boundary
            .iter()
            .any(|edge| covered.get(edge).copied() != Some(1))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green transition boundary is not a set of edge-disjoint triangular holes",
        ));
    }

    let mut repaired = mesh.clone();
    let orient = |mut corners: [usize; 3], points: &[earthmesh_mesh::LonLatDegrees]| {
        let face_points = corners.map(|cell| points[cell]);
        let xyz = face_points.map(earthmesh_mesh::lonlat_degrees_to_unit_xyz);
        match earthmesh_mesh::orientation_on_sphere(xyz[0], xyz[1], xyz[2]).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("red-green transition face orientation is ambiguous: {error}"),
            )
        })? {
            earthmesh_mesh::Sign::Positive => {}
            earthmesh_mesh::Sign::Negative => corners.swap(1, 2),
            earthmesh_mesh::Sign::Zero => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "red-green transition face is degenerate",
                ));
            }
        }
        let centroid =
            earthmesh_mesh::spherical_centroid_degrees(&face_points).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "red-green transition face has no spherical centroid",
                )
            })?;
        Ok::<_, io::Error>((corners, centroid))
    };
    for cycle in holes.iter().copied() {
        let xyz = cycle
            .map(|cell| earthmesh_mesh::lonlat_degrees_to_unit_xyz(repaired.cell_points[cell]));
        let edges = [
            (
                earthmesh_mesh::arc_length_unit_sphere(xyz[0], xyz[1]),
                0,
                1,
                2,
            ),
            (
                earthmesh_mesh::arc_length_unit_sphere(xyz[1], xyz[2]),
                1,
                2,
                0,
            ),
            (
                earthmesh_mesh::arc_length_unit_sphere(xyz[0], xyz[2]),
                0,
                2,
                1,
            ),
        ];
        let &(_, left, right, middle) = edges
            .iter()
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .expect("three edges");
        let a = cycle[left];
        let b = cycle[right];
        let midpoint = cycle[middle];
        let owners = repaired
            .cells_on_triangle
            .iter()
            .enumerate()
            .skip(repaired.num_vertex + 1)
            .filter(|(_, corners)| corners.contains(&a) && corners.contains(&b))
            .map(|(triangle, _)| triangle)
            .collect::<Vec<_>>();
        if owners.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "red-green hanging edge {a}/{b} has {} coarse owners",
                    owners.len()
                ),
            ));
        }
        let owner = owners[0];
        let opposite = repaired.cells_on_triangle[owner]
            .iter()
            .copied()
            .find(|&cell| cell != a && cell != b)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("red-green hanging-edge owner {owner} has no opposite cell"),
                )
            })?;
        let (first, first_center) = orient([opposite, a, midpoint], &repaired.cell_points)?;
        let (second, second_center) = orient([opposite, midpoint, b], &repaired.cell_points)?;
        repaired.cells_on_triangle[owner] = first;
        repaired.triangle_points[owner] = first_center;
        repaired.cells_on_triangle.push(second);
        repaired.triangle_points.push(second_center);
    }
    repaired.triangles_on_cell = vec![Vec::new(); repaired.cell_points.len()];
    for (triangle, corners) in repaired.cells_on_triangle.iter().enumerate().skip(2) {
        for &cell in corners {
            repaired.triangles_on_cell[cell].push(triangle);
        }
    }
    repaired.n_triangles_on_cell = repaired.triangles_on_cell.iter().map(Vec::len).collect();
    earthmesh_refine_redgreen::get_sort_new_one_based(
        repaired.cell_count(),
        &repaired.n_triangles_on_cell,
        &repaired.cells_on_triangle,
        &repaired.triangle_points,
        &mut repaired.triangles_on_cell,
    )?;
    if edge_counts(&repaired)?.values().any(|&owners| owners != 2) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green triangular-hole repair did not produce a closed mesh",
        ));
    }
    let repaired_count = holes.len();
    *mesh = repaired;
    Ok(repaired_count)
}

#[cfg(test)]
mod transition_hole_tests {
    use super::*;

    fn tetrahedron_with_one_hanging_midpoint() -> RedGreenMesh {
        let mut cell_points = vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 7];
        cell_points[2] = earthmesh_mesh::LonLatDegrees::new(0.0, 70.0);
        cell_points[3] = earthmesh_mesh::LonLatDegrees::new(-120.0, -20.0);
        cell_points[4] = earthmesh_mesh::LonLatDegrees::new(0.0, -20.0);
        cell_points[5] = earthmesh_mesh::LonLatDegrees::new(120.0, -20.0);
        cell_points[6] = earthmesh_refine_redgreen::midpoint_lonlat(cell_points[2], cell_points[4])
            .expect("edge midpoint");
        let mut cells_on_triangle = vec![[1usize; 3]; 2];
        let mut triangle_points = vec![earthmesh_mesh::LonLatDegrees::new(0.0, 0.0); 2];
        for mut corners in [[3, 2, 6], [3, 6, 4], [3, 5, 4], [2, 5, 3], [2, 4, 5]] {
            let xyz =
                corners.map(|cell| earthmesh_mesh::lonlat_degrees_to_unit_xyz(cell_points[cell]));
            if earthmesh_mesh::orientation_on_sphere(xyz[0], xyz[1], xyz[2])
                .expect("non-degenerate face")
                == earthmesh_mesh::Sign::Negative
            {
                corners.swap(1, 2);
            }
            triangle_points.push(
                earthmesh_mesh::spherical_centroid_degrees(&corners.map(|cell| cell_points[cell]))
                    .expect("face centroid"),
            );
            cells_on_triangle.push(corners);
        }
        let mut triangles_on_cell = vec![Vec::new(); cell_points.len()];
        for (triangle, corners) in cells_on_triangle.iter().enumerate().skip(2) {
            for &cell in corners {
                triangles_on_cell[cell].push(triangle);
            }
        }
        let n_triangles_on_cell = triangles_on_cell.iter().map(Vec::len).collect();
        RedGreenMesh {
            num_vertex: 1,
            num_center: 1,
            triangle_points,
            cell_points,
            cells_on_triangle,
            triangles_on_cell,
            n_triangles_on_cell,
        }
    }

    #[test]
    fn only_a_complete_hanging_edge_cycle_is_restored() {
        let mut mesh = tetrahedron_with_one_hanging_midpoint();

        assert_eq!(close_triangular_transition_holes(&mut mesh).unwrap(), 1);
        let neighbors = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
            &mesh.cells_on_triangle,
            &mesh.triangles_on_cell,
            &mesh.n_triangles_on_cell,
        )
        .expect("closed membership");
        assert!(
            neighbors.iter().skip(2).all(|row| !row.contains(&0)),
            "every restored edge must have a neighbor: {neighbors:?}"
        );
        assert!(mesh.cells_on_triangle.iter().skip(2).all(|corners| {
            earthmesh_mesh::spherical_triangle_area_unit(
                corners
                    .map(|cell| earthmesh_mesh::lonlat_degrees_to_unit_xyz(mesh.cell_points[cell])),
            ) > 0.0
        }));
        assert_eq!(close_triangular_transition_holes(&mut mesh).unwrap(), 0);
    }

    #[test]
    fn hanging_edge_geometry_disambiguates_graph_candidates() {
        let mesh = tetrahedron_with_one_hanging_midpoint();

        assert!(is_hanging_edge_cycle([2, 4, 6], &mesh.cell_points));
        assert!(!is_hanging_edge_cycle([2, 4, 3], &mesh.cell_points));
    }

    #[test]
    fn a_true_missing_face_is_not_hidden_as_a_transition_repair() {
        let mut mesh = tetrahedron_with_one_hanging_midpoint();
        close_triangular_transition_holes(&mut mesh).unwrap();
        mesh.cells_on_triangle.pop();
        mesh.triangle_points.pop();
        let before = mesh.clone();

        let error = close_triangular_transition_holes(&mut mesh)
            .expect_err("a non-collinear missing face is not a hanging edge");

        assert!(error.to_string().contains("true missing face"), "{error}");
        assert_eq!(mesh, before, "a refused repair must not mutate the mesh");
    }
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
    settings.protect_triangle_quality = preserve_locality;
    let primary = earthmesh_refine_redgreen::refine_redgreen_round_inside(
        mesh,
        &marking,
        &settings,
        previous_level_marks,
    );
    let mut fallback_reason = None;
    let mut outcome = match primary {
        Ok(outcome) => outcome,
        Err(error) if preserve_locality && settings.eliminate_weak_concavity => {
            fallback_reason = Some(format!(
                "weak-concavity elimination could not form a local transition ({error})"
            ));
            settings.eliminate_weak_concavity = false;
            earthmesh_refine_redgreen::refine_redgreen_round_inside(
                mesh,
                &marking,
                &settings,
                previous_level_marks,
            )?
        }
        Err(error) => return Err(error),
    };
    let refinable = mesh.triangle_count().saturating_sub(mesh.num_vertex);
    if preserve_locality
        && settings.eliminate_weak_concavity
        && outcome.grown_triangle_count > 0
        && outcome.refined_triangle_count == refinable
    {
        settings.eliminate_weak_concavity = false;
        outcome = earthmesh_refine_redgreen::refine_redgreen_round_inside(
            mesh,
            &marking,
            &settings,
            previous_level_marks,
        )?;
        fallback_reason =
            Some("weak-concavity elimination reached the whole triangular domain".to_string());
    }
    let closed = if preserve_locality {
        close_triangular_transition_holes(&mut outcome.mesh)?
    } else {
        0
    };
    outcome
        .interior_marks
        .resize(outcome.mesh.triangle_count() + 1, 0);
    if let Some(reason) = fallback_reason {
        eprintln!(
            "earthmesh_cli: Red-Green {reason}; carrying the boundary concavities instead{}",
            if closed == 0 {
                String::new()
            } else {
                format!(" and restoring {closed} triangular transition face(s)")
            }
        );
    } else if closed > 0 {
        eprintln!(
            "earthmesh_cli: Red-Green restored {closed} triangular hanging-edge transition face(s)"
        );
    }
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

    #[test]
    fn triangular_red_green_does_not_turn_distributed_demand_into_global_refinement() {
        let base =
            earthmesh_mesh::TriangularMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base mesh");
        let neighbors = base.m_neighbors.clone();
        let mesh = earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &neighbors)
            .expect("bridge in");
        let refine = earthmesh_core::RefineConfig::default();
        let refinable = mesh.triangle_count() - mesh.num_vertex;
        let mut fixture = None;
        'search: for step in [45, 30, 20] {
            for radius_meters in [1_500_000.0, 2_000_000.0, 2_500_000.0] {
                let regions = [-60.0, 0.0, 60.0]
                    .into_iter()
                    .flat_map(|lat| {
                        (-180..180)
                            .step_by(step)
                            .map(move |lon| RefinementRegion::Circle {
                                center: LonLatDegrees::new(f64::from(lon), lat),
                                radius_meters,
                                level: 1,
                            })
                    })
                    .collect::<Vec<_>>();
                let (_, filled) = refine_redgreen_level(&mesh, &regions, &refine, 1, None, false)
                    .expect("filled run");
                if filled.refined_triangle_count == refinable && filled.grown_triangle_count > 0 {
                    fixture = Some((regions, filled));
                    break 'search;
                }
            }
        }
        let (regions, filled) = fixture.expect("a distributed marking must exercise the fallback");

        let (written, local) =
            refine_redgreen_level(&mesh, &regions, &refine, 1, None, true).expect("local run");
        assert!(
            local.refined_triangle_count < filled.refined_triangle_count,
            "distributed local demand must retain a coarse exterior"
        );
        let neighbors = earthmesh_mesh::triangle_neighbors_from_cell_membership_one_based(
            &local.mesh.cells_on_triangle,
            &local.mesh.triangles_on_cell,
            &local.mesh.n_triangles_on_cell,
        )
        .expect("local transition membership must resolve");
        assert!(
            neighbors
                .iter()
                .skip(local.mesh.num_vertex + 1)
                .all(|row| !row.contains(&0)),
            "the carried transition must close every edge"
        );
        let topology = crate::unstructured_mesh_support::check_unstructured_mesh_topology(&written);
        assert!(
            topology
                .violations
                .iter()
                .all(|violation| !violation.starts_with("misoriented_shared_edge")),
            "the carried transition must stay consistently oriented: {:?}",
            topology.violations
        );
    }
}
