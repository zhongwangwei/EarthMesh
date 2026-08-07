use std::fmt;

/// Every way HARP-DV can decline to do what was asked.
///
/// Each variant carries enough context to say where: the cycle, the patch, the
/// site or cell, the criterion, the coordinates. A message that says only what
/// went wrong and not where is a message nobody can act on.
#[derive(Debug)]
pub enum HarpDvError {
    /// The request itself does not describe a run that could be made.
    InvalidConfig(String),
    /// The mesh handed in is not one this backend can adapt.
    InvalidMesh(String),
    /// A geometric predicate could not decide, even at higher precision.
    ///
    /// Kept as an error rather than resolved by picking a branch. A topology
    /// decision made by a coin toss is one nobody can reproduce.
    AmbiguousGeometry(String),
    /// A transaction would have left the mesh in a state the invariants reject.
    TopologyViolation(String),
    /// A boundary constraint would have been lost or crossed.
    BoundaryViolation(String),
    /// No candidate point survived the constraints.
    CandidateRejected(String),
    /// A transaction was rolled back rather than committed.
    TransactionRejected(String),
    /// A limit in the config was reached.
    BudgetExceeded(String),
    /// The criterion data source could not answer.
    DataProvider(String),
    Io(std::io::Error),
}

impl fmt::Display for HarpDvError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfig(message) => write!(formatter, "invalid HARP-DV config: {message}"),
            Self::InvalidMesh(message) => write!(formatter, "invalid mesh for HARP-DV: {message}"),
            Self::AmbiguousGeometry(message) => {
                write!(formatter, "ambiguous geometry: {message}")
            }
            Self::TopologyViolation(message) => write!(formatter, "topology violation: {message}"),
            Self::BoundaryViolation(message) => write!(formatter, "boundary violation: {message}"),
            Self::CandidateRejected(message) => write!(formatter, "candidate rejected: {message}"),
            Self::TransactionRejected(message) => {
                write!(formatter, "transaction rejected: {message}")
            }
            Self::BudgetExceeded(message) => write!(formatter, "budget exceeded: {message}"),
            Self::DataProvider(message) => write!(formatter, "criterion data: {message}"),
            Self::Io(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for HarpDvError {}

impl From<std::io::Error> for HarpDvError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

pub type Result<T> = std::result::Result<T, HarpDvError>;
