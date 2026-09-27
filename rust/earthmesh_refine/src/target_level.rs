//! The demand as a point query: how deep does this point ask to be?
//!
//! Every demand a project can state -- named regions, the criteria's circles,
//! the gradient-limited h-field -- answers the same question at a point. A
//! backend that marks cells by where they are (red-green) reads the demand
//! only through this, so it serves whichever form the project used. A backend
//! that builds from a region's shape (Method-C's seeds and perimeters) keeps
//! reading the shapes; this does not replace them.
//!
//! The region form is exact: it is the containment test the markings always
//! used, not a rasterisation of it, so reading regions through this interface
//! changes no marking.

use std::io;

use earthmesh_hfield::HField;
use earthmesh_mesh::{LonLatDegrees, RefinementRegion, RefinementRegionIndex};

/// A demand that says, for any point, whether it asks to be refined to at
/// least a given level.
pub trait TargetLevelField: Sync {
    /// Whether `point` asks to be at least `level` deep.
    fn demands(&self, point: LonLatDegrees, level: usize) -> io::Result<bool>;

    /// Whether any point asks to be at least `level` deep. `false` ends a
    /// level loop: nothing deeper will be asked either.
    fn demands_anywhere(&self, level: usize) -> bool;

    /// Whether every point that asks for level L also asks for every level
    /// above it, with the transition between levels already graded -- so a
    /// deeper demand always sits inside a shallower one. True of the h-field;
    /// not guaranteed of criteria circles, which are planned level by level.
    fn nests_by_construction(&self) -> bool {
        false
    }

    /// The deepest level `point` asks for, zero where it asks for none. What a
    /// target/actual reconciliation compares each cell's depth against.
    fn target_level(&self, point: LonLatDegrees) -> io::Result<usize> {
        let mut level = 0;
        while self.demands_anywhere(level + 1) && self.demands(point, level + 1)? {
            level += 1;
        }
        Ok(level)
    }
}

/// Named regions and criteria circles, each carrying its own level.
pub struct RegionTargets<'a> {
    index: RefinementRegionIndex<'a>,
    deepest: usize,
}

impl<'a> RegionTargets<'a> {
    pub fn new(regions: &'a [RefinementRegion]) -> Self {
        Self {
            index: RefinementRegionIndex::new(regions),
            deepest: regions
                .iter()
                .map(RefinementRegion::level)
                .max()
                .unwrap_or(0),
        }
    }
}

impl TargetLevelField for RegionTargets<'_> {
    fn demands(&self, point: LonLatDegrees, level: usize) -> io::Result<bool> {
        Ok(self.index.contains_lonlat_canonical(point, level))
    }

    fn demands_anywhere(&self, level: usize) -> bool {
        self.deepest >= level
    }
}

/// The gradient-limited cell-width field, quantised to levels the way
/// Method-C's h-field route quantises it.
pub struct HfieldTargets<'a> {
    field: &'a HField,
    base_m: f64,
    max_level: u8,
    deepest: usize,
}

impl<'a> HfieldTargets<'a> {
    pub fn new(field: &'a HField, base_m: f64, max_level: u8) -> io::Result<Self> {
        let deepest = field
            .level_map(base_m, max_level)?
            .into_iter()
            .max()
            .map_or(0, usize::from);
        Ok(Self {
            field,
            base_m,
            max_level,
            deepest,
        })
    }
}

impl TargetLevelField for HfieldTargets<'_> {
    fn demands(&self, point: LonLatDegrees, level: usize) -> io::Result<bool> {
        let target = self.field.try_level_at(
            point.lon_degrees,
            point.lat_degrees,
            self.base_m,
            self.max_level,
        )?;
        Ok(usize::from(target) >= level)
    }

    fn demands_anywhere(&self, level: usize) -> bool {
        self.deepest >= level
    }

    fn nests_by_construction(&self) -> bool {
        true
    }

    fn target_level(&self, point: LonLatDegrees) -> io::Result<usize> {
        self.field
            .try_level_at(
                point.lon_degrees,
                point.lat_degrees,
                self.base_m,
                self.max_level,
            )
            .map(usize::from)
    }
}

/// One circle the criteria route emitted, at the level of the pass that
/// emitted it. Covers what lies within `radius_meters` great-circle distance
/// of its centre -- the metric the route records, which is not the canonical
/// region containment `RegionTargets` uses.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeveledCircle {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
    pub radius_meters: f64,
    pub level: usize,
}

impl LeveledCircle {
    pub fn covers(&self, lon_degrees: f64, lat_degrees: f64) -> bool {
        earthmesh_hfield::great_circle_distance_m(
            self.lon_degrees,
            self.lat_degrees,
            lon_degrees,
            lat_degrees,
        ) <= self.radius_meters
    }
}

/// Emitted criteria circles: a point asks for the deepest level whose circles
/// cover it.
///
/// Circles are bucketed by one-degree longitude/latitude cells, so a point
/// tests only the circles whose bounding box reaches its cell. Every cell a
/// covered point can lie in is listed conservatively and the exact
/// great-circle test still decides, so the answer is the full scan's. Without
/// it a quality run on a global red-green mesh tested every cell against every
/// circle: 1.3e11 great-circle distances, single-threaded, hours.
pub struct CircleTargets<'a> {
    circles: &'a [LeveledCircle],
    buckets: Vec<Vec<u32>>,
    deepest: usize,
}

impl<'a> CircleTargets<'a> {
    const NLON: usize = 360;
    const NLAT: usize = 180;
    /// Degrees added to every box, far above floating-point noise in the exact
    /// test and far below a bucket.
    const MARGIN_DEGREES: f64 = 0.01;

    pub fn new(circles: &'a [LeveledCircle]) -> Self {
        let mut buckets = vec![Vec::new(); Self::NLON * Self::NLAT];
        for (index, circle) in circles.iter().enumerate() {
            let angle = circle.radius_meters.max(0.0) / earthmesh_hfield::EARTH_RADIUS_METERS;
            let angle_degrees = angle.to_degrees() + Self::MARGIN_DEGREES;
            let south = circle.lat_degrees - angle_degrees;
            let north = circle.lat_degrees + angle_degrees;
            // A circle reaching a pole, or one wide enough that its longitude
            // extent is not bounded, spans every longitude.
            let cos_lat = circle.lat_degrees.to_radians().cos();
            let lon_half = if north >= 90.0 || south <= -90.0 || angle.sin() >= cos_lat {
                None
            } else {
                Some((angle.sin() / cos_lat).asin().to_degrees() + Self::MARGIN_DEGREES)
            };
            let lat_rows = Self::lat_row(south)..=Self::lat_row(north);
            let lon_cols: Vec<usize> = match lon_half {
                Some(half) if 2.0 * half < 359.0 => {
                    let west = (circle.lon_degrees - half).floor() as i64;
                    let east = (circle.lon_degrees + half).floor() as i64;
                    (west..=east)
                        .map(|lon| (lon + 180).rem_euclid(Self::NLON as i64) as usize)
                        .collect()
                }
                _ => (0..Self::NLON).collect(),
            };
            for row in lat_rows {
                for &col in &lon_cols {
                    buckets[row * Self::NLON + col].push(index as u32);
                }
            }
        }
        Self {
            circles,
            buckets,
            deepest: circles.iter().map(|circle| circle.level).max().unwrap_or(0),
        }
    }

    fn lat_row(lat_degrees: f64) -> usize {
        ((lat_degrees + 90.0).floor().max(0.0) as usize).min(Self::NLAT - 1)
    }

    fn lon_col(lon_degrees: f64) -> usize {
        ((lon_degrees + 180.0).floor() as i64).rem_euclid(Self::NLON as i64) as usize
    }

    fn deepest_covering(&self, lon_degrees: f64, lat_degrees: f64) -> usize {
        let bucket = Self::lat_row(lat_degrees) * Self::NLON + Self::lon_col(lon_degrees);
        self.buckets[bucket]
            .iter()
            .map(|&index| &self.circles[index as usize])
            .filter(|circle| circle.covers(lon_degrees, lat_degrees))
            .map(|circle| circle.level)
            .max()
            .unwrap_or(0)
    }
}

impl TargetLevelField for CircleTargets<'_> {
    fn demands(&self, point: LonLatDegrees, level: usize) -> io::Result<bool> {
        Ok(self.deepest_covering(point.lon_degrees, point.lat_degrees) >= level)
    }

    fn demands_anywhere(&self, level: usize) -> bool {
        self.deepest >= level
    }

    fn target_level(&self, point: LonLatDegrees) -> io::Result<usize> {
        Ok(self.deepest_covering(point.lon_degrees, point.lat_degrees))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_answer_with_the_containment_the_markings_use() {
        let regions = [RefinementRegion::Circle {
            center: LonLatDegrees::new(10.0, 20.0),
            radius_meters: 200_000.0,
            level: 2,
        }];
        let targets = RegionTargets::new(&regions);
        let index = RefinementRegionIndex::new(&regions);
        for point in [
            LonLatDegrees::new(10.0, 20.0),
            LonLatDegrees::new(11.5, 20.0),
            LonLatDegrees::new(13.0, 20.0),
        ] {
            for level in 1..=3 {
                assert_eq!(
                    targets.demands(point, level).unwrap(),
                    index.contains_lonlat_canonical(point, level),
                );
            }
        }
        assert!(targets.demands(LonLatDegrees::new(10.0, 20.0), 2).unwrap());
        assert!(!targets.demands(LonLatDegrees::new(10.0, 20.0), 3).unwrap());
        assert!(targets.demands_anywhere(2));
        assert!(!targets.demands_anywhere(3));
        assert!(!RegionTargets::new(&[]).demands_anywhere(1));
    }

    /// Tests every circle. The oracle for `CircleTargets`' buckets.
    fn full_scan(circles: &[LeveledCircle], lon: f64, lat: f64) -> usize {
        circles
            .iter()
            .filter(|circle| circle.covers(lon, lat))
            .map(|circle| circle.level)
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn circle_targets_answer_exactly_as_the_full_scan() {
        // Circles at the poles, across the antimeridian, near the equator and
        // wide enough to span every longitude; points on a fine global lattice
        // plus each circle's own centre and rim.
        let mut circles = Vec::new();
        let mut state = 0x9e37_79b9_7f4a_7c15_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        for level in 1..=3 {
            for _ in 0..400 {
                circles.push(LeveledCircle {
                    lon_degrees: next() * 360.0 - 180.0,
                    lat_degrees: next() * 180.0 - 90.0,
                    radius_meters: 5_000.0 + next() * 400_000.0,
                    level,
                });
            }
        }
        for (lon, lat, radius) in [
            (0.0, 90.0, 300_000.0),
            (37.0, -89.5, 120_000.0),
            (179.95, 10.0, 200_000.0),
            (-179.99, -45.0, 250_000.0),
            (180.0, 60.0, 150_000.0),
            (10.0, 0.0, 6_000_000.0),
            (0.0, 88.0, 50_000.0),
        ] {
            circles.push(LeveledCircle {
                lon_degrees: lon,
                lat_degrees: lat,
                radius_meters: radius,
                level: 4,
            });
        }
        let targets = CircleTargets::new(&circles);
        let mut points = Vec::new();
        for i in 0..720 {
            for j in 0..=360 {
                points.push((i as f64 * 0.5 - 180.0 + 0.123, j as f64 * 0.5 - 90.0));
            }
        }
        for circle in &circles {
            let angle = (circle.radius_meters / earthmesh_hfield::EARTH_RADIUS_METERS).to_degrees();
            points.push((circle.lon_degrees, circle.lat_degrees));
            points.push((
                circle.lon_degrees,
                (circle.lat_degrees + angle * 0.999).min(90.0),
            ));
            points.push((circle.lon_degrees + 360.0, circle.lat_degrees));
        }
        let mut covered = 0usize;
        for (lon, lat) in points {
            let expected = full_scan(&circles, lon, lat);
            covered += usize::from(expected > 0);
            let point = LonLatDegrees::new(lon, lat);
            assert_eq!(
                targets.target_level(point).unwrap(),
                expected,
                "({lon}, {lat})"
            );
            // The level-by-level form agrees with the direct answer.
            assert!(targets.demands(point, expected).unwrap());
            assert!(!targets.demands(point, expected + 1).unwrap());
        }
        assert!(covered > 1000, "fixture must exercise covered points");
    }

    #[test]
    fn the_default_target_level_is_the_deepest_demanded_level() {
        let regions = [
            RefinementRegion::Circle {
                center: LonLatDegrees::new(10.0, 20.0),
                radius_meters: 400_000.0,
                level: 1,
            },
            RefinementRegion::Circle {
                center: LonLatDegrees::new(10.0, 20.0),
                radius_meters: 100_000.0,
                level: 3,
            },
        ];
        let targets = RegionTargets::new(&regions);
        assert_eq!(
            targets
                .target_level(LonLatDegrees::new(10.0, 20.0))
                .unwrap(),
            3
        );
        assert_eq!(
            targets
                .target_level(LonLatDegrees::new(12.0, 20.0))
                .unwrap(),
            1
        );
        assert_eq!(
            targets
                .target_level(LonLatDegrees::new(-90.0, 0.0))
                .unwrap(),
            0
        );
    }
}
