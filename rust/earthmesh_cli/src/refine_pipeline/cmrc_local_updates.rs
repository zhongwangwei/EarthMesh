use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use earthmesh_geometry::Point;
use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, CartesianPoint, LonLatDegrees, MeshState};
use earthmesh_quality::{compute, QualityCell, QualityMeshInput, QualityThresholds};
use earthmesh_refine_certified::{AngleContractId, Certificate, GeometryCertificateReport};
use serde::Deserialize;
use serde_json::{json, Value};

const UNIT_TOL: f64 = 1.0e-10;
const BASELINE_TOL: f64 = 1.0e-12;
const QUALITY_TOL: f64 = 1.0e-9;
const SIGNIFICANT: f64 = 1.0e-8;
const MAX_UPDATES: usize = 10;
const MAX_MOVE_FRACTION: f64 = 0.03;
const PREFERRED_MIN_DEG: f64 = 40.0;
const PREFERRED_MAX_DEG: f64 = 80.0;

#[derive(Deserialize)]
struct Request {
    source_mpas: String,
    source_gridfile: String,
    target_cell: usize,
    updates: Vec<Update>,
    #[serde(default)]
    flips: Vec<Value>,
    #[serde(default)]
    case: Option<String>,
}

#[derive(Deserialize)]
struct Update {
    id: usize,
    xyz: [f64; 3],
}

struct SourceMesh {
    physical_vertices: Vec<CartesianPoint>,
    physical_triangles: Vec<[usize; 3]>,
}

#[derive(Clone, Copy)]
struct CellMetrics {
    edge_cv: f64,
    aspect: f64,
}

#[derive(Clone, Copy)]
struct GlobalMetrics {
    cells: usize,
    triangles: usize,
    edge_cv_max: f64,
    aspect_max: f64,
    adjacent_ratio: f64,
    min_angle: f64,
    max_angle: f64,
    preferred_bad_angle_count: Option<usize>,
    preferred_bad_triangle_count: Option<usize>,
    angle_rmse_to_60: Option<f64>,
}

pub(super) fn apply(
    mesh: &mut MeshState,
    delivered_levels: &[usize],
    anchors: &[usize; 12],
    request_path: &Path,
    contract: AngleContractId,
) -> io::Result<Value> {
    let request_text = std::fs::read_to_string(request_path).map_err(|error| {
        invalid_input(format!(
            "read local update request {}: {error}",
            request_path.display()
        ))
    })?;
    let request: Request = serde_json::from_str(&request_text).map_err(|error| {
        invalid_input(format!(
            "malformed local update request {}: {error}",
            request_path.display()
        ))
    })?;
    require_absolute("source_mpas", &request.source_mpas)?;
    require_absolute("source_gridfile", &request.source_gridfile)?;
    if !request.flips.is_empty() {
        return Err(invalid_input(format!(
            "local CMRC update delivery accepts no flips; got {}",
            request.flips.len()
        )));
    }
    if request.updates.is_empty() || request.updates.len() > MAX_UPDATES {
        return Err(invalid_input(format!(
            "updates must contain 1..={MAX_UPDATES} entries, got {}",
            request.updates.len()
        )));
    }

    let active_vertices = mesh.active_vertex_slots().collect::<Vec<_>>();
    if delivered_levels.len() != active_vertices.len() {
        return Err(invalid_input(format!(
            "delivered_levels length {} must equal active cell count {}",
            delivered_levels.len(),
            active_vertices.len()
        )));
    }
    let id_for_physical = |physical: usize| -> io::Result<usize> {
        active_vertices
            .get(physical)
            .copied()
            .ok_or_else(|| invalid_input(format!("physical cell id {physical} is out of range")))
    };
    let target = id_for_physical(request.target_cell)?;
    let level_by_slot = level_by_slot(mesh, delivered_levels, &active_vertices);

    let source = read_mpas(&request.source_mpas)?;
    verify_baseline(mesh, &source)?;
    let gridfile_levels = read_tail_levels(&request.source_gridfile, active_vertices.len())?;
    if gridfile_levels != delivered_levels {
        return Err(invalid_input(
            "source_gridfile delivered levels differ from current mesh",
        ));
    }

    validate_anchors(mesh, anchors)?;
    let updates = validate_updates(&request.updates, mesh, anchors, &id_for_physical)?;
    reject_mixed_face_moves(mesh, &updates, &level_by_slot)?;
    let before_target = target_cell_metrics(mesh, target)
        .map_err(|error| invalid_input(format!("baseline target measurement failed: {error}")))?;
    check_move_lengths(mesh, &updates)?;
    Certificate::internal_for(contract)
        .verify_geometry(mesh)
        .map_err(|error| invalid_input(format!("baseline internal certificate failed: {error}")))?;
    Certificate::final_delivery_for(contract)
        .verify_geometry(mesh)
        .map_err(|error| invalid_input(format!("baseline final certificate failed: {error}")))?;
    let before_tri = triangular_quality(mesh)
        .map_err(|error| invalid_input(format!("baseline triangular quality failed: {error}")))?;
    let before_dual = dual_quality(mesh)
        .map_err(|error| invalid_input(format!("baseline dual quality failed: {error}")))?;

    let mut candidate = mesh.clone();
    for (&id, &point) in &updates {
        candidate.move_vertex(id, point);
    }
    Certificate::internal_for(contract)
        .verify_geometry(&candidate)
        .map_err(|error| invalid_data(format!("candidate internal certificate failed: {error}")))?;
    let candidate_final = Certificate::final_delivery_for(contract)
        .verify_geometry(&candidate)
        .map_err(|error| invalid_data(format!("candidate final certificate failed: {error}")))?;

    let after_target = target_cell_metrics(&candidate, target)?;
    let after_tri = triangular_quality(&candidate)?;
    let after_dual = dual_quality(&candidate)?;
    guard_quality(
        before_target,
        after_target,
        before_tri,
        after_tri,
        before_dual,
        after_dual,
    )?;

    *mesh = candidate;
    Ok(json!({
        "schema_version": 1,
        "geometry_only": true,
        "updated_count": updates.len(),
        "case": request.case,
        "target_cell": request.target_cell,
        "certificates": {"candidate_final": cert_json(&candidate_final)},
        "target": {
            "before": cell_json(before_target),
            "after": cell_json(after_target)
        },
        "global": {
            "tri_before": metrics_json(before_tri),
            "tri_after": metrics_json(after_tri),
            "dual_before": metrics_json(before_dual),
            "dual_after": metrics_json(after_dual)
        }
    }))
}

fn require_absolute(name: &str, path: &str) -> io::Result<()> {
    if Path::new(path).is_absolute() {
        Ok(())
    } else {
        Err(invalid_input(format!("{name} must be absolute")))
    }
}

fn validate_anchors(mesh: &MeshState, anchors: &[usize; 12]) -> io::Result<()> {
    let mut seen = BTreeSet::new();
    for &anchor in anchors {
        if !seen.insert(anchor) {
            return Err(invalid_input(format!("duplicate pentagon anchor {anchor}")));
        }
        if !mesh.is_vertex_live(anchor) {
            return Err(invalid_input(format!(
                "pentagon anchor {anchor} is not live"
            )));
        }
        let degree = mesh
            .active_triangle_slots()
            .filter(|&face| mesh.triangles()[face].contains(&anchor))
            .count();
        if degree != 5 {
            return Err(invalid_input(format!(
                "pentagon anchor {anchor} has degree {degree}, expected 5"
            )));
        }
    }
    Ok(())
}

fn validate_updates(
    updates: &[Update],
    mesh: &MeshState,
    anchors: &[usize; 12],
    id_for_physical: &dyn Fn(usize) -> io::Result<usize>,
) -> io::Result<BTreeMap<usize, CartesianPoint>> {
    let anchors = anchors.iter().copied().collect::<BTreeSet<_>>();
    let mut out = BTreeMap::new();
    for update in updates {
        let id = id_for_physical(update.id)?;
        if !mesh.is_vertex_live(id) {
            return Err(invalid_input(format!(
                "update id {} maps to non-live vertex {id}",
                update.id
            )));
        }
        if anchors.contains(&id) {
            return Err(invalid_input(format!(
                "update id {} would move pentagon anchor {id}",
                update.id
            )));
        }
        if out.contains_key(&id) {
            return Err(invalid_input(format!("duplicate update id {}", update.id)));
        }
        if update.xyz.iter().any(|value| !value.is_finite()) {
            return Err(invalid_input(format!(
                "update id {} contains non-finite xyz",
                update.id
            )));
        }
        let point = CartesianPoint::new(update.xyz[0], update.xyz[1], update.xyz[2]);
        let norm = dot(point, point).sqrt();
        if (norm - 1.0).abs() > UNIT_TOL {
            return Err(invalid_input(format!(
                "update id {} xyz norm {norm} is not unit",
                update.id
            )));
        }
        out.insert(id, point);
    }
    Ok(out)
}

fn reject_mixed_face_moves(
    mesh: &MeshState,
    updates: &BTreeMap<usize, CartesianPoint>,
    levels: &[Option<usize>],
) -> io::Result<()> {
    for face in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[face];
        let mut face_levels = [0usize; 3];
        for (idx, id) in tri.iter().copied().enumerate() {
            face_levels[idx] = levels.get(id).and_then(|level| *level).ok_or_else(|| {
                invalid_input(format!("active vertex {id} has no delivered level"))
            })?;
        }
        if face_levels.iter().any(|level| *level != face_levels[0])
            && tri.iter().any(|id| updates.contains_key(id))
        {
            return Err(invalid_input(format!(
                "update moves vertex on mixed delivered-level face {face}"
            )));
        }
    }
    Ok(())
}

fn check_move_lengths(
    mesh: &MeshState,
    updates: &BTreeMap<usize, CartesianPoint>,
) -> io::Result<()> {
    for (&id, &point) in updates {
        let local = local_spacings(mesh, id)?;
        let limit = MAX_MOVE_FRACTION * local.mean.min(local.median);
        let moved = central_angle(mesh.vertices()[id], point);
        if !moved.is_finite() || moved > limit {
            return Err(invalid_input(format!(
                "update vertex {id} moves {moved} rad, above 3% local spacing limit {limit} rad"
            )));
        }
    }
    Ok(())
}

struct Spacing {
    mean: f64,
    median: f64,
}

fn local_spacings(mesh: &MeshState, site: usize) -> io::Result<Spacing> {
    let seed = mesh
        .active_triangle_slots()
        .find(|&face| mesh.triangles()[face].contains(&site))
        .ok_or_else(|| invalid_input(format!("vertex {site} has no incident face")))?;
    let fan = mesh
        .triangle_fan_from(site, seed)
        .map_err(|error| invalid_input(format!("vertex {site} fan failed: {error}")))?;
    let mut neighbor_ids = BTreeSet::new();
    for face in fan {
        for &other in &mesh.triangles()[face] {
            if other != site {
                neighbor_ids.insert(other);
            }
        }
    }
    let mut lengths = neighbor_ids
        .into_iter()
        .map(|other| central_angle(mesh.vertices()[site], mesh.vertices()[other]))
        .collect::<Vec<_>>();
    lengths.sort_by(f64::total_cmp);
    if lengths.is_empty()
        || lengths
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid_input(
            "updated vertex has invalid local edge spacings",
        ));
    }
    let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
    let median = lengths[lengths.len() / 2];
    Ok(Spacing { mean, median })
}

fn level_by_slot(
    mesh: &MeshState,
    delivered_levels: &[usize],
    active_vertices: &[usize],
) -> Vec<Option<usize>> {
    let mut levels = vec![None; mesh.vertices().len()];
    for (&slot, &level) in active_vertices.iter().zip(delivered_levels) {
        levels[slot] = Some(level);
    }
    levels
}

fn verify_baseline(mesh: &MeshState, source: &SourceMesh) -> io::Result<()> {
    let active_vertices = mesh.active_vertex_slots().collect::<Vec<_>>();
    let active_faces = mesh.active_triangle_slots().collect::<Vec<_>>();
    if active_vertices.len() != source.physical_vertices.len()
        || active_faces.len() != source.physical_triangles.len()
    {
        return Err(invalid_input(
            "source_mpas active counts differ from current mesh",
        ));
    }
    for (physical, &slot) in active_vertices.iter().enumerate() {
        if distance(mesh.vertices()[slot], source.physical_vertices[physical]) > BASELINE_TOL {
            return Err(invalid_input(format!(
                "source_mpas physical cell {physical} differs from current mesh vertex slot {slot}"
            )));
        }
    }
    for (physical_face, &slot) in active_faces.iter().enumerate() {
        let mapped =
            source.physical_triangles[physical_face].map(|physical| active_vertices[physical]);
        if !same_cyclic_triangle(mesh.triangles()[slot], mapped) {
            return Err(invalid_input(format!(
                "source_mpas face {physical_face} topology differs from current mesh face slot {slot}"
            )));
        }
    }
    Ok(())
}

fn same_cyclic_triangle(a: [usize; 3], b: [usize; 3]) -> bool {
    a == b || a == [b[1], b[2], b[0]] || a == [b[2], b[0], b[1]]
}

fn read_mpas(path: &str) -> io::Result<SourceMesh> {
    let file = netcdf::open(path)
        .map_err(|error| invalid_input(format!("open source_mpas {path}: {error}")))?;
    let lon = values_f64(&file, "lonCell")?;
    let lat = values_f64(&file, "latCell")?;
    if lon.len() != lat.len() {
        return Err(invalid_input(format!(
            "lonCell/latCell length mismatch: {} vs {}",
            lon.len(),
            lat.len()
        )));
    }
    let cov = values_i32(&file, "cellsOnVertex")?;
    if cov.len() % 3 != 0 {
        return Err(invalid_input(format!(
            "cellsOnVertex length {} is not divisible by 3",
            cov.len()
        )));
    }
    let mut physical_vertices = Vec::with_capacity(lon.len());
    for (idx, (&lon, &lat)) in lon.iter().zip(&lat).enumerate() {
        if !lon.is_finite() || !lat.is_finite() {
            return Err(invalid_input(format!(
                "lonCell/latCell row {idx} is non-finite"
            )));
        }
        physical_vertices.push(lonlat_degrees_to_unit_xyz(LonLatDegrees::new(
            lon.to_degrees(),
            lat.to_degrees(),
        )));
    }
    let mut physical_triangles = Vec::with_capacity(cov.len() / 3);
    for face in 0..cov.len() / 3 {
        physical_triangles.push([
            mpas_cell_index(cov[face * 3], lon.len(), "cellsOnVertex")?,
            mpas_cell_index(cov[face * 3 + 1], lon.len(), "cellsOnVertex")?,
            mpas_cell_index(cov[face * 3 + 2], lon.len(), "cellsOnVertex")?,
        ]);
    }
    Ok(SourceMesh {
        physical_vertices,
        physical_triangles,
    })
}

fn read_tail_levels(path: &str, cells: usize) -> io::Result<Vec<usize>> {
    let file = netcdf::open(path)
        .map_err(|error| invalid_input(format!("open source_gridfile {path}: {error}")))?;
    let raw = values_i32(&file, "earthmesh_w_refine_level")?;
    if raw.len() < cells {
        return Err(invalid_input(format!(
            "earthmesh_w_refine_level rows {} fewer than cells {cells}",
            raw.len()
        )));
    }
    raw[raw.len() - cells..]
        .iter()
        .map(|&level| {
            usize::try_from(level)
                .map_err(|_| invalid_input(format!("negative delivered level {level}")))
        })
        .collect()
}

fn values_f64(file: &netcdf::File, name: &str) -> io::Result<Vec<f64>> {
    file.variable(name)
        .ok_or_else(|| invalid_input(format!("missing {name}")))?
        .get_values::<f64, _>(..)
        .map_err(|error| invalid_input(format!("read {name}: {error}")))
}

fn values_i32(file: &netcdf::File, name: &str) -> io::Result<Vec<i32>> {
    file.variable(name)
        .ok_or_else(|| invalid_input(format!("missing {name}")))?
        .get_values::<i32, _>(..)
        .map_err(|error| invalid_input(format!("read {name}: {error}")))
}

fn mpas_cell_index(value: i32, cells: usize, name: &str) -> io::Result<usize> {
    if value <= 0 {
        return Err(invalid_input(format!(
            "{name} contains non-positive cell id {value}"
        )));
    }
    let id = usize::try_from(value).map_err(|_| invalid_input(format!("invalid id {value}")))?;
    if id > cells {
        return Err(invalid_input(format!(
            "{name} id {id} exceeds nCells {cells}"
        )));
    }
    Ok(id - 1)
}

fn target_cell_metrics(mesh: &MeshState, site: usize) -> io::Result<CellMetrics> {
    let seed = mesh
        .active_triangle_slots()
        .find(|&face| mesh.triangles()[face].contains(&site))
        .ok_or_else(|| invalid_data(format!("site {site} has no incident face")))?;
    let cell = mesh
        .voronoi_cell_from(site, seed)
        .map_err(|error| invalid_data(format!("target Voronoi cell failed: {error}")))?;
    edge_metrics(&cell.corners)
}

fn edge_metrics(points: &[CartesianPoint]) -> io::Result<CellMetrics> {
    if points.len() < 3 {
        return Err(invalid_data("cell has fewer than 3 corners"));
    }
    let mut lengths = Vec::with_capacity(points.len());
    for idx in 0..points.len() {
        lengths.push(central_angle(points[idx], points[(idx + 1) % points.len()]));
    }
    if lengths
        .iter()
        .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(invalid_data("cell has invalid edge length"));
    }
    let mean = lengths.iter().sum::<f64>() / lengths.len() as f64;
    let variance = lengths
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / lengths.len() as f64;
    let min = lengths.iter().copied().fold(f64::INFINITY, f64::min);
    let max = lengths.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(CellMetrics {
        edge_cv: variance.sqrt() / mean.abs(),
        aspect: max / min,
    })
}

fn triangular_quality(mesh: &MeshState) -> io::Result<GlobalMetrics> {
    let vertex_slots = mesh.active_vertex_slots().collect::<Vec<_>>();
    let vertex_index: BTreeMap<usize, usize> = vertex_slots
        .iter()
        .copied()
        .enumerate()
        .map(|(index, slot)| (slot, index))
        .collect();
    let face_slots = mesh.active_triangle_slots().collect::<Vec<_>>();
    let face_index: BTreeMap<usize, usize> = face_slots
        .iter()
        .copied()
        .enumerate()
        .map(|(index, slot)| (slot, index))
        .collect();
    let vertices = vertex_slots
        .iter()
        .map(|&slot| point(mesh.vertices()[slot]))
        .collect();
    let cells = face_slots
        .iter()
        .map(|&face| QualityCell {
            vertices: mesh.triangles()[face]
                .iter()
                .map(|slot| vertex_index[slot])
                .collect(),
            refine_level: None,
            neighbors: mesh.neighbours()[face]
                .iter()
                .copied()
                .filter(|&neighbor| mesh.is_triangle_live(neighbor))
                .map(|neighbor| face_index[&neighbor])
                .collect(),
        })
        .collect();
    let report = compute(
        &QualityMeshInput { vertices, cells },
        &QualityThresholds::default(),
    );
    global_metrics(report, Some(angle_stats(mesh)))
}

fn dual_quality(mesh: &MeshState) -> io::Result<GlobalMetrics> {
    let face_slots = mesh.active_triangle_slots().collect::<Vec<_>>();
    let face_index: BTreeMap<usize, usize> = face_slots
        .iter()
        .copied()
        .enumerate()
        .map(|(index, slot)| (slot, index))
        .collect();
    let centers = face_slots
        .iter()
        .map(|&face| {
            mesh.circumcentre(face)
                .map_err(|error| invalid_data(format!("face {face} circumcentre failed: {error}")))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let vertices = centers.iter().map(|&center| point(center)).collect();
    let seeds = site_seeds(mesh);
    let neighbors = dual_neighbor_map(mesh);
    let cells = mesh
        .active_vertex_slots()
        .map(|site| {
            let seed = *seeds
                .get(&site)
                .ok_or_else(|| invalid_data(format!("site {site} has no seed face")))?;
            let mut fan = mesh
                .triangle_fan_from(site, seed)
                .map_err(|error| invalid_data(format!("site {site} fan failed: {error}")))?;
            if let Some((start, _)) = fan.iter().enumerate().min_by_key(|(_, &face)| face) {
                fan.rotate_left(start);
            }
            Ok(QualityCell {
                vertices: fan.iter().map(|face| face_index[face]).collect(),
                refine_level: None,
                neighbors: neighbors.get(&site).cloned().unwrap_or_default(),
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let report = compute(
        &QualityMeshInput { vertices, cells },
        &QualityThresholds::default(),
    );
    global_metrics(report, None)
}

fn global_metrics(
    report: earthmesh_quality::MeshQualityReport,
    angle_stats: Option<AngleStats>,
) -> io::Result<GlobalMetrics> {
    if report.verdict == earthmesh_quality::QualityLevel::Fail {
        return Err(invalid_data("quality report verdict is fail"));
    }
    let metrics = GlobalMetrics {
        cells: report.geometry.cell_count,
        triangles: report.topology.triangle_cell_count,
        edge_cv_max: report.geometry.cell_edge_length_cv.max,
        aspect_max: report.geometry.aspect_ratio.max,
        adjacent_ratio: report.topology.max_adjacent_resolution_ratio,
        min_angle: report.geometry.min_angle_deg,
        max_angle: report.geometry.max_angle_deg,
        preferred_bad_angle_count: angle_stats.map(|stats| stats.preferred_bad_angle_count),
        preferred_bad_triangle_count: angle_stats.map(|stats| stats.preferred_bad_triangle_count),
        angle_rmse_to_60: angle_stats.map(|stats| stats.rmse_to_60),
    };
    metrics.require_finite()?;
    Ok(metrics)
}

impl GlobalMetrics {
    fn require_finite(self) -> io::Result<()> {
        let values = [
            self.edge_cv_max,
            self.aspect_max,
            self.adjacent_ratio,
            self.min_angle,
            self.max_angle,
        ];
        if values.iter().any(|value| !value.is_finite())
            || self
                .angle_rmse_to_60
                .is_some_and(|value| !value.is_finite())
        {
            return Err(invalid_data("non-finite quality metric"));
        }
        Ok(())
    }
}

fn point(point: CartesianPoint) -> Point {
    let lonlat = earthmesh_mesh::xyz_to_lonlat_degrees(point);
    Point::new(lonlat.lon_degrees, lonlat.lat_degrees)
}

fn site_seeds(mesh: &MeshState) -> BTreeMap<usize, usize> {
    let mut seeds = BTreeMap::new();
    for face in mesh.active_triangle_slots() {
        for site in mesh.triangles()[face] {
            seeds.entry(site).or_insert(face);
        }
    }
    seeds
}

fn dual_neighbor_map(mesh: &MeshState) -> BTreeMap<usize, Vec<usize>> {
    let site_index: BTreeMap<usize, usize> = mesh
        .active_vertex_slots()
        .enumerate()
        .map(|(index, slot)| (slot, index))
        .collect();
    let mut neighbors = BTreeMap::<usize, BTreeSet<usize>>::new();
    for face in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[face];
        for site in tri {
            neighbors.entry(site).or_default().extend(
                tri.into_iter()
                    .filter(|&other| other != site)
                    .map(|other| site_index[&other]),
            );
        }
    }
    neighbors
        .into_iter()
        .map(|(site, neighbors)| (site, neighbors.into_iter().collect()))
        .collect()
}

#[derive(Clone, Copy)]
struct AngleStats {
    preferred_bad_angle_count: usize,
    preferred_bad_triangle_count: usize,
    rmse_to_60: f64,
}

fn angle_stats(mesh: &MeshState) -> AngleStats {
    let mut sum = 0.0;
    let mut count = 0usize;
    let mut bad_angles = 0usize;
    let mut bad_triangles = 0usize;
    for face in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[face];
        let points = tri.map(|id| mesh.vertices()[id]);
        let angles = spherical_triangle_angles(points);
        let mut triangle_bad = false;
        for angle in angles {
            if !(PREFERRED_MIN_DEG - QUALITY_TOL..=PREFERRED_MAX_DEG + QUALITY_TOL).contains(&angle)
            {
                bad_angles += 1;
                triangle_bad = true;
            }
            sum += (angle - 60.0).powi(2);
            count += 1;
        }
        if triangle_bad {
            bad_triangles += 1;
        }
    }
    AngleStats {
        preferred_bad_angle_count: bad_angles,
        preferred_bad_triangle_count: bad_triangles,
        rmse_to_60: (sum / count.max(1) as f64).sqrt(),
    }
}

fn spherical_triangle_angles(points: [CartesianPoint; 3]) -> [f64; 3] {
    [
        corner_angle(points[2], points[0], points[1]),
        corner_angle(points[0], points[1], points[2]),
        corner_angle(points[1], points[2], points[0]),
    ]
}

fn corner_angle(a: CartesianPoint, b: CartesianPoint, c: CartesianPoint) -> f64 {
    let Some(ta) = tangent(b, a) else {
        return f64::NAN;
    };
    let Some(tc) = tangent(b, c) else {
        return f64::NAN;
    };
    dot(ta, tc).clamp(-1.0, 1.0).acos().to_degrees()
}

fn tangent(origin: CartesianPoint, target: CartesianPoint) -> Option<CartesianPoint> {
    let projection = dot(origin, target);
    let v = CartesianPoint::new(
        target.x - projection * origin.x,
        target.y - projection * origin.y,
        target.z - projection * origin.z,
    );
    let norm = dot(v, v).sqrt();
    (norm > 64.0 * f64::EPSILON).then_some(CartesianPoint::new(v.x / norm, v.y / norm, v.z / norm))
}

fn guard_quality(
    before_target: CellMetrics,
    after_target: CellMetrics,
    before_tri: GlobalMetrics,
    after_tri: GlobalMetrics,
    before_dual: GlobalMetrics,
    after_dual: GlobalMetrics,
) -> io::Result<()> {
    if [
        before_target.edge_cv,
        before_target.aspect,
        after_target.edge_cv,
        after_target.aspect,
    ]
    .iter()
    .any(|value| !value.is_finite())
    {
        return Err(invalid_data("non-finite target metric"));
    }
    before_tri.require_finite()?;
    after_tri.require_finite()?;
    before_dual.require_finite()?;
    after_dual.require_finite()?;
    for (name, before, after) in [
        ("tri_min_angle", -before_tri.min_angle, -after_tri.min_angle),
        ("tri_max_angle", before_tri.max_angle, after_tri.max_angle),
        (
            "dual_edge_cv_max",
            before_dual.edge_cv_max,
            after_dual.edge_cv_max,
        ),
        (
            "dual_aspect_max",
            before_dual.aspect_max,
            after_dual.aspect_max,
        ),
        (
            "dual_adjacent_ratio",
            before_dual.adjacent_ratio,
            after_dual.adjacent_ratio,
        ),
        (
            "dual_min_angle",
            -before_dual.min_angle,
            -after_dual.min_angle,
        ),
        (
            "dual_max_angle",
            before_dual.max_angle,
            after_dual.max_angle,
        ),
    ] {
        if after > before + QUALITY_TOL {
            return Err(invalid_data(format!(
                "{name} regressed: {before} -> {after}"
            )));
        }
    }
    for (name, before, after) in [
        ("tri_cells", before_tri.cells, after_tri.cells),
        ("triangles", before_tri.triangles, after_tri.triangles),
        ("dual_cells", before_dual.cells, after_dual.cells),
        (
            "dual_triangles",
            before_dual.triangles,
            after_dual.triangles,
        ),
    ] {
        if after != before {
            return Err(invalid_data(format!(
                "{name} count changed: {before} -> {after}"
            )));
        }
    }
    if after_dual.edge_cv_max > before_dual.edge_cv_max - SIGNIFICANT {
        return Err(invalid_data(format!(
            "global dual edge CV did not significantly improve: {} -> {}",
            before_dual.edge_cv_max, after_dual.edge_cv_max
        )));
    }
    if after_target.edge_cv > before_target.edge_cv - SIGNIFICANT {
        return Err(invalid_data(format!(
            "target edge CV did not significantly improve: {} -> {}",
            before_target.edge_cv, after_target.edge_cv
        )));
    }
    if after_target.aspect > before_target.aspect - SIGNIFICANT {
        return Err(invalid_data(format!(
            "target aspect did not significantly improve: {} -> {}",
            before_target.aspect, after_target.aspect
        )));
    }
    if after_tri.preferred_bad_angle_count > before_tri.preferred_bad_angle_count {
        return Err(invalid_data(format!(
            "preferred angle bad count increased: {:?} -> {:?}",
            before_tri.preferred_bad_angle_count, after_tri.preferred_bad_angle_count
        )));
    }
    if after_tri.preferred_bad_triangle_count > before_tri.preferred_bad_triangle_count {
        return Err(invalid_data(format!(
            "preferred bad triangle count increased: {:?} -> {:?}",
            before_tri.preferred_bad_triangle_count, after_tri.preferred_bad_triangle_count
        )));
    }
    if let (Some(before), Some(after)) = (before_tri.angle_rmse_to_60, after_tri.angle_rmse_to_60) {
        if after > before + QUALITY_TOL {
            return Err(invalid_data(format!(
                "triangle angle RMSE to 60 regressed: {before} -> {after}"
            )));
        }
    }
    Ok(())
}

fn cert_json(report: &GeometryCertificateReport) -> Value {
    json!({
        "angle_contract": report.angle_contract_id.as_str(),
        "vertices": report.vertices,
        "edges": report.edges,
        "faces": report.faces,
        "min_angle_degrees": report.min_angle_degrees,
        "max_angle_degrees": report.max_angle_degrees,
        "delaunay_violations": report.delaunay_violations,
        "topology_errors": report.topology_errors,
        "open_edges": report.open_edges,
        "voronoi_invalid_cells": report.voronoi_invalid_cells
    })
}

fn cell_json(metrics: CellMetrics) -> Value {
    json!({"edge_cv": metrics.edge_cv, "aspect": metrics.aspect})
}

fn metrics_json(metrics: GlobalMetrics) -> Value {
    json!({
        "cells": metrics.cells,
        "triangles": metrics.triangles,
        "edge_cv_max": metrics.edge_cv_max,
        "aspect_max": metrics.aspect_max,
        "adjacent_ratio": metrics.adjacent_ratio,
        "min_angle": metrics.min_angle,
        "max_angle": metrics.max_angle,
        "preferred_bad_angle_count": metrics.preferred_bad_angle_count,
        "preferred_bad_triangle_count": metrics.preferred_bad_triangle_count,
        "angle_rmse_to_60": metrics.angle_rmse_to_60
    })
}

fn central_angle(a: CartesianPoint, b: CartesianPoint) -> f64 {
    cross_norm(a, b).atan2(dot(a, b))
}

fn distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt()
}

fn dot(a: CartesianPoint, b: CartesianPoint) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross_norm(a: CartesianPoint, b: CartesianPoint) -> f64 {
    ((a.y * b.z - a.z * b.y).powi(2)
        + (a.z * b.x - a.x * b.z).powi(2)
        + (a.x * b.y - a.y * b.x).powi(2))
    .sqrt()
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mesh() -> MeshState {
        MeshState::from_parts(
            vec![
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 0.0),
                CartesianPoint::new(1.0, 0.0, 0.0),
                CartesianPoint::new(0.0, 1.0, 0.0),
                CartesianPoint::new(0.0, 0.0, 1.0),
                CartesianPoint::new(-1.0, -1.0, -1.0),
            ],
            vec![
                [1, 1, 1],
                [1, 1, 1],
                [2, 3, 4],
                [2, 5, 3],
                [3, 5, 4],
                [4, 5, 2],
            ],
        )
        .expect("test mesh")
    }

    fn metrics() -> GlobalMetrics {
        GlobalMetrics {
            cells: 100,
            triangles: 200,
            edge_cv_max: 0.5,
            aspect_max: 4.0,
            adjacent_ratio: 1.8,
            min_angle: 38.4,
            max_angle: 81.8,
            preferred_bad_angle_count: Some(2),
            preferred_bad_triangle_count: Some(2),
            angle_rmse_to_60: Some(10.0),
        }
    }

    fn targets() -> (CellMetrics, CellMetrics) {
        (
            CellMetrics {
                edge_cv: 0.4,
                aspect: 3.0,
            },
            CellMetrics {
                edge_cv: 0.3,
                aspect: 2.0,
            },
        )
    }

    fn duals() -> (GlobalMetrics, GlobalMetrics) {
        let before = GlobalMetrics {
            min_angle: 100.0,
            max_angle: 130.0,
            preferred_bad_angle_count: None,
            preferred_bad_triangle_count: None,
            angle_rmse_to_60: None,
            ..metrics()
        };
        let after = GlobalMetrics {
            edge_cv_max: 0.4,
            aspect_max: 3.9,
            adjacent_ratio: 1.7,
            min_angle: 101.0,
            max_angle: 129.0,
            ..before
        };
        (before, after)
    }

    #[test]
    fn gate_allows_existing_preferred_angle_violations_when_not_worse() {
        let (before_target, after_target) = targets();
        let before_tri = metrics();
        let after_tri = GlobalMetrics {
            edge_cv_max: 0.51,
            aspect_max: 4.1,
            adjacent_ratio: 1.9,
            angle_rmse_to_60: Some(9.0),
            ..before_tri
        };
        let (before_dual, after_dual) = duals();

        assert!(guard_quality(
            before_target,
            after_target,
            before_tri,
            after_tri,
            before_dual,
            after_dual
        )
        .is_ok());
    }

    #[test]
    fn preferred_angle_count_uses_tolerance_and_counts_triangles_separately() {
        let angles = [39.9999999995, 60.0, 80.0000000005];
        let mut bad_angles = 0;
        let mut triangle_bad = false;
        for angle in angles {
            if !(PREFERRED_MIN_DEG - QUALITY_TOL..=PREFERRED_MAX_DEG + QUALITY_TOL).contains(&angle)
            {
                bad_angles += 1;
                triangle_bad = true;
            }
        }
        assert_eq!(bad_angles, 0);
        assert!(!triangle_bad);

        let angles = [39.99, 60.0, 80.01];
        let mut bad_angles = 0;
        let mut triangle_bad = false;
        for angle in angles {
            if !(PREFERRED_MIN_DEG - QUALITY_TOL..=PREFERRED_MAX_DEG + QUALITY_TOL).contains(&angle)
            {
                bad_angles += 1;
                triangle_bad = true;
            }
        }
        assert_eq!(bad_angles, 2);
        assert!(triangle_bad);
    }

    #[test]
    fn gate_rejects_bad_angle_same_count_spread_to_more_triangles() {
        let (before_target, after_target) = targets();
        let before_tri = metrics();
        let (before_dual, after_dual) = duals();
        let err = guard_quality(
            before_target,
            after_target,
            before_tri,
            GlobalMetrics {
                preferred_bad_angle_count: Some(2),
                preferred_bad_triangle_count: Some(3),
                angle_rmse_to_60: Some(9.0),
                ..before_tri
            },
            before_dual,
            after_dual,
        )
        .expect_err("spread bad triangles rejected");
        assert!(err.to_string().contains("preferred bad triangle count"));
    }

    #[test]
    fn gate_rejects_new_preferred_bad_angle_dual_angle_regression_and_nan() {
        let (before_target, after_target) = targets();
        let before_tri = metrics();
        let (before_dual, after_dual) = duals();

        let err = guard_quality(
            before_target,
            after_target,
            before_tri,
            GlobalMetrics {
                preferred_bad_angle_count: Some(3),
                ..before_tri
            },
            before_dual,
            after_dual,
        )
        .expect_err("bad angle count rejected");
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert!(err
            .to_string()
            .contains("preferred angle bad count increased"));

        let err = guard_quality(
            before_target,
            after_target,
            before_tri,
            GlobalMetrics {
                angle_rmse_to_60: Some(9.0),
                ..before_tri
            },
            before_dual,
            GlobalMetrics {
                min_angle: 99.0,
                ..after_dual
            },
        )
        .expect_err("dual angle regression rejected");
        assert!(err.to_string().contains("dual_min_angle"));

        let err = guard_quality(
            before_target,
            after_target,
            before_tri,
            GlobalMetrics {
                edge_cv_max: f64::NAN,
                ..before_tri
            },
            before_dual,
            after_dual,
        )
        .expect_err("nan rejected");
        assert!(err.to_string().contains("non-finite quality metric"));
    }

    #[test]
    fn rejects_count_changes() {
        let (before_target, after_target) = targets();
        let before_tri = metrics();
        let (before_dual, after_dual) = duals();
        let err = guard_quality(
            before_target,
            after_target,
            before_tri,
            GlobalMetrics {
                triangles: before_tri.triangles - 1,
                ..before_tri
            },
            before_dual,
            after_dual,
        )
        .expect_err("any triangle count change rejected");
        assert!(err.to_string().contains("triangles count changed"));
    }

    #[test]
    fn rejects_duplicate_update_ids() {
        let mesh = test_mesh();
        let updates = vec![
            Update {
                id: 0,
                xyz: [1.0, 0.0, 0.0],
            },
            Update {
                id: 0,
                xyz: [1.0, 0.0, 0.0],
            },
        ];
        let active = mesh.active_vertex_slots().collect::<Vec<_>>();
        let mapper = |physical: usize| Ok(active[physical]);
        let err = validate_updates(&updates, &mesh, &[99; 12], &mapper).expect_err("duplicate");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("duplicate update id 0"));
    }

    #[test]
    fn rejects_mixed_face_vertex_moves() {
        let mesh = test_mesh();
        let mut updates = BTreeMap::new();
        updates.insert(2, CartesianPoint::new(1.0, 0.0, 0.0));
        let levels = vec![None, None, Some(0), Some(1), Some(1), Some(1)];
        let err = reject_mixed_face_moves(&mesh, &updates, &levels).expect_err("mixed move");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("mixed delivered-level face"));
    }
}
