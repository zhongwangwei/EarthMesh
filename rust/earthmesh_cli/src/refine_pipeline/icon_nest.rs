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

/// Plan the nests over the published global grid, write the set into
/// `result_dir/standard/ICON_nest`, and describe it.
pub(super) fn write_icon_nests(
    mesh: &UnstructuredMesh,
    demand: &IconNestDemand,
    hfield: Option<&crate::hfield_gridfile_context::HfieldGridfileContext>,
    nxp: usize,
    result_dir: &Path,
) -> io::Result<IconNestRunRecord> {
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
    let options = earthmesh_mesh::IconNestOptions::default();
    let planned = earthmesh_mesh::plan_icon_nests(points, triangles, target, &options);
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

    let dir = result_dir.join("standard").join("ICON_nest");
    let stem = "earthmesh";
    let reports = crate::write_icon_nest_set(&domains, nxp, &dir, stem)?;

    // What each nest serves: its cells the demand asks at least this deep,
    // and how many of those sit in its boundary zone rather than its interior.
    let mut rows = Vec::with_capacity(domains.len());
    for ((domain, report), (low, high)) in domains.iter().zip(&reports).zip(&angles) {
        let level = domain.depth;
        let (mut served, mut in_boundary_zone) = (0usize, 0usize);
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
                        served += 1;
                    } else {
                        in_boundary_zone += 1;
                    }
                }
            }
        }
        rows.push(serde_json::json!({
            "domain": domain.id,
            "parent": domain.parent,
            "grid_level": level,
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
    if let Some(error) = failure.borrow_mut().take() {
        return Err(error);
    }
    let summary = dir.join("icon_nest_summary.json");
    std::fs::write(
        &summary,
        serde_json::to_vec_pretty(&serde_json::json!({
            "grid_root": nxp,
            "boundary_depth": crate::ICON_NEST_BOUNDARY_DEPTH,
            "boundary_zone_rows": options.boundary_zone,
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
