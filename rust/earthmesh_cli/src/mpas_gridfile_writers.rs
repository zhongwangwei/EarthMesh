use crate::build_mpas_mesh_from_unstructured_one_based;
use crate::read_unstructured_mesh_netcdf;
use crate::subset_mpas_mesh;
use crate::write_icon_grid_netcdf;
use crate::write_mpas_graph_info;
use crate::write_mpas_mesh_netcdf;
use crate::write_mpas_ocean_mesh_netcdf;
use crate::GridRegion;
use crate::MpasFullMeshPipelineReport;
use crate::{build_mpas_simple_mesh_from_unstructured_one_based, write_mpas_simple_mesh_netcdf};
use std::io;
use std::path::Path;

/// Write an ICON triangular C-grid from the same validated dual connectivity
/// used by the MPAS adapters.
pub fn write_standard_icon_from_gridfile(
    gridfile: impl AsRef<Path>,
    output: impl AsRef<Path>,
    nxp: usize,
) -> io::Result<crate::IconGridWriteReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let mpas = build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, 1)?;
    write_icon_grid_netcdf(output, &mpas)
}

pub fn write_standard_mpas_simple_from_gridfile(
    gridfile: impl AsRef<Path>,
    output: impl AsRef<Path>,
    nxp: usize,
) -> io::Result<crate::MpasSimpleMeshWriteReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let simple = build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &cellwidth)?;
    write_mpas_simple_mesh_netcdf(output, &simple)
}

/// Build a standard MPAS mesh NetCDF (+ `graph.info`) straight from a base
/// `gridfile`, without the spring/refine pipeline or a cellwidth file. A uniform
/// cellwidth is synthesized: for an unrefined mesh every cell has the same width,
/// so `meshDensity == 1`, which is exactly correct. Reuses the same validated
/// builder/writer as the full pipeline, so the output is the standard MPAS schema.
pub fn write_standard_mpas_from_gridfile(
    gridfile: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    nxp: usize,
) -> io::Result<MpasFullMeshPipelineReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let mpas = build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, 1)?;
    let mesh_report = write_mpas_mesh_netcdf(mesh_output, &mpas)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &mpas.cells_on_cell,
        &mpas.cells_on_edge,
        &mpas.n_edges_on_cell,
    )?;
    Ok(MpasFullMeshPipelineReport {
        mesh: mesh_report,
        graph_info,
    })
}

/// Write the full MPAS-Ocean schema using physical metre/m² metrics and the
/// canonical MPAS-Ocean sphere radius.
pub fn write_standard_mpas_ocean_from_gridfile(
    gridfile: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    nxp: usize,
) -> io::Result<MpasFullMeshPipelineReport> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let mpas = build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, 1)?;
    let mesh_report = write_mpas_ocean_mesh_netcdf(mesh_output, &mpas)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &mpas.cells_on_cell,
        &mpas.cells_on_edge,
        &mpas.n_edges_on_cell,
    )?;
    Ok(MpasFullMeshPipelineReport {
        mesh: mesh_report,
        graph_info,
    })
}

/// Write a regional (limited-area) MPAS mesh + graph.info from a global hex
/// gridfile, keeping only the cells whose centre falls inside `region`.
///
/// Builds the full global MPAS mesh (the validated path), then [`subset_mpas_mesh`]
/// re-indexes it to the region: every kept cell is geometrically complete and its
/// geometry is preserved exactly, while connectivity to dropped cells collapses to
/// the MPAS `0` no-neighbour marker (boundary cells/edges). Returns the report and
/// the number of cells kept.
pub fn write_regional_mpas_from_gridfile(
    gridfile: impl AsRef<Path>,
    mesh_output: impl AsRef<Path>,
    graph_output: impl AsRef<Path>,
    region: &GridRegion,
    nxp: usize,
) -> io::Result<(MpasFullMeshPipelineReport, usize)> {
    let mesh = read_unstructured_mesh_netcdf(gridfile)?;
    let base_width = if nxp > 0 { 7680.0 / nxp as f64 } else { 1.0 };
    let cellwidth = vec![base_width; mesh.w_points.len()];
    let global = build_mpas_mesh_from_unstructured_one_based(&mesh, &cellwidth, nxp, 1)?;

    let n_cells = global.lat_cell.len();
    let mut keep_cell = vec![false; n_cells];
    let mut kept = 0usize;
    for c in 1..n_cells {
        let lon_deg = global.lon_cell[c].to_degrees();
        let lat_deg = global.lat_cell[c].to_degrees();
        if region.contains(lon_deg, lat_deg) {
            keep_cell[c] = true;
            kept += 1;
        }
    }
    if kept == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "region contains no cells",
        ));
    }

    let regional = subset_mpas_mesh(&global, &keep_cell)?;
    let mesh_report = write_mpas_mesh_netcdf(mesh_output, &regional)?;
    let graph_info = write_mpas_graph_info(
        graph_output,
        10,
        &regional.cells_on_cell,
        &regional.cells_on_edge,
        &regional.n_edges_on_cell,
    )?;
    Ok((
        MpasFullMeshPipelineReport {
            mesh: mesh_report,
            graph_info,
        },
        kept,
    ))
}

/// Deliver an admitted closed global grid using its producer-owned nominal W
/// widths. Open/masked meshes need parent metrics before they can use this path.
/// Returns the final mesh and optional graph paths; MPAS-Simple has no graph.
pub fn write_mpas_from_final_gridfile(
    gridfile: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    format: earthmesh_project::ModelFormat,
) -> io::Result<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    write_final_mpas(gridfile.as_ref(), None, output_dir.as_ref(), format)
}

/// Deliver an exact whole-cell selection using an explicit closed global parent.
/// Parent geometry/metrics and density reference survive; boundary connectivity
/// is reindexed by the existing MPAS subset adapter, not reconstructed.
pub fn write_mpas_from_final_gridfile_with_parent(
    gridfile: impl AsRef<Path>,
    parent_gridfile: impl AsRef<Path>,
    output_dir: impl AsRef<Path>,
    format: earthmesh_project::ModelFormat,
) -> io::Result<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    write_final_mpas(
        gridfile.as_ref(),
        Some(parent_gridfile.as_ref()),
        output_dir.as_ref(),
        format,
    )
}

fn write_final_mpas(
    gridfile: &Path,
    parent: Option<&Path>,
    output_dir: &Path,
    format: earthmesh_project::ModelFormat,
) -> io::Result<(std::path::PathBuf, Option<std::path::PathBuf>)> {
    use crate::atomic_output::{publish_artifacts, validate_output_path};
    use earthmesh_project::ModelFormat;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    if !matches!(
        format,
        ModelFormat::Mpas | ModelFormat::MpasOcean | ModelFormat::MpasSimple
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected an MPAS model format",
        ));
    }
    let source = parent.unwrap_or(gridfile);
    let context = required_mpas_context(source)?;
    let mesh = read_unstructured_mesh_netcdf(source)?;
    crate::validate_published_cell_degrees(&mesh, "hex")?;
    let points = crate::read_gridfile_mesh_points(source)?;
    validate_final_mpas_topology(&points, true)?;
    let selection = if parent.is_some() {
        let selected_context = required_mpas_context(gridfile)?;
        let selected_points = crate::read_gridfile_mesh_points(gridfile)?;
        validate_final_mpas_topology(&selected_points, false)?;
        let rows = crate::regional_gridfile_writers::verify_whole_cell_lineage(
            source,
            gridfile,
            &selected_points,
        )?;
        let source_first = crate::gridfile_w_row_layout(&points).first_physical_row;
        let selected_first = crate::gridfile_w_row_layout(&selected_points).first_physical_row;
        if context.base_nxp != selected_context.base_nxp
            || context.step != selected_context.step
            || context.source != selected_context.source
            || context.density_reference_width_km != selected_context.density_reference_width_km
            || rows.iter().enumerate().any(|(i, &row)| {
                selected_context.cellwidth_km[selected_first + i]
                    != context.cellwidth_km[source_first + row - 1]
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "selected MPAS width context differs from explicit parent",
            ));
        }
        Some(rows)
    } else {
        None
    };
    let first =
        crate::unstructured_mesh_support::unstructured_w_row_layout(&mesh).first_physical_row;
    // The builder's local-min normalization is not the producer-global reference.
    // Never infer nominal sizes from final geometry. Only the explicit HField
    // source below uses its generation-demand reference for nominalMinDc.
    let density = std::iter::once(1.0)
        .chain(
            context.cellwidth_km[first..]
                .iter()
                .map(|width| (context.density_reference_width_km / width).powi(4)),
        )
        .collect::<Vec<_>>();
    if density.iter().any(|d| !d.is_finite() || *d <= 0.0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MPAS density must be finite and positive",
        ));
    }
    let mesh_output = output_dir.join("mesh.nc4");
    let graph_output = output_dir.join("graph.info");
    validate_output_path(gridfile, &mesh_output)?;
    validate_output_path(gridfile, &graph_output)?;
    validate_output_path(source, &mesh_output)?;
    validate_output_path(source, &graph_output)?;
    if mesh_output.exists() {
        validate_output_path(&mesh_output, &graph_output)?;
    }
    let stage = output_dir.join(format!(
        ".mpas-tmp-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    fs::create_dir(&stage)?;
    let result = (|| {
        let staged_mesh = stage.join("mesh.nc4");
        let staged_graph = stage.join("graph.info");
        if format == ModelFormat::MpasSimple && selection.is_none() {
            let mut simple =
                build_mpas_simple_mesh_from_unstructured_one_based(&mesh, &context.cellwidth_km)?;
            if simple.mesh_density.len() != density.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MPAS physical W row count mismatch",
                ));
            }
            simple.mesh_density = density;
            write_mpas_simple_mesh_netcdf(&staged_mesh, &simple)?;
            context.write_delivery_provenance(&staged_mesh)?;
            publish_artifacts(&[(&staged_mesh, &mesh_output)], &[&graph_output])?;
            Ok((mesh_output, None))
        } else {
            let mut full = build_mpas_mesh_from_unstructured_one_based(
                &mesh,
                &context.cellwidth_km,
                context.base_nxp,
                context.step,
            )?;
            if full.mesh_density.len() != density.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MPAS physical W row count mismatch",
                ));
            }
            if context.source == crate::mpas_gridfile_context::HFIELD_QUANTIZED_DEMAND_V1 {
                full.nominal_min_dc = context.density_reference_width_km * 1000.0
                    / earthmesh_core::EARTH_RADIUS_METERS;
            }
            if !full.nominal_min_dc.is_finite() || full.nominal_min_dc <= 0.0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MPAS nominalMinDc is not positive for the recorded width source",
                ));
            }
            full.mesh_density = density;
            if let Some(rows) = &selection {
                full = crate::mpas_subset::subset_mpas_mesh_in_cell_order(&full, rows)?;
            }
            if format == ModelFormat::MpasSimple {
                let simple = crate::MpasSimpleMesh {
                    x_cell: full.x_cell,
                    y_cell: full.y_cell,
                    z_cell: full.z_cell,
                    x_vertex: full.x_vertex,
                    y_vertex: full.y_vertex,
                    z_vertex: full.z_vertex,
                    cells_on_vertex: full.cells_on_vertex,
                    mesh_density: full.mesh_density,
                };
                write_mpas_simple_mesh_netcdf(&staged_mesh, &simple)?;
                context.write_delivery_provenance(&staged_mesh)?;
                publish_artifacts(&[(&staged_mesh, &mesh_output)], &[&graph_output])?;
                return Ok((mesh_output, None));
            }
            if format == ModelFormat::MpasOcean {
                write_mpas_ocean_mesh_netcdf(&staged_mesh, &full)?;
            } else {
                write_mpas_mesh_netcdf(&staged_mesh, &full)?;
            }
            context.write_delivery_provenance(&staged_mesh)?;
            write_mpas_graph_info(
                &staged_graph,
                10,
                &full.cells_on_cell,
                &full.cells_on_edge,
                &full.n_edges_on_cell,
            )?;
            // Mesh is the readiness marker: publish it after graph, restore last.
            publish_artifacts(
                &[(&staged_graph, &graph_output), (&staged_mesh, &mesh_output)],
                &[],
            )?;
            Ok((mesh_output, Some(graph_output)))
        }
    })();
    if let Err(error) = fs::remove_dir_all(&stage) {
        eprintln!(
            "earthmesh_cli: MPAS staging cleanup {}: {error}",
            stage.display()
        );
    }
    result
}

fn required_mpas_context(
    path: &Path,
) -> io::Result<crate::mpas_gridfile_context::MpasGridfileContext> {
    crate::mpas_gridfile_context::read_mpas_gridfile_context(path)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData,
            "missing persisted MPAS width context; the producer must supply nominal W widths (uniform fallback is not allowed)"))
}

fn validate_final_mpas_topology(
    points: &crate::GridfileMeshPoints,
    closed: bool,
) -> io::Result<()> {
    use earthmesh_quality::topology::{
        boundary_topology, connected_component_count, euler_characteristic,
        genus_zero_euler_expectation, MeshTopologyValidator, Severity, TopologyIssueType,
    };
    let input = crate::grid_quality_pipeline::quality_input_from_gridfile_hex_native(points)?;
    if input
        .cells
        .iter()
        .any(|cell| !(5..=7).contains(&cell.vertices.len()))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MPAS final cells require 5–7 corners",
        ));
    }
    let boundary = boundary_topology(&input);
    let euler = euler_characteristic(&input);
    if closed && (boundary.edge_count != 0 || euler != 2 || connected_component_count(&input) != 1)
    {
        return Err(io::Error::new(io::ErrorKind::Unsupported,
            "regional/masked MPAS final delivery requires global-parent metrics and exact cell mapping; expected one closed sphere"));
    }
    if !closed && genus_zero_euler_expectation(&input, &boundary) != Some(euler) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MPAS selected topology has invalid regional Euler/boundary structure",
        ));
    }
    if let Some(issue) = MeshTopologyValidator::new(&input)
        .validate_all()
        .into_iter()
        .find(|issue| {
            issue.severity == Severity::Fail
                && (closed
                    || !matches!(
                        issue.issue_type,
                        TopologyIssueType::DisconnectedMesh | TopologyIssueType::OrphanCell
                    ))
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "MPAS final topology {}: {}",
                issue.issue_type.as_str(),
                issue.message
            ),
        ));
    }
    Ok(())
}
