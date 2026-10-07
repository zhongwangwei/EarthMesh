pub mod heterogeneity;

use crate::fingerprint::mesh_fingerprint;
use crate::remap::{voronoi_rings, ConservativeRemap};
use earthmesh_mesh::MeshState;
use rayon::prelude::*;
use std::collections::BinaryHeap;

mod sealed {
    pub trait Sealed {}
}

pub trait LevelFieldRole: sealed::Sealed {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRole;
impl sealed::Sealed for SourceRole {}
impl LevelFieldRole for SourceRole {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetRole;
impl sealed::Sealed for TargetRole {}
impl LevelFieldRole for TargetRole {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellLevelField<R: LevelFieldRole> {
    active_sites: Vec<usize>,
    levels: Vec<usize>,
    _role: std::marker::PhantomData<R>,
}

pub type SourceLevelField = CellLevelField<SourceRole>;
pub type TargetLevelField = CellLevelField<TargetRole>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RasterLevelField {
    nlon: usize,
    nlat: usize,
    levels: Vec<usize>,
}

impl RasterLevelField {
    pub fn new(nlon: usize, nlat: usize, levels: Vec<usize>) -> Result<Self, String> {
        if nlon < 4 || nlat < 2 {
            return Err("raster level field needs at least 4x2 cells".into());
        }
        let expected = nlon
            .checked_mul(nlat)
            .ok_or("raster level field dimensions overflow usize")?;
        if levels.len() != expected {
            return Err(format!(
                "raster level field has {} rows but {nlon}x{nlat} needs {expected}",
                levels.len()
            ));
        }
        Ok(Self { nlon, nlat, levels })
    }

    pub fn nlon(&self) -> usize {
        self.nlon
    }

    pub fn nlat(&self) -> usize {
        self.nlat
    }

    pub fn levels(&self) -> &[usize] {
        &self.levels
    }

    fn spherical_cells(&self) -> Vec<Vec<(f64, f64)>> {
        let dlon = 360.0 / self.nlon as f64;
        let dlat = 180.0 / self.nlat as f64;
        (0..self.levels.len())
            .map(|cell| self.spherical_cell(cell, dlon, dlat))
            .collect()
    }

    /// The raster cells that may overlap `rings`, as polygons, with their
    /// numbers in ascending order: those whose latitude-longitude box meets
    /// the box of the rings' joint cap, widened by a cell for the bulge of a
    /// cell's great-circle edges. A projection onto a region needs only
    /// these, however fine the raster covering the globe; in ascending order
    /// they keep the overlaps' order, so the projection is the same float
    /// for float.
    pub(crate) fn spherical_cells_near(
        &self,
        rings: &[Vec<(f64, f64)>],
    ) -> Result<(crate::remap::Rings, Vec<usize>), String> {
        let points = rings
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|&(lon, lat)| earthmesh_geometry::Point::new(lon, lat))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let cap = earthmesh_boundary::SphericalCap::for_rings(&points)
            .ok_or("the projected cells have no spherical cap")?;
        let (lon, lat) = cap.center_lon_lat_degrees();
        let radius = cap.radius_radians();
        let dlon = 360.0 / self.nlon as f64;
        let dlat = 180.0 / self.nlat as f64;
        let (south, north) = (
            lat - radius.to_degrees() - dlat,
            lat + radius.to_degrees() + dlat,
        );
        let row = |lat: f64| (((lat + 90.0) / dlat).floor().max(0.0) as usize).min(self.nlat - 1);
        let rows = row(south)..=row(north);
        // Tangent meridians bound a cap clear of the poles.
        let half_width = if south <= -90.0 || north >= 90.0 || radius >= std::f64::consts::FRAC_PI_2
        {
            None
        } else {
            let sine = radius.sin() / lat.to_radians().cos();
            (sine < 1.0).then(|| sine.asin().to_degrees() + dlon)
        };
        let columns = match half_width.filter(|&half| 2.0 * half + dlon < 360.0) {
            None => (0..self.nlon).collect::<Vec<_>>(),
            Some(half) => {
                let first = ((lon - half + 180.0) / dlon).floor() as isize;
                let last = ((lon + half + 180.0) / dlon).floor() as isize;
                (first..=last)
                    .map(|column| column.rem_euclid(self.nlon as isize) as usize)
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }
        };
        let mut cells = Vec::new();
        let mut ids = Vec::new();
        for row in rows {
            for &column in &columns {
                let cell = row * self.nlon + column;
                cells.push(self.spherical_cell(cell, dlon, dlat));
                ids.push(cell);
            }
        }
        Ok((cells, ids))
    }

    /// Raster cell `cell` (row-major from the south-west) as a spherical
    /// polygon; the polar rows are triangles meeting at the pole.
    pub(crate) fn spherical_cell(&self, cell: usize, dlon: f64, dlat: f64) -> Vec<(f64, f64)> {
        let (j, i) = (cell / self.nlon, cell % self.nlon);
        let south = -90.0 + j as f64 * dlat;
        let north = south + dlat;
        let west = -180.0 + i as f64 * dlon;
        let east = west + dlon;
        if j == 0 {
            vec![(0.0, -90.0), (east, north), (west, north)]
        } else if j + 1 == self.nlat {
            vec![(west, south), (east, south), (0.0, 90.0)]
        } else {
            vec![(west, south), (east, south), (east, north), (west, north)]
        }
    }
}

impl<R: LevelFieldRole> CellLevelField<R> {
    pub fn from_active_voronoi_cells(mesh: &MeshState, levels: Vec<usize>) -> Result<Self, String> {
        let active_sites = mesh.active_vertex_slots().collect::<Vec<_>>();
        if levels.len() != active_sites.len() {
            return Err(format!(
                "level field has {} rows but mesh has {} active Voronoi cells",
                levels.len(),
                active_sites.len()
            ));
        }
        Ok(Self {
            active_sites,
            levels,
            _role: std::marker::PhantomData,
        })
    }

    pub fn levels(&self) -> &[usize] {
        &self.levels
    }

    pub fn active_sites(&self) -> &[usize] {
        &self.active_sites
    }

    fn validate_for(&self, mesh: &MeshState) -> Result<(), String> {
        let current = mesh.active_vertex_slots().collect::<Vec<_>>();
        if self.active_sites != current {
            return Err("level field active cell ids do not match the mesh".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementWitness {
    pub kind: &'static str,
    pub target_cell: usize,
    pub target_site: usize,
    pub source_cell: Option<usize>,
    pub source_site: Option<usize>,
    pub neighbour_cell: Option<usize>,
    pub neighbour_site: Option<usize>,
    pub required_level: usize,
    pub delivered_level: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalCellRequirementReport {
    target_fingerprint: u64,
    target_cells: usize,
    physical_residuals: usize,
    balance_residuals: usize,
    required_levels: Vec<usize>,
    witnesses: Vec<RequirementWitness>,
}

impl FinalCellRequirementReport {
    pub fn target_cells(&self) -> usize {
        self.target_cells
    }

    pub(crate) fn target_fingerprint(&self) -> u64 {
        self.target_fingerprint
    }

    pub fn physical_residuals(&self) -> usize {
        self.physical_residuals
    }

    pub fn balance_residuals(&self) -> usize {
        self.balance_residuals
    }

    pub fn required_levels(&self) -> &[usize] {
        &self.required_levels
    }

    pub fn witnesses(&self) -> &[RequirementWitness] {
        &self.witnesses
    }
}

pub type FinalCellRequirementCertificate = FinalCellRequirementReport;
pub type FinalCellRequirementResiduals = FinalCellRequirementReport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalCellRequirementError {
    InvalidInput(String),
    Residuals(FinalCellRequirementResiduals),
}

impl FinalCellRequirementError {
    pub fn physical_residuals(&self) -> usize {
        match self {
            Self::InvalidInput(_) => 0,
            Self::Residuals(report) => report.physical_residuals(),
        }
    }

    pub fn balance_residuals(&self) -> usize {
        match self {
            Self::InvalidInput(_) => 0,
            Self::Residuals(report) => report.balance_residuals(),
        }
    }

    pub fn witnesses(&self) -> &[RequirementWitness] {
        match self {
            Self::InvalidInput(_) => &[],
            Self::Residuals(report) => report.witnesses(),
        }
    }
}

impl std::fmt::Display for FinalCellRequirementError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(reason) => formatter.write_str(reason),
            Self::Residuals(report) => write!(
                formatter,
                "{} physical and {} balance residual(s)",
                report.physical_residuals(),
                report.balance_residuals()
            ),
        }
    }
}

impl std::error::Error for FinalCellRequirementError {}

pub fn certify_final_cell_requirements(
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    max_adjacent_level_delta: usize,
) -> Result<FinalCellRequirementCertificate, FinalCellRequirementError> {
    let remap = if source_mesh == target_mesh {
        ConservativeRemap::identity_for_mesh(target_mesh)
    } else {
        ConservativeRemap::between_voronoi_meshes(source_mesh, target_mesh)
            .map_err(FinalCellRequirementError::InvalidInput)?
    };
    certify_final_cell_requirements_with_remap(
        source_mesh,
        source_levels,
        target_mesh,
        target_levels,
        max_adjacent_level_delta,
        &remap,
    )
}

pub fn certify_final_cell_requirements_with_remap(
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    max_adjacent_level_delta: usize,
    remap: &ConservativeRemap,
) -> Result<FinalCellRequirementCertificate, FinalCellRequirementError> {
    let report = final_cell_requirement_report(
        source_mesh,
        source_levels,
        target_mesh,
        target_levels,
        max_adjacent_level_delta,
        remap,
    )
    .map_err(FinalCellRequirementError::InvalidInput)?;
    if report.physical_residuals == 0 && report.balance_residuals == 0 {
        Ok(report)
    } else {
        Err(FinalCellRequirementError::Residuals(report))
    }
}

pub fn certify_final_cell_requirements_from_raster(
    source_levels: &RasterLevelField,
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    max_adjacent_level_delta: usize,
) -> Result<FinalCellRequirementCertificate, FinalCellRequirementError> {
    target_levels
        .validate_for(target_mesh)
        .map_err(FinalCellRequirementError::InvalidInput)?;
    let remap = ConservativeRemap::spherical_overlap(
        &source_levels.spherical_cells(),
        &voronoi_rings(target_mesh).map_err(FinalCellRequirementError::InvalidInput)?,
    )
    .map_err(FinalCellRequirementError::InvalidInput)?;
    let remap_certificate =
        remap.certify_spherical_overlap(source_levels.levels().len(), target_levels.levels().len());
    if remap_certificate.negative_weights()
        + remap_certificate.bad_row_sums()
        + remap_certificate.bad_lineage_rows()
        != 0
        || remap_certificate.constant_closure_error() > remap_certificate.closure_tolerance()
        || remap_certificate.global_area_closure_error() > remap_certificate.closure_tolerance()
    {
        return Err(FinalCellRequirementError::InvalidInput(
            format!(
                "raster-to-Voronoi overlap remap failed certification: negative={}, bad_rows={}, bad_lineage={}, constant_error={}, area_error={}, tolerance={}",
                remap_certificate.negative_weights(),
                remap_certificate.bad_row_sums(),
                remap_certificate.bad_lineage_rows(),
                remap_certificate.constant_closure_error(),
                remap_certificate.global_area_closure_error(),
                remap_certificate.closure_tolerance(),
            ),
        ));
    }
    let (required_levels, source_for_target) =
        maximum_overlapping_levels(&remap, source_levels.levels(), target_levels.levels().len())
            .map_err(FinalCellRequirementError::InvalidInput)?;
    let report = final_report_from_required_levels(
        target_mesh,
        target_levels,
        required_levels,
        source_for_target,
        None,
        max_adjacent_level_delta,
    );
    if report.physical_residuals == 0 && report.balance_residuals == 0 {
        Ok(report)
    } else {
        Err(FinalCellRequirementError::Residuals(report))
    }
}

/// The raster's requirement on the cells of a built region (on-demand
/// reverse coarsening): the highest level among the raster cells each cell
/// overlaps, for every active site of `mesh` in slot order. Sites in
/// `cellless` -- the region's open edge, on the frame's far side -- have no
/// cell and require level zero by construction. The overlap is certified as
/// on the whole sphere, its tolerances scaled by `whole_cells`, so each
/// cell gets the level the whole sphere's projection gives it.
pub fn region_required_levels_from_raster(
    raster: &RasterLevelField,
    mesh: &MeshState,
    cellless: &std::collections::BTreeSet<usize>,
    whole_cells: usize,
) -> Result<Vec<usize>, String> {
    region_required_levels_with_sources(raster, mesh, cellless, whole_cells).map(|levels| levels.0)
}

/// `certify_final_cell_requirements_from_raster` on the final mesh of a
/// built region published without the rest of the sphere: every cell off
/// the outer boundary (`cellless`) gets the raster's requirement as
/// `region_required_levels_from_raster` projects it, the boundary sites --
/// on the frame's far side, settled by construction -- require level zero,
/// and balance is checked across every edge.
pub fn certify_region_final_cell_requirements_from_raster(
    raster: &RasterLevelField,
    mesh: &MeshState,
    levels: &TargetLevelField,
    cellless: &std::collections::BTreeSet<usize>,
    whole_cells: usize,
    max_adjacent_level_delta: usize,
) -> Result<FinalCellRequirementCertificate, FinalCellRequirementError> {
    levels
        .validate_for(mesh)
        .map_err(FinalCellRequirementError::InvalidInput)?;
    let (required_levels, source_for_target) =
        region_required_levels_with_sources(raster, mesh, cellless, whole_cells)
            .map_err(FinalCellRequirementError::InvalidInput)?;
    let report = final_report_from_required_levels(
        mesh,
        levels,
        required_levels,
        source_for_target,
        None,
        max_adjacent_level_delta,
    );
    if report.physical_residuals == 0 && report.balance_residuals == 0 {
        Ok(report)
    } else {
        Err(FinalCellRequirementError::Residuals(report))
    }
}

fn region_required_levels_with_sources(
    raster: &RasterLevelField,
    mesh: &MeshState,
    cellless: &std::collections::BTreeSet<usize>,
    whole_cells: usize,
) -> Result<(Vec<usize>, Vec<Option<usize>>), String> {
    let (rings, ids) =
        crate::remap::voronoi_rings_selected(mesh, |site| !cellless.contains(&site))?;
    let cells = mesh.active_vertex_slots().count();
    let (sources, source_ids) = raster.spherical_cells_near(&rings)?;
    let remap = ConservativeRemap::spherical_overlap_partial(
        &sources,
        Some(&source_ids),
        &rings,
        ids,
        whole_cells,
    )?;
    let certificate = remap.certify_spherical_overlap(raster.levels().len(), cells);
    if certificate.negative_weights() + certificate.bad_row_sums() + certificate.bad_lineage_rows()
        != 0
        || certificate.constant_closure_error() > certificate.closure_tolerance()
        || certificate.global_area_closure_error() > certificate.closure_tolerance()
    {
        return Err(format!(
            "raster-to-Voronoi overlap remap failed certification: negative={}, bad_rows={}, bad_lineage={}, constant_error={}, area_error={}, tolerance={}",
            certificate.negative_weights(),
            certificate.bad_row_sums(),
            certificate.bad_lineage_rows(),
            certificate.constant_closure_error(),
            certificate.global_area_closure_error(),
            certificate.closure_tolerance(),
        ));
    }
    maximum_overlapping_levels(&remap, raster.levels(), cells)
}

pub fn certify_final_cell_requirements_from_raster_global_bound(
    source_levels: &RasterLevelField,
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    max_adjacent_level_delta: usize,
) -> Result<FinalCellRequirementCertificate, FinalCellRequirementError> {
    target_levels
        .validate_for(target_mesh)
        .map_err(FinalCellRequirementError::InvalidInput)?;
    let (source, required) = source_levels
        .levels()
        .iter()
        .copied()
        .enumerate()
        .max_by_key(|&(source, level)| (level, std::cmp::Reverse(source)))
        .ok_or_else(|| {
            FinalCellRequirementError::InvalidInput("empty raster level field".into())
        })?;
    let target_cells = target_levels.levels().len();
    let report = final_report_from_required_levels(
        target_mesh,
        target_levels,
        vec![required; target_cells],
        vec![Some(source); target_cells],
        None,
        max_adjacent_level_delta,
    );
    if report.physical_residuals == 0 && report.balance_residuals == 0 {
        Ok(report)
    } else {
        Err(FinalCellRequirementError::Residuals(report))
    }
}

fn final_cell_requirement_report(
    source_mesh: &MeshState,
    source_levels: &SourceLevelField,
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    max_adjacent_level_delta: usize,
    remap: &ConservativeRemap,
) -> Result<FinalCellRequirementReport, String> {
    source_levels.validate_for(source_mesh)?;
    target_levels.validate_for(target_mesh)?;
    remap.validate_mesh_binding(source_mesh, target_mesh)?;
    let source_sites = source_levels.active_sites();
    let target_sites = target_levels.active_sites();
    let remap_cert = remap.certify_spherical_overlap(source_sites.len(), target_sites.len());
    if remap_cert.negative_weights() + remap_cert.bad_row_sums() + remap_cert.bad_lineage_rows()
        != 0
        || remap_cert.constant_closure_error() > remap_cert.closure_tolerance()
        || remap_cert.global_area_closure_error() > remap_cert.closure_tolerance()
    {
        return Err(format!(
            "Voronoi overlap remap failed certification: negative={}, bad_rows={}, bad_lineage={}, constant_error={}, area_error={}, tolerance={}",
            remap_cert.negative_weights(),
            remap_cert.bad_row_sums(),
            remap_cert.bad_lineage_rows(),
            remap_cert.constant_closure_error(),
            remap_cert.global_area_closure_error(),
            remap_cert.closure_tolerance(),
        ));
    }

    let (required_levels, source_for_target) =
        maximum_overlapping_levels(remap, source_levels.levels(), target_sites.len())?;
    Ok(final_report_from_required_levels(
        target_mesh,
        target_levels,
        required_levels,
        source_for_target,
        Some(source_sites),
        max_adjacent_level_delta,
    ))
}

fn maximum_overlapping_levels(
    remap: &ConservativeRemap,
    source_levels: &[usize],
    target_cells: usize,
) -> Result<(Vec<usize>, Vec<Option<usize>>), String> {
    let row_maxima = remap
        .rows()
        .par_iter()
        .map(|row| {
            if row.target >= target_cells {
                return Err("remap target row is outside target level field");
            }
            let (required, source_for_row) = row_required_level(row, source_levels)?;
            Ok((row.target, required, source_for_row))
        })
        .collect::<Vec<_>>();

    let mut required_levels = vec![0; target_cells];
    let mut source_for_target = vec![None; target_cells];
    for row in row_maxima {
        let (target, required, source) = row.map_err(str::to_owned)?;
        if required > required_levels[target] {
            required_levels[target] = required;
            source_for_target[target] = source;
        }
    }
    Ok((required_levels, source_for_target))
}

/// The level a remap row's target cell requires: the highest of the
/// sources it overlaps, and the first source with it.
pub(crate) fn row_required_level(
    row: &crate::remap::RemapRow,
    source_levels: &[usize],
) -> Result<(usize, Option<usize>), &'static str> {
    let mut required = 0;
    let mut source_for_row = None;
    for &(source, weight) in &row.sources {
        if weight <= 0.0 {
            continue;
        }
        let level = *source_levels
            .get(source)
            .ok_or("remap source row is outside source level field")?;
        if level > required {
            required = level;
            source_for_row = Some(source);
        }
    }
    Ok((required, source_for_row))
}

fn final_report_from_required_levels(
    target_mesh: &MeshState,
    target_levels: &TargetLevelField,
    required_levels: Vec<usize>,
    source_for_target: Vec<Option<usize>>,
    source_sites: Option<&[usize]>,
    max_adjacent_level_delta: usize,
) -> FinalCellRequirementReport {
    let target_sites = target_levels.active_sites();
    let mut witnesses = Vec::new();
    for (target, (&required, &delivered)) in required_levels
        .iter()
        .zip(target_levels.levels())
        .enumerate()
    {
        if delivered < required {
            let source = source_for_target[target];
            witnesses.push(RequirementWitness {
                kind: "physical",
                target_cell: target,
                target_site: target_sites[target],
                source_cell: source,
                source_site: source.and_then(|source| source_sites?.get(source).copied()),
                neighbour_cell: None,
                neighbour_site: None,
                required_level: required,
                delivered_level: delivered,
            });
        }
    }
    let physical_residuals = witnesses.len();

    let mut site_to_target = vec![usize::MAX; target_mesh.vertices().len()];
    for (cell, &site) in target_sites.iter().enumerate() {
        site_to_target[site] = cell;
    }
    let mut balance_residuals = 0;
    for (left_site, right_site) in target_site_edges(target_mesh) {
        let Some(&left) = site_to_target
            .get(left_site)
            .filter(|&&cell| cell != usize::MAX)
        else {
            continue;
        };
        let Some(&right) = site_to_target
            .get(right_site)
            .filter(|&&cell| cell != usize::MAX)
        else {
            continue;
        };
        let dl = target_levels.levels()[left];
        let dr = target_levels.levels()[right];
        if dl.abs_diff(dr) > max_adjacent_level_delta {
            balance_residuals += 1;
            witnesses.push(RequirementWitness {
                kind: "balance",
                target_cell: left,
                target_site: left_site,
                source_cell: None,
                source_site: None,
                neighbour_cell: Some(right),
                neighbour_site: Some(right_site),
                required_level: dl.min(dr) + max_adjacent_level_delta,
                delivered_level: dl.max(dr),
            });
        }
    }

    FinalCellRequirementReport {
        target_fingerprint: mesh_fingerprint(target_mesh),
        target_cells: target_sites.len(),
        physical_residuals,
        balance_residuals,
        required_levels,
        witnesses,
    }
}

pub fn target_site_edges(mesh: &MeshState) -> Vec<(usize, usize)> {
    let mut edges = Vec::with_capacity(mesh.triangle_count().saturating_mul(3).div_ceil(2));
    for face in mesh.active_triangle_slots() {
        let [a, b, c] = mesh.triangles()[face];
        for (corner, u, v) in [(2, a, b), (0, b, c), (1, c, a)] {
            let neighbour = mesh.neighbours()[face][corner];
            if neighbour == 0 || face < neighbour {
                edges.push(if u < v { (u, v) } else { (v, u) });
            }
        }
    }
    edges
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementSource {
    pub vertex: usize,
    pub level: usize,
}

pub fn merge_sources(vertex_count: usize, sources: &[RequirementSource]) -> Vec<usize> {
    let mut levels = vec![0; vertex_count];
    for source in sources {
        if source.vertex < vertex_count {
            levels[source.vertex] = levels[source.vertex].max(source.level);
        }
    }
    levels
}

pub fn graded_envelope(
    adjacency: &[Vec<usize>],
    required: &[usize],
    ring_width: usize,
) -> Vec<usize> {
    let width = ring_width.max(1);
    let mut score = required
        .iter()
        .map(|level| level.saturating_mul(width))
        .collect::<Vec<_>>();
    let mut queue = score
        .iter()
        .copied()
        .enumerate()
        .filter(|&(_, value)| value > 0)
        .map(|(vertex, value)| (value, vertex))
        .collect::<BinaryHeap<_>>();
    while let Some((value, vertex)) = queue.pop() {
        if score[vertex] != value || value <= 1 {
            continue;
        }
        let propagated = value - 1;
        for &neighbour in adjacency.get(vertex).into_iter().flatten() {
            if neighbour < score.len() && propagated > score[neighbour] {
                score[neighbour] = propagated;
                queue.push((propagated, neighbour));
            }
        }
    }
    score
        .into_iter()
        .map(|value| value.div_ceil(width))
        .collect()
}

pub fn one_ring_adjacency(triangles: &[[usize; 3]], vertex_count: usize) -> Vec<Vec<usize>> {
    let mut adjacency = vec![Vec::new(); vertex_count];
    for &[a, b, c] in triangles.iter().skip(2) {
        for (u, v) in [(a, b), (b, c), (c, a)] {
            if u < vertex_count && v < vertex_count {
                if !adjacency[u].contains(&v) {
                    adjacency[u].push(v);
                }
                if !adjacency[v].contains(&u) {
                    adjacency[v].push(u);
                }
            }
        }
    }
    for row in &mut adjacency {
        row.sort_unstable();
    }
    adjacency
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::{analytic_counts, MotherGrid};
    use std::collections::BTreeSet;

    #[test]
    fn target_site_edges_are_unique_and_complete() {
        let grid = MotherGrid::generate(2).unwrap();
        let edges = target_site_edges(&grid.mesh);
        assert_eq!(edges.len(), analytic_counts(2).unwrap().1);
        assert_eq!(
            edges.iter().copied().collect::<BTreeSet<_>>().len(),
            edges.len()
        );
        assert!(edges.iter().all(|&(left, right)| {
            left < right && grid.mesh.is_vertex_live(left) && grid.mesh.is_vertex_live(right)
        }));
    }

    #[test]
    fn merge_is_max_and_order_invariant() {
        let a = vec![
            RequirementSource {
                vertex: 1,
                level: 2,
            },
            RequirementSource {
                vertex: 1,
                level: 5,
            },
            RequirementSource {
                vertex: 3,
                level: 4,
            },
        ];
        let mut b = a.clone();
        b.reverse();
        assert_eq!(merge_sources(5, &a), vec![0, 5, 0, 4, 0]);
        assert_eq!(merge_sources(5, &a), merge_sources(5, &b));
    }

    #[test]
    fn maximum_overlapping_levels_keeps_serial_ties_and_errors() {
        let remap = ConservativeRemap::from_rows_for_test(vec![
            crate::remap::RemapRow {
                target: 1,
                sources: vec![(0, 0.0), (2, 1.0), (1, 1.0)],
            },
            crate::remap::RemapRow {
                target: 1,
                sources: vec![(3, 1.0)],
            },
            crate::remap::RemapRow {
                target: 0,
                sources: vec![(1, -1.0), (0, 1.0)],
            },
        ]);
        let (levels, sources) = maximum_overlapping_levels(&remap, &[2, 7, 7, 5], 2).unwrap();
        assert_eq!(levels, vec![2, 7]);
        assert_eq!(sources, vec![Some(0), Some(2)]);

        let bad = ConservativeRemap::from_rows_for_test(vec![
            crate::remap::RemapRow {
                target: 1,
                sources: vec![(99, 1.0)],
            },
            crate::remap::RemapRow {
                target: 99,
                sources: vec![],
            },
        ]);
        assert_eq!(
            maximum_overlapping_levels(&bad, &[0], 2).unwrap_err(),
            "remap source row is outside source level field"
        );
    }

    #[test]
    fn graded_envelope_bridges_close_sources() {
        let adjacency = vec![vec![1], vec![0, 2], vec![1, 3], vec![2, 4], vec![3]];
        let required = merge_sources(
            5,
            &[
                RequirementSource {
                    vertex: 0,
                    level: 4,
                },
                RequirementSource {
                    vertex: 4,
                    level: 4,
                },
            ],
        );
        assert_eq!(
            graded_envelope(&adjacency, &required, 1),
            vec![4, 3, 2, 3, 4]
        );
        assert_eq!(
            graded_envelope(&adjacency, &required, 2),
            vec![4, 4, 3, 4, 4]
        );
    }

    /// Projected from the raster cells near a region, the region's levels
    /// are those projected from every raster cell, float for float -- at the
    /// poles and across the dateline too -- and far fewer cells are used.
    #[test]
    fn a_region_projects_from_the_nearby_raster_cells_alone() {
        use crate::mother_grid::lattice::{faces_around, locate};
        let n = 8;
        let (nlon, nlat) = (72, 36);
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let levels = (0..nlon * nlat)
            .map(|_| {
                if next() % 5 == 0 {
                    1 + (next() % 3) as usize
                } else {
                    0
                }
            })
            .collect::<Vec<_>>();
        let raster = RasterLevelField::new(nlon, nlat, levels).unwrap();
        let unit = |lon: f64, lat: f64| {
            let (lon, lat) = (lon.to_radians(), lat.to_radians());
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        };
        for (lon, lat) in [
            (0.0, 90.0),
            (10.0, -88.0),
            (180.0, 0.0),
            (-179.0, 40.0),
            (101.0, 25.0),
        ] {
            let mut built = BTreeSet::from([locate(n, unit(lon, lat)).unwrap()]);
            for _ in 0..2 {
                for face in built.clone() {
                    built.extend(faces_around(face).unwrap());
                }
            }
            let region = MotherGrid::generate_faces(n, built).unwrap();
            let outer = region.region.as_ref().unwrap().outer_boundary().clone();
            let whole_cells = 10 * n * n + 2;
            let near =
                region_required_levels_with_sources(&raster, &region.mesh, &outer, whole_cells)
                    .unwrap();
            let (rings, ids) =
                crate::remap::voronoi_rings_selected(&region.mesh, |site| !outer.contains(&site))
                    .unwrap();
            let remap = ConservativeRemap::spherical_overlap_partial(
                &raster.spherical_cells(),
                None,
                &rings,
                ids,
                whole_cells,
            )
            .unwrap();
            let cells = region.mesh.active_vertex_slots().count();
            let every = maximum_overlapping_levels(&remap, raster.levels(), cells).unwrap();
            assert_eq!(near, every, "region at ({lon}, {lat})");
            assert!(
                near.0.iter().any(|&level| level > 0),
                "({lon}, {lat}) sees nothing"
            );
            let (used, _) = raster.spherical_cells_near(&rings).unwrap();
            assert!(
                used.len() < nlon * nlat / 4,
                "({lon}, {lat}) used {}",
                used.len()
            );
        }
    }
}
