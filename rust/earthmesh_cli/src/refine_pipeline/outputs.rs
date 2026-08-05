use crate::landtype_file_is_real;
use crate::namelist_sets_landtype_file;
use crate::read_unstructured_mesh_netcdf;
use crate::regional_gridfile_writers::{
    write_landtype_masked_gridfile_with_hard_demand_report, LandtypeMaskedGridfileReport,
};
use crate::unstructured_dimc;
use crate::unstructured_mesh_write_report_from_file;
use crate::write_clean_regional_ocean_gridfile_with_hard_demand;
use crate::write_colm_coupling_csv_from_mesh_with_options;
use crate::write_colm_coupling_netcdf_from_csv;
use crate::write_colm_package_delivery_manifest_with_quality;
use crate::write_coupling_quality_from_gridfile;
use crate::write_method_c_mesh_with_optional_domain_and_metadata;
use crate::write_regional_gridfile_with_refine_levels;
use crate::CouplingCsvOptions;
use crate::GridRegion;
use crate::MaskPostprocLayout;
use crate::MethodCGridfileMetadataSlices;
use crate::RefineCoupledOutputReport;
use crate::UnstructuredMesh;
use crate::UnstructuredMeshWriteReport;
use std::collections::VecDeque;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::EarthmeshConfig;

fn mkgrd_tmpfile_path(file_dir: &Path, nxp: usize, step: usize, suffix: &str) -> PathBuf {
    file_dir
        .join("tmpfile")
        .join(format!("gridfile_NXP{nxp:04}_{step:02}_{suffix}.nc4"))
}

fn coupled_staging_path(output: &Path) -> PathBuf {
    let name = output
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "coupled-gridfile.nc4".to_string());
    output.with_file_name(format!(".{name}.earthmesh-staged-{}", std::process::id()))
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn orient_hex_cells(mesh: &mut UnstructuredMesh, spherical: bool) -> io::Result<()> {
    let mut edges = BTreeMap::<(i32, i32), Vec<(usize, bool)>>::new();
    for (cell, row) in mesh.w_to_m.iter().enumerate() {
        let count = usize::try_from(*mesh.n_w_to_m.get(cell).unwrap_or(&0)).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("hex cell {cell} has a negative vertex count"),
            )
        })?;
        if count < 3 {
            continue;
        }
        if count > row.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "hex cell {cell} vertex count {count} exceeds row width {}",
                    row.len()
                ),
            ));
        }
        if row[..count].iter().any(|&id| id <= 1) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("hex cell {cell} contains a placeholder inside its physical ring"),
            ));
        }
        for slot in 0..count {
            let start = row[slot];
            let end = row[(slot + 1) % count];
            edges
                .entry((start.min(end), start.max(end)))
                .or_default()
                .push((cell, start < end));
        }
    }

    let mut adjacent = vec![Vec::<(usize, bool)>::new(); mesh.w_to_m.len()];
    for (edge, owners) in edges {
        if owners.len() > 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "hex edge {}-{} belongs to {} cells",
                    edge.0,
                    edge.1,
                    owners.len()
                ),
            ));
        }
        if let [(left, left_forward), (right, right_forward)] = owners.as_slice() {
            if left == right {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("hex cell {left} repeats edge {}-{}", edge.0, edge.1),
                ));
            }
            let same_direction = left_forward == right_forward;
            adjacent[*left].push((*right, same_direction));
            adjacent[*right].push((*left, same_direction));
        }
    }

    let mut reversed = vec![None; mesh.w_to_m.len()];
    for start in 0..mesh.w_to_m.len() {
        if reversed[start].is_some()
            || usize::try_from(*mesh.n_w_to_m.get(start).unwrap_or(&0)).unwrap_or(0) < 3
        {
            continue;
        }
        reversed[start] = Some(false);
        let mut queue = VecDeque::from([start]);
        let mut component = Vec::new();
        while let Some(cell) = queue.pop_front() {
            component.push(cell);
            let cell_reversed = reversed[cell].unwrap_or(false);
            for &(neighbor, same_direction) in &adjacent[cell] {
                let expected = cell_reversed ^ same_direction;
                match reversed[neighbor] {
                    Some(actual) if actual != expected => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "hex cell rings cannot be oriented consistently",
                        ));
                    }
                    Some(_) => {}
                    None => {
                        reversed[neighbor] = Some(expected);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        let component_is_clockwise = component
            .iter()
            .find_map(|&cell| {
                let count = usize::try_from(mesh.n_w_to_m[cell]).ok()?;
                let mut ids = mesh.w_to_m[cell][..count].to_vec();
                if reversed[cell] == Some(true) {
                    ids.reverse();
                }
                let ring = ids
                    .into_iter()
                    .map(|id| {
                        usize::try_from(id)
                            .ok()
                            .and_then(|id| id.checked_sub(1))
                            .and_then(|index| mesh.m_points.get(index))
                            .map(|point| earthmesh_geometry::Point::new(point.lon, point.lat))
                    })
                    .collect::<Option<Vec<_>>>()?;
                if spherical {
                    earthmesh_geometry::try_spherical_polygon_area(&ring)
                        .ok()
                        .and_then(|area| match area.winding {
                            earthmesh_geometry::SphericalWinding::CounterClockwise => Some(false),
                            earthmesh_geometry::SphericalWinding::Clockwise => Some(true),
                            earthmesh_geometry::SphericalWinding::Indeterminate => None,
                        })
                } else {
                    let area2 = ring
                        .iter()
                        .zip(ring.iter().cycle().skip(1))
                        .take(ring.len())
                        .map(|(a, b)| a.x * b.y - b.x * a.y)
                        .sum::<f64>();
                    (area2.is_finite() && area2 != 0.0).then_some(area2 < 0.0)
                }
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "hex component has no orientable physical cell",
                )
            })?;
        if component_is_clockwise {
            for cell in component {
                reversed[cell] = reversed[cell].map(|value| !value);
            }
        }
    }

    for (cell, reverse) in reversed.into_iter().enumerate() {
        if reverse == Some(true) {
            let count = usize::try_from(mesh.n_w_to_m[cell]).unwrap_or(0);
            mesh.w_to_m[cell][..count].reverse();
        }
    }
    Ok(())
}

fn publish_coupled_staged_outputs(
    land_staged: &Path,
    land_output: &Path,
    ocean_staged: &Path,
    ocean_output: &Path,
) -> io::Result<()> {
    for output in [land_output, ocean_output] {
        if output.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "refusing to replace an existing coupled output during atomic publication: {}",
                    output.display()
                ),
            ));
        }
    }
    fs::metadata(land_staged)?;
    fs::metadata(ocean_staged)?;
    fs::rename(land_staged, land_output)?;
    if let Err(error) = fs::rename(ocean_staged, ocean_output) {
        let _ = remove_if_exists(land_output);
        return Err(error);
    }
    Ok(())
}

pub(super) struct MethodCRefinedOutputReports {
    pub raw_output: Option<UnstructuredMeshWriteReport>,
    pub landtype_masked_cells: Option<usize>,
    pub coupled_outputs: Option<RefineCoupledOutputReport>,
    pub output: UnstructuredMeshWriteReport,
}

pub(super) struct MethodCMetadataSlices<'a> {
    pub m_lineage: &'a [i64],
    pub m_refine_level: &'a [i32],
    pub m_refine_level_orig: &'a [i32],
    pub m_ngr: &'a [i32],
    pub w_lineage: &'a [i64],
    pub w_refine_level: &'a [i32],
    pub w_refine_level_orig: &'a [i32],
    pub w_ngr: &'a [i32],
}

impl<'a> MethodCMetadataSlices<'a> {
    fn gridfile(&self) -> MethodCGridfileMetadataSlices<'a> {
        MethodCGridfileMetadataSlices {
            m_lineage: Some(self.m_lineage),
            m_refine_level: Some(self.m_refine_level),
            m_refine_level_orig: Some(self.m_refine_level_orig),
            m_ngr: Some(self.m_ngr),
            w_lineage: Some(self.w_lineage),
            w_refine_level: Some(self.w_refine_level),
            w_refine_level_orig: Some(self.w_refine_level_orig),
            w_ngr: Some(self.w_ngr),
        }
    }
}

fn project_single_product_hard_demand(
    demand: Option<&crate::source_demand_artifact::PreparedHfieldDemand>,
    product_support: Option<&[bool]>,
    gridfile: &Path,
    mode_grid: &str,
) -> io::Result<Option<Vec<bool>>> {
    match (demand, product_support) {
        (Some(demand), Some(product_support)) => demand
            .hard_center_demand_for_product_gridfile(gridfile, mode_grid, product_support)
            .map(Some),
        (None, None) => Ok(None),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "single land/ocean HField demand and product support must be supplied together",
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn write_method_c_refined_outputs(
    namelist_contents: &str,
    config: &EarthmeshConfig,
    source_gridnum_perdegree: Option<usize>,
    file_dir: &Path,
    nxp: usize,
    max_level: usize,
    output_mesh: &UnstructuredMesh,
    domain_region: Option<&GridRegion>,
    metadata: Option<MethodCMetadataSlices<'_>>,
    hfield_demand: Option<&crate::source_demand_artifact::PreparedHfieldDemand>,
    single_hfield_product_support: Option<&[bool]>,
    coupled_hfield_product_support: Option<(&[bool], &[bool])>,
    cartesian_xy: bool,
) -> io::Result<MethodCRefinedOutputReports> {
    let mut oriented_output_mesh = None;
    if config.mode_grid.trim() == "hex" {
        let mut mesh = output_mesh.clone();
        orient_hex_cells(&mut mesh, !cartesian_xy)?;
        oriented_output_mesh = Some(mesh);
    }
    let output_mesh = oriented_output_mesh.as_ref().unwrap_or(output_mesh);
    let output_path = file_dir.join("result").join(format!(
        "gridfile_NXP{nxp:04}_{}.nc4",
        config.mode_grid.trim()
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
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let raw_output = crate::write_unstructured_mesh_netcdf_with_method_c_metadata(
            &raw_path,
            output_mesh,
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
        )?;
        if config.mesh_type.trim() == "oceanmesh" && config.mode_grid.trim() == "tri" {
            if let Some(GridRegion::Close { points }) = domain_region {
                let hard_center_demand = project_single_product_hard_demand(
                    hfield_demand,
                    single_hfield_product_support,
                    &raw_output.output,
                    config.mode_grid.trim(),
                )?
                .unwrap_or_default();
                let plan = write_clean_regional_ocean_gridfile_with_hard_demand(
                    &raw_output.output,
                    points,
                    Path::new(&config.landtype_file),
                    nxp,
                    gridnum_perdegree,
                    config.mask_sea_ratio,
                    file_dir,
                    &hard_center_demand,
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
                &format!("refine_domain_{}", config.mode_grid.trim()),
            );
            let kept = write_regional_gridfile_with_refine_levels(
                &raw_output.output,
                &domain_path,
                region,
                config.mode_grid.trim(),
                metadata.as_ref().map(|fields| fields.m_refine_level),
                metadata.as_ref().map(|fields| fields.w_refine_level),
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
        let hard_center_demand = project_single_product_hard_demand(
            hfield_demand,
            single_hfield_product_support,
            &landtype_input,
            config.mode_grid.trim(),
        )?;
        let masked = write_landtype_masked_gridfile_with_hard_demand_report(
            &landtype_input,
            &output_path,
            &config.landtype_file,
            gridnum_perdegree,
            config.mode_grid.trim(),
            config.mesh_type.trim(),
            None,
            None,
            hard_center_demand.as_deref(),
            false,
        )?;
        let kept = masked.kept;
        let masked_mesh = read_unstructured_mesh_netcdf(&output_path)?;
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
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
        )?;
        let land_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_landmesh.nc4",
            config.mode_grid.trim()
        ));
        let ocean_output_path = file_dir.join("result").join(format!(
            "gridfile_NXP{nxp:04}_{}_oceanmesh.nc4",
            config.mode_grid.trim()
        ));
        let land_staged_path = coupled_staging_path(&land_output_path);
        let ocean_staged_path = coupled_staging_path(&ocean_output_path);
        let (land_hard_demand, ocean_hard_demand) =
            match (hfield_demand, coupled_hfield_product_support) {
                (Some(demand), Some((land_support, ocean_support))) => (
                    Some(demand.hard_center_demand_for_product_gridfile(
                        &output.output,
                        config.mode_grid.trim(),
                        land_support,
                    )?),
                    Some(demand.hard_center_demand_for_product_gridfile(
                        &output.output,
                        config.mode_grid.trim(),
                        ocean_support,
                    )?),
                ),
                (None, None) => (None, None),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "LOCmesh HField demand and product support must be supplied together",
                    ));
                }
            };
        remove_if_exists(&land_staged_path)?;
        remove_if_exists(&ocean_staged_path)?;
        let staged = (|| {
            let land_mask = write_landtype_masked_gridfile_with_hard_demand_report(
                &output.output,
                &land_staged_path,
                &config.landtype_file,
                gridnum_perdegree,
                config.mode_grid.trim(),
                "landmesh",
                None,
                None,
                land_hard_demand.as_deref(),
                true,
            )?;
            let ocean_mask = write_landtype_masked_gridfile_with_hard_demand_report(
                &output.output,
                &ocean_staged_path,
                &config.landtype_file,
                gridnum_perdegree,
                config.mode_grid.trim(),
                "oceanmesh",
                None,
                None,
                ocean_hard_demand.as_deref(),
                true,
            )?;
            let source_mesh = read_unstructured_mesh_netcdf(&output.output)?;
            let source_layout = crate::ensure_leading_mask_postproc_placeholder(
                crate::mask_postproc_layout_from_unstructured_mesh(
                    &source_mesh,
                    config.mode_grid.trim(),
                )?,
            );
            validate_coupled_source_edge_interface(
                &source_layout,
                &land_mask,
                &ocean_mask,
                config.mode_grid.trim(),
            )?;
            validate_coupled_hard_demand_delivery(&land_mask, land_hard_demand.as_deref(), "land")?;
            validate_coupled_hard_demand_delivery(
                &ocean_mask,
                ocean_hard_demand.as_deref(),
                "ocean",
            )?;
            Ok::<_, io::Error>((land_mask, ocean_mask))
        })();
        let (land_mask, ocean_mask) = match staged {
            Ok(reports) => reports,
            Err(error) => {
                let _ = remove_if_exists(&land_staged_path);
                let _ = remove_if_exists(&ocean_staged_path);
                return Err(error);
            }
        };
        if let Err(error) = publish_coupled_staged_outputs(
            &land_staged_path,
            &land_output_path,
            &ocean_staged_path,
            &ocean_output_path,
        ) {
            let _ = remove_if_exists(&land_staged_path);
            let _ = remove_if_exists(&ocean_staged_path);
            return Err(error);
        }
        let land_kept = land_mask.kept;
        let ocean_kept = ocean_mask.kept;
        let land_mesh = read_unstructured_mesh_netcdf(&land_output_path)?;
        let ocean_mesh = read_unstructured_mesh_netcdf(&ocean_output_path)?;
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
            .unwrap_or_else(|| format!("gridfile_NXP{nxp:04}_{}", config.mode_grid.trim()));
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
            &format!("refine_raw_{}", config.mode_grid.trim()),
        );
        let (raw_output, output) = write_method_c_mesh_with_optional_domain_and_metadata(
            output_mesh,
            &raw_path,
            &output_path,
            domain_region,
            config.mode_grid.trim(),
            metadata
                .as_ref()
                .map(MethodCMetadataSlices::gridfile)
                .unwrap_or_default(),
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

fn validate_coupled_source_edge_interface(
    source: &MaskPostprocLayout,
    land: &LandtypeMaskedGridfileReport,
    ocean: &LandtypeMaskedGridfileReport,
    mode_grid: &str,
) -> io::Result<()> {
    if !matches!(mode_grid.trim(), "hex" | "tri") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "LandOceanCoupled delivered interface contract supports hex or tri, got {mode_grid}"
            ),
        ));
    }
    let land_cells = validate_delivered_source_cells(source, land, "land")?;
    let ocean_cells = validate_delivered_source_cells(source, ocean, "ocean")?;
    for source_center_id in 2..source.ustr_points {
        match (
            land.active_source_centers[source_center_id],
            ocean.active_source_centers[source_center_id],
        ) {
            (true, false) | (false, true) => {}
            (true, true) => {
                return coupled_interface_error(format!(
                    "source cell {source_center_id} was delivered by both land and ocean products"
                ));
            }
            (false, false) => {
                return coupled_interface_error(format!(
                    "source cell {source_center_id} was omitted by both land and ocean products"
                ));
            }
        }
    }

    let expected_land = active_source_cells(source, &land.active_source_centers)?;
    let expected_ocean = active_source_cells(source, &ocean.active_source_centers)?;
    let expected_land_edges = oriented_source_edges(&expected_land);
    let expected_ocean_edges = oriented_source_edges(&expected_ocean);
    let delivered_land_edges = oriented_source_edges(&land_cells);
    let delivered_ocean_edges = oriented_source_edges(&ocean_cells);
    let interface_edges = expected_land_edges
        .keys()
        .filter(|edge| expected_ocean_edges.contains_key(edge))
        .copied()
        .collect::<BTreeSet<_>>();

    for edge in interface_edges {
        let expected_land_side = &expected_land_edges[&edge];
        let expected_ocean_side = &expected_ocean_edges[&edge];
        validate_one_interface_edge(
            edge,
            expected_land_side,
            expected_ocean_side,
            "canonical source",
        )?;
        let delivered_land_side = delivered_land_edges
            .get(&edge)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let delivered_ocean_side = delivered_ocean_edges
            .get(&edge)
            .map(Vec::as_slice)
            .unwrap_or_default();
        validate_one_interface_edge(edge, delivered_land_side, delivered_ocean_side, "delivered")?;
        if delivered_land_side != expected_land_side || delivered_ocean_side != expected_ocean_side
        {
            return coupled_interface_error(format!(
                "source edge {}-{} changed cell ownership or direction during delivery",
                edge.0, edge.1
            ));
        }
    }
    Ok(())
}

fn validate_coupled_hard_demand_delivery(
    delivered: &LandtypeMaskedGridfileReport,
    hard_demand: Option<&[bool]>,
    side: &str,
) -> io::Result<()> {
    let Some(hard_demand) = hard_demand else {
        return Ok(());
    };
    let physical_cells = delivered.active_source_centers.len().saturating_sub(2);
    if hard_demand.len() != physical_cells {
        return coupled_interface_error(format!(
            "{side} hard-demand mask length {} does not match {physical_cells} physical source cells",
            hard_demand.len()
        ));
    }
    if let Some(offset) = hard_demand.iter().enumerate().find_map(|(offset, hard)| {
        (*hard && !delivered.active_source_centers[offset + 2]).then_some(offset)
    }) {
        return coupled_interface_error(format!(
            "{side} product omitted immutable hard-demand source cell {}",
            offset + 2
        ));
    }
    Ok(())
}

fn validate_delivered_source_cells(
    source: &MaskPostprocLayout,
    delivered: &LandtypeMaskedGridfileReport,
    side: &str,
) -> io::Result<BTreeMap<usize, Vec<usize>>> {
    if delivered.active_source_centers.len() != source.ustr_points {
        return coupled_interface_error(format!(
            "{side} active source mask length {} does not match source center count {}",
            delivered.active_source_centers.len(),
            source.ustr_points
        ));
    }
    let expected = active_source_cells(source, &delivered.active_source_centers)?;
    if delivered.kept != expected.len() {
        return coupled_interface_error(format!(
            "{side} reported {} kept cells but clipping/orphan cleanup retained {} source cells",
            delivered.kept,
            expected.len()
        ));
    }
    let mut actual = BTreeMap::new();
    for (source_center_id, ring) in &delivered.delivered_source_cells {
        if actual.insert(*source_center_id, ring.clone()).is_some() {
            return coupled_interface_error(format!(
                "{side} source cell {source_center_id} was delivered more than once"
            ));
        }
    }
    let expected_ids = expected.keys().copied().collect::<BTreeSet<_>>();
    let actual_ids = actual.keys().copied().collect::<BTreeSet<_>>();
    if expected_ids != actual_ids {
        let missing = expected_ids
            .difference(&actual_ids)
            .copied()
            .collect::<Vec<_>>();
        let unexpected = actual_ids
            .difference(&expected_ids)
            .copied()
            .collect::<Vec<_>>();
        return coupled_interface_error(format!(
            "{side} delivered source cells differ after clipping/orphan cleanup: missing={missing:?} unexpected={unexpected:?}"
        ));
    }
    for (source_center_id, delivered_ring) in &actual {
        let source_ring = &expected[source_center_id];
        let canonical_source = canonical_directed_ring(source_ring);
        let canonical_delivered = canonical_directed_ring(delivered_ring);
        if canonical_source != canonical_delivered {
            return coupled_interface_error(format!(
                "{side} source cell {source_center_id} boundary {canonical_delivered:?} differs from canonical source ring {canonical_source:?} (loss, duplication, T-junction, partial overlap, or orientation change)"
            ));
        }
    }
    Ok(actual)
}

fn active_source_cells(
    source: &MaskPostprocLayout,
    active: &[bool],
) -> io::Result<BTreeMap<usize, Vec<usize>>> {
    if active.len() != source.ustr_points {
        return coupled_interface_error(format!(
            "active source mask length {} does not match source center count {}",
            active.len(),
            source.ustr_points
        ));
    }
    let mut cells = BTreeMap::new();
    for source_center_id in 2..source.ustr_points {
        if !active[source_center_id] {
            continue;
        }
        let count = source.center_neighbor_counts[source_center_id];
        let row = &source.center_neighbors[source_center_id];
        if count < 3 || count > row.len() {
            return coupled_interface_error(format!(
                "canonical source cell {source_center_id} has invalid vertex count {count}"
            ));
        }
        let ring = row[..count].to_vec();
        if ring
            .iter()
            .any(|&vertex| vertex <= 1 || vertex >= source.ustr_bounds)
        {
            return coupled_interface_error(format!(
                "canonical source cell {source_center_id} references an invalid vertex in {ring:?}"
            ));
        }
        if ring
            .iter()
            .enumerate()
            .any(|(slot, &vertex)| vertex == ring[(slot + 1) % ring.len()])
        {
            return coupled_interface_error(format!(
                "canonical source cell {source_center_id} contains a zero-length edge"
            ));
        }
        cells.insert(source_center_id, ring);
    }
    Ok(cells)
}

fn canonical_directed_ring(ring: &[usize]) -> Vec<usize> {
    (0..ring.len())
        .map(|offset| {
            ring[offset..]
                .iter()
                .chain(&ring[..offset])
                .copied()
                .collect::<Vec<_>>()
        })
        .min()
        .unwrap_or_default()
}

type SourceEdge = (usize, usize);
type DirectedSourceEdge = (usize, usize, usize);

fn oriented_source_edges(
    cells: &BTreeMap<usize, Vec<usize>>,
) -> BTreeMap<SourceEdge, Vec<DirectedSourceEdge>> {
    let mut edges = BTreeMap::<SourceEdge, Vec<DirectedSourceEdge>>::new();
    for (&source_center_id, ring) in cells {
        for slot in 0..ring.len() {
            let start = ring[slot];
            let end = ring[(slot + 1) % ring.len()];
            edges
                .entry((start.min(end), start.max(end)))
                .or_default()
                .push((source_center_id, start, end));
        }
    }
    edges
}

fn validate_one_interface_edge(
    edge: SourceEdge,
    land: &[DirectedSourceEdge],
    ocean: &[DirectedSourceEdge],
    stage: &str,
) -> io::Result<()> {
    if land.len() != 1 || ocean.len() != 1 {
        return coupled_interface_error(format!(
            "{stage} interface edge {}-{} occurs {} times on land and {} times on ocean; expected exactly once per side",
            edge.0,
            edge.1,
            land.len(),
            ocean.len(),
        ));
    }
    if land[0].1 != ocean[0].2 || land[0].2 != ocean[0].1 {
        return coupled_interface_error(format!(
            "{stage} interface edge {}-{} has non-opposite orientation land={}->{} ocean={}->{}",
            edge.0, edge.1, land[0].1, land[0].2, ocean[0].1, ocean[0].2,
        ));
    }
    Ok(())
}

fn coupled_interface_error<T>(message: String) -> io::Result<T> {
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("LandOceanCoupled delivered interface contract failed: {message}"),
    ))
}

pub(super) fn method_c_source_gridnum_perdegree(
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
    use crate::LonLatPoint;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn coupled_layout(rings: Vec<Vec<usize>>, vertex_count: usize) -> MaskPostprocLayout {
        let mut center_neighbors = vec![Vec::new(), Vec::new()];
        center_neighbors.extend(rings);
        let center_neighbor_counts = center_neighbors.iter().map(Vec::len).collect::<Vec<_>>();
        MaskPostprocLayout {
            ustr_points: center_neighbors.len(),
            ustr_bounds: vertex_count,
            center_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; center_neighbors.len()],
            vertex_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; vertex_count],
            center_neighbors,
            vertex_neighbors: vec![Vec::new(); vertex_count],
            center_neighbor_counts,
            vertex_neighbor_counts: vec![0; vertex_count],
        }
    }

    fn coupled_delivery(
        source_centers: usize,
        cells: Vec<(usize, Vec<usize>)>,
    ) -> LandtypeMaskedGridfileReport {
        let mut active_source_centers = vec![false; source_centers];
        for (source_center_id, _) in &cells {
            active_source_centers[*source_center_id] = true;
        }
        LandtypeMaskedGridfileReport {
            kept: cells.len(),
            active_source_centers,
            delivered_source_cells: cells,
        }
    }

    fn closed_tetrahedron_tri_contract() -> (
        MaskPostprocLayout,
        LandtypeMaskedGridfileReport,
        LandtypeMaskedGridfileReport,
    ) {
        let source = coupled_layout(
            vec![vec![2, 3, 4], vec![2, 5, 3], vec![3, 5, 4], vec![4, 5, 2]],
            6,
        );
        let land = coupled_delivery(
            source.ustr_points,
            vec![(2, vec![2, 3, 4]), (3, vec![2, 5, 3])],
        );
        let ocean = coupled_delivery(
            source.ustr_points,
            vec![(4, vec![3, 5, 4]), (5, vec![4, 5, 2])],
        );
        (source, land, ocean)
    }

    #[test]
    fn coupled_interface_accepts_signed_hex_regional_antimeridian_edges() {
        let mut source = coupled_layout(
            vec![
                vec![2, 3, 7, 6],
                vec![3, 4, 8, 7],
                vec![4, 5, 9, 8],
                vec![5, 10, 11, 9],
            ],
            12,
        );
        source.vertex_points[4] = LonLatPoint {
            lon: 179.0,
            lat: -1.0,
        };
        source.vertex_points[8] = LonLatPoint {
            lon: -179.0,
            lat: 1.0,
        };
        let land = coupled_delivery(
            source.ustr_points,
            vec![(2, vec![2, 3, 7, 6]), (3, vec![3, 4, 8, 7])],
        );
        let ocean = coupled_delivery(
            source.ustr_points,
            vec![(4, vec![4, 5, 9, 8]), (5, vec![5, 10, 11, 9])],
        );

        validate_coupled_source_edge_interface(&source, &land, &ocean, "hex").unwrap();
    }

    #[test]
    fn coupled_interface_accepts_signed_closed_global_tri_partition() {
        let (source, land, ocean) = closed_tetrahedron_tri_contract();

        // The low-level mask writer accepts triangular products even though the
        // current project capability registry exposes LandOceanCoupled as Hex.
        validate_coupled_source_edge_interface(&source, &land, &ocean, "tri").unwrap();
    }

    #[test]
    fn coupled_interface_rejects_same_signed_orientation_deterministically() {
        let (mut source, land, mut ocean) = closed_tetrahedron_tri_contract();
        source.center_neighbors[4] = vec![3, 4, 5];
        ocean.delivered_source_cells[0].1 = vec![3, 4, 5];

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("same-directed shared edge must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: canonical source interface edge 3-4 has non-opposite orientation land=3->4 ocean=3->4"
        );
    }

    #[test]
    fn hex_output_orients_shared_edges_oppositely() {
        let mut mesh = UnstructuredMesh {
            m_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 1.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 1.0 },
                LonLatPoint {
                    lon: 1.0,
                    lat: -1.0,
                },
            ],
            w_points: Vec::new(),
            m_to_w: Vec::new(),
            w_to_m: vec![vec![1], vec![2, 3, 4], vec![2, 3, 5]],
            n_w_to_m: vec![1, 3, 3],
        };

        orient_hex_cells(&mut mesh, true).unwrap();

        let has_edge = |row: &[i32], start, end| {
            (0..row.len()).any(|slot| row[slot] == start && row[(slot + 1) % row.len()] == end)
        };
        let first = has_edge(&mesh.w_to_m[1], 2, 3);
        let second = has_edge(&mesh.w_to_m[2], 3, 2);
        assert!(first && second);
    }

    #[test]
    fn cartesian_hex_output_orients_cells_counterclockwise() {
        let mut mesh = UnstructuredMesh {
            m_points: vec![
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint {
                    lon: 1_000_000.0,
                    lat: 0.0,
                },
                LonLatPoint {
                    lon: 0.0,
                    lat: 1_000_000.0,
                },
            ],
            w_points: Vec::new(),
            m_to_w: Vec::new(),
            w_to_m: vec![vec![1], vec![2, 4, 3]],
            n_w_to_m: vec![1, 3],
        };

        orient_hex_cells(&mut mesh, false).unwrap();

        assert_eq!(mesh.w_to_m[1], vec![3, 4, 2]);
    }

    #[test]
    fn hex_output_rejects_placeholder_inside_physical_ring() {
        let mut mesh = UnstructuredMesh {
            m_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; 4],
            w_points: Vec::new(),
            m_to_w: Vec::new(),
            w_to_m: vec![vec![1], vec![2, 3, 1]],
            n_w_to_m: vec![1, 3],
        };

        let error = orient_hex_cells(&mut mesh, false).expect_err("placeholder must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert_eq!(
            error.to_string(),
            "hex cell 1 contains a placeholder inside its physical ring"
        );
    }

    #[test]
    fn coupled_interface_rejects_delivered_orientation_change() {
        let (source, land, mut ocean) = closed_tetrahedron_tri_contract();
        ocean.delivered_source_cells[0].1.reverse();

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("reversing a delivered cell must fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains(
            "ocean source cell 4 boundary [3, 4, 5] differs from canonical source ring [3, 5, 4]"
        ));
    }

    #[test]
    fn coupled_interface_rejects_lost_delivered_side_deterministically() {
        let (source, land, mut ocean) = closed_tetrahedron_tri_contract();
        ocean.delivered_source_cells.remove(0);

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("missing ocean source cell must fail");

        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: ocean delivered source cells differ after clipping/orphan cleanup: missing=[4] unexpected=[]"
        );
    }

    #[test]
    fn coupled_interface_rejects_source_cell_omitted_by_both_products() {
        let (source, mut land, ocean) = closed_tetrahedron_tri_contract();
        land.active_source_centers[2] = false;
        land.delivered_source_cells.remove(0);
        land.kept -= 1;

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("every source cell must belong to exactly one product");

        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: source cell 2 was omitted by both land and ocean products"
        );
    }

    #[test]
    fn coupled_interface_rejects_source_cell_delivered_by_both_products() {
        let (source, land, mut ocean) = closed_tetrahedron_tri_contract();
        ocean.active_source_centers[2] = true;
        ocean.delivered_source_cells.push((2, vec![2, 3, 4]));
        ocean.kept += 1;

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("every source cell must belong to exactly one product");

        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: source cell 2 was delivered by both land and ocean products"
        );
    }

    #[test]
    fn coupled_interface_rejects_inconsistent_kept_count() {
        let (source, mut land, ocean) = closed_tetrahedron_tri_contract();
        land.kept += 1;

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("reported kept count must match retained source cells");

        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: land reported 3 kept cells but clipping/orphan cleanup retained 2 source cells"
        );
    }

    #[test]
    fn coupled_interface_rejects_partial_source_edge_or_t_junction() {
        let mut source = coupled_layout(
            vec![
                vec![2, 3, 7, 6],
                vec![3, 4, 8, 7],
                vec![4, 5, 9, 8],
                vec![5, 10, 11, 9],
            ],
            12,
        );
        source.vertex_points[4] = LonLatPoint { lon: 0.0, lat: 0.0 };
        source.vertex_points[8] = LonLatPoint { lon: 2.0, lat: 0.0 };
        // Vertex 9 is a real geometric midpoint of the canonical 4-8 interface.
        source.vertex_points[9] = LonLatPoint { lon: 1.0, lat: 0.0 };
        let mut land = coupled_delivery(
            source.ustr_points,
            vec![(2, vec![2, 3, 7, 6]), (3, vec![3, 4, 8, 7])],
        );
        let ocean = coupled_delivery(
            source.ustr_points,
            vec![(4, vec![4, 5, 9, 8]), (5, vec![5, 10, 11, 9])],
        );
        land.delivered_source_cells[1].1 = vec![3, 4, 9, 8, 7];

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "hex")
            .expect_err("partial source edge must fail");

        assert!(error.to_string().contains(
            "land source cell 3 boundary [3, 4, 9, 8, 7] differs from canonical source ring [3, 4, 8, 7] (loss, duplication, T-junction, partial overlap, or orientation change)"
        ));
    }

    #[test]
    fn coupled_publication_rolls_back_the_first_rename_if_the_second_fails() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "earthmesh_coupled_publish_{}_{}",
            std::process::id(),
            stamp
        ));
        fs::create_dir_all(&root).unwrap();
        let land_staged = root.join("land.staged");
        let ocean_staged = root.join("ocean.staged");
        let land_output = root.join("land.nc4");
        let ocean_output = root.join("missing-parent").join("ocean.nc4");
        fs::write(&land_staged, b"land").unwrap();
        fs::write(&ocean_staged, b"ocean").unwrap();

        publish_coupled_staged_outputs(&land_staged, &land_output, &ocean_staged, &ocean_output)
            .expect_err("second rename must fail when its destination parent is absent");

        assert!(!land_output.exists());
        assert!(!ocean_output.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn coupled_interface_rejects_duplicate_source_edge_per_side() {
        let source = coupled_layout(vec![vec![2, 3, 4], vec![2, 3, 5], vec![3, 2, 6]], 7);
        let land = coupled_delivery(
            source.ustr_points,
            vec![(2, vec![2, 3, 4]), (3, vec![2, 3, 5])],
        );
        let ocean = coupled_delivery(source.ustr_points, vec![(4, vec![3, 2, 6])]);

        let error = validate_coupled_source_edge_interface(&source, &land, &ocean, "tri")
            .expect_err("duplicate land interface segment must fail");

        assert_eq!(
            error.to_string(),
            "LandOceanCoupled delivered interface contract failed: canonical source interface edge 2-3 occurs 2 times on land and 1 times on ocean; expected exactly once per side"
        );
    }

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
