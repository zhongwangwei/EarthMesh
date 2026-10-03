//! ICON nests, cut in the shared tail from the published global grid
//! (guide 11.86).
//!
//! The backend (`NL%refine_backend = 'icon_nest'`) leaves the global grid as
//! it is and hands its demand here. The nests are planned only after the
//! angle contract has settled the global grid's vertices, so every nest
//! corner is exactly the parent vertex ICON will look for.

use std::{cell::RefCell, io, path::Path};

use crate::{IconNestRunRecord, LonLatPoint, UnstructuredMesh};

use super::global_source::unstructured_mesh_with_one_based_rows;

type P = [f64; 3];

/// The demand a nested run serves, as the backend planned it.
pub(super) struct IconNestDemand {
    pub(super) regions: Vec<earthmesh_mesh::RefinementRegion>,
}

fn unit_xyz(point: LonLatPoint) -> P {
    let xyz = earthmesh_mesh::lonlat_degrees_to_unit_xyz(earthmesh_mesh::LonLatDegrees::new(
        point.lon, point.lat,
    ));
    [xyz.x, xyz.y, xyz.z]
}

fn outward(points: &[P], [a, b, c]: [usize; 3]) -> bool {
    let (a, b, c) = (points[a], points[b], points[c]);
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    n[0] * a[0] + n[1] * a[1] + n[2] * a[2] > 0.0
}

/// The published global grid as unit vectors and outward triangles.
fn global_triangles(mesh: &UnstructuredMesh) -> io::Result<(Vec<P>, Vec<[usize; 3]>)> {
    let norm = unstructured_mesh_with_one_based_rows(mesh);
    // Rows 0 and 1 are placeholders in the one-based layout.
    let first = 2;
    let points = norm.w_points[first..]
        .iter()
        .map(|&p| unit_xyz(p))
        .collect::<Vec<_>>();
    let mut triangles = Vec::with_capacity(norm.m_to_w.len().saturating_sub(first));
    for corners in &norm.m_to_w[first..] {
        let mut tri = [0usize; 3];
        for (slot, &id) in corners.iter().enumerate() {
            tri[slot] = usize::try_from(id)
                .ok()
                .and_then(|id| id.checked_sub(first))
                .filter(|&id| id < points.len())
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "ICON nests need a global grid of whole triangles",
                    )
                })?;
        }
        if !outward(&points, tri) {
            tri.swap(1, 2);
        }
        triangles.push(tri);
    }
    Ok((points, triangles))
}

/// The deepest nest over each global triangle. A nest is its parent's
/// triangles split 1 -> 4, so it covers whole global triangles, and each of
/// its triangles traces back, parent by parent, to exactly one.
fn nest_depth_over_global(domains: &[earthmesh_mesh::IconNestDomain]) -> Vec<u32> {
    let Some(global) = domains.first() else {
        return Vec::new();
    };
    let mut depth = vec![0u32; global.triangles.len()];
    // Domain ids are 1-based and a parent comes before its children.
    let mut roots: Vec<Vec<usize>> = vec![(0..global.triangles.len()).collect()];
    for domain in &domains[1..] {
        let parent_roots = domain
            .parent
            .checked_sub(1)
            .and_then(|index| roots.get(index))
            .cloned()
            .unwrap_or_default();
        let own: Vec<usize> = domain
            .parent_triangle
            .iter()
            .filter_map(|&triangle| parent_roots.get(triangle).copied())
            .collect();
        for &root in &own {
            depth[root] = depth[root].max(domain.depth);
        }
        roots.push(own);
    }
    depth
}

/// The delivery's level on each published row of the global grid: the global
/// grid alone is level 0 everywhere, and the reconciliation read that as the
/// demand unmet -- two levels short over a two-level nest. A triangle (M row)
/// takes the deepest nest over it; a cell (W row) the deepest of its
/// triangles, as the other backends' rows do.
pub(super) fn nest_cell_levels(
    mesh: &UnstructuredMesh,
    depth: &[u32],
) -> super::global_source::CellRefineLevels {
    // `global_triangles` reads the rows one-based with two placeholders, and
    // inserts the second when the mesh carries one.
    let m_shift =
        usize::from(!crate::unstructured_mesh_support::mesh_m_has_two_placeholder_rows(mesh));
    let w_shift =
        usize::from(!crate::unstructured_mesh_support::mesh_w_has_two_placeholder_rows(mesh));
    let mut m = vec![0i32; mesh.m_points.len()];
    let mut w = vec![0i32; mesh.w_points.len()];
    for (triangle, &level) in depth.iter().enumerate() {
        let level = i32::try_from(level).unwrap_or(i32::MAX);
        if let Some(slot) = (triangle + 2)
            .checked_sub(m_shift)
            .and_then(|row| m.get_mut(row))
        {
            *slot = level;
        }
        let Some(corners) = (triangle + 2)
            .checked_sub(m_shift)
            .and_then(|row| mesh.m_to_w.get(row))
        else {
            continue;
        };
        for &corner in corners {
            let row = usize::try_from(corner)
                .ok()
                .and_then(|id| id.checked_sub(w_shift));
            if let Some(slot) = row.and_then(|row| w.get_mut(row)) {
                *slot = (*slot).max(level);
            }
        }
    }
    super::global_source::CellRefineLevels { m, w }
}

/// The nests planned over the published global grid and checked against
/// the angle contract, with what each serves: the algorithm's part, no files.
pub(super) struct IconNestPlan {
    domains: Vec<earthmesh_mesh::IconNestDomain>,
    /// Smallest and largest triangle angle of each domain, degrees.
    angles: Vec<(f64, f64)>,
    /// Each domain's cells the demand asks at least this deep: in its
    /// interior, and in its boundary zone.
    served: Vec<(usize, usize)>,
    boundary_zone: u32,
    /// The deepest nest over each global triangle (`nest_cell_levels`).
    pub(super) depth: Vec<u32>,
}

/// Plan the nests over the published global grid (after the angle contract,
/// so each nest corner is the parent vertex ICON looks for).
pub(super) fn plan_icon_nests(
    mesh: &UnstructuredMesh,
    demand: &IconNestDemand,
    hfield: Option<&crate::hfield_gridfile_context::HfieldGridfileContext>,
) -> io::Result<IconNestPlan> {
    let (points, triangles) = global_triangles(mesh)?;
    let targets = super::global_source::fixed_topology_targets(&demand.regions, hfield)?;
    // The planner asks by position; a failed lookup is kept and returned.
    let failure = RefCell::new(None::<io::Error>);
    let target = |p: P| -> u32 {
        let lonlat = earthmesh_mesh::xyz_to_lonlat_degrees(earthmesh_mesh::CartesianPoint::new(
            p[0], p[1], p[2],
        ));
        match targets.target_level(lonlat) {
            Ok(level) => u32::try_from(level).unwrap_or(u32::MAX),
            Err(error) => {
                failure.borrow_mut().get_or_insert(error);
                0
            }
        }
    };
    let options = earthmesh_refine_icon_nest::IconNestOptions::default();
    let planned = earthmesh_refine_icon_nest::plan_icon_nests(points, triangles, target, &options);
    if let Some(error) = failure.borrow_mut().take() {
        return Err(error);
    }
    let domains =
        planned.map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    if domains.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "icon_nest refinement found no demand to nest: no triangle of the global grid is \
             asked deeper than the base; check the regions' levels and the enabled criteria",
        ));
    }

    // Every grid in the set is published, so each meets the angle contract.
    let mut angles = Vec::with_capacity(domains.len());
    for domain in &domains {
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        for tri in &domain.triangles {
            for a in earthmesh_mesh::spherical_triangle_angles_deg(tri.map(|v| domain.points[v])) {
                low = low.min(a);
                high = high.max(a);
            }
        }
        if !(35.0..=85.0).contains(&low) || !(35.0..=85.0).contains(&high) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ICON domain {} has triangle angles {low:.1}-{high:.1} deg, outside the \
                     35-85 deg contract",
                    domain.id
                ),
            ));
        }
        angles.push((low, high));
    }

    // What each nest serves: its cells the demand asks at least this deep,
    // and how many of those sit in its boundary zone rather than its interior.
    let mut served = Vec::with_capacity(domains.len());
    for domain in &domains {
        let level = domain.depth;
        let (mut interior, mut in_boundary_zone) = (0usize, 0usize);
        if domain.parent > 0 {
            for (tri, &row) in domain.triangles.iter().zip(&domain.cell_row) {
                let centre = {
                    let [a, b, c] = tri.map(|v| domain.points[v]);
                    let s = [a[0] + b[0] + c[0], a[1] + b[1] + c[1], a[2] + b[2] + c[2]];
                    let n = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
                    [s[0] / n, s[1] / n, s[2] / n]
                };
                if target(centre) >= level {
                    if row > options.boundary_zone {
                        interior += 1;
                    } else {
                        in_boundary_zone += 1;
                    }
                }
            }
        }
        served.push((interior, in_boundary_zone));
    }
    if let Some(error) = failure.borrow_mut().take() {
        return Err(error);
    }
    let depth = nest_depth_over_global(&domains);
    Ok(IconNestPlan {
        domains,
        angles,
        served,
        boundary_zone: options.boundary_zone,
        depth,
    })
}

/// Write a planned set into `result_dir/standard/ICON_nest`: the grids, a
/// summary of what each serves and the `&grid_nml` that names them.
pub(super) fn write_icon_nests(
    plan: &IconNestPlan,
    nxp: usize,
    result_dir: &Path,
) -> io::Result<IconNestRunRecord> {
    let dir = result_dir.join("standard").join("ICON_nest");
    let stem = "earthmesh";
    let reports = crate::write_icon_nest_set(&plan.domains, nxp, &dir, stem)?;

    let mut rows = Vec::with_capacity(plan.domains.len());
    for (((domain, report), (low, high)), (served, in_boundary_zone)) in plan
        .domains
        .iter()
        .zip(&reports)
        .zip(&plan.angles)
        .zip(&plan.served)
    {
        rows.push(serde_json::json!({
            "domain": domain.id,
            "parent": domain.parent,
            "grid_level": domain.depth,
            "file": report.output,
            "uuid": report.uuid,
            "cells": report.cells,
            "vertices": report.vertices,
            "edges": report.edges,
            "demanded_cells_served": served,
            "demanded_cells_in_boundary_zone": in_boundary_zone,
            "min_angle_deg": low,
            "max_angle_deg": high,
        }));
    }
    let summary = dir.join("icon_nest_summary.json");
    std::fs::write(
        &summary,
        serde_json::to_vec_pretty(&serde_json::json!({
            "grid_root": nxp,
            "boundary_depth": crate::ICON_NEST_BOUNDARY_DEPTH,
            "boundary_zone_rows": plan.boundary_zone,
            "domains": rows,
        }))
        .map_err(io::Error::other)?,
    )?;

    // ICON finds a nest's parent by UUID; the ids are the fallback it uses
    // when the UUIDs do not settle it.
    let quote = |r: &crate::IconNestFileReport| {
        format!(
            "'{}'",
            r.output.file_name().unwrap_or_default().to_string_lossy()
        )
    };
    let namelist = dir.join("icon_grid_nml.txt");
    std::fs::write(
        &namelist,
        format!(
            "&grid_nml\n  dynamics_grid_filename  = {}\n  dynamics_parent_grid_id = {}\n  lredgrid_phys           = {}\n/\n",
            reports.iter().map(quote).collect::<Vec<_>>().join(", "),
            reports
                .iter()
                .map(|r| r.parent.to_string())
                .collect::<Vec<_>>()
                .join(", "),
            vec![".false."; reports.len()].join(", "),
        ),
    )?;
    for report in &reports {
        eprintln!(
            "earthmesh_cli: ICON domain {} (parent {}, grid_level {}): {} cells -> {}",
            report.domain,
            report.parent,
            report.grid_level,
            report.cells,
            report.output.display()
        );
    }
    Ok(IconNestRunRecord {
        domains: reports.into_iter().map(|r| r.output).collect(),
        namelist,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_mesh::{IconNestDomain, NestVertexOrigin};

    fn domain(id: usize, parent: usize, depth: u32, parents: Vec<usize>) -> IconNestDomain {
        IconNestDomain {
            id,
            parent,
            depth,
            points: Vec::new(),
            triangles: vec![[0, 0, 0]; parents.len().max(3)],
            cell_row: vec![u32::MAX; parents.len().max(3)],
            parent_triangle: parents,
            vertex_origin: vec![NestVertexOrigin::Base],
            vertex_row: Vec::new(),
        }
    }

    #[test]
    fn each_global_triangle_takes_the_deepest_nest_traced_back_to_it() {
        // Global: three triangles. Domain 2 splits global 0 and 2; domain 3
        // splits domain 2's triangle 5, which came from global 2.
        let domains = [
            domain(1, 0, 0, Vec::new()),
            domain(2, 1, 1, vec![0, 0, 0, 0, 2, 2, 2, 2]),
            domain(3, 2, 2, vec![5, 5, 5, 5]),
        ];
        assert_eq!(nest_depth_over_global(&domains), vec![1, 0, 2]);
    }

    #[test]
    fn the_published_rows_take_the_nest_levels() {
        // Two placeholder rows each side, then two triangles over four points.
        let point = LonLatPoint { lon: 0.0, lat: 0.0 };
        let mesh = UnstructuredMesh {
            m_points: vec![point; 4],
            w_points: vec![point; 6],
            m_to_w: vec![[0; 3], [1; 3], [2, 3, 4], [3, 5, 4]],
            w_to_m: vec![Vec::new(); 6],
            n_w_to_m: vec![0; 6],
        };
        let levels = nest_cell_levels(&mesh, &[2, 0]);
        assert_eq!(levels.m, vec![0, 0, 2, 0]);
        // Points of the nested triangle take its level; the other's own point does not.
        assert_eq!(levels.w, vec![0, 0, 2, 2, 2, 0]);
    }
}
