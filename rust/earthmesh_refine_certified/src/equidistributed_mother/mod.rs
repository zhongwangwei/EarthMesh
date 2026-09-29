//! A certified mother grid with its vertices redistributed toward the demand
//! (guide 11.88).
//!
//! The Schmidt stretch (guide 11.87) moves every vertex by one conformal map,
//! so it serves one focus. Here the vertices move freely on the sphere to
//! minimise, per triangle,
//!
//!   shape `l^2 / (4 sqrt(3) A)` (1 for an equilateral triangle, unbounded as
//!   it flattens) plus `beta (A/T + T/A - 2)` (0 at the target area `T`,
//!   unbounded as the area vanishes),
//!
//! where the targets follow a gradient-limited size field of the demand. The
//! connectivity never changes, so every vertex keeps its degree -- twelve of
//! degree 5, the rest 6 -- and no heptagon can appear. A line search that
//! never accepts an inverted triangle keeps the mesh valid throughout.
//!
//! What a fixed topology cannot do is hold a large contrast in one mesh: each
//! ring of mother vertices stays a ring, so shrinking cells inside a region
//! stretches the rings around it (anisotropy `~ 1 / (1 - r dln h/dr)`). The
//! spare cells of a mother finer than the minimum are therefore spent on
//! lowering the contrast ("water-filling": no cell is made coarser than a cap
//! chosen as small as the cell count allows). A mother is tried only when that
//! contrast is small enough to be worth it, and it counts only when CMRC's own
//! final certificates pass.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use earthmesh_mesh::CartesianPoint;
use rayon::prelude::*;

use crate::mother_geometry::{dual_areas, unit, xyz, P};
use crate::{
    certificate::{is_supported_mother_subdivision, AngleContractId},
    mother_grid::MotherGrid,
    outcome::{CertifiedMeshOutcome, GeometryCertifiedMotherGrid},
    requirement::{
        certify_final_cell_requirements_from_raster, FinalCellRequirementCertificate,
        RasterLevelField, TargetLevelField,
    },
};

/// Relative growth of the target size per unit distance away from a demand.
const GRADATION: f64 = 0.05;
/// Weights of the size term against the shape term: the first, and a
/// lighter one tried when the angles miss the certified window.
const SIZE_WEIGHTS: [f64; 2] = [0.3, 0.15];
/// A mother is tried only when its water-filled contrast (coarsest target /
/// finest) is at most this; beyond it a fixed topology cannot keep the
/// angles in the certified window (guide 11.88).
const MAX_CONTRAST: f64 = 3.0;
/// Fractions of the required size aimed at: the first that passes counts.
const MARGINS: [f64; 3] = [0.7, 0.65, 0.6];
const OUTER_ITERATIONS: usize = 8;
const INNER_ITERATIONS: usize = 200;

/// An equidistributed mother that passed every final certificate.
pub struct EquidistributedMother {
    pub geometry: GeometryCertifiedMotherGrid,
    /// Subdivision of the mother whose vertices were moved.
    pub subdivision: usize,
    /// Coarsest target size over the finest.
    pub contrast: f64,
    /// Fraction of the required size the targets aimed at.
    pub margin: f64,
    /// Each active Voronoi cell's delivered level, measured from its area.
    pub delivered_levels: Vec<usize>,
    pub final_requirements: FinalCellRequirementCertificate,
    /// Why each candidate tried before this one was not delivered.
    pub rejected: Vec<String>,
}

/// A gradient-limited target size (radians) on the requirement raster:
/// `base * 2^-level` at each cell, and nowhere larger than a neighbour's
/// size plus `GRADATION` times the distance to it.
struct SizeField {
    nlon: usize,
    nlat: usize,
    size: Vec<f64>,
}

fn centre(nlon: usize, nlat: usize, i: usize, j: usize) -> P {
    let lat = (-90.0 + (j as f64 + 0.5) * 180.0 / nlat as f64).to_radians();
    let lon = (-180.0 + (i as f64 + 0.5) * 360.0 / nlon as f64).to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn arc(a: P, b: P) -> f64 {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let chord = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    2.0 * (0.5 * chord).min(1.0).asin()
}

impl SizeField {
    fn new(raster: &RasterLevelField, base: f64) -> Self {
        let (nlon, nlat) = (raster.nlon(), raster.nlat());
        let mut size = raster
            .levels()
            .iter()
            .map(|&level| base * 2f64.powi(-(level.min(60) as i32)))
            .collect::<Vec<_>>();
        // Dijkstra on the 8-neighbour raster: sizes only ever shrink, and a
        // settled cell's size is final (the positive `g * d` keeps the order).
        let key = |h: f64| h.to_bits();
        let mut heap = size
            .iter()
            .enumerate()
            .filter(|(_, &h)| h < base)
            .map(|(index, &h)| Reverse((key(h), index)))
            .collect::<BinaryHeap<_>>();
        while let Some(Reverse((bits, index))) = heap.pop() {
            let h = f64::from_bits(bits);
            if h > size[index] {
                continue;
            }
            let (i, j) = (index % nlon, index / nlon);
            let here = centre(nlon, nlat, i, j);
            for dj in [-1i64, 0, 1] {
                let jj = j as i64 + dj;
                if jj < 0 || jj >= nlat as i64 {
                    continue;
                }
                for di in [-1i64, 0, 1] {
                    if di == 0 && dj == 0 {
                        continue;
                    }
                    let ii = (i as i64 + di).rem_euclid(nlon as i64) as usize;
                    let other = jj as usize * nlon + ii;
                    let candidate = h + GRADATION * arc(here, centre(nlon, nlat, ii, jj as usize));
                    if candidate < size[other] {
                        size[other] = candidate;
                        heap.push(Reverse((key(candidate), other)));
                    }
                }
            }
        }
        Self { nlon, nlat, size }
    }

    fn at(&self, p: P) -> f64 {
        let lat = p[2].clamp(-1.0, 1.0).asin().to_degrees();
        let lon = p[1].atan2(p[0]).to_degrees();
        let j = (((lat + 90.0) / 180.0 * self.nlat as f64) as usize).min(self.nlat - 1);
        let i = (((lon + 180.0) / 360.0 * self.nlon as f64) as usize).min(self.nlon - 1);
        self.size[j * self.nlon + i]
    }
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: P, s: f64) -> P {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// The triangle energy and, when asked, its gradient per vertex. `None`
/// when a triangle is inverted or degenerate: the energy is unbounded there.
fn energy(
    points: &[P],
    triangles: &[[usize; 3]],
    targets: &[f64],
    size_weight: f64,
    gradient: Option<&mut [P]>,
) -> Option<f64> {
    const SQRT3_4: f64 = 4.0 * 1.732_050_807_568_877_2;
    let want_gradient = gradient.is_some();
    let terms = triangles
        .par_iter()
        .zip(targets.par_iter())
        .map(|(&[ia, ib, ic], &target)| {
            let (a, b, c) = (points[ia], points[ib], points[ic]);
            let normal = cross(sub(b, a), sub(c, a));
            let length = dot(normal, normal).sqrt();
            if !length.is_finite() || length <= 1e-300 || dot(normal, add(add(a, b), c)) <= 0.0 {
                return None;
            }
            let area = 0.5 * length;
            let l2 =
                dot(sub(b, a), sub(b, a)) + dot(sub(c, a), sub(c, a)) + dot(sub(c, b), sub(c, b));
            let value = l2 / (SQRT3_4 * area) + size_weight * (area / target + target / area - 2.0);
            if !want_gradient {
                return Some((value, [[0.0; 3]; 3]));
            }
            let unit_normal = scale(normal, 1.0 / length);
            let d_area = [
                scale(cross(sub(b, c), unit_normal), 0.5),
                scale(cross(sub(c, a), unit_normal), 0.5),
                scale(cross(sub(a, b), unit_normal), 0.5),
            ];
            let d_l2 = [
                scale(sub(sub(scale(a, 2.0), b), c), 2.0),
                scale(sub(sub(scale(b, 2.0), a), c), 2.0),
                scale(sub(sub(scale(c, 2.0), a), b), 2.0),
            ];
            let e_area = -l2 / (SQRT3_4 * area * area)
                + size_weight * (1.0 / target - target / (area * area));
            let e_l2 = 1.0 / (SQRT3_4 * area);
            Some((
                value,
                [0, 1, 2].map(|k| add(scale(d_area[k], e_area), scale(d_l2[k], e_l2))),
            ))
        })
        .collect::<Option<Vec<_>>>()?;
    let total = terms.iter().map(|(value, _)| value).sum();
    if let Some(gradient) = gradient {
        gradient.iter_mut().for_each(|g| *g = [0.0; 3]);
        for ((_, parts), tri) in terms.iter().zip(triangles) {
            for k in 0..3 {
                gradient[tri[k]] = add(gradient[tri[k]], parts[k]);
            }
        }
        // Only the motion along the sphere matters.
        gradient
            .par_iter_mut()
            .zip(points.par_iter())
            .for_each(|(g, &x)| *g = sub(*g, scale(x, dot(*g, x))));
    }
    Some(total)
}

/// A dot product over all points, summed in a fixed order: chunk sums in
/// parallel, then those in sequence, so the result -- and the mesh -- does
/// not depend on the number of threads or how the work was split.
fn vdot(a: &[P], b: &[P]) -> f64 {
    a.par_chunks(4096)
        .zip(b.par_chunks(4096))
        .map(|(x, y)| x.iter().zip(y).map(|(p, q)| dot(*p, *q)).sum::<f64>())
        .collect::<Vec<_>>()
        .iter()
        .sum()
}

/// L-BFGS on the sphere: steps in the tangent planes, points renormalised,
/// and a backtracking line search that only accepts valid, lower-energy
/// meshes. Returns the points and the energy they reach.
fn minimise(
    mut x: Vec<P>,
    triangles: &[[usize; 3]],
    targets: &[f64],
    size_weight: f64,
    iterations: usize,
) -> Vec<P> {
    const MEMORY: usize = 12;
    let n = x.len();
    let mut g = vec![[0.0; 3]; n];
    let Some(mut e) = energy(&x, triangles, targets, size_weight, Some(&mut g)) else {
        return x;
    };
    let mut history = VecDeque::<(Vec<P>, Vec<P>, f64)>::new();
    let mut stalled = 0;
    let mut candidate = vec![[0.0; 3]; n];
    let mut g_new = vec![[0.0; 3]; n];
    for _ in 0..iterations {
        // Two-loop recursion.
        let mut q = g.clone();
        let mut alphas = Vec::with_capacity(history.len());
        for (s, y, rho) in history.iter().rev() {
            let alpha = rho * vdot(s, &q);
            q.par_iter_mut()
                .zip(y.par_iter())
                .for_each(|(qi, yi)| *qi = sub(*qi, scale(*yi, alpha)));
            alphas.push(alpha);
        }
        let gamma = match history.back() {
            Some((s, y, _)) => vdot(s, y) / vdot(y, y),
            None => 1e-3 / vdot(&g, &g).sqrt().max(1e-300),
        };
        q.par_iter_mut().for_each(|qi| *qi = scale(*qi, gamma));
        for ((s, y, rho), alpha) in history.iter().zip(alphas.iter().rev()) {
            let beta = rho * vdot(y, &q);
            q.par_iter_mut()
                .zip(s.par_iter())
                .for_each(|(qi, si)| *qi = add(*qi, scale(*si, alpha - beta)));
        }
        let mut direction = q.par_iter().map(|qi| scale(*qi, -1.0)).collect::<Vec<_>>();
        let mut slope = vdot(&direction, &g);
        if slope >= 0.0 {
            let norm = vdot(&g, &g).sqrt().max(1e-300);
            direction = g.par_iter().map(|gi| scale(*gi, -1e-3 / norm)).collect();
            slope = vdot(&direction, &g);
            history.clear();
        }
        let mut step = 1.0;
        let mut accepted = None;
        for _ in 0..40 {
            candidate
                .par_iter_mut()
                .zip(x.par_iter().zip(direction.par_iter()))
                .for_each(|(c, (xi, di))| *c = unit(add(*xi, scale(*di, step))).unwrap_or(*xi));
            if let Some(e_new) = energy(
                &candidate,
                triangles,
                targets,
                size_weight,
                Some(&mut g_new),
            ) {
                if e_new <= e + 1e-4 * step * slope {
                    accepted = Some(e_new);
                    break;
                }
            }
            step *= 0.5;
        }
        let Some(e_new) = accepted else {
            break;
        };
        let s = candidate
            .par_iter()
            .zip(x.par_iter())
            .map(|(c, xi)| sub(*c, *xi))
            .collect::<Vec<_>>();
        let y = g_new
            .par_iter()
            .zip(g.par_iter())
            .map(|(a, b)| sub(*a, *b))
            .collect::<Vec<_>>();
        let sy = vdot(&s, &y);
        if sy > 1e-12 {
            history.push_back((s, y, 1.0 / sy));
            if history.len() > MEMORY {
                history.pop_front();
            }
        }
        stalled = if (e - e_new).abs() <= 1e-10 * e.abs().max(1.0) {
            stalled + 1
        } else {
            0
        };
        std::mem::swap(&mut x, &mut candidate);
        std::mem::swap(&mut g, &mut g_new);
        e = e_new;
        if stalled >= 5 {
            break;
        }
    }
    x
}

fn centroid(points: &[P], [a, b, c]: [usize; 3]) -> P {
    unit(add(add(points[a], points[b]), points[c])).unwrap_or(points[a])
}

/// The smallest size cap for which the targets still give every triangle at
/// most `margin^2` of its required area, the cells being spent on lowering
/// the contrast; `None` when even uncapped sizes do not fit the cell count.
fn water_fill(
    required: &[f64],
    base: f64,
    kappa: f64,
    total_area: f64,
    margin: f64,
) -> Option<f64> {
    let need = total_area / (margin * margin);
    let sum = |cap: f64| {
        required
            .iter()
            .map(|&h| kappa * h.min(cap).powi(2))
            .sum::<f64>()
    };
    if sum(base) < need {
        return None;
    }
    let (mut lo, mut hi) = (required.iter().copied().fold(f64::INFINITY, f64::min), base);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if sum(mid) >= need {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(hi)
}

/// Try the supported mothers finer than the base and coarser than
/// `below_subdivision`, coarsest first; the first whose equidistributed
/// vertices pass CMRC's final certificates is returned, or why none did.
pub fn equidistributed_certified_mother(
    base_subdivision: usize,
    raster: &RasterLevelField,
    angle_contract: AngleContractId,
    max_cells: usize,
    below_subdivision: usize,
) -> Result<EquidistributedMother, Vec<String>> {
    let mut rejected = Vec::new();
    let n0 = base_subdivision as f64;
    let base = (4.0 * std::f64::consts::PI / (10.0 * n0 * n0 + 2.0)).sqrt();
    // Area of a base triangle per squared base size: sizes to target areas.
    let kappa = (4.0 * std::f64::consts::PI / (20.0 * n0 * n0)) / (base * base);
    let field = SizeField::new(raster, base);
    for subdivision in
        (base_subdivision + 1..below_subdivision).filter(|&n| is_supported_mother_subdivision(n))
    {
        let cells = 10usize
            .saturating_mul(subdivision)
            .saturating_mul(subdivision)
            + 2;
        if cells > max_cells {
            rejected.push(format!("n={subdivision}: {cells} cells exceed the budget"));
            break;
        }
        let mother = match MotherGrid::generate(subdivision) {
            Ok(grid) => grid.mesh,
            Err(error) => {
                rejected.push(format!("n={subdivision}: {error}"));
                continue;
            }
        };
        let slots = mother.active_vertex_slots().collect::<Vec<_>>();
        let mut index = vec![usize::MAX; mother.vertices().len()];
        for (i, &slot) in slots.iter().enumerate() {
            index[slot] = i;
        }
        let points0 = slots
            .iter()
            .map(|&v| unit(xyz(mother.vertices()[v])).unwrap_or([0.0, 0.0, 1.0]))
            .collect::<Vec<_>>();
        let triangles = mother
            .active_triangle_slots()
            .map(|t| {
                let mut tri = mother.triangles()[t].map(|v| index[v]);
                let (a, b, c) = (points0[tri[0]], points0[tri[1]], points0[tri[2]]);
                if dot(cross(sub(b, a), sub(c, a)), add(add(a, b), c)) < 0.0 {
                    tri.swap(1, 2);
                }
                tri
            })
            .collect::<Vec<_>>();
        let area0 = dual_areas(&mother);
        let areas = |points: &[P]| {
            triangles
                .iter()
                .map(|&[a, b, c]| {
                    let normal = cross(sub(points[b], points[a]), sub(points[c], points[a]));
                    0.5 * dot(normal, normal).sqrt()
                })
                .collect::<Vec<_>>()
        };
        let total_area = areas(&points0).iter().sum::<f64>();
        let required0 = triangles
            .iter()
            .map(|&t| field.at(centroid(&points0, t)))
            .collect::<Vec<_>>();
        let finest = required0.iter().copied().fold(f64::INFINITY, f64::min);
        let mut last_reason = None;
        for margin in MARGINS {
            let Some(cap) = water_fill(&required0, base, kappa, total_area, margin) else {
                last_reason = Some(format!("n={subdivision}: too few cells for the demand"));
                break;
            };
            let contrast = cap / finest;
            if contrast > MAX_CONTRAST {
                last_reason = Some(format!(
                    "n={subdivision}: contrast {contrast:.2} exceeds {MAX_CONTRAST}, more than a \
                     fixed topology holds within the angle window"
                ));
                break;
            }
            // Physical residuals ask for finer targets (the next margin);
            // angles outside the window ask for a lighter size weight.
            let mut next_margin = false;
            for size_weight in SIZE_WEIGHTS {
                let mut points = points0.clone();
                for _ in 0..OUTER_ITERATIONS {
                    let required = triangles
                        .iter()
                        .map(|&t| field.at(centroid(&points, t)))
                        .collect::<Vec<_>>();
                    let cap =
                        water_fill(&required, base, kappa, total_area, margin).unwrap_or(base);
                    let weights = required
                        .iter()
                        .map(|&h| h.min(cap).powi(2))
                        .collect::<Vec<_>>();
                    let area_now = areas(&points).iter().sum::<f64>();
                    let norm = area_now / weights.iter().sum::<f64>();
                    let targets = weights.iter().map(|w| w * norm).collect::<Vec<_>>();
                    points = minimise(points, &triangles, &targets, size_weight, INNER_ITERATIONS);
                }
                let mut mesh = mother.clone();
                for (&slot, p) in slots.iter().zip(&points) {
                    let q = mesh.vertices()[slot];
                    let r = (q.x * q.x + q.y * q.y + q.z * q.z).sqrt();
                    mesh.move_vertex(slot, CartesianPoint::new(p[0] * r, p[1] * r, p[2] * r));
                }
                let area1 = dual_areas(&mesh);
                let ratio = (subdivision as f64 / n0).powi(2);
                let measured = slots
                    .iter()
                    .map(|&v| (0.5 * (area0[v] * ratio / area1[v]).log2() + 1e-9).floor())
                    .collect::<Vec<_>>();
                if measured.iter().any(|&l| !l.is_finite() || l < 0.0) {
                    last_reason = Some(format!(
                        "n={subdivision}, margin {margin}, size weight {size_weight}: cells fall \
                         below the base level"
                    ));
                    next_margin = true;
                    break;
                }
                let delivered = measured.iter().map(|&l| l as usize).collect::<Vec<_>>();
                let target =
                    match TargetLevelField::from_active_voronoi_cells(&mesh, delivered.clone()) {
                        Ok(target) => target,
                        Err(error) => {
                            last_reason = Some(format!("n={subdivision}: {error}"));
                            break;
                        }
                    };
                let final_requirements =
                    match certify_final_cell_requirements_from_raster(raster, &mesh, &target, 1) {
                        Ok(report) => report,
                        Err(error) => {
                            last_reason = Some(format!(
                                "n={subdivision}, margin {margin}, size weight {size_weight}: {}",
                                format!("{error:?}").chars().take(160).collect::<String>()
                            ));
                            next_margin = true;
                            break;
                        }
                    };
                match crate::certify_geometry_with_contract(mesh, angle_contract) {
                    CertifiedMeshOutcome::GeometryCertified(geometry) => {
                        if let Some(reason) = last_reason {
                            rejected.push(reason);
                        }
                        return Ok(EquidistributedMother {
                            geometry: *geometry,
                            subdivision,
                            contrast,
                            margin,
                            delivered_levels: delivered,
                            final_requirements,
                            rejected,
                        });
                    }
                    other => {
                        last_reason = Some(format!(
                            "n={subdivision}, margin {margin}, size weight {size_weight}: \
                             geometry {}",
                            format!("{other:?}").chars().take(160).collect::<String>()
                        ));
                    }
                }
            }
            if !next_margin {
                // Neither weight kept the angles in the window: more cells.
                break;
            }
        }
        if let Some(reason) = last_reason {
            rejected.push(reason);
        }
    }
    Err(rejected)
}

#[cfg(test)]
mod tests;
