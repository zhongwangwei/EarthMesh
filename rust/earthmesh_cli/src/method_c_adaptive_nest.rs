//! Criteria-driven refinement as Method-C builds it.
//!
//! The demand itself -- circles per level, the pass report, the target-level
//! function the quality step reconciles against -- is the request layer's
//! (`refinement_demand::nest`) and serves every backend. What is here turns it
//! into a Method-C mesh: group the circles, refine group by group, repair or
//! drop what the transition template refuses. It lives beside
//! `redgreen_bridge` rather than inside the request layer, so the request
//! layer never names an algorithm crate.

use std::io;

use earthmesh_core::RefineConfig;
use earthmesh_mesh::{RefinementRegion, TriangularMesh};
use earthmesh_refine_method_c::MethodCMesh;

use crate::refinement_demand::ladder::MEASURED_PARENT_HALO_ROWS;
use crate::refinement_demand::nest::{
    adaptive_demand_circles_for_level_windows, AdaptiveNestReport, NestPassReport,
};
use crate::refinement_demand::plan::DemandPlanInputs;

/// What the nest spring needs, when one is configured.
///
/// `None` at the call site means no spring, which is not the same as a spring
/// of zero iterations: the first skips the spring-bearing `spawn_nest` overload
/// entirely, and the second would ask it for a smoothing pass that does nothing.
#[derive(Clone, Copy, Debug)]
pub struct AdaptiveNestSpring {
    /// The base mesh's NXP, which sets the spring's target edge length.
    pub nxp: usize,
    /// Iterations per pass, from `refinement_spring_iterations`.
    pub iterations: usize,
    /// Transition width, `MAX_MROWS_SURFACE` or `MAX_MROWS_ATMOS`.
    ///
    /// Chosen by the caller because `is_atmosmesh` lives there. The
    /// spring-free path reaches the same choice through bare `spawn_nest`,
    /// which hard-codes the surface width -- so passing the atmosphere width
    /// here is what finally makes the two routes agree for an atmosmesh.
    pub max_mrows: usize,
}

/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
///
/// `named_regions` are the regions the run asked for outright — a project's
/// `specified_circle`, a bbox, a closed curve. They are instructions, not
/// criteria, so they are refined whether or not any criterion also asks: a run
/// that names a circle and enables nothing else must still get that circle.
/// Criteria-driven refinement is suspended on the Method-C backend.
///
/// It does not fail loudly on its own, which is the problem. On a global
/// coastal run at NXP 81, 25 of 59 region groups were refused and the mesh that
/// came out was valid, passed its topology gates, and was not what the project
/// asked for -- the exact silent-failure shape section 11.1 of the technical
/// guide is about. Two attempts at the largest single cause were measured and
/// both cost more refinement than they recovered (196,548 faces down to
/// 185,592 for five tiles).
///
/// The constraint underneath is structural rather than a defect: Method-C seeds
/// on a lattice that steps three cells at a time, so refined area comes in
/// quanta of one seed footprint, and its perimeter has to be a multiple of
/// three, so a region shaped by a coastline is refused rather than approximated.
/// Named regions -- circles, corridors, boxes, closed curves -- are shapes
/// Method-C can build and are unaffected; it is the criteria-driven path, where
/// the shape comes from the data, that has no such guarantee.
///
/// Refusing is the honest answer until the red-green backend
/// (`earthmesh_refine_redgreen`) can serve it: there the judge chain grows a
/// marking it cannot take as given and never rejects a shape.
pub const METHOD_C_ADAPTIVE_SUSPENDED: &str = concat!(
    "criteria-driven (adaptive) refinement is suspended on the Method-C backend: ",
    "its seed lattice steps three cells at a time and its perimeter must be a multiple of three, ",
    "so a region whose shape comes from the data is refused rather than approximated -- ",
    "a global coastal run had 25 of 59 groups refused and still produced a mesh that passed every gate. ",
    "Named regions (circle, corridor, bbox, closed curve) are unaffected. ",
    "Set refinement.adaptive.enabled = false, or use named regions, until the red-green backend serves this path."
);

pub fn spawn_nest_adaptive_with_named_regions(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    named_regions: &[RefinementRegion],
    base_cell_meters: f64,
    max_level: usize,
    spring: Option<AdaptiveNestSpring>,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    spawn_nest_adaptive_with_named_region_windows(
        mesh,
        refine,
        std::slice::from_ref(inputs),
        named_regions,
        base_cell_meters,
        max_level,
        spring,
    )
}

pub fn spawn_nest_adaptive_with_named_region_windows(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &[DemandPlanInputs<'_>],
    named_regions: &[RefinementRegion],
    base_cell_meters: f64,
    max_level: usize,
    spring: Option<AdaptiveNestSpring>,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    if !base_cell_meters.is_finite() || base_cell_meters <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "base cell size must be positive and finite",
        ));
    }
    // A named region is an instruction. One asking for a level this run will
    // never reach would be filtered out by the per-level loop below and vanish
    // without a word, leaving a mesh that is valid, passes its checks, and is
    // coarser than the project asked for. The `deepest_level == 0` guard
    // downstream only catches it when *nothing* refined; a run that also names a
    // reachable level slips through. Refuse here instead.
    if let Some(region) = named_regions
        .iter()
        .find(|region| region.level() == 0 || region.level() > max_level)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "a named refinement region asks for level {} but this run refines to level \
                 {max_level}; raise the maximum level or lower the region's",
                region.level()
            ),
        ));
    }
    let mut current = mesh.clone();
    let mut passes = Vec::new();
    let mut deepest_level = 0usize;
    let mut stopped_on_empty_demand = false;
    let mut spring_passes = 0usize;
    // One place decides whether a pass springs, so the two call sites below --
    // the grouped one and the single-group one -- cannot drift apart. They did:
    // both called bare `spawn_nest`, and a run that configured
    // `SpringRegional_type` got no spring at all on this route while the direct
    // route sprang the same mesh. Guide 11.39.
    let refine_once = |mesh: &MethodCMesh,
                       regions: &[RefinementRegion],
                       level: usize|
     -> io::Result<(MethodCMesh, usize)> {
        match spring {
            Some(spring) => mesh.spawn_nest_with_spring_and_max_mrows(
                regions,
                level,
                spring.max_mrows,
                spring.nxp,
                spring.iterations,
            ),
            None => mesh.spawn_nest(regions, level).map(|mesh| (mesh, 0)),
        }
    };

    for level in 1..=max_level {
        let cell_meters = base_cell_meters / 2f64.powi((level - 1) as i32);
        let demand = adaptive_demand_circles_for_level_windows(
            refine,
            inputs,
            level,
            base_cell_meters,
            max_level,
        )?;
        // Named regions are shapes Method-C can build and stay served; it is
        // demand whose shape came from the data that is suspended. Testing
        // whether a criterion asked at all is what separates them exactly -- a
        // config that names no criterion, or one that finds nothing, demands
        // nothing and passes straight through.
        if demand.demanded {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                METHOD_C_ADAPTIVE_SUSPENDED,
            ));
        }
        let demanded_cells = demand.demanded_cells;
        let radius_meters = demand.radius_meters;
        let mut regions = demand.circles;
        regions.extend(
            named_regions
                .iter()
                .filter(|region| region.level() == level)
                .cloned(),
        );
        if regions.is_empty() {
            for region in named_regions.iter().filter(|region| region.level() > level) {
                regions.push(region_parent_for_level(region, level, base_cell_meters)?);
            }
            if regions.is_empty() {
                stopped_on_empty_demand = true;
                break;
            }
        }
        let faces_before = face_count(&current);
        // A single call unions every group's mask and emits once, which is the
        // cheap path -- but on a global demand field the blocks can sit close
        // enough that one block's transition patch lands on faces another block
        // already subdivided, and the whole pass dies for a collision between
        // two of hundreds of blocks. Refining group by group makes a collision
        // cost exactly the group that collided, named and counted; the rest of
        // the level still refines. Serial cost is one emit per group, which the
        // measured radius floor keeps affordable: it cut the circle count from
        // 114566 to 8190, so the groups are hundreds, not tens of thousands.
        let mut groups: Vec<Vec<RefinementRegion>> =
            earthmesh_refine_method_c::method_c_connected_region_groups(&regions, false)
                .into_iter()
                .flat_map(split_oversized_group)
                .collect();
        // Largest groups first. Serial refinement makes every earlier block a
        // wall for later ones: run small islands first and a continental band
        // arrives to ground already pocked with finer blocks -- its mask gets
        // carved around each one and shreds into fragments no repair can make
        // legal. Measured: the ten continental groups, 97% of the circles,
        // refused in size-ascending order; largest-first gives each big band
        // virgin ground, and the islands that follow only ever concede a strip
        // that their big neighbour already refined.
        groups.sort_by_key(|group| std::cmp::Reverse(group.len()));
        if groups.len() > 1 {
            eprintln!(
                "adaptive refine level {level}: {} circles in {} disjoint groups, refining each \
                 on its own",
                regions.len(),
                groups.len()
            );
            let mut refused_groups = 0usize;
            let mut refused_circles = 0usize;
            let mut first_reason: Option<String> = None;
            let report_every = (groups.len() / 20).max(1);
            // Groups are not comparable units of work: a refused one gives up
            // in milliseconds, while one that lands spends its time in
            // perimeter repair, which is quadratic in the block's boundary. A
            // count alone made a run that was working look like one that had
            // hung, so the line carries what the last group cost.
            let mut group_started = std::time::Instant::now();
            let mut last_group_circles = 0usize;
            for (index, group) in groups.iter().enumerate() {
                if index > 0 && index.is_multiple_of(report_every) {
                    eprintln!(
                        "adaptive refine level {level}: group {index}/{} ({} faces so far; group \
                         {} took {:.1}s over {last_group_circles} circles)",
                        groups.len(),
                        face_count(&current),
                        index - 1,
                        group_started.elapsed().as_secs_f64()
                    );
                }
                group_started = std::time::Instant::now();
                last_group_circles = group.len();
                let outcome = refine_once(&current, group, level);
                let reason = match outcome {
                    Ok((next, sprang)) => {
                        current = next;
                        spring_passes += sprang;
                        None
                    }
                    Err(error) => {
                        refused_groups += 1;
                        refused_circles += group.len();
                        let reason = error.to_string();
                        if first_reason.is_none() {
                            first_reason = Some(reason.clone());
                        }
                        Some(reason)
                    }
                };
                // Leave every group where a diagnosis can pick it up, in the
                // order it was refined. Replaying one group locally takes
                // seconds; re-running the globe to reach the same failure takes
                // most of an hour -- and a refused group cannot be replayed
                // alone anyway, because what it collides with is whatever the
                // groups before it already put on the mesh.
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("refinement_groups.jsonl")
                {
                    use std::io::Write;
                    let circles: Vec<String> = group
                        .iter()
                        .filter_map(|region| match region {
                            RefinementRegion::Circle {
                                center,
                                radius_meters,
                                ..
                            } => Some(format!(
                                "{{\"lon\":{},\"lat\":{},\"r\":{}}}",
                                center.lon_degrees, center.lat_degrees, radius_meters
                            )),
                            _ => None,
                        })
                        .collect();
                    let status = if reason.is_some() { "refused" } else { "ok" };
                    let _ = writeln!(
                        file,
                        "{{\"level\":{level},\"order\":{index},\"status\":\"{status}\",\"faces\":{},\"reason\":{:?},\"circles\":[{}]}}",
                        face_count(&current),
                        reason.unwrap_or_default(),
                        circles.join(",")
                    );
                }
            }
            if refused_groups == groups.len() {
                let reason = first_reason.unwrap_or_default();
                if level > 1 {
                    eprintln!(
                        "adaptive refine level {level}: stopping at level {} -- every region \
                         group was refused: {reason}",
                        level - 1
                    );
                    break;
                }
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "adaptive refinement level {level}: all {} region groups were refused; \
                         first: {reason}",
                        groups.len()
                    ),
                ));
            }
            if refused_groups > 0 {
                eprintln!(
                    "adaptive refine level {level}: {refused_groups} of {} groups refused \
                     ({refused_circles} of {} circles); first reason: {}",
                    groups.len(),
                    regions.len(),
                    first_reason.unwrap_or_default()
                );
            }
        } else {
            current = match refine_once(&current, &regions, level) {
                Ok((refined, sprang)) => {
                    spring_passes += sprang;
                    refined
                }
                // A level the geometry cannot carry ends the run at the depth it
                // reached, rather than throwing away the levels that did work. The
                // mesh so far is valid and is what the criteria asked for down to
                // here; the ceiling is the icosahedral frame refusing to bend
                // further at this place, not a fault in the plan. Level 1 is
                // different -- nothing was refined at all, and that is a failure.
                Err(error) if level > 1 => {
                    eprintln!(
                    "adaptive refine level {level}: stopping at level {} -- {} circles of radius \
                     {:.0} m could not be nested: {error}",
                    level - 1,
                    regions.len(),
                    radius_meters
                );
                    break;
                }
                Err(error) => {
                    return Err(io::Error::new(
                        error.kind(),
                        format!(
                            "adaptive refinement level {level} with {} circles of radius {:.0} m: \
                         {error}",
                            regions.len(),
                            radius_meters
                        ),
                    ))
                }
            };
        }
        // A pass that emitted circles and produced no face at its level did not
        // refine anything, and nothing downstream would notice: the mesh is
        // valid, just coarser than the run asked for. Say so here rather than
        // let it reach a quality report that has no reason to object.
        let deepest_mrlw = deepest_mrlw(&current);
        if deepest_mrlw < level + 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "adaptive refinement level {level} emitted {} circles over {} demanded source \
                     cells but the mesh reached only mrlw {deepest_mrlw}; the circles are too \
                     small for this generation to seed inside",
                    regions.len(),
                    demanded_cells
                ),
            ));
        }
        passes.push(NestPassReport {
            level,
            cell_meters,
            circle_count: regions.len(),
            regions: regions.clone(),
            demanded_cells,
            faces_before,
            faces_after: face_count(&current),
        });
        deepest_level = level;
    }

    Ok((
        current,
        AdaptiveNestReport {
            passes,
            deepest_level,
            stopped_on_empty_demand,
            spring_passes,
        },
    ))
}

fn region_parent_for_level(
    region: &RefinementRegion,
    level: usize,
    base_cell_meters: f64,
) -> io::Result<RefinementRegion> {
    let mut region = region.clone();
    let halo: f64 = (level..region.level())
        .map(|transition_level| {
            MEASURED_PARENT_HALO_ROWS * base_cell_meters / 2f64.powi((transition_level - 1) as i32)
        })
        .sum();
    match &mut region {
        RefinementRegion::Circle {
            radius_meters,
            level: region_level,
            ..
        } => {
            *radius_meters += halo;
            *region_level = level;
        }
        RefinementRegion::Corridor {
            radius_meters,
            level: region_level,
            ..
        } => {
            for radius in radius_meters {
                *radius += halo;
            }
            *region_level = level;
        }
        RefinementRegion::Bbox { .. } | RefinementRegion::Polygon { .. } => {
            let kind = match region {
                RefinementRegion::Bbox { .. } => "bbox",
                RefinementRegion::Polygon { .. } => "polygon",
                _ => unreachable!(),
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "a named {kind} region asks for level {} after an empty level {level}; \
                     provide explicit parent halo regions for bbox/polygon nests",
                    region.level()
                ),
            ));
        }
    }
    Ok(region)
}

fn face_count(mesh: &TriangularMesh) -> usize {
    mesh.w_faces.len().saturating_sub(2)
}

fn deepest_mrlw(mesh: &TriangularMesh) -> usize {
    mesh.w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .unwrap_or(0)
}

/// Adaptive refinement with no regions named outright.
pub fn spawn_nest_adaptive(
    mesh: &MethodCMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<(MethodCMesh, AdaptiveNestReport)> {
    spawn_nest_adaptive_with_named_regions(
        mesh,
        refine,
        inputs,
        &[],
        base_cell_meters,
        max_level,
        None,
    )
}

/// Largest group size one selection walk handles reliably.
///
/// Groups of a few hundred circles refine dependably -- on the global coastal
/// case, all 28 of them did. The one group of 7022 did not, twice over: walked
/// as a single globe-spanning band it either covered two seeds and stopped
/// (the start's local phase decides everything at that span) or covered a
/// continent and then met itself at a vertex whose valence no mesh may carry.
/// Both failures are properties of the span, not the demand, so demand beyond
/// this size is split before walking.
const MAX_GROUP_CIRCLES: usize = 500;

/// Split an oversized group into spatial tiles by median bisection.
///
/// Each tile is walked from its own start and emitted on its own, which is the
/// size class that works; where a tile meets ground an earlier tile refined,
/// the standoff concedes a strip a few cells wide -- bounded, and beside a
/// block that already serves the demand. Bisection is by the wider axis at the
/// sorted midpoint, so the tiling is deterministic.
fn split_oversized_group(group: Vec<RefinementRegion>) -> Vec<Vec<RefinementRegion>> {
    if group.len() <= MAX_GROUP_CIRCLES {
        return vec![group];
    }
    let all_circles = group
        .iter()
        .all(|region| matches!(region, RefinementRegion::Circle { .. }));
    if !all_circles {
        return vec![group];
    }
    let center = |region: &RefinementRegion| -> (f64, f64) {
        match region {
            RefinementRegion::Circle { center, .. } => (center.lon_degrees, center.lat_degrees),
            _ => (0.0, 0.0),
        }
    };
    let lons: Vec<f64> = group.iter().map(|r| center(r).0).collect();
    let lats: Vec<f64> = group.iter().map(|r| center(r).1).collect();
    let span = |values: &[f64]| -> f64 {
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for &value in values {
            lo = lo.min(value);
            hi = hi.max(value);
        }
        hi - lo
    };
    let by_lon = span(&lons) >= span(&lats);
    let mut sorted = group;
    sorted.sort_by(|a, b| {
        let (ka, kb) = if by_lon {
            (center(a).0, center(b).0)
        } else {
            (center(a).1, center(b).1)
        };
        ka.partial_cmp(&kb).unwrap_or(std::cmp::Ordering::Equal)
    });
    let second = sorted.split_off(sorted.len() / 2);
    let mut tiles = split_oversized_group(sorted);
    tiles.extend(split_oversized_group(second));
    tiles
}
