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
}
