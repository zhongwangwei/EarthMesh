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

/// Caps binned for overlap queries (guide 11.120). Each cap lies on the level
/// whose latitude-longitude tiles, 2^k degrees on a side, are at least as wide
/// as the cap, so it occupies a tile or two whatever the caps' sizes, number or
/// place; only occupied tiles are kept, sorted. A query reads, on every level,
/// the tiles its own cap reaches. The grid this replaces was sized by the
/// number of caps alone, as if they covered the sphere: a region's caps
/// crowded into a few of its tiles and every query read thousands of them.
pub(crate) struct SphericalCapIndex {
    /// Occupied tiles in ascending (level, row, column) order. Column -1
    /// holds the caps that occupy their rows whole: within a tile of a pole,
    /// or reaching round the sphere.
    tiles: Vec<(i32, i64, i64)>,
    /// `members[starts[i]..starts[i + 1]]` are the caps on `tiles[i]`,
    /// ascending.
    starts: Vec<usize>,
    members: Vec<usize>,
    /// The levels holding caps, ascending.
    levels: Vec<i32>,
    caps: Vec<SphericalCap>,
}

/// How far, in radians, the index reaches beyond a cap's radius: well past
/// the rounding of `SphericalCap::overlaps` (its `acos` errs by up to about
/// 1.5e-8 for nearly coincident centres) and of the tiles' coordinates, so
/// two caps it calls overlapping share an open set, and with it a tile.
const CAP_INDEX_SLACK: f64 = 1.0e-7;

/// Tile levels: 2^k degrees, from about 0.1 m to 64 degrees.
const CAP_LEVEL_MIN: i32 = -20;
const CAP_LEVEL_MAX: i32 = 6;

/// The tiles of one level: `tile` degrees on a side, `rows` by `columns`
/// (the last row and column narrower where the tile does not divide the
/// sphere).
#[derive(Clone, Copy)]
struct CapTiling {
    level: i32,
    tile: f64,
    rows: i64,
    columns: i64,
}

impl CapTiling {
    fn new(level: i32) -> Self {
        let tile = 2f64.powi(level);
        Self {
            level,
            tile,
            rows: (180.0 / tile).ceil() as i64,
            columns: (360.0 / tile).ceil() as i64,
        }
    }

    /// The level a cap is stored on: tiles at least its reach's diameter.
    fn for_cap(cap: SphericalCap) -> Self {
        let reach = (cap.radius_radians() + CAP_INDEX_SLACK)
            .min(std::f64::consts::PI)
            .to_degrees();
        Self::new(((2.0 * reach).log2().ceil() as i32).clamp(CAP_LEVEL_MIN, CAP_LEVEL_MAX))
    }

    fn row(self, lat: f64) -> i64 {
        (((lat + 90.0) / self.tile).floor() as i64).clamp(0, self.rows - 1)
    }

    fn column(self, lon: f64) -> i64 {
        (((lon + 180.0) / self.tile).floor() as i64).clamp(0, self.columns - 1)
    }

    /// The rows a cap reaches, its radius grown by `CAP_INDEX_SLACK`, and,
    /// clear of the poles by a tile, its columns: those within
    /// `asin(sin r / cos lat)` of its centre's longitude, split at the
    /// antimeridian (the second range empty when it is not crossed);
    /// `None` -- every column -- nearer a pole or when they go round. Band
    /// ends map to rows and columns by `floor`, which is monotone, so caps
    /// whose grown bands meet have a row and a column in common.
    fn span(self, cap: SphericalCap) -> (std::ops::RangeInclusive<i64>, Option<[(i64, i64); 2]>) {
        let (lon, lat) = cap.center_lon_lat_degrees();
        let reach = (cap.radius_radians() + CAP_INDEX_SLACK).min(std::f64::consts::PI);
        let (south, north) = (lat - reach.to_degrees(), lat + reach.to_degrees());
        let rows = self.row(south)..=self.row(north);
        let columns = (south - self.tile > -90.0 && north + self.tile < 90.0)
            .then(|| (reach.sin() / lat.to_radians().cos()).asin().to_degrees())
            .filter(|half| half.is_finite() && 2.0 * (half + self.tile) < 360.0)
            .map(|half| {
                let (west, east) = (lon - half, lon + half);
                if west < -180.0 {
                    [
                        (self.column(west + 360.0), self.columns - 1),
                        (0, self.column(east)),
                    ]
                } else if east > 180.0 {
                    [
                        (self.column(west), self.columns - 1),
                        (0, self.column(east - 360.0)),
                    ]
                } else {
                    [(self.column(west), self.column(east)), (1, 0)]
                }
            });
        (rows, columns)
    }

    /// The tiles a cap occupies on this level.
    fn tiles(self, cap: SphericalCap) -> Vec<(i32, i64, i64)> {
        let (rows, columns) = self.span(cap);
        match columns {
            None => rows.map(|row| (self.level, row, -1)).collect(),
            Some(ranges) => rows
                .flat_map(|row| {
                    ranges
                        .into_iter()
                        .flat_map(move |(first, last)| first..=last)
                        .map(move |column| (self.level, row, column))
                })
                .collect(),
        }
    }
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
        let mut entries = caps
            .par_iter()
            .enumerate()
            .flat_map_iter(|(member, &cap)| {
                CapTiling::for_cap(cap)
                    .tiles(cap)
                    .into_iter()
                    .map(move |tile| (tile, member))
            })
            .collect::<Vec<_>>();
        entries.par_sort_unstable();
        let mut tiles = Vec::new();
        let mut starts = Vec::new();
        let mut members = Vec::with_capacity(entries.len());
        for (tile, member) in entries {
            if tiles.last() != Some(&tile) {
                tiles.push(tile);
                starts.push(members.len());
            }
            members.push(member);
        }
        starts.push(members.len());
        let mut levels = tiles.iter().map(|&(level, _, _)| level).collect::<Vec<_>>();
        levels.dedup();
        Self {
            tiles,
            starts,
            members,
            levels,
            caps,
        }
    }

    /// Calls `visit` with every cap on a tile `cap` reaches, on every level --
    /// all the caps it overlaps among them, some more than once.
    fn visit_reached(&self, cap: SphericalCap, mut visit: impl FnMut(usize)) {
        for &level in &self.levels {
            let tiling = CapTiling::new(level);
            let (rows, columns) = tiling.span(cap);
            let (first_row, last_row) = (*rows.start(), *rows.end());
            let lower = self
                .tiles
                .partition_point(|&tile| tile < (level, first_row, -1));
            let upper = self
                .tiles
                .partition_point(|&tile| tile <= (level, last_row, i64::MAX));
            let mut members = |index: usize| {
                for &member in &self.members[self.starts[index]..self.starts[index + 1]] {
                    visit(member);
                }
            };
            let wanted = |column: i64| {
                column == -1
                    || columns.is_none_or(|ranges| {
                        ranges
                            .iter()
                            .any(|&(first, last)| (first..=last).contains(&column))
                    })
            };
            if (last_row - first_row) as usize >= upper - lower {
                // More rows than occupied tiles in the band: walk the tiles.
                for index in lower..upper {
                    if wanted(self.tiles[index].2) {
                        members(index);
                    }
                }
                continue;
            }
            let band = &self.tiles[lower..upper];
            let seek = |key: (i32, i64, i64)| lower + band.partition_point(|&tile| tile < key);
            for row in rows {
                let whole = seek((level, row, -1));
                if whole < upper && self.tiles[whole] == (level, row, -1) {
                    members(whole);
                }
                let ranges = columns.unwrap_or([(0, tiling.columns - 1), (1, 0)]);
                for (first, last) in ranges.into_iter().filter(|(first, last)| first <= last) {
                    let mut index = seek((level, row, first));
                    while index < upper && self.tiles[index] <= (level, row, last) {
                        members(index);
                        index += 1;
                    }
                }
            }
        }
    }

    /// The caps on the tiles `cap` reaches, ascending: every cap it overlaps
    /// among them.
    pub(crate) fn candidates(&self, cap: SphericalCap) -> Vec<usize> {
        let mut candidates = Vec::new();
        self.visit_reached(cap, |member| candidates.push(member));
        candidates.sort_unstable();
        candidates.dedup();
        candidates
    }

    /// `candidates` without its sort, deduplicated through `seen`, which
    /// marks the caps listed for `generation`: in the index's order, so a
    /// sum over them should not depend on it.
    pub(crate) fn candidates_into(
        &self,
        cap: SphericalCap,
        seen: &mut [u32],
        generation: u32,
        candidates: &mut Vec<usize>,
    ) {
        candidates.clear();
        self.visit_reached(cap, |member| {
            if seen[member] != generation {
                seen[member] = generation;
                candidates.push(member);
            }
        });
    }
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

    /// A deterministic stream of numbers in [0, 1) (splitmix64).
    fn uniform(state: &mut u64) -> f64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// The cap of a triangle `size` degrees about a centre.
    fn cap_around(lon: f64, lat: f64, size: f64, turn: f64) -> SphericalCap {
        let ring = (0..3)
            .map(|corner| {
                let angle = turn + std::f64::consts::TAU * corner as f64 / 3.0;
                let lat = (lat + size * angle.sin()).clamp(-90.0, 90.0);
                let lon = lon + size * angle.cos() / lat.to_radians().cos().max(0.05);
                Point::new(lon, lat)
            })
            .collect::<Vec<_>>();
        SphericalCap::for_rings(&[ring]).unwrap()
    }

    /// Caps where tiles are awkward -- about the poles, the antimeridian and
    /// a tile edge -- and scattered, from a decimetre to a hemisphere.
    fn awkward_caps(state: &mut u64, count: usize) -> Vec<SphericalCap> {
        let centres = [
            (0.0, 90.0),
            (40.0, -89.99),
            (180.0, 12.0),
            (-179.999, -40.0),
            (0.0, 0.0),
            (100.0, 38.0),
        ];
        (0..count)
            .map(|slot| {
                let (lon, lat) = if slot % 2 == 0 {
                    let (lon, lat) = centres[(uniform(state) * centres.len() as f64) as usize];
                    let jitter = 10f64.powf(-5.0 + 6.0 * uniform(state));
                    (
                        lon + jitter * (2.0 * uniform(state) - 1.0),
                        (lat + jitter * (2.0 * uniform(state) - 1.0)).clamp(-90.0, 90.0),
                    )
                } else {
                    (
                        360.0 * uniform(state) - 180.0,
                        (2.0 * uniform(state) - 1.0).asin().to_degrees(),
                    )
                };
                let size = match slot % 7 {
                    6 => 20.0 + 50.0 * uniform(state),
                    _ => 10f64.powf(-6.0 + 6.5 * uniform(state)),
                };
                cap_around(lon, lat, size, std::f64::consts::TAU * uniform(state))
            })
            .collect()
    }

    #[test]
    fn the_tiled_cap_index_finds_every_overlap() {
        let mut state = 11;
        let caps = awkward_caps(&mut state, 3000);
        let queries = awkward_caps(&mut state, 1500);
        let index = SphericalCapIndex::from_caps(caps.clone());
        let mut seen = vec![0; caps.len()];
        let mut listed = Vec::new();
        let mut overlaps = 0;
        for (generation, &query) in (1..).zip(&queries) {
            let scan = (0..caps.len())
                .filter(|&cap| query.overlaps(caps[cap]))
                .collect::<Vec<_>>();
            let candidates = index.candidates(query);
            assert!(candidates.windows(2).all(|pair| pair[0] < pair[1]));
            let found = candidates
                .iter()
                .copied()
                .filter(|&cap| query.overlaps(caps[cap]))
                .collect::<Vec<_>>();
            assert_eq!(found, scan, "query {query:?}");
            index.candidates_into(query, &mut seen, generation, &mut listed);
            let mut sorted = listed.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, candidates);
            overlaps += scan.len();
        }
        assert!(overlaps > 10_000, "{overlaps}");
    }

    /// A region's small caps, which a grid sized by their number alone put
    /// in one tile: a query now reads its neighbours, not all of them.
    #[test]
    fn the_tiled_cap_index_reads_a_regions_neighbours_only() {
        let mut state = 5;
        let caps = (0..3000)
            .map(|_| {
                cap_around(
                    100.0 + uniform(&mut state),
                    38.0 + uniform(&mut state),
                    0.01,
                    std::f64::consts::TAU * uniform(&mut state),
                )
            })
            .collect::<Vec<_>>();
        let index = SphericalCapIndex::from_caps(caps.clone());
        let read = caps
            .iter()
            .map(|&cap| index.candidates(cap).len())
            .sum::<usize>();
        let overlapping = caps
            .iter()
            .map(|&cap| caps.iter().filter(|&&other| cap.overlaps(other)).count())
            .sum::<usize>();
        assert!(
            read < 4 * overlapping,
            "read {read}, overlapping {overlapping}"
        );
        assert!(read < 3000 * 3000 / 20, "read {read}");
    }

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
