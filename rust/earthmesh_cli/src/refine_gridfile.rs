use std::io;
use std::path::Path;

use earthmesh_mesh::{
    method_c_gridinit_factorization_canonical, LonLatDegrees, MethodCGridfileMetadata,
    TriangularMesh,
};

use crate::{
    read_unstructured_mesh_netcdf, unstructured_dimc, write_regional_gridfile_with_refine_levels,
    write_unstructured_mesh_netcdf_with_method_c_metadata, GridRegion,
    MethodCGridfileMetadataSlices, UnstructuredMesh, UnstructuredMeshWriteReport,
};

pub(crate) fn method_c_delaunay_mesh_from_unstructured_gridfile(
    mesh: &UnstructuredMesh,
    metadata: MethodCGridfileMetadataSlices<'_>,
    nxp: usize,
    nspring: usize,
    beta: f64,
    spring_relax: f64,
) -> io::Result<TriangularMesh> {
    let m_point_lonlat = mesh
        .w_points
        .iter()
        .map(|point| LonLatDegrees::new(point.lon, point.lat))
        .collect::<Vec<_>>();
    let w_face_m_points = mesh
        .m_to_w
        .iter()
        .map(|row| {
            Ok([
                usize::try_from(row[0]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C gridfile face contains negative M id {}", row[0]),
                    )
                })?,
                usize::try_from(row[1]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C gridfile face contains negative M id {}", row[1]),
                    )
                })?,
                usize::try_from(row[2]).map_err(|_| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Method-C gridfile face contains negative M id {}", row[2]),
                    )
                })?,
            ])
        })
        .collect::<io::Result<Vec<_>>>()?;
    let m_face_counts = mesh
        .n_w_to_m
        .iter()
        .map(|&count| {
            usize::try_from(count).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Method-C gridfile M-point valence is negative: {count}"),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;

    let pentagons = m_face_counts
        .iter()
        .enumerate()
        .filter(|&(row, &count)| row > 0 && count == 5)
        .count();
    if pentagons == 12 {
        return TriangularMesh::from_voronoi_gridfile_tables_with_metadata(
            &m_point_lonlat,
            &w_face_m_points,
            &m_face_counts,
            MethodCGridfileMetadata {
                m_lineage: metadata.m_lineage,
                m_refine_level: metadata.m_refine_level,
                m_refine_level_orig: metadata.m_refine_level_orig,
                m_ngr: metadata.m_ngr,
                w_lineage: metadata.w_lineage,
                w_refine_level: metadata.w_refine_level,
                w_refine_level_orig: metadata.w_refine_level_orig,
                w_ngr: metadata.w_ngr,
            },
        );
    }
    let factors = method_c_gridinit_factorization_canonical(nxp).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid Method-C gridinit NXP {nxp}"),
        )
    })?;
    let mut mesh = TriangularMesh::from_icosahedron(factors.base_nxp, nspring, beta, spring_relax)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "failed to build Method-C icosahedron fallback for NXP={}",
                    factors.base_nxp
                ),
            )
        })?;
    if factors.expansion_factor > 1 {
        mesh = mesh.expand_by_factor(factors.expansion_factor)?;
    }
    Ok(mesh)
}

pub(crate) fn unstructured_mesh_write_report_from_file(
    output: impl AsRef<Path>,
) -> io::Result<UnstructuredMeshWriteReport> {
    let output = output.as_ref();
    let mesh = read_unstructured_mesh_netcdf(output)?;
    Ok(UnstructuredMeshWriteReport {
        output: output.to_path_buf(),
        sjx_points: mesh.m_points.len(),
        lbx_points: mesh.w_points.len(),
        dimc: unstructured_dimc(&mesh),
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn write_method_c_mesh_with_optional_domain_and_metadata(
    mesh: &UnstructuredMesh,
    raw_output_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    domain_region: Option<&GridRegion>,
    mode_grid: &str,
    metadata: MethodCGridfileMetadataSlices<'_>,
) -> io::Result<(
    Option<UnstructuredMeshWriteReport>,
    UnstructuredMeshWriteReport,
)> {
    let output_path = output_path.as_ref();
    match domain_region {
        Some(region) => {
            let raw_output = write_unstructured_mesh_netcdf_with_method_c_metadata(
                raw_output_path,
                mesh,
                metadata,
            )?;
            let kept = write_regional_gridfile_with_refine_levels(
                &raw_output.output,
                output_path,
                region,
                mode_grid,
                None,
                None,
            )?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Method-C domain mask kept no cells",
                ));
            }
            let output_mesh = read_unstructured_mesh_netcdf(output_path)?;
            crate::validate_published_cell_degrees(&output_mesh, mode_grid)?;
            let output = UnstructuredMeshWriteReport {
                output: output_path.to_path_buf(),
                sjx_points: output_mesh.m_points.len(),
                lbx_points: output_mesh.w_points.len(),
                dimc: unstructured_dimc(&output_mesh),
            };
            Ok((Some(raw_output), output))
        }
        None => {
            crate::validate_published_cell_degrees(mesh, mode_grid)?;
            let output =
                write_unstructured_mesh_netcdf_with_method_c_metadata(output_path, mesh, metadata)?;
            Ok((None, output))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LonLatPoint;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn polygon_cell_mesh(degree: usize) -> UnstructuredMesh {
        let p = |lon, lat| LonLatPoint { lon, lat };
        let mut m_points = vec![p(0.0, 0.0), p(0.0, 0.0)];
        for i in 0..degree {
            let theta = std::f64::consts::TAU * (i as f64) / (degree as f64);
            m_points.push(p(theta.cos(), theta.sin()));
        }
        let w_points = vec![p(0.0, 0.0), p(0.0, 0.0), p(0.0, 0.0)];
        let m_to_w = vec![[1, 1, 1]; m_points.len()];
        let mut ring = (2..(2 + degree as i32)).collect::<Vec<_>>();
        let w_to_m = vec![vec![1; degree], vec![1; degree], std::mem::take(&mut ring)];
        UnstructuredMesh {
            m_points,
            w_points,
            m_to_w,
            w_to_m,
            n_w_to_m: vec![0, 0, degree as i32],
        }
    }

    fn four_edge_published_mesh() -> UnstructuredMesh {
        polygon_cell_mesh(4)
    }

    fn temp_gridfile(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "earthmesh_{name}_{}_{}.nc4",
            std::process::id(),
            stamp
        ))
    }

    fn gridinit_mesh() -> UnstructuredMesh {
        let state = earthmesh_mesh::gridinit_voronoi_state_canonical(2, 0, 1.0, 0.25, 100)
            .expect("gridinit fixture");
        crate::gridfile_mesh_from_one_based_state(&state.grid, &state.tabs)
            .expect("gridinit gridfile mesh")
    }

    fn cleanup(paths: &[&std::path::Path]) {
        for path in paths {
            let _ = std::fs::remove_file(path);
        }
    }

    #[test]
    fn legacy_refined_global_and_regional_hex_publish_real_gridinit_mesh() {
        let mesh = gridinit_mesh();
        let degrees = mesh
            .n_w_to_m
            .iter()
            .copied()
            .filter(|&degree| degree > 0)
            .collect::<Vec<_>>();
        assert!(
            degrees.contains(&5),
            "gridinit fixture should include pentagons"
        );
        assert!(
            degrees.contains(&6),
            "gridinit fixture should include hexagons"
        );

        let output = temp_gridfile("legacy_global_hex_degree_gate");
        let raw = temp_gridfile("legacy_global_hex_degree_gate_raw");
        let report = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &raw,
            &output,
            None,
            "hex",
            MethodCGridfileMetadataSlices::default(),
        )
        .expect("global helper-generated mesh should publish");
        assert_eq!(report.1.lbx_points, mesh.w_points.len());
        cleanup(&[&output, &raw]);

        let regional_output = temp_gridfile("legacy_regional_hex_degree_gate");
        let regional_raw = temp_gridfile("legacy_regional_hex_degree_gate_raw");
        let region = GridRegion::Bbox {
            west: 0.0,
            east: 90.0,
            south: -60.0,
            north: 60.0,
        };
        let report = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &regional_raw,
            &regional_output,
            Some(&region),
            "hex",
            MethodCGridfileMetadataSlices::default(),
        )
        .expect("regional extracted helper-generated mesh should publish");
        assert!(
            report.0.is_some(),
            "regional extraction should keep raw parent"
        );
        assert!(report.1.lbx_points > 2, "regional subset must keep cells");
        assert!(
            report.1.lbx_points < mesh.w_points.len(),
            "regional subset must be smaller than the global parent"
        );
        cleanup(&[&regional_output, &regional_raw]);
    }

    #[test]
    fn legacy_refined_hex_publication_accepts_seven_edge_contract() {
        let mesh = polygon_cell_mesh(7);
        let output = temp_gridfile("legacy_seven_edge_hex_degree_gate");
        let raw = temp_gridfile("legacy_seven_edge_hex_degree_gate_raw");

        let report = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &raw,
            &output,
            None,
            "hex",
            MethodCGridfileMetadataSlices::default(),
        )
        .expect("published hex degree 7 is inside the legacy contract");

        assert_eq!(report.1.dimc, 7);
        cleanup(&[&output, &raw]);
    }

    #[test]
    fn legacy_refined_hex_publication_rejects_non_5_to_7_cells() {
        let mesh = four_edge_published_mesh();
        let output = temp_gridfile("legacy_hex_degree_gate");
        let raw = temp_gridfile("legacy_hex_degree_gate_raw");

        let err = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &raw,
            &output,
            None,
            "hex",
            MethodCGridfileMetadataSlices::default(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("5..=7 edges"), "{err}");
        assert!(!output.exists(), "hex gate should fail before writing");
        cleanup(&[&raw]);
    }

    #[test]
    fn legacy_refined_hex_domain_publication_rejects_bad_extracted_degrees() {
        let mesh = four_edge_published_mesh();
        let output = temp_gridfile("legacy_hex_domain_degree_gate");
        let raw = temp_gridfile("legacy_hex_domain_degree_gate_raw");
        let region = GridRegion::Bbox {
            west: -180.0,
            east: 180.0,
            south: -90.0,
            north: 90.0,
        };

        let err = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &raw,
            &output,
            Some(&region),
            "hex",
            MethodCGridfileMetadataSlices::default(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("5..=7 edges"), "{err}");
        assert!(
            raw.exists(),
            "raw parent is allowed before final extraction gate"
        );
        cleanup(&[&output, &raw]);
    }

    #[test]
    fn legacy_refined_tri_publication_keeps_triangles_out_of_hex_degree_gate() {
        let mesh = gridinit_mesh();
        let output = temp_gridfile("legacy_tri_degree_gate");
        let raw = temp_gridfile("legacy_tri_degree_gate_raw");

        let report = write_method_c_mesh_with_optional_domain_and_metadata(
            &mesh,
            &raw,
            &output,
            None,
            "tri",
            MethodCGridfileMetadataSlices::default(),
        )
        .expect("tri publication should use triangle cells, not the hex 5..7 gate");

        assert_eq!(report.1.sjx_points, mesh.m_points.len());
        cleanup(&[&output, &raw]);
    }
}
