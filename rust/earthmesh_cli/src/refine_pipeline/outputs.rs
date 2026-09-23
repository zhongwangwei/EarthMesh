use crate::landtype_file_is_real;
use crate::namelist_sets_landtype_file;
use crate::read_unstructured_mesh_netcdf;
use crate::unstructured_dimc;
use crate::unstructured_mesh_write_report_from_file;
use crate::write_clean_regional_ocean_gridfile;
use crate::write_colm_coupling_csv_from_mesh_with_options;
use crate::write_colm_coupling_netcdf_from_csv;
use crate::write_colm_package_delivery_manifest_with_quality;
use crate::write_coupling_quality_from_gridfile;
use crate::write_landtype_masked_gridfile_with_refine_levels;
use crate::write_method_c_mesh_with_optional_domain_and_metadata;
use crate::write_regional_gridfile_with_refine_levels;
use crate::CouplingCsvOptions;
use crate::GridRegion;
use crate::MethodCGridfileMetadataSlices;
use crate::RefineCoupledOutputReport;
use crate::UnstructuredMesh;
use crate::UnstructuredMeshWriteReport;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

fn mkgrd_tmpfile_path(file_dir: &Path, nxp: usize, step: usize, suffix: &str) -> PathBuf {
    file_dir
        .join("tmpfile")
        .join(format!("gridfile_NXP{nxp:04}_{step:02}_{suffix}.nc4"))
}

pub(super) struct MethodCRefinedOutputReports {
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<RefineCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
}

pub(super) struct MethodCMetadataSlices<'a> {
    /// Which face of the pre-refinement mesh each final cell descends from.
    pub m_lineage: &'a [i64],
    pub w_lineage: &'a [i64],
    pub m_refine_level: &'a [i32],
    pub m_refine_level_orig: &'a [i32],
    pub m_ngr: &'a [i32],
    pub w_refine_level: &'a [i32],
    pub w_refine_level_orig: &'a [i32],
    pub w_ngr: &'a [i32],
}

impl<'a> MethodCMetadataSlices<'a> {
    fn gridfile(&self) -> MethodCGridfileMetadataSlices<'a> {
        MethodCGridfileMetadataSlices {
            hfield: None,
            mpas: None, // Only a producer with an explicit demand contract fills this below.
            m_refine_level: Some(self.m_refine_level),
            m_refine_level_orig: Some(self.m_refine_level_orig),
            m_ngr: Some(self.m_ngr),
            w_refine_level: Some(self.w_refine_level),
            w_refine_level_orig: Some(self.w_refine_level_orig),
            w_ngr: Some(self.w_ngr),
            m_lineage: Some(self.m_lineage),
            w_lineage: Some(self.w_lineage),
        }
    }
}

#[allow(clippy::too_many_arguments)]
/// Write a refined mesh and everything that goes with it.
///
/// Not Method-C's, despite where it grew up: it takes an `UnstructuredMesh` and
/// an *optional* `MethodCMetadataSlices`, so a backend with no mrlm/ngr/lineage
/// to offer passes `None` and is served the same. That is the seam the
/// red-green route attaches through, and the old name said the opposite.
pub(super) fn write_refined_outputs(
    namelist_contents: &str,
    config: &EarthmeshConfig,
    source_gridnum_perdegree: Option<usize>,
    file_dir: &Path,
    nxp: usize,
    max_level: usize,
    output_mesh: &UnstructuredMesh,
    domain_region: Option<&GridRegion>,
    metadata: Option<MethodCMetadataSlices<'_>>,
    hfield: Option<&crate::hfield_gridfile_context::HfieldGridfileContext>,
    adaptive: Option<(&crate::refinement_demand::nest::AdaptiveNestReport, f64)>,
    lepp: Option<&earthmesh_refine_method_c::AdaptiveHybridReport>,
    hard_center_demand: Option<&[bool]>,
    name_suffix: &str,
    native_cartesian_xy: bool,
) -> io::Result<MethodCRefinedOutputReports> {
    // Finish native W construction after ALL algorithm/Spring work, before
    // any crop/mask/model branch. Never reorder the algorithm-owned input.
    let oriented;
    let output_mesh = if native_cartesian_xy {
        output_mesh
    } else {
        oriented = oriented_spherical_native_w_rings(output_mesh)?;
        &oriented
    };
    let mpas = match (hfield, adaptive, lepp) {
        (Some(demand), None, None) => Some(
            crate::mpas_gridfile_context::MpasGridfileContext::from_hfield_quantized_demand(
                output_mesh,
                demand,
                nxp,
            )?,
        ),
        (None, Some((report, base_m)), None) => Some(
            crate::mpas_gridfile_context::MpasGridfileContext::from_adaptive_region_demand(
                output_mesh,
                report,
                base_m,
                nxp,
            )?,
        ),
        (None, None, Some(report)) => {
            let context =
                crate::mpas_gridfile_context::MpasGridfileContext::from_lepp_resolved_demand(
                    output_mesh,
                    report,
                    nxp,
                )?;
            if context.is_none() {
                eprintln!("earthmesh_cli: LEPP MPAS nominal context unavailable: resolved regions do not cover every parent W site; no background width was supplied");
            }
            context
        }
        (None, None, None) => None,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "refined output cannot have two competing MPAS demand producers",
            ))
        }
    };
    // A fresh producer without algorithm ancestry still needs a root snapshot
    // identity before whole-cell extraction. These are not refinement levels;
    // existing algorithm lineage and no-context routes remain untouched.
    let snapshot_lineage = (mpas.is_some() && metadata.is_none()).then(|| {
        (
            (1..=output_mesh.m_points.len())
                .map(|row| row as i64)
                .collect::<Vec<_>>(),
            (1..=output_mesh.w_points.len())
                .map(|row| row as i64)
                .collect::<Vec<_>>(),
        )
    });
    let mut metadata = MethodCGridfileMetadataSlices {
        hfield,
        mpas: mpas.as_ref(),
        ..metadata
            .as_ref()
            .map(MethodCMetadataSlices::gridfile)
            .unwrap_or_default()
    };
    if let Some((m, w)) = &snapshot_lineage {
        metadata.m_lineage = Some(m);
        metadata.w_lineage = Some(w);
    }
    let output_path = file_dir.join("result").join(format!(
        "gridfile_NXP{nxp:04}_{}{name_suffix}.nc4",
        config.mode_grid.trim(),
    ));
    let has_landtype_file = namelist_sets_landtype_file(namelist_contents)
        && landtype_file_is_real(&config.landtype_file);

    let (raw_output, landtype_masked_cells, coupled_outputs, output) = if has_landtype_file
        && matches!(config.mesh_type.trim(), "landmesh" | "oceanmesh")
    {
        let gridnum_perdegree = method_c_source_gridnum_perdegree(
            source_gridnum_perdegree,
            config,
            "Method-C landtype mask",
        )?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("refine_raw_{}{name_suffix}", config.mode_grid.trim()),
        );
        let raw_output = crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &raw_path,
            output_mesh,
            metadata,
        )?;
        if config.mesh_type.trim() == "oceanmesh" && config.mode_grid.trim() == "tri" {
            if let Some(GridRegion::Close { points }) = domain_region {
                let plan = write_clean_regional_ocean_gridfile(
                    &raw_output.output,
                    points,
                    Path::new(&config.landtype_file),
                    nxp,
                    gridnum_perdegree,
                    config.mask_sea_ratio,
                    file_dir,
                )?;
                let output = unstructured_mesh_write_report_from_file(&plan.result_gridfile)?;
                return Ok(MethodCRefinedOutputReports {
                    raw_output: Some(raw_output),
                    landtype_masked_cells: Some(output.sjx_points.saturating_sub(2)),
                    coupled_outputs: None,
                    output,
                });
            }
        }
        let landtype_input = if let Some(region) = domain_region {
            let domain_path = mkgrd_tmpfile_path(
                file_dir,
                nxp,
                max_level,
                &format!("refine_domain_{}{name_suffix}", config.mode_grid.trim()),
            );
            let kept = write_regional_gridfile_with_refine_levels(
                &raw_output.output,
                &domain_path,
                region,
                config.mode_grid.trim(),
                metadata.m_refine_level,
                metadata.w_refine_level,
            )?;
            if kept == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Method-C domain mask kept no cells",
                ));
            }
            domain_path
        } else {
            raw_output.output.clone()
        };
        let kept = write_landtype_masked_gridfile_with_refine_levels(
            &landtype_input,
            &output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            config.mesh_type.trim(),
            None,
            None,
            config.isolated_ocean,
            hard_center_demand,
        )?;
        let masked_mesh = read_unstructured_mesh_netcdf(&output_path)?;
        crate::validate_published_cell_degrees(&masked_mesh, config.mode_grid.trim())?;
        let output = UnstructuredMeshWriteReport {
            output: output_path.clone(),
            sjx_points: masked_mesh.m_points.len(),
            lbx_points: masked_mesh.w_points.len(),
            dimc: unstructured_dimc(&masked_mesh),
        };
        (Some(raw_output), Some(kept), None, output)
    } else if config.mesh_type.trim() == "LOCmesh" {
        if !has_landtype_file {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "LOCmesh Method-C specified refine requires a real NL%landtype_file",
            ));
        }
        let gridnum_perdegree = method_c_source_gridnum_perdegree(
            source_gridnum_perdegree,
            config,
            "Method-C LOC coupling",
        )?;
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("refine_raw_{}{name_suffix}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata,
        )?;
        let land_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}{name_suffix}_landmesh.nc4",
            config.mode_grid.trim(),
        ));
        let ocean_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}{name_suffix}_oceanmesh.nc4",
            config.mode_grid.trim(),
        ));
        let land_kept = write_landtype_masked_gridfile_with_refine_levels(
            &output.output,
            &land_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "landmesh",
            None,
            None,
            false,
            hard_center_demand,
        )?;
        let ocean_kept = write_landtype_masked_gridfile_with_refine_levels(
            &output.output,
            &ocean_output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            "oceanmesh",
            None,
            None,
            config.isolated_ocean,
            hard_center_demand,
        )?;
        let land_mesh = read_unstructured_mesh_netcdf(&land_output_path)?;
        let ocean_mesh = read_unstructured_mesh_netcdf(&ocean_output_path)?;
        crate::validate_published_cell_degrees(&land_mesh, config.mode_grid.trim())?;
        crate::validate_published_cell_degrees(&ocean_mesh, config.mode_grid.trim())?;
        let land_output = UnstructuredMeshWriteReport {
            output: land_output_path,
            sjx_points: land_mesh.m_points.len(),
            lbx_points: land_mesh.w_points.len(),
            dimc: unstructured_dimc(&land_mesh),
        };
        let ocean_output = UnstructuredMeshWriteReport {
            output: ocean_output_path,
            sjx_points: ocean_mesh.m_points.len(),
            lbx_points: ocean_mesh.w_points.len(),
            dimc: unstructured_dimc(&ocean_mesh),
        };
        let output_stem = output_path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                format!(
                    "gridfile_NXP{nxp:04}_{}{name_suffix}",
                    config.mode_grid.trim()
                )
            });
        let standard_dir = file_dir.join("standard");
        let coupling_csv = standard_dir.join(format!("CoLM_{output_stem}_cells.csv"));
        let coupling_netcdf_path = standard_dir.join(format!("CoLM_{output_stem}_coupling.nc4"));
        let manifest_path = standard_dir.join(format!("CoLM_{output_stem}_manifest.json"));
        let coupling_quality = standard_dir.join(format!("CoLM_{output_stem}_quality.json"));
        let case_name = config.experiment_name.trim();
        let counts = write_colm_coupling_csv_from_mesh_with_options(
            &output.output,
            &config.landtype_file,
            gridnum_perdegree,
            case_name,
            config.mode_grid.trim(),
            &coupling_csv,
            CouplingCsvOptions {
                fraction_method: config.coupling_fraction_method.trim(),
                identify_coastline: config.coupling_identify_coastline,
                identify_river_mouth: config.coupling_identify_river_mouth,
                cama_root: config
                    .coupling_identify_river_mouth
                    .then(|| Path::new(config.coupling_cama_root.trim())),
                target_dx_km: earthmesh_project::nxp_to_km(nxp as i32),
            },
        )?;
        write_coupling_quality_from_gridfile(
            &output.output,
            &config.landtype_file,
            gridnum_perdegree,
            &coupling_quality,
        )?;
        let coupling_netcdf = write_colm_coupling_netcdf_from_csv(
            &coupling_csv,
            &coupling_netcdf_path,
            case_name,
            &manifest_path,
        )?;
        let manifest = write_colm_package_delivery_manifest_with_quality(
            &manifest_path,
            case_name,
            coupling_netcdf.rows,
            &coupling_netcdf.output,
            None,
            None,
            Some(&coupling_quality),
        )?;
        let coupled_outputs = RefineCoupledOutputReport {
            land_output,
            ocean_output,
            coupling_csv,
            coupling_netcdf,
            coupling_quality,
            manifest,
            counts,
        };
        (
            raw_output.or_else(|| Some(output.clone())),
            Some(land_kept + ocean_kept),
            Some(coupled_outputs),
            output,
        )
    } else {
        let raw_path = mkgrd_tmpfile_path(
            file_dir,
            nxp,
            max_level,
            &format!("refine_raw_{}{name_suffix}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata,
        )?;
        (raw_output, None, None, output)
    };

    Ok(MethodCRefinedOutputReports {
        raw_output,
        landtype_masked_cells,
        coupled_outputs,
        output,
    })
}

fn method_c_source_gridnum_perdegree(
    source_gridnum_perdegree: Option<usize>,
    config: &EarthmeshConfig,
    purpose: &str,
) -> io::Result<usize> {
    let value = match source_gridnum_perdegree {
        Some(value) => value,
        None => crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(Path::new(
            config.landtype_file.trim(),
        ))?,
    };
    if value == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("gridnum_perdegree must be positive for {purpose}"),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn method_c_infers_landtype_resolution_instead_of_using_the_source_grid_default() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "earthmesh_method_c_landtype_resolution_{}_{}.nc",
            std::process::id(),
            stamp
        ));
        let mut file = netcdf::create_with(&path, netcdf::Options::default()).unwrap();
        file.add_dimension("lon", 720).unwrap();
        file.add_dimension("lat", 360).unwrap();
        drop(file);

        let config = EarthmeshConfig {
            gridnum_perdegree: 120,
            landtype_file: path.display().to_string(),
            ..EarthmeshConfig::default()
        };
        assert_eq!(
            method_c_source_gridnum_perdegree(None, &config, "test").unwrap(),
            2
        );
        assert_eq!(
            method_c_source_gridnum_perdegree(Some(3), &config, "test").unwrap(),
            3
        );

        let _ = std::fs::remove_file(path);
    }
}

fn oriented_spherical_native_w_rings(mesh: &UnstructuredMesh) -> io::Result<UnstructuredMesh> {
    use crate::unstructured_mesh_support::{
        mesh_canonical_id_for_row, mesh_m_has_two_placeholder_rows, unstructured_w_row_layout,
    };
    let invalid = || {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid spherical native W ring cycle, coordinates or membership",
        )
    };
    if let Some((row, point)) =
        mesh.m_points
            .iter()
            .chain(&mesh.w_points)
            .enumerate()
            .find(|(_, point)| {
                !point.lon.is_finite()
                    || !point.lat.is_finite()
                    || !(-90.0..=90.0).contains(&point.lat)
            })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "invalid spherical native W ring coordinates at row {row}: ({}, {})",
                point.lon, point.lat
            ),
        ));
    }
    let normalized =
        crate::mpas_unstructured_mesh_builders::normalize_unstructured_mesh_placeholder_rows(mesh)?;
    let m_padding = normalized.m_points.len() - mesh.m_points.len();
    let w_padding = normalized.w_points.len() - mesh.w_points.len();
    let m_two = mesh_m_has_two_placeholder_rows(mesh);
    let faces = crate::triangles_on_cell_one_based_from_mesh(&normalized)?;
    let degrees = crate::n_edges_on_cell_usize_from_mesh(&normalized)?;
    let edges = crate::get_edge_from_unstructured_mesh(&normalized)?;
    let vertices = earthmesh_mesh::lonlat_points_to_unit_xyz(&crate::lonlat_degrees_from_points(
        &normalized.m_points,
    ));
    let cells = earthmesh_mesh::lonlat_points_to_unit_xyz(&crate::lonlat_degrees_from_points(
        &normalized.w_points,
    ));
    let ordered = earthmesh_mesh::order_vertices_on_cell_by_shared_edges_one_based(
        &faces,
        &degrees,
        &edges.edges_on_vertex,
        &vertices,
        &cells,
    )
    .ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid spherical native W ring cycle: shared-edge ordering failed",
        )
    })?;
    let edge_set = |ring: &[i32]| {
        let mut edges = (0..ring.len())
            .map(|k| {
                let (a, b) = (ring[k], ring[(k + 1) % ring.len()]);
                (a.min(b), a.max(b))
            })
            .collect::<Vec<_>>();
        edges.sort_unstable();
        edges
    };
    let mut output = mesh.clone();
    for row in unstructured_w_row_layout(mesh).first_physical_row..mesh.w_points.len() {
        let n = usize::try_from(mesh.n_w_to_m[row]).map_err(|_| invalid())?;
        // TRI can have high-degree W fans; only HEX publication has the 5..=7 cap.
        if n < 3 || n > mesh.w_to_m[row].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid spherical native W ring degree {n} at row {row}"),
            ));
        }
        let original = &mesh.w_to_m[row][..n];
        let mut ring = ordered[row + w_padding][..n]
            .iter()
            .map(|&id| {
                id.checked_sub(m_padding)
                    .and_then(|index| mesh_canonical_id_for_row(index, m_two))
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "invalid spherical native W ring canonical id {id} at row {row}"
                            ),
                        )
                    })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let start = ring
            .iter()
            .position(|&id| id == original[0])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, format!("invalid spherical native W ring start id {} at row {row}; ordered={ring:?}", original[0])))?;
        ring.rotate_left(start);
        let mut before = original.to_vec();
        let mut after = ring.clone();
        before.sort_unstable();
        after.sort_unstable();
        if before != after || edge_set(original) != edge_set(&ring) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!(
                "invalid spherical native W ring cycle: row {row}, original={original:?}, ordered={ring:?}"
            )));
        }
        output.w_to_m[row][..n].copy_from_slice(&ring);
    }
    Ok(output)
}

#[cfg(test)]
mod native_ring_tests {
    use super::*;

    #[test]
    fn publication_orients_only_rings_and_preserves_each_placeholder_layout() {
        let state = earthmesh_mesh::gridinit_voronoi_state_canonical(2, 0, 1.0, 0.25, 100).unwrap();
        let compact = crate::gridfile_mesh_from_one_based_state(&state.grid, &state.tabs).unwrap();
        for placeholders in 0..=2 {
            let mut mesh = compact.clone();
            if placeholders == 0 {
                mesh.m_points.remove(0);
                mesh.w_points.remove(0);
                mesh.m_to_w.remove(0);
                mesh.w_to_m.remove(0);
                mesh.n_w_to_m.remove(0);
                for row in &mut mesh.m_to_w {
                    for id in row {
                        *id -= 1;
                    }
                }
                for row in &mut mesh.w_to_m {
                    for id in row {
                        *id -= 1;
                    }
                }
            } else if placeholders == 2 {
                let zero = crate::LonLatPoint { lon: 0.0, lat: 0.0 };
                mesh.m_points.insert(0, zero);
                mesh.w_points.insert(0, zero);
                mesh.m_to_w.insert(0, [0; 3]);
                mesh.w_to_m.insert(0, vec![0]);
                mesh.n_w_to_m.insert(0, 0);
            }
            let expected = mesh.w_to_m.clone();
            let row = placeholders;
            let n = mesh.n_w_to_m[row] as usize;
            mesh.w_to_m[row][1..n].reverse();
            let oriented = oriented_spherical_native_w_rings(&mesh).unwrap();
            assert_eq!(oriented.m_points, mesh.m_points);
            assert_eq!(oriented.w_points, mesh.w_points);
            assert_eq!(oriented.m_to_w, mesh.m_to_w);
            assert_eq!(oriented.n_w_to_m, mesh.n_w_to_m);
            assert_eq!(oriented.w_to_m, expected, "layout {placeholders}");
            assert_ne!(mesh.w_to_m, expected, "input not mutated");
        }
        for case in [
            "duplicate",
            "changed_edges",
            "bad_lat",
            "nan",
            "count_overflow",
        ] {
            let mut mesh = compact.clone();
            match case {
                "duplicate" => mesh.w_to_m[1][1] = mesh.w_to_m[1][0],
                "changed_edges" => mesh.w_to_m[1].swap(1, 2),
                "bad_lat" => mesh.w_points[1].lat = 91.0,
                "nan" => mesh.m_points[2].lon = f64::NAN,
                "count_overflow" => mesh.n_w_to_m[1] = mesh.w_to_m[1].len() as i32 + 1,
                _ => unreachable!(),
            }
            assert!(oriented_spherical_native_w_rings(&mesh).is_err(), "{case}");
        }
    }

    #[test]
    fn triangular_publication_orients_a_high_degree_redgreen_ring() {
        let base = earthmesh_mesh::TriangularMesh::from_icosahedron(12, 0, 1.0, 0.25).unwrap();
        let mut mesh =
            earthmesh_refine_redgreen::redgreen_mesh_from_triangular(&base, &base.m_neighbors)
                .unwrap();
        let settings = earthmesh_refine_redgreen::RedGreenSettings {
            protect_triangle_quality: true,
            min_triangle_angle_deg: 25.0,
            ..Default::default()
        };
        let mut previous = None;
        for _ in 0..3 {
            let marks = mesh
                .triangle_points
                .iter()
                .enumerate()
                .map(|(face, point)| {
                    i32::from(
                        face > mesh.num_vertex
                            && point.lon_degrees.abs() < 35.0
                            && point.lat_degrees.abs() < 30.0,
                    )
                })
                .collect::<Vec<_>>();
            let outcome = earthmesh_refine_redgreen::refine_redgreen_round_inside(
                &mesh,
                &marks,
                &settings,
                previous.as_deref(),
            )
            .unwrap();
            previous = Some(outcome.interior_marks);
            mesh = outcome.mesh;
        }
        crate::redgreen_bridge::finalize_redgreen_mesh(&mut mesh).unwrap();
        let native = crate::redgreen_bridge::unstructured_mesh_from_redgreen(&mesh).unwrap();
        let widest = native.n_w_to_m.iter().max().copied().unwrap();
        assert!(
            widest > 7,
            "fixture must exercise a high-degree ring: {widest}"
        );
        let oriented = oriented_spherical_native_w_rings(&native).unwrap();
        assert!(
            crate::unstructured_mesh_support::check_unstructured_mesh_topology(&oriented)
                .is_consistent()
        );
        crate::validate_published_cell_degrees(&oriented, "tri").unwrap();
        assert!(crate::validate_published_cell_degrees(&oriented, "hex").is_err());
    }
}
