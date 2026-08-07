//! What HARP-DV adapts, and what it remembers about it.

mod id_allocator;

pub use id_allocator::SiteIdAllocator;

use earthmesh_mesh::{xyz_to_lonlat_degrees, LonLatDegrees, MeshState, MESH_STATE_FIRST_ID};

use crate::error::{HarpDvError, Result};

/// How far apart two positions are along the sphere, in metres.
fn displacement_metres(from: LonLatDegrees, to: LonLatDegrees) -> f64 {
    let a = earthmesh_mesh::lonlat_degrees_to_unit_xyz(from);
    let b = earthmesh_mesh::lonlat_degrees_to_unit_xyz(to);
    let dot = (a.x * b.x + a.y * b.y + a.z * b.z).clamp(-1.0, 1.0);
    dot.acos() * earthmesh_core::EARTH_RADIUS_METERS
}

/// A Voronoi site's identity, stable for the life of the run.
///
/// Bound to the site rather than to a Delaunay face, because a face is not a
/// lasting thing: an edge flip or an insertion rebuilds it. The site outlives
/// both, which is what makes it the thing a lineage record can point at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SiteId(pub u64);

/// How far a site is allowed to move, and along what.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteMobility {
    /// A corner, a junction, or a point the run was told to protect.
    Fixed,
    /// Free on the sphere.
    Interior,
    /// Only along the boundary curve it belongs to.
    BoundaryCurve { curve: u64 },
    /// Only along a material interface.
    InterfaceCurve { curve: u64 },
}

/// One Voronoi site, and everything the run knows about where it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct AdaptiveSite {
    pub site_id: SiteId,
    pub position: LonLatDegrees,
    /// The cycle that created it. Zero means it came in with the mesh.
    pub birth_cycle: u32,
    /// How many times it has been through a refinement that created it.
    pub depth: u16,
    /// False once a transaction removes it. The row stays so its id keeps
    /// meaning what it meant.
    pub active: bool,
    /// Where it was when the run first saw it, which is what a displacement
    /// budget is measured against.
    pub reference_position: LonLatDegrees,
    pub cumulative_displacement_m: f64,
    pub mobility: SiteMobility,
}

impl AdaptiveSite {
    /// A site as it arrives with the mesh, before any adaptation.
    pub fn inherited(site_id: SiteId, position: LonLatDegrees) -> Self {
        Self {
            site_id,
            position,
            birth_cycle: 0,
            depth: 0,
            active: true,
            reference_position: position,
            cumulative_displacement_m: 0.0,
            mobility: SiteMobility::Interior,
        }
    }
}

/// The mesh HARP-DV adapts, with a stable identity for every site.
///
/// Wraps `MeshState`, the backend-neutral triangulation, rather than
/// `TriangularMesh`. That matters more than it looks: `TriangularMesh` is
/// Method-C's own type, and a backend built on it inherits generations,
/// transition rows and grid numbers it has no use for and cannot maintain.
/// What HARP-DV needs is sites, triangles and adjacency, which is what this
/// carries.
#[derive(Clone, Debug)]
pub struct AdaptiveMesh {
    state: MeshState,
    /// The twelve pentagon ids of the mesh this one descends from.
    ///
    /// Carried rather than derived: a refined mesh has more than twelve
    /// degree-5 sites, and site ids never move, so these keep naming the same
    /// twelve however much refining happens. Without them a run cannot be
    /// written out at all.
    impent: [usize; 12],
    sites: Vec<AdaptiveSite>,
    allocator: SiteIdAllocator,
    cycles_completed: u32,
    /// The site whose demand is being served, so a new one can be recorded a
    /// generation deeper than it.
    pub(crate) refining: Option<usize>,
    /// The boundary as a list of segments, each an edge of the mesh.
    ///
    /// Ruppert's PSLG. An earlier version approximated this with a set of
    /// boundary *sites* and called any edge between two of them a segment,
    /// which is a different predicate -- two boundary sites adjacent without
    /// lying on the curve read as a segment, and every split made more such
    /// pairs. Guide 11.28 measured what that cost.
    ///
    /// Keys are ordered pairs, smaller id first, so an edge is the same
    /// segment from either side.
    pub(crate) segments: std::collections::BTreeSet<(usize, usize)>,
}

impl AdaptiveMesh {
    /// Take a neutral triangulation and give every site a stable id.
    pub fn from_mesh_state(state: MeshState) -> Result<Self> {
        if state.vertex_count() == 0 {
            return Err(HarpDvError::InvalidMesh(
                "a triangulation with no sites carries nothing to adapt".to_string(),
            ));
        }
        state.validate().map_err(|errors| {
            HarpDvError::InvalidMesh(format!(
                "the triangulation does not hold together: {}",
                errors
                    .iter()
                    .take(4)
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ))
        })?;
        let mut allocator = SiteIdAllocator::default();
        let mut sites = Vec::with_capacity(state.vertex_count());
        for vertex in MESH_STATE_FIRST_ID..state.vertices().len() {
            let position = xyz_to_lonlat_degrees(state.vertices()[vertex]);
            sites.push(AdaptiveSite::inherited(allocator.allocate(), position));
        }
        Ok(Self {
            state,
            impent: [MESH_STATE_FIRST_ID; 12],
            sites,
            allocator,
            cycles_completed: 0,
            refining: None,
            segments: std::collections::BTreeSet::new(),
        })
    }

    /// Take the neutral part of a Method-C mesh and adapt that.
    ///
    /// A convenience over `from_mesh_state`, because a Method-C mesh is what
    /// the rest of the engine produces today. What arrives here is the
    /// triangulation; the generations stay behind.
    pub fn from_triangular_mesh(mesh: &earthmesh_mesh::TriangularMesh) -> Result<Self> {
        let state = MeshState::from_triangular_mesh(mesh)
            .map_err(|error| HarpDvError::InvalidMesh(error.to_string()))?;
        let mut adaptive = Self::from_mesh_state(state)?;
        adaptive.impent = mesh.impent;
        Ok(adaptive)
    }

    /// The twelve pentagons, for whatever writes this mesh out.
    pub fn pentagon_ids(&self) -> [usize; 12] {
        self.impent
    }

    /// The three-table mesh the gridfile writers consume.
    ///
    /// The generation carried per face is the depth of the site that made it,
    /// so a reader can tell an original face from one adaptation produced.
    pub fn to_triangular_mesh(&self) -> Result<earthmesh_mesh::TriangularMesh> {
        // The site table and the triangulation must agree, because the levels
        // below are looked up by subtracting `MESH_STATE_FIRST_ID` from a
        // vertex id. If they ever drift the lookup misses and `unwrap_or(1)`
        // reports generation 1 for every face -- a wrong answer that reads
        // exactly like an unrefined mesh.
        if self.sites.len() != self.state.vertex_count() {
            return Err(HarpDvError::InvalidMesh(format!(
                "the site table has {} rows and the triangulation {} sites; a rollback or an \
                 insertion left them out of step",
                self.sites.len(),
                self.state.vertex_count()
            )));
        }
        let mut levels = vec![1usize; self.state.triangles().len()];
        for (triangle, level) in levels.iter_mut().enumerate().skip(MESH_STATE_FIRST_ID) {
            *level = self.state.triangles()[triangle]
                .iter()
                .filter_map(|&corner| {
                    self.sites
                        .get(corner.checked_sub(MESH_STATE_FIRST_ID)?)
                        .map(|site| usize::from(site.depth) + 1)
                })
                .max()
                .unwrap_or(1);
        }
        // Grid number 2, not 1. The nest spring only moves points whose `ngr`
        // matches the one it is called with, and it refuses `ngr <= 1` -- so a
        // mesh marked 1 is a mesh no spring will touch, which is what this was
        // and why 5000 iterations changed nothing at all.
        self.state
            .to_triangular_mesh_with_grid_number(self.impent, Some(&levels), 2)
            .map_err(HarpDvError::Io)
    }

    /// The triangulation, to change.
    ///
    /// Crate-private: a caller that writes here without also recording the
    /// site leaves the two out of step, and the site table is what every id in
    /// every report resolves against.
    pub(crate) fn state_mut(&mut self) -> &mut MeshState {
        &mut self.state
    }

    /// Give a vertex an insertion just created its identity.
    ///
    /// Called only after the gates pass, so an id is never spent on a site the
    /// run then rolled back.
    /// Record a site an insertion just created, one generation deeper than the
    /// cell it refined.
    ///
    /// `parent` is the site whose demand this served. Depth is a refinement
    /// generation, so it counts halvings of the cell that asked -- not the
    /// length of the chain of insertions that reached here, which is what
    /// taking the deepest neighbour produced: a two-level request reported
    /// thirteen.
    pub(crate) fn adopt_inserted_site(&mut self, vertex: usize, parent: Option<usize>) -> SiteId {
        let position = xyz_to_lonlat_degrees(self.state.vertices()[vertex]);
        let site_id = self.allocator.allocate();
        let mut site = AdaptiveSite::inherited(site_id, position);
        site.birth_cycle = self.cycles_completed + 1;
        site.depth = parent
            .and_then(|parent| self.sites.get(parent.checked_sub(MESH_STATE_FIRST_ID)?))
            .map_or(1, |parent| parent.depth.saturating_add(1));
        self.sites.push(site);
        site_id
    }

    /// Record that a site moved: its position, and what that cost its budget.
    pub(crate) fn record_moved_site(&mut self, vertex: usize) -> SiteId {
        let position = xyz_to_lonlat_degrees(self.state.vertices()[vertex]);
        let row = vertex - MESH_STATE_FIRST_ID;
        let site = &mut self.sites[row];
        let unit_from = crate::state::displacement_metres(site.position, position);
        site.cumulative_displacement_m += unit_from;
        site.position = position;
        site.site_id
    }

    /// Give the refinement the boundary it must respect, as segments.
    ///
    /// Without them a quality-driven refinement subdivides without end near a
    /// region's edge -- guide 11.25.
    pub fn protect_segments(&mut self, segments: impl IntoIterator<Item = (usize, usize)>) {
        self.segments = segments
            .into_iter()
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
    }

    pub(crate) fn is_protected_edge(&self, tail: usize, head: usize) -> bool {
        self.segments.contains(&(tail.min(head), tail.max(head)))
    }

    pub(crate) fn segments_are_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Replace a segment with its two halves, once its midpoint exists.
    ///
    /// The induction Ruppert's proof runs on: a split segment is two segments,
    /// so the rule that made the refinement terminate keeps applying where it
    /// was just applied.
    pub(crate) fn split_segment(&mut self, tail: usize, head: usize, midpoint: usize) {
        let key = (tail.min(head), tail.max(head));
        if self.segments.remove(&key) {
            self.segments
                .insert((tail.min(midpoint), tail.max(midpoint)));
            self.segments
                .insert((head.min(midpoint), head.max(midpoint)));
        }
    }

    pub(crate) fn refining_site(&self) -> Option<usize> {
        self.refining
    }

    pub fn state(&self) -> &MeshState {
        &self.state
    }

    pub fn into_state(self) -> MeshState {
        self.state
    }

    pub fn sites(&self) -> &[AdaptiveSite] {
        &self.sites
    }

    pub fn active_site_count(&self) -> usize {
        self.sites.iter().filter(|site| site.active).count()
    }

    pub fn cycles_completed(&self) -> u32 {
        self.cycles_completed
    }

    /// The id the next adapted site would take.
    pub fn next_site_id(&self) -> SiteId {
        self.allocator.peek()
    }
}
