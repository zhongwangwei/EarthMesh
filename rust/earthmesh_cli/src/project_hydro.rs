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
    read_close_mask_nml_points, read_lonlat_text_points, read_shapefile_polygon_rings,
    transform_close_boundary, CloseBoundaryGeometry, CloseMaskFormat, GeometryPoint, MeshCellKind,
    ProjectConfig, ProjectLayerRole, RegionShape,
};

use crate::cama_binary_io::{
    CamaLonLatBbox, CamaReachClassificationThresholds, CamaReachInventoryReport, CamaReachRecord,
};
use crate::cama_reach_inventory::{
    classify_cama_reach_record, read_cama_reach_inventory_from_map_dir,
    write_cama_reach_inventory_point_geojson,
};
use crate::hydro_delivery_cells::write_gridfile_cell_polygons_geojson_with_report;
use crate::hydro_delivery_refine_workflow::run_hydro_workflow;
use crate::hydro_workflow_types::HydroWorkflowReport;
use crate::merit_hydro_io::{
    read_merit_hydro_window, write_merit_hydro_mask_geojson_layers, MeritMaskThresholds,
};
use crate::merit_tile_selection::{select_merit_hydro_tiles, MeritLonLatBbox};
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
    rings: Vec<Vec<(f64, f64)>>,
    /// Directed west/south/east/north bbox used to filter mesh-cell centers.
    bbox: [f64; 4],
    /// Non-wrapping windows used by rectangular MERIT/CaMa readers.
    query_bboxes: Vec<[f64; 4]>,
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
        .map(|layer| resolve_project_path(project_path, layer.path.trim()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "Project hydro production coupling requires an enabled landtype data layer",
            )
        })?;
    if !landtype.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Project hydro production coupling requires landtype file {}",
                landtype.display()
            ),
        ));
    }
    let landtype_gridnum_perdegree =
        crate::mkgrd_gridinit_driver::landtype_gridnum_perdegree(&landtype)?;
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

    let mut windows = Vec::new();
    // MERIT-Hydro is a 3 arc-second raster. Read one native cell beyond the
    // requested footprint so coast/river classification at the exact domain
    // edge can still inspect its immediate neighbor; workflow clipping remains
    // tied to `domain.rings` and therefore does not enlarge the output domain.
    for query in expanded_merit_query_bboxes(domain.bbox) {
        let bbox = merit_bbox(query);
        let tiles = select_merit_hydro_tiles(&merit_root, bbox)?;
        for tile in tiles {
            windows.push(read_merit_hydro_window(tile, bbox, plan.merit_stride)?);
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
    let merit_dir = out_dir.join("merit_masks");
    let thresholds = MeritMaskThresholds {
        r2_width_m: plan.r2_width_m,
        r3_width_m: plan.r3_width_m,
        ..MeritMaskThresholds::default()
    };
    let merit = write_merit_hydro_mask_geojson_layers(&windows, thresholds, &merit_dir, false)?;

    let (
        cama_reaches_geojson,
        cama_river_mouths_geojson,
        cama_corridors_geojson,
        cama_reach_count,
        cama_river_mouth_count,
        cama_corridor_count,
    ) = if let Some(cama_root) = &cama_root {
        let inventory =
            read_project_cama_inventory(cama_root, &domain.query_bboxes, plan.target_dx_km)?;
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
    let hydro = run_hydro_workflow(
        &cells_geojson,
        &corridors_geojson,
        &workflow_dir,
        &plan.include_classes,
        0.0,
        false,
        Some(&domain.rings),
        plan.max_level,
        None,
        Some(gridfile),
        Some(&landtype),
        landtype_gridnum_perdegree,
    )?;
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
            "{{\n  \"kind\": \"earthmesh_project_hydro\",\n  \"domain_parts\": {},\n  \"cell_count\": {},\n  \"rejected_unsupported_cell_count\": {},\n  \"merit_window_count\": {},\n  \"merit_stride\": {},\n  \"cama_reach_count\": {},\n  \"cama_river_mouth_count\": {},\n  \"cama_corridor_count\": {},\n  \"artifacts\": {{\n    \"cells_geojson\": \"{}\",\n    \"corridors_geojson\": \"{}\",\n    \"merit_corridors_geojson\": \"{}\",\n    \"cama_reaches_geojson\": {},\n    \"cama_river_mouths_geojson\": {},\n    \"cama_corridors_geojson\": {},\n    \"hydro_workflow_manifest\": \"{}\"\n  }}\n}}\n",
            domain.rings.len(),
            cell_export.emitted_cells,
            cell_export.rejected_unsupported_cells,
            windows.len(),
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

fn expanded_merit_query_bboxes([west, south, east, north]: [f64; 4]) -> Vec<[f64; 4]> {
    const MERIT_NATIVE_CELL_DEG: f64 = 3.0 / 3_600.0;
    let span = if west <= east {
        east - west
    } else {
        360.0 - (west - east)
    };
    let south = (south - MERIT_NATIVE_CELL_DEG).max(-90.0);
    let north = (north + MERIT_NATIVE_CELL_DEG).min(90.0);
    if span + 2.0 * MERIT_NATIVE_CELL_DEG >= 360.0 {
        return vec![[-180.0, south, 180.0, north]];
    }
    split_directed_bbox([
        wrap_lon(west - MERIT_NATIVE_CELL_DEG),
        south,
        wrap_lon(east + MERIT_NATIVE_CELL_DEG),
        north,
    ])
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
        let downstream = cama_downstream_point(inventory, record);
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
) -> Option<(f64, f64)> {
    let x = usize::try_from(record.downstream_x).ok()?;
    let y = usize::try_from(record.downstream_y).ok()?;
    if x >= inventory.grid.nx
        || y >= inventory.grid.ny
        || (x, y) == (record.x_index, record.y_index)
    {
        return None;
    }
    Some((
        wrap_lon(inventory.grid.lon_center(x)),
        inventory.grid.lat_center(y),
    ))
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
    const EARTH_RADIUS_M: f64 = 6_371_008.8;
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
            let rings = read_shapefile_polygon_rings(&path)?;
            domain_from_outer_rings(rings, "shapefile")
        }
        RegionShape::Close {
            path,
            format,
            boundary,
        } => {
            let path = resolve_project_path(project_path, path);
            let rings = match format {
                CloseMaskFormat::PolygonShp => read_shapefile_polygon_rings(&path)?,
                CloseMaskFormat::LonLatText => vec![read_lonlat_text_points(&path)?],
                CloseMaskFormat::Nml => vec![read_close_mask_nml_points(&path)?],
                CloseMaskFormat::Netcdf => vec![crate::read_close_mask_netcdf(&path)?
                    .points
                    .into_iter()
                    .map(|point| (point.lon, point.lat))
                    .collect()],
            };
            if rings.len() != 1
                && !matches!(boundary, earthmesh_project::CloseBoundaryMode::Polyline)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "transformed multi-part close domains are not supported",
                ));
            }
            let mut transformed_rings = Vec::new();
            for ring in rings {
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
            domain_from_outer_rings(transformed_rings, "close domain")
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
    let rings = query_bboxes.iter().map(|bbox| bbox_ring(*bbox)).collect();
    Ok(ProjectHydroDomain {
        rings,
        bbox: [west, south, east, north],
        query_bboxes,
    })
}

fn domain_from_circle(lon: f64, lat: f64, radius_km: f64) -> io::Result<ProjectHydroDomain> {
    const EARTH_RADIUS_KM: f64 = 6_371.008_8;
    let angular = radius_km / EARTH_RADIUS_KM;
    let center_lat = lat.to_radians();
    let center_lon = lon.to_radians();
    let mut ring = Vec::with_capacity(181);
    for index in 0..180 {
        let bearing = 2.0 * PI * index as f64 / 180.0;
        let point_lat = (center_lat.sin() * angular.cos()
            + center_lat.cos() * angular.sin() * bearing.cos())
        .asin();
        let point_lon = center_lon
            + (bearing.sin() * angular.sin() * center_lat.cos())
                .atan2(angular.cos() - center_lat.sin() * point_lat.sin());
        ring.push((wrap_lon(point_lon.to_degrees()), point_lat.to_degrees()));
    }
    ring.push(ring[0]);
    domain_from_outer_rings(vec![ring], "circle")
}

fn domain_from_outer_rings(
    mut rings: Vec<Vec<(f64, f64)>>,
    label: &str,
) -> io::Result<ProjectHydroDomain> {
    rings.retain(|ring| ring.len() >= 3);
    if rings.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains no polygon rings"),
        ));
    }
    if rings
        .iter()
        .any(|ring| has_nonadjacent_duplicate_vertex(ring))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{label} contains a bridged hole or self-touching polygon; Project hydro refuses to silently fill or flatten it"
            ),
        ));
    }
    if has_nested_ring(&rings) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{label} contains polygon holes; the current Project hydro domain interface cannot represent holes without filling them"
            ),
        ));
    }
    for ring in &mut rings {
        if ring.first() != ring.last() {
            ring.push(ring[0]);
        }
    }
    let bbox = minimal_directed_bbox(&rings)?;
    Ok(ProjectHydroDomain {
        query_bboxes: split_directed_bbox(bbox),
        bbox,
        rings,
    })
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
    vec![
        (west, south),
        (east, south),
        (east, north),
        (west, north),
        (west, south),
    ]
}

fn split_directed_bbox([west, south, east, north]: [f64; 4]) -> Vec<[f64; 4]> {
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
    let south = rings
        .iter()
        .flatten()
        .map(|point| point.1)
        .fold(f64::INFINITY, f64::min);
    let north = rings
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

fn has_nested_ring(rings: &[Vec<(f64, f64)>]) -> bool {
    rings.iter().enumerate().any(|(index, ring)| {
        let Some(&sample) = ring.first() else {
            return false;
        };
        rings
            .iter()
            .enumerate()
            .any(|(other_index, other)| index != other_index && point_in_ring(sample, other))
    })
}

fn point_in_ring(point: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let px = point.0;
    for index in 0..ring.len() {
        let (mut ax, ay) = ring[index];
        let (mut bx, by) = ring[(index + 1) % ring.len()];
        while ax - px > 180.0 {
            ax -= 360.0;
        }
        while ax - px < -180.0 {
            ax += 360.0;
        }
        while bx - px > 180.0 {
            bx -= 360.0;
        }
        while bx - px < -180.0 {
            bx += 360.0;
        }
        if (ay > point.1) != (by > point.1) && px < (bx - ax) * (point.1 - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    inside
}

fn wrap_lon(lon: f64) -> f64 {
    let wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped == -180.0 && lon > 0.0 {
        180.0
    } else {
        wrapped
    }
}

fn resolve_project_path(project_path: &Path, configured: &str) -> PathBuf {
    let path = PathBuf::from(configured);
    if path.is_absolute() {
        path
    } else {
        project_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        domain_from_circle, domain_from_directed_bbox, domain_from_outer_rings,
        expanded_merit_query_bboxes, merge_corridor_geojson, reset_project_hydro_output_dir,
        resolve_project_path, run_hydro_workflow, run_project_hydro_postprocess,
        write_cama_reach_corridor_geojson,
    };
    use crate::cama_binary_io::{
        CamaBinaryGridSpec, CamaBinaryWindow, CamaReachInventoryReport, CamaReachRecord,
    };
    use earthmesh_project::{DomainConfig, HydroCoastConfig, ProjectConfig, RegionShape};
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
        assert_eq!(domain.rings.len(), 2);
    }

    #[test]
    fn merit_query_adds_one_native_cell_halo_without_expanding_domain_clip() {
        let halo = 3.0 / 3_600.0;
        let windows = expanded_merit_query_bboxes([100.0, 10.0, 101.0, 11.0]);
        assert_eq!(windows.len(), 1);
        assert!((windows[0][0] - (100.0 - halo)).abs() < 1.0e-12);
        assert!((windows[0][1] - (10.0 - halo)).abs() < 1.0e-12);
        assert!((windows[0][2] - (101.0 + halo)).abs() < 1.0e-12);
        assert!((windows[0][3] - (11.0 + halo)).abs() < 1.0e-12);

        let seam = expanded_merit_query_bboxes([179.9995, 89.9995, 180.0, 90.0]);
        assert_eq!(seam.len(), 2);
        assert_eq!(seam[0][2], 180.0);
        assert_eq!(seam[1][0], -180.0);
        assert_eq!(seam[0][3], 90.0);
        assert_eq!(seam[1][3], 90.0);

        let antimeridian = expanded_merit_query_bboxes([170.0, -10.0, -170.0, 10.0]);
        assert_eq!(antimeridian.len(), 2);
        assert!((antimeridian[0][0] - (170.0 - halo)).abs() < 1.0e-12);
        assert!((antimeridian[1][2] - (-170.0 + halo)).abs() < 1.0e-12);
    }

    #[test]
    fn circle_crossing_antimeridian_produces_two_external_windows() {
        let domain = domain_from_circle(179.0, 0.0, 500.0).unwrap();
        assert!(domain.bbox[0] > domain.bbox[2]);
        assert_eq!(domain.query_bboxes.len(), 2);
        assert_eq!(domain.rings.len(), 1);
    }

    #[test]
    fn bridged_shapefile_hole_is_rejected_instead_of_silently_filled() {
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
    fn disjoint_multi_part_domain_is_preserved() {
        let domain = domain_from_outer_rings(
            vec![
                vec![(100.0, 10.0), (101.0, 10.0), (101.0, 11.0)],
                vec![(110.0, 20.0), (111.0, 20.0), (111.0, 21.0)],
            ],
            "shapefile",
        )
        .unwrap();
        assert_eq!(domain.rings.len(), 2);
        assert_eq!(domain.bbox, [100.0, 10.0, 111.0, 21.0]);
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
