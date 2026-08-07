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
    MaximumCyclesReached,
    BudgetReached,
    MinimumScaleReached,
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
    /// Adjacent cell pairs still past `max_neighbour_scale_ratio` when the run
    /// stopped.
    ///
    /// Rarely zero. The degree bound and the scale bound pull against each
    /// other, and closing the last ratios needs cells the degree gate refuses.
    /// Section 8.1's r-adaptation is the move that would resolve it and is not
    /// implemented; until it is, this number is what a caller decides on.
    pub unbalanced_pairs_remaining: usize,
    /// Demands the run could not meet. Counted rather than dropped: a run that
    /// silently serves less than was asked is the failure mode this whole
    /// backend is arranged against.
    pub unresolved_count: usize,
    /// Every refusal the run made, by kind. One demand can contribute several
    /// -- the ladder tries every rung before giving up.
    pub refusals: RejectionTally,
    /// Moves that lowered a neighbourhood's maximum degree so a demand the
    /// degree bound had turned away could be tried again.
    pub degree_relieving_moves: usize,
    pub deterministic: bool,
}

impl HarpDvRunReport {
    pub const SCHEMA_VERSION: u32 = 1;

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
            unbalanced_pairs_remaining: 0,
            unresolved_count: 0,
            refusals: RejectionTally::default(),
            degree_relieving_moves: 0,
            deterministic: true,
        }
    }
}
