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
        let mut cells = Vec::with_capacity(self.levels.len());
        for j in 0..self.nlat {
            let south = -90.0 + j as f64 * dlat;
            let north = south + dlat;
            for i in 0..self.nlon {
                let west = -180.0 + i as f64 * dlon;
                let east = west + dlon;
                let cell = if j == 0 {
                    vec![(0.0, -90.0), (east, north), (west, north)]
                } else if j + 1 == self.nlat {
                    vec![(west, south), (east, south), (0.0, 90.0)]
                } else {
                    vec![(west, south), (east, south), (east, north), (west, north)]
                };
                cells.push(cell);
            }
        }
        cells
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
        ConservativeRemap::identity(target_mesh.vertex_count())
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
}
