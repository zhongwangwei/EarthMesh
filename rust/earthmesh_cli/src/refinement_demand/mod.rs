//! Where refinement is wanted, and how that becomes Method-C regions.
//!
//! Every criterion asks the same question of every source-raster cell: does
//! this cell need a finer mesh? A coastline asks it of a land/sea class map,
//! sea surface temperature of an SST field, land-cover of a category count,
//! and bathymetry will ask it of a depth field. The answer is always a boolean
//! raster, and reducing such a raster to circles does not depend on which
//! criterion produced it.
//!
//! So the two halves live apart. Producers ([`landtype`], [`threshold`]) turn a
//! data source into a [`RefinementDemand`]; [`reduce_demand_to_circles`] turns
//! any demand into circles. Adding a criterion means adding a producer, not
//! touching the reduction.
//!
//! The h-field is the other consumer of the same input: it takes the union of
//! criteria and gradient-limits it into a continuous `h(x)`, where this takes
//! the union and covers it with circles. Both start from demand, which is why
//! they can be compared on the same criterion.
//!
//! # Where this comes from
//!
//! Points-plus-a-radius is not a representation invented here. Walko & Avissar
//! (2011) give it as OLAM's own way of naming a refined area -- "a sequence of
//! points plus a radius of influence" -- in the same paper that defines the
//! conforming subdivision and transition rows this engine implements as
//! Method-C. Deriving those points from data criteria rather than from a user
//! is Fan et al. (2024). What is added here is asking the criteria again after
//! each level, at the size of the cells that level just made.
//!
//! That last part is the regrid loop of structured AMR (Berger & Oliger 1984)
//! with a different reason behind it: AMR re-evaluates because the solution
//! moves, this re-evaluates because the answer depends on the cell. "How many
//! land-cover classes are in this cell" cannot be asked before the cell exists,
//! and its answer changes once the cell is halved -- which is precisely what a
//! single up-front field has no way to express.
//!
//! - Walko, R. L., & Avissar, R. (2011). A direct method for constructing
//!   refined regions in unstructured conforming triangular-hexagonal
//!   computational grids: Application to OLAM. Monthly Weather Review 139(12),
//!   3923-3937. doi:10.1175/MWR-D-11-00021.1
//! - Fan, H., Xu, Q., Bai, F., Wei, Z., Zhang, Y., Lu, X., et al. (2024). An
//!   unstructured mesh generation tool for efficient high-resolution
//!   representation of spatial heterogeneity in land surface models.
//!   Geophysical Research Letters 51(6). doi:10.1029/2023GL107059
//! - Berger, M. J., & Oliger, J. (1984). Adaptive mesh refinement for
//!   hyperbolic partial differential equations. Journal of Computational
//!   Physics 53(3), 484-512. doi:10.1016/0021-9991(84)90073-1

mod class_counts;
pub mod ladder;
pub mod landtype;
pub mod nest;
pub mod plan;
pub mod threshold;

use std::io;

use earthmesh_mesh::{AreaJudgeSourceBounds, LonLatDegrees, RefinementRegion};

const RASTER_PROGRESS_MIN_CELLS: usize = 10_000_000;

/// Refinement demand over a window of the source raster.
///
/// Indices are the engine's global one-based source indices: index 1 sits at
/// -180 longitude and +90 latitude, and latitude runs north to south.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefinementDemand {
    bounds: AreaJudgeSourceBounds,
    gridnum_perdegree: usize,
    nlons: usize,
    nlats: usize,
    /// One bit per source cell, packed 64 to a word.
    ///
    /// A `Vec<bool>` spends a byte on each bit. That is invisible on a regional
    /// window and decisive on a global one: at the 240 cells per degree the
    /// production IGBP raster carries, the window is 86400x43200 -- 3.7 billion
    /// cells, 3.5 GB per criterion before any of them are unioned. Packed, the
    /// same window is 435 MB.
    ///
    /// `len` is carried separately because the last word is partly padding.
    /// Those padding bits are held at zero by every operation here, so derived
    /// `PartialEq` still means "the same cells are demanded".
    words: Vec<u64>,
    len: usize,
}

impl RefinementDemand {
    /// An empty demand over `bounds`, with nothing yet marked.
    pub fn new(bounds: AreaJudgeSourceBounds, gridnum_perdegree: usize) -> io::Result<Self> {
        if gridnum_perdegree == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "gridnum_perdegree must be positive",
            ));
        }
        if bounds.maxlon_source < bounds.minlon_source
            || bounds.minlat_source < bounds.maxlat_source
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demand bounds must be non-empty",
            ));
        }
        let nlons = bounds.maxlon_source - bounds.minlon_source + 1;
        let nlats = bounds.minlat_source - bounds.maxlat_source + 1;
        let len = nlons.checked_mul(nlats).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demand window overflows",
            )
        })?;
        Ok(Self {
            bounds,
            gridnum_perdegree,
            nlons,
            nlats,
            words: vec![0u64; len.div_ceil(64)],
            len,
        })
    }

    pub fn bounds(&self) -> AreaJudgeSourceBounds {
        self.bounds
    }

    pub fn gridnum_perdegree(&self) -> usize {
        self.gridnum_perdegree
    }

    fn offset(&self, lon_index: usize, lat_index: usize) -> Option<usize> {
        if lon_index < self.bounds.minlon_source
            || lon_index > self.bounds.maxlon_source
            || lat_index < self.bounds.maxlat_source
            || lat_index > self.bounds.minlat_source
        {
            return None;
        }
        let lon_offset = lon_index - self.bounds.minlon_source;
        let lat_offset = lat_index - self.bounds.maxlat_source;
        Some(lat_offset * self.nlons + lon_offset)
    }

    /// Mark or clear one source cell. Indices outside the window are ignored,
    /// which lets a producer walk a halo without bounds arithmetic.
    pub fn set(&mut self, lon_index: usize, lat_index: usize, demanded: bool) {
        if let Some(offset) = self.offset(lon_index, lat_index) {
            let (word, bit) = (offset / 64, offset % 64);
            if demanded {
                self.words[word] |= 1u64 << bit;
            } else {
                self.words[word] &= !(1u64 << bit);
            }
        }
    }

    pub fn is_demanded(&self, lon_index: usize, lat_index: usize) -> bool {
        self.offset(lon_index, lat_index)
            .is_some_and(|offset| self.words[offset / 64] >> (offset % 64) & 1 == 1)
    }

    /// How many source cells the window holds, demanded or not.
    pub fn bounds_cell_count(&self) -> usize {
        self.len
    }

    pub fn demanded_count(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.words.iter().all(|word| *word == 0)
    }

    /// Clear demanded cells that do not pass `keep`.
    pub(crate) fn retain_where(&mut self, keep: impl Fn(usize, usize) -> bool) {
        for lat in self.bounds.maxlat_source..=self.bounds.minlat_source {
            for lon in self.bounds.minlon_source..=self.bounds.maxlon_source {
                if self.is_demanded(lon, lat) && !keep(lon, lat) {
                    self.set(lon, lat, false);
                }
            }
        }
    }

    /// Fill the window from a per-cell predicate, one latitude row at a time,
    /// in parallel.
    ///
    /// Every criterion here decides each cell on its own, so the rows are
    /// independent and the work is embarrassingly parallel -- and the answer
    /// does not depend on how it is divided, because each cell's bit is written
    /// exactly once from its own inputs. A run was measured single-threaded
    /// with rayon's workers sitting in `wait_until_cold`.
    ///
    /// `decide` takes global one-based source indices, the same as
    /// [`Self::set`].
    pub fn fill_par(&mut self, decide: impl Fn(usize, usize) -> bool + Sync + Send) {
        use rayon::prelude::*;

        let (minlon, nlons) = (self.bounds.minlon_source, self.nlons);
        let maxlat = self.bounds.maxlat_source;
        let nlats = self.nlats;
        // One row of bits per latitude, gathered in order, then packed. Packing
        // separately keeps the words free of cross-row interference without
        // making every write atomic.
        let rows: Vec<Vec<bool>> = (0..nlats)
            .into_par_iter()
            .map(|lat_offset| {
                let lat = maxlat + lat_offset;
                (0..nlons)
                    .map(|lon_offset| decide(minlon + lon_offset, lat))
                    .collect()
            })
            .collect();

        for (lat_offset, row) in rows.into_iter().enumerate() {
            for (lon_offset, demanded) in row.into_iter().enumerate() {
                if demanded {
                    let offset = lat_offset * nlons + lon_offset;
                    self.words[offset / 64] |= 1u64 << (offset % 64);
                }
            }
        }
    }

    /// Fill the window a whole latitude row at a time, in parallel.
    ///
    /// The neighbourhood criteria slide a window along each row, so they answer
    /// a row far more cheaply than they answer its cells one at a time.
    /// `decide_row` is handed the global latitude, the first and last global
    /// longitude of the row, and a buffer to write one bool per column into.
    ///
    /// Rows are independent, and each cell's bit is written exactly once from
    /// its own inputs, so how the rows are divided cannot change the answer.
    pub fn fill_rows_par(
        &mut self,
        decide_row: impl Fn(usize, usize, usize, &mut Vec<bool>) + Sync + Send,
    ) {
        use rayon::prelude::*;

        let (minlon, nlons) = (self.bounds.minlon_source, self.nlons);
        let maxlat = self.bounds.maxlat_source;
        let nlats = self.nlats;
        let report_progress = self.len >= RASTER_PROGRESS_MIN_CELLS;
        let started = std::time::Instant::now();
        if report_progress {
            eprintln!(
                "earthmesh_cli: raster demand row evaluation started: {nlats} rows x {nlons} columns ({} cells)",
                self.len
            );
        }
        let rows: Vec<Vec<bool>> = (0..nlats)
            .into_par_iter()
            .map(|lat_offset| {
                let mut row = Vec::new();
                decide_row(maxlat + lat_offset, minlon, minlon + nlons - 1, &mut row);
                debug_assert_eq!(row.len(), nlons, "a row must cover the window width");
                row
            })
            .collect();

        for (lat_offset, row) in rows.into_iter().enumerate() {
            for (lon_offset, demanded) in row.into_iter().enumerate() {
                if demanded {
                    let offset = lat_offset * nlons + lon_offset;
                    self.words[offset / 64] |= 1u64 << (offset % 64);
                }
            }
        }
        if report_progress {
            eprintln!(
                "earthmesh_cli: raster demand scan complete in {:.1}s",
                started.elapsed().as_secs_f64()
            );
        }
    }

    /// Fill consecutive latitude bands in parallel and pack each emitted row
    /// immediately, without retaining one byte per source cell until the scan
    /// finishes.
    pub fn fill_row_bands_par(
        &mut self,
        decide_band: impl Fn(usize, usize, usize, usize, &mut dyn FnMut(usize, &[bool])) + Sync + Send,
    ) {
        let nlons = self.nlons;
        self.fill_packed_bands_par(|lat_from, lat_to, lon_from, lon_to, words| {
            let rows = lat_to - lat_from + 1;
            let mut emitted_rows = 0usize;
            let mut emit = |lat: usize, row: &[bool]| {
                debug_assert_eq!(row.len(), nlons, "a row must cover the window width");
                debug_assert_eq!(lat, lat_from + emitted_rows);
                let row_bit_from = emitted_rows * nlons;
                for (lon_offset, demanded) in row.iter().copied().enumerate() {
                    if demanded {
                        let bit = row_bit_from + lon_offset;
                        words[bit / 64] |= 1u64 << (bit % 64);
                    }
                }
                emitted_rows += 1;
            };
            decide_band(lat_from, lat_to, lon_from, lon_to, &mut emit);
            debug_assert_eq!(emitted_rows, rows, "a band must emit every row once");
        });
    }

    /// Fill word-aligned latitude bands directly into the packed demand.
    pub fn fill_packed_bands_par(
        &mut self,
        decide_band: impl Fn(usize, usize, usize, usize, &mut [u64]) + Sync + Send,
    ) {
        use rayon::prelude::*;

        let (minlon, nlons) = (self.bounds.minlon_source, self.nlons);
        let maxlat = self.bounds.maxlat_source;
        let nlats = self.nlats;
        let workers = rayon::current_num_threads().max(1);
        let target_rows = nlats.div_ceil(workers.saturating_mul(2)).max(1);
        // A whole number of 64-row groups aligns every band's first bit for any
        // raster width, so Rayon can hand out disjoint word slices directly.
        let band_rows = target_rows.div_ceil(64).saturating_mul(64).max(64);
        let words_per_band = band_rows.saturating_mul(nlons) / 64;
        let report_progress = self.len >= RASTER_PROGRESS_MIN_CELLS;
        let started = std::time::Instant::now();
        if report_progress {
            eprintln!(
                "earthmesh_cli: raster demand band evaluation started: {nlats} rows x {nlons} columns ({} cells, {band_rows} rows/band)",
                self.len
            );
        }

        self.words.fill(0);
        self.words
            .par_chunks_mut(words_per_band.max(1))
            .enumerate()
            .for_each(|(band_index, words)| {
                let lat_offset_from = band_index * band_rows;
                let rows = (nlats - lat_offset_from).min(band_rows);
                decide_band(
                    maxlat + lat_offset_from,
                    maxlat + lat_offset_from + rows - 1,
                    minlon,
                    minlon + nlons - 1,
                    words,
                );
            });
        if report_progress {
            eprintln!(
                "earthmesh_cli: raster demand scan complete in {:.1}s",
                started.elapsed().as_secs_f64()
            );
        }
    }

    /// Union with another demand over the same window, so several criteria can
    /// drive one reduction. Both the window and the sampling must agree.
    pub fn union_with(&mut self, other: &Self) -> io::Result<()> {
        if self.bounds != other.bounds || self.gridnum_perdegree != other.gridnum_perdegree {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "refinement demands must share a window and sampling to be unioned",
            ));
        }
        for (target, source) in self.words.iter_mut().zip(&other.words) {
            *target |= *source;
        }
        Ok(())
    }

    fn lon_degrees(&self, lon_index_centre: f64) -> f64 {
        (lon_index_centre - 1.0) / self.gridnum_perdegree as f64 - 180.0
    }

    fn lat_degrees(&self, lat_index_centre: f64) -> f64 {
        90.0 - (lat_index_centre - 1.0) / self.gridnum_perdegree as f64
    }
}

/// Smallest circle radius at which a chain of circles refines a continuous band.
///
/// The earlier value, 0.4 base cells, answered a different question: whether a
/// *single* circle can seed. It can -- measured at 100% over a sweep of
/// positions. But selection expands along a stride-3 lattice, and it only steps
/// to a neighbouring seed if that seed is *also* inside some circle. Seeds sit
/// three base cells apart, so a circle 0.8 base cells across cannot reach the
/// next one however many circles are laid beside it: a chain refines one seed's
/// footprint and stops. Measured on a real global coastal case, 114566 circles
/// grew the mesh by 144 faces -- one seed's worth.
///
/// 2.5 comes from a sweep of twelve positions at NXP 21, 81 and 162, laying a
/// chain along ten base cells and asking whether the band comes out continuous:
///
/// | k | NXP 21 | NXP 81 | NXP 162 |
/// |---|---|---|---|
/// | 1.5 | 3/12 | 5/12 | 2/12 |
/// | 2.0 | 9/12 | 7/12 | 7/12 |
/// | 2.2 | 11/12 | — | 11/12 |
/// | 2.5 | 12/12 | 12/12 | 11/12 |
/// | 3.0 | 11/12 | 12/12 | 12/12 |
///
/// The threshold does not move with resolution, which is what marks it as a
/// property of the lattice rather than of a particular grid. And as with
/// [`ladder::MEASURED_PARENT_HALO_ROWS`], the admissible set is not upward
/// closed -- NXP 21 loses a position going from 2.5 to 3.0 -- so a larger value
/// is not safe by being larger, and only the sweep settles it.
///
/// What this costs is honesty about granularity: refinement cannot be finer
/// than the lattice it is laid on, so demand narrower than a few base cells is
/// served by a band that wide. The previous value hid that by producing
/// disconnected specks instead.
pub fn materializable_radius_meters(base_cell_meters: f64) -> f64 {
    2.5 * base_cell_meters
}

/// Cover demand with a chain of circles at `level`.
///
/// The window is walked in blocks half a radius across, and a block holding any
/// demanded cell gets one circle on its centre. Half-radius blocking makes
/// consecutive circles overlap by half, which is what keeps a chain continuous
/// along a curving feature — the failure mode that fragments h-field demand.
///
/// `radius_meters` must clear [`materializable_radius_meters`] for the parent
/// generation or Method-C cannot seed inside the circle; that is the caller's
/// choice because only the caller knows the base cell size.
pub fn reduce_demand_to_circles(
    demand: &RefinementDemand,
    level: usize,
    radius_meters: f64,
) -> io::Result<Vec<RefinementRegion>> {
    reduce_demand_to_circles_on_blocks(demand, level, radius_meters, radius_meters)
}

/// Cover demand with circles of `radius_meters`, on blocks sized for
/// `block_radius_meters`.
///
/// A nested run needs the two to differ. Blocks scale with the radius, so a
/// chain that re-blocks per level puts each level's circles on different
/// centres, and a deep circle can then land where its parent never refined —
/// Method-C rejects that for crossing the parent boundary. Sizing every level's
/// blocks from the *finest* radius makes the centres coincide, so the levels
/// are concentric and nest the way the measured single-feature ladder does,
/// while the blocks stay small enough that even the finest circle still covers
/// its own block.
pub fn reduce_demand_to_circles_on_blocks(
    demand: &RefinementDemand,
    level: usize,
    radius_meters: f64,
    block_radius_meters: f64,
) -> io::Result<Vec<RefinementRegion>> {
    if !radius_meters.is_finite() || radius_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement circle radius must be positive and finite",
        ));
    }
    if !block_radius_meters.is_finite() || block_radius_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement block radius must be positive and finite",
        ));
    }
    let per_degree = demand.gridnum_perdegree as f64;
    let meters_per_degree = std::f64::consts::PI * earthmesh_core::EARTH_RADIUS_METERS / 180.0;
    let block_degrees = (block_radius_meters / meters_per_degree) / 2.0;
    let block_cells = ((block_degrees * per_degree).round() as usize).max(1);

    let mut regions = Vec::new();
    let mut lat_block = demand.bounds.maxlat_source;
    while lat_block <= demand.bounds.minlat_source {
        let lat_end = (lat_block + block_cells - 1).min(demand.bounds.minlat_source);
        let mut lon_block = demand.bounds.minlon_source;
        while lon_block <= demand.bounds.maxlon_source {
            let lon_end = (lon_block + block_cells - 1).min(demand.bounds.maxlon_source);
            let mut wanted = false;
            'block: for lat in lat_block..=lat_end {
                for lon in lon_block..=lon_end {
                    if demand.is_demanded(lon, lat) {
                        wanted = true;
                        break 'block;
                    }
                }
            }
            if wanted {
                let lon_centre = (lon_block + lon_end) as f64 / 2.0;
                let lat_centre = (lat_block + lat_end) as f64 / 2.0;
                regions.push(RefinementRegion::Circle {
                    center: LonLatDegrees::new(
                        demand.lon_degrees(lon_centre),
                        demand.lat_degrees(lat_centre),
                    ),
                    radius_meters,
                    level,
                });
            }
            lon_block = lon_end + 1;
        }
        lat_block = lat_end + 1;
    }
    Ok(regions)
}

/// Global one-based source indices for a geographic window.
pub fn source_bounds_for_bbox(
    west_degrees: f64,
    east_degrees: f64,
    south_degrees: f64,
    north_degrees: f64,
    gridnum_perdegree: usize,
) -> io::Result<AreaJudgeSourceBounds> {
    if gridnum_perdegree == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gridnum_perdegree must be positive",
        ));
    }
    if !(west_degrees.is_finite()
        && east_degrees.is_finite()
        && south_degrees.is_finite()
        && north_degrees.is_finite())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement demand bounds must be finite",
        ));
    }
    if east_degrees <= west_degrees || north_degrees <= south_degrees {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refinement demand bounds must be non-empty",
        ));
    }
    let per_degree = gridnum_perdegree as f64;
    // Clamped to the source dimensions: the floor-plus-one mapping puts exactly
    // 180 east and exactly -90 one cell past the last one, and a window ending
    // there would otherwise carry a column the raster cannot answer for.
    let nlons_source = gridnum_perdegree.saturating_mul(360);
    let nlats_source = gridnum_perdegree.saturating_mul(180);
    let lon_index = |lon: f64| {
        (((lon + 180.0) * per_degree).floor().max(0.0) as usize + 1).clamp(1, nlons_source)
    };
    let lat_index = |lat: f64| {
        (((90.0 - lat) * per_degree).floor().max(0.0) as usize + 1).clamp(1, nlats_source)
    };
    Ok(AreaJudgeSourceBounds {
        minlon_source: lon_index(west_degrees),
        maxlon_source: lon_index(east_degrees),
        maxlat_source: lat_index(north_degrees),
        minlat_source: lat_index(south_degrees),
    })
}

#[cfg(test)]
mod tests;
