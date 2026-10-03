use crate::fingerprint::mesh_fingerprint;
use crate::mother_grid::{MotherGrid, TriangleAddress};
use earthmesh_boundary::SphericalCap;
use earthmesh_geometry::{Point, PreparedSphericalPolygon};
use earthmesh_mesh::{spherical_triangle_area_unit, MeshState};
use rayon::prelude::*;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Debug, Clone, PartialEq)]
pub struct RemapRow {
    pub target: usize,
    pub sources: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConservativeRemap {
    rows: Vec<RemapRow>,
    coverage_error: f64,
    source_fingerprint: Option<u64>,
    target_fingerprint: Option<u64>,
    /// Set when rows cover only some target cells -- those of a built
    /// region certified cell by cell.
    covered_targets: Option<PartialCoverage>,
}

/// Rows of a remap over a built region: the target cells they cover, in row
/// order, and the cell count of the whole meshes the region is part of. The
/// tolerances scale with that count, so a row passes or fails exactly as it
/// would among all the rows of the whole sphere.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialCoverage {
    targets: Vec<usize>,
    whole_cells: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RemapCertificate {
    rows: usize,
    negative_weights: usize,
    bad_row_sums: usize,
    bad_lineage_rows: usize,
    constant_closure_error: f64,
    global_area_closure_error: f64,
    closure_tolerance: f64,
    target_fingerprint: Option<u64>,
}

impl RemapCertificate {
    pub fn rows(&self) -> usize {
        self.rows
    }
    pub fn negative_weights(&self) -> usize {
        self.negative_weights
    }
    pub fn bad_row_sums(&self) -> usize {
        self.bad_row_sums
    }
    pub fn bad_lineage_rows(&self) -> usize {
        self.bad_lineage_rows
    }
    pub fn constant_closure_error(&self) -> f64 {
        self.constant_closure_error
    }
    pub fn global_area_closure_error(&self) -> f64 {
        self.global_area_closure_error
    }
    pub fn closure_tolerance(&self) -> f64 {
        self.closure_tolerance
    }
    pub(crate) fn target_fingerprint(&self) -> Option<u64> {
        self.target_fingerprint
    }
}

impl ConservativeRemap {
    pub fn rows(&self) -> &[RemapRow] {
        &self.rows
    }

    #[cfg(test)]
    pub(crate) fn from_rows_for_test(rows: Vec<RemapRow>) -> Self {
        Self {
            rows,
            coverage_error: 0.0,
            source_fingerprint: None,
            target_fingerprint: None,
            covered_targets: None,
        }
    }

    pub fn identity(cell_count: usize) -> Self {
        Self {
            rows: (0..cell_count)
                .map(|cell| RemapRow {
                    target: cell,
                    sources: vec![(cell, 1.0)],
                })
                .collect(),
            coverage_error: 0.0,
            source_fingerprint: None,
            target_fingerprint: None,
            covered_targets: None,
        }
    }

    pub fn identity_for_mesh(mesh: &MeshState) -> Self {
        let fingerprint = mesh_fingerprint(mesh);
        let mut remap = Self::identity(mesh.active_vertex_slots().count());
        remap.source_fingerprint = Some(fingerprint);
        remap.target_fingerprint = Some(fingerprint);
        remap
    }

    /// The identity on some of one mesh's cells, numbered as its active sites
    /// are: each keeps itself, whole. What a built region certifies when no
    /// component committed -- no level asked for, or a closed region whose
    /// demand coarsens nothing -- its outer boundary sites having no cell.
    pub fn identity_on(mesh: &MeshState, cells: impl IntoIterator<Item = usize>) -> Self {
        let fingerprint = mesh_fingerprint(mesh);
        let rows = cells
            .into_iter()
            .map(|cell| RemapRow {
                target: cell,
                sources: vec![(cell, 1.0)],
            })
            .collect::<Vec<_>>();
        let targets = rows.iter().map(|row| row.target).collect();
        Self {
            rows,
            coverage_error: 0.0,
            source_fingerprint: Some(fingerprint),
            target_fingerprint: Some(fingerprint),
            covered_targets: Some(PartialCoverage {
                targets,
                whole_cells: mesh.active_vertex_slots().count(),
            }),
        }
    }

    pub(crate) fn validate_mesh_binding(
        &self,
        source: &MeshState,
        target: &MeshState,
    ) -> Result<(), String> {
        if self.source_fingerprint != Some(mesh_fingerprint(source)) {
            return Err("Voronoi overlap remap source mesh is unbound or stale".into());
        }
        if self.target_fingerprint != Some(mesh_fingerprint(target)) {
            return Err("Voronoi overlap remap target mesh is unbound or stale".into());
        }
        Ok(())
    }

    pub fn hierarchy_2_to_1_average(coarse: &MotherGrid, fine: &MotherGrid) -> Option<Self> {
        if fine.subdivision != coarse.subdivision * 2 {
            return None;
        }
        let coarse_faces = active_faces(coarse)?;
        let fine_faces = active_faces(fine)?;
        let coarse_by_address = coarse_faces
            .iter()
            .enumerate()
            .map(|(target, (_, address, _))| (*address, target))
            .collect::<BTreeMap<_, _>>();
        let mut children = vec![Vec::new(); coarse_faces.len()];
        for (source, (_, address, area)) in fine_faces.iter().enumerate() {
            let parent = address.parent_2_to_1()?;
            children[*coarse_by_address.get(&parent)?].push((source, *area));
        }
        let mut rows = Vec::with_capacity(coarse_faces.len());
        for (target, child_faces) in children.into_iter().enumerate() {
            if child_faces.len() != 4 {
                return None;
            }
            let covered_area = child_faces.iter().map(|(_, area)| area).sum::<f64>();
            if !covered_area.is_finite() || covered_area <= 0.0 {
                return None;
            }
            rows.push(RemapRow {
                target,
                sources: child_faces
                    .into_iter()
                    .map(|(source, area)| (source, area / covered_area))
                    .collect(),
            });
        }
        Some(Self {
            rows,
            coverage_error: 0.0,
            source_fingerprint: Some(mesh_fingerprint(&fine.mesh)),
            target_fingerprint: Some(mesh_fingerprint(&coarse.mesh)),
            covered_targets: None,
        })
    }

    /// Whether this remap was certified from `source` to `target`, by their
    /// fingerprints: a remap handed on is used only for the meshes it was
    /// computed between.
    pub fn joins(&self, source: &MeshState, target: &MeshState) -> bool {
        self.source_fingerprint == Some(mesh_fingerprint(source))
            && self.target_fingerprint == Some(mesh_fingerprint(target))
    }

    pub fn spherical_overlap(
        source_cells: &[Vec<(f64, f64)>],
        target_cells: &[Vec<(f64, f64)>],
    ) -> Result<Self, String> {
        if source_cells.is_empty() || target_cells.is_empty() {
            return Err("spherical remap needs non-empty source and target cells".into());
        }
        let sources = prepare_cells(source_cells)?;
        let targets = prepare_cells(target_cells)?;
        let (source_rings, sources): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
        let index = SphericalCapIndex::new(&source_rings)?;
        drop(source_rings);
        Self::overlap_prepared(
            source_cells,
            &sources,
            None,
            &index,
            target_cells,
            targets,
            None,
        )
    }

    /// Rows for `targets` against `sources`. Without ids, cells are numbered
    /// by position; with them, `source_ids` / `target_ids` give each prepared
    /// cell its number among the meshes' cells (ascending, so rows keep the
    /// order they have among all cells).
    fn overlap_prepared(
        source_cells: &[Vec<(f64, f64)>],
        sources: &[PreparedSphericalPolygon],
        source_ids: Option<&[usize]>,
        index: &SphericalCapIndex,
        target_cells: &[Vec<(f64, f64)>],
        targets: Vec<(Vec<Point>, PreparedSphericalPolygon)>,
        target_ids: Option<&[usize]>,
    ) -> Result<Self, String> {
        let rows = targets
            // Consume each target after its row: its lazily prepared clipping
            // planes need not accumulate across the entire global mesh.
            .into_par_iter()
            .enumerate()
            .map(|(target, (target_ring, polygon))| {
                let target_cap = SphericalCap::for_rings(std::slice::from_ref(&target_ring))
                    .ok_or_else(|| format!("target cell {target} has no spherical cap"))?;
                let mut overlaps = Vec::new();
                for source in index.candidates(target_cap) {
                    if !target_cap.overlaps(index.caps[source]) {
                        continue;
                    }
                    let fraction = polygon.overlap_fraction(&sources[source])
                        .map_err(|error| {
                            format!(
                                "source {source} {:?} and target {target} {:?} overlap failed: {error}",
                                source_cells[source], target_cells[target]
                            )
                        })?;
                    if fraction > 1.0e-14 {
                        overlaps.push((source_ids.map_or(source, |ids| ids[source]), fraction));
                    }
                }
                let target = target_ids.map_or(target, |ids| ids[target]);
                let covered = compensated_sum(overlaps.iter().map(|(_, weight)| *weight));
                if !covered.is_finite() || covered <= 0.0 {
                    return Err(format!("target cell {target} has no source overlap"));
                }
                for (_, weight) in &mut overlaps {
                    *weight /= covered;
                }
                Ok((
                    RemapRow {
                        target,
                        sources: overlaps,
                    },
                    (covered - 1.0).abs(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let coverage_error = rows.iter().map(|(_, error)| *error).fold(0.0_f64, f64::max);
        let rows = rows.into_iter().map(|(row, _)| row).collect();
        Ok(Self {
            rows,
            coverage_error,
            source_fingerprint: None,
            target_fingerprint: None,
            covered_targets: None,
        })
    }

    /// `spherical_overlap` with rows for some target cells only: `target_ids`
    /// gives each ring's number among all target cells, and `whole_cells` the
    /// cell count the tolerances scale with (see `PartialCoverage`).
    /// `source_ids`, when the sources are some of a field's cells, gives each
    /// one's number in the field (ascending).
    pub(crate) fn spherical_overlap_partial(
        source_cells: &[Vec<(f64, f64)>],
        source_ids: Option<&[usize]>,
        target_cells: &[Vec<(f64, f64)>],
        target_ids: Vec<usize>,
        whole_cells: usize,
    ) -> Result<Self, String> {
        if source_cells.is_empty() || target_cells.is_empty() {
            return Err("spherical remap needs non-empty source and target cells".into());
        }
        let sources = prepare_cells(source_cells)?;
        let targets = prepare_cells(target_cells)?;
        let (source_rings, sources): (Vec<_>, Vec<_>) = sources.into_iter().unzip();
        let index = SphericalCapIndex::new(&source_rings)?;
        drop(source_rings);
        let mut remap = Self::overlap_prepared(
            source_cells,
            &sources,
            source_ids,
            &index,
            target_cells,
            targets,
            Some(&target_ids),
        )?;
        remap.covered_targets = Some(PartialCoverage {
            targets: target_ids,
            whole_cells,
        });
        Ok(remap)
    }

    pub fn between_voronoi_meshes(source: &MeshState, target: &MeshState) -> Result<Self, String> {
        let mut remap = Self::spherical_overlap(&voronoi_rings(source)?, &voronoi_rings(target)?)?;
        remap.source_fingerprint = Some(mesh_fingerprint(source));
        remap.target_fingerprint = Some(mesh_fingerprint(target));
        Ok(remap)
    }

    pub fn certify_identity(&self, cell_count: usize) -> RemapCertificate {
        let mut certificate = self.certify_with_lineage(|row| {
            row.target < cell_count && row.sources == vec![(row.target, 1.0)]
        });
        if self.rows.len() != cell_count
            || self
                .rows
                .iter()
                .enumerate()
                .any(|(target, row)| row.target != target)
        {
            certificate.bad_lineage_rows += 1;
        }
        certificate
    }

    pub fn certify_spherical_overlap(
        &self,
        source_cells: usize,
        target_cells: usize,
    ) -> RemapCertificate {
        let mut certificate = self.certify_with_lineage(|row| {
            row.target < target_cells
                && !row.sources.is_empty()
                && row.sources.iter().all(|&(source, _)| source < source_cells)
        });
        let lineage_broken = match &self.covered_targets {
            None => {
                self.rows.len() != target_cells
                    || self
                        .rows
                        .iter()
                        .enumerate()
                        .any(|(target, row)| row.target != target)
            }
            Some(partial) => {
                self.rows.len() != partial.targets.len()
                    || self
                        .rows
                        .iter()
                        .zip(&partial.targets)
                        .any(|(row, &target)| row.target != target)
                    || partial.targets.windows(2).any(|pair| pair[0] >= pair[1])
            }
        };
        if lineage_broken {
            certificate.bad_lineage_rows += 1;
        }
        let cells = source_cells.max(target_cells).max(
            self.covered_targets
                .as_ref()
                .map_or(0, |partial| partial.whole_cells),
        );
        certificate.closure_tolerance = certificate
            .closure_tolerance
            .max(128.0 * f64::EPSILON * cells as f64);
        certificate.global_area_closure_error = self.coverage_error;
        certificate
    }

    /// This remap with its cells renumbered -- a built region's remap in the
    /// whole sphere's numbering: `source_id` and `target_id` map the region's
    /// cell numbers to the whole meshes', `whole_cells` scales the
    /// tolerances, and `target_fingerprint` binds the whole target mesh. Rows
    /// keep their order, so the target numbering must preserve it.
    pub fn renumbered(
        &self,
        source_id: impl Fn(usize) -> usize,
        target_id: impl Fn(usize) -> usize,
        whole_cells: usize,
        target_fingerprint: u64,
    ) -> Self {
        let rows = self
            .rows
            .iter()
            .map(|row| RemapRow {
                target: target_id(row.target),
                sources: row
                    .sources
                    .iter()
                    .map(|&(source, weight)| (source_id(source), weight))
                    .collect(),
            })
            .collect::<Vec<_>>();
        let targets = rows.iter().map(|row| row.target).collect();
        Self {
            rows,
            coverage_error: self.coverage_error,
            source_fingerprint: None,
            target_fingerprint: Some(target_fingerprint),
            covered_targets: Some(PartialCoverage {
                targets,
                whole_cells,
            }),
        }
    }

    /// The target cells the rows cover, when they cover only a region.
    pub fn covered_targets(&self) -> Option<&[usize]> {
        self.covered_targets
            .as_ref()
            .map(|partial| partial.targets.as_slice())
    }

    pub fn certify_hierarchy_2_to_1_average(
        &self,
        coarse: &MotherGrid,
        fine: &MotherGrid,
    ) -> RemapCertificate {
        let coarse_faces = active_faces(coarse).unwrap_or_default();
        let fine_faces = active_faces(fine).unwrap_or_default();
        let coarse_by_address = coarse_faces
            .iter()
            .enumerate()
            .map(|(target, (_, address, _))| (*address, target))
            .collect::<BTreeMap<_, _>>();
        let mut certificate = self.certify_with_lineage(|row| {
            row.target < coarse_faces.len()
                && row.sources.len() == 4
                && row.sources.iter().all(|&(source, _)| {
                    fine_faces
                        .get(source)
                        .and_then(|(_, address, _)| address.parent_2_to_1())
                        .and_then(|parent| coarse_by_address.get(&parent).copied())
                        == Some(row.target)
                })
        });
        let coarse_area = coarse_faces.iter().map(|(_, _, area)| area).sum::<f64>();
        let fine_area = fine_faces.iter().map(|(_, _, area)| area).sum::<f64>();
        certificate.global_area_closure_error = (coarse_area - fine_area).abs();
        certificate
    }

    fn certify_with_lineage(
        &self,
        valid_lineage: impl Fn(&RemapRow) -> bool + Sync,
    ) -> RemapCertificate {
        let rows_scale = self
            .covered_targets
            .as_ref()
            .map_or(self.rows.len(), |partial| {
                partial.whole_cells.max(self.rows.len())
            });
        let closure_tolerance = (128.0
            * f64::EPSILON
            * rows_scale.max(
                self.rows
                    .par_iter()
                    .map(|row| row.sources.len())
                    .max()
                    .unwrap_or(1),
            ) as f64)
            .max(1.0e-11);
        let stats = self
            .rows
            .par_iter()
            .map(|row| {
                let mut negative_weights = 0;
                let sum: f64 = row
                    .sources
                    .iter()
                    .map(|&(_, weight)| {
                        if weight < 0.0 {
                            negative_weights += 1;
                        }
                        weight
                    })
                    .sum();
                let error = (sum - 1.0).abs();
                (
                    negative_weights,
                    usize::from(error > closure_tolerance),
                    usize::from(!valid_lineage(row)),
                    error,
                )
            })
            .reduce(
                || (0, 0, 0, 0.0_f64),
                |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2, a.3.max(b.3)),
            );
        RemapCertificate {
            rows: self.rows.len(),
            negative_weights: stats.0,
            bad_row_sums: stats.1,
            bad_lineage_rows: stats.2,
            constant_closure_error: stats.3,
            global_area_closure_error: 0.0,
            closure_tolerance,
            target_fingerprint: self.target_fingerprint,
        }
    }
}

fn compensated_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let mut sum = 0.0;
    let mut correction = 0.0;
    for value in values {
        let adjusted = value - correction;
        let next = sum + adjusted;
        correction = (next - sum) - adjusted;
        sum = next;
    }
    sum
}

// One immutable source per scheduler invocation, deliberately outside cloned
// transaction state. The borrow prevents stale geometry; targets are never cached.
pub(crate) struct VoronoiRemapSource<'a> {
    mesh: &'a MeshState,
    /// Sites of a built region's edge: open fans, no cell.
    cellless: Option<&'a std::collections::BTreeSet<usize>>,
    prepared: OnceLock<Result<PreparedRemapSource, String>>,
}

struct PreparedRemapSource {
    cells: Vec<Vec<(f64, f64)>>,
    /// Each cell's number among all the mesh's cells, when some have none.
    ids: Option<Vec<usize>>,
    polygons: Vec<PreparedSphericalPolygon>,
    index: SphericalCapIndex,
}

impl<'a> VoronoiRemapSource<'a> {
    pub(crate) fn new(mesh: &'a MeshState) -> Self {
        Self {
            mesh,
            cellless: None,
            prepared: OnceLock::new(),
        }
    }

    /// The source cells of a built region: every site but those on its edge,
    /// numbered as among all the region's sites.
    pub(crate) fn new_region(
        mesh: &'a MeshState,
        cellless: &'a std::collections::BTreeSet<usize>,
    ) -> Self {
        Self {
            mesh,
            cellless: Some(cellless),
            prepared: OnceLock::new(),
        }
    }

    fn prepared(&self) -> Result<&PreparedRemapSource, String> {
        self.prepared
            .get_or_init(|| {
                let (cells, ids) = match self.cellless {
                    None => (voronoi_rings(self.mesh)?, None),
                    Some(cellless) => {
                        let (cells, ids) =
                            voronoi_rings_selected(self.mesh, |site| !cellless.contains(&site))?;
                        (cells, Some(ids))
                    }
                };
                if cells.is_empty() {
                    return Err("spherical remap needs non-empty source and target cells".into());
                }
                let (rings, polygons): (Vec<_>, Vec<_>) =
                    prepare_cells(&cells)?.into_iter().unzip();
                let index = SphericalCapIndex::new(&rings)?;
                Ok(PreparedRemapSource {
                    cells,
                    ids,
                    polygons,
                    index,
                })
            })
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(crate) fn remap_to(&self, target: &MeshState) -> Result<ConservativeRemap, String> {
        let source = self.prepared()?;
        let target_cells = voronoi_rings(target)?;
        if target_cells.is_empty() {
            return Err("spherical remap needs non-empty source and target cells".into());
        }
        let targets = prepare_cells(&target_cells)?;
        let mut remap = ConservativeRemap::overlap_prepared(
            &source.cells,
            &source.polygons,
            source.ids.as_deref(),
            &source.index,
            &target_cells,
            targets,
            None,
        )?;
        remap.source_fingerprint = Some(mesh_fingerprint(self.mesh));
        remap.target_fingerprint = Some(mesh_fingerprint(target));
        Ok(remap)
    }

    /// Rows for the target sites `covered` picks -- the cells of a built
    /// region certified cell by cell -- numbered as among all the target's
    /// cells. `whole_cells` is the cell count of the whole meshes the region
    /// belongs to, which the certificate's tolerances scale with.
    pub(crate) fn remap_region_to(
        &self,
        target: &MeshState,
        covered: impl Fn(usize) -> bool + Sync,
        whole_cells: usize,
    ) -> Result<ConservativeRemap, String> {
        let source = self.prepared()?;
        let (target_cells, target_ids) = voronoi_rings_selected(target, covered)?;
        if target_cells.is_empty() {
            return Err("spherical remap needs non-empty source and target cells".into());
        }
        let targets = prepare_cells(&target_cells)?;
        let mut remap = ConservativeRemap::overlap_prepared(
            &source.cells,
            &source.polygons,
            source.ids.as_deref(),
            &source.index,
            &target_cells,
            targets,
            Some(&target_ids),
        )?;
        remap.source_fingerprint = Some(mesh_fingerprint(self.mesh));
        remap.target_fingerprint = Some(mesh_fingerprint(target));
        remap.covered_targets = Some(PartialCoverage {
            targets: target_ids,
            whole_cells,
        });
        Ok(remap)
    }
}

fn prepare_cells(
    cells: &[Vec<(f64, f64)>],
) -> Result<Vec<(Vec<Point>, PreparedSphericalPolygon)>, String> {
    cells
        .iter()
        .enumerate()
        .map(|(cell, ring)| {
            let points = ring
                .iter()
                .map(|&(lon, lat)| Point::new(lon, lat))
                .collect::<Vec<_>>();
            let polygon = PreparedSphericalPolygon::new(&points)
                .map_err(|error| format!("invalid spherical cell {cell}: {error}"))?;
            Ok((points, polygon))
        })
        .collect()
}

pub(crate) fn voronoi_rings(mesh: &MeshState) -> Result<Vec<Vec<(f64, f64)>>, String> {
    Ok(voronoi_rings_selected(mesh, |_| true)?.0)
}

/// Lon-lat rings, one per cell.
pub(crate) type Rings = Vec<Vec<(f64, f64)>>;

/// Voronoi rings of the sites `select` picks, with each one's number among
/// all the mesh's active sites.
pub(crate) fn voronoi_rings_selected(
    mesh: &MeshState,
    select: impl Fn(usize) -> bool + Sync,
) -> Result<(Rings, Vec<usize>), String> {
    let mut seeds = vec![usize::MAX; mesh.vertices().len()];
    for triangle in mesh.active_triangle_slots() {
        for site in mesh.triangles()[triangle] {
            if seeds[site] == usize::MAX {
                seeds[site] = triangle;
            }
        }
    }
    let mut corners = vec![(0.0, 0.0); mesh.triangles().len()];
    corners.par_iter_mut().enumerate().try_for_each(
        |(triangle, corner)| -> Result<(), String> {
            if !mesh.is_triangle_live(triangle) {
                return Ok(());
            }
            let point = mesh.circumcentre(triangle).map_err(|error| {
                format!("Voronoi triangle {triangle} cannot be remapped: {error}")
            })?;
            let radius = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
            if !radius.is_finite() || radius <= 0.0 {
                return Err(format!(
                    "Voronoi triangle {triangle} has a non-finite corner"
                ));
            }
            let point = [point.x / radius, point.y / radius, point.z / radius];
            *corner = (
                point[1].atan2(point[0]).to_degrees(),
                point[2].clamp(-1.0, 1.0).asin().to_degrees(),
            );
            Ok(())
        },
    )?;
    let (ids, sites): (Vec<_>, Vec<_>) = mesh
        .active_vertex_slots()
        .enumerate()
        .filter(|&(_, site)| select(site))
        .unzip();
    let rings = sites
        .into_par_iter()
        .map(|site| {
            let seed = seeds[site];
            if seed == usize::MAX {
                return Err(format!("Voronoi cell {site} is in no triangle"));
            }
            let ring = mesh
                .triangle_fan_from(site, seed)
                .map_err(|error| format!("Voronoi cell {site} cannot be remapped: {error}"))?
                .into_iter()
                .map(|triangle| corners[triangle])
                .collect::<Vec<_>>();
            Ok(ring)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok((rings, ids))
}

pub(crate) struct SphericalCapIndex {
    nlon: usize,
    nlat: usize,
    bins: Vec<Vec<usize>>,
    caps: Vec<SphericalCap>,
}

impl SphericalCapIndex {
    fn new(rings: &[Vec<Point>]) -> Result<Self, String> {
        let caps = rings
            .iter()
            .enumerate()
            .map(|(cell, ring)| {
                SphericalCap::for_rings(std::slice::from_ref(ring))
                    .ok_or_else(|| format!("source cell {cell} has no spherical cap"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_caps(caps))
    }

    pub(crate) fn from_caps(caps: Vec<SphericalCap>) -> Self {
        let nlat = ((caps.len() as f64).sqrt() / 2.0).ceil().clamp(4.0, 2048.0) as usize;
        let nlon = nlat * 2;
        let mut bins = vec![Vec::new(); nlon * nlat];
        for (source, &cap) in caps.iter().enumerate() {
            for bin in cap_bins(cap, nlon, nlat) {
                bins[bin].push(source);
            }
        }
        Self {
            nlon,
            nlat,
            bins,
            caps,
        }
    }

    pub(crate) fn candidates(&self, cap: SphericalCap) -> Vec<usize> {
        let mut candidates = cap_bins(cap, self.nlon, self.nlat)
            .into_iter()
            .flat_map(|bin| self.bins[bin].iter().copied())
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    pub(crate) fn candidates_into(
        &self,
        cap: SphericalCap,
        seen: &mut [u32],
        generation: u32,
        candidates: &mut Vec<usize>,
    ) {
        candidates.clear();
        for source in cap_bins(cap, self.nlon, self.nlat)
            .into_iter()
            .flat_map(|bin| self.bins[bin].iter().copied())
        {
            if seen[source] != generation {
                seen[source] = generation;
                candidates.push(source);
            }
        }
    }
}

fn cap_bins(cap: SphericalCap, nlon: usize, nlat: usize) -> Vec<usize> {
    let (lon, lat) = cap.center_lon_lat_degrees();
    let radius = cap.radius_radians().min(std::f64::consts::PI);
    let radius_degrees = radius.to_degrees();
    let lat_min = (lat - radius_degrees).max(-90.0);
    let lat_max = (lat + radius_degrees).min(90.0);
    let lat_bin = |value: f64| {
        (((value + 90.0) / 180.0) * nlat as f64)
            .floor()
            .clamp(0.0, (nlat - 1) as f64) as usize
    };
    let lon_extent = if radius >= std::f64::consts::FRAC_PI_2 || lat_min <= -90.0 || lat_max >= 90.0
    {
        180.0
    } else {
        (radius.sin() / lat.to_radians().cos().abs())
            .clamp(-1.0, 1.0)
            .asin()
            .abs()
            .to_degrees()
    };
    let lon_bin = |value: f64| {
        ((value.rem_euclid(360.0) / 360.0) * nlon as f64)
            .floor()
            .clamp(0.0, (nlon - 1) as f64) as usize
    };
    let lon_bins = if lon_extent >= 180.0 {
        (0..nlon).collect::<Vec<_>>()
    } else {
        let start = lon_bin(lon - lon_extent);
        let end = lon_bin(lon + lon_extent);
        if start <= end {
            (start..=end).collect()
        } else {
            (start..nlon).chain(0..=end).collect()
        }
    };
    let mut bins = Vec::new();
    for j in lat_bin(lat_min)..=lat_bin(lat_max) {
        bins.extend(lon_bins.iter().map(|&i| j * nlon + i));
    }
    bins
}

fn active_faces(grid: &MotherGrid) -> Option<Vec<(usize, TriangleAddress, f64)>> {
    grid.mesh
        .active_triangle_slots()
        .map(|slot| {
            let address = grid.triangle_addresses.get(slot)?.as_ref().copied()?;
            let [a, b, c] = grid.mesh.triangles()[slot];
            let area = spherical_triangle_area_unit([
                grid.mesh.vertices()[a],
                grid.mesh.vertices()[b],
                grid.mesh.vertices()[c],
            ]);
            (area.is_finite() && area > 0.0).then_some((slot, address, area))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_geometry::{try_spherical_polygon_excess, SphericalAreaBranch};

    // Original remap data flow: prepare neither side across overlap pairs.
    fn scalar_overlap_reference(
        source_cells: &[Vec<(f64, f64)>],
        target_cells: &[Vec<(f64, f64)>],
    ) -> Result<ConservativeRemap, String> {
        let points = |cells: &[Vec<(f64, f64)>]| -> Result<Vec<Vec<Point>>, String> {
            cells
                .iter()
                .enumerate()
                .map(|(cell, ring)| {
                    let points = ring
                        .iter()
                        .map(|&(lon, lat)| Point::new(lon, lat))
                        .collect::<Vec<_>>();
                    try_spherical_polygon_excess(&points, SphericalAreaBranch::Minor)
                        .map_err(|error| format!("invalid spherical cell {cell}: {error}"))?;
                    Ok(points)
                })
                .collect()
        };
        let sources = points(source_cells)?;
        let targets = points(target_cells)?;
        let index = SphericalCapIndex::new(&sources)?;
        let rows = targets
            .par_iter()
            .enumerate()
            .map(|(target, ring)| {
                let cap = SphericalCap::for_rings(std::slice::from_ref(ring)).unwrap();
                let mut overlaps = Vec::new();
                for source in index.candidates(cap) {
                    if !cap.overlaps(index.caps[source]) {
                        continue;
                    }
                    let weight = earthmesh_geometry::spherical_convex_overlap_fraction(
                        ring,
                        &sources[source],
                    )
                    .map_err(|error| error.to_string())?;
                    if weight > 1.0e-14 {
                        overlaps.push((source, weight));
                    }
                }
                let covered = compensated_sum(overlaps.iter().map(|(_, weight)| *weight));
                if !covered.is_finite() || covered <= 0.0 {
                    return Err(format!("target cell {target} has no source overlap"));
                }
                for (_, weight) in &mut overlaps {
                    *weight /= covered;
                }
                Ok((
                    RemapRow {
                        target,
                        sources: overlaps,
                    },
                    (covered - 1.0).abs(),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let coverage_error = rows.iter().map(|(_, error)| *error).fold(0.0_f64, f64::max);
        Ok(ConservativeRemap {
            rows: rows.into_iter().map(|(row, _)| row).collect(),
            coverage_error,
            source_fingerprint: None,
            target_fingerprint: None,
            covered_targets: None,
        })
    }

    #[test]
    fn repeated_voronoi_remaps_preserve_rows_certificates_and_target_binding() {
        let source = MotherGrid::generate(2).unwrap();
        let sources = voronoi_rings(&source.mesh).unwrap();
        let cached = VoronoiRemapSource::new(&source.mesh);
        assert!(cached.prepared.get().is_none());
        for n in [2, 3, 2] {
            let target = MotherGrid::generate(n).unwrap();
            let targets = voronoi_rings(&target.mesh).unwrap();
            let mut expected = scalar_overlap_reference(&sources, &targets).unwrap();
            expected.source_fingerprint = Some(mesh_fingerprint(&source.mesh));
            expected.target_fingerprint = Some(mesh_fingerprint(&target.mesh));
            for threads in [1, 4] {
                let actual = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| cached.remap_to(&target.mesh))
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    actual,
                    ConservativeRemap::between_voronoi_meshes(&source.mesh, &target.mesh).unwrap()
                );
                assert_eq!(
                    actual.certify_spherical_overlap(sources.len(), targets.len()),
                    expected.certify_spherical_overlap(sources.len(), targets.len())
                );
            }
        }
    }

    #[test]
    fn cached_voronoi_source_preserves_errors_and_rebuilds_moved_targets() {
        let source = MotherGrid::generate(2).unwrap();
        let mut vertices = source.mesh.vertices().to_vec();
        vertices.push(vertices[0]); // One orphan site gives a deterministic input error.
        let invalid = MeshState::from_parts(vertices, source.mesh.triangles().to_vec()).unwrap();
        let invalid_source = VoronoiRemapSource::new(&invalid);
        let cached = VoronoiRemapSource::new(&source.mesh);
        for _ in 0..2 {
            assert_eq!(
                invalid_source.remap_to(&source.mesh).unwrap_err(),
                ConservativeRemap::between_voronoi_meshes(&invalid, &source.mesh).unwrap_err()
            );
            assert_eq!(
                cached.remap_to(&invalid).unwrap_err(),
                ConservativeRemap::between_voronoi_meshes(&source.mesh, &invalid).unwrap_err()
            );
        }
        let mut target = source.mesh.clone();
        let before = cached.remap_to(&target).unwrap();
        let prepared = cached.prepared.get().unwrap() as *const _;
        let site = target.active_vertex_slots().next().unwrap();
        let mut point = target.vertices()[site];
        point.x += 0.001;
        target.move_vertex(site, point);
        let after = cached.remap_to(&target).unwrap();
        assert_ne!(before, after);
        assert_eq!(
            after,
            ConservativeRemap::between_voronoi_meshes(&source.mesh, &target).unwrap()
        );
        assert_eq!(prepared, cached.prepared.get().unwrap() as *const _);
    }

    #[test]
    #[ignore = "manual release comparison, including lazy source preparation on first remap"]
    fn cached_voronoi_source_benchmark() {
        let subdivision = std::env::var("EARTHMESH_REMAP_BENCH_SUBDIVISION")
            .map(|value| {
                value
                    .parse::<usize>()
                    .expect("benchmark subdivision must be an integer")
            })
            .unwrap_or(80);
        let source = MotherGrid::generate(subdivision).unwrap();
        let target = MotherGrid::generate(subdivision / 2).unwrap();
        let cached = VoronoiRemapSource::new(&source.mesh);
        let mut fresh_seconds = 0.0;
        let mut cached_seconds = 0.0;
        for iteration in 0..6 {
            let mut measure = |reuse| {
                let started = std::time::Instant::now();
                let remap = if reuse {
                    cached.remap_to(&target.mesh).unwrap()
                } else {
                    ConservativeRemap::between_voronoi_meshes(&source.mesh, &target.mesh).unwrap()
                };
                let seconds = started.elapsed().as_secs_f64();
                if reuse {
                    cached_seconds += seconds;
                } else {
                    fresh_seconds += seconds;
                }
                eprintln!(
                    "source_remap_pair iteration={iteration} reused={reuse} seconds={seconds:.6}"
                );
                remap
            };
            let first_reuses = iteration % 2 != 0;
            let first = measure(first_reuses);
            let second = measure(!first_reuses);
            assert!(first == second, "cached remap or target binding changed");
        }
        eprintln!("source_remap_reuse repeats=6 source_cells={} target_cells={} fresh_seconds={fresh_seconds:.6} cached_seconds={cached_seconds:.6}", source.mesh.vertex_count(), target.mesh.vertex_count());
    }

    #[test]
    fn prepared_overlap_preserves_scalar_rows_and_certificates() {
        let source = MotherGrid::generate(2).unwrap();
        let target = MotherGrid::generate(3).unwrap();
        let sources = voronoi_rings(&source.mesh).unwrap();
        let mut targets = voronoi_rings(&target.mesh).unwrap();
        for reversed in [false, true] {
            if reversed {
                targets.iter_mut().for_each(|ring| ring.reverse());
            }
            let expected = scalar_overlap_reference(&sources, &targets).unwrap();
            for threads in [1, 4] {
                let actual = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap()
                    .install(|| ConservativeRemap::spherical_overlap(&sources, &targets))
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(
                    actual.certify_spherical_overlap(sources.len(), targets.len()),
                    expected.certify_spherical_overlap(sources.len(), targets.len())
                );
            }
        }
    }

    #[test]
    fn prepared_overlap_preserves_validation_and_unused_nonconvex_sources() {
        let valid = vec![(0., 0.), (2., 0.), (2., 2.), (0., 2.)];
        let concave = vec![(100., 0.), (102., 0.), (101., 0.5), (102., 2.), (100., 2.)];
        let sources = vec![valid.clone(), concave.clone()];
        let targets = vec![valid.clone()];
        assert_eq!(
            ConservativeRemap::spherical_overlap(&sources, &targets).unwrap(),
            scalar_overlap_reference(&sources, &targets).unwrap()
        );
        assert!(ConservativeRemap::spherical_overlap(&sources, &[concave])
            .unwrap_err()
            .contains("changes great-circle half-space"));

        let crossing = vec![(100., 0.), (102., 2.), (100., 2.), (102., 0.)];
        let invalid_sources = vec![valid.clone(), crossing];
        assert_eq!(
            ConservativeRemap::spherical_overlap(&invalid_sources, &targets).unwrap_err(),
            scalar_overlap_reference(&invalid_sources, &targets).unwrap_err()
        );
        assert!(ConservativeRemap::spherical_overlap(&[], &targets).is_err());
        assert!(ConservativeRemap::spherical_overlap(&sources, &[]).is_err());
        let mut invalid = valid;
        invalid[0].0 = f64::NAN;
        assert!(ConservativeRemap::spherical_overlap(&sources, &[invalid])
            .unwrap_err()
            .contains("invalid spherical cell 0"));
    }

    #[test]
    #[ignore = "manual release timing including input preparation, index and weight normalization"]
    fn prepared_overlap_benchmark() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        for n in [8, 16] {
            let sources = voronoi_rings(&MotherGrid::generate(n).unwrap().mesh).unwrap();
            let targets = voronoi_rings(&MotherGrid::generate(n + n / 2).unwrap().mesh).unwrap();
            let start = std::time::Instant::now();
            let expected = pool
                .install(|| scalar_overlap_reference(&sources, &targets))
                .unwrap();
            let scalar = start.elapsed();
            let start = std::time::Instant::now();
            let actual = pool
                .install(|| ConservativeRemap::spherical_overlap(&sources, &targets))
                .unwrap();
            let prepared = start.elapsed();
            assert_eq!(actual, expected);
            eprintln!("prepared_remap sources={} targets={} scalar_ms={:.3} prepared_ms={:.3} speedup={:.2}x",
                sources.len(), targets.len(), scalar.as_secs_f64()*1000., prepared.as_secs_f64()*1000.,
                scalar.as_secs_f64()/prepared.as_secs_f64());
        }
    }

    #[test]
    fn voronoi_ring_order_matches_the_scanned_cell_path() {
        let grid = MotherGrid::generate(2).unwrap();
        let rings = voronoi_rings(&grid.mesh).unwrap();
        for (cell, site) in grid.mesh.active_vertex_slots().enumerate() {
            let scanned = grid.mesh.voronoi_cell(site).unwrap();
            let expected = scanned
                .corners
                .into_iter()
                .map(|point| {
                    let radius = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
                    let point = [point.x / radius, point.y / radius, point.z / radius];
                    (
                        point[1].atan2(point[0]).to_degrees(),
                        point[2].clamp(-1.0, 1.0).asin().to_degrees(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(rings[cell], expected);
        }
    }

    #[test]
    fn remap_certification_is_thread_count_independent() {
        fn serial_certify(
            remap: &ConservativeRemap,
            valid_lineage: impl Fn(&RemapRow) -> bool,
        ) -> RemapCertificate {
            let closure_tolerance = (128.0
                * f64::EPSILON
                * remap.rows.len().max(
                    remap
                        .rows
                        .iter()
                        .map(|row| row.sources.len())
                        .max()
                        .unwrap_or(1),
                ) as f64)
                .max(1.0e-11);
            let mut negative_weights = 0;
            let mut bad_row_sums = 0;
            let mut bad_lineage_rows = 0;
            let mut constant_closure_error = 0.0_f64;
            for row in &remap.rows {
                if !valid_lineage(row) {
                    bad_lineage_rows += 1;
                }
                let sum: f64 = row
                    .sources
                    .iter()
                    .map(|&(_, weight)| {
                        if weight < 0.0 {
                            negative_weights += 1;
                        }
                        weight
                    })
                    .sum();
                let error = (sum - 1.0).abs();
                constant_closure_error = constant_closure_error.max(error);
                if error > closure_tolerance {
                    bad_row_sums += 1;
                }
            }
            RemapCertificate {
                rows: remap.rows.len(),
                negative_weights,
                bad_row_sums,
                bad_lineage_rows,
                constant_closure_error,
                global_area_closure_error: 0.0,
                closure_tolerance,
                target_fingerprint: remap.target_fingerprint,
            }
        }

        let remap = ConservativeRemap {
            rows: vec![
                RemapRow {
                    target: 0,
                    sources: vec![(0, 0.25), (1, 0.75)],
                },
                RemapRow {
                    target: 1,
                    sources: vec![(2, 0.4), (3, 0.4)],
                },
                RemapRow {
                    target: 6,
                    sources: vec![(4, 1.0)],
                },
                RemapRow {
                    target: 3,
                    sources: vec![(0, -0.5), (1, 1.5)],
                },
            ],
            coverage_error: 0.0,
            source_fingerprint: Some(7),
            target_fingerprint: Some(7),
            covered_targets: None,
        };
        let valid_lineage =
            |row: &RemapRow| row.target < 4 && row.sources.iter().all(|&(source, _)| source < 4);
        let expected = serial_certify(&remap, valid_lineage);
        let one_thread = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| remap.certify_with_lineage(valid_lineage));
        let four_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap()
            .install(|| remap.certify_with_lineage(valid_lineage));

        assert_eq!(one_thread, expected);
        assert_eq!(four_threads, expected);
    }

    #[test]
    fn overlap_tolerance_scales_with_the_finest_input_mesh() {
        let small = ConservativeRemap::identity(1).certify_identity(1);
        let large = ConservativeRemap::identity(1).certify_spherical_overlap(1_000, 1);
        assert!(large.closure_tolerance() > small.closure_tolerance());
    }
}
