//! The merge-if-homogeneous requirement (design H1,
//! `docs/certified_mesh/heterogeneity_merge.md`). Reverse coarsening starts
//! at the finest lattice and forms a parent from its four children; here a
//! parent may be formed only if its footprint is homogeneous enough -- every
//! criterion passes on the samples it holds -- and all four children were
//! formed. A finest face then requires the level of its topmost formed
//! ancestor. Bottom up on the nested lattice: a parent's statistics are its
//! children's summed, so the field costs one pass over the samples and one
//! over the faces.
//!
//! Formed-ness is closed downwards (a formed parent's children were formed),
//! so the requirement is well defined, and unlike a top-down test ("refine
//! where the parent is heterogeneous") it never stops above a heterogeneous
//! child whose parent happens to look homogeneous.

use crate::mother_grid::lattice::locate;
use crate::mother_grid::TriangleAddress;
use std::collections::{BTreeMap, BTreeSet};

/// What a criterion measures over the samples a face holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Statistic {
    /// Standard deviation of a continuous layer, in its units: a face passes
    /// at or below the threshold.
    StandardDeviation,
    /// Standard deviation over the absolute mean: passes at or below the
    /// threshold.
    CoefficientOfVariation,
    /// Share of the most frequent class of a categorical layer (values are
    /// class codes): passes at or above the threshold.
    Purity,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Criterion {
    pub layer: usize,
    pub statistic: Statistic,
    pub threshold: f64,
}

/// The samples one face holds: count, and per layer the sum and the sum of
/// squares of the values less the base face's first value (shifted, so a
/// DEM's squares do not swamp its variance), and class counts for the
/// layers a purity criterion reads.
#[derive(Debug, Clone, Default)]
struct Moments {
    count: u64,
    sums: Vec<f64>,
    squares: Vec<f64>,
    classes: Vec<BTreeMap<i64, u64>>,
}

impl Moments {
    fn new(layers: usize, categorical: usize) -> Self {
        Self {
            count: 0,
            sums: vec![0.0; layers],
            squares: vec![0.0; layers],
            classes: vec![BTreeMap::new(); categorical],
        }
    }

    fn add(&mut self, other: &Self) {
        self.count += other.count;
        for (sum, value) in self.sums.iter_mut().zip(&other.sums) {
            *sum += value;
        }
        for (square, value) in self.squares.iter_mut().zip(&other.squares) {
            *square += value;
        }
        for (classes, theirs) in self.classes.iter_mut().zip(&other.classes) {
            for (&class, &count) in theirs {
                *classes.entry(class).or_default() += count;
            }
        }
    }
}

/// The required level of every finest lattice face under some base faces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeterogeneityField {
    base_n: usize,
    levels: usize,
    base_faces: usize,
    /// For each base face with a demand, the level each finest descendant
    /// requires, in quadtree order (`quadtree_index`); the others require 0.
    required: BTreeMap<TriangleAddress, Vec<u8>>,
    /// Leaves of the criterion-only mesh at each level, 0 (base) to `levels`.
    leaves: Vec<usize>,
}

/// The index of a finest face among its base face's descendants: the child
/// positions (`children_2_to_1` order) along the way down, in base 4.
fn quadtree_index(mut face: TriangleAddress, base_n: usize) -> Result<usize, String> {
    let mut index = 0usize;
    let mut weight = 1usize;
    while face.n > base_n {
        let parent = face
            .parent_2_to_1()
            .ok_or_else(|| format!("face {face:?} has no parent"))?;
        let position = parent
            .children_2_to_1()
            .and_then(|children| children.iter().position(|&child| child == face))
            .ok_or_else(|| format!("face {face:?} is not a child of {parent:?}"))?;
        index += position * weight;
        weight *= 4;
        face = parent;
    }
    Ok(index)
}

fn base_ancestor(mut face: TriangleAddress, base_n: usize) -> Option<TriangleAddress> {
    while face.n > base_n {
        face = face.parent_2_to_1()?;
    }
    (face.n == base_n).then_some(face)
}

impl HeterogeneityField {
    /// The field under `base_faces` (of level `base_n`), refined `levels`
    /// times, from samples -- a unit vector and one value per layer each.
    /// Samples outside the base faces are ignored. A face holding fewer
    /// than `minimum_samples` gives no evidence against forming it.
    pub fn build(
        base_n: usize,
        levels: usize,
        base_faces: &BTreeSet<TriangleAddress>,
        layers: usize,
        samples: impl IntoIterator<Item = ([f64; 3], Vec<f64>)>,
        criteria: &[Criterion],
        minimum_samples: u64,
    ) -> Result<Self, String> {
        if levels > 12 {
            return Err(format!(
                "{levels} levels are more than a base face can index"
            ));
        }
        if let Some(face) = base_faces.iter().find(|face| face.n != base_n) {
            return Err(format!("{face:?} is not a level-{base_n} face"));
        }
        if let Some(criterion) = criteria.iter().find(|criterion| criterion.layer >= layers) {
            return Err(format!(
                "criterion {criterion:?} reads a layer past {layers}"
            ));
        }
        let fine_n = base_n << levels;
        // Categorical layers: those a purity criterion reads, numbered.
        let categorical = criteria
            .iter()
            .filter(|criterion| criterion.statistic == Statistic::Purity)
            .map(|criterion| criterion.layer)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let leaves_per_face = 1usize << (2 * levels);

        // Samples by base face, each at its finest face's quadtree index.
        let mut grouped = BTreeMap::<TriangleAddress, Vec<(usize, Vec<f64>)>>::new();
        for (point, values) in samples {
            if values.len() != layers {
                return Err(format!(
                    "a sample has {} values for {layers} layers",
                    values.len()
                ));
            }
            let Some(finest) = locate(fine_n, point) else {
                continue;
            };
            let Some(base) = base_ancestor(finest, base_n) else {
                continue;
            };
            if base_faces.contains(&base) {
                grouped
                    .entry(base)
                    .or_default()
                    .push((quadtree_index(finest, base_n)?, values));
            }
        }

        let mut required = BTreeMap::new();
        let mut leaves = vec![0usize; levels + 1];
        leaves[0] = base_faces.len() - grouped.len();
        for (base, samples) in grouped {
            let shift = samples[0].1.clone();
            // Depth `levels`: the finest faces.
            let mut moments = vec![Moments::new(layers, categorical.len()); leaves_per_face];
            for (index, values) in &samples {
                let face = &mut moments[*index];
                face.count += 1;
                for layer in 0..layers {
                    let value = values[layer] - shift[layer];
                    face.sums[layer] += value;
                    face.squares[layer] += value * value;
                }
                for (slot, &layer) in categorical.iter().enumerate() {
                    *face.classes[slot]
                        .entry(values[layer].round() as i64)
                        .or_default() += 1;
                }
            }
            let passes = |face: &Moments| -> bool {
                if face.count < minimum_samples.max(1) {
                    return true;
                }
                let count = face.count as f64;
                criteria.iter().all(|criterion| {
                    let layer = criterion.layer;
                    let mean = face.sums[layer] / count;
                    let variance = (face.squares[layer] / count - mean * mean).max(0.0);
                    match criterion.statistic {
                        Statistic::StandardDeviation => variance.sqrt() <= criterion.threshold,
                        Statistic::CoefficientOfVariation => {
                            let absolute = (mean + shift[layer]).abs();
                            if absolute == 0.0 {
                                variance == 0.0
                            } else {
                                variance.sqrt() / absolute <= criterion.threshold
                            }
                        }
                        Statistic::Purity => {
                            let slot = categorical
                                .iter()
                                .position(|&categorical| categorical == layer)
                                .expect("purity layers are listed");
                            let top = face.classes[slot].values().copied().max().unwrap_or(0);
                            top as f64 / count >= criterion.threshold
                        }
                    }
                })
            };
            // Up the levels: formed[depth][index].
            let mut formed = vec![vec![true; leaves_per_face]];
            for depth in (0..levels).rev() {
                let width = 1usize << (2 * depth);
                let children = formed.last().expect("one level below");
                let mut parents = Vec::with_capacity(width);
                let mut parent_formed = Vec::with_capacity(width);
                for index in 0..width {
                    let mut parent = Moments::new(layers, categorical.len());
                    for child in 0..4 {
                        parent.add(&moments[4 * index + child]);
                    }
                    let all_children = (0..4).all(|child| children[4 * index + child]);
                    parent_formed.push(all_children && passes(&parent));
                    parents.push(parent);
                }
                moments = parents;
                formed.push(parent_formed);
            }
            formed.reverse(); // formed[depth], depth 0 = the base face
            let mut levels_here = vec![0u8; leaves_per_face];
            for (finest, level) in levels_here.iter_mut().enumerate() {
                let depth = (0..=levels)
                    .find(|&depth| formed[depth][finest >> (2 * (levels - depth))])
                    .expect("finest faces are formed");
                *level = depth as u8;
            }
            // Leaves: formed faces whose parent is not.
            for depth in 0..=levels {
                for index in 0..1usize << (2 * depth) {
                    if formed[depth][index] && (depth == 0 || !formed[depth - 1][index >> 2]) {
                        leaves[depth] += 1;
                    }
                }
            }
            if levels_here.iter().any(|&level| level > 0) {
                required.insert(base, levels_here);
            }
        }
        Ok(Self {
            base_n,
            levels,
            base_faces: base_faces.len(),
            required,
            leaves,
        })
    }

    /// The level a finest face requires: 0 under a base face without a
    /// demand or outside the field.
    pub fn required_level(&self, finest: TriangleAddress) -> Result<usize, String> {
        if finest.n != self.base_n << self.levels {
            return Err(format!("{finest:?} is not a finest face of this field"));
        }
        let base = base_ancestor(finest, self.base_n)
            .ok_or_else(|| format!("{finest:?} has no base face"))?;
        Ok(match self.required.get(&base) {
            Some(levels) => levels[quadtree_index(finest, self.base_n)?] as usize,
            None => 0,
        })
    }

    /// Base faces some finest face of which requires a level above 0.
    pub fn demanding_base_faces(&self) -> impl Iterator<Item = TriangleAddress> + '_ {
        self.required.keys().copied()
    }

    /// The criterion-only mesh -- every face formed where it may be, before
    /// balance and transitions -- as leaves per level, 0 (base) first.
    pub fn leaves_per_level(&self) -> &[usize] {
        &self.leaves
    }

    /// Base faces the field covers.
    pub fn base_faces(&self) -> usize {
        self.base_faces
    }

    /// The base faces a requirement above level 0 reaches: the demanding
    /// ones and `rings` vertex rings around them -- the seeds of the extent
    /// (`on_demand::materialization_extent_from_seeds`). The graded envelope
    /// spreads a requirement `gradation_rings_per_level` finest rings per
    /// level, `reach_rings` base rings in all.
    pub fn seed_faces(&self, rings: usize) -> Result<BTreeSet<TriangleAddress>, String> {
        let mut seeds = self.required.keys().copied().collect::<BTreeSet<_>>();
        for _ in 0..rings {
            for face in seeds.clone() {
                seeds.extend(crate::mother_grid::lattice::faces_around(face)?);
            }
        }
        Ok(seeds)
    }

    /// Base rings the graded envelope's spread reaches: `gradation` finest
    /// rings per level over the field's levels, in base edges, rounded up,
    /// plus one for the faces a site's cell overlaps.
    pub fn reach_rings(&self, gradation: usize) -> usize {
        (gradation * self.levels).div_ceil(1usize << self.levels) + 1
    }

    /// The requirement of each active site of `grid` -- a mother at the
    /// field's finest level, whole or a region -- in slot order: the
    /// highest level any face at the site requires. The lattice is acute,
    /// so a site's Voronoi cell lies in its faces and none it overlaps is
    /// missed.
    pub fn required_by_site(
        &self,
        grid: &crate::mother_grid::MotherGrid,
    ) -> Result<Vec<usize>, String> {
        let mut level_at = vec![0usize; grid.mesh.vertices().len()];
        for face in grid.mesh.active_triangle_slots() {
            let address = grid.triangle_addresses[face]
                .ok_or_else(|| format!("face {face} has no address"))?;
            let level = self.required_level(address)?;
            for site in grid.mesh.triangles()[face] {
                level_at[site] = level_at[site].max(level);
            }
        }
        Ok(grid
            .mesh
            .active_vertex_slots()
            .map(|site| level_at[site])
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mother_grid::lattice::faces_around;
    use crate::mother_grid::region::descendant_faces;

    fn next(seed: &mut u64) -> u64 {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    }

    /// A point inside `face`: its corners weighted and normalized.
    fn inside(face: TriangleAddress, weights: [f64; 3]) -> [f64; 3] {
        let corners = crate::mother_grid::lattice::face_corner_points(face).unwrap();
        let mut point = [0.0; 3];
        for (corner, weight) in corners.iter().zip(weights) {
            for axis in 0..3 {
                point[axis] += corner[axis] * weight;
            }
        }
        let length = point.iter().map(|value| value * value).sum::<f64>().sqrt();
        point.map(|value| value / length)
    }

    /// The field against a direct evaluation: each face's samples counted
    /// afresh from its descendants, formed-ness by recursion, the
    /// requirement as the topmost formed ancestor.
    #[test]
    fn the_field_is_the_direct_evaluation() {
        let mut seed = 0x1357_9bdf_2468_ace0u64;
        let base_n = 6;
        let levels = 3;
        let fine_n = base_n << levels;
        let centre = locate(base_n, [0.3, 0.4, 0.866]).unwrap();
        let mut base_faces = BTreeSet::from([centre]);
        base_faces.extend(faces_around(centre).unwrap());
        let finest = descendant_faces(base_faces.iter().copied(), fine_n).unwrap();
        let finest = finest.into_iter().collect::<Vec<_>>();
        // Two layers: a smooth one with a cliff, and classes.
        let mut samples = Vec::new();
        for _ in 0..6000 {
            let face = finest[(next(&mut seed) % finest.len() as u64) as usize];
            let weights = [
                1.0 + (next(&mut seed) % 100) as f64,
                1.0 + (next(&mut seed) % 100) as f64,
                1.0 + (next(&mut seed) % 100) as f64,
            ];
            let point = inside(face, weights);
            let height = 1000.0 + 50.0 * point[0] + if point[1] > 0.39 { 40.0 } else { 0.0 };
            let class = if point[2] > 0.87 {
                3.0
            } else {
                (next(&mut seed) % 2) as f64
            };
            samples.push((point, vec![height, class]));
        }
        let criteria = [
            Criterion {
                layer: 0,
                statistic: Statistic::StandardDeviation,
                threshold: 4.0,
            },
            Criterion {
                layer: 1,
                statistic: Statistic::Purity,
                threshold: 0.7,
            },
        ];
        let field = HeterogeneityField::build(
            base_n,
            levels,
            &base_faces,
            2,
            samples.clone(),
            &criteria,
            2,
        )
        .unwrap();

        // Direct: samples per finest face, then every face's samples as the
        // union of its finest descendants'.
        let mut at = BTreeMap::<TriangleAddress, Vec<Vec<f64>>>::new();
        for (point, values) in &samples {
            at.entry(locate(fine_n, *point).unwrap())
                .or_default()
                .push(values.clone());
        }
        let held = |face: TriangleAddress| -> Vec<Vec<f64>> {
            descendant_faces([face], fine_n)
                .unwrap()
                .into_iter()
                .flat_map(|finest| at.get(&finest).cloned().unwrap_or_default())
                .collect()
        };
        let passes = |values: &[Vec<f64>]| -> bool {
            if values.len() < 2 {
                return true;
            }
            let count = values.len() as f64;
            let mean = values.iter().map(|v| v[0]).sum::<f64>() / count;
            let variance = values.iter().map(|v| (v[0] - mean).powi(2)).sum::<f64>() / count;
            let mut classes = BTreeMap::<i64, usize>::new();
            for v in values {
                *classes.entry(v[1] as i64).or_default() += 1;
            }
            let top = *classes.values().max().unwrap() as f64;
            variance.sqrt() <= 4.0 + 1.0e-9 && top / count >= 0.7
        };
        fn formed(
            face: TriangleAddress,
            fine_n: usize,
            held: &dyn Fn(TriangleAddress) -> Vec<Vec<f64>>,
            passes: &dyn Fn(&[Vec<f64>]) -> bool,
            memo: &mut BTreeMap<TriangleAddress, bool>,
        ) -> bool {
            if face.n == fine_n {
                return true;
            }
            if let Some(&known) = memo.get(&face) {
                return known;
            }
            let children = face.children_2_to_1().unwrap();
            let result = children
                .iter()
                .all(|&child| formed(child, fine_n, held, passes, memo))
                && passes(&held(face));
            memo.insert(face, result);
            result
        }
        let mut memo = BTreeMap::new();
        let mut demanding = 0;
        for &face in &finest {
            let mut chain = vec![face];
            while chain.last().unwrap().n > base_n {
                chain.push(chain.last().unwrap().parent_2_to_1().unwrap());
            }
            // Topmost formed ancestor: the last in the chain that is formed
            // when every one below it is.
            let mut level = levels;
            for (steps, &ancestor) in chain.iter().enumerate().skip(1) {
                if formed(ancestor, fine_n, &held, &passes, &mut memo) {
                    level = levels - steps;
                } else {
                    break;
                }
            }
            assert_eq!(field.required_level(face).unwrap(), level, "{face:?}");
            demanding += usize::from(level > 0);
        }
        assert!(
            demanding > 50 && demanding < finest.len() - 50,
            "{demanding} of {}",
            finest.len()
        );
        // Leaves: every finest face is under exactly one leaf.
        let covered = field
            .leaves_per_level()
            .iter()
            .enumerate()
            .map(|(level, &count)| count << (2 * (levels - level)))
            .sum::<usize>();
        assert_eq!(covered, finest.len());
    }

    /// A parent can look homogeneous while one child is not -- the other
    /// children sitting at the parent's mean. Top down, the parent would
    /// stop the refinement above that child; bottom up, the child is not
    /// formed, so neither is the parent, and the child's faces stay finer.
    #[test]
    fn a_homogeneous_parent_does_not_hide_a_heterogeneous_child() {
        let base_n = 4;
        let levels = 2;
        let base = locate(base_n, [0.1, 0.2, 0.975]).unwrap();
        let children = base.children_2_to_1().unwrap();
        let mut samples = Vec::new();
        // Child 0: half at 0, half at 10 -- standard deviation 5.
        for (k, grandchild) in children[0].children_2_to_1().unwrap().iter().enumerate() {
            let value = if k % 2 == 0 { 0.0 } else { 10.0 };
            for weights in [[1.0, 1.0, 1.0], [2.0, 1.0, 1.0]] {
                samples.push((inside(*grandchild, weights), vec![value]));
            }
        }
        // The other children: many samples at 5, the parent's mean.
        for child in &children[1..] {
            for grandchild in child.children_2_to_1().unwrap() {
                for a in 1..6 {
                    samples.push((inside(grandchild, [a as f64, 2.0, 3.0]), vec![5.0]));
                }
            }
        }
        let field = HeterogeneityField::build(
            base_n,
            levels,
            &BTreeSet::from([base]),
            1,
            samples,
            &[Criterion {
                layer: 0,
                statistic: Statistic::StandardDeviation,
                threshold: 3.0,
            }],
            2,
        )
        .unwrap();
        let fine_n = base_n << levels;
        // The parent alone is homogeneous enough: 8 samples at 0 or 10
        // among 60 at 5 give a standard deviation of about 1.8.
        // Yet child 0's faces require the finest level, the others level 1.
        for (position, child) in children.iter().enumerate() {
            for finest in descendant_faces([*child], fine_n).unwrap() {
                let expected = if position == 0 { 2 } else { 1 };
                assert_eq!(
                    field.required_level(finest).unwrap(),
                    expected,
                    "child {position}"
                );
            }
        }
        assert_eq!(field.leaves_per_level(), &[0, 3, 4]);
    }
}
