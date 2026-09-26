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

    pub(super) fn plane_of(&self) -> &[u16] {
        &self.plane_of
    }

    /// Count consecutive output rows, north to south and west to east.
    ///
    /// The vertical column counts are initialized once for the first row, then
    /// updated by removing the row that leaves the neighbourhood and adding the
    /// row that enters it. `emit` receives one row at a time; its slices remain
    /// valid only until `emit` returns.
    pub(super) fn for_each_row(
        &self,
        lat_from: usize,
        lat_to: usize,
        lon_from: usize,
        lon_to: usize,
        radius_cells: usize,
        emit: impl FnMut(usize, &[u32], &[u32]),
    ) {
        let scan_lo = lon_from
            .saturating_sub(radius_cells)
            .max(self.bounds.minlon_source);
        let scan_hi = (lon_to + radius_cells).min(self.bounds.maxlon_source);
        for_each_count_row(
            lat_from,
            lat_to,
            lon_from,
            lon_to,
            self.bounds.maxlat_source,
            self.bounds.minlat_source,
            scan_lo as isize,
            scan_hi as isize,
            radius_cells,
            self.classes.len(),
            &self.plane_of,
            |lon, lat| {
                usize::try_from(lon)
                    .ok()
                    .and_then(|lon| self.window.value_at_global(lon, lat))
            },
            emit,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn for_each_count_row(
    lat_from: usize,
    lat_to: usize,
    lon_from: usize,
    lon_to: usize,
    source_lat_from: usize,
    source_lat_to: usize,
    scan_lon_from: isize,
    scan_lon_to: isize,
    radius_cells: usize,
    class_count: usize,
    plane_of: &[u16],
    value_at: impl Fn(isize, usize) -> Option<i8>,
    mut emit: impl FnMut(usize, &[u32], &[u32]),
) {
    if lat_from > lat_to || lon_from > lon_to || scan_lon_from > scan_lon_to {
        return;
    }

    let width = lon_to - lon_from + 1;
    let scan_width = (scan_lon_to - scan_lon_from + 1) as usize;
    let mut out = vec![0u32; width * class_count];
    let mut totals = vec![0u32; width];
    if class_count == 0 {
        for lat in lat_from..=lat_to {
            emit(lat, &out, &totals);
        }
        return;
    }

    let mut covered_lat_from = lat_from.saturating_sub(radius_cells).max(source_lat_from);
    let mut covered_lat_to = lat_from.saturating_add(radius_cells).min(source_lat_to);
    if covered_lat_from > covered_lat_to {
        for lat in lat_from..=lat_to {
            emit(lat, &out, &totals);
        }
        return;
    }

    let mut columns = vec![0u32; scan_width * class_count];
    let mut column_totals = vec![0u32; scan_width];
    for logical_lon in scan_lon_from..=scan_lon_to {
        let slot = (logical_lon - scan_lon_from) as usize;
        for lat in covered_lat_from..=covered_lat_to {
            add_value(
                &mut columns,
                &mut column_totals,
                slot,
                class_count,
                plane_of,
                value_at(logical_lon, lat),
            );
        }
    }

    for lat in lat_from..=lat_to {
        fill_horizontal_row(
            &columns,
            &column_totals,
            scan_lon_from,
            lon_from,
            lon_to,
            radius_cells,
            class_count,
            &mut out,
            &mut totals,
        );
        emit(lat, &out, &totals);

        if lat == lat_to {
            break;
        }
        let next_lat_from = (lat + 1).saturating_sub(radius_cells).max(source_lat_from);
        let next_lat_to = (lat + 1).saturating_add(radius_cells).min(source_lat_to);
        while covered_lat_from < next_lat_from {
            for logical_lon in scan_lon_from..=scan_lon_to {
                let slot = (logical_lon - scan_lon_from) as usize;
                remove_value(
                    &mut columns,
                    &mut column_totals,
                    slot,
                    class_count,
                    plane_of,
                    value_at(logical_lon, covered_lat_from),
                );
            }
            covered_lat_from += 1;
        }
        while covered_lat_to < next_lat_to {
            covered_lat_to += 1;
            for logical_lon in scan_lon_from..=scan_lon_to {
                let slot = (logical_lon - scan_lon_from) as usize;
                add_value(
                    &mut columns,
                    &mut column_totals,
                    slot,
                    class_count,
                    plane_of,
                    value_at(logical_lon, covered_lat_to),
                );
            }
        }
    }
}

fn add_value(
    columns: &mut [u32],
    totals: &mut [u32],
    slot: usize,
    class_count: usize,
    plane_of: &[u16],
    value: Option<i8>,
) {
    let Some(value) = value else { return };
    let plane = plane_of[value as isize as usize & 0xff] as usize;
    columns[slot * class_count + plane] += 1;
    totals[slot] += 1;
}

fn remove_value(
    columns: &mut [u32],
    totals: &mut [u32],
    slot: usize,
    class_count: usize,
    plane_of: &[u16],
    value: Option<i8>,
) {
    let Some(value) = value else { return };
    let plane = plane_of[value as isize as usize & 0xff] as usize;
    columns[slot * class_count + plane] -= 1;
    totals[slot] -= 1;
}

#[allow(clippy::too_many_arguments)]
fn fill_horizontal_row(
    columns: &[u32],
    column_totals: &[u32],
    scan_lon_from: isize,
    lon_from: usize,
    lon_to: usize,
    radius_cells: usize,
    class_count: usize,
    out: &mut [u32],
    totals: &mut [u32],
) {
    out.fill(0);
    totals.fill(0);
    let mut running = vec![0u32; class_count];
    let mut running_total = 0u32;
    let mut covered_lon_from = lon_from as isize - radius_cells as isize;
    let mut covered_lon_to = lon_from as isize + radius_cells as isize;
    covered_lon_from = covered_lon_from.max(scan_lon_from);
    let scan_lon_to = scan_lon_from + column_totals.len() as isize - 1;
    covered_lon_to = covered_lon_to.min(scan_lon_to);

    for logical_lon in covered_lon_from..=covered_lon_to {
        let slot = (logical_lon - scan_lon_from) as usize;
        for plane in 0..class_count {
            running[plane] += columns[slot * class_count + plane];
        }
        running_total += column_totals[slot];
    }

    for lon in lon_from..=lon_to {
        if lon > lon_from {
            let wanted_from = (lon as isize - radius_cells as isize).max(scan_lon_from);
            let wanted_to = (lon as isize + radius_cells as isize).min(scan_lon_to);
            while covered_lon_to < wanted_to {
                covered_lon_to += 1;
                let slot = (covered_lon_to - scan_lon_from) as usize;
                for plane in 0..class_count {
                    running[plane] += columns[slot * class_count + plane];
                }
                running_total += column_totals[slot];
            }
            while covered_lon_from < wanted_from {
                let slot = (covered_lon_from - scan_lon_from) as usize;
                for plane in 0..class_count {
                    running[plane] -= columns[slot * class_count + plane];
                }
                running_total -= column_totals[slot];
                covered_lon_from += 1;
            }
        }
        let base = (lon - lon_from) * class_count;
        out[base..base + class_count].copy_from_slice(&running);
        totals[lon - lon_from] = running_total;
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

        for radius in [1usize, 2, 3, 5, 20] {
            counts.for_each_row(1, 7, 1, 11, radius, |lat, row, totals| {
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
            });
        }
    }

    #[test]
    fn a_band_starting_mid_window_matches_the_loop() {
        let window = window(13, 9, |lon, lat| ((lon * 7 + lat * 11) % 5) as i8);
        let counts = ClassCounts::build(&window);
        counts.for_each_row(4, 7, 2, 12, 3, |lat, row, totals| {
            let class_count = counts.classes().len();
            for lon in 2..=12 {
                let (expected, expected_total) = brute_force(&window, lon, lat, 3);
                assert_eq!(totals[lon - 2], expected_total);
                let base = (lon - 2) * class_count;
                let got: Vec<(i8, u32)> = counts
                    .classes()
                    .iter()
                    .copied()
                    .zip(row[base..base + class_count].iter().copied())
                    .filter(|(_, count)| *count > 0)
                    .collect();
                assert_eq!(got, expected, "counts at {lon},{lat}");
            }
        });
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

        counts.for_each_row(2, 2, 1, 6, 1, |_, row, totals| {
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
        });
    }
}
