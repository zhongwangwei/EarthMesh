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

pub fn check_unstructured_mesh_topology(mesh: &UnstructuredMesh) -> UnstructuredMeshTopologyReport {
    let mut violations = Vec::new();
    if let Err(err) = validate_unstructured_mesh(mesh) {
        violations.push(err.to_string());
        return UnstructuredMeshTopologyReport {
            m_rows: mesh.m_points.len(),
            w_rows: mesh.w_points.len(),
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

    UnstructuredMeshTopologyReport {
        m_rows,
        w_rows,
        violations,
    }
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
