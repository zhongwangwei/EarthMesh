use crate::fingerprint::mesh_fingerprint;
use crate::mother_grid::{MotherGrid, TriangleAddress};
use earthmesh_boundary::SphericalCap;
use earthmesh_geometry::{
    spherical_convex_overlap_fraction, try_spherical_polygon_excess, Point, SphericalAreaBranch,
};
use earthmesh_mesh::{spherical_triangle_area_unit, MeshState};
use rayon::prelude::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct RemapRow {
    pub target: usize,
    pub sources: Vec<(usize, f64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConservativeRemap {
    rows: Vec<RemapRow>,
    coverage_error: f64,
    target_fingerprint: Option<u64>,
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
            target_fingerprint: None,
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
            target_fingerprint: None,
        }
    }

    pub fn identity_for_mesh(mesh: &MeshState) -> Self {
        let fingerprint = mesh_fingerprint(mesh);
        let mut remap = Self::identity(mesh.active_vertex_slots().count());
        remap.target_fingerprint = Some(fingerprint);
        remap
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
            target_fingerprint: Some(mesh_fingerprint(&coarse.mesh)),
        })
    }

    pub fn spherical_overlap(
        source_cells: &[Vec<(f64, f64)>],
        target_cells: &[Vec<(f64, f64)>],
    ) -> Result<Self, String> {
        if source_cells.is_empty() || target_cells.is_empty() {
            return Err("spherical remap needs non-empty source and target cells".into());
        }
        let to_points = |cells: &[Vec<(f64, f64)>]| -> Result<Vec<Vec<Point>>, String> {
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
        let sources = to_points(source_cells)?;
        let targets = to_points(target_cells)?;
        let index = SphericalCapIndex::new(&sources)?;
        let rows = targets
            .par_iter()
            .enumerate()
            .map(|(target, target_ring)| {
                let target_cap = SphericalCap::for_rings(std::slice::from_ref(target_ring))
                    .ok_or_else(|| format!("target cell {target} has no spherical cap"))?;
                let mut overlaps = Vec::new();
                for source in index.candidates(target_cap) {
                    if !target_cap.overlaps(index.caps[source]) {
                        continue;
                    }
                    let fraction = spherical_convex_overlap_fraction(target_ring, &sources[source])
                        .map_err(|error| {
                            format!(
                                "source {source} {:?} and target {target} {:?} overlap failed: {error}",
                                source_cells[source], target_cells[target]
                            )
                        })?;
                    if fraction > 1.0e-14 {
                        overlaps.push((source, fraction));
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
        let rows = rows.into_iter().map(|(row, _)| row).collect();
        Ok(Self {
            rows,
            coverage_error,
            target_fingerprint: None,
        })
    }

    pub fn between_voronoi_meshes(source: &MeshState, target: &MeshState) -> Result<Self, String> {
        let mut remap = Self::spherical_overlap(&voronoi_rings(source)?, &voronoi_rings(target)?)?;
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
        if self.rows.len() != target_cells
            || self
                .rows
                .iter()
                .enumerate()
                .any(|(target, row)| row.target != target)
        {
            certificate.bad_lineage_rows += 1;
        }
        certificate.closure_tolerance = certificate
            .closure_tolerance
            .max(128.0 * f64::EPSILON * source_cells.max(target_cells) as f64);
        certificate.global_area_closure_error = self.coverage_error;
        certificate
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
        let closure_tolerance = (128.0
            * f64::EPSILON
            * self.rows.len().max(
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

pub(crate) fn voronoi_rings(mesh: &MeshState) -> Result<Vec<Vec<(f64, f64)>>, String> {
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
    mesh.active_vertex_slots()
        .collect::<Vec<_>>()
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
        .collect()
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
            target_fingerprint: Some(7),
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
