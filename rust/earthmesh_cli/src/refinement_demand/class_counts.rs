//! Per-class neighbourhood counts over a land-type window, by sliding window.
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
//! A summed-area table answers that in four reads, and is the wrong tool here: it
//! needs one plane per class over the whole window, which at global scale is
//! 236 GB for the seventeen IGBP classes. Measured the hard way -- the first
//! version of this module was exactly that, and the run it was meant to rescue
//! died allocating.
//!
//! Sliding the window costs the same time and almost no memory. Walking output
//! rows north to south, each column keeps the count of every class in the
//! `2r+1` rows currently covered; a step south adds one row and drops another.
//! Walking columns west to east, the neighbourhood total is those column counts
//! over `2r+1` columns, again by add-one-drop-one. Memory is `nlons * classes`,
//! independent of the radius and of the window's height.
//!
//! **The counts are identical, not approximate.** They are integer cell counts
//! over exactly the cells `LandtypeWindow::value_at_global` would have returned:
//! the window is clipped the same way, and cells outside it are excluded from
//! the total rather than counted as anything. Integers do not care in what order
//! they are added, so add-one-drop-one gives the same number the nested loop
//! reached by summing from scratch.

use earthmesh_mesh::AreaJudgeSourceBounds;

use crate::mkgrd_data_preprocess_source::LandtypeWindow;

/// Per-class neighbourhood counts, produced one output row at a time.
pub(super) struct ClassCounts<'a> {
    window: &'a LandtypeWindow,
    bounds: AreaJudgeSourceBounds,
    classes: Vec<i8>,
    /// Plane index for each class value, indexed by `value as usize + 128`.
    plane_of: Vec<u16>,
}

impl<'a> ClassCounts<'a> {
    pub(super) fn build(window: &'a LandtypeWindow) -> Self {
        let mut present = vec![false; 256];
        for value in &window.values {
            present[*value as isize as usize & 0xff] = true;
        }
        let mut classes: Vec<i8> = Vec::new();
        for raw in 0..256usize {
            if present[raw] {
                classes.push(raw as u8 as i8);
            }
        }
        classes.sort_unstable();

        let mut plane_of = vec![u16::MAX; 256];
        for (index, class) in classes.iter().enumerate() {
            plane_of[*class as isize as usize & 0xff] = index as u16;
        }

        Self {
            window,
            bounds: window.bounds,
            classes,
            plane_of,
        }
    }

    /// Classes present anywhere in the window, ascending.
    pub(super) fn classes(&self) -> &[i8] {
        &self.classes
    }

    /// Counts for one output row, west to east.
    ///
    /// `out[i * classes().len() + c]` is how many cells of `classes()[c]` the
    /// neighbourhood of the `i`-th output column holds; `totals[i]` is how many
    /// cells that neighbourhood held at all.
    pub(super) fn row_counts(
        &self,
        lat_index: usize,
        lon_from: usize,
        lon_to: usize,
        radius_cells: usize,
        out: &mut Vec<u32>,
        totals: &mut Vec<u32>,
    ) {
        let class_count = self.classes.len();
        let width = lon_to.saturating_sub(lon_from) + 1;
        out.clear();
        out.resize(width * class_count, 0);
        totals.clear();
        totals.resize(width, 0);
        if class_count == 0 {
            return;
        }

        // Rows the neighbourhood covers, clipped as `value_at_global` clips.
        let lat_lo = lat_index
            .saturating_sub(radius_cells)
            .max(self.bounds.maxlat_source);
        let lat_hi = (lat_index + radius_cells).min(self.bounds.minlat_source);
        if lat_lo > lat_hi {
            return;
        }

        // Columns the whole row will ever touch, and each one's class counts
        // down the covered rows. Built once per output row: the vertical extent
        // is the same for every column in it.
        let scan_lo = lon_from
            .saturating_sub(radius_cells)
            .max(self.bounds.minlon_source);
        let scan_hi = (lon_to + radius_cells).min(self.bounds.maxlon_source);
        if scan_lo > scan_hi {
            return;
        }
        let scan_width = scan_hi - scan_lo + 1;
        let mut column = vec![0u32; scan_width * class_count];
        let mut column_total = vec![0u32; scan_width];
        for lon in scan_lo..=scan_hi {
            let slot = lon - scan_lo;
            for lat in lat_lo..=lat_hi {
                let Some(value) = self.window.value_at_global(lon, lat) else {
                    continue;
                };
                let plane = self.plane_of[value as isize as usize & 0xff] as usize;
                column[slot * class_count + plane] += 1;
                column_total[slot] += 1;
            }
        }

        // Slide east: the neighbourhood of column `lon` is the column counts
        // over `lon-r ..= lon+r`, clipped.
        let mut running = vec![0u32; class_count];
        let mut running_total = 0u32;
        let mut covered_lo = scan_lo;
        let mut covered_hi = scan_lo;
        let mut primed = false;
        for lon in lon_from..=lon_to {
            let want_lo = lon
                .saturating_sub(radius_cells)
                .max(self.bounds.minlon_source)
                .max(scan_lo);
            let want_hi = (lon + radius_cells)
                .min(self.bounds.maxlon_source)
                .min(scan_hi);
            if want_lo > want_hi {
                continue;
            }
            if !primed {
                for column_index in want_lo..=want_hi {
                    let slot = column_index - scan_lo;
                    for plane in 0..class_count {
                        running[plane] += column[slot * class_count + plane];
                    }
                    running_total += column_total[slot];
                }
                covered_lo = want_lo;
                covered_hi = want_hi;
                primed = true;
            } else {
                while covered_hi < want_hi {
                    covered_hi += 1;
                    let slot = covered_hi - scan_lo;
                    for plane in 0..class_count {
                        running[plane] += column[slot * class_count + plane];
                    }
                    running_total += column_total[slot];
                }
                while covered_lo < want_lo {
                    let slot = covered_lo - scan_lo;
                    for plane in 0..class_count {
                        running[plane] -= column[slot * class_count + plane];
                    }
                    running_total -= column_total[slot];
                    covered_lo += 1;
                }
            }
            let base = (lon - lon_from) * class_count;
            out[base..base + class_count].copy_from_slice(&running);
            totals[lon - lon_from] = running_total;
        }
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
        // clipping is where a sliding window and a nested loop are most likely
        // to disagree.
        let window = window(11, 7, |lon, lat| ((lon * 3 + lat * 5) % 4) as i8);
        let counts = ClassCounts::build(&window);
        let (mut row, mut totals) = (Vec::new(), Vec::new());

        for radius in [1usize, 2, 3, 5, 20] {
            for lat in 1..=7 {
                counts.row_counts(lat, 1, 11, radius, &mut row, &mut totals);
                let class_count = counts.classes().len();
                for lon in 1..=11 {
                    let (expected, expected_total) = brute_force(&window, lon, lat, radius);
                    assert_eq!(
                        totals[lon - 1],
                        expected_total,
                        "total at {lon},{lat} r{radius}"
                    );
                    let base = (lon - 1) * class_count;
                    let got: Vec<(i8, u32)> = counts
                        .classes()
                        .iter()
                        .copied()
                        .zip(row[base..base + class_count].iter().copied())
                        .filter(|(_, count)| *count > 0)
                        .collect();
                    assert_eq!(got, expected, "counts at {lon},{lat} r{radius}");
                }
            }
        }
    }

    #[test]
    fn negative_and_zero_class_ids_keep_their_own_planes() {
        // Land-type values are i8 and the readers do not promise them positive;
        // indexing a plane table by a raw cast is where that goes wrong.
        let window = window(6, 4, |lon, _| match lon % 3 {
            0 => 0,
            1 => -5,
            _ => 9,
        });
        let counts = ClassCounts::build(&window);
        assert_eq!(counts.classes(), &[-5, 0, 9]);

        let (mut row, mut totals) = (Vec::new(), Vec::new());
        counts.row_counts(2, 1, 6, 1, &mut row, &mut totals);
        for lon in 1..=6 {
            let (expected, expected_total) = brute_force(&window, lon, 2, 1);
            assert_eq!(totals[lon - 1], expected_total);
            let base = (lon - 1) * 3;
            let got: Vec<(i8, u32)> = counts
                .classes()
                .iter()
                .copied()
                .zip(row[base..base + 3].iter().copied())
                .filter(|(_, count)| *count > 0)
                .collect();
            assert_eq!(got, expected, "at {lon}");
        }
    }
}
