//! Refine one level at a time, asking the criteria again before each pass.
//!
//! Whether a cell needs to be refined further is a question about that cell, so
//! it cannot be settled before the cell exists. The h-field settles everything
//! up front — one field, quantised once — which is why a criterion like
//! land-cover heterogeneity had nowhere to say "the cells I just made are still
//! too mixed". Here each pass re-plans against the generation it is about to
//! refine, and stops as soon as nothing asks for more.
//!
//! The demand for a level is reduced to circles and handed to `spawn_nest` on
//! its own, so the mesh grows one level per call. `spawn_nest` refines from
//! whatever it is given, so chaining is the same operation the engine already
//! performs internally between passes — the only difference is that the regions
//! for pass N+1 are computed after pass N instead of before pass 1.
//!
//! This is the regrid loop of structured AMR (Berger & Oliger 1984) applied to
//! a static field: there the grid is rebuilt because the solution moved, here
//! because the criterion's answer depends on the cell size it is asked at. See
//! the module docs of the parent for the full lineage.
//!
//! What this does **not** do is read the criterion off the refined mesh's own
//! cells. That needs raster-to-cell statistics the port does not have (see the
//! technical guide on `getref_mean_std`), so the scale is carried as a length
//! and the criterion is asked over a matching neighbourhood of the source
//! raster. The size is right; the placement is grid-aligned rather than
//! cell-aligned.

use std::io;

use earthmesh_mesh::{MethodCDelaunayMesh, MethodCRefinementRegion};

use super::ladder::nested_circle_radii_meters;
use super::plan::{plan_demand_at_scale, DemandPlanInputs, LevelDemand};
use super::reduce_demand_to_circles_on_blocks;
use earthmesh_core::RefineConfig;

/// What one pass did, so a run can say why it stopped.
#[derive(Clone, Debug, PartialEq)]
pub struct NestPassReport {
    pub level: usize,
    /// Circles this pass handed to `spawn_nest`. Kept so the quality report can
    /// ask the same question afterwards -- did the mesh reach the level these
    /// circles asked for -- without re-planning the demand.
    pub regions: Vec<MethodCRefinementRegion>,
    /// Cell size this pass was judging — the generation it refines away.
    pub cell_meters: f64,
    pub circle_count: usize,
    pub demanded_cells: usize,
    pub faces_before: usize,
    pub faces_after: usize,
}

/// The whole adaptive run.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveNestReport {
    pub passes: Vec<NestPassReport>,
    /// Level the run stopped at, zero if nothing was ever demanded.
    pub deepest_level: usize,
    /// True when the run stopped because a level demanded nothing, rather than
    /// because it hit `max_level`. A caller that cares about resolution wants
    /// to know which.
    pub stopped_on_empty_demand: bool,
}

/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
/// Refine `mesh` up to `max_level`, re-planning demand before every pass.
///
/// `named_regions` are the regions the run asked for outright — a project's
/// `specified_circle`, a bbox, a closed curve. They are instructions, not
/// criteria, so they are refined whether or not any criterion also asks: a run
/// that names a circle and enables nothing else must still get that circle.
pub fn spawn_nest_adaptive_with_named_regions(
    mesh: &MethodCDelaunayMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    named_regions: &[MethodCRefinementRegion],
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<(MethodCDelaunayMesh, AdaptiveNestReport)> {
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
    let radii = nested_circle_radii_meters(base_cell_meters, max_level)?;
    let mut current = mesh.clone();
    let mut passes = Vec::new();
    let mut deepest_level = 0usize;
    let mut stopped_on_empty_demand = false;

    for level in 1..=max_level {
        // The cell this pass refines away is the one the previous level left.
        let cell_meters = base_cell_meters / 2f64.powi((level - 1) as i32);
        let plan: LevelDemand = plan_demand_at_scale(refine, inputs, level, cell_meters)?;
        // Every level blocks on the finest radius so the centres coincide and
        // the levels come out concentric; only the radius changes per level.
        let mut regions = if plan.is_empty() {
            Vec::new()
        } else {
            reduce_demand_to_circles_on_blocks(
                &plan.demand,
                level,
                radii[level - 1],
                radii[max_level - 1],
            )?
        };
        regions.extend(
            named_regions
                .iter()
                .filter(|region| region.level() == level)
                .cloned(),
        );
        if regions.is_empty() {
            stopped_on_empty_demand = true;
            break;
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
        let mut groups = earthmesh_mesh::method_c_connected_region_groups(&regions, false);
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
            for (index, group) in groups.iter().enumerate() {
                if index > 0 && index.is_multiple_of(report_every) {
                    eprintln!(
                        "adaptive refine level {level}: group {index}/{} ({} faces so far)",
                        groups.len(),
                        face_count(&current)
                    );
                }
                match current.spawn_nest(group, level) {
                    Ok(next) => current = next,
                    Err(error) => {
                        refused_groups += 1;
                        refused_circles += group.len();
                        if first_reason.is_none() {
                            first_reason = Some(error.to_string());
                        }
                        // Leave the refused circles where a diagnosis can pick
                        // them up: replaying one group locally takes seconds,
                        // re-running the globe to reach the same failure takes
                        // forty minutes.
                        if let Ok(mut file) = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open("refused_groups.jsonl")
                        {
                            use std::io::Write;
                            let circles: Vec<String> = group
                                .iter()
                                .filter_map(|region| match region {
                                    MethodCRefinementRegion::Circle {
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
                            let _ = writeln!(
                                file,
                                "{{\"level\":{level},\"reason\":{:?},\"circles\":[{}]}}",
                                error.to_string(),
                                circles.join(",")
                            );
                        }
                    }
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
            current = match current.spawn_nest(&regions, level) {
                Ok(refined) => refined,
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
                    radii[level - 1]
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
                            radii[level - 1]
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
                    plan.demand.demanded_count()
                ),
            ));
        }
        passes.push(NestPassReport {
            level,
            cell_meters,
            circle_count: regions.len(),
            regions: regions.clone(),
            demanded_cells: plan.demand.demanded_count(),
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
        },
    ))
}

fn face_count(mesh: &MethodCDelaunayMesh) -> usize {
    mesh.w_faces.len().saturating_sub(2)
}

fn deepest_mrlw(mesh: &MethodCDelaunayMesh) -> usize {
    mesh.w_faces
        .iter()
        .skip(2)
        .map(|face| face.mrlw)
        .max()
        .unwrap_or(0)
}

/// Adaptive refinement with no regions named outright.
pub fn spawn_nest_adaptive(
    mesh: &MethodCDelaunayMesh,
    refine: &RefineConfig,
    inputs: &DemandPlanInputs<'_>,
    base_cell_meters: f64,
    max_level: usize,
) -> io::Result<(MethodCDelaunayMesh, AdaptiveNestReport)> {
    spawn_nest_adaptive_with_named_regions(mesh, refine, inputs, &[], base_cell_meters, max_level)
}

impl AdaptiveNestReport {
    /// Deepest level whose circles cover this point, zero where none do.
    ///
    /// This is the target-level function the quality report reconciles against
    /// the mesh's actual levels. It reads the circles the run actually emitted
    /// rather than re-deriving them, so a discrepancy is a refinement failure
    /// and never a planning difference.
    pub fn target_level_at(&self, lon_degrees: f64, lat_degrees: f64) -> u32 {
        let mut deepest = 0u32;
        for pass in &self.passes {
            for region in &pass.regions {
                let MethodCRefinementRegion::Circle {
                    center,
                    radius_meters,
                    level,
                } = region
                else {
                    continue;
                };
                let distance = earthmesh_hfield::great_circle_distance_m(
                    center.lon_degrees,
                    center.lat_degrees,
                    lon_degrees,
                    lat_degrees,
                );
                if distance <= *radius_meters {
                    deepest = deepest.max(*level as u32);
                }
            }
        }
        deepest
    }

    pub fn circle_count(&self) -> usize {
        self.passes.iter().map(|pass| pass.circle_count).sum()
    }
}

/// Name of the file a run leaves beside its gridfile describing what the
/// point+radius route asked for.
///
/// The quality step runs separately, from a namelist and a gridfile path, so it
/// cannot see the run's [`AdaptiveNestReport`]. Both the final gridfile and the
/// saved namelist land in `<case>/result/`, so a sibling file there is reachable
/// from either — measured, not assumed: gridinit writes into `<case>/gridfile/`,
/// which is a different directory and would not be found.
pub const ADAPTIVE_REFINEMENT_FILE: &str = "adaptive_refinement.json";

impl AdaptiveNestReport {
    /// Serialize the circles this run emitted, for the quality step to read.
    pub fn to_json(&self, max_level: usize, base_meters: f64, coastline: bool) -> String {
        let passes = self
            .passes
            .iter()
            .map(|pass| {
                let circles = pass
                    .regions
                    .iter()
                    .filter_map(|region| match region {
                        MethodCRefinementRegion::Circle {
                            center,
                            radius_meters,
                            ..
                        } => Some(format!(
                            "{{\"lon\":{},\"lat\":{},\"radius_m\":{}}}",
                            center.lon_degrees, center.lat_degrees, radius_meters
                        )),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{{\"level\":{},\"cell_meters\":{},\"demanded_cells\":{},\"circles\":[{circles}]}}",
                    pass.level, pass.cell_meters, pass.demanded_cells
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"enabled\":true,\"max_level\":{max_level},\"base_m\":{base_meters},\
             \"coastline\":{coastline},\"deepest_level\":{},\
             \"stopped_on_empty_demand\":{},\"passes\":[{passes}]}}",
            self.deepest_level, self.stopped_on_empty_demand
        )
    }
}
