//! A certified mother grid stretched toward the demand (guide 11.87).
//!
//! The safe mother serves a demand by making the whole globe as fine as its
//! deepest level. A Schmidt stretch (guide 11.79) moves the vertices of a
//! coarser mother toward the demand instead; it is a Moebius map of the
//! sphere, so circles stay circles, the Delaunay property and the Voronoi
//! dual survive, and every vertex keeps degree 5 or 6 -- a refined grid a
//! closed ICON grid can take.
//!
//! A stretch also coarsens the far side, and every cell must stay at least
//! as fine as the base level. So a mother `m` times finer than the base can
//! be stretched by at most `m`, and it serves the demand only if the factor
//! the demand needs fits under that. Every certified mother between the base
//! and the safe one is a candidate, fewest cells first -- not only the powers
//! of two: a level-2 demand of any extent needs a little more than 2 from the
//! mother twice as fine, over its cap, while one 3.2 times as fine serves it
//! at about 1.3. Each candidate is stretched, its delivered levels are
//! measured from the cells' areas against the base, and it counts only when
//! CMRC's own final certificates pass -- geometry, physical and balance. A
//! demand no stretch serves falls back to the safe mother at the caller.

use earthmesh_mesh::CartesianPoint;

use crate::mother_geometry::{dual_areas, unit, xyz};
use crate::{
    certificate::AngleContractId,
    mother_grid::MotherGrid,
    outcome::{CertifiedMeshOutcome, GeometryCertifiedMotherGrid},
    requirement::{
        certify_final_cell_requirements_from_raster, FinalCellRequirementCertificate,
        RasterLevelField, TargetLevelField,
    },
};

use crate::mother_geometry::P;

/// A stretched mother that passed every final certificate.
pub struct StretchedMother {
    pub geometry: GeometryCertifiedMotherGrid,
    /// Subdivision of the mother that was stretched.
    pub subdivision: usize,
    /// How much finer than the base it is (`subdivision / base`).
    pub mother_ratio: f64,
    pub factor: f64,
    /// Focus longitude and latitude, degrees.
    pub focus_lonlat: (f64, f64),
    /// Each active Voronoi cell's delivered level, measured from its area.
    pub delivered_levels: Vec<usize>,
    pub final_requirements: FinalCellRequirementCertificate,
    /// Why each candidate tried before this one was not delivered.
    pub rejected: Vec<String>,
}

/// Centres of the raster cells and their levels.
fn raster_points(raster: &RasterLevelField) -> (Vec<P>, Vec<f64>) {
    let (nlon, nlat) = (raster.nlon(), raster.nlat());
    let mut points = Vec::with_capacity(nlon * nlat);
    let mut weights = Vec::with_capacity(nlon * nlat);
    for j in 0..nlat {
        let lat = (-90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64).to_radians();
        for i in 0..nlon {
            let lon = (-180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64).to_radians();
            points.push([lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]);
            weights.push(lat.cos());
        }
    }
    (points, weights)
}

/// Try a stretched mother for `raster` below `chosen_level`; the coarsest
/// that passes CMRC's final certificates is returned, or why none did.
pub fn stretched_certified_mother(
    base_subdivision: usize,
    chosen_level: usize,
    raster: &RasterLevelField,
    angle_contract: AngleContractId,
    max_cells: usize,
) -> Result<StretchedMother, Vec<String>> {
    let mut rejected = Vec::new();
    let (points, area_weights) = raster_points(raster);
    let levels = raster.levels();
    let safe = base_subdivision
        .checked_shl(chosen_level as u32)
        .unwrap_or(usize::MAX);
    let cells_of = |n: usize| {
        10usize
            .saturating_mul(n)
            .saturating_mul(n)
            .saturating_add(2)
    };
    // What one subdivision would take, analytically: its focus and the factor
    // it needs, or why it cannot serve. One pass over the raster.
    let screen = |subdivision: usize| -> Result<Screen, String> {
        let ratio = subdivision as f64 / base_subdivision as f64;
        // The scale each raster cell needs from the stretch: this mother is
        // `ratio` times finer than the base, and level L asks for 2^-L of it.
        // A cell the unstretched mother already serves asks for nothing.
        let scales = levels
            .iter()
            .map(|&l| ratio / 2f64.powi(l.min(60) as i32))
            .collect::<Vec<_>>();
        // The focus: the demand's mean direction, weighted by the cells the
        // stretch has to add there and by the raster cell's area.
        let mut sum = [0.0; 3];
        for ((p, &scale), &w) in points.iter().zip(&scales).zip(&area_weights) {
            if scale < 1.0 {
                let weight = (1.0 / (scale * scale) - 1.0) * w;
                for k in 0..3 {
                    sum[k] += weight * p[k];
                }
            }
        }
        let focus = unit(sum).ok_or_else(|| "the demand has no mean direction".to_string())?;
        // Coarsening the far side by more than `ratio` would take it below
        // the base level.
        let cap = ratio;
        let spacing = (4.0 * std::f64::consts::PI / cells_of(subdivision) as f64).sqrt();
        let needed =
            earthmesh_mesh::schmidt_factor_for_scales(&points, &scales, focus, spacing / 2.0, cap);
        if needed.unreachable > 0 {
            return Err(format!(
                "one focus cannot serve the demand ({} raster cells beyond reach of it, \
                 farthest {:.1} deg)",
                needed.unreachable, needed.farthest_unreachable_deg
            ));
        }
        if needed.capped {
            return Err(format!(
                "the demand needs a factor above {cap:.2}, which would take the far side below \
                 the base level"
            ));
        }
        Ok(Screen {
            ratio,
            cap,
            focus,
            factor: needed.factor,
        })
    };
    // Every subdivision certifies on its own proof (guide 11.101), so every
    // one between the base and the safe mother is a candidate. The screen
    // only gets easier with n -- a finer mother may stretch further and needs
    // less of it -- so the finest the budget allows is screened first (if it
    // fails, every coarser one does) and the coarsest that passes is found by
    // bisection: about log2 of the range in raster passes, not one per n.
    let in_budget = (((max_cells.saturating_sub(2)) / 10) as f64).sqrt().floor() as usize;
    let finest = safe.saturating_sub(1).min(in_budget);
    let over_budget = |rejected: &mut Vec<String>| {
        if finest < safe.saturating_sub(1) {
            rejected.push(format!(
                "n={} and finer: {} cells or more exceed the budget",
                finest + 1,
                cells_of(finest + 1)
            ));
        }
    };
    if finest <= base_subdivision {
        over_budget(&mut rejected);
        return Err(rejected);
    }
    let first = match screen(finest) {
        Err(reason) => {
            rejected.push(format!("{}: {reason}", span(base_subdivision + 1, finest)));
            over_budget(&mut rejected);
            return Err(rejected);
        }
        Ok(_) => {
            let (mut lo, mut hi) = (base_subdivision + 1, finest);
            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                if screen(mid).is_ok() {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lo
        }
    };
    if first > base_subdivision + 1 {
        if let Err(reason) = screen(first - 1) {
            rejected.push(format!(
                "{}: {reason}",
                span(base_subdivision + 1, first - 1)
            ));
        }
    }
    // Building a mother is what costs: after eight builds in a row fail,
    // each further one skips twice as far ahead.
    let mut failed_builds = 0u32;
    let mut next_build = first;
    for subdivision in first..=finest {
        if subdivision < next_build {
            continue;
        }
        let Screen {
            ratio,
            cap,
            focus,
            factor: needed_factor,
        } = match screen(subdivision) {
            Ok(pass) => pass,
            Err(reason) => {
                rejected.push(format!("n={subdivision}: {reason}"));
                continue;
            }
        };
        let mother = match MotherGrid::generate(subdivision) {
            Ok(grid) => grid.mesh,
            Err(error) => {
                rejected.push(format!("n={subdivision}: {error}"));
                continue;
            }
        };
        let active = mother.active_vertex_slots().collect::<Vec<_>>();
        let area0 = dual_areas(&mother);
        let mut factor = needed_factor;
        // The analytic factor meets the level at each demanded point; cells
        // straddling the demand's edge may still measure a level short. Step
        // it up while there is room under the cap.
        for _ in 0..8 {
            let mut mesh = mother.clone();
            let mut moved = active
                .iter()
                .map(|&v| unit(xyz(mesh.vertices()[v])).unwrap_or([0.0, 0.0, 1.0]))
                .collect::<Vec<_>>();
            earthmesh_mesh::schmidt_stretch(&mut moved, focus, factor);
            for (&v, p) in active.iter().zip(&moved) {
                let q = mesh.vertices()[v];
                let r = (q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
                mesh.move_vertex(v, CartesianPoint::new(p[0] * r, p[1] * r, p[2] * r));
            }
            let area1 = dual_areas(&mesh);
            // Against the base: the unstretched cell scaled to the base
            // spacing is `ratio^2` times its own area.
            let measured = active
                .iter()
                .map(|&v| (0.5 * (area0[v] * ratio * ratio / area1[v]).log2() + 1e-9).floor())
                .collect::<Vec<_>>();
            if measured.iter().any(|&l| !l.is_finite() || l < 0.0) {
                rejected.push(format!(
                    "n={subdivision}, factor {factor:.2}: cells fall below the base level"
                ));
                break;
            }
            let delivered = measured.iter().map(|&l| l as usize).collect::<Vec<_>>();
            let target = match TargetLevelField::from_active_voronoi_cells(&mesh, delivered.clone())
            {
                Ok(target) => target,
                Err(error) => {
                    rejected.push(format!("n={subdivision}: {error}"));
                    break;
                }
            };
            let final_requirements =
                match certify_final_cell_requirements_from_raster(raster, &mesh, &target, 1) {
                    Ok(report) => report,
                    Err(error) => {
                        let next = factor * 1.1;
                        if next > cap {
                            rejected.push(format!(
                                "n={subdivision}, factor {factor:.2}: {}",
                                format!("{error:?}").chars().take(160).collect::<String>()
                            ));
                            break;
                        }
                        factor = next;
                        continue;
                    }
                };
            let geometry = match crate::certify_geometry_with_contract(mesh, angle_contract) {
                CertifiedMeshOutcome::GeometryCertified(geometry) => *geometry,
                other => {
                    rejected.push(format!(
                        "n={subdivision}, factor {factor:.2}: geometry {}",
                        format!("{other:?}").chars().take(160).collect::<String>()
                    ));
                    break;
                }
            };
            let lonlat = (
                focus[1].atan2(focus[0]).to_degrees(),
                focus[2].asin().to_degrees(),
            );
            return Ok(StretchedMother {
                geometry,
                subdivision,
                mother_ratio: ratio,
                factor,
                focus_lonlat: lonlat,
                delivered_levels: delivered,
                final_requirements,
                rejected,
            });
        }
        failed_builds += 1;
        next_build = subdivision + (1usize << failed_builds.saturating_sub(8).min(20));
    }
    over_budget(&mut rejected);
    Err(rejected)
}

/// What the analytic screen found a subdivision would take.
struct Screen {
    ratio: f64,
    cap: f64,
    focus: [f64; 3],
    factor: f64,
}

/// `n=a` or `n=a..b (k mothers)`.
fn span(first: usize, last: usize) -> String {
    if first == last {
        format!("n={first}")
    } else {
        format!("n={first}..{last} ({} mothers)", last - first + 1)
    }
}

#[cfg(test)]
mod tests;
