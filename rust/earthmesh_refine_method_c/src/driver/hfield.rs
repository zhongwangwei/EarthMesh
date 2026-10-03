//! The h-field route on the sphere: spawn from the composed field, and compose
//! it once more, graded gently enough for the transition, when the steep one
//! cannot be nested whole.

use std::io;

use earthmesh_refine::hfield::LevelledHfield;

use crate::{method_c_repairable_payload, MethodCHfieldSpawnDiagnostics, MethodCMesh};

/// What the h-field route built, and the field it built from.
pub struct GradedHfieldSpawn {
    pub field: LevelledHfield,
    pub mesh: MethodCMesh,
    pub spring_passes: usize,
    pub diagnostics: MethodCHfieldSpawnDiagnostics,
    /// The gradation the kept field was composed at.
    pub g: f64,
}

/// Whether a spawn failed because a level's grid crossed its parent's
/// boundary: the failure a gentler grading avoids.
pub fn crosses_a_parent(error: &io::Error) -> bool {
    let message = error.to_string();
    message.contains("crosses the parent boundary")
        || message.contains("next coarser grid boundary")
}

/// Spawn from the field `compose` makes at gradation `g`.
///
/// Each level's band around the next finer one is `1 / g` of its cells, and
/// Method-C's transition takes up to `max_mrows` rows: at g = 0.2 a global
/// 30 km slope run's level-2 grid crossed its parent; at 0.1 it built. A run
/// that builds keeps its field; one that crosses a parent is composed once
/// more, graded gently enough.
///
/// Dropping blocks is a failure too, a partial one: a pass that gives up a
/// block has stopped honouring the field there. Once the drop loop could name
/// the block behind every gate, the global 30 km run stopped failing at
/// g = 0.2 and built with blocks missing -- 302,033 cells against 388,425 at
/// g = 0.1 -- because nothing failed for the retry to see. So a pass that
/// dropped blocks is composed again as well, and the result that lost fewer
/// parent faces is kept.
pub fn spawn_from_graded_hfield(
    mesh: &MethodCMesh,
    g: f64,
    compose: &dyn Fn(f64) -> io::Result<LevelledHfield>,
    max_mrows: usize,
    nxp: usize,
    spring_iterations: usize,
) -> io::Result<GradedHfieldSpawn> {
    let spawn = |g: f64| -> io::Result<GradedHfieldSpawn> {
        let field = compose(g)?;
        let (mesh, spring_passes, diagnostics) = mesh.spawn_nest_from_target_levels_with_spring(
            |lon, lat| {
                field
                    .field
                    .level_at(lon, lat, field.level_base_m, field.max_level as u8)
            },
            field.max_level,
            max_mrows,
            nxp,
            spring_iterations,
        )?;
        Ok(GradedHfieldSpawn {
            field,
            mesh,
            spring_passes,
            diagnostics,
            g,
        })
    };
    let gentle_g = 1.0 / (max_mrows as f64 + 3.0);
    match spawn(g) {
        Err(error)
            if g > gentle_g
                && (crosses_a_parent(&error) || method_c_repairable_payload(&error).is_some()) =>
        {
            eprintln!(
                "earthmesh_cli: warning: Method-C could not nest the h-field graded at \
                 g = {} ({error}); composing it again at g = {gentle_g:.4}, gentle \
                 enough for {max_mrows} transition rows",
                g
            );
            spawn(gentle_g)
        }
        Ok(steep) if g > gentle_g && steep.diagnostics.dropped_block_count > 0 => {
            let dropped = steep.diagnostics;
            eprintln!(
                "earthmesh_cli: warning: Method-C nested the h-field graded at g = {} only \
                 by leaving {} block(s) ({} parent faces) coarse; composing it again at \
                 g = {gentle_g:.4}, gentle enough for {max_mrows} transition rows",
                g, dropped.dropped_block_count, dropped.dropped_face_count
            );
            match spawn(gentle_g) {
                Ok(retry) if retry.diagnostics.dropped_face_count < dropped.dropped_face_count => {
                    Ok(retry)
                }
                retry => {
                    let why = match &retry {
                        Ok(retry) => format!(
                            "it left {} parent faces coarse",
                            retry.diagnostics.dropped_face_count
                        ),
                        Err(error) => format!("it failed: {error}"),
                    };
                    eprintln!(
                        "earthmesh_cli: warning: keeping the mesh nested at g = {}; at \
                         g = {gentle_g:.4} {why}",
                        g
                    );
                    Ok(steep)
                }
            }
        }
        result => result,
    }
}
