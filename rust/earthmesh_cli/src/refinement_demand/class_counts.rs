//! Per-class prefix sums over a land-type window.
//!
//! The three neighbourhood criteria all ask the same kind of question: over the
//! square of `radius_cells` around this cell, how many cells carry each class?
//! Asked directly that is `(2r+1)^2` reads per cell, and `r` grows with the
//! generation being refined, not with the raster. On the production IGBP raster
//! (240 cells per degree) refining an NXP 81 mesh, `r` is 107 and the square is
//! 46225 -- times 3.7 billion cells, about 1.7e14 reads for one criterion on one
//! pass. Measured by sampling a real run: every sample landed inside that loop,
//! and the run would have taken days.
//!
//! A prefix sum per class answers the same question in four reads. Building the
//! table costs one pass over the window per class, and each query is then
//! `count(class) = S[b][r] - S[t][r] - S[b][l] + S[t][l]`. Class ids are small
//! and dense (IGBP is under 20), so the table is bounded by the data rather than
//! by the radius.
//!
//! **The counts are identical, not approximate.** These are integer cell counts
//! over exactly the cells `LandtypeWindow::value_at_global` would have returned:
//! the window is clipped the same way, and cells outside it are excluded from
//! the total rather than counted as anything. Nothing here rounds, samples, or
//! subsets — only the order of summation changes, and integers do not care.

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::mkgrd_data_preprocess_source::LandtypeWindow;

/// Prefix sums over a land-type window, one plane per class present.
pub(super) struct ClassPrefixSums {
    /// Window width, in the window's own zero-based offsets. The height is
    /// implied by the plane length and never needed on its own.
    nlons: usize,
    bounds: AreaJudgeSourceBounds,
    /// Class value for each plane, ascending.
    classes: Vec<i8>,
    /// `(nlats + 1) * (nlons + 1)` inclusive prefix sums per plane.
    planes: Vec<Vec<u32>>,
}

impl ClassPrefixSums {
    pub(super) fn build(window: &LandtypeWindow) -> Self {
        let (nlons, nlats) = (window.nlons, window.nlats);
        let mut classes: Vec<i8> = Vec::new();
        for value in &window.values {
            if !classes.contains(value) {
                classes.push(*value);
            }
        }
        classes.sort_unstable();

        let stride = nlons + 1;
        let planes = classes
            .iter()
            .map(|&class| {
                let mut plane = vec![0u32; stride * (nlats + 1)];
                for lat in 0..nlats {
                    let mut row = 0u32;
                    for lon in 0..nlons {
                        // The window stores longitude-major, as `value_at_global`
                        // reads it.
                        if window.values[lon * nlats + lat] == class {
                            row += 1;
                        }
                        plane[(lat + 1) * stride + lon + 1] = plane[lat * stride + lon + 1] + row;
                    }
                }
                plane
            })
            .collect();

        Self {
            nlons,
            bounds: window.bounds,
            classes,
            planes,
        }
    }

    /// Classes present anywhere in the window, ascending.
    pub(super) fn classes(&self) -> &[i8] {
        &self.classes
    }

    /// The window offsets a `radius_cells` square around a global index covers,
    /// clipped to the window exactly as `value_at_global` clips it.
    fn clipped_box(
        &self,
        lon_index: usize,
        lat_index: usize,
        radius_cells: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let lon_lo = lon_index
            .saturating_sub(radius_cells)
            .max(self.bounds.minlon_source);
        let lon_hi = (lon_index + radius_cells).min(self.bounds.maxlon_source);
        let lat_lo = lat_index
            .saturating_sub(radius_cells)
            .max(self.bounds.maxlat_source);
        let lat_hi = (lat_index + radius_cells).min(self.bounds.minlat_source);
        if lon_lo > lon_hi || lat_lo > lat_hi {
            return None;
        }
        Some((
            lon_lo - self.bounds.minlon_source,
            lon_hi - self.bounds.minlon_source,
            lat_lo - self.bounds.maxlat_source,
            lat_hi - self.bounds.maxlat_source,
        ))
    }

    /// Cells of `plane` inside the box, by the four-corner rule.
    fn plane_count(plane: &[u32], stride: usize, b: (usize, usize, usize, usize)) -> u32 {
        let (lon_lo, lon_hi, lat_lo, lat_hi) = b;
        plane[(lat_hi + 1) * stride + lon_hi + 1] + plane[lat_lo * stride + lon_lo]
            - plane[lat_lo * stride + lon_hi + 1]
            - plane[(lat_hi + 1) * stride + lon_lo]
    }

    /// Per-class counts in the neighbourhood, in `classes()` order, and the
    /// total number of cells the neighbourhood actually held.
    pub(super) fn counts_at(
        &self,
        lon_index: usize,
        lat_index: usize,
        radius_cells: usize,
        out: &mut Vec<u32>,
    ) -> u32 {
        out.clear();
        let Some(b) = self.clipped_box(lon_index, lat_index, radius_cells) else {
            out.resize(self.classes.len(), 0);
            return 0;
        };
        let stride = self.nlons + 1;
        let mut total = 0u32;
        for plane in &self.planes {
            let count = Self::plane_count(plane, stride, b);
            total += count;
            out.push(count);
        }
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(nlons: usize, nlats: usize, value_at: impl Fn(usize, usize) -> i8) -> LandtypeWindow {
        let bounds = AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: nlons,
            maxlat_source: 1,
            minlat_source: nlats,
        };
        let mut values = vec![0i8; nlons * nlats];
        for lon in 0..nlons {
            for lat in 0..nlats {
                values[lon * nlats + lat] = value_at(lon, lat);
            }
        }
        LandtypeWindow {
            bounds,
            nlons,
            nlats,
            values,
        }
    }

    /// What the criteria used to do, kept here as the thing to agree with.
    fn brute_force(
        window: &LandtypeWindow,
        lon: usize,
        lat: usize,
        radius: usize,
    ) -> (Vec<(i8, u32)>, u32) {
        let mut counts: Vec<(i8, u32)> = Vec::new();
        let mut total = 0u32;
        for neighbour_lat in lat.saturating_sub(radius)..=lat + radius {
            for neighbour_lon in lon.saturating_sub(radius)..=lon + radius {
                let Some(value) = window.value_at_global(neighbour_lon, neighbour_lat) else {
                    continue;
                };
                total += 1;
                match counts.iter_mut().find(|(class, _)| *class == value) {
                    Some((_, count)) => *count += 1,
                    None => counts.push((value, 1)),
                }
            }
        }
        counts.sort_unstable();
        (counts, total)
    }

    #[test]
    fn every_neighbourhood_matches_the_loop_it_replaces() {
        // Several classes, an uneven layout, and radii that run off every edge —
        // the clipping is where a prefix-sum table and a nested loop are most
        // likely to disagree.
        let window = window(11, 7, |lon, lat| ((lon * 3 + lat * 5) % 4) as i8);
        let sums = ClassPrefixSums::build(&window);
        let mut out = Vec::new();

        for radius in [1usize, 2, 3, 5, 20] {
            for lon in 1..=11 {
                for lat in 1..=7 {
                    let total = sums.counts_at(lon, lat, radius, &mut out);
                    let (expected, expected_total) = brute_force(&window, lon, lat, radius);
                    assert_eq!(total, expected_total, "total at {lon},{lat} r{radius}");

                    let got: Vec<(i8, u32)> = sums
                        .classes()
                        .iter()
                        .copied()
                        .zip(out.iter().copied())
                        .filter(|(_, count)| *count > 0)
                        .collect();
                    assert_eq!(got, expected, "counts at {lon},{lat} r{radius}");
                }
            }
        }
    }

    #[test]
    fn a_single_class_window_still_counts_the_clipped_area() {
        let window = window(4, 3, |_, _| 7);
        let sums = ClassPrefixSums::build(&window);
        let mut out = Vec::new();

        assert_eq!(sums.classes(), &[7]);
        // Radius 1 at the corner sees a 2x2 clip, not 3x3.
        assert_eq!(sums.counts_at(1, 1, 1, &mut out), 4);
        assert_eq!(out, vec![4]);
        // Radius large enough to cover everything sees the whole window.
        assert_eq!(sums.counts_at(2, 2, 99, &mut out), 12);
    }
}
