//! Whole certified Voronoi cells, selected by regional and land-type centres.
//! Regional W rings have open boundaries; their partial M view is not a triangle mesh.
use super::global_source::CertifiedDomainPublication;
use crate::grid_quality_inputs::quality_input_from_gridfile_hex_native;
#[cfg(test)]
use crate::regional_gridfile_writers::lineage::same_cycle;
use crate::regional_gridfile_writers::verify_whole_cell_lineage;
use crate::{gridfile_w_row_layout, GridRegion};
use earthmesh_quality::topology::{self, MeshTopologyValidator, Severity, TopologyIssueType};
use std::{fs, io, path::Path};

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

pub(super) fn publish_regional_land(
    source: &Path,
    output: &Path,
    landtype: &Path,
    gridnum_perdegree: usize,
    region: &GridRegion,
    workdir: &Path,
) -> io::Result<CertifiedDomainPublication> {
    fs::create_dir_all(workdir)?;
    let regional = workdir.join("whole_regional_dual.nc4");
    crate::regional_gridfile_writers::write_regional_gridfile(source, &regional, region, "hex")?;
    let kept_cells = crate::regional_gridfile_writers::write_landtype_masked_gridfile(
        &regional,
        output,
        landtype,
        gridnum_perdegree,
        "hex",
        "landmesh",
    )?;
    let grid = crate::read_gridfile_mesh_points(output)?;
    let input = quality_input_from_gridfile_hex_native(&grid)?;
    for row in gridfile_w_row_layout(&grid).first_physical_row..grid.w_lon.len() {
        if !region.contains(grid.w_lon[row], grid.w_lat[row]) {
            return Err(invalid(
                "CMRC published land cell centre lies outside the region",
            ));
        }
    }
    verify_whole_cell_lineage(source, output, &grid)?;
    let (topology, quality_topology, mut geometry) = audit_land_dual(&input)?;
    geometry["whole_cell_lineage_verified"] = true.into();
    geometry["selection"] = "whole_cells_with_centres_inside_close_polygon_and_IGBP_land".into();
    geometry["boundary_clipping"] = false.into();
    Ok(CertifiedDomainPublication {
        report: crate::unstructured_mesh_write_report_from_file(output)?,
        kept_cells,
        topology,
        quality_topology,
        geometry,
        fvcom_2dm: None,
    })
}

type LandAudit = (
    serde_json::Value,
    (usize, Vec<serde_json::Value>),
    serde_json::Value,
);

fn audit_land_dual(input: &earthmesh_quality::QualityMeshInput) -> io::Result<LandAudit> {
    let mut issues = MeshTopologyValidator::new(input).validate_all();
    for issue in &mut issues {
        // Land islands, including one-cell islands, are valid separate components.
        // Retain their diagnostics; never suppress winding or manifold failures.
        if matches!(
            issue.issue_type,
            TopologyIssueType::DisconnectedMesh | TopologyIssueType::OrphanCell
        ) {
            issue.severity = Severity::Warn;
        }
    }
    let hard = issues
        .iter()
        .filter(|issue| issue.severity == Severity::Fail)
        .map(|issue| format!("{}: {}", issue.issue_type.as_str(), issue.message))
        .collect::<Vec<_>>();
    if !hard.is_empty() {
        return Err(invalid(format!(
            "CMRC published land dual topology failed: {}",
            hard.join("; ")
        )));
    }
    let boundary = topology::boundary_topology(input);
    let euler = topology::euler_characteristic(input);
    let expected = topology::genus_zero_euler_expectation(input, &boundary);
    if expected != Some(euler) {
        return Err(invalid(
            "CMRC published land dual has no valid regional Euler certificate",
        ));
    }
    let g = earthmesh_quality::compute(input, &Default::default()).geometry;
    if g.cell_count == 0
        || g.zero_area_cell_count != 0
        || g.negative_area_cell_count != 0
        || g.non_finite_cell_count != 0
        || g.self_intersection_count != 0
        || g.invalid_polygon_count != 0
        || !g.min_angle_deg.is_finite()
        || !g.max_angle_deg.is_finite()
    {
        return Err(invalid(
            "CMRC published land dual geometry failed native-ring validation",
        ));
    }
    Ok((
        serde_json::json!({
            "cell_view": "hex", "boundary_loops": boundary.loops.len(),
            "boundary_vertex_degree_violations": boundary.invalid_vertex_degrees.len(),
            "euler": euler, "expected_euler": expected, "violations": [],
            "component_policy": "preserve_all_land_components",
        }),
        (topology::connected_component_count(input), issues.iter().map(|issue| serde_json::json!({
            "type": issue.issue_type.as_str(), "severity": issue.severity.as_str(), "message": issue.message,
        })).collect()),
        serde_json::json!({
            "cell_view": "hex", "cells": g.cell_count, "ring_order": "native_outward",
            "minimum_angle_deg": g.min_angle_deg, "maximum_angle_deg": g.max_angle_deg,
            "angle_contract_scope": "pre_export_primal_triangles_not_dual_angles",
            "geometry_pass": true, "zero_area_cell_count": g.zero_area_cell_count,
            "negative_area_cell_count": g.negative_area_cell_count,
            "non_finite_cell_count": g.non_finite_cell_count,
            "self_intersection_count": g.self_intersection_count,
            "invalid_polygon_count": g.invalid_polygon_count,
            "cell_edge_length_cv_max": g.cell_edge_length_cv.max,
            "aspect_ratio_max": g.aspect_ratio.max,
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_cell_cycle_allows_only_start_and_direction_changes() {
        assert!(same_cycle(&[1, 2, 3, 4, 5], &[3, 4, 5, 1, 2]));
        assert!(same_cycle(&[1, 2, 3, 4, 5], &[3, 2, 1, 5, 4]));
        assert!(!same_cycle(&[1, 2, 3, 4, 5], &[1, 3, 2, 4, 5]));
        assert!(!same_cycle(&[1, 2, 3, 4, 5], &[1, 2, 3, 4]));
        assert!(!same_cycle(&[], &[]));
    }
    #[test]
    fn land_audit_preserves_islands_but_rejects_winding_and_vertex_contacts() {
        use earthmesh_geometry::Point;
        use earthmesh_quality::{QualityCell, QualityMeshInput};
        let mut input = QualityMeshInput {
            vertices: Vec::new(),
            cells: Vec::new(),
        };
        for lon in [-20.0, 0.0, 20.0] {
            let start = input.vertices.len();
            input.vertices.extend((0..5).map(|i| {
                let angle = i as f64 * std::f64::consts::TAU / 5.0;
                Point::new(lon + angle.cos(), 20.0 + angle.sin())
            }));
            input.cells.push(QualityCell {
                vertices: (start..start + 5).collect(),
                refine_level: Some(0),
                neighbors: Vec::new(),
            });
        }
        let (topology, (components, issues), _) = audit_land_dual(&input).unwrap();
        assert_eq!(components, 3);
        assert_eq!(topology["boundary_loops"], 3);
        assert_eq!(topology["euler"], 3);
        assert_eq!(topology["expected_euler"], 3);
        assert!(issues.iter().any(|issue| issue["type"] == "orphan_cell"));
        assert!(issues.iter().all(|issue| issue["severity"] == "warn"));
        input.cells[0].vertices.reverse();
        assert!(audit_land_dual(&input)
            .unwrap_err()
            .to_string()
            .contains("geometry"));
        input.cells[0].vertices.reverse();
        input.cells[1].vertices[0] = input.cells[0].vertices[0];
        assert!(audit_land_dual(&input)
            .unwrap_err()
            .to_string()
            .contains("topology"));
    }
    #[test]
    fn lineage_audit_rejects_changed_coordinates_levels_and_ring_order() {
        let path =
            std::env::temp_dir().join(format!("earthmesh_land_lineage_{}.nc4", std::process::id()));
        let mut points = vec![crate::LonLatPoint { lon: 0.0, lat: 0.0 }; 2];
        points.extend((0..5).map(|i| {
            let angle = i as f64 * std::f64::consts::TAU / 5.0;
            crate::LonLatPoint {
                lon: 20.0 + angle.cos(),
                lat: 20.0 + angle.sin(),
            }
        }));
        let mesh = crate::UnstructuredMesh {
            m_points: points,
            w_points: vec![
                crate::LonLatPoint { lon: 0.0, lat: 0.0 },
                crate::LonLatPoint { lon: 0.0, lat: 0.0 },
                crate::LonLatPoint {
                    lon: 20.0,
                    lat: 20.0,
                },
            ],
            m_to_w: vec![[0; 3], [1; 3], [2; 3], [2; 3], [2; 3], [2; 3], [2; 3]],
            w_to_m: vec![vec![0; 7], vec![1; 7], vec![2, 3, 4, 5, 6, 1, 1]],
            n_w_to_m: vec![0, 0, 5],
        };
        crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &path,
            &mesh,
            crate::MethodCGridfileMetadataSlices {
                m_lineage: Some(&[0, 0, 2, 3, 4, 5, 6]),
                w_lineage: Some(&[0, 0, 2]),
                m_refine_level: Some(&[0, 0, 1, 1, 1, 1, 1]),
                w_refine_level: Some(&[0, 0, 1]),
                ..Default::default()
            },
        )
        .unwrap();
        let mut grid = crate::read_gridfile_mesh_points(&path).unwrap();
        quality_input_from_gridfile_hex_native(&grid).unwrap();
        verify_whole_cell_lineage(&path, &path, &grid).unwrap();
        grid.m_lon[2] += 1.0;
        assert!(verify_whole_cell_lineage(&path, &path, &grid).is_err());
        grid = crate::read_gridfile_mesh_points(&path).unwrap();
        grid.w_refine_level[2] = 0;
        assert!(verify_whole_cell_lineage(&path, &path, &grid).is_err());
        grid = crate::read_gridfile_mesh_points(&path).unwrap();
        let start = 2 * grid.w_to_m_width;
        grid.w_to_m.swap(start + 1, start + 2);
        assert!(verify_whole_cell_lineage(&path, &path, &grid)
            .unwrap_err()
            .to_string()
            .contains("cyclic boundary"));
        fs::remove_file(path).unwrap();
    }
}
