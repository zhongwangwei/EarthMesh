//! What HARP-DV adapts, and what it remembers about it.

mod id_allocator;

pub use id_allocator::SiteIdAllocator;

use earthmesh_mesh::{xyz_to_lonlat_degrees, LonLatDegrees, MeshState, MESH_STATE_FIRST_ID};

use crate::error::{HarpDvError, Result};

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
    sites: Vec<AdaptiveSite>,
    allocator: SiteIdAllocator,
    cycles_completed: u32,
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
            sites,
            allocator,
            cycles_completed: 0,
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
        Self::from_mesh_state(state)
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
    pub(crate) fn adopt_inserted_site(&mut self, vertex: usize) -> SiteId {
        let position = xyz_to_lonlat_degrees(self.state.vertices()[vertex]);
        let site_id = self.allocator.allocate();
        let mut site = AdaptiveSite::inherited(site_id, position);
        site.birth_cycle = self.cycles_completed + 1;
        site.depth = 1;
        self.sites.push(site);
        site_id
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
