//! Which backend, and what it is being asked for.

/// The refinement backends, as a choice rather than a chain.
///
/// They differ in what they do with a request they cannot take as given, and
/// that difference is the whole reason there are three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum RefinementBackend {
    /// Nested regions with transition rows. Refuses a region off its lattice,
    /// which is why criteria-driven refinement is suspended on it.
    #[default]
    MethodC,
    /// Splits any marked triangle into four and closes the seams. Grows a
    /// marking it cannot take as given rather than rejecting a shape.
    RedGreen,
    /// Re-reads the criteria against the cells that exist now and changes the
    /// mesh locally where they are still unmet.
    HarpDv,
    /// Starts from a certified icosahedral mother grid and only coarsens a
    /// patch when the primal, dual, physical, and balance certificates pass.
    Certified,
}

impl RefinementBackend {
    /// The name this backend goes by in a namelist.
    pub fn engine_str(self) -> &'static str {
        match self {
            Self::MethodC => "method_c",
            Self::RedGreen => "red_green",
            Self::HarpDv => "harp_dv",
            Self::Certified => "certified",
        }
    }

    /// Read a backend from what a namelist wrote.
    pub fn from_engine_str(name: &str) -> Option<Self> {
        match name.trim() {
            "method_c" => Some(Self::MethodC),
            "red_green" => Some(Self::RedGreen),
            "harp_dv" => Some(Self::HarpDv),
            "certified" => Some(Self::Certified),
            _ => None,
        }
    }

    /// Whether this backend reads a criterion itself.
    ///
    /// Method-C does not: a region whose shape came from data is refused rather
    /// than approximated, so criteria reach it only as named regions someone
    /// else derived. Measured, and recorded in the technical guide.
    pub fn serves_criteria_directly(self) -> bool {
        matches!(self, Self::RedGreen | Self::HarpDv | Self::Certified)
    }
}
