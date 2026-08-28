//! What the run says about itself afterwards.

/// Why the driver stopped.
///
/// Every exit names one. "Finished" without a reason cannot be told apart from
/// "gave up quietly", and the second is the failure this backend exists to make
/// impossible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    /// Nothing was left asking.
    AllSatisfied,
    /// Demands remained, but no transaction over them was acceptable.
    NoAcceptedTransactions,
    /// Site moves committed, but the next evaluation reduced neither physical
    /// demands nor scale-balance demands.
    NoProductiveAdaptation,
    /// Every remaining demand was rejected only by the requested triangle
    /// angle floor and no local adaptation was accepted. More identical
    /// cycles cannot change that, so this is distinct from a generic refusal.
    QualityConstraintReached,
    MaximumCyclesReached,
    BudgetReached,
    /// Cells still wanted refining but had reached `minimum_cell_width_m`.
    MinimumScaleReached,
    /// What remained was finer than the data behind the criterion can justify.
    SourceResolutionReached,
}

impl StopReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AllSatisfied => "all_satisfied",
            Self::NoAcceptedTransactions => "no_accepted_transactions",
            Self::NoProductiveAdaptation => "no_productive_adaptation",
            Self::QualityConstraintReached => "quality_constraint_reached",
            Self::MaximumCyclesReached => "maximum_cycles_reached",
            Self::BudgetReached => "budget_reached",
            Self::MinimumScaleReached => "minimum_scale_reached",
            Self::SourceResolutionReached => "source_resolution_reached",
        }
    }
}

/// Whether the final HARP-DV mesh satisfies the requested angle window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AngleWindowVerdict {
    NotEvaluated,
    Pass,
    Fail,
}

impl AngleWindowVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotEvaluated => "not_evaluated",
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

/// Why candidates were turned away, by kind.
///
/// A total tells a reader that the run fell short; it does not tell them what
/// to change. These three want different answers -- a degree wall wants
/// r-adaptation, a pentagon wall wants the demand moved off it, and a ladder
/// that ran out wants another rung -- and a run reporting only "33 unresolved"
/// leaves the choice to guesswork.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RejectionTally {
    /// A site would have gone past the degree the gridfile carries.
    pub degree: usize,
    /// One of the twelve pentagons would have stopped being one.
    pub pentagon: usize,
    /// The point could not be inserted at all: duplicate, off-sphere, or a
    /// cavity that was not a disk.
    pub not_insertable: usize,
    /// The change left the surface open or the adjacency wrong.
    pub topology: usize,
    /// The change would have left a triangle too thin for the writer.
    pub sliver: usize,
    /// Legal, and no better than what it replaced.
    pub no_improvement: usize,
    /// The neighbourhood could not be read to check it.
    pub unmeasurable: usize,
}

impl RejectionTally {
    pub fn total(&self) -> usize {
        self.degree
            + self.pentagon
            + self.not_insertable
            + self.topology
            + self.sliver
            + self.no_improvement
            + self.unmeasurable
    }
}

/// The run, in the numbers a reader needs to trust it.
#[derive(Clone, Debug, PartialEq)]
pub struct HarpDvRunReport {
    pub schema_version: u32,
    pub cycles_completed: u32,
    pub stop_reason: StopReason,
    pub initial_sites: usize,
    pub final_sites: usize,
    /// Candidate insertion transactions. Geometry-only r-adaptation is
    /// reported separately in `r_adaptation_moves`.
    pub transactions_attempted: usize,
    pub transactions_committed: usize,
    pub transactions_rolled_back: usize,
    /// How many of the commits were the mesh balancing itself rather than a
    /// criterion asking.
    ///
    /// Separate because the two mean different things to a reader: physical
    /// refinement is the run doing what was asked, and balance refinement is
    /// what that cost in cells nobody requested.
    pub balance_transactions_committed: usize,
    /// Insertions found by the broader candidate ladder after a whole cycle
    /// made no insertion progress.
    pub fallback_transactions_committed: usize,
    /// Adjacent cell pairs still past `max_neighbour_scale_ratio` when the run
    /// stopped.
    ///
    /// Normally zero after unconstrained r-adaptation. Protected segments or a
    /// hard gate can leave a residue, so the value remains explicit.
    pub unbalanced_pairs_remaining: usize,
    /// Demands the run could not meet. Counted rather than dropped: a run that
    /// silently serves less than was asked is the failure mode this whole
    /// backend is arranged against.
    pub unresolved_count: usize,
    /// Criterion-driven cells still asking after the final re-evaluation.
    pub physical_demands_remaining: usize,
    /// Mesh-balance cells still asking after the final re-evaluation.
    pub balance_demands_remaining: usize,
    /// Final-cycle demands for which every candidate failed only the triangle
    /// angle gate.
    pub quality_constrained_count: usize,
    /// Every refusal the run made, by kind. One demand can contribute several
    /// -- the ladder tries every rung before giving up.
    pub refusals: RejectionTally,
    /// Moves that lowered the actual vertex named by a degree rejection so the
    /// refused demand could be tried again.
    pub degree_relieving_moves: usize,
    /// All committed r-adaptation moves, including degree, pentagon and scale
    /// relief. These change geometry without adding a cell.
    pub r_adaptation_moves: usize,
    /// Site moves committed as two-site transactions to cross a local
    /// Delaunay saddle. Each transaction contributes two moves.
    pub paired_r_adaptation_moves: usize,
    /// Moves committed by the bounded multi-ring recovery, which sweeps a few
    /// rings around a stalled region instead of the one site a rejection named.
    ///
    /// Counted apart from the rest because it answers a different question: the
    /// single-site phases say how much the mesh could be loosened one vertex at
    /// a time, and this says how much of the residue needed a wider
    /// neighbourhood before anything would move at all.
    pub multi_ring_r_adaptation_moves: usize,
    /// Triangle corner angles below the preferred 40 degree quality window.
    pub angles_below_40_deg: usize,
    /// Triangle corner angles inside the compatible legacy 40..=90 degree window.
    pub angles_in_40_90_deg: usize,
    /// Triangle corner angles above the compatible legacy 90 degree quality window.
    pub angles_above_90_deg: usize,
    /// Triangle corner angles inside the delivery 40..=80 degree window.
    pub angles_in_40_80_deg: usize,
    /// Triangle corner angles above the delivery 80 degree quality window.
    pub angles_above_80_deg: usize,
    /// Smallest realised triangle corner angle in degrees.
    pub angle_min_deg: f64,
    /// Largest realised triangle corner angle in degrees.
    pub angle_max_deg: f64,
    /// Final delivery verdict for the 40..=80 degree angle window.
    pub angle_window_40_80_verdict: AngleWindowVerdict,
    /// Triangles whose corner angles could not be measured.
    pub angle_window_unmeasurable_triangles: usize,
    /// Active vertices with fewer than five neighbours; these make an all-acute
    /// window impossible until topology changes.
    pub vertices_below_degree_5: usize,
    /// Active sites created by HARP rather than inherited with the base mesh.
    pub active_adaptive_sites: usize,
    /// Active adaptive sites with a known parent and no active child.
    ///
    /// A created site whose parent is unknown is reported separately and is
    /// deliberately not called a leaf: it cannot be retired safely from an
    /// incomplete lineage.
    pub active_leaf_sites: usize,
    /// Active leaves that are free interior sites and touch no protected segment.
    pub interior_leaf_sites: usize,
    /// Active adaptive sites whose insertion did not record a causal parent.
    pub lineage_unknown_adaptive_sites: usize,
    /// Active leaves by current Delaunay degree.
    pub leaf_degree_4: usize,
    pub leaf_degree_5: usize,
    pub leaf_degree_6: usize,
    pub leaf_degree_7: usize,
    pub leaf_degree_other: usize,
    /// Creation-cycle range of active leaves; both are zero when no leaf exists.
    pub leaf_birth_cycle_min: u32,
    pub leaf_birth_cycle_max: u32,
    /// Requested target-scale range measurable at active leaves.
    pub leaf_target_scale_measured: usize,
    pub leaf_target_scale_min_m: f64,
    pub leaf_target_scale_max_m: f64,
    /// Strict association: the vertex carrying the bad corner angle is a leaf.
    pub angles_below_40_at_leaf_vertices: usize,
    pub angles_above_80_at_leaf_vertices: usize,
    pub angles_below_40_at_interior_leaf_vertices: usize,
    pub angles_above_80_at_interior_leaf_vertices: usize,
    /// Broad association, kept separate so it cannot inflate the strict signal.
    pub violating_triangles_touching_leaf: usize,
    pub violating_triangles_touching_interior_leaf: usize,
    /// Read-only feasibility audit for retiring degree-four interior leaves.
    /// Enabled with `EARTHMESH_HARP_D4_RETIREMENT_AUDIT` on meshes without
    /// protected segments; these counts describe compact clones and no site is
    /// removed from the run.
    pub d4_leaf_retirement_candidates: usize,
    pub d4_leaf_retirement_triangulations: usize,
    pub d4_leaf_retirement_hard_gate_safe: usize,
    pub d4_leaf_retirement_physical_safe: usize,
    pub d4_leaf_retirement_balance_safe: usize,
    pub d4_leaf_retirement_quality_improving: usize,
    /// Leaves with at least one clone triangulation passing every audit gate.
    pub d4_leaf_retirement_fully_acceptable: usize,
    /// Degree-four interior leaves actually retired by the production pass.
    pub d4_leaf_retirement_committed: usize,
    /// All degree 4..=7 interior quality leaves retired by the production pass.
    /// Degrees 5..=7 remain opt-in through `EARTHMESH_HARP_LEAF_RETIREMENT`.
    pub quality_leaf_retirement_committed: usize,
    /// Target-field triangle corner angles below 40 degrees, measured from the
    /// frozen desired edge lengths rather than the realised mesh.
    pub target_triangle_angles_below_40_deg: usize,
    /// Target-field triangle corner angles above 80 degrees.
    pub target_triangle_angles_above_80_deg: usize,
    /// Number of target-field triangle corner angles measured. Zero means no
    /// usable target-scale field was available.
    pub target_triangle_angle_count: usize,
    /// Smallest target-field triangle corner angle in degrees.
    pub target_triangle_angle_min_deg: f64,
    /// Largest target-field triangle corner angle in degrees.
    pub target_triangle_angle_max_deg: f64,
    /// Squared distance outside the compatible legacy 40..=90 degree window.
    pub angle_window_penalty: f64,
    /// Squared distance outside the delivery 40..=80 degree window.
    pub angle_window_40_80_penalty: f64,
    /// Site moves committed by HARP's size-field-aware quality optimiser.
    pub quality_optimiser_moves: usize,
    /// Worst local area-length ratio after HARP optimisation (one is equilateral).
    pub triangle_eta_min: f64,
    /// First percentile local area-length ratio after HARP optimisation.
    pub triangle_eta_p1: f64,
    /// Triangles below the quality optimiser's 0.89 diagnostic threshold.
    pub triangles_below_eta_0_89: usize,
    pub deterministic: bool,
}

impl HarpDvRunReport {
    pub const SCHEMA_VERSION: u32 = 14;

    /// The report of a run that had nothing to do.
    pub fn empty(sites: usize, stop_reason: StopReason) -> Self {
        Self {
            schema_version: Self::SCHEMA_VERSION,
            cycles_completed: 0,
            stop_reason,
            initial_sites: sites,
            final_sites: sites,
            transactions_attempted: 0,
            transactions_committed: 0,
            transactions_rolled_back: 0,
            balance_transactions_committed: 0,
            fallback_transactions_committed: 0,
            unbalanced_pairs_remaining: 0,
            unresolved_count: 0,
            physical_demands_remaining: 0,
            balance_demands_remaining: 0,
            quality_constrained_count: 0,
            refusals: RejectionTally::default(),
            degree_relieving_moves: 0,
            r_adaptation_moves: 0,
            paired_r_adaptation_moves: 0,
            multi_ring_r_adaptation_moves: 0,
            angles_below_40_deg: 0,
            angles_in_40_90_deg: 0,
            angles_above_90_deg: 0,
            angles_in_40_80_deg: 0,
            angles_above_80_deg: 0,
            angle_min_deg: 0.0,
            angle_max_deg: 0.0,
            angle_window_40_80_verdict: AngleWindowVerdict::NotEvaluated,
            angle_window_unmeasurable_triangles: 0,
            vertices_below_degree_5: 0,
            active_adaptive_sites: 0,
            active_leaf_sites: 0,
            interior_leaf_sites: 0,
            lineage_unknown_adaptive_sites: 0,
            leaf_degree_4: 0,
            leaf_degree_5: 0,
            leaf_degree_6: 0,
            leaf_degree_7: 0,
            leaf_degree_other: 0,
            leaf_birth_cycle_min: 0,
            leaf_birth_cycle_max: 0,
            leaf_target_scale_measured: 0,
            leaf_target_scale_min_m: 0.0,
            leaf_target_scale_max_m: 0.0,
            angles_below_40_at_leaf_vertices: 0,
            angles_above_80_at_leaf_vertices: 0,
            angles_below_40_at_interior_leaf_vertices: 0,
            angles_above_80_at_interior_leaf_vertices: 0,
            violating_triangles_touching_leaf: 0,
            violating_triangles_touching_interior_leaf: 0,
            d4_leaf_retirement_candidates: 0,
            d4_leaf_retirement_triangulations: 0,
            d4_leaf_retirement_hard_gate_safe: 0,
            d4_leaf_retirement_physical_safe: 0,
            d4_leaf_retirement_balance_safe: 0,
            d4_leaf_retirement_quality_improving: 0,
            d4_leaf_retirement_fully_acceptable: 0,
            d4_leaf_retirement_committed: 0,
            quality_leaf_retirement_committed: 0,
            target_triangle_angles_below_40_deg: 0,
            target_triangle_angles_above_80_deg: 0,
            target_triangle_angle_count: 0,
            target_triangle_angle_min_deg: 0.0,
            target_triangle_angle_max_deg: 0.0,
            angle_window_penalty: 0.0,
            angle_window_40_80_penalty: 0.0,
            quality_optimiser_moves: 0,
            triangle_eta_min: 0.0,
            triangle_eta_p1: 0.0,
            triangles_below_eta_0_89: 0,
            deterministic: true,
        }
    }
}
