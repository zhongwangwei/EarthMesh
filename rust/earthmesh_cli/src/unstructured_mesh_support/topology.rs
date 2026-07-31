use std::io;

use super::indexing::{
    mesh_canonical_id_for_row, mesh_points_have_two_placeholder_rows, mesh_row_for_canonical_id,
};
use super::{UnstructuredMesh, UnstructuredMeshTopologyReport};

pub(crate) fn validate_unstructured_mesh(mesh: &UnstructuredMesh) -> io::Result<()> {
    if mesh.m_to_w.len() != mesh.m_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "m_to_w length must match m_points length",
        ));
    }
    if mesh.w_to_m.len() != mesh.w_points.len() || mesh.n_w_to_m.len() != mesh.w_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "w_to_m and n_w_to_m lengths must match w_points length",
        ));
    }
    if mesh.n_w_to_m.iter().any(|&n| n < 0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "n_w_to_m values must be non-negative",
        ));
    }
    Ok(())
}

fn triangle_quality_input(
    mesh: &UnstructuredMesh,
    m_has_two_placeholders: bool,
    w_has_two_placeholders: bool,
) -> io::Result<earthmesh_quality::QualityMeshInput> {
    let mut dense_vertex_for_row = vec![None; mesh.w_points.len()];
    let mut vertices = Vec::new();
    for (row, point) in mesh.w_points.iter().enumerate() {
        let Some(id) = mesh_canonical_id_for_row(row, w_has_two_placeholders) else {
            continue;
        };
        if id <= 1 {
            continue;
        }
        dense_vertex_for_row[row] = Some(vertices.len());
        vertices.push(earthmesh_geometry::Point::new(point.lon, point.lat));
    }
    let mut cells = Vec::new();
    for (row, triangle) in mesh.m_to_w.iter().enumerate() {
        let Some(id) = mesh_canonical_id_for_row(row, m_has_two_placeholders) else {
            continue;
        };
        if id <= 1 {
            continue;
        }
        let vertices = triangle
            .iter()
            .map(|&w_id| {
                let w_row =
                    mesh_row_for_canonical_id(w_id, mesh.w_points.len(), w_has_two_placeholders)
                        .ok_or_else(|| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("triangle row {row} contains invalid W vertex id {w_id}"),
                            )
                        })?;
                dense_vertex_for_row[w_row].ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("triangle row {row} references placeholder W vertex id {w_id}"),
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        cells.push(earthmesh_quality::QualityCell {
            vertices,
            refine_level: None,
            neighbors: Vec::new(),
        });
    }
    Ok(earthmesh_quality::QualityMeshInput { vertices, cells })
}

pub fn check_unstructured_mesh_topology(mesh: &UnstructuredMesh) -> UnstructuredMeshTopologyReport {
    let mut violations = Vec::new();
    if let Err(err) = validate_unstructured_mesh(mesh) {
        violations.push(err.to_string());
        return UnstructuredMeshTopologyReport {
            m_rows: mesh.m_points.len(),
            w_rows: mesh.w_points.len(),
            boundary_loop_count: 0,
            boundary_vertex_degree_violation_count: 0,
            euler_characteristic: None,
            expected_euler_characteristic: None,
            violations,
        };
    }

    let m_rows = mesh.m_points.len();
    let w_rows = mesh.w_points.len();
    let m_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let w_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let cap = 80usize;
    let push_violation = |violations: &mut Vec<String>, message: String| {
        if violations.len() < cap {
            violations.push(message);
        }
    };

    for (m_row, w_ids) in mesh.m_to_w.iter().enumerate() {
        let Some(m_id) = mesh_canonical_id_for_row(m_row, m_has_two_placeholders) else {
            continue;
        };
        for (slot, &w_id) in w_ids.iter().enumerate() {
            let Some(w_row) = mesh_row_for_canonical_id(w_id, w_rows, w_has_two_placeholders)
            else {
                push_violation(
                    &mut violations,
                    format!("m row {m_row} slot {slot} canonicals invalid w id {w_id}"),
                );
                continue;
            };
            let count = mesh.n_w_to_m.get(w_row).copied().unwrap_or_default().max(0) as usize;
            if count > mesh.w_to_m[w_row].len() {
                push_violation(
                    &mut violations,
                    format!(
                        "w row {w_row} n_w_to_m {count} exceeds row width {}",
                        mesh.w_to_m[w_row].len()
                    ),
                );
                continue;
            }
            if m_id > 1 && !mesh.w_to_m[w_row].iter().take(count).any(|&id| id == m_id) {
                push_violation(
                    &mut violations,
                    format!(
                        "m row {m_row} -> w id {w_id}, but w row {w_row} does not list m id {m_id}"
                    ),
                );
            }
        }
    }

    for (w_row, m_ids) in mesh.w_to_m.iter().enumerate() {
        let Some(w_id) = mesh_canonical_id_for_row(w_row, w_has_two_placeholders) else {
            continue;
        };
        let count = mesh.n_w_to_m[w_row].max(0) as usize;
        if count > m_ids.len() {
            push_violation(
                &mut violations,
                format!(
                    "w row {w_row} n_w_to_m {count} exceeds row width {}",
                    m_ids.len()
                ),
            );
            continue;
        }
        for (slot, &m_id) in m_ids.iter().take(count).enumerate() {
            let Some(m_row) = mesh_row_for_canonical_id(m_id, m_rows, m_has_two_placeholders)
            else {
                push_violation(
                    &mut violations,
                    format!("w row {w_row} slot {slot} canonicals invalid m id {m_id}"),
                );
                continue;
            };
            if w_id > 1 && !mesh.m_to_w[m_row].contains(&w_id) {
                push_violation(
                    &mut violations,
                    format!(
                        "w row {w_row} -> m id {m_id}, but m row {m_row} does not list w id {w_id}"
                    ),
                );
            }
        }
    }

    let mut boundary_loop_count = 0;
    let mut boundary_vertex_degree_violation_count = 0;
    let mut euler_characteristic = None;
    let mut expected_euler_characteristic = None;
    match triangle_quality_input(mesh, m_has_two_placeholders, w_has_two_placeholders) {
        Ok(input) => {
            let boundary = earthmesh_quality::topology::boundary_topology(&input);
            boundary_loop_count = boundary.loops.len();
            boundary_vertex_degree_violation_count = boundary.invalid_vertex_degrees.len();
            euler_characteristic = Some(earthmesh_quality::topology::euler_characteristic(&input));
            expected_euler_characteristic =
                earthmesh_quality::topology::genus_zero_euler_expectation(&input, &boundary);
            let validator = earthmesh_quality::topology::MeshTopologyValidator::new(&input);
            for issue in validator.validate_boundary_contract() {
                push_violation(
                    &mut violations,
                    format!("{}: {}", issue.issue_type.as_str(), issue.message),
                );
            }
        }
        Err(error) => push_violation(&mut violations, error.to_string()),
    }

    UnstructuredMeshTopologyReport {
        m_rows,
        w_rows,
        boundary_loop_count,
        boundary_vertex_degree_violation_count,
        euler_characteristic,
        expected_euler_characteristic,
        violations,
    }
}

/// Split coincident triangle fans into separate vertex ids without changing cells.
///
/// Returns the source row for each appended W vertex so optional per-vertex
/// metadata can copy the same value.
pub(crate) fn split_non_manifold_triangle_vertex_fans(
    mesh: &mut UnstructuredMesh,
) -> io::Result<Vec<usize>> {
    validate_unstructured_mesh(mesh)?;
    let m_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.m_points);
    let w_has_two_placeholders = mesh_points_have_two_placeholder_rows(&mesh.w_points);
    let original_w_rows = mesh.w_points.len();
    let mut duplicate_sources = Vec::new();

    for w_row in 0..original_w_rows {
        let Some(w_id) = mesh_canonical_id_for_row(w_row, w_has_two_placeholders) else {
            continue;
        };
        if w_id <= 1 {
            continue;
        }
        let count = usize::try_from(mesh.n_w_to_m[w_row])
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "negative n_w_to_m value"))?;
        if count > mesh.w_to_m[w_row].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("w row {w_row} n_w_to_m exceeds row width"),
            ));
        }
        let mut incident_rows = Vec::new();
        for &m_id in mesh.w_to_m[w_row].iter().take(count) {
            let Some(m_row) =
                mesh_row_for_canonical_id(m_id, mesh.m_points.len(), m_has_two_placeholders)
            else {
                continue;
            };
            if m_id > 1 && !incident_rows.contains(&m_row) {
                incident_rows.push(m_row);
            }
        }
        if incident_rows.len() <= 1 {
            continue;
        }

        let mut assigned = vec![false; incident_rows.len()];
        let mut components = Vec::<Vec<usize>>::new();
        for start in 0..incident_rows.len() {
            if assigned[start] {
                continue;
            }
            assigned[start] = true;
            let mut stack = vec![start];
            let mut component = Vec::new();
            while let Some(index) = stack.pop() {
                let m_row = incident_rows[index];
                component.push(m_row);
                for candidate in 0..incident_rows.len() {
                    if assigned[candidate] {
                        continue;
                    }
                    let candidate_row = incident_rows[candidate];
                    if mesh.m_to_w[m_row].iter().any(|&candidate_w_id| {
                        candidate_w_id != w_id
                            && mesh.m_to_w[candidate_row].contains(&candidate_w_id)
                    }) {
                        assigned[candidate] = true;
                        stack.push(candidate);
                    }
                }
            }
            components.push(component);
        }
        if components.len() <= 1 {
            continue;
        }
        components.sort_by_key(|component| component.iter().copied().min().unwrap_or(usize::MAX));

        for component in components.iter().skip(1) {
            let new_w_row = mesh.w_points.len();
            let new_w_id = mesh_canonical_id_for_row(new_w_row, w_has_two_placeholders)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "too many W vertices")
                })?;
            mesh.w_points.push(mesh.w_points[w_row]);
            duplicate_sources.push(w_row);
            for &m_row in component {
                let slot = mesh.m_to_w[m_row]
                    .iter()
                    .position(|&candidate| candidate == w_id)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("m row {m_row} does not contain split W vertex {w_id}"),
                        )
                    })?;
                mesh.m_to_w[m_row][slot] = new_w_id;
            }
        }
    }

    if duplicate_sources.is_empty() {
        return Ok(duplicate_sources);
    }
    rebuild_triangle_vertex_neighbors(mesh, m_has_two_placeholders, w_has_two_placeholders)?;
    validate_unstructured_mesh(mesh)?;
    Ok(duplicate_sources)
}

fn rebuild_triangle_vertex_neighbors(
    mesh: &mut UnstructuredMesh,
    m_has_two_placeholders: bool,
    w_has_two_placeholders: bool,
) -> io::Result<()> {
    let width = unstructured_dimc(mesh);
    let mut w_to_m = vec![vec![1; width]; mesh.w_points.len()];
    let mut n_w_to_m = vec![0i32; mesh.w_points.len()];
    for m_row in 0..mesh.m_to_w.len() {
        let Some(m_id) = mesh_canonical_id_for_row(m_row, m_has_two_placeholders) else {
            continue;
        };
        if m_id <= 1 {
            continue;
        }
        for &w_id in &mesh.m_to_w[m_row] {
            let w_row =
                mesh_row_for_canonical_id(w_id, mesh.w_points.len(), w_has_two_placeholders)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("m row {m_row} canonicals invalid W vertex {w_id}"),
                        )
                    })?;
            let slot = usize::try_from(n_w_to_m[w_row]).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "negative rebuilt W degree")
            })?;
            if slot >= width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("W vertex {w_id} exceeds neighbor width {width}"),
                ));
            }
            w_to_m[w_row][slot] = m_id;
            n_w_to_m[w_row] += 1;
        }
    }
    for w_row in 0..w_to_m.len() {
        let count = usize::try_from(n_w_to_m[w_row]).unwrap_or(0);
        if count > 0 && count < width {
            let fill = w_to_m[w_row][0];
            w_to_m[w_row][count..].fill(fill);
        }
    }
    mesh.w_to_m = w_to_m;
    mesh.n_w_to_m = n_w_to_m;
    Ok(())
}

pub(crate) fn unstructured_dimc(mesh: &UnstructuredMesh) -> usize {
    mesh.n_w_to_m
        .iter()
        .filter_map(|&value| usize::try_from(value).ok())
        .chain(mesh.w_to_m.iter().map(Vec::len))
        .max()
        .unwrap_or(0)
        .max(7)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LonLatPoint;

    fn two_triangle_disk() -> UnstructuredMesh {
        let point = |lon, lat| LonLatPoint { lon, lat };
        UnstructuredMesh {
            m_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.6, 0.3),
                point(0.3, 0.6),
            ],
            w_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(1.0, 1.0),
                point(0.0, 1.0),
            ],
            m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 4, 5]],
            w_to_m: vec![
                vec![1; 7],
                vec![1; 7],
                vec![2, 3, 1, 1, 1, 1, 1],
                vec![2; 7],
                vec![2, 3, 1, 1, 1, 1, 1],
                vec![3; 7],
            ],
            n_w_to_m: vec![0, 0, 2, 1, 2, 1],
        }
    }

    #[test]
    fn final_triangle_adapter_reports_one_closed_boundary_loop() {
        let report = check_unstructured_mesh_topology(&two_triangle_disk());

        assert!(report.is_consistent(), "{:?}", report.violations);
        assert_eq!(report.boundary_loop_count, 1);
        assert_eq!(report.boundary_vertex_degree_violation_count, 0);
        assert_eq!(report.euler_characteristic, Some(1));
        assert_eq!(report.expected_euler_characteristic, Some(1));
    }

    #[test]
    fn final_triangle_adapter_rejects_boundary_vertex_junction() {
        let point = |lon, lat| LonLatPoint { lon, lat };
        let mesh = UnstructuredMesh {
            m_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.3, 0.3),
                point(-0.3, -0.3),
            ],
            w_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(-1.0, 0.0),
                point(0.0, -1.0),
            ],
            m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 5, 6]],
            w_to_m: vec![
                vec![1; 7],
                vec![1; 7],
                vec![2, 3, 1, 1, 1, 1, 1],
                vec![2; 7],
                vec![2; 7],
                vec![3; 7],
                vec![3; 7],
            ],
            n_w_to_m: vec![0, 0, 2, 1, 1, 1, 1],
        };

        let report = check_unstructured_mesh_topology(&mesh);

        assert!(!report.is_consistent());
        assert_eq!(report.boundary_vertex_degree_violation_count, 1);
        assert!(report
            .violations
            .iter()
            .any(|violation| violation.contains("boundary_vertex_degree")));
    }

    #[test]
    fn split_vertex_fans_preserves_cells_and_rebuilds_reverse_neighbors() {
        let point = |lon, lat| LonLatPoint { lon, lat };
        let mut mesh = UnstructuredMesh {
            m_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 1.0),
                point(0.0, -1.0),
            ],
            w_points: vec![
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(0.0, 0.0),
                point(1.0, 0.0),
                point(0.0, 1.0),
                point(-1.0, 0.0),
                point(0.0, -1.0),
            ],
            m_to_w: vec![[1, 1, 1], [1, 1, 1], [2, 3, 4], [2, 5, 6]],
            w_to_m: vec![
                vec![1; 7],
                vec![1; 7],
                vec![2, 3, 1, 1, 1, 1, 1],
                vec![2; 7],
                vec![2; 7],
                vec![3; 7],
                vec![3; 7],
            ],
            n_w_to_m: vec![0, 0, 2, 1, 1, 1, 1],
        };

        let sources = split_non_manifold_triangle_vertex_fans(&mut mesh).unwrap();

        assert_eq!(sources, vec![2]);
        assert_eq!(mesh.m_points.len(), 4);
        assert_eq!(mesh.w_points.len(), 8);
        assert_ne!(mesh.m_to_w[2][0], mesh.m_to_w[3][0]);
        assert!(check_unstructured_mesh_topology(&mesh).is_consistent());
    }
}
