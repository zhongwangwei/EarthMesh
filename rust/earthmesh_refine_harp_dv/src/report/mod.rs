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
            deterministic: true,
        }
    }
}
