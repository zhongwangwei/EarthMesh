//! What HARP-DV adapts, and what it remembers about it.

mod id_allocator;

pub use id_allocator::SiteIdAllocator;

use earthmesh_mesh::{xyz_to_lonlat_degrees, LonLatDegrees, TriangularMesh};

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
/// Wraps `TriangularMesh` because that is the only mesh type the repository
/// has. It is also Method-C's own type -- `mrow`, `ngr` and `mrlw` are its
/// transition rows and generations -- so this wrapper is where a
/// backend-neutral mesh state belongs once one exists. That type is the same
/// one splitting Method-C into its own crate needs, and
/// `docs/HARP_DV_REUSE_MAP.md` records why the two are one job.
#[derive(Clone, Debug)]
pub struct AdaptiveMesh {
    mesh: TriangularMesh,
    sites: Vec<AdaptiveSite>,
    allocator: SiteIdAllocator,
    cycles_completed: u32,
}

impl AdaptiveMesh {
    /// Take a mesh and give every one of its M points a stable id.
    ///
    /// M points are the Voronoi sites: the polygon centres the dual is built
    /// around. Slots 0 and 1 are the canonical placeholders and are not sites,
    /// so ids start at the first real point.
    pub fn from_triangular_mesh(mesh: TriangularMesh) -> Result<Self> {
        if mesh.nmd < 2 {
            return Err(HarpDvError::InvalidMesh(format!(
                "a mesh with {} M points carries no sites to adapt",
                mesh.nmd
            )));
        }
        let mut allocator = SiteIdAllocator::default();
        let mut sites = Vec::with_capacity(mesh.nmd.saturating_sub(1));
        for im in 2..=mesh.nmd {
            let position = xyz_to_lonlat_degrees(mesh.m_points[im]);
            sites.push(AdaptiveSite::inherited(allocator.allocate(), position));
        }
        Ok(Self {
            mesh,
            sites,
            allocator,
            cycles_completed: 0,
        })
    }

    pub fn triangular_mesh(&self) -> &TriangularMesh {
        &self.mesh
    }

    pub fn into_triangular_mesh(self) -> TriangularMesh {
        self.mesh
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
