//! One source-statistics contract for both threshold-demand adapters.
//!
//! Supports are parent-scale angular rectangles, not mesh cells. Domain masks
//! select centers; all original samples and the full footprint of a selected
//! support remain eligible. Composition-raster resolution never sets this scale.

use std::{
    collections::{hash_map::DefaultHasher, BTreeMap},
    hash::{Hash, Hasher},
    io,
    path::Path,
};

use earthmesh_core::{RefineConfig, EARTH_RADIUS_METERS};
use earthmesh_hfield::HField;
use earthmesh_mesh::{AreaJudgeSourceBounds, LonLatDegrees, RefinementRegion};

use super::RefinementDemand;
use crate::{
    area_judge_threshold_inputs::{
        enabled_mean_threshold_field_specs, enabled_std_threshold_field_specs,
    },
    hfield_refine::{
        has_threshold_hfield_sources, read_landtype_support, read_numeric_support,
        support_landtype_mask, HfieldDomainMask,
    },
    GridRegion,
};

const MAX_SUPPORTS: usize = 16_777_216;

pub(crate) struct CriterionSupportDemand {
    pub id: String,
    pub hits: Vec<bool>,
    pub source_samples: usize,
    pub empty_supports: usize,
    pub singleton_supports: usize,
}

pub(crate) struct ThresholdSupportDemand {
    pub nlon: usize,
    pub nlat: usize,
    pub parent_m: f64,
    pub longitude_shift: f64,
    pub eligible_supports: usize,
    pub criteria: Vec<CriterionSupportDemand>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

pub(crate) fn threshold_level_cap(
    refine: &RefineConfig,
    mesh_type: &str,
    outer_cap: usize,
) -> io::Result<usize> {
    if !has_threshold_hfield_sources(refine, mesh_type.trim()) {
        return Ok(0);
    }
    if !(1..=5).contains(&refine.max_iter_cal) || !(1..=5).contains(&outer_cap) {
        return Err(invalid(
            "enabled thresholds require max_iter_cal and adapter level cap in 1..=5",
        ));
    }
    Ok(outer_cap.min(refine.max_iter_cal as usize))
}

fn support_dimensions(parent_m: f64) -> io::Result<(usize, usize)> {
    if !parent_m.is_finite() || parent_m <= 0.0 {
        return Err(invalid(
            "threshold parent scale must be positive and finite",
        ));
    }
    let count = std::f64::consts::PI * EARTH_RADIUS_METERS / parent_m;
    let nearest = count.round();
    let count = if (count - nearest).abs() <= count.abs() * 1e-12 {
        nearest
    } else {
        count.ceil()
    };
    if !count.is_finite() || count > ((MAX_SUPPORTS / 2) as f64).sqrt().floor() {
        return Err(invalid("threshold support exceeds 16,777,216 cells; increase the parent scale or reduce threshold levels"));
    }
    let nlat = (count as usize).max(2);
    Ok((2 * nlat, nlat))
}

fn support_mask(
    refine: &RefineConfig,
    nlon: usize,
    nlat: usize,
    domain: Option<&GridRegion>,
) -> io::Result<HfieldDomainMask> {
    let prefix = refine.mask_refine_cal_fprefix.trim().trim_end_matches('/');
    let mut masks = if matches!(prefix, "" | "/tmp" | "none") {
        Vec::new()
    } else {
        crate::read_method_c_calculated_refinement_regions(refine, 0, false)?
    };
    for mask in &mut masks {
        let (RefinementRegion::Bbox { level, .. }
        | RefinementRegion::Circle { level, .. }
        | RefinementRegion::Corridor { level, .. }
        | RefinementRegion::Polygon { level, .. }) = mask;
        *level = 1; // Geographic validation only; degree-zero masks are not demands.
        mask.validate()?;
    }
    let mut active = vec![false; nlon * nlat];
    for i in 0..nlon {
        let lon = -180.0 + i as f64 * 360.0 / nlon as f64;
        for j in 0..nlat {
            let lat = -90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64;
            active[i * nlat + j] = domain.is_none_or(|region| region.contains(lon, lat))
                && (masks.is_empty()
                    || masks.iter().any(|region| {
                        region.contains_lonlat_canonical(LonLatDegrees::new(lon, lat))
                    }));
        }
    }
    Ok(HfieldDomainMask { nlon, nlat, active })
}

fn criterion(
    id: String,
    active: &[bool],
    samples: impl Fn(usize) -> usize,
    hit: impl Fn(usize, usize) -> bool,
) -> CriterionSupportDemand {
    let mut out = CriterionSupportDemand {
        id,
        hits: vec![false; active.len()],
        source_samples: 0,
        empty_supports: 0,
        singleton_supports: 0,
    };
    for (index, &eligible) in active.iter().enumerate() {
        if eligible {
            let n = samples(index);
            out.source_samples += n;
            out.empty_supports += usize::from(n == 0);
            out.singleton_supports += usize::from(n == 1);
            out.hits[index] = n > 0 && hit(index, n);
        }
    }
    out
}

pub(crate) fn evaluate_threshold_support(
    refine: &RefineConfig,
    mesh_type: &str,
    landtype_file: Option<&Path>,
    parent_m: f64,
    domain: Option<&GridRegion>,
) -> io::Result<ThresholdSupportDemand> {
    if !parent_m.is_finite() || parent_m <= 0.0 {
        return Err(invalid(
            "threshold parent scale must be positive and finite",
        ));
    }
    let mut out = ThresholdSupportDemand {
        nlon: 0,
        nlat: 0,
        parent_m,
        longitude_shift: 0.0,
        eligible_supports: 0,
        criteria: Vec::new(),
    };
    if threshold_level_cap(refine, mesh_type, 5)? == 0 {
        return Ok(out);
    }
    let (nlon, nlat) = support_dimensions(parent_m)?;
    out.nlon = nlon;
    out.nlat = nlat;
    out.longitude_shift = 180.0 / nlon as f64;
    let mask = support_mask(refine, nlon, nlat, domain)?;
    out.eligible_supports = mask.active.iter().filter(|&&active| active).count();
    if out.eligible_supports == 0 {
        eprintln!("earthmesh_cli: no threshold support centers inside the domain/mask at parent scale {parent_m:.3} m ({nlon}x{nlat}); thin regions may need a finer support scale");
    }
    // Group a variable's mean/std so both use the same original samples and one read.
    let mut groups = BTreeMap::<(String, String), Vec<(bool, f64)>>::new();
    for (stddev, specs) in [
        (
            false,
            enabled_mean_threshold_field_specs(refine, mesh_type.trim()),
        ),
        (
            true,
            enabled_std_threshold_field_specs(refine, mesh_type.trim()),
        ),
    ] {
        for spec in specs {
            if !spec.threshold.is_finite() || (stddev && spec.threshold < 0.0) {
                return Err(invalid("numeric thresholds must be finite; standard-deviation thresholds must be non-negative"));
            }
            groups
                .entry((spec.file_stem, spec.var_name))
                .or_default()
                .push((stddev, spec.threshold));
        }
    }
    if refine.refine_num_landtypes && refine.th_num_landtypes < 0 {
        return Err(invalid(
            "landcover distinct-class threshold must be non-negative",
        ));
    }
    if refine.refine_area_mainland
        && (!refine.th_area_mainland.is_finite() || !(0.0..=1.0).contains(&refine.th_area_mainland))
    {
        return Err(invalid(
            "dominant land-class share threshold must be in [0,1]",
        ));
    }
    if refine.refine_sea_ratio
        && (!(0.0..=1.0).contains(&refine.th_sea_ratio[0])
            || !(0.0..=1.0).contains(&refine.th_sea_ratio[1])
            || refine.th_sea_ratio[0] >= refine.th_sea_ratio[1])
    {
        return Err(invalid(
            "sea-ratio thresholds must satisfy 0 <= min < max <= 1",
        ));
    }
    let grid = HField::uniform(nlon, nlat, parent_m)?;
    let land_mask = if groups.is_empty() {
        None
    } else {
        landtype_file.map(support_landtype_mask).transpose()?
    };
    for ((stem, name), comparisons) in groups {
        let path = Path::new(refine.threshold_dir.trim()).join(format!("{stem}.nc"));
        let file = crate::open_netcdf(&path).map_err(crate::netcdf_to_io_error)?;
        let stats = read_numeric_support(
            &file,
            &name,
            &grid,
            land_mask.as_ref(),
            Some(&mask),
            out.longitude_shift,
        )?;
        for (stddev, threshold) in comparisons {
            let id = format!("{name}_{}", if stddev { "std" } else { "mean" });
            out.criteria.push(criterion(
                id,
                &mask.active,
                |i| stats.samples[i],
                |i, n| {
                    if stddev {
                        n >= 2 && stats.stddev[i] > threshold
                    } else {
                        stats.mean[i] > threshold
                    }
                },
            ));
        }
    }
    if refine.refine_num_landtypes || refine.refine_area_mainland || refine.refine_sea_ratio {
        let path = landtype_file
            .ok_or_else(|| invalid("enabled categorical thresholds require a landtype source"))?;
        let bins = read_landtype_support(path, &grid, Some(&mask), out.longitude_shift)?;
        for (enabled, id) in [
            (refine.refine_num_landtypes, "landcover"),
            (refine.refine_area_mainland, "area_mainland"),
            (refine.refine_sea_ratio, "sea_ratio"),
        ] {
            if enabled {
                out.criteria.push(criterion(
                    id.to_string(),
                    &mask.active,
                    |i| bins.total_at(i),
                    |i, n| match id {
                        "landcover" => {
                            bins.class_counts_at(i).len() > refine.th_num_landtypes as usize
                        }
                        "area_mainland" => {
                            let largest = bins
                                .class_counts_at(i)
                                .iter()
                                .map(|(_, n)| *n)
                                .max()
                                .unwrap_or(0);
                            bins.land_at(i) > 0
                                && (largest as f64 / bins.land_at(i) as f64)
                                    < refine.th_area_mainland
                        }
                        _ => {
                            let ratio = bins.ocean_at(i) as f64 / n as f64;
                            ratio > refine.th_sea_ratio[0] && ratio < refine.th_sea_ratio[1]
                        }
                    },
                ));
            }
        }
    }
    Ok(out)
}

// Exact positive-area intersections. Longitude supports straddle the -180 origin;
// latitude supports start at -90. Touching an edge alone is not an intersection.
fn overlaps(index: usize, src: usize, dst: usize, shifted: bool) -> std::ops::Range<i128> {
    let (i, n, m) = (index as i128, src as i128, dst as i128);
    let (lo, hi, denominator) = if shifted {
        (2 * i - 1, 2 * i + 1, 2 * n)
    } else {
        (i, i + 1, n)
    };
    (lo * m).div_euclid(denominator)..(-(-hi * m).div_euclid(denominator))
}

impl ThresholdSupportDemand {
    pub(crate) fn criterion_report(&self, criterion: &CriterionSupportDemand) -> serde_json::Value {
        let mut hasher = DefaultHasher::new();
        criterion.hits.hash(&mut hasher);
        serde_json::json!({
            "criterion": criterion.id,
            "kind": "independent_parent_angular_support",
            "target_parent_scale_m": self.parent_m,
            "nlon": self.nlon, "nlat": self.nlat,
            "longitude_shift_degrees": self.longitude_shift,
            "span_degrees": 180.0 / self.nlat as f64,
            "eligible_supports": self.eligible_supports,
            "hit_supports": criterion.hits.iter().filter(|&&hit| hit).count(),
            "hit_map_diagnostic_hash": format!("{:016x}", hasher.finish()),
            "hash_algorithm": "rust_DefaultHasher_not_cryptographic",
            "valid_source_samples": criterion.source_samples,
            "empty_supports": criterion.empty_supports,
            "singleton_supports": criterion.singleton_supports,
            "domain_selection": "support_centers_full_sample_and_demand_footprint",
            "longitude_wraps": true,
            "numeric_scale": "stored_values_no_unit_conversion",
            "numeric_weighting": "equal_sample_population",
        })
    }

    fn validate_projection(&self, hits: &[bool], nlon: usize, nlat: usize) -> io::Result<()> {
        if self.nlon == 0
            || self.nlat == 0
            || Some(hits.len()) != self.nlon.checked_mul(self.nlat)
            || nlon == 0
            || nlat == 0
            || nlon.checked_mul(nlat).is_none()
        {
            return Err(invalid(
                "threshold projection requires matching non-empty source and target dimensions",
            ));
        }
        Ok(())
    }

    pub(crate) fn project_hfield(
        &self,
        hits: &[bool],
        nlon: usize,
        nlat: usize,
    ) -> io::Result<Vec<bool>> {
        self.validate_projection(hits, nlon, nlat)?;
        let mut active = vec![false; nlon * nlat];
        for (index, &hit) in hits.iter().enumerate() {
            if !hit {
                continue;
            }
            for i in overlaps(index / self.nlat, self.nlon, nlon, true) {
                let i = i.rem_euclid(nlon as i128) as usize;
                for j in overlaps(index % self.nlat, self.nlat, nlat, false) {
                    active[i * nlat + j as usize] = true;
                }
            }
        }
        Ok(active)
    }

    pub(crate) fn project_source(
        &self,
        hits: &[bool],
        bounds: AreaJudgeSourceBounds,
        gridnum_perdegree: usize,
    ) -> io::Result<RefinementDemand> {
        let nlon = gridnum_perdegree
            .checked_mul(360)
            .ok_or_else(|| invalid("source longitude size overflows"))?;
        let nlat = gridnum_perdegree
            .checked_mul(180)
            .ok_or_else(|| invalid("source latitude size overflows"))?;
        self.validate_projection(hits, nlon, nlat)?;
        if bounds.minlon_source == 0
            || bounds.maxlat_source == 0
            || bounds.maxlon_source > nlon
            || bounds.minlat_source > nlat
        {
            return Err(invalid(
                "threshold projection bounds must be global one-based source indices",
            ));
        }
        let mut demand = RefinementDemand::new(bounds, gridnum_perdegree)?;
        for (index, &hit) in hits.iter().enumerate() {
            if !hit {
                continue;
            }
            let longitude = overlaps(index / self.nlat, self.nlon, nlon, true);
            let latitude = overlaps(index % self.nlat, self.nlat, nlat, false);
            let north = (nlat - latitude.end as usize + 1).max(bounds.maxlat_source);
            let south = (nlat - latitude.start as usize).min(bounds.minlat_source);
            // Split the periodic interval at zero, then clip before visiting source rows.
            for period in [-1i128, 0] {
                let lo = (longitude.start - period * nlon as i128)
                    .max(0)
                    .max(bounds.minlon_source as i128 - 1);
                let hi = (longitude.end - period * nlon as i128)
                    .min(nlon as i128)
                    .min(bounds.maxlon_source as i128);
                if lo >= hi {
                    continue;
                }
                for lat in north..=south {
                    let start = (lat - bounds.maxlat_source) * demand.nlons + lo as usize + 1
                        - bounds.minlon_source;
                    let end = start + (hi - lo) as usize;
                    set_bit_range(&mut demand.words, start, end);
                }
            }
        }
        Ok(demand)
    }
}

fn set_bit_range(words: &mut [u64], start: usize, end: usize) {
    let first = start / 64;
    let last = (end - 1) / 64;
    let first_mask = u64::MAX << (start % 64);
    let last_mask = u64::MAX >> (63 - (end - 1) % 64);
    if first == last {
        words[first] |= first_mask & last_mask;
    } else {
        words[first] |= first_mask;
        words[first + 1..last].fill(u64::MAX);
        words[last] |= last_mask;
    }
}

#[cfg(test)]
mod tests;
