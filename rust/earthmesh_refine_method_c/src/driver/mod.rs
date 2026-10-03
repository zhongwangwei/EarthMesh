//! Method-C above the kernels: the routes a run takes through them.
//!
//! The h-field route composes its field again, graded gently enough, when the
//! steep one cannot be nested whole; the point+radius route refines level by
//! level and group by group; LEPP-Delaunay refines toward its demands and is
//! repaired back into Method-C's degrees and the angle window. Each reads its
//! demand already planned -- a field composer, a level planner, a demand list
//! -- so nothing here reads a raster or writes a file; the caller does both.

mod adaptive;
mod hfield;
mod lepp;

pub use adaptive::{
    spawn_nest_adaptive_levels, AdaptiveNestSpring, GroupAttempt, METHOD_C_ADAPTIVE_SUSPENDED,
};
pub use hfield::{crosses_a_parent, spawn_from_graded_hfield, GradedHfieldSpawn};
pub use lepp::{
    method_c_lepp_adaptive_insertion_gates, method_c_lepp_insertion_gates, refine_lepp,
    LeppRequest, LeppRun,
};
