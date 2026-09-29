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
//! as fine as the base level. So a mother at level `k` can be stretched by
//! at most `2^k`, and it serves the demand only if the factor the demand
//! needs fits under that. The levels are tried from the coarsest up; each
//! candidate is stretched, its delivered levels are measured from the cells'
//! areas, and it counts only when CMRC's own final certificates pass --
//! geometry, physical and balance. A demand no stretch serves falls back to
//! the safe mother at the caller.

use earthmesh_mesh::{CartesianPoint, MeshState};

use crate::{
    certificate::{is_supported_mother_subdivision, AngleContractId},
    mother_grid::MotherGrid,
    outcome::{CertifiedMeshOutcome, GeometryCertifiedMotherGrid},
    requirement::{
        certify_final_cell_requirements_from_raster, FinalCellRequirementCertificate,
        RasterLevelField, TargetLevelField,
    },
};

type P = [f64; 3];

/// A stretched mother that passed every final certificate.
pub struct StretchedMother {
    pub geometry: GeometryCertifiedMotherGrid,
    /// Subdivision of the mother that was stretched.
    pub subdivision: usize,
    /// Its level above the base (`subdivision = base * 2^level`).
    pub mother_level: usize,
    pub factor: f64,
    /// Focus longitude and latitude, degrees.
    pub focus_lonlat: (f64, f64),
    /// Each active Voronoi cell's delivered level, measured from its area.
    pub delivered_levels: Vec<usize>,
    pub final_requirements: FinalCellRequirementCertificate,
    /// Why each candidate tried before this one was not delivered.
    pub rejected: Vec<String>,
}

fn unit(p: P) -> Option<P> {
    let r = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    (r.is_finite() && r > 0.0).then(|| [p[0] / r, p[1] / r, p[2] / r])
}

fn xyz(p: CartesianPoint) -> P {
    [p.x, p.y, p.z]
}

/// Voronoi (dual) area of every vertex slot, from the kites of each triangle
/// around its circumcentre; zero for inactive slots.
fn dual_areas(mesh: &MeshState) -> Vec<f64> {
    let cp = |p: P| CartesianPoint::new(p[0], p[1], p[2]);
    let mut area = vec![0.0; mesh.vertices().len()];
    for t in mesh.active_triangle_slots() {
        let tri = mesh.triangles()[t];
        let Some(p) = tri
            .iter()
            .map(|&v| unit(xyz(mesh.vertices()[v])))
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let w = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let Some(mut c) = unit([
            u[1] * w[2] - u[2] * w[1],
            u[2] * w[0] - u[0] * w[2],
            u[0] * w[1] - u[1] * w[0],
        ]) else {
            continue;
        };
        if c[0] * p[0][0] + c[1] * p[0][1] + c[2] * p[0][2] < 0.0 {
            c = [-c[0], -c[1], -c[2]];
        }
        for k in 0..3 {
            let (v, a, b) = (p[k], p[(k + 1) % 3], p[(k + 2) % 3]);
            let (Some(ma), Some(mb)) = (
                unit([v[0] + a[0], v[1] + a[1], v[2] + a[2]]),
                unit([v[0] + b[0], v[1] + b[1], v[2] + b[2]]),
            ) else {
                continue;
            };
            area[tri[k]] += earthmesh_mesh::spherical_kite_area_unit(cp(v), cp(ma), cp(mb), cp(c));
        }
    }
    area
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
    for level in 0..chosen_level {
        let Some(subdivision) = base_subdivision.checked_shl(level as u32) else {
            break;
        };
        if !is_supported_mother_subdivision(subdivision) {
            rejected.push(format!(
                "level {level}: mother n={subdivision} is not certified"
            ));
            continue;
        }
        let cells = 10usize
            .saturating_mul(subdivision)
            .saturating_mul(subdivision)
            + 2;
        if cells > max_cells {
            rejected.push(format!("level {level}: {cells} cells exceed the budget"));
            break;
        }
        // What this mother still has to gain, per raster cell.
        let relative = levels
            .iter()
            .map(|&l| l.saturating_sub(level))
            .collect::<Vec<_>>();
        // The focus: the demand's mean direction, weighted by the cells a
        // level adds and by the raster cell's area.
        let mut sum = [0.0; 3];
        for ((p, &l), &w) in points.iter().zip(&relative).zip(&area_weights) {
            if l > 0 {
                let weight = (4f64.powi(l.min(16) as i32) - 1.0) * w;
                for k in 0..3 {
                    sum[k] += weight * p[k];
                }
            }
        }
        let Some(focus) = unit(sum) else {
            rejected.push(format!("level {level}: the demand has no mean direction"));
            continue;
        };
        // Coarsening the far side by more than 2^level would take it below
        // the base level.
        let cap = 2f64.powi(level as i32);
        let spacing = (4.0 * std::f64::consts::PI / cells as f64).sqrt();
        let needed = earthmesh_mesh::schmidt_factor_for_levels(
            &points,
            &relative,
            focus,
            spacing / 2.0,
            cap,
        );
        if needed.unreachable > 0 {
            rejected.push(format!(
                "level {level}: one focus cannot serve the demand ({} raster cells beyond \
                 asin(2^-level) of it, farthest {:.1} deg)",
                needed.unreachable, needed.farthest_unreachable_deg
            ));
            continue;
        }
        if needed.capped {
            rejected.push(format!(
                "level {level}: the demand needs a factor above {cap}, which would take the \
                 far side below the base level"
            ));
            continue;
        }
        let mother = match MotherGrid::generate(subdivision) {
            Ok(grid) => grid.mesh,
            Err(error) => {
                rejected.push(format!("level {level}: {error}"));
                continue;
            }
        };
        let active = mother.active_vertex_slots().collect::<Vec<_>>();
        let area0 = dual_areas(&mother);
        let mut factor = needed.factor;
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
            let measured = active
                .iter()
                .map(|&v| level as f64 + (0.5 * (area0[v] / area1[v]).log2() + 1e-9).floor())
                .collect::<Vec<_>>();
            if measured.iter().any(|&l| !l.is_finite() || l < 0.0) {
                rejected.push(format!(
                    "level {level}, factor {factor:.2}: cells fall below the base level"
                ));
                break;
            }
            let delivered = measured.iter().map(|&l| l as usize).collect::<Vec<_>>();
            let target = match TargetLevelField::from_active_voronoi_cells(&mesh, delivered.clone())
            {
                Ok(target) => target,
                Err(error) => {
                    rejected.push(format!("level {level}: {error}"));
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
                                "level {level}, factor {factor:.2}: {}",
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
                        "level {level}, factor {factor:.2}: geometry {}",
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
                mother_level: level,
                factor,
                focus_lonlat: lonlat,
                delivered_levels: delivered,
                final_requirements,
                rejected,
            });
        }
    }
    Err(rejected)
}

#[cfg(test)]
mod tests;
