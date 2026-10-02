//! On-demand reverse coarsening: where the finest mother has to be built
//! (design B1b in `docs/certified_mesh/on_demand_reverse_coarsening.md`).
//!
//! Reverse coarsening merges every parent whose requirement allows it, level by
//! level, from the finest mother down to the base. Far from any requirement the
//! result is the base mother itself, so only the base faces a requirement can
//! reach need the finest level; the rest are settled by construction. This
//! module draws that line from the requirement raster alone, before anything
//! finer than the base is built.

use crate::mother_grid::{MotherGrid, TriangleAddress};
use crate::requirement::RasterLevelField;
use earthmesh_boundary::SphericalCap;
use earthmesh_geometry::Point;
use std::collections::{BTreeSet, VecDeque};

/// Base faces in three parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterializationExtent {
    pub base_subdivision: usize,
    /// R: base faces a requirement can reach, built at the finest level and
    /// certified cell by cell.
    pub region: BTreeSet<TriangleAddress>,
    /// F: base faces within two vertex rings outside R, built at the finest
    /// level so that R's boundary cells have their neighbours and their
    /// remap sources. One ring is not enough: a base-sized cell on R's edge
    /// reaches 0.58 base edges into F, and the open fine cells on F's outer
    /// edge reach 0.29 back -- together the 0.87 of a single ring's height.
    /// Their requirement is zero at every level, so they only ever condense.
    pub frame: BTreeSet<TriangleAddress>,
    /// S: the remaining base faces, never built -- settled at the base level
    /// and certified by construction.
    pub settled_faces: usize,
}

/// How far a requirement reaches, in the units the coarsening counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtentMargins {
    /// `graded_envelope`'s fine rings per level.
    pub gradation_rings_per_level: usize,
    /// Parent rings, at every level, that a transaction may touch beyond a
    /// parent that cannot coarsen: the widest transition ring, the elastic
    /// domain around it, and the certificates' neighbourhood of what moved.
    pub parent_rings_per_level: usize,
}

impl MaterializationExtent {
    /// Base faces built at the finest level: R and F.
    pub fn built_faces(&self) -> impl Iterator<Item = TriangleAddress> + '_ {
        self.region.iter().chain(&self.frame).copied()
    }
}

fn unit(point: earthmesh_mesh::CartesianPoint) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2])
        .clamp(-1.0, 1.0)
        .acos()
}

/// The base faces a requirement can reach, the frame around them, and how many
/// are left settled. `levels` is the number of 2:1 levels between `base` and
/// the finest mother.
///
/// A fine cell takes the highest level of every raster cell it overlaps, the
/// graded envelope spreads that `gradation_rings_per_level` fine rings per
/// level, and at each coarser level k a parent that cannot coarsen draws a
/// transition, an elastic domain and certificate checks up to
/// `parent_rings_per_level` (D) parent rings around it -- (D + 1) / 2^(k-1)
/// base rings, rounded up. R is every base face within the sum of those over
/// the levels, plus two for rounding, of a base face whose fine cells may
/// overlap a raster cell above level zero.
///
/// `delivered` are base faces whose cells a regional run delivers: they are
/// certified cell by cell too, so they join R with a ring around them.
pub fn materialization_extent(
    raster: &RasterLevelField,
    base: &MotherGrid,
    levels: usize,
    margins: ExtentMargins,
    delivered: &BTreeSet<TriangleAddress>,
) -> Result<MaterializationExtent, String> {
    if base.region.is_some() {
        return Err("the extent is drawn on a whole base mother".into());
    }
    let faces = base.mesh.active_triangle_slots().collect::<Vec<_>>();
    let caps = faces
        .iter()
        .map(|&face| {
            let corners = base.mesh.triangles()[face].map(|site| unit(base.mesh.vertices()[site]));
            let sum = [
                corners[0][0] + corners[1][0] + corners[2][0],
                corners[0][1] + corners[1][1] + corners[2][1],
                corners[0][2] + corners[1][2] + corners[2][2],
            ];
            let length = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
            let center = [sum[0] / length, sum[1] / length, sum[2] / length];
            let radius = corners
                .iter()
                .map(|&corner| angle(center, corner))
                .fold(0.0, f64::max);
            (center, radius)
        })
        .collect::<Vec<_>>();
    let longest_base_edge = faces
        .iter()
        .flat_map(|&face| {
            let corners = base.mesh.triangles()[face].map(|site| unit(base.mesh.vertices()[site]));
            (0..3).map(move |side| angle(corners[side], corners[(side + 1) % 3]))
        })
        .fold(0.0, f64::max);
    // A fine edge is at most the longest base edge halved per level; twice
    // that covers the lattice's uneven spacing inside a base face.
    let fine_edge = 2.0 * longest_base_edge / (1u64 << levels.min(62)) as f64;

    // Seeds: base faces whose fine cells may overlap a raster cell above
    // level zero, or lie within the graded envelope's spread of one. The
    // raster is regular, so each base face scans the raster cells inside its
    // reach's latitude-longitude box and tests their caps exactly.
    let (nlon, nlat) = (raster.nlon(), raster.nlat());
    let dlon = 360.0 / nlon as f64;
    let dlat = 180.0 / nlat as f64;
    let reach =
        |level: usize| fine_edge * (margins.gradation_rings_per_level.max(1) * level + 2) as f64;
    let widest_reach = reach(raster.levels().iter().copied().max().unwrap_or(0));
    // A raster cell's cap (`SphericalCap::for_rings` adds its longest edge to
    // its corners' radius) is within its two sides plus its diagonal.
    let cell_radius = 2.0 * (dlon + dlat).to_radians();
    let mut raster_caps = std::collections::HashMap::new();
    let mut seeds = BTreeSet::new();
    for (index, &(face_center, face_radius)) in caps.iter().enumerate() {
        let lat = face_center[2].clamp(-1.0, 1.0).asin().to_degrees();
        let lon = face_center[1].atan2(face_center[0]).to_degrees();
        let span = (face_radius + widest_reach + cell_radius).to_degrees();
        let rows = (((lat - span + 90.0) / dlat).floor().max(0.0) as usize)
            ..=(((lat + span + 90.0) / dlat).floor().min((nlat - 1) as f64) as usize);
        let polar = lat + span >= 90.0 || lat - span <= -90.0;
        let lon_span = if polar {
            360.0
        } else {
            span / (lat.abs() + span).to_radians().cos().max(1.0e-9)
        };
        let columns = if lon_span >= 180.0 {
            (0..nlon).collect::<Vec<_>>()
        } else {
            let first = ((lon - lon_span + 180.0) / dlon).floor() as isize;
            let last = ((lon + lon_span + 180.0) / dlon).floor() as isize;
            (first..=last)
                .map(|column| column.rem_euclid(nlon as isize) as usize)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        'scan: for row in rows {
            for &column in &columns {
                let cell = row * nlon + column;
                let level = raster.levels()[cell];
                if level == 0 {
                    continue;
                }
                let (cell_center, cell_radius) = match raster_caps.get(&cell) {
                    Some(&cap) => cap,
                    None => {
                        let points = raster
                            .spherical_cell(cell, dlon, dlat)
                            .into_iter()
                            .map(|(lon, lat)| Point::new(lon, lat))
                            .collect::<Vec<_>>();
                        let cap = SphericalCap::for_rings(std::slice::from_ref(&points))
                            .ok_or_else(|| format!("raster cell {cell} has no spherical cap"))?;
                        let (lon, lat) = cap.center_lon_lat_degrees();
                        let (lon, lat) = (lon.to_radians(), lat.to_radians());
                        let center = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
                        *raster_caps
                            .entry(cell)
                            .or_insert((center, cap.radius_radians()))
                    }
                };
                if angle(cell_center, face_center) <= cell_radius + reach(level) + face_radius {
                    seeds.insert(index);
                    break 'scan;
                }
            }
        }
    }

    // Base faces around each face (sharing a vertex).
    let mut faces_at_site = vec![Vec::new(); base.mesh.vertices().len()];
    for (index, &face) in faces.iter().enumerate() {
        for site in base.mesh.triangles()[face] {
            faces_at_site[site].push(index);
        }
    }
    let around = |index: usize| {
        base.mesh.triangles()[faces[index]]
            .into_iter()
            .flat_map(|site| faces_at_site[site].iter().copied())
    };
    let rings = (1..=levels)
        .map(|level| (margins.parent_rings_per_level + 1).div_ceil(1usize << (level - 1).min(62)))
        .sum::<usize>()
        + 2;
    let mut distance = vec![usize::MAX; faces.len()];
    let mut queue = VecDeque::new();
    for &seed in &seeds {
        distance[seed] = 0;
        queue.push_back(seed);
    }
    while let Some(index) = queue.pop_front() {
        if distance[index] == rings {
            continue;
        }
        for next in around(index) {
            if distance[next] == usize::MAX {
                distance[next] = distance[index] + 1;
                queue.push_back(next);
            }
        }
    }
    // Delivered faces and a ring around them, at distance zero.
    let delivered_indices = faces
        .iter()
        .enumerate()
        .filter(|(_, &face)| {
            base.triangle_addresses[face].is_some_and(|address| delivered.contains(&address))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let mut delivered_ring = BTreeSet::new();
    for &index in &delivered_indices {
        delivered_ring.insert(index);
        delivered_ring.extend(around(index));
    }
    for index in delivered_ring {
        if distance[index] == usize::MAX {
            distance[index] = 0;
        }
    }
    let address = |index: usize| {
        base.triangle_addresses[faces[index]]
            .ok_or_else(|| format!("base face {} has no address", faces[index]))
    };
    let mut region = BTreeSet::new();
    let mut frame_indices = BTreeSet::new();
    for (index, &reached) in distance.iter().enumerate() {
        if reached != usize::MAX {
            region.insert(address(index)?);
            frame_indices.extend(around(index).filter(|&next| distance[next] == usize::MAX));
        }
    }
    let first_ring = frame_indices.clone();
    for index in first_ring {
        frame_indices.extend(around(index).filter(|&next| distance[next] == usize::MAX));
    }
    let frame = frame_indices
        .into_iter()
        .map(address)
        .collect::<Result<BTreeSet<_>, _>>()?;
    Ok(MaterializationExtent {
        base_subdivision: base.subdivision,
        settled_faces: faces.len() - region.len() - frame.len(),
        region,
        frame,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::region::descendant_faces;
    use crate::requirement::{graded_envelope, one_ring_adjacency};

    fn raster_with(nlon: usize, nlat: usize, cells: &[(usize, usize)]) -> RasterLevelField {
        let mut levels = vec![0; nlon * nlat];
        for &(cell, level) in cells {
            levels[cell] = level;
        }
        RasterLevelField::new(nlon, nlat, levels).unwrap()
    }

    const MARGINS: ExtentMargins = ExtentMargins {
        gradation_rings_per_level: 3,
        parent_rings_per_level: 1,
    };

    #[test]
    fn no_requirement_builds_nothing() {
        let base = MotherGrid::generate(4).unwrap();
        let extent = materialization_extent(
            &raster_with(36, 18, &[]),
            &base,
            2,
            MARGINS,
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(extent.region.is_empty() && extent.frame.is_empty());
        assert_eq!(extent.settled_faces, 320);
    }

    #[test]
    fn the_frame_surrounds_the_region_and_wider_margins_widen_it() {
        let base = MotherGrid::generate(6).unwrap();
        // One raster cell near (95 E, 25 N).
        let raster = raster_with(72, 36, &[(25 * 72 + 55, 2)]);
        let narrow = materialization_extent(&raster, &base, 2, MARGINS, &BTreeSet::new()).unwrap();
        let wide = materialization_extent(
            &raster,
            &base,
            2,
            ExtentMargins {
                parent_rings_per_level: 3,
                ..MARGINS
            },
            &BTreeSet::new(),
        )
        .unwrap();
        assert!(!narrow.region.is_empty());
        assert!(narrow.region.is_subset(&wide.region) && narrow.region.len() < wide.region.len());
        for extent in [&narrow, &wide] {
            assert!(extent.region.is_disjoint(&extent.frame));
            assert_eq!(
                extent.region.len() + extent.frame.len() + extent.settled_faces,
                720
            );
        }
    }

    /// The point of the extent: every fine site whose graded requirement is
    /// above zero lies under R, for rasters with scattered cells of levels 1
    /// and 2 over bases of 3 and 4 with one and two levels.
    #[test]
    fn every_site_a_requirement_reaches_lies_in_the_region() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut checked = 0;
        for (base_n, levels) in [(3, 1), (3, 2), (4, 1), (4, 2)] {
            let fine_n = base_n << levels;
            let base = MotherGrid::generate(base_n).unwrap();
            let fine = MotherGrid::generate(fine_n).unwrap();
            let sites = fine.mesh.active_vertex_slots().collect::<Vec<_>>();
            let mut compact = vec![usize::MAX; fine.mesh.vertices().len()];
            for (index, &site) in sites.iter().enumerate() {
                compact[site] = index;
            }
            let triangles = fine
                .mesh
                .active_triangle_slots()
                .map(|face| fine.mesh.triangles()[face].map(|site| compact[site]))
                .collect::<Vec<_>>();
            let adjacency = one_ring_adjacency(&[vec![[0; 3]; 2], triangles].concat(), sites.len());
            let target = crate::requirement::TargetLevelField::from_active_voronoi_cells(
                &fine.mesh,
                vec![levels; sites.len()],
            )
            .unwrap();
            for _ in 0..3 {
                let (nlon, nlat) = (72, 36);
                let cells = (0..1 + next() % 4)
                    .map(|_| {
                        (
                            (next() % (nlon * nlat) as u64) as usize,
                            1 + (next() % 2) as usize,
                        )
                    })
                    .map(|(cell, level)| (cell, level.min(levels)))
                    .collect::<Vec<_>>();
                let raster = raster_with(nlon, nlat, &cells);
                let extent =
                    materialization_extent(&raster, &base, levels, MARGINS, &BTreeSet::new())
                        .unwrap();
                let projected = crate::requirement::certify_final_cell_requirements_from_raster(
                    &raster, &fine.mesh, &target, 1,
                )
                .unwrap();
                let graded = graded_envelope(
                    &adjacency,
                    projected.required_levels(),
                    MARGINS.gradation_rings_per_level,
                );
                let built = descendant_faces(extent.region.iter().copied(), fine_n).unwrap();
                let mut covered = vec![false; fine.mesh.vertices().len()];
                for face in built {
                    for site in fine.mesh.triangles()[fine.face_slot(face).unwrap()] {
                        covered[site] = true;
                    }
                }
                for (index, &site) in sites.iter().enumerate() {
                    if graded[index] > 0 {
                        checked += 1;
                        assert!(
                            covered[site],
                            "base {base_n}, {levels} levels, cells {cells:?}: site {site} \
                             (graded {}) is outside R",
                            graded[index]
                        );
                    }
                }
            }
        }
        assert!(checked > 100, "{checked}");
    }
    /// Projected on the built region, every cell gets the level the whole
    /// sphere's projection gives it; the open edge's sites need none.
    #[test]
    fn the_region_projects_the_raster_as_the_whole_sphere() {
        let (base_n, levels) = (4, 2);
        let fine_n = base_n << levels;
        let base = MotherGrid::generate(base_n).unwrap();
        let fine = MotherGrid::generate(fine_n).unwrap();
        let raster = raster_with(72, 36, &[(25 * 72 + 55, 2), (24 * 72 + 56, 1)]);
        let extent =
            materialization_extent(&raster, &base, levels, MARGINS, &BTreeSet::new()).unwrap();
        let region = MotherGrid::generate_faces(
            fine_n,
            descendant_faces(extent.built_faces(), fine_n).unwrap(),
        )
        .unwrap();
        let target = crate::requirement::TargetLevelField::from_active_voronoi_cells(
            &fine.mesh,
            vec![levels; fine.mesh.active_vertex_slots().count()],
        )
        .unwrap();
        let whole = crate::requirement::certify_final_cell_requirements_from_raster(
            &raster, &fine.mesh, &target, 1,
        )
        .unwrap();
        let whole_at = fine
            .mesh
            .active_vertex_slots()
            .zip(whole.required_levels())
            .map(|(site, &level)| (fine.addresses[site].clone().unwrap(), level))
            .collect::<std::collections::BTreeMap<_, _>>();
        let outer = region.region.as_ref().unwrap().outer_boundary();
        let projected = crate::requirement::region_required_levels_from_raster(
            &raster,
            &region.mesh,
            outer,
            fine.mesh.vertex_count(),
        )
        .unwrap();
        assert!(projected.iter().any(|&level| level > 0));
        for (site, level) in region.mesh.active_vertex_slots().zip(projected) {
            let expected = whole_at[region.addresses[site].as_ref().unwrap()];
            if outer.contains(&site) {
                assert_eq!((level, expected), (0, 0), "open edge site {site}");
            } else {
                assert_eq!(level, expected, "site {site}");
            }
        }
    }

    /// Delivered faces join R with a ring, even with no requirement at all.
    #[test]
    fn delivered_faces_are_built_and_certified() {
        let base = MotherGrid::generate(4).unwrap();
        let delivered = base
            .triangle_addresses
            .iter()
            .flatten()
            .copied()
            .take(3)
            .collect::<BTreeSet<_>>();
        let extent =
            materialization_extent(&raster_with(36, 18, &[]), &base, 2, MARGINS, &delivered)
                .unwrap();
        assert!(delivered.is_subset(&extent.region));
        assert!(extent.region.len() > delivered.len());
        assert!(!extent.frame.is_empty() && extent.settled_faces > 0);
    }
}
