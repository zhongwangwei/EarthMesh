//! Shared Project -> post-mesh hydro execution stage.
//!
//! This module owns orchestration only. The geometry, MERIT classification,
//! overlay, coupling, and refinement planning remain in their existing focused
//! modules so CLI and GUI callers execute identical hydro semantics.

use std::collections::BTreeMap;
use std::f64::consts::PI;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use earthmesh_project::{
    read_close_mask_nml_points, read_lonlat_text_points, read_shapefile_polygon_components,
    read_shapefile_polygon_parts, transform_close_boundary, CloseBoundaryGeometry, CloseMaskFormat,
    GeometryPoint, MeshCellKind, MeshDomainKind, ProjectConfig, ProjectLayerRole, RegionShape,
};

use crate::cama_binary_io::{
    CamaLonLatBbox, CamaReachClassificationThresholds, CamaReachInventoryReport, CamaReachRecord,
};
use crate::cama_reach_inventory::{
    classify_cama_reach_record, read_cama_reach_inventory_from_map_dir,
    write_cama_reach_inventory_point_geojson,
};
use crate::hydro_delivery_cells::write_gridfile_cell_polygons_geojson_with_report;
use crate::hydro_delivery_intersections::HydroDomainComponent;
use crate::hydro_delivery_refine_workflow::{run_project_hydro_workflow, HydroRefinementPolicy};
use crate::hydro_workflow_types::HydroWorkflowReport;
use crate::merit_hydro_io::{
    read_merit_hydro_window, write_merit_hydro_mask_geojson_layers, MeritMaskThresholds,
};
use crate::merit_tile_selection::{select_merit_hydro_tiles, MeritLonLatBbox};
use crate::project_coast_refinement::write_project_coast_refinement_cells;
use crate::resolve_project_path;
use crate::unstructured_mesh_support::GridfileCellKind;

#[derive(Clone, Debug)]
pub struct ProjectHydroReport {
    pub cells_geojson: PathBuf,
    pub cell_count: usize,
    pub rejected_unsupported_cell_count: usize,
    pub corridors_geojson: PathBuf,
    pub cama_reaches_geojson: Option<PathBuf>,
    pub cama_river_mouths_geojson: Option<PathBuf>,
    pub cama_corridors_geojson: Option<PathBuf>,
    pub cama_reach_count: usize,
    pub cama_river_mouth_count: usize,
    pub cama_corridor_count: usize,
    pub manifest_path: PathBuf,
    pub hydro: HydroWorkflowReport,
}

#[derive(Clone, Debug)]
struct ProjectHydroDomain {
    components: Vec<HydroDomainComponent>,
    /// Directed west/south/east/north bbox used to filter mesh-cell centers.
    bbox: [f64; 4],
    /// Non-wrapping windows used by rectangular MERIT/CaMa readers.
    query_bboxes: Vec<[f64; 4]>,
    /// Preserve O(1) center filtering for literal bbox domains. The densified
    /// spherical components remain the source of truth for polygon clipping.
    directed_bbox: Option<[f64; 4]>,
}

impl ProjectHydroDomain {
    fn contains(&self, point: (f64, f64)) -> bool {
        if let Some(bbox) = self.directed_bbox {
            return directed_bbox_contains(bbox, point);
        }
        self.components.iter().any(|component| {
            spherical_point_relation(point, &component.shell) != SphericalPointRelation::Outside
                && component.holes.iter().all(|hole| {
                    spherical_point_relation(point, hole) == SphericalPointRelation::Outside
                })
        })
    }
}

/// Execute the hydro stage declared by `project.hydro_coast`, if present.
///
/// Relative `merit_root` paths are resolved against the Project file rather
/// than the process working directory. The function deliberately errors on a
/// configured-but-unexecutable stage; it never silently skips missing data.
pub fn run_project_hydro_postprocess(
    project: &ProjectConfig,
    project_path: impl AsRef<Path>,
    gridfile: impl AsRef<Path>,
    out_dir: impl AsRef<Path>,
) -> io::Result<Option<ProjectHydroReport>> {
    let Some(plan) = project
        .hydro_execution_plan()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?
    else {
        return Ok(None);
    };

    let project_path = project_path.as_ref();
    let gridfile = gridfile.as_ref();
    let out_dir = out_dir.as_ref();
    if !gridfile.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Project hydro stage requires generated gridfile {}",
                gridfile.display()
            ),
        ));
    }
    let merit_root = resolve_project_path(project_path, &plan.merit_root);
    if !merit_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Project hydro stage requires MERIT-Hydro directory {}",
                merit_root.display()
            ),
        ));
    }
    let cama_root = plan
        .cama_root
        .as_deref()
        .map(|configured| resolve_project_path(project_path, configured));
    if let Some(cama_root) = &cama_root {
        if !cama_root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Project hydro stage requires CaMa map directory {}",
                    cama_root.display()
                ),
            ));
        }
    }
    let landtype = project
        .data_layers
        .iter()
        .find(|layer| {
            layer.enabled
                && layer.role == ProjectLayerRole::LandType
                && !layer.path.trim().is_empty()
        })
        .map(|layer| resolve_project_path(project_path, layer.path.trim()));
    if project.target.kind == MeshDomainKind::Coupled && landtype.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project hydro production coupling requires an enabled landtype data layer",
        ));
    }
    if let Some(landtype) = &landtype {
        if !landtype.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Project hydro production coupling requires landtype file {}",
                    landtype.display()
                ),
            ));
        }
    }
    let landtype_gridnum_perdegree = if let Some(landtype) = &landtype {
        crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(landtype)?
    } else {
        1
    };
    let domain = project_hydro_domain(project_path, &plan.domain)?;
    // Validate every external input before mutating the owned output set.
    reset_project_hydro_output_dir(out_dir)?;

    let cells_geojson = out_dir.join("cells.geojson");
    let cell_kind = match project.target.cell {
        MeshCellKind::Hex => GridfileCellKind::Hex,
        MeshCellKind::Tri => GridfileCellKind::Tri,
    };
    let cell_export = write_gridfile_cell_polygons_geojson_with_report(
        gridfile,
        &cells_geojson,
        cell_kind,
        Some(domain.bbox),
        None,
    )?;
    if cell_export.emitted_cells == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Project hydro stage produced no mesh cells inside the configured bbox",
        ));
    }
    if cell_export.rejected_unsupported_cells > 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Project hydro stage rejected {} mesh cells whose great-circle arcs exceed the supported limit; refusing a partial production coupling",
                cell_export.rejected_unsupported_cells
            ),
        ));
    }
    eprintln!(
        "earthmesh_cli: project hydro: exported {} mesh cells; reading native MERIT-Hydro windows",
        cell_export.emitted_cells
    );

    let mut windows = Vec::new();
    // Read the larger of one native MERIT cell or the requested coast distance
    // beyond the footprint. Workflow clipping remains tied to `domain.components`,
    // so the halo cannot enlarge the output domain.
    let coast_buffer_km = if plan.max_level >= 1
        && plan.coast_refinement_enabled
        && (plan.coast_land_refinement_enabled || plan.coast_ocean_refinement_enabled)
    {
        plan.coast_buffer_km
    } else {
        0.0
    };
    for query in expanded_merit_query_bboxes(domain.bbox, coast_buffer_km) {
        let bbox = merit_bbox(query);
        let tiles = select_merit_hydro_tiles(&merit_root, bbox)?;
        for tile in tiles {
            let mut window = read_merit_hydro_window(tile, bbox, plan.merit_stride)?;
            // The mask classifier and exported coupling attributes do not use
            // flow direction. Release this native-resolution plane before the
            // multi-tile adjacency pass instead of retaining hundreds of MB.
            window.dir = Vec::new();
            windows.push(window);
        }
    }
    if windows.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no MERIT-Hydro tiles in {} intersect the Project footprint",
                merit_root.display()
            ),
        ));
    }
    eprintln!(
        "earthmesh_cli: project hydro: loaded {} MERIT-Hydro windows; classifying native river/coast cells",
        windows.len()
    );
    let merit_dir = out_dir.join("merit_masks");
    let thresholds = MeritMaskThresholds {
        r2_width_m: plan.r2_width_m,
        r3_width_m: plan.r3_width_m,
        r2_upa_km2: plan.r2_upa_km2,
        r3_upa_km2: plan.r3_upa_km2,
        river_width_refinement_m: plan.river_width_threshold_m,
        river_upstream_area_refinement_km2: plan.river_upstream_area_threshold_km2,
    };
    let merit_window_count = windows.len();
    let merit =
        write_merit_hydro_mask_geojson_layers(&windows, thresholds, &merit_dir, false, false)?;
    let coast_refinement_cells = if coast_buffer_km > 0.0 {
        let output = out_dir.join("coast_refinement_cells.geojson");
        write_project_coast_refinement_cells(
            &cells_geojson,
            &merit.combined_geojson,
            &windows,
            coast_buffer_km,
            plan.coast_land_refinement_enabled,
            plan.coast_ocean_refinement_enabled,
            &output,
        )?;
        let count = filter_centered_geojson_to_domain(&output, &domain)?;
        eprintln!(
            "earthmesh_cli: project hydro: selected {count} coast-distance cells within {coast_buffer_km} km"
        );
        Some(output)
    } else {
        None
    };
    drop(windows);
    eprintln!(
        "earthmesh_cli: project hydro: compacted {} native river/coast cells into {} disjoint corridors; intersecting mesh cells",
        merit
            .mask_counts
            .iter()
            .filter(|(class, _)| matches!(class.as_str(), "R2" | "R3" | "COAST_LAND" | "COAST_OCEAN"))
            .map(|(_, count)| count)
            .sum::<usize>(),
        merit.combined_feature_count
    );

    let (
        cama_reaches_geojson,
        cama_river_mouths_geojson,
        cama_corridors_geojson,
        cama_reach_count,
        cama_river_mouth_count,
        cama_corridor_count,
    ) = if let Some(cama_root) = &cama_root {
        let mut inventory =
            read_project_cama_inventory(cama_root, &domain.query_bboxes, plan.target_dx_km)?;
        let before = inventory.records.len();
        inventory
            .records
            .retain(|record| domain.contains((record.lon, record.lat)));
        inventory.skipped_cells += before - inventory.records.len();
        inventory.valid_channel_cells = inventory.records.len();
        let reaches = out_dir.join("cama_reaches.geojson");
        write_cama_reach_inventory_point_geojson(&inventory, &reaches)?;
        let reach_count = inventory.records.len();
        let mut mouths_inventory = inventory.clone();
        mouths_inventory.records.retain(|record| record.is_estuary);
        mouths_inventory.valid_channel_cells = mouths_inventory.records.len();
        let mouth_count = mouths_inventory.records.len();
        let mouths = out_dir.join("cama_river_mouths.geojson");
        write_cama_reach_inventory_point_geojson(&mouths_inventory, &mouths)?;
        let corridors = out_dir.join("cama_reach_corridors.geojson");
        let corridor_count = write_cama_reach_corridor_geojson(
            &inventory,
            &corridors,
            plan.r2_width_m,
            plan.r3_width_m,
        )?;
        (
            Some(reaches),
            Some(mouths),
            Some(corridors),
            reach_count,
            mouth_count,
            corridor_count,
        )
    } else {
        (None, None, None, 0, 0, 0)
    };
    let corridors_geojson = if let Some(cama_corridors) = &cama_corridors_geojson {
        let combined = out_dir.join("combined_hydro_corridors.geojson");
        merge_corridor_geojson(&merit.combined_geojson, cama_corridors, &combined)?;
        combined
    } else {
        merit.combined_geojson.clone()
    };
    let workflow_dir = out_dir.join("workflow");
    let hydro = run_project_hydro_workflow(
        &cells_geojson,
        &corridors_geojson,
        &workflow_dir,
        &plan.include_classes,
        0.0,
        false,
        Some(&domain.components),
        plan.max_level,
        None,
        landtype.as_ref().map(|_| gridfile),
        landtype.as_deref(),
        landtype_gridnum_perdegree,
        cama_corridors_geojson.is_some(),
        coast_refinement_cells.as_deref(),
        HydroRefinementPolicy {
            river_width: plan.river_width_refinement_enabled,
            river_upstream_area: plan.river_upstream_area_refinement_enabled,
            legacy_river_classes: false,
            coast_land: plan.coast_refinement_enabled && plan.coast_land_refinement_enabled,
            coast_ocean: plan.coast_refinement_enabled && plan.coast_ocean_refinement_enabled,
        },
    )?;
    eprintln!(
        "earthmesh_cli: project hydro: completed {} cell/class intersections and {} coupling rows",
        hydro.intersection_cells, hydro.coupling_rows
    );
    let manifest_path = out_dir.join("project_hydro_manifest.json");
    let optional_path = |path: &Option<PathBuf>| {
        path.as_ref()
            .map(|path| {
                format!(
                    "\"{}\"",
                    crate::json_escape_string(&path.display().to_string())
                )
            })
            .unwrap_or_else(|| "null".to_string())
    };
    fs::write(
        &manifest_path,
        format!(
            "{{\n  \"kind\": \"earthmesh_project_hydro\",\n  \"domain_parts\": {},\n  \"cell_count\": {},\n  \"rejected_unsupported_cell_count\": {},\n  \"merit_window_count\": {},\n  \"merit_stride\": {},\n  \"cama_reach_count\": {},\n  \"cama_river_mouth_count\": {},\n  \"cama_corridor_count\": {},\n  \"artifacts\": {{\n    \"cells_geojson\": \"{}\",\n    \"corridors_geojson\": \"{}\",\n    \"merit_corridors_geojson\": \"{}\",\n    \"cama_reaches_geojson\": {},\n    \"cama_river_mouths_geojson\": {},\n    \"cama_corridors_geojson\": {},\n    \"coast_refinement_cells_geojson\": {},\n    \"hydro_workflow_manifest\": \"{}\"\n  }}\n}}\n",
            domain.components.len(),
            cell_export.emitted_cells,
            cell_export.rejected_unsupported_cells,
            merit_window_count,
            plan.merit_stride,
            cama_reach_count,
            cama_river_mouth_count,
            cama_corridor_count,
            crate::json_escape_string(&cells_geojson.display().to_string()),
            crate::json_escape_string(&corridors_geojson.display().to_string()),
            crate::json_escape_string(&merit.combined_geojson.display().to_string()),
            optional_path(&cama_reaches_geojson),
            optional_path(&cama_river_mouths_geojson),
            optional_path(&cama_corridors_geojson),
            optional_path(&coast_refinement_cells),
            crate::json_escape_string(&hydro.manifest_path.display().to_string()),
        ),
    )?;
    Ok(Some(ProjectHydroReport {
        cells_geojson,
        cell_count: cell_export.emitted_cells,
        rejected_unsupported_cell_count: cell_export.rejected_unsupported_cells,
        corridors_geojson,
        cama_reaches_geojson,
        cama_river_mouths_geojson,
        cama_corridors_geojson,
        cama_reach_count,
        cama_river_mouth_count,
        cama_corridor_count,
        manifest_path,
        hydro,
    }))
}

fn merit_bbox(bbox: [f64; 4]) -> MeritLonLatBbox {
    MeritLonLatBbox {
        west: bbox[0],
        south: bbox[1],
        east: bbox[2],
        north: bbox[3],
    }
}

fn expanded_merit_query_bboxes(
    [west, south, east, north]: [f64; 4],
    coast_buffer_km: f64,
) -> Vec<[f64; 4]> {
    const MERIT_NATIVE_CELL_DEG: f64 = 3.0 / 3_600.0;
    let lat_halo =
        (coast_buffer_km / earthmesh_core::KM_PER_DEGREE_EQUATOR).max(MERIT_NATIVE_CELL_DEG);
    let expanded_south = (south - lat_halo).max(-90.0);
    let expanded_north = (north + lat_halo).min(90.0);
    let max_abs_lat = expanded_south.abs().max(expanded_north.abs());
    let lon_halo = if max_abs_lat >= 89.999 {
        180.0
    } else {
        (lat_halo / max_abs_lat.to_radians().cos()).min(180.0)
    };
    let span = directed_longitude_span(west, east);
    if span + 2.0 * lon_halo >= 360.0 {
        return vec![[-180.0, expanded_south, 180.0, expanded_north]];
    }
    split_directed_bbox([
        wrap_lon(west - lon_halo),
        expanded_south,
        wrap_lon(east + lon_halo),
        expanded_north,
    ])
}

fn filter_centered_geojson_to_domain(
    path: &Path,
    domain: &ProjectHydroDomain,
) -> io::Result<usize> {
    let root = crate::JsonParser::new(&crate::read_text_maybe_gzip(path)?).parse()?;
    let mut features = Vec::new();
    for feature in crate::geojson_feature_nodes(&root) {
        let center = feature
            .as_object()
            .and_then(|object| object.get("properties"))
            .and_then(crate::JsonNode::as_object)
            .and_then(|properties| {
                Some((
                    properties.get("center_lon")?.as_f64()?,
                    properties.get("center_lat")?.as_f64()?,
                ))
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "coast refinement cell is missing center_lon/center_lat",
                )
            })?;
        if domain.contains(center) {
            features.push(crate::json_node_to_string(feature));
        }
    }
    fs::write(
        path,
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}\n",
            features.join(",")
        ),
    )?;
    Ok(features.len())
}

fn reset_project_hydro_output_dir(out_dir: &Path) -> io::Result<()> {
    if out_dir.exists() && !out_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Project hydro output path is not a directory: {}",
                out_dir.display()
            ),
        ));
    }
    fs::create_dir_all(out_dir)?;
    for owned in [
        "cells.geojson",
        "cama_reaches.geojson",
        "cama_river_mouths.geojson",
        "cama_reach_corridors.geojson",
        "combined_hydro_corridors.geojson",
        "coast_refinement_cells.geojson",
        "project_hydro_manifest.json",
        "merit_masks",
        "workflow",
    ] {
        let path = out_dir.join(owned);
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() && !metadata.file_type().is_symlink() {
            fs::remove_dir_all(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn read_project_cama_inventory(
    root: &Path,
    query_bboxes: &[[f64; 4]],
    target_dx_km: f64,
) -> io::Result<CamaReachInventoryReport> {
    let mut merged: Option<CamaReachInventoryReport> = None;
    let mut records = BTreeMap::new();
    for bbox in query_bboxes {
        let inventory = read_cama_reach_inventory_from_map_dir(
            root,
            CamaLonLatBbox {
                west: bbox[0],
                south: bbox[1],
                east: bbox[2],
                north: bbox[3],
            },
            target_dx_km,
            1.0e-6,
            true,
        )?;
        for record in &inventory.records {
            records.insert(record.reach_id.clone(), record.clone());
        }
        if merged.is_none() {
            merged = Some(inventory);
        }
    }
    let mut merged = merged.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Project footprint has no CaMa query window",
        )
    })?;
    merged.records = records.into_values().collect();
    merged.valid_channel_cells = merged.records.len();
    Ok(merged)
}

/// Export the R2/R3 subset of a CaMa inventory as finite-width spherical reach
/// footprints. Linked reaches are represented by geodesic capsules; terminal
/// reaches use a circular cap because CaMa supplies no downstream direction.
fn write_cama_reach_corridor_geojson(
    inventory: &CamaReachInventoryReport,
    output: &Path,
    r2_width_m: f64,
    r3_width_m: f64,
) -> io::Result<usize> {
    if !r2_width_m.is_finite() || r2_width_m <= 0.0 || !r3_width_m.is_finite() || r3_width_m <= 0.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CaMa corridor widths must be finite and positive",
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut features = Vec::new();
    for record in &inventory.records {
        let classification =
            classify_cama_reach_record(record, CamaReachClassificationThresholds::default())?;
        if !matches!(classification.river_class.as_str(), "R2" | "R3") {
            continue;
        }
        let policy_width_m = if classification.river_class == "R3" {
            r3_width_m
        } else {
            r2_width_m
        };
        let corridor_width_m = classification.effective_width_m.max(policy_width_m);
        let downstream = cama_downstream_point(inventory, record)?;
        let ring =
            geodesic_reach_capsule((record.lon, record.lat), downstream, corridor_width_m / 2.0)?;
        let coordinates = ring
            .iter()
            .map(|&(lon, lat)| format!("[{},{}]", crate::json_number(lon), crate::json_number(lat)))
            .collect::<Vec<_>>()
            .join(",");
        features.push(format!(
            "{{\"type\":\"Feature\",\"geometry\":{{\"type\":\"Polygon\",\"coordinates\":[[{coordinates}]]}},\"properties\":{{\"corridor_width_m\":{},\"effective_width_m\":{},\"feature_id\":\"cama-corridor:{}\",\"is_estuary\":{},\"reach_id\":\"{}\",\"river_class\":\"{}\",\"source\":\"CaMa-Flood\",\"upstream_area_km2\":{},\"width_m\":{}}}}}",
            crate::json_number(corridor_width_m),
            crate::json_number(classification.effective_width_m),
            crate::json_escape_string(&record.reach_id),
            record.is_estuary,
            crate::json_escape_string(&record.reach_id),
            crate::json_escape_string(&classification.river_class),
            crate::json_number(record.upstream_area_km2),
            crate::json_number(record.width_m),
        ));
    }
    let mut handle = fs::File::create(output)?;
    writeln!(
        handle,
        "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}",
        features.join(",")
    )?;
    Ok(features.len())
}

fn cama_downstream_point(
    inventory: &CamaReachInventoryReport,
    record: &CamaReachRecord,
) -> io::Result<Option<(f64, f64)>> {
    if record.downstream_x < 0 || record.downstream_y < 0 {
        return Ok(None);
    }
    let x = usize::try_from(record.downstream_x).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "negative CaMa downstream x index",
        )
    })?;
    let y = usize::try_from(record.downstream_y).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "negative CaMa downstream y index",
        )
    })?;
    if x >= inventory.grid.nx || y >= inventory.grid.ny {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CaMa reach {} downstream index ({x},{y}) is outside the {}x{} grid",
                record.reach_id, inventory.grid.nx, inventory.grid.ny
            ),
        ));
    }
    if (x, y) == (record.x_index, record.y_index) {
        return Ok(None);
    }
    Ok(Some((
        wrap_lon(inventory.grid.lon_center(x)),
        inventory.grid.lat_center(y),
    )))
}

fn geodesic_reach_capsule(
    start: (f64, f64),
    end: Option<(f64, f64)>,
    radius_m: f64,
) -> io::Result<Vec<(f64, f64)>> {
    if !start.0.is_finite()
        || !start.1.is_finite()
        || start.1.abs() > 90.0
        || !radius_m.is_finite()
        || radius_m <= 0.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid CaMa reach center or corridor radius",
        ));
    }
    const CAP_STEPS: usize = 12;
    let mut ring = Vec::with_capacity(CAP_STEPS * 2 + 3);
    match end.filter(|point| {
        point.0.is_finite()
            && point.1.is_finite()
            && point.1.abs() <= 90.0
            && angular_distance(start, *point) > 1.0e-12
    }) {
        Some(end) => {
            let bearing = initial_bearing(start, end);
            for index in 0..=CAP_STEPS {
                let offset = -PI / 2.0 + PI * index as f64 / CAP_STEPS as f64;
                ring.push(geodesic_destination(end, bearing + offset, radius_m));
            }
            for index in 0..=CAP_STEPS {
                let offset = PI / 2.0 + PI * index as f64 / CAP_STEPS as f64;
                ring.push(geodesic_destination(start, bearing + offset, radius_m));
            }
        }
        None => {
            for index in 0..CAP_STEPS * 2 {
                let bearing = 2.0 * PI * index as f64 / (CAP_STEPS * 2) as f64;
                ring.push(geodesic_destination(start, bearing, radius_m));
            }
        }
    }
    ring.push(ring[0]);
    Ok(ring)
}

fn angular_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dlat = (b.1 - a.1).to_radians();
    let dlon = (b.0 - a.0).to_radians();
    let a_lat = a.1.to_radians();
    let b_lat = b.1.to_radians();
    let hav = (dlat / 2.0).sin().powi(2) + a_lat.cos() * b_lat.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * hav.clamp(0.0, 1.0).sqrt().asin()
}

fn initial_bearing(start: (f64, f64), end: (f64, f64)) -> f64 {
    let start_lat = start.1.to_radians();
    let end_lat = end.1.to_radians();
    let dlon = (end.0 - start.0).to_radians();
    (dlon.sin() * end_lat.cos())
        .atan2(start_lat.cos() * end_lat.sin() - start_lat.sin() * end_lat.cos() * dlon.cos())
}

fn geodesic_destination(start: (f64, f64), bearing: f64, distance_m: f64) -> (f64, f64) {
    const EARTH_RADIUS_M: f64 = earthmesh_core::EARTH_RADIUS_METERS;
    let angular = distance_m / EARTH_RADIUS_M;
    let lat = start.1.to_radians();
    let lon = start.0.to_radians();
    let out_lat = (lat.sin() * angular.cos() + lat.cos() * angular.sin() * bearing.cos()).asin();
    let out_lon = lon
        + (bearing.sin() * angular.sin() * lat.cos())
            .atan2(angular.cos() - lat.sin() * out_lat.sin());
    (wrap_lon(out_lon.to_degrees()), out_lat.to_degrees())
}

fn merge_corridor_geojson(merit: &Path, cama: &Path, output: &Path) -> io::Result<usize> {
    let mut features = Vec::new();
    for path in [merit, cama] {
        let root = crate::JsonParser::new(&crate::read_text_maybe_gzip(path)?).parse()?;
        features.extend(
            crate::geojson_feature_nodes(&root)
                .into_iter()
                .map(crate::json_node_to_string),
        );
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        output,
        format!(
            "{{\"type\":\"FeatureCollection\",\"features\":[{}]}}\n",
            features.join(",")
        ),
    )?;
    Ok(features.len())
}

fn project_hydro_domain(
    project_path: &Path,
    shape: &RegionShape,
) -> io::Result<ProjectHydroDomain> {
    match shape {
        RegionShape::Bbox { w, e, s, n } => domain_from_directed_bbox(*w, *s, *e, *n),
        RegionShape::Circle {
            lon,
            lat,
            radius_km,
        } => domain_from_circle(*lon, *lat, *radius_km),
        RegionShape::Shapefile { path } => {
            let path = resolve_project_path(project_path, path);
            let components = read_shapefile_polygon_components(&path)?
                .into_iter()
                .map(|component| HydroDomainComponent {
                    shell: component.shell,
                    holes: component.holes,
                })
                .collect();
            domain_from_components(components, "shapefile")
        }
        RegionShape::Close {
            path,
            format,
            boundary,
        } => {
            let path = resolve_project_path(project_path, path);
            let records = match format {
                CloseMaskFormat::PolygonShp => read_shapefile_polygon_parts(&path)?,
                CloseMaskFormat::LonLatText => vec![vec![read_lonlat_text_points(&path)?]],
                CloseMaskFormat::Nml => vec![vec![read_close_mask_nml_points(&path)?]],
                CloseMaskFormat::Netcdf => vec![vec![crate::read_close_mask_netcdf(&path)?
                    .points
                    .into_iter()
                    .map(|point| (point.lon, point.lat))
                    .collect()]],
            };
            if records.iter().map(Vec::len).sum::<usize>() != 1
                && matches!(
                    boundary,
                    earthmesh_project::CloseBoundaryMode::EnclosingCap { .. }
                )
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "enclosing_cap cannot preserve a multi-part close domain",
                ));
            }
            let mut transformed_records = Vec::with_capacity(records.len());
            for record in records {
                let mut transformed_rings = Vec::with_capacity(record.len());
                for ring in record {
                    let points = ring
                        .iter()
                        .map(|&(lon, lat)| GeometryPoint::new(lon, lat))
                        .collect::<Vec<_>>();
                    match transform_close_boundary(&points, boundary)
                        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
                        .geometry
                    {
                        CloseBoundaryGeometry::Polygon(points) => transformed_rings.push(
                            points
                                .into_iter()
                                .map(|point| (point.lon, point.lat))
                                .collect(),
                        ),
                        CloseBoundaryGeometry::EnclosingCap { center, radius_km } => {
                            return domain_from_circle(center.lon, center.lat, radius_km)
                        }
                    }
                }
                transformed_records.push(transformed_rings);
            }
            domain_from_ring_records(transformed_records, "close domain")
        }
    }
}

fn domain_from_directed_bbox(
    west: f64,
    south: f64,
    east: f64,
    north: f64,
) -> io::Result<ProjectHydroDomain> {
    let query_bboxes = split_directed_bbox([west, south, east, north]);
    let longitude_span = directed_longitude_span(west, east);
    let component_count = (longitude_span / 90.0).ceil().max(1.0) as usize;
    let components = (0..component_count)
        .map(|index| {
            let start = west + longitude_span * index as f64 / component_count as f64;
            let end = west + longitude_span * (index + 1) as f64 / component_count as f64;
            let segment_west = if index == 0 { west } else { wrap_lon(start) };
            let segment_east = if index + 1 == component_count {
                east
            } else {
                wrap_lon(end)
            };
            HydroDomainComponent::shell(bbox_ring([segment_west, south, segment_east, north]))
        })
        .collect();
    Ok(ProjectHydroDomain {
        components,
        bbox: [west, south, east, north],
        query_bboxes,
        directed_bbox: Some([west, south, east, north]),
    })
}

fn domain_from_circle(lon: f64, lat: f64, radius_km: f64) -> io::Result<ProjectHydroDomain> {
    const EARTH_RADIUS_KM: f64 = earthmesh_core::EARTH_RADIUS_METERS / 1000.0;
    let angular = radius_km / EARTH_RADIUS_KM;
    let center = lonlat_unit((lon, lat));
    let lon_radians = lon.to_radians();
    let east = [-lon_radians.sin(), lon_radians.cos(), 0.0];
    let north = spherical_cross(center, east);
    let mut ring = Vec::with_capacity(181);
    for index in 0..180 {
        let bearing = 2.0 * PI * index as f64 / 180.0;
        let tangent = [
            north[0] * bearing.cos() + east[0] * bearing.sin(),
            north[1] * bearing.cos() + east[1] * bearing.sin(),
            north[2] * bearing.cos() + east[2] * bearing.sin(),
        ];
        let point = [
            center[0] * angular.cos() + tangent[0] * angular.sin(),
            center[1] * angular.cos() + tangent[1] * angular.sin(),
            center[2] * angular.cos() + tangent[2] * angular.sin(),
        ];
        ring.push((
            wrap_lon(point[1].atan2(point[0]).to_degrees()),
            point[2].clamp(-1.0, 1.0).asin().to_degrees(),
        ));
    }
    ring.push(ring[0]);
    domain_from_outer_rings(vec![ring], "circle")
}

fn domain_from_outer_rings(
    rings: Vec<Vec<(f64, f64)>>,
    label: &str,
) -> io::Result<ProjectHydroDomain> {
    domain_from_ring_records(vec![rings], label)
}

fn domain_from_ring_records(
    records: Vec<Vec<Vec<(f64, f64)>>>,
    label: &str,
) -> io::Result<ProjectHydroDomain> {
    if records.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains no polygon rings"),
        ));
    }
    let mut components = Vec::new();
    for (record, rings) in records.into_iter().enumerate() {
        components.extend(domain_components_from_record(
            rings,
            &format!("{label} record {}", record + 1),
        )?);
    }
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains no polygon rings"),
        ));
    }
    domain_from_components(components, label)
}

fn domain_from_components(
    components: Vec<HydroDomainComponent>,
    label: &str,
) -> io::Result<ProjectHydroDomain> {
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains no polygon components"),
        ));
    }
    let shells = components
        .iter()
        .map(|component| component.shell.clone())
        .collect::<Vec<_>>();
    let bbox = minimal_directed_bbox(&shells)?;
    Ok(ProjectHydroDomain {
        query_bboxes: split_directed_bbox(bbox),
        bbox,
        components,
        directed_bbox: None,
    })
}

fn domain_components_from_record(
    mut rings: Vec<Vec<(f64, f64)>>,
    label: &str,
) -> io::Result<Vec<HydroDomainComponent>> {
    if rings.is_empty() || rings.iter().any(|ring| ring.len() < 3) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains a polygon ring with fewer than three vertices"),
        ));
    }
    for ring in &mut rings {
        if ring.first() != ring.last() {
            ring.push(ring[0]);
        }
    }
    if rings
        .iter()
        .any(|ring| has_nonadjacent_duplicate_vertex(ring))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} contains a bridged hole or self-touching polygon; Project hydro refuses to silently fill or flatten it"
            ),
        ));
    }
    let areas = rings
        .iter()
        .enumerate()
        .map(|(ring_index, ring)| {
            let points = ring
                .iter()
                .map(|&(lon, lat)| earthmesh_geometry::Point::new(lon, lat))
                .collect::<Vec<_>>();
            earthmesh_geometry::try_spherical_polygon_excess(
                &points,
                earthmesh_geometry::SphericalAreaBranch::Minor,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{label} ring {} is invalid: {error}", ring_index + 1),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut contains = vec![vec![false; rings.len()]; rings.len()];
    for left in 0..rings.len() {
        for right in (left + 1)..rings.len() {
            if spherical_rings_intersect_or_touch(&rings[left], &rings[right]) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{label} rings {} and {} intersect or touch; rings within one polygon record must be strictly nested or disjoint",
                        left + 1,
                        right + 1
                    ),
                ));
            }
            contains[right][left] = spherical_point_relation(rings[left][0], &rings[right])
                == SphericalPointRelation::Inside;
            contains[left][right] = spherical_point_relation(rings[right][0], &rings[left])
                == SphericalPointRelation::Inside;
            if contains[right][left] && contains[left][right] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{label} rings {} and {} have ambiguous spherical nesting",
                        left + 1,
                        right + 1
                    ),
                ));
            }
        }
    }
    let mut parents = vec![None; rings.len()];
    for child in 0..rings.len() {
        parents[child] = (0..rings.len())
            .filter(|&candidate| {
                candidate != child
                    && areas[candidate] > areas[child] + 1.0e-12
                    && contains[candidate][child]
            })
            .min_by(|&left, &right| areas[left].total_cmp(&areas[right]));
    }
    let mut depths = vec![0usize; rings.len()];
    for index in 0..rings.len() {
        let mut cursor = index;
        for depth in 0..rings.len() {
            let Some(parent) = parents[cursor] else {
                depths[index] = depth;
                break;
            };
            cursor = parent;
            if depth + 1 == rings.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{label} contains cyclic polygon nesting"),
                ));
            }
        }
    }
    Ok((0..rings.len())
        .filter(|&index| depths[index].is_multiple_of(2))
        .map(|shell| HydroDomainComponent {
            shell: rings[shell].clone(),
            holes: (0..rings.len())
                .filter(|&index| {
                    parents[index] == Some(shell) && depths[index] == depths[shell] + 1
                })
                .map(|index| rings[index].clone())
                .collect(),
        })
        .collect())
}

fn has_nonadjacent_duplicate_vertex(ring: &[(f64, f64)]) -> bool {
    for (index, point) in ring.iter().enumerate() {
        for (other_index, other) in ring.iter().enumerate().skip(index + 1) {
            let closes_ring = index == 0 && other_index + 1 == ring.len();
            let adjacent = other_index == index + 1 || closes_ring;
            if !adjacent
                && (point.0 - other.0).abs() < 1.0e-12
                && (point.1 - other.1).abs() < 1.0e-12
            {
                return true;
            }
        }
    }
    false
}

fn bbox_ring([west, south, east, north]: [f64; 4]) -> Vec<(f64, f64)> {
    const POLE_EPSILON: f64 = 1.0e-12;
    const MAX_PARALLEL_STEP_DEGREES: f64 = 0.1;
    let south_is_pole = south <= -90.0 + POLE_EPSILON;
    let north_is_pole = north >= 90.0 - POLE_EPSILON;
    if south_is_pole && north_is_pole {
        return vec![
            (east, 0.0),
            (east, north),
            (west, 0.0),
            (west, south),
            (east, 0.0),
        ];
    }
    let span = directed_longitude_span(west, east);
    let segments = (span / MAX_PARALLEL_STEP_DEGREES).ceil().max(1.0) as usize;
    let longitude_at = |index: usize| wrap_lon(west + span * index as f64 / segments as f64);
    let mut ring = Vec::with_capacity(2 * (segments + 1) + 1);
    if south_is_pole {
        ring.push((west, south));
    } else {
        ring.extend((0..=segments).map(|index| (longitude_at(index), south)));
    }
    if north_is_pole {
        ring.push((east, north));
    } else {
        ring.extend(
            (0..=segments)
                .rev()
                .map(|index| (longitude_at(index), north)),
        );
    }
    ring.push(ring[0]);
    ring
}

fn directed_longitude_span(west: f64, east: f64) -> f64 {
    let raw = east - west;
    if raw.abs() >= 360.0 - 1.0e-12 {
        360.0
    } else if raw < 0.0 {
        raw + 360.0
    } else {
        raw
    }
}

fn directed_bbox_contains([west, south, east, north]: [f64; 4], point: (f64, f64)) -> bool {
    const EPSILON: f64 = 1.0e-12;
    if !point.0.is_finite() || !point.1.is_finite() {
        return false;
    }
    let in_latitude =
        point.1 >= south.min(north) - EPSILON && point.1 <= south.max(north) + EPSILON;
    if !in_latitude {
        return false;
    }
    let span = directed_longitude_span(west, east);
    if span >= 360.0 - EPSILON {
        return true;
    }
    let offset = (wrap_lon(point.0) - wrap_lon(west)).rem_euclid(360.0);
    offset <= span + EPSILON
}

fn split_directed_bbox([west, south, east, north]: [f64; 4]) -> Vec<[f64; 4]> {
    if directed_longitude_span(west, east) >= 360.0 - 1.0e-12 {
        return vec![[-180.0, south, 180.0, north]];
    }
    if west <= east {
        vec![[west, south, east, north]]
    } else {
        vec![[west, south, 180.0, north], [-180.0, south, east, north]]
    }
}

fn minimal_directed_bbox(rings: &[Vec<(f64, f64)>]) -> io::Result<[f64; 4]> {
    let mut lons = rings
        .iter()
        .flatten()
        .map(|point| wrap_lon(point.0))
        .collect::<Vec<_>>();
    let mut south = rings
        .iter()
        .flatten()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let mut north = rings
        .iter()
        .flatten()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max);
    if lons.is_empty() || !south.is_finite() || !north.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-finite domain bounds",
        ));
    }
    let contains_north_pole = rings
        .iter()
        .any(|ring| spherical_point_relation((0.0, 90.0), ring) != SphericalPointRelation::Outside);
    let contains_south_pole = rings.iter().any(|ring| {
        spherical_point_relation((0.0, -90.0), ring) != SphericalPointRelation::Outside
    });
    if contains_north_pole || contains_south_pole {
        if contains_north_pole {
            north = 90.0;
        }
        if contains_south_pole {
            south = -90.0;
        }
        return Ok([-180.0, south, 180.0, north]);
    }
    lons.sort_by(f64::total_cmp);
    lons.dedup_by(|a, b| (*a - *b).abs() < 1.0e-12);
    if lons.len() == 1 {
        return Ok([lons[0], south, lons[0], north]);
    }
    let mut largest_gap = -1.0;
    let mut gap_index = 0;
    for index in 0..lons.len() {
        let next = if index + 1 < lons.len() {
            lons[index + 1]
        } else {
            lons[0] + 360.0
        };
        let gap = next - lons[index];
        if gap > largest_gap {
            largest_gap = gap;
            gap_index = index;
        }
    }
    Ok([
        lons[(gap_index + 1) % lons.len()],
        south,
        lons[gap_index],
        north,
    ])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SphericalPointRelation {
    Outside,
    Boundary,
    Inside,
}

type SphericalVector = [f64; 3];

fn spherical_dot(left: SphericalVector, right: SphericalVector) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn spherical_cross(left: SphericalVector, right: SphericalVector) -> SphericalVector {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn spherical_normalize(vector: SphericalVector) -> Option<SphericalVector> {
    let norm = spherical_dot(vector, vector).sqrt();
    (norm > 64.0 * f64::EPSILON).then(|| [vector[0] / norm, vector[1] / norm, vector[2] / norm])
}

fn lonlat_unit((lon, lat): (f64, f64)) -> SphericalVector {
    let lon = lon.to_radians();
    let lat = lat.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn spherical_angle(left: SphericalVector, right: SphericalVector) -> f64 {
    spherical_dot(left, right).clamp(-1.0, 1.0).acos()
}

fn point_on_minor_arc(
    point: SphericalVector,
    start: SphericalVector,
    end: SphericalVector,
) -> bool {
    (spherical_angle(start, point) + spherical_angle(point, end) - spherical_angle(start, end))
        .abs()
        <= 1.0e-9
}

fn open_ring_units(ring: &[(f64, f64)]) -> Vec<SphericalVector> {
    let mut units = ring.iter().copied().map(lonlat_unit).collect::<Vec<_>>();
    if units.len() > 1
        && spherical_dot(units[0], *units.last().expect("non-empty ring")) >= 1.0 - 1.0e-14
    {
        units.pop();
    }
    units
}

fn tangent_winding(point: SphericalVector, vertices: &[SphericalVector]) -> Option<f64> {
    let mut winding = 0.0;
    for index in 0..vertices.len() {
        let start = vertices[index];
        let end = vertices[(index + 1) % vertices.len()];
        let start_tangent = spherical_normalize([
            start[0] - point[0] * spherical_dot(point, start),
            start[1] - point[1] * spherical_dot(point, start),
            start[2] - point[2] * spherical_dot(point, start),
        ])?;
        let end_tangent = spherical_normalize([
            end[0] - point[0] * spherical_dot(point, end),
            end[1] - point[1] * spherical_dot(point, end),
            end[2] - point[2] * spherical_dot(point, end),
        ])?;
        winding += spherical_dot(point, spherical_cross(start_tangent, end_tangent))
            .atan2(spherical_dot(start_tangent, end_tangent));
    }
    Some(winding)
}

fn spherical_point_relation(point: (f64, f64), ring: &[(f64, f64)]) -> SphericalPointRelation {
    let vertices = open_ring_units(ring);
    if vertices.len() < 3 {
        return SphericalPointRelation::Outside;
    }
    let point = lonlat_unit(point);
    if (0..vertices.len()).any(|index| {
        point_on_minor_arc(
            point,
            vertices[index],
            vertices[(index + 1) % vertices.len()],
        )
    }) {
        return SphericalPointRelation::Boundary;
    }
    let classify = |probe| tangent_winding(probe, &vertices).map(|winding| winding.abs() > PI);
    let inside = classify(point).unwrap_or_else(|| {
        let seed = if point[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let tangent = spherical_normalize(spherical_cross(seed, point))
            .expect("axis seed is not parallel to point");
        let perturbed = spherical_normalize([
            point[0] + 1.0e-10 * tangent[0],
            point[1] + 1.0e-10 * tangent[1],
            point[2] + 1.0e-10 * tangent[2],
        ])
        .expect("perturbed unit point");
        classify(perturbed).unwrap_or(false)
    });
    if inside {
        SphericalPointRelation::Inside
    } else {
        SphericalPointRelation::Outside
    }
}

fn minor_arcs_intersect_or_touch(
    left_start: SphericalVector,
    left_end: SphericalVector,
    right_start: SphericalVector,
    right_end: SphericalVector,
) -> bool {
    let intersections = spherical_cross(
        spherical_cross(left_start, left_end),
        spherical_cross(right_start, right_end),
    );
    if let Some(intersection) = spherical_normalize(intersections) {
        for point in [
            intersection,
            [-intersection[0], -intersection[1], -intersection[2]],
        ] {
            if point_on_minor_arc(point, left_start, left_end)
                && point_on_minor_arc(point, right_start, right_end)
            {
                return true;
            }
        }
        false
    } else {
        [left_start, left_end]
            .into_iter()
            .any(|point| point_on_minor_arc(point, right_start, right_end))
            || [right_start, right_end]
                .into_iter()
                .any(|point| point_on_minor_arc(point, left_start, left_end))
    }
}

fn spherical_rings_intersect_or_touch(left: &[(f64, f64)], right: &[(f64, f64)]) -> bool {
    let left = open_ring_units(left);
    let right = open_ring_units(right);
    (0..left.len()).any(|left_index| {
        (0..right.len()).any(|right_index| {
            minor_arcs_intersect_or_touch(
                left[left_index],
                left[(left_index + 1) % left.len()],
                right[right_index],
                right[(right_index + 1) % right.len()],
            )
        })
    })
}

fn wrap_lon(lon: f64) -> f64 {
    let wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped == -180.0 && lon > 0.0 {
        180.0
    } else {
        wrapped
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cama_downstream_point, domain_from_circle, domain_from_directed_bbox,
        domain_from_outer_rings, expanded_merit_query_bboxes, filter_centered_geojson_to_domain,
        merge_corridor_geojson, project_hydro_domain, reset_project_hydro_output_dir,
        run_project_hydro_postprocess, write_cama_reach_corridor_geojson,
    };
    use crate::cama_binary_io::{
        CamaBinaryGridSpec, CamaBinaryWindow, CamaReachInventoryReport, CamaReachRecord,
    };
    use crate::hydro_delivery_refine_workflow::run_hydro_workflow;
    use crate::resolve_project_path;
    use earthmesh_project::{
        CloseBoundaryMode, CloseMaskFormat, DomainConfig, HydroCoastConfig, ProjectConfig,
        RegionShape,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "earthmesh-project-hydro-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn relative_hydro_inputs_follow_the_project_file() {
        assert_eq!(
            resolve_project_path(Path::new("/cases/gba/project.yaml"), "data/merit"),
            Path::new("/cases/gba/data/merit")
        );
        assert_eq!(
            resolve_project_path(Path::new("/cases/gba/project.yaml"), "/data/merit"),
            Path::new("/data/merit")
        );
    }

    #[test]
    fn owned_hydro_output_is_reset_between_runs() {
        let root = temp_root("reset-output");
        let output = root.join("hydro");
        fs::create_dir_all(&output).unwrap();
        fs::write(output.join("cama_reaches.geojson"), "stale").unwrap();
        fs::write(output.join("unrelated.txt"), "keep me").unwrap();

        reset_project_hydro_output_dir(&output).unwrap();

        assert!(output.is_dir());
        assert!(!output.join("cama_reaches.geojson").exists());
        assert_eq!(
            fs::read_to_string(output.join("unrelated.txt")).unwrap(),
            "keep me"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn configured_hydro_stage_reports_missing_external_gridfile() {
        let yaml = include_str!("../../../examples/projects/quickstart.yaml");
        let mut project = ProjectConfig::from_yaml(yaml).expect("quickstart project");
        project.domain = DomainConfig::Regional {
            shape: RegionShape::Bbox {
                w: 112.0,
                e: 115.0,
                s: 21.0,
                n: 24.0,
            },
            sea_ratio: None,
        };
        project.hydro_coast = Some(HydroCoastConfig {
            merit_root: "/missing/merit".to_string(),
            cama_root: None,
            merit_stride: 1,
            r3_width_m: 300.0,
            r2_width_m: 50.0,
            r3_upa_km2: 50_000.0,
            r2_upa_km2: 5_000.0,
            river_refinement_enabled: true,
            river_width_refinement_enabled: true,
            river_upstream_area_refinement_enabled: true,
            river_width_threshold_m: None,
            river_upstream_area_threshold_km2: None,
            coast_refinement_enabled: true,
            coast_buffer_km: 50.0,
            coast_land_refinement_enabled: true,
            coast_ocean_refinement_enabled: true,
        });
        let err = run_project_hydro_postprocess(
            &project,
            "/cases/gba/project.yaml",
            "/missing/gridfile.nc4",
            "/tmp/earthmesh-hydro-missing",
        )
        .expect_err("missing generated gridfile must not be silently skipped");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        assert!(err.to_string().contains("requires generated gridfile"));
    }

    #[test]
    fn antimeridian_bbox_splits_external_windows_but_keeps_directed_cell_filter() {
        let domain = domain_from_directed_bbox(170.0, -10.0, -170.0, 10.0).unwrap();
        assert_eq!(domain.bbox, [170.0, -10.0, -170.0, 10.0]);
        assert_eq!(
            domain.query_bboxes,
            vec![[170.0, -10.0, 180.0, 10.0], [-180.0, -10.0, -170.0, 10.0]]
        );
        assert_eq!(domain.components.len(), 1);
    }

    #[test]
    fn polar_and_full_longitude_bboxes_use_valid_spherical_components() {
        let validate = |domain: &super::ProjectHydroDomain| {
            for component in &domain.components {
                let ring = component
                    .shell
                    .iter()
                    .map(|&(lon, lat)| earthmesh_geometry::Point::new(lon, lat))
                    .collect::<Vec<_>>();
                earthmesh_geometry::try_spherical_polygon_excess(
                    &ring,
                    earthmesh_geometry::SphericalAreaBranch::Minor,
                )
                .expect("bbox component must be a valid minor-arc spherical polygon");
            }
        };

        let north = domain_from_directed_bbox(-45.0, 80.0, 45.0, 90.0).unwrap();
        validate(&north);
        assert!(north.contains((0.0, 90.0)));
        assert!(north.contains((0.0, 85.0)));
        assert!(!north.contains((90.0, 85.0)));

        let pole_to_pole = domain_from_directed_bbox(-20.0, -90.0, 20.0, 90.0).unwrap();
        validate(&pole_to_pole);
        assert!(pole_to_pole.contains((0.0, 0.0)));
        assert!(!pole_to_pole.contains((90.0, 0.0)));

        let mut full_longitude = domain_from_directed_bbox(-180.0, -80.0, 180.0, 80.0).unwrap();
        validate(&full_longitude);
        assert_eq!(full_longitude.components.len(), 4);
        let edge_count = full_longitude
            .components
            .iter()
            .map(|component| component.shell.len() - 1)
            .sum::<usize>();
        assert!(
            edge_count <= 7_208,
            "0.1-degree parallel sampling has a bounded full-world edge count: {edge_count}"
        );
        for longitude in [-179.0, -91.0, -1.0, 89.0, 179.0] {
            assert!(full_longitude.contains((longitude, 0.0)));
            assert!(full_longitude.contains((longitude, 80.0)));
            assert!(
                !full_longitude.contains((longitude, 80.001)),
                "constant-latitude bbox edge must not bow poleward at {longitude}"
            );
        }
        assert!(!full_longitude.contains((0.0, 85.0)));
        full_longitude.components.clear();
        assert!(
            full_longitude.contains((179.0, 0.0)),
            "literal bbox center filtering must use its O(1) predicate, not scan densified edges"
        );
        assert!(!full_longitude.contains((0.0, 85.0)));

        let reversed_full_longitude =
            domain_from_directed_bbox(180.0, -80.0, -180.0, 80.0).unwrap();
        assert_eq!(
            reversed_full_longitude.query_bboxes,
            vec![[-180.0, -80.0, 180.0, 80.0]],
            "the alternate full-longitude spelling must not become zero-width query windows"
        );
        assert_eq!(
            expanded_merit_query_bboxes([180.0, -80.0, -180.0, 80.0], 0.0),
            vec![[-180.0, -80.00083333333333, 180.0, 80.00083333333333]],
            "MERIT query expansion must preserve a reversed full-longitude bbox"
        );

        let seam = domain_from_directed_bbox(170.0, 70.0, -170.0, 80.0).unwrap();
        assert!(seam.contains((179.0, 79.999)));
        assert!(seam.contains((-179.0, 80.0)));
        assert!(!seam.contains((179.0, 80.001)));
        assert!(!seam.contains((0.0, 75.0)));
    }

    #[test]
    fn merit_query_uses_native_or_physical_coast_halo_without_expanding_domain_clip() {
        let halo: f64 = 3.0 / 3_600.0;
        let windows = expanded_merit_query_bboxes([100.0, 10.0, 101.0, 11.0], 0.0);
        assert_eq!(windows.len(), 1);
        let lon_halo = halo / (11.0 + halo).to_radians().cos();
        assert!((windows[0][0] - (100.0 - lon_halo)).abs() < 1.0e-12);
        assert!((windows[0][1] - (10.0 - halo)).abs() < 1.0e-12);
        assert!((windows[0][2] - (101.0 + lon_halo)).abs() < 1.0e-12);
        assert!((windows[0][3] - (11.0 + halo)).abs() < 1.0e-12);

        let seam = expanded_merit_query_bboxes([179.9995, 89.9995, 180.0, 90.0], 0.0);
        assert_eq!(seam.len(), 1);
        assert_eq!(seam[0][0], -180.0);
        assert!((seam[0][1] - (89.9995 - halo)).abs() < 1.0e-12);
        assert_eq!(seam[0][2], 180.0);
        assert_eq!(seam[0][3], 90.0);

        let antimeridian = expanded_merit_query_bboxes([170.0, -10.0, -170.0, 10.0], 0.0);
        assert_eq!(antimeridian.len(), 2);
        let antimeridian_lon_halo = halo / (10.0 + halo).to_radians().cos();
        assert!((antimeridian[0][0] - (170.0 - antimeridian_lon_halo)).abs() < 1.0e-9);
        assert!((antimeridian[1][2] - (-170.0 + antimeridian_lon_halo)).abs() < 1.0e-9);

        let physical = expanded_merit_query_bboxes([100.0, 60.0, 101.0, 61.0], 50.0);
        let lat_halo = 50.0 / earthmesh_core::KM_PER_DEGREE_EQUATOR;
        assert!((physical[0][1] - (60.0 - lat_halo)).abs() < 1.0e-9);
        assert!(100.0 - physical[0][0] > lat_halo);
    }

    #[test]
    fn circle_crossing_antimeridian_produces_two_external_windows() {
        let domain = domain_from_circle(179.0, 0.0, 500.0).unwrap();
        assert!(domain.bbox[0] > domain.bbox[2]);
        assert_eq!(domain.query_bboxes.len(), 2);
        assert_eq!(domain.components.len(), 1);
    }

    #[test]
    fn self_touching_ring_is_rejected_instead_of_guessed() {
        let bridged = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
            (8.0, 8.0),
            (2.0, 8.0),
            (2.0, 2.0),
            (8.0, 2.0),
            (8.0, 8.0),
            (10.0, 10.0),
            (0.0, 10.0),
        ];
        let err = domain_from_outer_rings(vec![bridged], "shapefile").unwrap_err();
        assert!(err.to_string().contains("bridged hole"));
    }

    #[test]
    fn nested_rings_preserve_holes_islands_and_multipart_union() {
        let shell = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let hole = vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)];
        let island = vec![(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)];
        let disjoint = vec![(20.0, 0.0), (22.0, 0.0), (22.0, 2.0), (20.0, 2.0)];
        let domain = domain_from_outer_rings(vec![hole, disjoint, island, shell], "test").unwrap();

        assert_eq!(domain.components.len(), 3);
        assert_eq!(
            domain
                .components
                .iter()
                .map(|component| component.holes.len())
                .sum::<usize>(),
            1
        );
        assert!(domain.contains((1.0, 1.0)));
        assert!(!domain.contains((3.0, 3.0)));
        assert!(domain.contains((5.0, 5.0)));
        assert!(domain.contains((21.0, 1.0)));
        assert_eq!(domain.bbox, [0.0, 0.0, 22.0, 10.0]);
    }

    #[test]
    fn crossing_or_touching_rings_inside_one_record_are_rejected() {
        let crossing = domain_from_outer_rings(
            vec![
                vec![(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)],
                vec![(4.0, -1.0), (8.0, -1.0), (8.0, 3.0), (4.0, 3.0)],
            ],
            "crossing",
        )
        .unwrap_err();
        assert_eq!(crossing.kind(), std::io::ErrorKind::InvalidData);
        assert!(crossing.to_string().contains("intersect or touch"));

        let touching = domain_from_outer_rings(
            vec![
                vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
                vec![(4.0, 1.0), (6.0, 1.0), (6.0, 3.0), (4.0, 3.0)],
            ],
            "touching",
        )
        .unwrap_err();
        assert_eq!(touching.kind(), std::io::ErrorKind::InvalidData);
        assert!(touching.to_string().contains("intersect or touch"));
    }

    #[test]
    fn polar_circles_contain_their_centers_and_query_every_longitude() {
        for latitude in [90.0, 89.0, -89.0, -90.0] {
            let domain = domain_from_circle(17.0, latitude, 500.0).unwrap();
            assert!(
                domain.contains((17.0, latitude)),
                "polar circle at {latitude} excluded its center"
            );
            assert_eq!(domain.bbox[0], -180.0);
            assert_eq!(domain.bbox[2], 180.0);
            assert_eq!(domain.query_bboxes.len(), 1);
            if latitude > 0.0 {
                assert_eq!(domain.bbox[3], 90.0);
            } else {
                assert_eq!(domain.bbox[1], -90.0);
            }
        }
    }

    #[test]
    fn every_hole_boundary_edge_and_vertex_is_excluded() {
        let midpoint = |left: (f64, f64), right: (f64, f64)| {
            let left = super::lonlat_unit(left);
            let right = super::lonlat_unit(right);
            let point = super::spherical_normalize([
                left[0] + right[0],
                left[1] + right[1],
                left[2] + right[2],
            ])
            .unwrap();
            (
                point[1].atan2(point[0]).to_degrees(),
                point[2].asin().to_degrees(),
            )
        };
        let domain = domain_from_outer_rings(
            vec![
                vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)],
            ],
            "hole-boundary",
        )
        .unwrap();
        for point in [
            midpoint((2.0, 2.0), (2.0, 8.0)),
            midpoint((2.0, 8.0), (8.0, 8.0)),
            midpoint((8.0, 8.0), (8.0, 2.0)),
            midpoint((8.0, 2.0), (2.0, 2.0)),
            (2.0, 2.0),
            (8.0, 8.0),
        ] {
            assert!(!domain.contains(point), "hole boundary retained {point:?}");
        }
    }

    #[test]
    fn refinement_only_cells_inside_holes_are_removed() {
        let root = temp_root("domain-cell-filter");
        let path = root.join("refinement.geojson");
        fs::write(
            &path,
            r#"{"type":"FeatureCollection","features":[
              {"type":"Feature","properties":{"cell_id":"kept","center_lon":1,"center_lat":1},"geometry":null},
              {"type":"Feature","properties":{"cell_id":"hole","center_lon":5,"center_lat":5},"geometry":null}
            ]}"#,
        )
        .unwrap();
        let domain = domain_from_outer_rings(
            vec![
                vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)],
            ],
            "test",
        )
        .unwrap();
        assert_eq!(
            filter_centered_geojson_to_domain(&path, &domain).unwrap(),
            1
        );
        let filtered = fs::read_to_string(path).unwrap();
        assert!(filtered.contains("\"kept\""));
        assert!(!filtered.contains("\"hole\""));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn antimeridian_shell_and_hole_keep_directed_queries_and_containment() {
        let shell = vec![
            (170.0, -10.0),
            (-170.0, -10.0),
            (-170.0, 10.0),
            (170.0, 10.0),
        ];
        let hole = vec![(175.0, -5.0), (-175.0, -5.0), (-175.0, 5.0), (175.0, 5.0)];
        let domain = domain_from_outer_rings(vec![hole, shell], "seam").unwrap();

        assert_eq!(domain.components.len(), 1);
        assert_eq!(domain.components[0].holes.len(), 1);
        assert_eq!(domain.bbox, [170.0, -10.0, -170.0, 10.0]);
        assert_eq!(domain.query_bboxes.len(), 2);
        assert!(domain.contains((172.0, 0.0)));
        assert!(!domain.contains((179.0, 0.0)));
        assert!(!domain.contains((-179.0, 0.0)));
        assert!(domain.contains((-172.0, 0.0)));
    }

    #[test]
    fn shapefile_parts_reach_project_hydro_as_shell_and_hole() {
        let root = temp_root("shapefile-hole");
        let shape = root.join("domain.shp");
        write_test_polygon_shp_parts(
            &shape,
            &[
                vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)],
                vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)],
            ],
        );

        let domain = project_hydro_domain(
            &root.join("project.yaml"),
            &RegionShape::Shapefile {
                path: "domain.shp".to_string(),
            },
        )
        .unwrap();
        assert_eq!(domain.components.len(), 1);
        assert_eq!(domain.components[0].holes.len(), 1);
        assert!(domain.contains((1.0, 1.0)));
        assert!(!domain.contains((5.0, 5.0)));

        let close_domain = project_hydro_domain(
            &root.join("project.yaml"),
            &RegionShape::Close {
                path: "domain.shp".to_string(),
                format: CloseMaskFormat::PolygonShp,
                boundary: CloseBoundaryMode::Polyline,
            },
        )
        .unwrap();
        assert_eq!(close_domain.components, domain.components);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn nested_shapefile_records_remain_a_union_not_a_hole() {
        let root = temp_root("shapefile-record-union");
        let shape = root.join("domain.shp");
        write_test_polygon_shp_records(
            &shape,
            &[
                vec![vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]],
                vec![vec![(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)]],
            ],
        );
        let domain = project_hydro_domain(
            &root.join("project.yaml"),
            &RegionShape::Shapefile {
                path: "domain.shp".to_string(),
            },
        )
        .unwrap();
        assert!(domain.contains((1.0, 1.0)));
        assert!(
            domain.contains((5.0, 5.0)),
            "a nested polygon in a separate SHP record is union, not a hole"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disjoint_multi_part_domain_is_preserved() {
        let domain = domain_from_outer_rings(
            vec![
                vec![(100.0, 10.0), (101.0, 10.0), (101.0, 11.0)],
                vec![(110.0, 20.0), (111.0, 20.0), (111.0, 21.0)],
            ],
            "shapefile",
        )
        .unwrap();
        assert_eq!(domain.components.len(), 2);
        assert_eq!(domain.bbox, [100.0, 10.0, 111.0, 21.0]);
    }

    fn write_test_polygon_shp_parts(path: &Path, rings: &[Vec<(f64, f64)>]) {
        write_test_polygon_shp_records(path, &[rings.to_vec()]);
    }

    fn write_test_polygon_shp_records(path: &Path, records: &[Vec<Vec<(f64, f64)>>]) {
        let records = records
            .iter()
            .map(|rings| {
                rings
                    .iter()
                    .map(|ring| {
                        let mut ring = ring.clone();
                        ring.push(ring[0]);
                        ring
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let points = records
            .iter()
            .flatten()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let content_bytes = records
            .iter()
            .map(|rings| 44 + rings.len() * 4 + rings.iter().map(Vec::len).sum::<usize>() * 16)
            .collect::<Vec<_>>();
        let file_bytes = 100 + content_bytes.iter().map(|bytes| 8 + bytes).sum::<usize>();
        let min_x = points
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
        let min_y = points
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min);
        let max_x = points
            .iter()
            .map(|point| point.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let max_y = points
            .iter()
            .map(|point| point.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let mut bytes = Vec::with_capacity(file_bytes);
        bytes.extend(9994_i32.to_be_bytes());
        bytes.extend([0_u8; 20]);
        bytes.extend(((file_bytes / 2) as i32).to_be_bytes());
        bytes.extend(1000_i32.to_le_bytes());
        bytes.extend(5_i32.to_le_bytes());
        for value in [min_x, min_y, max_x, max_y, 0.0, 0.0, 0.0, 0.0] {
            bytes.extend(value.to_le_bytes());
        }
        for (record_index, (rings, content_bytes)) in records.iter().zip(content_bytes).enumerate()
        {
            let record_points = rings.iter().flatten().copied().collect::<Vec<_>>();
            let record_min_x = record_points
                .iter()
                .map(|point| point.0)
                .fold(f64::INFINITY, f64::min);
            let record_min_y = record_points
                .iter()
                .map(|point| point.1)
                .fold(f64::INFINITY, f64::min);
            let record_max_x = record_points
                .iter()
                .map(|point| point.0)
                .fold(f64::NEG_INFINITY, f64::max);
            let record_max_y = record_points
                .iter()
                .map(|point| point.1)
                .fold(f64::NEG_INFINITY, f64::max);
            bytes.extend(((record_index + 1) as i32).to_be_bytes());
            bytes.extend(((content_bytes / 2) as i32).to_be_bytes());
            bytes.extend(5_i32.to_le_bytes());
            for value in [record_min_x, record_min_y, record_max_x, record_max_y] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend((rings.len() as i32).to_le_bytes());
            bytes.extend((record_points.len() as i32).to_le_bytes());
            let mut start = 0usize;
            for ring in rings {
                bytes.extend((start as i32).to_le_bytes());
                start += ring.len();
            }
            for (lon, lat) in record_points {
                bytes.extend(lon.to_le_bytes());
                bytes.extend(lat.to_le_bytes());
            }
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn out_of_grid_cama_downstream_is_an_error_not_a_terminal_cap() {
        let record = CamaReachRecord {
            reach_id: "invalid-downstream".to_string(),
            x_index: 0,
            y_index: 0,
            lon: 0.0,
            lat: 0.0,
            upstream_area_km2: 12_000.0,
            width_m: 80.0,
            floodplain_width_m: 0.0,
            target_dx_km: 2.5,
            is_estuary: false,
            river_length_m: 1_100.0,
            downstream_x: 2,
            downstream_y: 0,
        };
        let inventory = CamaReachInventoryReport {
            grid: CamaBinaryGridSpec {
                nx: 2,
                ny: 1,
                west: -0.005,
                south: -0.005,
                grid_size_deg: 0.01,
                little_endian: true,
                y_reversed_storage: false,
            },
            window: CamaBinaryWindow {
                x_start: 0,
                y_start: 0,
                width: 1,
                height: 1,
            },
            records: vec![record.clone()],
            valid_channel_cells: 1,
            skipped_cells: 0,
        };

        let error = cama_downstream_point(&inventory, &record).unwrap_err();
        assert!(
            error.to_string().contains("outside the 2x1 grid"),
            "{error}"
        );
    }

    #[test]
    fn cama_corridor_is_merged_into_the_exact_workflow_input() {
        let root = temp_root("cama-corridor-merge");
        let inventory = CamaReachInventoryReport {
            grid: CamaBinaryGridSpec {
                nx: 2,
                ny: 1,
                west: -0.005,
                south: -0.005,
                grid_size_deg: 0.01,
                little_endian: true,
                y_reversed_storage: false,
            },
            window: CamaBinaryWindow {
                x_start: 0,
                y_start: 0,
                width: 2,
                height: 1,
            },
            records: vec![CamaReachRecord {
                reach_id: "synthetic-cama-reach".to_string(),
                x_index: 0,
                y_index: 0,
                lon: 0.0,
                lat: 0.0,
                upstream_area_km2: 12_000.0,
                width_m: 80.0,
                floodplain_width_m: 0.0,
                target_dx_km: 2.5,
                is_estuary: false,
                river_length_m: 1_100.0,
                downstream_x: 1,
                downstream_y: 0,
            }],
            valid_channel_cells: 1,
            skipped_cells: 0,
        };
        let cama = root.join("cama.geojson");
        assert_eq!(
            write_cama_reach_corridor_geojson(&inventory, &cama, 100.0, 300.0).unwrap(),
            1
        );
        let merit = root.join("merit.geojson");
        fs::write(
            &merit,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[10,10],[10.1,10],[10.1,10.1],[10,10.1],[10,10]]]},"properties":{"mask_class":"R2","source":"MERIT-Hydro"}}]}"#,
        )
        .unwrap();
        let combined = root.join("combined.geojson");
        assert_eq!(merge_corridor_geojson(&merit, &cama, &combined).unwrap(), 2);
        let combined_text = fs::read_to_string(&combined).unwrap();
        assert!(combined_text.contains("synthetic-cama-reach"));
        assert!(combined_text.contains("CaMa-Flood"));
        assert!(combined_text.contains("MERIT-Hydro"));

        let cells = root.join("cells.geojson");
        fs::write(
            &cells,
            r#"{"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Polygon","coordinates":[[[-0.02,-0.02],[0.02,-0.02],[0.02,0.02],[-0.02,0.02],[-0.02,-0.02]]]},"properties":{"cell_id":"cama-cell"}}]}"#,
        )
        .unwrap();
        let report = run_hydro_workflow(
            &cells,
            &combined,
            root.join("workflow"),
            &["R2".to_string()],
            0.0,
            false,
            None,
            1,
            None,
            None,
            None,
            1,
        )
        .unwrap();
        assert_eq!(report.intersection_cells, 1);
        assert_eq!(report.coupling_rows, 1);
        assert!(fs::read_to_string(report.intersections_path)
            .unwrap()
            .contains("cama-cell"));

        let _ = fs::remove_dir_all(root);
    }
}
