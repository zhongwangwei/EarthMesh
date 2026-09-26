//! The triangle angle contract, met once for every backend.
//!
//! Every all-triangle mesh the pipeline publishes must keep each interior
//! angle inside [35, 85] degrees (guide 11.72), and inside that pass line the
//! goal is angles as close to 60 as the mesh allows. This runs on the refined
//! mesh after the backend has finished and before anything is written, so it
//! holds whichever algorithm built the mesh: a mesh already inside the window
//! is still pushed toward equilateral, and one outside is repaired into it
//! (`earthmesh_mesh::repair_triangle_angle_window_traced`). The quality gate
//! still judges the result, so a mesh the repair cannot bring in is refused
//! rather than delivered.
//!
//! The repair may flip edges and remove refined vertices of valence 3 or 4.
//! Everything kept per row beside the mesh -- per-cell levels, lineage, `ngr`,
//! the pentagon ids -- follows the rows through the origins the repair
//! reports. The repair works on the two-placeholder layout; the rows it
//! inserted to get there are taken out again, so the mesh comes back in the
//! layout it arrived in (stored ids are the same in both).

use std::io;

use crate::{LonLatPoint, UnstructuredMesh};

use super::global_source::{
    unstructured_mesh_with_one_based_rows, CellRefineLevels, MethodCMetadataOwned,
};

/// Aimed at with a small margin so rounding on the way to the file cannot
/// carry an angle back out of the contract's window.
const WINDOW_MARGIN_DEG: f64 = 0.25;

type P = [f64; 3];

fn unit_xyz(point: LonLatPoint) -> P {
    let xyz = earthmesh_mesh::lonlat_degrees_to_unit_xyz(earthmesh_mesh::LonLatDegrees::new(
        point.lon, point.lat,
    ));
    [xyz.x, xyz.y, xyz.z]
}

fn lonlat(point: P) -> LonLatPoint {
    let degrees = earthmesh_mesh::xyz_to_lonlat_degrees(earthmesh_mesh::CartesianPoint::new(
        point[0], point[1], point[2],
    ));
    LonLatPoint {
        lon: degrees.lon_degrees,
        lat: degrees.lat_degrees,
    }
}

fn circumcenter(corners: [P; 3]) -> Option<P> {
    let [a, b, c] = corners;
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let mut n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length.is_nan() || length <= 0.0 {
        return None;
    }
    let side =
        n[0] * (a[0] + b[0] + c[0]) + n[1] * (a[1] + b[1] + c[1]) + n[2] * (a[2] + b[2] + c[2]);
    let sign = if side < 0.0 { -1.0 } else { 1.0 };
    for value in &mut n {
        *value *= sign / length;
    }
    Some(n)
}

fn centroid(corners: [P; 3]) -> Option<P> {
    let sum = [
        corners[0][0] + corners[1][0] + corners[2][0],
        corners[0][1] + corners[1][1] + corners[2][1],
        corners[0][2] + corners[1][2] + corners[2][2],
    ];
    let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
    (length > 0.0).then(|| [sum[0] / length, sum[1] / length, sum[2] / length])
}

fn distance(a: P, b: P) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Prepend `offset` copies of the first value, the way normalisation inserts
/// placeholder rows ahead of a compact layout.
fn normalised<T: Copy>(values: &[T], offset: usize) -> Vec<T> {
    let mut out = Vec::with_capacity(values.len() + offset);
    if let Some(&first) = values.first() {
        out.extend(std::iter::repeat_n(first, offset));
    }
    out.extend_from_slice(values);
    out
}

fn follow<T: Copy>(values: &[T], origin: &[usize]) -> Vec<T> {
    origin.iter().map(|&row| values[row]).collect()
}

/// The records kept per row beside a refined mesh.
pub(super) struct PublishedRows<'a> {
    pub(super) levels: &'a mut Option<CellRefineLevels>,
    pub(super) metadata: &'a mut Option<MethodCMetadataOwned>,
    pub(super) pentagons: &'a mut [usize; 12],
}

/// Meet the angle contract on a refined triangle mesh; returns the mesh to
/// publish and the repair's report, or the mesh untouched when nothing moved.
pub(super) fn enforce_triangle_angles(
    mesh: UnstructuredMesh,
    rows: PublishedRows<'_>,
) -> io::Result<(UnstructuredMesh, Option<earthmesh_mesh::AngleWindowReport>)> {
    let norm = unstructured_mesh_with_one_based_rows(&mesh);
    let offset_m = norm.m_points.len() - mesh.m_points.len();
    let offset_w = norm.w_points.len() - mesh.w_points.len();
    let vertex_rows = norm.w_points.len();
    let mut faces = Vec::with_capacity(norm.m_to_w.len());
    for (row, corners) in norm.m_to_w.iter().enumerate() {
        let corners = corners.map(|id| usize::try_from(id).unwrap_or(usize::MAX));
        if row >= 2 {
            let valid = corners.iter().all(|&id| (2..vertex_rows).contains(&id))
                && corners[0] != corners[1]
                && corners[1] != corners[2]
                && corners[0] != corners[2];
            if !valid {
                eprintln!(
                    "earthmesh_cli: warning: triangle angle contract not applied: M row {row} is not \
                     a triangle of physical rows; the quality gate still judges the mesh"
                );
                return Ok((mesh, None));
            }
        }
        faces.push(corners);
    }
    let original: Vec<P> = norm.w_points.iter().map(|&p| unit_xyz(p)).collect();
    let mut points = original.clone();

    let levels_m = rows.levels.as_ref().and_then(|levels| {
        (levels.m.len() == mesh.m_points.len() && levels.w.len() == mesh.w_points.len())
            .then(|| normalised(&levels.m, offset_m))
    });
    let mut face_levels: Vec<usize> = levels_m
        .as_ref()
        .map(|m| m.iter().map(|&level| level.max(0) as usize).collect())
        .unwrap_or_default();

    let (lo, hi) = earthmesh_quality::TRIANGLE_ANGLE_WINDOW_DEG;
    let mut options =
        earthmesh_mesh::AngleWindowOptions::new((lo + WINDOW_MARGIN_DEG, hi - WINDOW_MARGIN_DEG));
    options.first_vertex = 2;
    options.first_face = 2;
    // Pentagons keep their ids: nothing at or below the highest is removed.
    options.removable_from = rows
        .pentagons
        .iter()
        .map(|&id| id + offset_w + 1)
        .max()
        .unwrap_or(2)
        .max(2);
    // Never widen the widest fan: model formats bound it (ICON takes six).
    options.max_valence = norm
        .n_w_to_m
        .iter()
        .skip(2)
        .map(|&count| count.max(0) as usize)
        .max()
        .unwrap_or(0);
    let (report, origins) = earthmesh_mesh::repair_triangle_angle_window_traced(
        &mut points,
        &mut faces,
        &mut face_levels,
        options,
    );
    if report.flips + report.moves + report.removed_vertices == 0 {
        return Ok((mesh, Some(report)));
    }

    // W rows: unmoved vertices keep their stored coordinates exactly.
    let moved: Vec<bool> = points
        .iter()
        .zip(&origins.vertex_origin)
        .map(|(point, &row)| *point != original[row])
        .collect();
    let w_points: Vec<LonLatPoint> = points
        .iter()
        .zip(&origins.vertex_origin)
        .zip(&moved)
        .map(|((&point, &row), &was_moved)| {
            if was_moved {
                lonlat(point)
            } else {
                norm.w_points[row]
            }
        })
        .collect();

    // M rows: an untouched triangle keeps its centre; a changed one gets a new
    // centre in the convention the backend used (circumcentre or centroid).
    let unchanged = |row: usize| -> bool {
        let source = origins.face_origin[row];
        row >= 2
            && norm.m_to_w[source].map(|id| id as usize) == faces[row]
            && faces[row].iter().all(|&v| !moved[v])
    };
    let uses_circumcentre = (2..faces.len())
        .find(|&row| unchanged(row))
        .is_none_or(|row| {
            let corners = faces[row].map(|v| points[v]);
            let stored = unit_xyz(norm.m_points[origins.face_origin[row]]);
            match (circumcenter(corners), centroid(corners)) {
                (Some(circ), Some(cent)) => distance(circ, stored) <= distance(cent, stored),
                _ => true,
            }
        });
    let mut m_points = Vec::with_capacity(faces.len());
    for row in 0..faces.len() {
        if row < 2 || unchanged(row) {
            m_points.push(norm.m_points[origins.face_origin[row]]);
            continue;
        }
        let corners = faces[row].map(|v| points[v]);
        let centre = if uses_circumcentre {
            circumcenter(corners)
        } else {
            centroid(corners)
        }
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("repaired triangle {row} has no centre"),
            )
        })?;
        m_points.push(lonlat(centre));
    }
    let m_to_w: Vec<[i32; 3]> = faces
        .iter()
        .enumerate()
        .map(|(row, corners)| {
            if row < 2 {
                norm.m_to_w[row]
            } else {
                corners.map(|v| v as i32)
            }
        })
        .collect();

    // W rings in rotational order; the writer orients them.
    let mut incident: Vec<Vec<usize>> = vec![Vec::new(); w_points.len()];
    for (row, corners) in faces.iter().enumerate().skip(2) {
        for &v in corners {
            incident[v].push(row);
        }
    }
    let padded_width = norm.w_to_m.iter().skip(2).map(Vec::len).max().unwrap_or(0);
    let padding = norm
        .w_to_m
        .iter()
        .zip(&norm.n_w_to_m)
        .skip(2)
        .find(|(ring, &count)| ring.len() > count.max(0) as usize)
        .map(|(ring, &count)| ring[count.max(0) as usize]);
    let mut w_to_m = Vec::with_capacity(w_points.len());
    let mut n_w_to_m = Vec::with_capacity(w_points.len());
    for v in 0..w_points.len() {
        if v < 2 {
            w_to_m.push(norm.w_to_m[v].clone());
            n_w_to_m.push(norm.n_w_to_m[v]);
            continue;
        }
        let mut next = std::collections::HashMap::new();
        for &row in &incident[v] {
            let corners = faces[row];
            let i = corners.iter().position(|&u| u == v).unwrap_or(0);
            next.insert(corners[(i + 1) % 3], (corners[(i + 2) % 3], row));
        }
        let successors: std::collections::HashSet<usize> = next.values().map(|&(u, _)| u).collect();
        let start = next
            .keys()
            .copied()
            .filter(|key| !successors.contains(key))
            .min()
            .or_else(|| next.keys().copied().min());
        let mut ring = Vec::with_capacity(incident[v].len());
        let mut at = start;
        while let Some(key) = at {
            let Some(&(following, row)) = next.get(&key) else {
                break;
            };
            ring.push(row as i32);
            if ring.len() >= incident[v].len() || Some(following) == start {
                break;
            }
            at = Some(following);
        }
        if ring.len() != incident[v].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("repaired W row {v} has no single triangle fan"),
            ));
        }
        n_w_to_m.push(ring.len() as i32);
        if let Some(fill) = padding {
            ring.resize(padded_width.max(ring.len()), fill);
        }
        w_to_m.push(ring);
    }

    // Records follow their rows.
    let face_origin = &origins.face_origin;
    let vertex_origin = &origins.vertex_origin;
    if let (Some(levels), Some(m)) = (rows.levels.as_mut(), levels_m.as_ref()) {
        let merged: Vec<i32> = (0..faces.len())
            .map(|row| {
                if row < 2 {
                    m[face_origin[row]]
                } else {
                    face_levels[row] as i32
                }
            })
            .collect();
        levels.w = follow(&normalised(&levels.w, offset_w), vertex_origin);
        levels.m = merged;
    } else if rows.levels.is_some() {
        *rows.levels = None;
    }
    if let Some(meta) = rows.metadata.as_mut() {
        let m_rows = |values: &[i32]| follow(&normalised(values, offset_m), face_origin);
        let w_rows = |values: &[i32]| follow(&normalised(values, offset_w), vertex_origin);
        meta.m_refine_levels = rows
            .levels
            .as_ref()
            .map_or_else(|| m_rows(&meta.m_refine_levels), |levels| levels.m.clone());
        meta.m_refine_levels_orig = m_rows(&meta.m_refine_levels_orig);
        meta.m_ngr = m_rows(&meta.m_ngr);
        meta.m_lineages = follow(&normalised(&meta.m_lineages, offset_m), face_origin);
        meta.w_refine_levels = rows
            .levels
            .as_ref()
            .map_or_else(|| w_rows(&meta.w_refine_levels), |levels| levels.w.clone());
        meta.w_refine_levels_orig = w_rows(&meta.w_refine_levels_orig);
        meta.w_ngr = w_rows(&meta.w_ngr);
        meta.w_lineages = follow(&normalised(&meta.w_lineages, offset_w), vertex_origin);
    }
    let mut new_id = vec![usize::MAX; vertex_rows];
    for (new, &old) in vertex_origin.iter().enumerate() {
        new_id[old] = new;
    }
    for pentagon in rows.pentagons.iter_mut() {
        let row = *pentagon + offset_w;
        if let Some(&id) = new_id.get(row).filter(|&&id| id != usize::MAX) {
            *pentagon = id - offset_w;
        }
    }

    // Back to the layout the mesh arrived in: drop the rows normalisation
    // inserted. Stored ids are untouched by it, so nothing else shifts.
    let (mut m_points, mut m_to_w) = (m_points, m_to_w);
    let (mut w_points, mut w_to_m, mut n_w_to_m) = (w_points, w_to_m, n_w_to_m);
    m_points.drain(..offset_m);
    m_to_w.drain(..offset_m);
    w_points.drain(..offset_w);
    w_to_m.drain(..offset_w);
    n_w_to_m.drain(..offset_w);
    if let Some(levels) = rows.levels.as_mut() {
        levels.m.drain(..offset_m);
        levels.w.drain(..offset_w);
    }
    if let Some(meta) = rows.metadata.as_mut() {
        for values in [
            &mut meta.m_refine_levels,
            &mut meta.m_refine_levels_orig,
            &mut meta.m_ngr,
        ] {
            values.drain(..offset_m);
        }
        meta.m_lineages.drain(..offset_m);
        for values in [
            &mut meta.w_refine_levels,
            &mut meta.w_refine_levels_orig,
            &mut meta.w_ngr,
        ] {
            values.drain(..offset_w);
        }
        meta.w_lineages.drain(..offset_w);
    }

    Ok((
        UnstructuredMesh {
            m_points,
            w_points,
            m_to_w,
            w_to_m,
            n_w_to_m,
        },
        Some(report),
    ))
}
