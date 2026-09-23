//! Triangle output needs shape-regular closure, not the dual-degree transition
//! rows. Decide every split on the unchanged parents before creating children.
use std::{collections::HashMap, io};

use super::{orient_triangles_outward, RedGreenBalanceReport, RedGreenMesh, RedGreenOutcome};
use crate::{midpoint_lonlat, LonLatDegrees};

fn angle_range(points: [LonLatDegrees; 3]) -> io::Result<(f64, f64)> {
    let metrics = earthmesh_mesh::polygon_length_angle_metrics(&points)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid red-green parent"))?;
    if metrics.angles_degrees.iter().any(|a| !a.is_finite()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-finite red-green angle",
        ));
    }
    let max = metrics.angles_degrees.iter().copied().fold(0.0, f64::max);
    let min = metrics.angles_degrees.into_iter().fold(180.0, f64::min);
    if !min.is_finite() || min <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "degenerate red-green face",
        ));
    }
    Ok((min, max))
}

fn min_angle(points: [LonLatDegrees; 3]) -> io::Result<f64> {
    Ok(angle_range(points)?.0)
}

/// Green closure retains half the minimum angle of the red-leaf mesh. This
/// shape floor is inherited from the source geometry, NOT a 40–80 contract.
fn green_is_safe(
    points: [LonLatDegrees; 3],
    edge: usize,
    minimum: f64,
    maximum: f64,
) -> io::Result<bool> {
    let [a, b, c] = [points[edge], points[(edge + 1) % 3], points[(edge + 2) % 3]];
    let m = midpoint_lonlat(a, b)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "antipodal split edge"))?;
    let (first, second) = (angle_range([a, m, c])?, angle_range([m, b, c])?);
    Ok(first.0.min(second.0) + 1.0e-8 >= minimum && first.1.max(second.1) <= maximum + 1.0e-8)
}

fn edge(a: usize, b: usize) -> (usize, usize) {
    (a.min(b), a.max(b))
}

fn needs_red(
    corners: [usize; 3],
    midpoints: &HashMap<(usize, usize), usize>,
    points: &[LonLatDegrees],
    minimum: f64,
    maximum: f64,
) -> io::Result<bool> {
    let hanging = (0..3)
        .filter(|&e| midpoints.contains_key(&edge(corners[e], corners[(e + 1) % 3])))
        .collect::<Vec<_>>();
    match hanging.as_slice() {
        [] => Ok(false),
        &[e] => {
            let (a, b) = (corners[e], corners[(e + 1) % 3]);
            let m = midpoints[&edge(a, b)];
            Ok(midpoints.contains_key(&edge(a, m))
                || midpoints.contains_key(&edge(m, b))
                || !green_is_safe(corners.map(|v| points[v]), e, minimum, maximum)?)
        }
        _ => Ok(true),
    }
}

/// Use exact spherical triangle areas, not nominal generation differences.
/// Returns (larger-side face marks, number of violating edges, maximum scale ratio).
/// Open patch boundaries are ignored here; the conforming driver separately
/// requires a closed sphere before and after any refinement transaction.
pub fn triangle_balance_marks(
    points: &[LonLatDegrees],
    triangles: &[[usize; 3]],
    first_physical_face: usize,
) -> io::Result<(Vec<i32>, usize, f64)> {
    if first_physical_face > triangles.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid first physical face",
        ));
    }
    let mut marks = vec![0; triangles.len()];
    let mut edges = HashMap::new();
    let mut warnings = 0;
    let mut maximum = 1.0_f64;
    for (face, corners) in triangles.iter().enumerate().skip(first_physical_face) {
        let [a, b, c] = corners.map(|v| points.get(v).copied());
        let (Some(a), Some(b), Some(c)) = (a, b, c) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "missing triangle vertex",
            ));
        };
        let area = earthmesh_mesh::spherical_triangle_area_unit(
            [a, b, c].map(earthmesh_mesh::lonlat_degrees_to_unit_xyz),
        );
        if !area.is_finite() || area <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid area in red-green balance",
            ));
        }
        for e in 0..3 {
            let key = edge(corners[e], corners[(e + 1) % 3]);
            if let Some((other, other_area)) = edges.remove(&key) {
                let ratio = (area.max(other_area) / area.min(other_area)).sqrt();
                maximum = maximum.max(ratio);
                if ratio > 2.0 {
                    warnings += 1;
                    marks[if area > other_area { face } else { other }] = 1;
                }
            } else {
                edges.insert(key, (face, area));
            }
        }
    }
    Ok((marks, warnings, maximum))
}

fn balance_marks(mesh: &RedGreenMesh) -> io::Result<(Vec<i32>, usize, f64)> {
    triangle_balance_marks(
        &mesh.cell_points,
        &mesh.cells_on_triangle,
        mesh.num_vertex + 1,
    )
}

fn mesh_angle_range(mesh: &RedGreenMesh) -> io::Result<(f64, f64)> {
    let mut range = (180.0_f64, 0.0_f64);
    for corners in mesh.cells_on_triangle.iter().skip(mesh.num_vertex + 1) {
        let angles = angle_range(corners.map(|v| mesh.cell_points[v]))?;
        range.0 = range.0.min(angles.0);
        range.1 = range.1.max(angles.1);
    }
    Ok(range)
}

pub(super) fn refine(
    mesh: &RedGreenMesh,
    marking: &[i32],
    neighbors: &[Vec<usize>],
    halo_cancelled_count: usize,
    absolute_minimum: f64,
) -> io::Result<RedGreenOutcome> {
    let baseline = refine_conforming(
        mesh,
        marking,
        neighbors,
        halo_cancelled_count,
        absolute_minimum,
        None,
        usize::MAX,
    )?;
    let count = baseline.mesh.triangle_count() - baseline.mesh.num_vertex;
    // ponytail: bounded local repair, not a new global refinement pass. A cascade
    // beyond 10% (at least 64 faces for small cases) is rolled back, not hidden.
    let budget = count.saturating_add((count / 10).max(64));
    repair_balance(baseline, absolute_minimum, budget)
}

fn repair_balance(
    mut baseline: RedGreenOutcome,
    absolute_minimum: f64,
    budget: usize,
) -> io::Result<RedGreenOutcome> {
    let (mut marks, warnings, _) = balance_marks(&baseline.mesh)?;
    baseline.balance_repair = Some(RedGreenBalanceReport {
        initial_warning_count: warnings,
        remaining_warning_count: warnings,
        added_triangle_count: 0,
        rejection_reason: None,
    });
    if warnings == 0 {
        return Ok(baseline);
    }
    let window = mesh_angle_range(&baseline.mesh)?;
    let count = baseline.mesh.triangle_count() - baseline.mesh.num_vertex;
    let mut candidate = baseline.clone();
    let rejected = loop {
        let neighbors = super::triangle_neighbor_rows(&candidate.mesh)?;
        let mut next = match refine_conforming(
            &candidate.mesh,
            &marks,
            &neighbors,
            0,
            absolute_minimum,
            Some(window),
            budget,
        ) {
            Ok(next) => next,
            Err(error) => break error.to_string(),
        };
        let new_count = next.mesh.triangle_count() - next.mesh.num_vertex;
        let angles = match mesh_angle_range(&next.mesh) {
            Ok(angles) => angles,
            Err(error) => break error.to_string(),
        };
        if new_count > budget
            || new_count <= candidate.mesh.triangle_count() - candidate.mesh.num_vertex
        {
            break "physical balance exceeded its local growth budget or made no progress"
                .to_string();
        }
        if angles.0 < window.0 - 1.0e-8 || angles.1 > window.1 + 1.0e-8 {
            break "physical balance worsened the baseline triangle angle envelope".to_string();
        }
        next.grown_triangle_count = candidate.grown_triangle_count + next.refined_triangle_count;
        next.refined_triangle_count += candidate.refined_triangle_count;
        next.halo_cancelled_count = baseline.halo_cancelled_count;
        next.cell_renumbering.clone_from(&baseline.cell_renumbering);
        let (next_marks, remaining, _) = match balance_marks(&next.mesh) {
            Ok(audit) => audit,
            Err(error) => break error.to_string(),
        };
        next.balance_repair = Some(RedGreenBalanceReport {
            initial_warning_count: warnings,
            remaining_warning_count: remaining,
            added_triangle_count: new_count - count,
            rejection_reason: None,
        });
        if remaining == 0 {
            return Ok(next);
        }
        candidate = next;
        marks = next_marks;
    };
    baseline.balance_repair.as_mut().unwrap().rejection_reason = Some(rejected);
    Ok(baseline)
}

fn refine_conforming(
    mesh: &RedGreenMesh,
    marking: &[i32],
    neighbors: &[Vec<usize>],
    halo_cancelled_count: usize,
    absolute_minimum: f64,
    balance_window: Option<(f64, f64)>,
    face_budget: usize,
) -> io::Result<RedGreenOutcome> {
    if !absolute_minimum.is_finite() || !(0.0..60.0).contains(&absolute_minimum) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "red-green triangle angle floor must be finite and in [0, 60) degrees",
        ));
    }
    let start = mesh.num_vertex + 1;
    if neighbors.iter().skip(start).any(|row| row.contains(&0)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green input has an open edge",
        ));
    }
    let mut parents = mesh.cells_on_triangle.clone();
    let mut levels = mesh.refinement_levels.clone();
    if levels.len() != parents.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green refinement levels have stale numbering",
        ));
    }
    let target = levels
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(usize::from(balance_window.is_none()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "red-green refinement depth overflow",
            )
        })?;
    let mut points = mesh.cell_points.clone();
    // Never iterate this table: face order alone assigns midpoint ids, so hash
    // randomization cannot change the mesh.
    let mut midpoints = HashMap::new();
    let mut requested = marking[..parents.len()].to_vec();
    // Roll back temporary green pairs, retaining their midpoint: the red side
    // of the interface still needs it. Red ancestors are never coarsened.
    for &(corners, children) in &mesh.green_parents {
        let [first, second] = children;
        if first < start
            || second >= parents.len()
            || first >= second
            || parents[first] == [1; 3]
            || parents[second] == [1; 3]
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid red-green ancestry",
            ));
        }
        if corners
            .iter()
            .any(|&v| v <= mesh.num_center || v >= points.len())
            || corners[0] == corners[1]
            || corners[1] == corners[2]
            || corners[2] == corners[0]
            || levels[first] != levels[second]
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid green parent corners/depth",
            ));
        }
        let common = parents[first]
            .iter()
            .copied()
            .filter(|v| parents[second].contains(v))
            .collect::<Vec<_>>();
        if common.len() != 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "green siblings must share one edge",
            ));
        }
        let midpoint = common
            .iter()
            .copied()
            .find(|v| !corners.contains(v))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "green pair has no midpoint")
            })?;
        let opposite = common
            .iter()
            .copied()
            .find(|v| corners.contains(v))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "green pair has no opposite corner",
                )
            })?;
        let ends = corners
            .into_iter()
            .filter(|v| *v != opposite)
            .collect::<Vec<_>>();
        let mut expected = [
            vec![opposite, ends[0], midpoint],
            vec![opposite, ends[1], midpoint],
        ];
        let mut actual = [parents[first].to_vec(), parents[second].to_vec()];
        for c in expected.iter_mut().chain(actual.iter_mut()) {
            c.sort_unstable();
        }
        expected.sort();
        actual.sort();
        if actual != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "green ancestry does not match the delivered children",
            ));
        }
        if midpoints
            .insert(edge(ends[0], ends[1]), midpoint)
            .is_some_and(|old| old != midpoint)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "conflicting green midpoint ids",
            ));
        }
        parents[first] = corners;
        parents[second] = [1; 3];
        requested[first] |= requested[second];
        requested[second] = 0;
    }
    let mut minimum = 180.0_f64;
    for corners in parents.iter().skip(start).filter(|&&c| c != [1; 3]) {
        minimum = minimum.min(0.5 * min_angle(corners.map(|v| points[v]))?);
    }
    minimum = minimum
        .max(absolute_minimum)
        .max(balance_window.map_or(0.0, |w| w.0));
    let maximum = balance_window.map_or(180.0, |w| w.1);
    let mut active_parents = parents.iter().skip(start).filter(|&&c| c != [1; 3]).count();
    let mut pending = (start..parents.len())
        .filter(|&f| requested[f] == 1)
        .collect::<Vec<_>>();
    let asked = pending.len();
    let mut required = requested
        .iter()
        .enumerate()
        .map(|(face, &mark)| {
            if mark == 1 {
                if balance_window.is_some() {
                    levels[face].saturating_add(1)
                } else {
                    target
                }
            } else {
                0
            }
        })
        .collect::<Vec<_>>();
    let mut refined = 0;
    loop {
        for face in pending.drain(..) {
            if active_parents.saturating_add(3) > face_budget {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "physical balance exceeded its local face budget",
                ));
            }
            active_parents += 3;
            let corners = parents[face];
            let mut mids = [0; 3];
            for e in 0..3 {
                let key = edge(corners[e], corners[(e + 1) % 3]);
                mids[e] = if let Some(&id) = midpoints.get(&key) {
                    id
                } else {
                    let p = midpoint_lonlat(points[key.0], points[key.1]).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "antipodal red-green edge")
                    })?;
                    let id = points.len();
                    points.push(p);
                    midpoints.insert(key, id);
                    id
                };
            }
            let [a, b, c] = corners;
            let [ab, bc, ca] = mids;
            let next_level = levels[face].checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "red-green depth overflow")
            })?;
            if next_level > target {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "red-green closure would exceed the requested refinement depth",
                ));
            }
            parents[face] = [a, ab, ca];
            levels[face] = next_level;
            parents.extend([[ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            levels.extend([next_level; 3]);
            required.extend([required[face]; 3]);
            refined += 1;
        }
        // ponytail: O(faces * closure waves) batch scans avoid a second mutable
        // adjacency index; use an edge-local queue if profiles show this dominates.
        // All split decisions use red leaves, never already-cut green children.
        // A second hanging edge or a recursively split edge needs a red split.
        for face in start..parents.len() {
            let corners = parents[face];
            if corners == [1; 3] {
                continue;
            }
            if levels[face] < required[face]
                || needs_red(corners, &midpoints, &points, minimum, maximum)?
            {
                pending.push(face);
            }
        }
        if pending.is_empty() {
            break;
        }
    }
    let mut out = RedGreenMesh {
        num_vertex: mesh.num_vertex,
        num_center: mesh.num_center,
        cell_points: points,
        triangle_points: mesh.triangle_points[..start].to_vec(),
        cells_on_triangle: mesh.cells_on_triangle[..start].to_vec(),
        triangles_on_cell: Vec::new(),
        n_triangles_on_cell: Vec::new(),
        green_parents: Vec::new(),
        refinement_levels: vec![0; start],
    };
    let mut interior_marks = vec![0; start];
    for face in start..parents.len() {
        let corners = parents[face];
        if corners == [1; 3] {
            continue;
        }
        let hanging =
            (0..3).find(|&e| midpoints.contains_key(&edge(corners[e], corners[(e + 1) % 3])));
        let children = if let Some(e) = hanging {
            let (a, b, c) = (corners[e], corners[(e + 1) % 3], corners[(e + 2) % 3]);
            let m = midpoints[&edge(a, b)];
            let first = out.cells_on_triangle.len();
            out.green_parents.push((corners, [first, first + 1]));
            vec![[a, m, c], [m, b, c]]
        } else {
            vec![corners]
        };
        for child in children {
            out.triangle_points.push(
                earthmesh_mesh::spherical_centroid_degrees(&child.map(|v| out.cell_points[v]))
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "degenerate red-green child")
                    })?,
            );
            out.cells_on_triangle.push(child);
            out.refinement_levels.push(levels[face]);
            interior_marks.push(i32::from(hanging.is_none() && levels[face] == target));
        }
    }
    orient_triangles_outward(&out.cell_points, &mut out.cells_on_triangle)?;
    out.triangles_on_cell = vec![Vec::new(); out.cell_points.len()];
    for (face, corners) in out.cells_on_triangle.iter().enumerate().skip(start) {
        for &v in corners {
            out.triangles_on_cell[v].push(face);
        }
    }
    out.n_triangles_on_cell = out.triangles_on_cell.iter().map(Vec::len).collect();
    crate::get_sort_new_one_based(
        out.cell_count(),
        &out.n_triangles_on_cell,
        &out.cells_on_triangle,
        &out.triangle_points,
        &mut out.triangles_on_cell,
    )?;
    let closed = super::triangle_neighbor_rows(&out)?;
    if closed.iter().skip(start).any(|row| row.contains(&0)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "red-green closure left an open edge",
        ));
    }
    Ok(RedGreenOutcome {
        balance_repair: None,
        mesh: out,
        interior_marks,
        refined_triangle_count: refined,
        grown_triangle_count: refined - asked,
        isolated_dropped_count: 0,
        halo_cancelled_count,
        flipped_triangle_count: 0,
        weak_concavity_count: 0,
        cell_renumbering: (0..=mesh.cell_count()).collect(),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_27_real_ocean_pairs_mark_only_the_larger_side() {
        let fixture = include_str!("../../tests/fixtures/nxp81_ratio_pairs.csv");
        let mut pairs = 0;
        for line in fixture.lines().skip(1) {
            let values = line
                .split(',')
                .map(|x| x.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(values.len(), 12);
            let mut points = vec![LonLatDegrees::new(0.0, 0.0); 2];
            points.extend((0..4).map(|i| LonLatDegrees::new(values[2 * i], values[2 * i + 1])));
            let mesh = RedGreenMesh {
                num_vertex: 1,
                num_center: 1,
                cell_points: points,
                triangle_points: vec![],
                cells_on_triangle: vec![
                    [1; 3],
                    [1; 3],
                    [2, 3, 4],
                    [
                        values[8] as usize + 2,
                        values[9] as usize + 2,
                        values[10] as usize + 2,
                    ],
                ],
                triangles_on_cell: vec![],
                n_triangles_on_cell: vec![],
                green_parents: vec![],
                refinement_levels: vec![],
            };
            let (marks, warnings, ratio) = balance_marks(&mesh).unwrap();
            assert_eq!(warnings, 1);
            assert_eq!(marks, [0, 0, 1, 0]);
            assert!((ratio - values[11]).abs() < 1.0e-9);
            pairs += 1;
        }
        assert_eq!(pairs, 27);
    }

    #[test]
    fn physical_balance_is_local_deterministic_and_rolls_back_on_budget() {
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
        let mut mesh = crate::redgreen_mesh_from_triangular(&base, &base.m_neighbors).unwrap();
        let mut previous: Option<Vec<i32>> = None;
        let mut baseline = None;
        for _ in 0..2 {
            let mut marks = mesh
                .triangle_points
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    i32::from(
                        i > mesh.num_vertex
                            && p.lon_degrees.abs() < 35.0
                            && p.lat_degrees.abs() < 30.0,
                    )
                })
                .collect::<Vec<_>>();
            let cancelled = if let Some(previous) = &previous {
                super::super::cancel_marks_outside_halo(&mesh, previous, 3, &mut marks).unwrap()
            } else {
                0
            };
            let neighbors = super::super::triangle_neighbor_rows(&mesh).unwrap();
            let out =
                refine_conforming(&mesh, &marks, &neighbors, cancelled, 25.0, None, usize::MAX)
                    .unwrap();
            previous = Some(out.interior_marks.clone());
            mesh = out.mesh.clone();
            baseline = Some(out);
        }
        let baseline = baseline.unwrap();
        assert_eq!(balance_marks(&baseline.mesh).unwrap().1, 2);
        let count = baseline.mesh.triangle_count() - baseline.mesh.num_vertex;
        let refused = repair_balance(baseline.clone(), 25.0, count).unwrap();
        assert_eq!(refused.mesh, baseline.mesh);
        assert_eq!(refused.interior_marks, baseline.interior_marks);
        let report = refused.balance_repair.unwrap();
        assert_eq!(report.remaining_warning_count, 2);
        assert_eq!(report.added_triangle_count, 0);
        assert!(report.rejection_reason.unwrap().contains("budget"));
        let repaired = repair_balance(baseline.clone(), 25.0, count + count / 10).unwrap();
        let report = repaired.balance_repair.as_ref().unwrap();
        assert_eq!(report.remaining_warning_count, 0, "{report:?}");
        assert!(report.added_triangle_count > 0 && report.added_triangle_count <= count / 10);
        assert_eq!(
            repair_balance(baseline.clone(), 25.0, count + count / 10).unwrap(),
            repaired
        );
        let old_angles = mesh_angle_range(&baseline.mesh).unwrap();
        let new_angles = mesh_angle_range(&repaired.mesh).unwrap();
        assert!(new_angles.0 >= old_angles.0 - 1.0e-8 && new_angles.1 <= old_angles.1 + 1.0e-8);
        assert_eq!(repaired.mesh.refinement_levels.iter().max(), Some(&2));
        assert_eq!(
            &repaired.mesh.cell_points[..baseline.mesh.cell_points.len()],
            &baseline.mesh.cell_points
        );
        let canonical = |mut face: [usize; 3]| {
            face.sort_unstable();
            face
        };
        let accepted_faces = repaired
            .mesh
            .cells_on_triangle
            .iter()
            .copied()
            .map(canonical)
            .collect::<std::collections::HashSet<_>>();
        for (&face, &demanded_interior) in baseline
            .mesh
            .cells_on_triangle
            .iter()
            .zip(&baseline.interior_marks)
        {
            if demanded_interior == 1 {
                assert!(
                    accepted_faces.contains(&canonical(face)),
                    "settled finest demand must not be coarsened"
                );
            }
        }
        let area = repaired
            .mesh
            .cells_on_triangle
            .iter()
            .skip(2)
            .map(|c| {
                earthmesh_mesh::spherical_triangle_area_unit(c.map(|v| {
                    earthmesh_mesh::lonlat_degrees_to_unit_xyz(repaired.mesh.cell_points[v])
                }))
            })
            .sum::<f64>();
        assert!((area - 4.0 * std::f64::consts::PI).abs() < 1.0e-9);
    }

    #[test]
    fn the_nxp80_short_edge_split_is_promoted_before_creating_24_degree_children() {
        let points = [
            LonLatDegrees::new(35.49706099390156, -24.676768668287018),
            LonLatDegrees::new(35.24368285502307, -24.990301092143007),
            LonLatDegrees::new(35.7478872719528, -24.99202032031854),
        ];
        assert!(min_angle(points).unwrap() > 54.0);
        assert!(!green_is_safe(points, 0, 0.5 * min_angle(points).unwrap(), 180.0).unwrap());
        assert!(green_is_safe(points, 1, 0.5 * min_angle(points).unwrap(), 180.0).unwrap());
    }
    #[test]
    fn two_hanging_edges_of_the_123_degree_nxp80_parent_require_red() {
        let mut points = vec![
            LonLatDegrees::new(-79.1148751478424, 23.301941669376788),
            LonLatDegrees::new(-79.01932336882689, 23.09056029998553),
            LonLatDegrees::new(-79.23444189921554, 23.13700174821615),
        ];
        // The old sequential repair created A, midpoint(B,C), midpoint(A,C).
        points.push(midpoint_lonlat(points[1], points[2]).unwrap());
        points.push(midpoint_lonlat(points[0], points[2]).unwrap());
        let old = earthmesh_mesh::polygon_length_angle_metrics(&[points[0], points[3], points[4]])
            .unwrap();
        assert!(old.angles_degrees.iter().any(|&a| a > 123.7));
        let mids = HashMap::from([(edge(1, 2), 3), (edge(0, 2), 4)]);
        assert!(needs_red([0, 1, 2], &mids, &points, 27.0, 180.0).unwrap());
        points.push(midpoint_lonlat(points[0], points[1]).unwrap());
        for child in [[0, 5, 4], [5, 1, 3], [4, 3, 2], [5, 3, 4]] {
            let angles =
                earthmesh_mesh::polygon_length_angle_metrics(&child.map(|v| points[v])).unwrap();
            assert!(angles
                .angles_degrees
                .iter()
                .all(|&a| (54.20..69.53).contains(&a)));
        }
    }
}

/// Counterexample to enabling a tighter Green window as an angle "fix".
/// This is test-only: production keeps the existing physical-balance repair.
#[cfg(test)]
#[test]
fn tighter_green_templates_can_improve_extrema_but_increase_bad_faces() {
    let base = earthmesh_mesh::TriangularMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
    let mut mesh = crate::redgreen_mesh_from_triangular(&base, &base.m_neighbors).unwrap();
    let settings = crate::RedGreenSettings {
        protect_triangle_quality: true,
        min_triangle_angle_deg: 25.0,
        ..Default::default()
    };
    let mut previous = None;
    for _ in 1..=3 {
        let marks = mesh
            .triangle_points
            .iter()
            .enumerate()
            .map(|(i, p)| {
                i32::from(
                    i > mesh.num_vertex && p.lon_degrees.abs() < 35.0 && p.lat_degrees.abs() < 30.0,
                )
            })
            .collect::<Vec<_>>();
        let out =
            crate::refine_redgreen_round_inside(&mesh, &marks, &settings, previous.as_deref())
                .unwrap();
        previous = Some(out.interior_marks);
        mesh = out.mesh;
    }
    let original = mesh.clone();
    let count = mesh.triangle_count() - mesh.num_vertex;
    let budget = count + count / 10;
    let neighbors = super::triangle_neighbor_rows(&mesh).unwrap();
    let marks = vec![0; mesh.cells_on_triangle.len()];
    let strict = refine_conforming(
        &mesh,
        &marks,
        &neighbors,
        0,
        25.0,
        Some((40.0, 80.0)),
        budget,
    );
    assert!(strict.unwrap_err().to_string().contains("face budget"));

    let candidate = refine_conforming(
        &mesh,
        &marks,
        &neighbors,
        0,
        25.0,
        Some((26.0, 102.0)),
        budget,
    )
    .unwrap();
    let candidate = repair_balance(candidate, 25.0, budget).unwrap().mesh;
    let before = mesh_angle_range(&mesh).unwrap();
    let after = mesh_angle_range(&candidate).unwrap();
    let bad_faces = |mesh: &RedGreenMesh| {
        mesh.cells_on_triangle
            .iter()
            .skip(mesh.num_vertex + 1)
            .filter(|c| {
                let (min, max) = angle_range(c.map(|v| mesh.cell_points[v])).unwrap();
                min < 40.0 || max > 80.0
            })
            .count()
    };
    eprintln!("tighter Green candidate: {count} -> {} faces, {before:?} -> {after:?}, strict-window violations {} -> {}",
        candidate.triangle_count()-candidate.num_vertex, bad_faces(&mesh), bad_faces(&candidate));
    assert!(after.0 > before.0 && after.1 < before.1);
    assert!(
        bad_faces(&candidate) > bad_faces(&mesh),
        "better extrema alone must not be mistaken for strict-angle improvement"
    );
    assert_eq!(balance_marks(&candidate).unwrap().1, 0);
    assert!(candidate.triangle_count() - candidate.num_vertex <= budget);
    assert_eq!(candidate.refinement_levels.iter().max(), Some(&3));
    assert_eq!(
        &candidate.cell_points[..mesh.cell_points.len()],
        &mesh.cell_points
    );
    let canonical = |mut c: [usize; 3]| {
        c.sort_unstable();
        c
    };
    let final_faces = candidate
        .cells_on_triangle
        .iter()
        .copied()
        .map(canonical)
        .collect::<std::collections::HashSet<_>>();
    for (i, &c) in mesh
        .cells_on_triangle
        .iter()
        .enumerate()
        .skip(mesh.num_vertex + 1)
    {
        if mesh.refinement_levels[i] == 3 && previous.as_ref().unwrap()[i] == 1 {
            assert!(
                final_faces.contains(&canonical(c)),
                "lost finest demanded face {i}"
            );
        }
    }
    assert_eq!(
        mesh, original,
        "both rejected experiments leave the input untouched"
    );
}
