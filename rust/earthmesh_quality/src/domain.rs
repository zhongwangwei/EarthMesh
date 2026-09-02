use std::collections::VecDeque;
use std::fmt;

use earthmesh_boundary::SphericalBoundaryModel;
use earthmesh_geometry::EARTH_RADIUS_KM;

use crate::QualityMeshInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QualityZone {
    TargetCore,
    BoundaryProtection,
    ExportCorridor,
    DeepExterior,
    GlobalNeutral,
}

impl QualityZone {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TargetCore => "target_core",
            Self::BoundaryProtection => "boundary_protection",
            Self::ExportCorridor => "export_corridor",
            Self::DeepExterior => "deep_exterior",
            Self::GlobalNeutral => "global_neutral",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct QualityPrioritySample {
    pub zone: QualityZone,
    pub maximum_priority: f64,
    pub mean_priority: f64,
    pub minimum_distance_to_target: f64,
    pub minimum_distance_to_boundary: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PriorityAggregate {
    pub maximum: f64,
    pub mean: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphericalDistanceSample {
    pub target_km: f64,
    pub boundary_km: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoastTarget {
    Land,
    Ocean,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoastPrioritySample {
    pub quality: QualityPrioritySample,
    /// Land is positive, ocean is negative, and coast seeds are zero.
    pub signed_distance_rings: isize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PriorityFieldError {
    EmptyTarget,
    InvalidBoundary(usize),
    InvalidCell(usize),
    InvalidNeighbor { cell: usize, neighbor: usize },
    NonReciprocalNeighbor { cell: usize, neighbor: usize },
    LengthMismatch,
    InvalidSourcePriority(usize),
    MissingProvenance(usize),
}

impl fmt::Display for PriorityFieldError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTarget => formatter.write_str("quality domain requires a target"),
            Self::InvalidBoundary(index) => write!(formatter, "target boundary {index} is invalid"),
            Self::InvalidCell(index) => write!(formatter, "quality cell {index} is invalid"),
            Self::InvalidNeighbor { cell, neighbor } => {
                write!(
                    formatter,
                    "quality cell {cell} has invalid neighbor {neighbor}"
                )
            }
            Self::NonReciprocalNeighbor { cell, neighbor } => write!(
                formatter,
                "quality cells {cell} and {neighbor} are not reciprocal neighbors"
            ),
            Self::LengthMismatch => formatter.write_str("quality field input lengths differ"),
            Self::InvalidSourcePriority(index) => {
                write!(formatter, "source priority {index} is outside [0, 1]")
            }
            Self::MissingProvenance(index) => {
                write!(formatter, "quality cell {index} has no source provenance")
            }
        }
    }
}

impl std::error::Error for PriorityFieldError {}

/// Aggregate a source-grid priority field through final-cell provenance.
/// Maximum protects hard-like ordering; mean supports later weighted scoring.
pub fn aggregate_source_priority(
    source_priority: &[f64],
    cell_provenance: &[Vec<usize>],
) -> Result<Vec<PriorityAggregate>, PriorityFieldError> {
    for (index, value) in source_priority.iter().copied().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(PriorityFieldError::InvalidSourcePriority(index));
        }
    }
    cell_provenance
        .iter()
        .enumerate()
        .map(|(cell, provenance)| {
            if provenance.is_empty() {
                return Err(PriorityFieldError::MissingProvenance(cell));
            }
            let mut maximum = 0.0_f64;
            let mut sum = 0.0;
            for &source in provenance {
                let value = source_priority
                    .get(source)
                    .copied()
                    .ok_or(PriorityFieldError::InvalidSourcePriority(source))?;
                maximum = maximum.max(value);
                sum += value;
            }
            Ok(PriorityAggregate {
                maximum,
                mean: sum / provenance.len() as f64,
            })
        })
        .collect()
}

/// Classify a regional triangle field using conservative spherical overlap.
/// Distances in the returned samples are deterministic graph-ring distances.
pub fn region_priority_field(
    mesh: &QualityMeshInput,
    targets: &[SphericalBoundaryModel],
    boundary_protection_rings: usize,
    export_corridor_rings: usize,
) -> Result<Vec<QualityPrioritySample>, PriorityFieldError> {
    validate_mesh(mesh)?;
    if targets.is_empty() {
        return Err(PriorityFieldError::EmptyTarget);
    }
    for (index, target) in targets.iter().enumerate() {
        if target.validate().is_err() {
            return Err(PriorityFieldError::InvalidBoundary(index));
        }
    }

    let polygons = cell_polygons(mesh)?;
    let mut target_cells = vec![false; mesh.cells.len()];
    let mut boundary_cells = vec![false; mesh.cells.len()];
    for (cell, polygon) in polygons.iter().enumerate() {
        for target in targets {
            let overlap = target
                .polygon_overlap(polygon)
                .map_err(|_| PriorityFieldError::InvalidCell(cell))?;
            if overlap.positive_area {
                target_cells[cell] = true;
                boundary_cells[cell] |= overlap.intersects_boundary;
            }
        }
    }
    for (cell, item) in mesh.cells.iter().enumerate() {
        if item
            .neighbors
            .iter()
            .any(|&neighbor| target_cells[neighbor] != target_cells[cell])
        {
            boundary_cells[cell] = true;
        }
    }

    let target_distance = graph_distances(mesh, &target_cells);
    let boundary_distance = graph_distances(mesh, &boundary_cells);
    Ok((0..mesh.cells.len())
        .map(|cell| {
            let to_target = distance_value(target_distance[cell]);
            let to_boundary = distance_value(boundary_distance[cell]);
            let zone = if target_cells[cell] {
                QualityZone::TargetCore
            } else if to_boundary <= boundary_protection_rings as f64 {
                QualityZone::BoundaryProtection
            } else if to_boundary
                <= boundary_protection_rings.saturating_add(export_corridor_rings) as f64
            {
                QualityZone::ExportCorridor
            } else {
                QualityZone::DeepExterior
            };
            let priority = zone_priority(
                zone,
                to_boundary,
                boundary_protection_rings,
                export_corridor_rings,
            );
            QualityPrioritySample {
                zone,
                maximum_priority: priority,
                mean_priority: priority,
                minimum_distance_to_target: to_target,
                minimum_distance_to_boundary: to_boundary,
            }
        })
        .collect())
}

/// Minimum spherical distance from each cell to any target and its boundary.
/// A crossing cell is exactly zero even when every vertex is outside.
pub fn region_spherical_distances(
    mesh: &QualityMeshInput,
    targets: &[SphericalBoundaryModel],
) -> Result<Vec<SphericalDistanceSample>, PriorityFieldError> {
    validate_mesh(mesh)?;
    if targets.is_empty() {
        return Err(PriorityFieldError::EmptyTarget);
    }
    for (index, target) in targets.iter().enumerate() {
        if target.validate().is_err() {
            return Err(PriorityFieldError::InvalidBoundary(index));
        }
    }
    cell_polygons(mesh)?
        .iter()
        .enumerate()
        .map(|(cell, polygon)| {
            let mut target_km = f64::INFINITY;
            let mut boundary_km = f64::INFINITY;
            for target in targets {
                let overlap = target
                    .polygon_overlap(polygon)
                    .map_err(|_| PriorityFieldError::InvalidCell(cell))?;
                if overlap.positive_area {
                    target_km = 0.0;
                    if overlap.intersects_boundary {
                        boundary_km = 0.0;
                    }
                }
                for &(lon, lat) in polygon {
                    if target.contains(lon, lat) {
                        target_km = 0.0;
                    }
                    if let Some(distance) = target.distance_to_boundary_radians(lon, lat) {
                        let distance = distance * EARTH_RADIUS_KM;
                        boundary_km = boundary_km.min(distance);
                        if !target.contains(lon, lat) {
                            target_km = target_km.min(distance);
                        }
                    }
                }
            }
            Ok(SphericalDistanceSample {
                target_km,
                boundary_km,
            })
        })
        .collect()
}

/// Build the signed coast distance and protect both sides of the coast.
pub fn coast_priority_field(
    mesh: &QualityMeshInput,
    land_fraction: &[f64],
    target: CoastTarget,
    coast_protection_rings: usize,
    deep_exterior_start_rings: usize,
) -> Result<Vec<CoastPrioritySample>, PriorityFieldError> {
    validate_mesh(mesh)?;
    if land_fraction.len() != mesh.cells.len()
        || deep_exterior_start_rings <= coast_protection_rings
    {
        return Err(PriorityFieldError::LengthMismatch);
    }
    for (index, fraction) in land_fraction.iter().copied().enumerate() {
        if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
            return Err(PriorityFieldError::InvalidSourcePriority(index));
        }
    }
    let land = land_fraction
        .iter()
        .map(|&fraction| fraction >= 0.5)
        .collect::<Vec<_>>();
    let mut coast = land_fraction
        .iter()
        .map(|&fraction| fraction > 0.0 && fraction < 1.0)
        .collect::<Vec<_>>();
    for (cell, item) in mesh.cells.iter().enumerate() {
        if item
            .neighbors
            .iter()
            .any(|&neighbor| land[neighbor] != land[cell])
        {
            coast[cell] = true;
        }
    }
    let coast_distance = graph_distances(mesh, &coast);
    let target_cells = land
        .iter()
        .map(|&is_land| is_land == (target == CoastTarget::Land))
        .collect::<Vec<_>>();
    let target_distance = graph_distances(mesh, &target_cells);
    let export_width = deep_exterior_start_rings - coast_protection_rings;

    Ok((0..mesh.cells.len())
        .map(|cell| {
            let distance = coast_distance[cell].unwrap_or(usize::MAX);
            let signed = if coast[cell] {
                0
            } else if land[cell] {
                distance.min(isize::MAX as usize) as isize
            } else {
                -(distance.min(isize::MAX as usize) as isize)
            };
            let zone = if distance <= coast_protection_rings {
                QualityZone::BoundaryProtection
            } else if target_cells[cell] {
                QualityZone::TargetCore
            } else if distance < deep_exterior_start_rings {
                QualityZone::ExportCorridor
            } else {
                QualityZone::DeepExterior
            };
            let to_boundary = distance_value(coast_distance[cell]);
            let priority = zone_priority(zone, to_boundary, coast_protection_rings, export_width);
            CoastPrioritySample {
                quality: QualityPrioritySample {
                    zone,
                    maximum_priority: priority,
                    mean_priority: priority,
                    minimum_distance_to_target: distance_value(target_distance[cell]),
                    minimum_distance_to_boundary: to_boundary,
                },
                signed_distance_rings: signed,
            }
        })
        .collect())
}

fn validate_mesh(mesh: &QualityMeshInput) -> Result<(), PriorityFieldError> {
    for (cell, item) in mesh.cells.iter().enumerate() {
        if item.vertices.len() < 3
            || item.vertices.iter().any(|&vertex| {
                mesh.vertices.get(vertex).is_none_or(|point| {
                    !point.x.is_finite()
                        || !point.y.is_finite()
                        || !(-90.0..=90.0).contains(&point.y)
                })
            })
        {
            return Err(PriorityFieldError::InvalidCell(cell));
        }
        if let Some(&neighbor) = item
            .neighbors
            .iter()
            .find(|&&neighbor| neighbor >= mesh.cells.len())
        {
            return Err(PriorityFieldError::InvalidNeighbor { cell, neighbor });
        }
        if let Some(&neighbor) = item
            .neighbors
            .iter()
            .find(|&&neighbor| !mesh.cells[neighbor].neighbors.contains(&cell))
        {
            return Err(PriorityFieldError::NonReciprocalNeighbor { cell, neighbor });
        }
    }
    Ok(())
}

fn cell_polygons(mesh: &QualityMeshInput) -> Result<Vec<Vec<(f64, f64)>>, PriorityFieldError> {
    mesh.cells
        .iter()
        .enumerate()
        .map(|(cell, item)| {
            item.vertices
                .iter()
                .map(|&vertex| {
                    mesh.vertices
                        .get(vertex)
                        .map(|point| (point.x, point.y))
                        .ok_or(PriorityFieldError::InvalidCell(cell))
                })
                .collect()
        })
        .collect()
}

fn graph_distances(mesh: &QualityMeshInput, seeds: &[bool]) -> Vec<Option<usize>> {
    let mut distance: Vec<Option<usize>> = vec![None; mesh.cells.len()];
    let mut queue = VecDeque::new();
    for (cell, &seed) in seeds.iter().enumerate() {
        if seed {
            distance[cell] = Some(0);
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        let next = distance[cell]
            .expect("queued cell has distance")
            .saturating_add(1);
        for &neighbor in &mesh.cells[cell].neighbors {
            if distance[neighbor].is_none() {
                distance[neighbor] = Some(next);
                queue.push_back(neighbor);
            }
        }
    }
    distance
}

fn distance_value(distance: Option<usize>) -> f64 {
    distance.map_or(f64::INFINITY, |value| value as f64)
}

fn zone_priority(
    zone: QualityZone,
    boundary_distance: f64,
    protection_rings: usize,
    export_rings: usize,
) -> f64 {
    match zone {
        QualityZone::TargetCore | QualityZone::BoundaryProtection | QualityZone::GlobalNeutral => {
            1.0
        }
        QualityZone::ExportCorridor => {
            (-(boundary_distance - protection_rings as f64) / export_rings.max(1) as f64).exp()
        }
        QualityZone::DeepExterior => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_boundary::{BoundaryLoop, BoundaryRole, BoundaryVertex, LoopType};
    use earthmesh_geometry::Point;

    fn boundary(rings: &[&[(f64, f64)]], hole_parent: Option<usize>) -> SphericalBoundaryModel {
        let mut vertices = Vec::new();
        let mut loops = Vec::new();
        for (index, ring) in rings.iter().enumerate() {
            let start = vertices.len();
            vertices.extend(ring.iter().map(|&(lon, lat)| BoundaryVertex {
                lon_degrees: lon,
                lat_degrees: lat,
                pinned: false,
            }));
            loops.push(BoundaryLoop::counter_clockwise(
                if index == 1 && hole_parent.is_some() {
                    LoopType::Hole
                } else {
                    LoopType::Outer
                },
                BoundaryRole::HardDomain,
                (start..vertices.len()).collect(),
                if index == 1 { hole_parent } else { None },
            ));
        }
        SphericalBoundaryModel { vertices, loops }
    }

    fn one_cell(points: &[(f64, f64)]) -> QualityMeshInput {
        QualityMeshInput {
            vertices: points.iter().map(|&(x, y)| Point::new(x, y)).collect(),
            cells: vec![crate::QualityCell {
                vertices: (0..points.len()).collect(),
                refine_level: None,
                neighbors: Vec::new(),
            }],
        }
    }

    #[test]
    fn centroid_outside_but_overlap_target_is_protected() {
        let target = boundary(&[&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]], None);
        let mesh = one_cell(&[(-1.0, 0.4), (-1.0, 0.6), (0.1, 0.5)]);
        let field = region_priority_field(&mesh, &[target], 1, 1).unwrap();
        assert_eq!(field[0].zone, QualityZone::TargetCore);
        assert_eq!(field[0].maximum_priority, 1.0);
    }

    #[test]
    fn multipart_region_uses_the_max_target_priority() {
        let target = boundary(
            &[
                &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
                &[(10.0, 0.0), (11.0, 0.0), (11.0, 1.0), (10.0, 1.0)],
            ],
            None,
        );
        let mesh = one_cell(&[(10.2, 0.2), (10.8, 0.2), (10.5, 0.8)]);
        assert_eq!(
            region_priority_field(&mesh, &[target], 1, 1).unwrap()[0].zone,
            QualityZone::TargetCore
        );
    }

    #[test]
    fn polygon_with_hole_does_not_promote_the_hole() {
        let target = boundary(
            &[
                &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
                &[(0.25, 0.25), (0.75, 0.25), (0.75, 0.75), (0.25, 0.75)],
            ],
            Some(0),
        );
        let mesh = one_cell(&[(0.4, 0.4), (0.6, 0.4), (0.5, 0.6)]);
        assert_eq!(
            region_priority_field(&mesh, std::slice::from_ref(&target), 1, 1).unwrap()[0].zone,
            QualityZone::DeepExterior
        );

        let crossing = one_cell(&[(0.2, 0.2), (0.8, 0.2), (0.5, 0.8)]);
        let field = region_priority_field(&crossing, std::slice::from_ref(&target), 1, 1).unwrap();
        assert_eq!(field[0].minimum_distance_to_boundary, 0.0);
        assert_eq!(
            region_spherical_distances(&crossing, &[target]).unwrap()[0].boundary_km,
            0.0
        );
    }

    #[test]
    fn dateline_and_polar_regions_use_spherical_geometry() {
        let dateline = boundary(
            &[&[
                (170.0, -10.0),
                (-170.0, -10.0),
                (-170.0, 10.0),
                (170.0, 10.0),
            ]],
            None,
        );
        let dateline_mesh = one_cell(&[(169.0, -1.0), (169.0, 1.0), (171.0, 0.0)]);
        assert_eq!(
            region_priority_field(&dateline_mesh, &[dateline], 1, 1).unwrap()[0].zone,
            QualityZone::TargetCore
        );

        let polar = boundary(
            &[&[(0.0, 80.0), (90.0, 80.0), (180.0, 80.0), (-90.0, 80.0)]],
            None,
        );
        let polar_mesh = one_cell(&[(0.0, 79.0), (20.0, 82.0), (-20.0, 82.0)]);
        assert_eq!(
            region_priority_field(&polar_mesh, &[polar], 1, 1).unwrap()[0].zone,
            QualityZone::TargetCore
        );
    }

    #[test]
    fn land_coast_both_sides_protected() {
        let mesh = QualityMeshInput {
            vertices: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(0.0, 1.0),
            ],
            cells: (0_usize..7)
                .map(|cell| crate::QualityCell {
                    vertices: vec![0, 1, 2],
                    refine_level: None,
                    neighbors: [cell.checked_sub(1), (cell + 1 < 7).then_some(cell + 1)]
                        .into_iter()
                        .flatten()
                        .collect(),
                })
                .collect(),
        };
        let field = coast_priority_field(
            &mesh,
            &[1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0],
            CoastTarget::Land,
            1,
            2,
        )
        .unwrap();
        assert_eq!(field[1].quality.zone, QualityZone::TargetCore);
        assert_eq!(field[2].quality.zone, QualityZone::BoundaryProtection);
        assert_eq!(field[3].quality.zone, QualityZone::BoundaryProtection);
        assert_eq!(field[4].quality.zone, QualityZone::BoundaryProtection);
        assert_eq!(field[5].quality.zone, QualityZone::BoundaryProtection);
        assert_eq!(field[6].quality.zone, QualityZone::DeepExterior);
        assert!(field[2].signed_distance_rings > 0);
        assert!(field[5].signed_distance_rings < 0);
    }

    #[test]
    fn source_priority_aggregation_keeps_max_and_mean() {
        assert_eq!(
            aggregate_source_priority(&[0.0, 0.5, 1.0], &[vec![0, 2], vec![1]]).unwrap(),
            vec![
                PriorityAggregate {
                    maximum: 1.0,
                    mean: 0.5
                },
                PriorityAggregate {
                    maximum: 0.5,
                    mean: 0.5
                }
            ]
        );
    }
}
