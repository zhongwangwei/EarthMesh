use std::io;

use earthmesh_mesh::LonLatDegrees;

use crate::usize_to_i32;

pub(crate) fn derive_iap_w_to_m_one_based(
    canonical_vertices: usize,
    m_to_w_canonical: &[[usize; 3]],
    m_points_canonical: &[LonLatDegrees],
) -> io::Result<(Vec<Vec<i32>>, Vec<i32>)> {
    let mut incident = vec![Vec::<usize>::new(); canonical_vertices + 1];
    for triangle_id in 2..m_to_w_canonical.len() {
        for &vertex_id in &m_to_w_canonical[triangle_id] {
            if vertex_id == 0 || vertex_id > canonical_vertices {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "IAP-Ocean triangle {triangle_id} canonicals W point {vertex_id}, outside 1..={canonical_vertices}"
                    ),
                ));
            }
            incident[vertex_id].push(triangle_id);
        }
    }

    let maxnum = incident
        .iter()
        .take(canonical_vertices + 1)
        .skip(1)
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(7);
    let mut w_to_m = Vec::with_capacity(canonical_vertices);
    let mut n_w_to_m = Vec::with_capacity(canonical_vertices);
    for vertex_id in 1..=canonical_vertices {
        let sorted = sort_iap_incident_triangles(
            &incident[vertex_id],
            m_to_w_canonical,
            m_points_canonical,
        )?;
        n_w_to_m.push(i32::try_from(incident[vertex_id].len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "IAP-Ocean W point has too many incident triangles",
            )
        })?);
        let mut row = vec![1; maxnum];
        for (slot, triangle_id) in sorted.iter().copied().enumerate() {
            row[slot] = usize_to_i32("IAP-Ocean triangle id", triangle_id)?;
        }
        w_to_m.push(row);
    }
    Ok((w_to_m, n_w_to_m))
}

fn sort_iap_incident_triangles(
    incident: &[usize],
    m_to_w_canonical: &[[usize; 3]],
    m_points_canonical: &[LonLatDegrees],
) -> io::Result<Vec<usize>> {
    if incident.len() <= 1 {
        return Ok(incident.to_vec());
    }

    let mut neighbor_degree = vec![0; incident.len()];
    for (idx, &triangle_id) in incident.iter().enumerate() {
        for (other_idx, &other_triangle_id) in incident.iter().enumerate() {
            if idx == other_idx {
                continue;
            }
            if iap_triangles_are_neighbors(
                m_to_w_canonical[triangle_id],
                m_to_w_canonical[other_triangle_id],
            ) {
                neighbor_degree[idx] += 1;
            }
        }
    }

    let start_pos = neighbor_degree
        .iter()
        .position(|&degree| degree == 1)
        .unwrap_or(0);
    let mut used = vec![false; incident.len()];
    let mut ordered = Vec::with_capacity(incident.len());
    let mut ref_triangle = incident[start_pos];
    used[start_pos] = true;
    ordered.push(ref_triangle);

    while ordered.len() < incident.len() {
        let mut found_pos = None;
        for (idx, &candidate) in incident.iter().enumerate() {
            if used[idx] {
                continue;
            }
            if iap_triangles_are_neighbors(
                m_to_w_canonical[ref_triangle],
                m_to_w_canonical[candidate],
            ) {
                found_pos = Some(idx);
                break;
            }
        }
        if found_pos.is_none() {
            found_pos = used.iter().position(|is_used| !*is_used);
        }
        let Some(pos) = found_pos else {
            break;
        };
        ref_triangle = incident[pos];
        used[pos] = true;
        ordered.push(ref_triangle);
    }

    let area = robust_spherical_area_degrees(
        &ordered
            .iter()
            .map(|&triangle_id| {
                m_points_canonical.get(triangle_id).copied().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "IAP-Ocean sorted triangle id missing M point",
                    )
                })
            })
            .collect::<io::Result<Vec<_>>>()?,
    );
    if area < 0.0 {
        ordered.reverse();
    }
    Ok(ordered)
}

fn iap_triangles_are_neighbors(a: [usize; 3], b: [usize; 3]) -> bool {
    let shared = a
        .iter()
        .filter(|&&vertex_id| b.contains(&vertex_id))
        .count();
    shared >= 2
}

fn robust_spherical_area_degrees(points: &[LonLatDegrees]) -> f64 {
    if points.is_empty() {
        return 0.0;
    }
    let mut area = 0.0;
    for idx in 0..points.len() {
        let next = (idx + 1) % points.len();
        let mut delta_lon = (points[next].lon_degrees - points[idx].lon_degrees).to_radians();
        if delta_lon > std::f64::consts::PI {
            delta_lon -= 2.0 * std::f64::consts::PI;
        } else if delta_lon < -std::f64::consts::PI {
            delta_lon += 2.0 * std::f64::consts::PI;
        }
        area += delta_lon
            * (2.0
                + points[idx].lat_degrees.to_radians().sin()
                + points[next].lat_degrees.to_radians().sin());
    }
    area / 2.0
}
