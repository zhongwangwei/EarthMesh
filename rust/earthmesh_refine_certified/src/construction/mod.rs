//! CMRC's construction: the certified mother grid, its reverse coarsening
//! against a requirement -- a raster, or the merge criteria's lattice field
//! -- and the certificates, over the whole sphere or a built region. The
//! algorithm only: the CLI composes the requirement from the inputs and
//! publishes what this returns.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::time::Instant;

use earthmesh_geometry::GridRegion;
use earthmesh_mesh::MeshState;

use crate::AngleContractId;

mod options;
pub use options::{CertifiedDelivery, CertifiedMode, CertifiedRunOptions};

/// What became of an experimental local update: kept as the candidate, with
/// the caller's report, or refused -- the control kept -- with the reason.
#[derive(Clone, Debug, PartialEq)]
pub enum LocalUpdateDecision<R> {
    Candidate(R),
    Control(String),
}

/// A local update the caller applies to the coarsened mesh before the final
/// gates (`EARTHMESH_CMRC_LOCAL_UPDATE`): it reads its own request and moves
/// the mesh, given the delivered levels, the twelve pentagons and the angle
/// contract, and reports. An `InvalidData` error refuses the candidate; any
/// other error aborts the run.
pub type LocalUpdate<'a, R> =
    dyn Fn(&mut MeshState, &[usize], &[usize; 12], AngleContractId) -> io::Result<R> + 'a;

pub fn cmrc_timing_enabled() -> bool {
    std::env::var("EARTHMESH_CMRC_TIMING").as_deref() == Ok("1")
}

pub fn log_cmrc_phase(enabled: bool, phase: &str, started: &mut Instant) {
    if enabled {
        eprintln!(
            "earthmesh_cli: cmrc_timing phase={phase} elapsed_ms={}",
            started.elapsed().as_millis()
        );
        *started = Instant::now();
    }
}

pub struct CertifiedConstruction<R = ()> {
    pub geometry: Box<crate::GeometryCertifiedMotherGrid>,
    pub pentagons: [usize; 12],
    pub remap: crate::remap::ConservativeRemap,
    pub remap_certificate: crate::remap::RemapCertificate,
    pub final_cell_requirements: Option<crate::FinalCellRequirementCertificate>,
    pub delivered_level: usize,
    pub delivered_levels: Vec<usize>,
    pub coarsening_strategy: &'static str,
    pub initial_subdivision: usize,
    pub final_subdivision: usize,
    pub initial_cells: usize,
    pub attempted_patches: usize,
    pub accepted_patches: usize,
    pub removed_vertices: usize,
    pub removed_faces: usize,
    pub search_budget_exhausted: bool,
    pub components_total: usize,
    pub components_committed: usize,
    pub components_promoted: usize,
    pub components_exhausted: usize,
    pub search_complete: bool,
    pub elastic_report: Option<crate::coarsen::ElasticCmrcReport>,
    /// What became of an experimental local update, when one was asked.
    pub local_update: Option<LocalUpdateDecision<R>>,
    /// The built region, for a run with a delivery domain (guide 11.116):
    /// its final mesh is open at the frame's far edge, or closed when the
    /// region reaches round the sphere. `None` for a global run.
    pub region: Option<RegionPublication>,
}

/// What a regional run's publication covers: the final mesh is the built
/// region's -- R, certified cell by cell, inside its frame F -- open at F's
/// far edge; nothing outside it was built, certified or published. When R
/// and F reach round the sphere nothing is settled and the mesh is closed.
/// Counts for the certificate and the manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionPublication {
    /// The radius `pcvt` normalizes circumcentres to on the whole sphere --
    /// its first site's, the lattice vertex of rank 0 -- so the region's
    /// dual vertices do not depend on which site the region happens to
    /// number first.
    pub site_radius: f64,
    /// For each active final site, in slot order: whether it has a cell.
    /// Sites on the outer boundary -- settled by construction -- have none,
    /// and the published rows (and remap targets) number the others.
    pub cell_sites: Vec<bool>,
    /// Final sites on the outer boundary.
    pub outer_sites: usize,
    /// Cells certified cell by cell, each with its remap row.
    pub certified_cells: usize,
    /// Finest mother cells built, of `whole_cells` on the sphere.
    pub built_cells: usize,
    pub whole_cells: usize,
    pub base_subdivision: usize,
    pub delivered_base_faces: usize,
    pub region_base_faces: usize,
    pub frame_base_faces: usize,
    pub settled_base_faces: usize,
}

impl RegionPublication {
    /// `levels`, one per active final site, kept for the sites with cells:
    /// one per published W row.
    pub fn published<T: Copy>(&self, levels: &[T]) -> Vec<T> {
        levels
            .iter()
            .zip(&self.cell_sites)
            .filter_map(|(&level, &cell)| cell.then_some(level))
            .collect()
    }
}

pub fn certified_subdivision(base_nxp: usize, level: usize) -> io::Result<usize> {
    let scale = 1usize.checked_shl(level as u32).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC level overflows the platform subdivision range",
        )
    })?;
    base_nxp.checked_mul(scale).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC mother subdivision overflows usize",
        )
    })
}

/// What a regional construction's requirement comes from.
#[derive(Clone, Copy)]
pub enum RegionRequirement<'a> {
    /// The requirement raster the requirement plan composes.
    Raster(&'a crate::RasterLevelField),
    /// A lattice requirement field: a parent merges only if it is
    /// homogeneous (guide 11.111). Published as the region only.
    Lattice(&'a crate::requirement::heterogeneity::HeterogeneityField),
    /// The safe mother over the region: every cell at the chosen level,
    /// nothing coarsened, so no vertex gets degree 5 or 7 but the
    /// icosahedron's (guide 11.116).
    Uniform,
}

#[allow(clippy::too_many_arguments)]
pub fn build_certified_construction<R>(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    raster_requirements: &crate::RasterLevelField,
    max_tris: usize,
    local_update: Option<&LocalUpdate<'_, R>>,
    fixed_topology: bool,
    delivery_domain: Option<&GridRegion>,
    lattice: Option<&crate::requirement::heterogeneity::HeterogeneityField>,
) -> io::Result<CertifiedConstruction<R>> {
    let budget = options.maximum_cells.min(max_tris);
    // A run with a delivery domain is built as its region and published as
    // it; only a global run builds the whole sphere (guide 11.116). What
    // cannot be built as a region is refused, never rebuilt whole.
    if let Some(domain) = delivery_domain {
        if local_update.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the experimental CMRC local update moves a closed sphere; a regional run \
                 is built as its region and cannot take it",
            ));
        }
        let requirement = match (options.mode, lattice) {
            (_, Some(field)) => RegionRequirement::Lattice(field),
            (CertifiedMode::ReverseCoarsening, None) => {
                RegionRequirement::Raster(raster_requirements)
            }
            // The safe mother is the same lattice over the region alone; so
            // is any mode when no level is asked for.
            (CertifiedMode::SafeMotherOnly, None) => RegionRequirement::Uniform,
            (_, None) if chosen_level == 0 => RegionRequirement::Uniform,
            (mode, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "CMRC mode {mode:?} moves the vertices of a closed sphere (it serves \
                         a global ICON grid); a regional run is built as its region, with \
                         reverse_coarsening or safe_mother_only"
                    ),
                ));
            }
        };
        return build_region_certified_construction(
            base_nxp,
            chosen_level,
            options,
            requirement,
            budget,
            domain,
        );
    }
    if lattice.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC merge criteria need a regional domain",
        ));
    }
    let mixed_requirement = chosen_level > 0
        && raster_requirements
            .levels()
            .iter()
            .any(|&level| level < chosen_level);
    // Moved mothers: the vertices of a certified mother move toward the
    // demand, the connectivity stays, so no heptagon appears.
    let moved = match options.mode {
        CertifiedMode::StretchedMother => Some(
            crate::stretched_certified_mother(
                base_nxp,
                chosen_level,
                raster_requirements,
                options.angle_contract,
                budget,
            )
            .map(|stretched| crate::AdaptedMother {
                summary: format!(
                    "stretched mother n={} (factor {:.2} toward {:.3}E {:.3}N)",
                    stretched.subdivision,
                    stretched.factor,
                    stretched.focus_lonlat.0,
                    stretched.focus_lonlat.1
                ),
                geometry: stretched.geometry,
                subdivision: stretched.subdivision,
                strategy: "schmidt_stretch",
                delivered_levels: stretched.delivered_levels,
                final_requirements: stretched.final_requirements,
                rejected: stretched.rejected,
            }),
        ),
        CertifiedMode::EquidistributedMother => Some(crate::adapted_certified_mother(
            base_nxp,
            chosen_level,
            raster_requirements,
            options.angle_contract,
            budget,
        )),
        _ => None,
    };
    match moved {
        Some(Ok(adapted)) => {
            for reason in &adapted.rejected {
                eprintln!("earthmesh_cli: CMRC moved mother: set aside {reason}");
            }
            let safe_subdivision = certified_subdivision(base_nxp, chosen_level)?;
            eprintln!(
                "earthmesh_cli: CMRC moved mother: {} serves level {chosen_level}; the safe \
                 mother would be n={safe_subdivision} ({:.2}x the cells)",
                adapted.summary,
                (safe_subdivision as f64 / adapted.subdivision as f64).powi(2),
            );
            let geometry = Box::new(adapted.geometry);
            let cell_count = geometry.primal().vertex_count();
            let face_count = geometry.primal().triangle_count();
            let pentagons = certified_mother_pentagons(geometry.primal())?;
            // Only vertices moved: every cell is its mother cell, so the
            // lineage is the identity.
            let remap = crate::remap::ConservativeRemap::identity_for_mesh(geometry.primal());
            let remap_certificate = remap.certify_identity(cell_count);
            return Ok(CertifiedConstruction {
                initial_cells: face_count,
                geometry,
                pentagons,
                remap,
                remap_certificate,
                final_cell_requirements: Some(adapted.final_requirements),
                delivered_level: adapted.delivered_levels.iter().copied().max().unwrap_or(0),
                delivered_levels: adapted.delivered_levels,
                coarsening_strategy: adapted.strategy,
                initial_subdivision: adapted.subdivision,
                final_subdivision: adapted.subdivision,
                attempted_patches: 0,
                accepted_patches: 0,
                removed_vertices: 0,
                removed_faces: 0,
                search_budget_exhausted: false,
                components_total: 0,
                components_committed: 0,
                components_promoted: 0,
                components_exhausted: 0,
                search_complete: true,
                elastic_report: None,
                local_update: None,
                region: None,
            });
        }
        // No moved mother serves a scattered demand: c09's global DEM roughness
        // has none below the safe mother even searched one subdivision at a
        // time (guide 11.101). Fixed topology leaves nothing between them but
        // a grid the demand never asked for -- 64,002 cells at level 1 where
        // coarsening the same mother keeps 33,622. So where a 5/7 pair may
        // stand, which is every target but ICON, the safe mother is coarsened
        // where the demand allows, as reverse coarsening does.
        Some(Err(reasons)) if mixed_requirement && !fixed_topology => {
            eprintln!(
                "earthmesh_cli: warning: CMRC moved mother: none passes the final certificates; \
                 coarsening the safe mother where the demand allows instead, so the grid has \
                 vertices of degree 5 and 7 (an ICON delivery keeps the safe mother). {}",
                reasons.join("; ")
            );
            return build_mixed_certified_construction(
                base_nxp,
                chosen_level,
                options,
                raster_requirements,
                budget,
                local_update,
            );
        }
        Some(Err(reasons)) => {
            eprintln!(
                "earthmesh_cli: CMRC moved mother: none passes the final certificates; \
                 delivering the safe mother. {}",
                reasons.join("; ")
            );
        }
        None => {}
    }
    if matches!(
        options.mode,
        CertifiedMode::SafeMotherOnly
            | CertifiedMode::StretchedMother
            | CertifiedMode::EquidistributedMother
    ) {
        let subdivision = certified_subdivision(base_nxp, chosen_level)?;
        let mut config = crate::CertifiedConfig::mother_only(subdivision);
        config.angle_contract = options.angle_contract;
        config.max_cells = Some(budget);
        config.grading_ring_width = options.gradation_rings_per_level;
        config.delivery = match options.delivery {
            CertifiedDelivery::Tri => crate::DeliveryMode::Triangular,
            CertifiedDelivery::Hex => crate::DeliveryMode::Voronoi,
            CertifiedDelivery::Coupled => crate::DeliveryMode::Coupled,
        };
        let geometry = match crate::generate_certified_mother_grid(&config) {
            crate::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
            other => return Err(certified_outcome_error(other)),
        };
        let cell_count = geometry.primal().vertex_count();
        let face_count = geometry.primal().triangle_count();
        let pentagons = certified_mother_pentagons(geometry.primal())?;
        let remap = crate::remap::ConservativeRemap::identity_for_mesh(geometry.primal());
        let remap_certificate = remap.certify_identity(cell_count);
        return Ok(CertifiedConstruction {
            initial_cells: face_count,
            geometry,
            pentagons,
            remap,
            remap_certificate,
            final_cell_requirements: None,
            delivered_level: chosen_level,
            delivered_levels: vec![chosen_level; cell_count],
            coarsening_strategy: "none",
            initial_subdivision: subdivision,
            final_subdivision: subdivision,
            attempted_patches: 0,
            accepted_patches: 0,
            removed_vertices: 0,
            removed_faces: 0,
            search_budget_exhausted: false,
            components_total: 0,
            components_committed: 0,
            components_promoted: 0,
            components_exhausted: 0,
            search_complete: true,
            elastic_report: None,
            local_update: None,
            region: None,
        });
    }

    if mixed_requirement {
        return build_mixed_certified_construction(
            base_nxp,
            chosen_level,
            options,
            raster_requirements,
            budget,
            local_update,
        );
    }

    let initial_level = chosen_level.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC initial level overflows usize",
        )
    })?;
    if initial_level > options.maximum_level {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "CMRC MaximumLevelReached: reverse coarsening needs safe level {initial_level}, maximum {}",
                options.maximum_level
            ),
        ));
    }
    let initial_subdivision = certified_subdivision(base_nxp, initial_level)?;
    let required_cells =
        crate::mother_grid::mother_cell_count(initial_subdivision).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "CMRC initial mother cell count overflows usize",
            )
        })?;
    if required_cells > budget {
        return Err(certified_outcome_error(
            crate::CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells,
                budget,
            },
        ));
    }
    let fine = crate::MotherGrid::generate(initial_subdivision).map_err(io::Error::other)?;
    crate::Certificate::final_delivery_for(options.angle_contract)
        .verify_mother_grid(&fine)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC initial mother certification failed: {error}"),
            )
        })?;
    let initial_cells = fine.mesh.triangle_count();
    let initial_mesh = fine.mesh.clone();
    let hierarchy_components_total =
        crate::coarsen::complete_four_child_patch_candidates(&fine).len();
    match crate::coarsen::rebuild_one_level_from_complete_mother_patches(
        fine,
        options.search_budget,
    ) {
        crate::coarsen::HierarchyRebuildOutcome::Rebuilt {
            mesh,
            removed_vertices,
            removed_faces,
            candidates,
            remap: _,
            remap_certificate: _,
        } => {
            let remap = crate::remap::ConservativeRemap::between_voronoi_meshes(
                &initial_mesh,
                mesh.primal(),
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CMRC Voronoi remap failed: {error}"),
                )
            })?;
            let remap_certificate = remap.certify_spherical_overlap(
                initial_mesh.vertex_count(),
                mesh.primal().vertex_count(),
            );
            Ok(CertifiedConstruction {
                pentagons: certified_mother_pentagons(mesh.primal())?,
                delivered_levels: vec![chosen_level; mesh.primal().active_vertex_slots().count()],
                coarsening_strategy: "complete_global_hierarchy_2_to_1",
                geometry: mesh,
                remap,
                remap_certificate,
                final_cell_requirements: None,
                delivered_level: chosen_level,
                initial_subdivision,
                final_subdivision: certified_subdivision(base_nxp, chosen_level)?,
                initial_cells,
                attempted_patches: candidates.len(),
                accepted_patches: candidates.len(),
                removed_vertices,
                removed_faces,
                search_budget_exhausted: false,
                components_total: candidates.len(),
                components_committed: candidates.len(),
                components_promoted: 0,
                components_exhausted: 0,
                search_complete: true,
                elastic_report: None,
                local_update: None,
                region: None,
            })
        }
        crate::coarsen::HierarchyRebuildOutcome::SearchBudgetExhausted {
            attempted_patches,
            snapshot_unchanged,
            mesh,
        } => {
            if !snapshot_unchanged {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "CMRC exhausted hierarchy search changed its rollback snapshot",
                ));
            }
            let geometry =
                match crate::certify_mother_grid_with_contract(mesh, options.angle_contract) {
                    crate::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
                    other => return Err(certified_outcome_error(other)),
                };
            let cell_count = geometry.primal().vertex_count();
            let remap = crate::remap::ConservativeRemap::identity_for_mesh(geometry.primal());
            let remap_certificate = remap.certify_identity(cell_count);
            let pentagons = certified_mother_pentagons(geometry.primal())?;
            Ok(CertifiedConstruction {
                geometry,
                pentagons,
                remap,
                remap_certificate,
                final_cell_requirements: None,
                delivered_level: initial_level,
                delivered_levels: vec![initial_level; cell_count],
                coarsening_strategy: "retained_fine_mother_after_budget_exhaustion",
                initial_subdivision,
                final_subdivision: initial_subdivision,
                initial_cells,
                attempted_patches,
                accepted_patches: 0,
                removed_vertices: 0,
                removed_faces: 0,
                search_budget_exhausted: true,
                components_total: hierarchy_components_total,
                components_committed: 0,
                components_promoted: hierarchy_components_total,
                components_exhausted: 1,
                search_complete: false,
                elastic_report: None,
                local_update: None,
                region: None,
            })
        }
        crate::coarsen::HierarchyRebuildOutcome::UnsupportedCavity { reason, .. } => {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("CMRC CriterionNotCertifiable: {reason}"),
            ))
        }
    }
}

/// Base faces that a regional run delivers cells of: a corner, the centroid
/// or an edge midpoint inside the domain, or a point of the domain itself
/// (bbox corners and centre, circle centre, polygon vertices) within the
/// face's cap -- so a domain smaller than a face is found too. On the whole
/// base: the oracle `delivery_base_faces_by_address` is tested against.
#[cfg(test)]
fn delivery_base_faces(
    domain: &GridRegion,
    base: &crate::MotherGrid,
) -> BTreeSet<crate::TriangleAddress> {
    let prepared = domain.prepared();
    let own = domain_points(domain);
    let mut faces = BTreeSet::new();
    for face in base.mesh.active_triangle_slots() {
        let corners = base.mesh.triangles()[face].map(|site| {
            let point = base.mesh.vertices()[site];
            [point.x, point.y, point.z]
        });
        if face_meets_domain(corners, &prepared, &own) {
            if let Some(address) = base.triangle_addresses[face] {
                faces.insert(address);
            }
        }
    }
    faces
}

/// The domain's own points -- bbox corners and centre, circle centre,
/// polygon vertices -- as unit vectors.
fn domain_points(domain: &GridRegion) -> Vec<[f64; 3]> {
    fn points(domain: &GridRegion, out: &mut Vec<(f64, f64)>) {
        match domain {
            GridRegion::Bbox {
                west,
                east,
                north,
                south,
            } => {
                let span = (east - west).rem_euclid(360.0);
                out.extend([
                    (*west, *south),
                    (*west, *north),
                    (*east, *south),
                    (*east, *north),
                    (west + span / 2.0, (south + north) / 2.0),
                ]);
            }
            GridRegion::Circle { lon, lat, .. } => out.push((*lon, *lat)),
            GridRegion::Close { points } => {
                out.extend(points.iter().map(|point| (point.lon, point.lat)))
            }
            GridRegion::Any(regions) => {
                for region in regions {
                    points(region, out);
                }
            }
        }
    }
    let mut own = Vec::new();
    points(domain, &mut own);
    own.into_iter()
        .map(|(lon, lat)| unit_lon_lat(lon, lat))
        .collect()
}

fn unit_lon_lat(lon: f64, lat: f64) -> [f64; 3] {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn arc_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let length = |p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    ((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (length(a) * length(b)))
        .clamp(-1.0, 1.0)
        .acos()
}

/// Whether a base face with these corners delivers cells: a corner, the
/// centroid or an edge midpoint inside the domain, or a point of the domain
/// within the face's cap.
#[cfg(test)]
fn face_meets_domain(
    corners: [[f64; 3]; 3],
    prepared: &earthmesh_geometry::PreparedGridRegion<'_>,
    own: &[[f64; 3]],
) -> bool {
    face_meets_domain_by(corners, prepared, |centroid, radius| {
        own.iter()
            .any(|&point| arc_between(point, centroid) <= radius)
    })
}

/// `face_meets_domain` with the domain's own points behind `own_within`:
/// whether one of them lies within the radius of the (unnormalized)
/// centroid, by `arc_between`.
fn face_meets_domain_by(
    corners: [[f64; 3]; 3],
    prepared: &earthmesh_geometry::PreparedGridRegion<'_>,
    own_within: impl Fn([f64; 3], f64) -> bool,
) -> bool {
    let lon_lat = |point: [f64; 3]| {
        let length = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
        (
            point[1].atan2(point[0]).to_degrees(),
            (point[2] / length).clamp(-1.0, 1.0).asin().to_degrees(),
        )
    };
    let sum = |points: &[[f64; 3]]| {
        points.iter().fold([0.0; 3], |sum, point| {
            [sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]]
        })
    };
    let centroid = sum(&corners);
    let mut samples = corners.to_vec();
    samples.push(centroid);
    for side in 0..3 {
        samples.push(sum(&[corners[side], corners[(side + 1) % 3]]));
    }
    let radius = corners
        .iter()
        .map(|&corner| arc_between(centroid, corner))
        .fold(0.0, f64::max);
    samples.iter().any(|&sample| {
        let (lon, lat) = lon_lat(sample);
        prepared.contains(lon, lat)
    }) || own_within(centroid, radius)
}

/// The domain's own points binned on a latitude-longitude grid of tiles, so
/// that a face asks only the points that may lie in its cap. A polygon of a
/// hundred thousand vertices made every face outside it scan them all: the
/// Heihe basin's 130,000 took 345 s on a 2 km base (guide 11.119).
struct OwnPoints<'a> {
    points: &'a [[f64; 3]],
    tile: f64,
    rows: i64,
    columns: i64,
    /// The occupied tiles, by row and then column: a query walks only
    /// these, never the empty tiles of a band.
    bins: BTreeMap<i64, BTreeMap<i64, Vec<usize>>>,
}

impl<'a> OwnPoints<'a> {
    /// Tiles `tile` degrees wide, about the faces' size.
    fn new(points: &'a [[f64; 3]], tile: f64) -> Self {
        let tile = if tile.is_finite() {
            tile.clamp(1.0e-6, 90.0)
        } else {
            90.0
        };
        let rows = (180.0 / tile).ceil() as i64;
        let columns = (360.0 / tile).ceil() as i64;
        let mut index = Self {
            points,
            tile,
            rows,
            columns,
            bins: Default::default(),
        };
        for (slot, &point) in points.iter().enumerate() {
            let (lon, lat) = lon_lat_degrees(point);
            let (row, column) = (index.row(lat), index.column(lon));
            index
                .bins
                .entry(row)
                .or_default()
                .entry(column)
                .or_default()
                .push(slot);
        }
        index
    }

    fn row(&self, lat: f64) -> i64 {
        (((lat + 90.0) / self.tile).floor() as i64).clamp(0, self.rows - 1)
    }

    fn column(&self, lon: f64) -> i64 {
        (((lon + 180.0) / self.tile).floor() as i64).rem_euclid(self.columns)
    }

    /// Whether a point lies within `radius` of `centre`, which need not be
    /// of unit length: the test `arc_between(point, centre) <= radius`,
    /// asked of the points in the tiles a cap that wide can reach -- the
    /// rows of its latitude band and, clear of the poles by a tile, the
    /// columns within `asin(sin r / cos lat)` of its centre's longitude;
    /// every column nearer a pole. A tile more each way covers the rounding
    /// of the points' own coordinates.
    fn any_within(&self, centre: [f64; 3], radius: f64) -> bool {
        let (lon, lat) = lon_lat_degrees(centre);
        let reach = (radius + 1.0e-9).to_degrees();
        let (south, north) = (lat - reach, lat + reach);
        let span = (south - self.tile > -90.0 && north + self.tile < 90.0)
            .then(|| {
                (reach.to_radians().sin() / lat.to_radians().cos())
                    .asin()
                    .to_degrees()
            })
            .filter(|half| half.is_finite())
            .map(|half| (half / self.tile).ceil() as i64 + 2)
            .filter(|&span| 2 * span + 1 < self.columns);
        // The column window, split where it crosses the dateline column.
        let windows = match span {
            Some(span) => {
                let (first, last) = (self.column(lon) - span, self.column(lon) + span);
                if first < 0 {
                    [(0, last), (first + self.columns, self.columns - 1)]
                } else if last >= self.columns {
                    [(first, self.columns - 1), (0, last - self.columns)]
                } else {
                    [(first, last), (1, 0)]
                }
            }
            None => [(0, self.columns - 1), (1, 0)],
        };
        let near = |slots: &Vec<usize>| {
            slots
                .iter()
                .any(|&slot| arc_between(self.points[slot], centre) <= radius)
        };
        self.bins
            .range(self.row(south) - 1..=self.row(north) + 1)
            .any(|(_, columns)| {
                windows
                    .iter()
                    .filter(|(first, last)| first <= last)
                    .any(|&(first, last)| columns.range(first..=last).any(|(_, slots)| near(slots)))
            })
    }
}

/// Longitude and latitude in degrees of a point of any length.
fn lon_lat_degrees(point: [f64; 3]) -> (f64, f64) {
    let length = (point[0] * point[0] + point[1] * point[1] + point[2] * point[2]).sqrt();
    (
        point[1].atan2(point[0]).to_degrees(),
        (point[2] / length).clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

/// A cap -- centre and angular radius -- holding every point of `domain`
/// and its own points, one per part of a union; the whole sphere for a part
/// with no known bounds.
fn domain_caps(domain: &GridRegion) -> Vec<([f64; 3], f64)> {
    if let GridRegion::Any(regions) = domain {
        return regions.iter().flat_map(domain_caps).collect();
    }
    let Some(bounds) = domain.lonlat_bounds() else {
        return vec![([0.0, 0.0, 1.0], std::f64::consts::PI)];
    };
    let (south, north) = (bounds.south.max(-90.0), bounds.north.min(90.0));
    // Caps about either pole hold any band; a box clear of the poles and
    // narrower than the globe is held by the cap about its centre through
    // its farthest corner (distance from the centre grows along both its
    // parallels and its meridians towards the corners).
    let mut caps = vec![
        ([0.0, 0.0, 1.0], (90.0 - south).to_radians()),
        ([0.0, 0.0, -1.0], (90.0 + north).to_radians()),
    ];
    if bounds.width < 360.0 && south > -90.0 && north < 90.0 {
        let centre = unit_lon_lat(bounds.west + bounds.width / 2.0, (south + north) / 2.0);
        let east = bounds.west + bounds.width;
        let radius = [
            (bounds.west, south),
            (bounds.west, north),
            (east, south),
            (east, north),
        ]
        .into_iter()
        .map(|(lon, lat)| arc_between(centre, unit_lon_lat(lon, lat)))
        .fold(0.0, f64::max);
        caps.push((centre, radius));
    }
    let (centre, radius) = caps
        .into_iter()
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .expect("two pole caps");
    vec![(centre, radius + 1.0e-9)]
}

/// What the region's coarsening left: the final mesh, each final site's
/// source slot in the region mother, each source slot's delivered level, the
/// last commit's remap and the report -- or, for the safe mother, the mother
/// itself.
struct RegionCoarsening {
    mesh: MeshState,
    source_slots: Vec<Option<usize>>,
    delivered: Vec<Option<usize>>,
    remap: Option<crate::remap::ConservativeRemap>,
    report: Option<crate::coarsen::ElasticCmrcReport>,
}

/// Lattice faces a delivery domain may be walked over before the base is
/// too fine for it: a regional domain on a fine base is thousands.
const DELIVERY_WALK_LIMIT: usize = 1 << 24;

/// `delivery_base_faces` by lattice address, for bases too fine to build
/// whole: the faces whose cap meets one of the domain's caps (`domain_caps`)
/// are walked from the face holding its centre -- through faces within the
/// longest edge more, which no face between them and the centre is beyond --
/// and tested as `delivery_base_faces` tests every face.
pub fn delivery_base_faces_by_address(
    domain: &GridRegion,
    base_n: usize,
) -> io::Result<BTreeSet<crate::TriangleAddress>> {
    use crate::mother_grid::lattice;
    let invalid_data = |error: String| io::Error::new(io::ErrorKind::InvalidData, error);
    let longest = lattice::longest_edge(base_n).map_err(invalid_data)?;
    let mut candidates = BTreeSet::new();
    for (centre, radius) in domain_caps(domain) {
        let start = lattice::locate(base_n, centre)
            .ok_or_else(|| invalid_data("a delivery domain centre lies on no base face".into()))?;
        let mut visited = BTreeSet::from([start]);
        let mut queue = std::collections::VecDeque::from([start]);
        while let Some(face) = queue.pop_front() {
            let (face_centre, face_radius) = lattice::face_cap(face).map_err(invalid_data)?;
            let apart = arc_between(centre, face_centre);
            if apart > radius + face_radius + longest {
                continue;
            }
            if apart <= radius + face_radius {
                candidates.insert(face);
            }
            for next in lattice::faces_around(face).map_err(invalid_data)? {
                if visited.insert(next) {
                    if visited.len() > DELIVERY_WALK_LIMIT {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "the delivery domain spans more than {DELIVERY_WALK_LIMIT} faces \
                                 of the level-{base_n} base: a domain that large on a base that \
                                 fine is a global run"
                            ),
                        ));
                    }
                    queue.push_back(next);
                }
            }
        }
    }
    let prepared = domain.prepared();
    let own = domain_points(domain);
    let own = OwnPoints::new(&own, (2.0 * longest).to_degrees());
    let mut faces = BTreeSet::new();
    for face in candidates {
        let corners = lattice::face_corner_points(face).map_err(invalid_data)?;
        if face_meets_domain_by(corners, &prepared, |centroid, radius| {
            own.any_within(centroid, radius)
        }) {
            faces.insert(face);
        }
    }
    Ok(faces)
}

/// The widest transition a component's search may grow to, in parent rings:
/// the initial ring and five halo expansions (guide 11.123). The width is
/// reached only where a failed candidate's promotions ask for it. With four
/// rings the 20 km 30 m trial stuck at its third level, where one component
/// needed five.
const MAXIMUM_TRANSITION_RINGS: usize = 6;

/// Parent rings at every level that a transaction may touch beyond a parent
/// that cannot coarsen: the widest transition ring
/// (`MAXIMUM_TRANSITION_RINGS`), the elastic domain around it (two ordinary
/// rings) and the certificates' neighbourhood of what moved (one).
const REGION_PARENT_RINGS: usize = MAXIMUM_TRANSITION_RINGS + 2 + 1;

/// A regional run (guide 11.116): reverse coarsening with the finest mother
/// built only where the requirement reaches and the domain delivers (design
/// B1, guide 11.106) -- R and its frame F -- and published as that region
/// (guide 11.109); the settled base faces S are counted, never built. When R
/// and F reach round the sphere nothing is settled and the region is the
/// closed sphere. What the region cannot represent is an error: a regional
/// run is never rebuilt over the whole sphere.
fn build_region_certified_construction<R>(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    requirement: RegionRequirement<'_>,
    budget: usize,
    delivery_domain: &GridRegion,
) -> io::Result<CertifiedConstruction<R>> {
    use crate::{coarsen, mother_grid, on_demand, MotherGrid};
    let timing_enabled = cmrc_timing_enabled();
    let mut phase_started = Instant::now();
    let invalid_data = |error: String| io::Error::new(io::ErrorKind::InvalidData, error);
    let initial_subdivision = certified_subdivision(base_nxp, chosen_level)?;
    let whole_faces = mother_grid::mother_cell_count(initial_subdivision).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC mixed mother cell count overflows usize",
        )
    })?;
    let whole_vertices = whole_faces / 2 + 2;
    // Everything up to the assembly is computed by lattice address: the
    // base is never built whole before it (guide 11.108).
    let delivered = delivery_base_faces_by_address(delivery_domain, base_nxp)?;
    let margins = on_demand::ExtentMargins {
        gradation_rings_per_level: options.gradation_rings_per_level,
        parent_rings_per_level: REGION_PARENT_RINGS,
    };
    let extent = match requirement {
        RegionRequirement::Raster(raster) => on_demand::materialization_extent_by_address(
            raster,
            base_nxp,
            chosen_level,
            margins,
            &delivered,
        ),
        RegionRequirement::Lattice(field) => field
            .seed_faces(field.reach_rings(options.gradation_rings_per_level))
            .and_then(|seeds| {
                on_demand::materialization_extent_from_seeds(
                    base_nxp,
                    chosen_level,
                    margins,
                    &seeds,
                    &delivered,
                )
            }),
        // Nothing coarsens: the region is the domain and its margins.
        RegionRequirement::Uniform => on_demand::materialization_extent_from_seeds(
            base_nxp,
            chosen_level,
            margins,
            &BTreeSet::new(),
            &delivered,
        ),
    }
    .map_err(invalid_data)?;
    if extent.region.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CMRC regional run: the delivery domain covers no base face",
        ));
    }
    let built = extent.built_faces().collect::<BTreeSet<_>>();
    let fine = MotherGrid::generate_faces(
        initial_subdivision,
        mother_grid::region::descendant_faces(built.iter().copied(), initial_subdivision)
            .map_err(invalid_data)?,
    )
    .map_err(invalid_data)?;
    let required_cells = fine.mesh.triangle_count();
    if required_cells > budget {
        return Err(certified_outcome_error(
            crate::CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells,
                budget,
            },
        ));
    }
    eprintln!(
        "earthmesh_cli: cmrc_region delivered_base_faces={} region_base_faces={} frame_base_faces={} settled_base_faces={} built_cells={required_cells} whole_cells={whole_faces}",
        delivered.len(),
        extent.region.len(),
        extent.frame.len(),
        extent.settled_faces
    );
    log_cmrc_phase(timing_enabled, "region_mother_grid", &mut phase_started);
    let outer = fine
        .region
        .as_ref()
        .map(|region| region.outer_boundary().clone())
        .unwrap_or_default();
    let euler = {
        let faces = fine.mesh.triangle_count();
        let edges = (3 * faces + fine.mesh.open_edge_count()) / 2;
        fine.mesh.vertex_count() as isize - edges as isize + faces as isize
    };
    crate::Certificate::internal_for(options.angle_contract)
        .verify_geometry_within(&fine.mesh, &outer, euler)
        .map_err(|error| {
            invalid_data(format!(
                "CMRC initial region mother certification failed: {error}"
            ))
        })?;
    log_cmrc_phase(
        timing_enabled,
        "initial_geometry_certificate",
        &mut phase_started,
    );
    let initial_mesh = fine.mesh.clone();
    let active_sites = initial_mesh.active_vertex_slots().collect::<Vec<_>>();
    let (coarsened, source_levels) = if let RegionRequirement::Uniform = requirement {
        // The safe mother: every site keeps itself, at the chosen level.
        let mut delivered = vec![None; initial_mesh.vertices().len()];
        for &site in &active_sites {
            delivered[site] = Some(chosen_level);
        }
        let mut source_slots = vec![None; initial_mesh.vertices().len()];
        for &site in &active_sites {
            source_slots[site] = Some(site);
        }
        (
            RegionCoarsening {
                mesh: initial_mesh.clone(),
                source_slots,
                delivered,
                remap: None,
                report: None,
            },
            None,
        )
    } else {
        let projected = match requirement {
            RegionRequirement::Raster(raster) => crate::region_required_levels_from_raster(
                raster,
                &initial_mesh,
                &outer,
                whole_vertices,
            )
            .map_err(|error| {
                invalid_data(format!("CMRC initial raster projection failed: {error}"))
            })?,
            RegionRequirement::Uniform => unreachable!("the uniform region is not projected"),
            RegionRequirement::Lattice(field) => {
                let levels = field.required_by_site(&fine).map_err(invalid_data)?;
                // The frame's far edge requires nothing by construction: the
                // extent reaches every demanding face.
                if let Some((slot, level)) = initial_mesh
                    .active_vertex_slots()
                    .zip(&levels)
                    .find(|(slot, &level)| level > 0 && outer.contains(slot))
                {
                    return Err(invalid_data(format!(
                        "CMRC merge criteria require level {level} at outer site {slot}"
                    )));
                }
                levels
            }
        };
        log_cmrc_phase(
            timing_enabled,
            "initial_requirement_projection",
            &mut phase_started,
        );
        let source_levels =
            crate::SourceLevelField::from_active_voronoi_cells(&initial_mesh, projected.clone())
                .map_err(invalid_data)?;
        let mut cell_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
        for (cell, &site) in active_sites.iter().enumerate() {
            cell_by_site[site] = cell;
        }
        let mut adjacency = vec![Vec::new(); active_sites.len()];
        for (left, right) in crate::requirement::target_site_edges(&initial_mesh) {
            let left = cell_by_site[left];
            let right = cell_by_site[right];
            adjacency[left].push(right);
            adjacency[right].push(left);
        }
        let graded = crate::requirement::graded_envelope(
            &adjacency,
            &projected,
            options.gradation_rings_per_level,
        );
        let mut graded_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
        for (&site, level) in active_sites.iter().zip(graded) {
            graded_by_site[site] = level;
        }
        log_cmrc_phase(timing_enabled, "graded_envelope", &mut phase_started);
        let settled = coarsen::SettledRegion::by_address(base_nxp, &built).map_err(invalid_data)?;
        let scope =
            coarsen::RegionScope::new(&fine, &extent.region, base_nxp).map_err(invalid_data)?;
        let epoch = coarsen::run_region_component_epochs(
            fine.clone(),
            &initial_mesh,
            &source_levels,
            &graded_by_site,
            &coarsen::ElasticCmrcConfig {
                angle_contract: options.angle_contract,
                max_level: chosen_level,
                max_adjacent_level_delta: 1,
                initial_transition_rings: 1,
                maximum_transition_rings: MAXIMUM_TRANSITION_RINGS,
                topology_states_per_component: options.search_budget.clamp(1, 10_000),
                elastic_iterations_per_topology: 256,
                interval_boxes_per_component: whole_faces.saturating_mul(3),
                total_transition_states: options.search_budget,
                allow_safe_fallback: false,
                retry_at_failure: true,
            },
            &coarsen::RegionEpochs {
                built_bases: built,
                settled: settled.clone(),
                scope,
            },
        );
        log_cmrc_phase(
            timing_enabled,
            "elastic_component_epochs",
            &mut phase_started,
        );
        let mut result = match epoch {
            coarsen::ElasticCmrcOutcome::Completed(result) => result,
            coarsen::ElasticCmrcOutcome::NotCertifiable { reason } => {
                return Err(invalid_data(format!(
                    "CMRC mixed coarsening lost final-cell certification: {reason}"
                )));
            }
            coarsen::ElasticCmrcOutcome::InvalidInput { reason } => {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, reason));
            }
        };
        let state_mesh = result.state.mesh();
        (
            RegionCoarsening {
                mesh: state_mesh.mesh.clone(),
                source_slots: state_mesh.source_vertex_slots.clone(),
                delivered: result.state.source_delivered_levels().to_vec(),
                remap: result.final_remap.take(),
                report: Some(result.report.clone()),
            },
            Some(source_levels),
        )
    };
    // The last commit's remap, its sources numbered by their place in the
    // whole fine mother (computed from their origins: nothing outside the
    // region is built). Nothing committed -- no level asked for, a closed
    // region whose demand coarsens nothing, the safe mother -- leaves every
    // cell its own.
    let committed_remap = coarsened.remap;
    let numbering = mother_grid::region::GlobalNumbering::new(initial_subdivision);
    let region_index = fine
        .region
        .as_ref()
        .ok_or_else(|| invalid_data("CMRC region lost its index".into()))?;
    let source_ids = active_sites
        .iter()
        .map(|&site| {
            region_index
                .origin(site)
                .map(|origin| numbering.rank(origin))
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| invalid_data("CMRC region site without an origin".into()))?;
    {
        // Published as the region (guide 11.109): the final mesh is the built
        // region's, open at the frame's far edge, and every certificate is
        // drawn on it -- the sphere outside is never built.
        let mesh = coarsened.mesh;
        let mut outer_sites = BTreeSet::new();
        let mut delivered_levels = Vec::new();
        // Pentagons the region holds keep their slots; the rest name the
        // gridfile's placeholder row.
        let mut pentagons = [1usize; 12];
        for compact in mesh.active_vertex_slots() {
            let slot = coarsened.source_slots[compact]
                .ok_or_else(|| invalid_data(format!("final site {compact} has no source")))?;
            if outer.contains(&slot) {
                outer_sites.insert(compact);
            }
            delivered_levels.push(coarsened.delivered[slot].ok_or_else(|| {
                invalid_data(format!("source site {slot} has no delivered level"))
            })?);
            let origin = region_index
                .origin(slot)
                .ok_or_else(|| invalid_data(format!("source site {slot} has no origin")))?;
            if let crate::VertexAddress::IcosahedronVertex(vertex) =
                mother_grid::region::origin_address(initial_subdivision, origin)
            {
                pentagons[vertex as usize] = compact;
            }
        }
        let region_remap = match committed_remap {
            Some(remap) => remap,
            None => crate::remap::ConservativeRemap::identity_on(
                &mesh,
                mesh.active_vertex_slots()
                    .enumerate()
                    .filter(|(_, slot)| !outer_sites.contains(slot))
                    .map(|(cell, _)| cell),
            ),
        };
        let final_levels =
            crate::TargetLevelField::from_active_voronoi_cells(&mesh, delivered_levels.clone())
                .map_err(invalid_data)?;
        let final_cell_requirements = match (requirement, &source_levels) {
            (RegionRequirement::Raster(raster), _) => {
                crate::certify_region_final_cell_requirements_from_raster(
                    raster,
                    &mesh,
                    &final_levels,
                    &outer_sites,
                    whole_vertices,
                    1,
                )
                .map(Some)
            }
            // Against the sites' requirement through the region's certified
            // remap: each final cell takes the highest level of the finest
            // cells it overlaps.
            (RegionRequirement::Lattice(_), Some(source_levels)) => {
                crate::certify_final_cell_requirements_with_remap(
                    &initial_mesh,
                    source_levels,
                    &mesh,
                    &final_levels,
                    1,
                    &region_remap,
                )
                .map(Some)
            }
            // The safe mother asks no requirement of its cells, as on the
            // whole sphere.
            _ => Ok(None),
        }
        .map_err(|error| invalid_data(format!("CMRC final-cell certification failed: {error}")))?;
        log_cmrc_phase(
            timing_enabled,
            "final_requirement_projection",
            &mut phase_started,
        );
        // Targets are the final mesh's cells, numbered as the published rows
        // number them: the sites off the outer boundary, in slot order.
        let cell_sites = mesh
            .active_vertex_slots()
            .map(|compact| !outer_sites.contains(&compact))
            .collect::<Vec<_>>();
        let mut published_row = vec![usize::MAX; cell_sites.len()];
        let mut rows = 0;
        for (target, &cell) in cell_sites.iter().enumerate() {
            if cell {
                published_row[target] = rows;
                rows += 1;
            }
        }
        if let Some(target) = region_remap.covered_targets().and_then(|covered| {
            covered
                .iter()
                .copied()
                .find(|&t| published_row[t] == usize::MAX)
        }) {
            return Err(invalid_data(format!(
                "CMRC region remap has a row for boundary site {target}, which has no cell"
            )));
        }
        let remap = region_remap.renumbered(
            |source| source_ids[source],
            |target| published_row[target],
            whole_vertices,
            crate::mesh_fingerprint(&mesh),
        );
        let certified_cells = remap.covered_targets().map_or(0, |covered| covered.len());
        log_cmrc_phase(timing_enabled, "voronoi_remap", &mut phase_started);
        let remap_certificate =
            remap.certify_spherical_overlap(whole_vertices, mesh.vertex_count());
        let built_vertices = fine.mesh.vertex_count();
        let geometry = match crate::certify_region_geometry_with_contract(
            mesh,
            &outer_sites,
            euler,
            options.angle_contract,
        ) {
            crate::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
            other => return Err(certified_outcome_error(other)),
        };
        log_cmrc_phase(
            timing_enabled,
            "final_geometry_certificate",
            &mut phase_started,
        );
        let report = coarsened.report;
        let counted = |count: fn(&crate::coarsen::ElasticCmrcReport) -> usize| {
            report.as_ref().map_or(0, count)
        };
        Ok(CertifiedConstruction {
            delivered_level: delivered_levels.iter().copied().max().unwrap_or(0),
            delivered_levels,
            coarsening_strategy: if report.is_some() {
                "elastic_component_epochs"
            } else {
                "none"
            },
            pentagons,
            initial_subdivision,
            final_subdivision: initial_subdivision,
            initial_cells: required_cells,
            attempted_patches: counted(|report| report.components_total),
            accepted_patches: counted(|report| report.components_committed),
            removed_vertices: built_vertices - geometry.primal().vertex_count(),
            removed_faces: required_cells - geometry.primal().triangle_count(),
            search_budget_exhausted: report
                .as_ref()
                .is_some_and(|report| !report.search_complete),
            components_total: counted(|report| report.components_total),
            components_committed: counted(|report| report.components_committed),
            components_promoted: counted(|report| report.components_promoted),
            components_exhausted: counted(|report| report.components_exhausted),
            search_complete: report.as_ref().is_none_or(|report| report.search_complete),
            geometry,
            remap,
            remap_certificate,
            final_cell_requirements,
            elastic_report: report,
            local_update: None,
            region: Some(RegionPublication {
                site_radius: earthmesh_mesh::magnitude(
                    mother_grid::region::origin_position(
                        initial_subdivision,
                        mother_grid::region::VertexOrigin {
                            face: 0,
                            i: 0,
                            j: 0,
                        },
                    )
                    .map_err(invalid_data)?,
                ),
                cell_sites,
                outer_sites: outer_sites.len(),
                certified_cells,
                built_cells: required_cells,
                whole_cells: whole_faces,
                base_subdivision: base_nxp,
                delivered_base_faces: delivered.len(),
                region_base_faces: extent.region.len(),
                frame_base_faces: extent.frame.len(),
                settled_base_faces: extent.settled_faces,
            }),
        })
    }
}

/// Reverse coarsening of a global run's mixed requirement: the finest mother
/// over the whole sphere, coarsened where the demand allows.
pub fn build_mixed_certified_construction<R>(
    base_nxp: usize,
    chosen_level: usize,
    options: &CertifiedRunOptions,
    raster_requirements: &crate::RasterLevelField,
    budget: usize,
    local_update: Option<&LocalUpdate<'_, R>>,
) -> io::Result<CertifiedConstruction<R>> {
    let timing_enabled = cmrc_timing_enabled();
    let mut phase_started = Instant::now();
    let initial_subdivision = certified_subdivision(base_nxp, chosen_level)?;
    let required_cells =
        crate::mother_grid::mother_cell_count(initial_subdivision).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "CMRC mixed mother cell count overflows usize",
            )
        })?;
    if required_cells > budget {
        return Err(certified_outcome_error(
            crate::CertifiedMeshOutcome::CellBudgetInsufficient {
                required_cells,
                budget,
            },
        ));
    }
    let fine = crate::MotherGrid::generate(initial_subdivision).map_err(io::Error::other)?;
    log_cmrc_phase(timing_enabled, "mother_grid", &mut phase_started);
    let source_pentagons = certified_icosahedron_vertices(&fine.addresses)?;
    crate::Certificate::internal_for(options.angle_contract)
        .verify_mother_grid(&fine)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC initial mixed mother certification failed: {error}"),
            )
        })?;
    log_cmrc_phase(
        timing_enabled,
        "initial_geometry_certificate",
        &mut phase_started,
    );
    let initial_mesh = fine.mesh.clone();
    let initial_vertices = initial_mesh.vertex_count();
    let initial_faces = initial_mesh.triangle_count();
    let initial_levels = crate::TargetLevelField::from_active_voronoi_cells(
        &initial_mesh,
        vec![chosen_level; initial_mesh.active_vertex_slots().count()],
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let projected = crate::certify_final_cell_requirements_from_raster(
        raster_requirements,
        &initial_mesh,
        &initial_levels,
        1,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("CMRC initial raster projection failed: {error}"),
        )
    })?;
    log_cmrc_phase(
        timing_enabled,
        "initial_requirement_projection",
        &mut phase_started,
    );
    let source_levels = crate::SourceLevelField::from_active_voronoi_cells(
        &initial_mesh,
        projected.required_levels().to_vec(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let active_sites = initial_mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut cell_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
    for (cell, &site) in active_sites.iter().enumerate() {
        cell_by_site[site] = cell;
    }
    let mut adjacency = vec![Vec::new(); active_sites.len()];
    for (left, right) in crate::requirement::target_site_edges(&initial_mesh) {
        let left = cell_by_site[left];
        let right = cell_by_site[right];
        adjacency[left].push(right);
        adjacency[right].push(left);
    }
    let graded = crate::requirement::graded_envelope(
        &adjacency,
        projected.required_levels(),
        options.gradation_rings_per_level,
    );
    let mut graded_by_site = vec![usize::MAX; initial_mesh.vertices().len()];
    for (&site, level) in active_sites.iter().zip(graded) {
        graded_by_site[site] = level;
    }
    log_cmrc_phase(timing_enabled, "graded_envelope", &mut phase_started);
    let epoch = crate::coarsen::run_elastic_component_epochs(
        fine,
        &initial_mesh,
        &source_levels,
        &graded_by_site,
        &crate::coarsen::ElasticCmrcConfig {
            angle_contract: options.angle_contract,
            max_level: chosen_level,
            max_adjacent_level_delta: 1,
            initial_transition_rings: 1,
            maximum_transition_rings: MAXIMUM_TRANSITION_RINGS,
            topology_states_per_component: options.search_budget.clamp(1, 10_000),
            elastic_iterations_per_topology: 256,
            interval_boxes_per_component: initial_faces.saturating_mul(3),
            total_transition_states: options.search_budget,
            allow_safe_fallback: false,
            retry_at_failure: true,
        },
    );
    log_cmrc_phase(
        timing_enabled,
        "elastic_component_epochs",
        &mut phase_started,
    );
    let mut result = match epoch {
        crate::coarsen::ElasticCmrcOutcome::Completed(result) => result,
        crate::coarsen::ElasticCmrcOutcome::NotCertifiable { reason } => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("CMRC mixed coarsening lost final-cell certification: {reason}"),
            ));
        }
        crate::coarsen::ElasticCmrcOutcome::InvalidInput { reason } => {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, reason));
        }
    };
    let handed_on_remap = result.final_remap.take();
    let leaf_mesh = result.state.mesh();
    let pentagons = source_pentagons
        .into_iter()
        .map(|source| {
            leaf_mesh
                .source_vertex_slots
                .iter()
                .position(|slot| *slot == Some(source))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CMRC removed protected icosahedron vertex {source}"),
                    )
                })
        })
        .collect::<io::Result<Vec<_>>>()?
        .try_into()
        .expect("twelve source pentagons map to twelve compact sites");
    let mut mesh = leaf_mesh.mesh.clone();
    let delivered_levels = result
        .state
        .target_levels()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .levels()
        .to_vec();
    // Trial updates precede every final source/remap/geometry gate.
    // Invalid input aborts; a rejected trial cannot mutate the retained control.
    let local_update = local_update
        .map(|apply| {
            let mut candidate = mesh.clone();
            match apply(
                &mut candidate,
                &delivered_levels,
                &pentagons,
                options.angle_contract,
            ) {
                Ok(report) => {
                    mesh = candidate;
                    Ok(LocalUpdateDecision::Candidate(report))
                }
                Err(error) if error.kind() == io::ErrorKind::InvalidData => {
                    Ok(LocalUpdateDecision::Control(error.to_string()))
                }
                Err(error) => Err(error),
            }
        })
        .transpose()?;
    let final_cell_requirements =
        if mesh == initial_mesh && delivered_levels.iter().all(|&level| level == chosen_level) {
            projected
        } else {
            let final_levels =
                crate::TargetLevelField::from_active_voronoi_cells(&mesh, delivered_levels.clone())
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            crate::certify_final_cell_requirements_from_raster(
                raster_requirements,
                &mesh,
                &final_levels,
                1,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CMRC final-cell certification failed: {error}"),
                )
            })?
        };
    log_cmrc_phase(
        timing_enabled,
        "final_requirement_projection",
        &mut phase_started,
    );
    let remap = if initial_mesh == mesh {
        crate::remap::ConservativeRemap::identity_for_mesh(&mesh)
    } else if let Some(remap) = handed_on_remap.filter(|remap| remap.joins(&initial_mesh, &mesh)) {
        // The last committed component certified this very remap; computing it
        // again cost as much as all the components' own (guide 11.103).
        remap
    } else {
        crate::remap::ConservativeRemap::between_voronoi_meshes(&initial_mesh, &mesh).map_err(
            |error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CMRC mixed Voronoi remap failed: {error}"),
                )
            },
        )?
    };
    log_cmrc_phase(timing_enabled, "voronoi_remap", &mut phase_started);
    let remap_certificate =
        remap.certify_spherical_overlap(initial_mesh.vertex_count(), mesh.vertex_count());
    let geometry = match crate::certify_geometry_with_contract(mesh, options.angle_contract) {
        crate::CertifiedMeshOutcome::GeometryCertified(mesh) => mesh,
        other => return Err(certified_outcome_error(other)),
    };
    log_cmrc_phase(
        timing_enabled,
        "final_geometry_certificate",
        &mut phase_started,
    );
    Ok(CertifiedConstruction {
        delivered_level: delivered_levels.iter().copied().max().unwrap_or(0),
        delivered_levels,
        coarsening_strategy: "elastic_component_epochs",
        pentagons,
        initial_subdivision,
        final_subdivision: initial_subdivision,
        initial_cells: initial_faces,
        attempted_patches: result.report.components_total,
        accepted_patches: result.report.components_committed,
        removed_vertices: initial_vertices - geometry.primal().vertex_count(),
        removed_faces: initial_faces - geometry.primal().triangle_count(),
        search_budget_exhausted: !result.report.search_complete,
        components_total: result.report.components_total,
        components_committed: result.report.components_committed,
        components_promoted: result.report.components_promoted,
        components_exhausted: result.report.components_exhausted,
        search_complete: result.report.search_complete,
        geometry,
        remap,
        remap_certificate,
        final_cell_requirements: Some(final_cell_requirements),
        elastic_report: Some(result.report.clone()),
        local_update,
        region: None,
    })
}

pub fn certified_outcome_error(outcome: crate::CertifiedMeshOutcome) -> io::Error {
    use crate::CertifiedMeshOutcome;
    let (kind, message) = match outcome {
        CertifiedMeshOutcome::CellBudgetInsufficient {
            required_cells,
            budget,
        } => (
            io::ErrorKind::OutOfMemory,
            format!(
                "CMRC CellBudgetInsufficient: requires {required_cells} cells, budget is {budget}"
            ),
        ),
        CertifiedMeshOutcome::MaximumLevelReached {
            requested_level,
            max_level,
        } => (
            io::ErrorKind::InvalidInput,
            format!("CMRC MaximumLevelReached: requested {requested_level}, maximum {max_level}"),
        ),
        CertifiedMeshOutcome::CriterionNotCertifiable { reason } => (
            io::ErrorKind::Unsupported,
            format!("CMRC CriterionNotCertifiable: {reason}"),
        ),
        CertifiedMeshOutcome::PhysicalCriterionUnsatisfiable { reason } => (
            io::ErrorKind::InvalidData,
            format!("CMRC PhysicalCriterionUnsatisfiable: {reason}"),
        ),
        CertifiedMeshOutcome::UnsupportedBoundaryConstraint { reason } => (
            io::ErrorKind::Unsupported,
            format!("CMRC UnsupportedBoundaryConstraint: {reason}"),
        ),
        CertifiedMeshOutcome::SearchBudgetExhausted { attempted_patches } => (
            io::ErrorKind::TimedOut,
            format!("CMRC SearchBudgetExhausted after {attempted_patches} patches"),
        ),
        CertifiedMeshOutcome::InternalCertificationFailure { reason } => (
            io::ErrorKind::InvalidData,
            format!("CMRC InternalCertificationFailure: {reason}"),
        ),
        CertifiedMeshOutcome::CompressionIncomplete {
            fulfillment,
            reason,
            ..
        } => (
            io::ErrorKind::Other,
            format!(
                "CMRC CompressionIncomplete: {reason}; requested={:?}; delivered={:?}; components committed/promoted/exhausted={}/{}/{}; search_complete={}",
                fulfillment.requested_histogram,
                fulfillment.delivered_histogram,
                fulfillment.components_committed,
                fulfillment.components_promoted,
                fulfillment.components_exhausted,
                fulfillment.search_complete,
            ),
        ),
        CertifiedMeshOutcome::GeometryCertified(_)
        | CertifiedMeshOutcome::Certified(_)
        | CertifiedMeshOutcome::CertifiedAdaptive { .. }
        | CertifiedMeshOutcome::CertifiedSafeFallback { .. } => (
            io::ErrorKind::InvalidData,
            "CMRC returned an unexpected success outcome".to_string(),
        ),
    };
    io::Error::new(kind, message)
}

pub fn certified_mother_pentagons(mesh: &MeshState) -> io::Result<[usize; 12]> {
    let mut degree = vec![0usize; mesh.vertices().len()];
    for triangle in mesh.active_triangle_slots() {
        for vertex in mesh.triangles()[triangle] {
            degree[vertex] += 1;
        }
    }
    let sites: Vec<_> = mesh
        .active_vertex_slots()
        .filter(|&site| degree[site] == 5)
        .collect();
    sites.try_into().map_err(|sites: Vec<usize>| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "CMRC certified mother grid has {} degree-5 sites, expected 12",
                sites.len()
            ),
        )
    })
}

pub fn certified_icosahedron_vertices(
    addresses: &[Option<crate::VertexAddress>],
) -> io::Result<[usize; 12]> {
    let mut sites = addresses
        .iter()
        .enumerate()
        .filter_map(|(site, address)| match address {
            Some(crate::VertexAddress::IcosahedronVertex(vertex)) => Some((*vertex, site)),
            _ => None,
        })
        .collect::<Vec<_>>();
    sites.sort_unstable();
    sites
        .into_iter()
        .map(|(_, site)| site)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|sites: Vec<usize>| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CMRC mother grid has {} icosahedron vertices, expected 12",
                    sites.len()
                ),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use earthmesh_geometry::LonLatPoint;

    #[test]
    fn delivered_faces_by_address_are_the_whole_bases() {
        let domains = vec![
            GridRegion::Bbox {
                west: 100.0,
                east: 104.0,
                north: 27.0,
                south: 23.0,
            },
            GridRegion::Bbox {
                west: 175.0,
                east: -170.0,
                north: -10.0,
                south: -30.0,
            },
            GridRegion::Bbox {
                west: -180.0,
                east: 180.0,
                north: 90.0,
                south: 70.0,
            },
            GridRegion::Bbox {
                west: 10.0,
                east: 10.2,
                north: 45.1,
                south: 45.0,
            },
            GridRegion::Circle {
                lon: 91.0,
                lat: 31.0,
                radius_km: 300.0,
            },
            GridRegion::Circle {
                lon: -60.0,
                lat: -89.0,
                radius_km: 500.0,
            },
            GridRegion::Circle {
                lon: 0.0,
                lat: 0.0,
                radius_km: 5.0,
            },
            GridRegion::Close {
                points: [(30.0, 10.0), (36.0, 11.0), (34.0, 16.0), (31.0, 14.0)]
                    .into_iter()
                    .map(|(lon, lat)| LonLatPoint { lon, lat })
                    .collect(),
            },
            GridRegion::Any(vec![
                GridRegion::Circle {
                    lon: 120.0,
                    lat: 40.0,
                    radius_km: 200.0,
                },
                GridRegion::Bbox {
                    west: -80.0,
                    east: -75.0,
                    north: 5.0,
                    south: 0.0,
                },
            ]),
        ];
        for n in [1, 2, 5, 12, 40] {
            let base = crate::MotherGrid::generate(n).unwrap();
            for domain in &domains {
                let whole = delivery_base_faces(domain, &base);
                let by_address = delivery_base_faces_by_address(domain, n).unwrap();
                assert!(!whole.is_empty(), "n {n} {domain:?}");
                assert_eq!(by_address, whole, "n {n} {domain:?}");
            }
        }
    }

    /// A deterministic stream of numbers in [0, 1) (splitmix64).
    fn uniform(state: &mut u64) -> f64 {
        *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Points clustered where tiles are odd -- the poles, the dateline, the
    /// meridian of a tile edge -- and scattered over the sphere, each with
    /// its scale of jitter in degrees.
    fn awkward_points(state: &mut u64, count: usize) -> Vec<[f64; 3]> {
        let seeds = [
            (0.0, 90.0),
            (77.0, -90.0),
            (180.0, 10.0),
            (-180.0, -45.0),
            (179.99, 89.9),
            (-0.5, 0.0),
            (100.0, 38.0),
        ];
        (0..count)
            .map(|slot| {
                let (lon, lat, jitter) = match slot % 3 {
                    0 => {
                        let (lon, lat) = seeds[(uniform(state) * seeds.len() as f64) as usize];
                        (lon, lat, 10f64.powf(-6.0 + 6.0 * uniform(state)))
                    }
                    1 => (
                        360.0 * uniform(state) - 180.0,
                        (2.0 * uniform(state) - 1.0).asin().to_degrees(),
                        0.0,
                    ),
                    _ => (100.0, 38.0, 3.0),
                };
                let lon = lon + jitter * (2.0 * uniform(state) - 1.0);
                let lat = (lat + jitter * (2.0 * uniform(state) - 1.0)).clamp(-90.0, 90.0);
                unit_lon_lat(lon, lat)
            })
            .collect()
    }

    #[test]
    fn the_tiled_own_points_find_what_the_scan_finds() {
        let mut state = 7;
        let points = awkward_points(&mut state, 3000);
        let centres = awkward_points(&mut state, 3000);
        let mut found = 0;
        for tile in [1.0e-4, 0.01, 0.4, 7.0, 90.0] {
            let index = OwnPoints::new(&points, tile);
            for (slot, &centre) in centres.iter().enumerate() {
                // An unnormalized centre, as a face's corner sum is.
                let centre = centre.map(|value| value * (0.4 + 2.0 * uniform(&mut state)));
                // Radii about the tile and far from it, and exactly at the
                // distance of a point, where the test is `<=`.
                let radius = match slot % 4 {
                    0 => (tile * 10f64.powf(2.0 * uniform(&mut state) - 1.5)).to_radians(),
                    1 => 10f64.powf(-7.0 + 7.5 * uniform(&mut state)),
                    2 => arc_between(points[slot % points.len()], centre),
                    _ => std::f64::consts::PI * uniform(&mut state),
                };
                let scan = points
                    .iter()
                    .any(|&point| arc_between(point, centre) <= radius);
                assert_eq!(
                    index.any_within(centre, radius),
                    scan,
                    "tile {tile} centre {centre:?} radius {radius}"
                );
                found += usize::from(scan);
            }
        }
        // Both answers occur.
        assert!(found > 1000 && found < 14000, "{found}");
    }

    #[test]
    fn a_dense_polygon_delivers_by_address_what_the_whole_base_does() {
        // A ring of thousands of vertices -- what a basin outline is --
        // round a point near the dateline and one near a pole.
        let ring = |lon: f64, lat: f64, radius: f64, count: usize| GridRegion::Close {
            points: (0..count)
                .map(|step| {
                    let angle = std::f64::consts::TAU * step as f64 / count as f64;
                    let wobble = 1.0 + 0.3 * (7.0 * angle).sin();
                    LonLatPoint {
                        lon: lon + radius * wobble * angle.cos() / lat.to_radians().cos(),
                        lat: lat + radius * wobble * angle.sin(),
                    }
                })
                .collect(),
        };
        for domain in [ring(179.0, -20.0, 1.5, 4000), ring(40.0, 84.0, 1.0, 4000)] {
            for n in [12, 40] {
                let base = crate::MotherGrid::generate(n).unwrap();
                let whole = delivery_base_faces(&domain, &base);
                assert!(!whole.is_empty(), "n {n}");
                assert_eq!(delivery_base_faces_by_address(&domain, n).unwrap(), whole);
            }
        }
    }
}
