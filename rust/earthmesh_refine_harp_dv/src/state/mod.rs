//! What HARP-DV adapts, and what it remembers about it.

mod id_allocator;

pub use id_allocator::SiteIdAllocator;

use earthmesh_boundary::local_equal_area_overlap_fraction_lonlat;
use earthmesh_mesh::{
    xyz_to_lonlat_degrees, LonLatDegrees, MeshState, RetirementError, RetirementReport,
    MESH_STATE_FIRST_ID,
};

use crate::candidate::CandidateSource;
use crate::error::{HarpDvError, Result};

/// One conservative transfer from a pre-adaptation cell to a post-adaptation cell.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConservativeRemapWeight {
    pub old_site_id: SiteId,
    pub new_site_id: SiteId,
    pub overlap_fraction: f64,
}

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
    pub parent_site_id: Option<SiteId>,
    pub position: LonLatDegrees,
    /// The cycle that created it. Zero means it came in with the mesh.
    pub birth_cycle: u32,
    /// The production ladder rung that created it. None for inherited sites and low-level tests.
    pub birth_candidate_source: Option<CandidateSource>,
    /// How many times it has been through a refinement that created it.
    pub depth: u16,
    /// False once a transaction removes it. The row stays so its id keeps
    /// meaning what it meant.
    pub active: bool,
    /// Where it was when the run first saw it, which is what a displacement
    /// budget is measured against.
    pub origin_position: LonLatDegrees,
    pub cumulative_displacement_m: f64,
    pub mobility: SiteMobility,
}

impl AdaptiveSite {
    /// A site as it arrives with the mesh, before any adaptation.
    pub fn inherited(site_id: SiteId, position: LonLatDegrees) -> Self {
        Self {
            site_id,
            parent_site_id: None,
            position,
            birth_cycle: 0,
            birth_candidate_source: None,
            depth: 0,
            active: true,
            origin_position: position,
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
    /// Site records indexed by `SiteId.0`. Rows are tombstones once retired.
    sites: Vec<AdaptiveSite>,
    /// Mesh vertex row -> stable site id. Slots 0 and 1 are the mesh placeholders.
    vertex_site_ids: Vec<Option<SiteId>>,
    allocator: SiteIdAllocator,
    cycles_completed: u32,
    /// The site whose demand is being served, so a new one can be recorded a
    /// generation deeper than it.
    pub(crate) refining: Option<usize>,
    pub(crate) refining_candidate_source: Option<CandidateSource>,
    /// The boundary as a list of segments, each an edge of the mesh.
    ///
    /// Ruppert's PSLG. An earlier version approximated this with a set of
    /// boundary *sites* and called any edge between two of them a segment,
    /// which is a different predicate -- two boundary sites adjacent without
    /// lying on the curve read as a segment, and every split made more such
    /// pairs. Guide 11.28 measured what that cost.
    ///
    /// The list and its split rule live in `earthmesh_boundary` -- the same
    /// one Method-C and red-green can reach -- because the induction is not
    /// this backend's. Nothing here has to undo a split: it happens only after
    /// a transaction commits, so a rollback never sees one.
    pub(crate) segments: earthmesh_boundary::SegmentList,
    conservative_remap: Vec<ConservativeRemapWeight>,
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
        let mut vertex_site_ids = vec![None; state.vertices().len()];
        for vertex in state.active_vertex_slots() {
            let position = xyz_to_lonlat_degrees(state.vertices()[vertex]);
            let site = AdaptiveSite::inherited(allocator.allocate(), position);
            vertex_site_ids[vertex] = Some(site.site_id);
            sites.push(site);
        }
        Ok(Self {
            state,
            impent: [MESH_STATE_FIRST_ID; 12],
            sites,
            vertex_site_ids,
            allocator,
            cycles_completed: 0,
            refining: None,
            refining_candidate_source: None,
            segments: earthmesh_boundary::SegmentList::default(),
            conservative_remap: Vec::new(),
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
        if self.vertex_site_ids.len() != self.state.vertices().len() {
            return Err(HarpDvError::InvalidMesh(format!(
                "the vertex-site map has {} rows and the triangulation {} vertex rows",
                self.vertex_site_ids.len(),
                self.state.vertices().len()
            )));
        }
        let mut levels = vec![1usize; self.state.triangles().len()];
        for triangle in self.state.active_triangle_slots() {
            levels[triangle] = self.state.triangles()[triangle]
                .iter()
                .filter_map(|&corner| {
                    self.site_for_vertex(corner)
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
        let parent_site = parent
            .and_then(|parent| self.site_for_vertex(parent))
            .map(|site| (site.site_id, site.depth));
        let site_id = self.allocator.allocate();
        let mut site = AdaptiveSite::inherited(site_id, position);
        site.parent_site_id = parent_site.map(|(site_id, _)| site_id);
        site.birth_cycle = self.cycles_completed + 1;
        site.birth_candidate_source = self.refining_candidate_source;
        site.depth = parent_site.map_or(1, |(_, depth)| depth.saturating_add(1));
        self.sites.push(site);
        if vertex >= self.vertex_site_ids.len() {
            self.vertex_site_ids.resize(vertex + 1, None);
        }
        self.vertex_site_ids[vertex] = Some(site_id);
        site_id
    }

    /// Record that sites moved: their positions, and what that cost their budgets.
    pub(crate) fn record_moved_sites(&mut self, vertices: &[usize]) -> Option<SiteId> {
        let site_ids = vertices
            .iter()
            .map(|&vertex| {
                self.vertex_site_ids
                    .get(vertex)
                    .and_then(|site_id| *site_id)
            })
            .collect::<Option<Vec<_>>>()?;
        if site_ids.iter().any(|site_id| {
            self.sites
                .get(site_id.0 as usize)
                .is_none_or(|site| !site.active)
        }) {
            return None;
        }
        let first = *site_ids.first()?;
        for (&vertex, site_id) in vertices.iter().zip(site_ids) {
            let position = xyz_to_lonlat_degrees(self.state.vertices()[vertex]);
            let site = &mut self.sites[site_id.0 as usize];
            let unit_from = crate::state::displacement_metres(site.position, position);
            site.cumulative_displacement_m += unit_from;
            site.position = position;
        }
        Some(first)
    }

    /// Give the refinement the boundary it must respect, as segments.
    ///
    /// Without them a quality-driven refinement subdivides without end near a
    /// region's edge -- guide 11.25.
    pub fn protect_segments(&mut self, segments: impl IntoIterator<Item = (usize, usize)>) {
        self.segments = earthmesh_boundary::SegmentList::from_pairs(segments);
    }

    pub(crate) fn segments_are_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub(crate) fn can_move_site(&self, vertex: usize) -> bool {
        self.site_for_vertex(vertex)
            .is_some_and(|site| site.mobility == SiteMobility::Interior)
    }

    pub(crate) fn site_for_vertex(&self, vertex: usize) -> Option<&AdaptiveSite> {
        self.vertex_site_ids
            .get(vertex)
            .and_then(|site_id| *site_id)
            .and_then(|site_id| self.sites.get(site_id.0 as usize))
            .filter(|site| site.active)
    }

    pub(crate) fn is_retirable_leaf(&self, vertex: usize) -> bool {
        let Some(site) = self.site_for_vertex(vertex) else {
            return false;
        };
        site.parent_site_id.is_some()
            && site.mobility == SiteMobility::Interior
            && !self.impent.contains(&vertex)
            && !self
                .sites
                .iter()
                .any(|child| child.active && child.parent_site_id == Some(site.site_id))
    }

    pub(crate) fn retire_leaf_transactionally(
        &mut self,
        vertex: usize,
        mut postcondition: impl FnMut(&MeshState, &RetirementReport) -> bool,
    ) -> std::result::Result<RetirementReport, RetirementError> {
        if !self.is_retirable_leaf(vertex) {
            return Err(RetirementError::Rejected);
        }
        let site_id = self.vertex_site_ids[vertex].expect("a retirable leaf has a stable id");
        let before_state = self.state.clone();
        let mut remap = None;
        let report = self
            .state
            .retire_vertex_transactionally(vertex, |state, report| {
                if !postcondition(state, report) {
                    return false;
                }
                remap = conservative_remap_for_retirement(
                    &before_state,
                    state,
                    &self.vertex_site_ids,
                    vertex,
                    report,
                );
                remap.is_some()
            })?;
        self.sites[site_id.0 as usize].active = false;
        self.vertex_site_ids[vertex] = None;
        let remap = remap.expect("the retirement postcondition required a remap");
        if self.conservative_remap.is_empty() {
            self.conservative_remap = remap;
        } else {
            self.conservative_remap = compose_conservative_remap(&self.conservative_remap, &remap);
        }
        Ok(report)
    }

    #[cfg(test)]
    pub(crate) fn retire_degree_four_leaf_transactionally(
        &mut self,
        vertex: usize,
        postcondition: impl FnMut(&MeshState, &RetirementReport) -> bool,
    ) -> std::result::Result<RetirementReport, RetirementError> {
        if self.state.vertex_degree(vertex).ok() != Some(4) {
            return Err(RetirementError::Rejected);
        }
        self.retire_leaf_transactionally(vertex, postcondition)
    }

    #[allow(dead_code)]
    pub(crate) fn vertex_for_site_id(&self, site_id: SiteId) -> Option<usize> {
        self.vertex_site_ids
            .iter()
            .position(|mapped| *mapped == Some(site_id))
    }

    /// Replace a segment with its two halves, once its midpoint exists.
    ///
    /// The induction Ruppert's proof runs on: a split segment is two segments,
    /// so the rule that made the refinement terminate keeps applying where it
    /// was just applied.
    pub(crate) fn split_segment(&mut self, tail: usize, head: usize, midpoint: usize) {
        self.segments.split(tail, head, midpoint);
    }

    pub(crate) fn refining_site(&self) -> Option<usize> {
        self.refining
    }

    pub(crate) fn set_refining_context(
        &mut self,
        parent: Option<usize>,
        source: Option<CandidateSource>,
    ) {
        self.refining = parent;
        self.refining_candidate_source = source;
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

    pub(crate) fn record_cycle_completed(&mut self) {
        self.cycles_completed += 1;
    }

    /// The id the next adapted site would take.
    pub fn next_site_id(&self) -> SiteId {
        self.allocator.peek()
    }

    pub fn conservative_remap(&self) -> &[ConservativeRemapWeight] {
        &self.conservative_remap
    }
}

fn voronoi_ring_lonlat(state: &MeshState, vertex: usize) -> Option<Vec<(f64, f64)>> {
    Some(
        state
            .voronoi_cell(vertex)
            .ok()?
            .corners
            .into_iter()
            .map(xyz_to_lonlat_degrees)
            .map(|point| (point.lon_degrees, point.lat_degrees))
            .collect(),
    )
}

fn conservative_remap_for_retirement(
    before: &MeshState,
    after: &MeshState,
    vertex_site_ids: &[Option<SiteId>],
    retired_vertex: usize,
    report: &RetirementReport,
) -> Option<Vec<ConservativeRemapWeight>> {
    let mut affected = report
        .fan
        .iter()
        .flat_map(|&triangle| before.triangles()[triangle])
        .filter(|&vertex| vertex != retired_vertex)
        .collect::<std::collections::BTreeSet<_>>();
    affected.insert(retired_vertex);
    let new_vertices = affected
        .iter()
        .copied()
        .filter(|&vertex| vertex != retired_vertex && after.is_vertex_live(vertex))
        .collect::<Vec<_>>();
    let new_rings = new_vertices
        .iter()
        .map(|&vertex| Some((vertex, voronoi_ring_lonlat(after, vertex)?)))
        .collect::<Option<Vec<_>>>()?;
    let mut rows = Vec::new();
    for old_vertex in affected {
        let old_site_id = vertex_site_ids.get(old_vertex).and_then(|id| *id)?;
        let old_ring = voronoi_ring_lonlat(before, old_vertex)?;
        let mut weights = new_rings
            .iter()
            .filter_map(|(new_vertex, new_ring)| {
                let overlap = local_equal_area_overlap_fraction_lonlat(&old_ring, new_ring)?;
                (overlap > 1.0e-12).then_some((*new_vertex, overlap))
            })
            .collect::<Vec<_>>();
        let sum: f64 = weights.iter().map(|(_, weight)| *weight).sum();
        if !sum.is_finite() || (sum - 1.0).abs() > 1.0e-5 {
            return None;
        }
        for (new_vertex, weight) in weights.drain(..) {
            rows.push(ConservativeRemapWeight {
                old_site_id,
                new_site_id: vertex_site_ids.get(new_vertex).and_then(|id| *id)?,
                overlap_fraction: weight / sum,
            });
        }
    }
    Some(rows)
}

fn compose_conservative_remap(
    prior: &[ConservativeRemapWeight],
    next: &[ConservativeRemapWeight],
) -> Vec<ConservativeRemapWeight> {
    let next_by_old = next.iter().fold(
        std::collections::BTreeMap::<SiteId, Vec<(SiteId, f64)>>::new(),
        |mut rows, weight| {
            rows.entry(weight.old_site_id)
                .or_default()
                .push((weight.new_site_id, weight.overlap_fraction));
            rows
        },
    );
    let mut composed = std::collections::BTreeMap::<(SiteId, SiteId), f64>::new();
    let prior_old = prior
        .iter()
        .map(|weight| weight.old_site_id)
        .collect::<std::collections::BTreeSet<_>>();
    for weight in prior {
        if let Some(replacements) = next_by_old.get(&weight.new_site_id) {
            for &(new_site_id, fraction) in replacements {
                *composed
                    .entry((weight.old_site_id, new_site_id))
                    .or_default() += weight.overlap_fraction * fraction;
            }
        } else {
            *composed
                .entry((weight.old_site_id, weight.new_site_id))
                .or_default() += weight.overlap_fraction;
        }
    }
    for weight in next {
        if !prior_old.contains(&weight.old_site_id) {
            *composed
                .entry((weight.old_site_id, weight.new_site_id))
                .or_default() += weight.overlap_fraction;
        }
    }
    composed
        .into_iter()
        .filter(|(_, fraction)| *fraction > 1.0e-12)
        .map(
            |((old_site_id, new_site_id), overlap_fraction)| ConservativeRemapWeight {
                old_site_id,
                new_site_id,
                overlap_fraction,
            },
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::transaction::{Acceptance, HardGates};
    use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, CartesianPoint, TriangularMesh};

    fn sphere(nxp: usize) -> AdaptiveMesh {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
        AdaptiveMesh::from_triangular_mesh(&mesh).expect("adaptive mesh")
    }

    fn on(mesh: &AdaptiveMesh, lon: f64, lat: f64) -> CartesianPoint {
        let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(lon, lat));
        let radius = mesh.state().sphere_radius();
        CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius)
    }

    fn permissive() -> HardGates {
        HardGates {
            min_triangle_angle_deg: 0.0,
            ..HardGates::default()
        }
    }

    fn degree_three_leaf() -> (AdaptiveMesh, usize, SiteId) {
        let mut mesh = sphere(4);
        let report = mesh
            .propose_site_for(on(&mesh, -170.0, -5.0), None, permissive(), 20)
            .expect("degree-three proposal")
            .committed()
            .expect("degree-three proposal commits")
            .clone();
        assert_eq!(mesh.state.vertex_degree(report.vertex), Ok(3));
        assert!(mesh.is_retirable_leaf(report.vertex));
        (mesh, report.vertex, report.site_id)
    }

    #[test]
    fn rejected_degree_three_leaf_retirement_restores_all_adaptive_state() {
        let (mut mesh, child, _) = degree_three_leaf();
        let before = mesh.clone();

        let error = mesh
            .retire_leaf_transactionally(child, |_, _| false)
            .expect_err("postcondition rejects retirement");

        assert_eq!(error, RetirementError::Rejected);
        assert_eq!(mesh.state, before.state);
        assert_eq!(mesh.impent, before.impent);
        assert_eq!(mesh.sites, before.sites);
        assert_eq!(mesh.vertex_site_ids, before.vertex_site_ids);
        assert_eq!(mesh.allocator, before.allocator);
        assert_eq!(mesh.cycles_completed, before.cycles_completed);
        assert_eq!(mesh.refining, before.refining);
        assert_eq!(mesh.segments, before.segments);
        assert_eq!(mesh.conservative_remap, before.conservative_remap);
    }

    #[test]
    fn degree_three_leaf_retirement_tombstones_id_and_builds_remap() {
        let (mut mesh, child, child_id) = degree_three_leaf();
        let before = mesh.active_site_count();

        mesh.retire_leaf_transactionally(child, |state, _| state.validate().is_ok())
            .expect("retire degree-three leaf");

        assert_eq!(mesh.active_site_count(), before - 1);
        assert!(!mesh.sites[child_id.0 as usize].active);
        assert!(mesh.vertex_for_site_id(child_id).is_none());
        assert!(!mesh.state.is_vertex_live(child));
        assert!(mesh
            .conservative_remap
            .iter()
            .any(|weight| weight.old_site_id == child_id));
    }

    #[test]
    fn from_mesh_state_ignores_retired_slots() {
        let mut fixture = None;
        'search: for lon in (-160..=160).step_by(20) {
            for lat in (-60..=60).step_by(20) {
                let mut trial = sphere(6);
                if let Acceptance::Committed(report) = trial
                    .propose_site(on(&trial, lon as f64, lat as f64), permissive())
                    .expect("proposal")
                {
                    if trial.state().vertex_degree(report.vertex).ok() == Some(4)
                        && !trial.pentagon_ids().contains(&report.vertex)
                    {
                        let retired_vertex = report.vertex;
                        let impent = trial.pentagon_ids();
                        let mut state = trial.into_state();
                        if let Ok(retirement) = state.retire_degree_four_vertex_transactionally(
                            retired_vertex,
                            |state, _| state.validate().is_ok(),
                        ) {
                            fixture = Some((state, impent, retired_vertex, retirement));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (state, impent, retired_vertex, report) =
            fixture.expect("fixture has a retirable degree-four inserted site");
        assert!(!state.is_vertex_live(retired_vertex));
        assert!(report
            .retired_faces
            .iter()
            .all(|&face| !state.is_triangle_live(face)));

        let mut adaptive = AdaptiveMesh::from_mesh_state(state.clone()).expect("adaptive mesh");
        adaptive.impent = impent;

        assert_eq!(adaptive.active_site_count(), state.vertex_count());
        assert_eq!(adaptive.sites().len(), state.vertex_count());
        assert!(adaptive.site_for_vertex(retired_vertex).is_none());
        assert!(state
            .active_vertex_slots()
            .all(|vertex| adaptive.site_for_vertex(vertex).is_some()));
        adaptive
            .to_triangular_mesh()
            .expect("export skips dead faces");
    }

    #[test]
    fn degree_four_leaf_retirement_tombstones_its_stable_id() {
        let mut fixture = None;
        'search: for lon in (-160..=160).step_by(20) {
            for lat in (-60..=60).step_by(20) {
                let mut trial = sphere(6);
                let parent = 20;
                if let Acceptance::Committed(report) = trial
                    .propose_site_for(
                        on(&trial, lon as f64, lat as f64),
                        None,
                        permissive(),
                        parent,
                    )
                    .expect("proposal")
                {
                    if trial.state().vertex_degree(report.vertex).ok() == Some(4)
                        && !trial.pentagon_ids().contains(&report.vertex)
                    {
                        let vertex = report.vertex;
                        let site_id = report.site_id;
                        let before = trial.active_site_count();
                        if trial
                            .retire_degree_four_leaf_transactionally(vertex, |state, _| {
                                state.validate().is_ok()
                            })
                            .is_ok()
                        {
                            fixture = Some((trial, vertex, site_id, before));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (mesh, vertex, site_id, before) =
            fixture.expect("fixture has a retirable degree-four leaf");

        assert_eq!(mesh.active_site_count(), before - 1);
        assert!(!mesh.sites()[site_id.0 as usize].active);
        assert!(mesh.vertex_for_site_id(site_id).is_none());
        assert!(!mesh.state().is_vertex_live(vertex));
        let rows = mesh.conservative_remap();
        assert!(rows.iter().any(|row| row.old_site_id == site_id));
        for old_site_id in rows
            .iter()
            .map(|row| row.old_site_id)
            .collect::<std::collections::BTreeSet<_>>()
        {
            let sum: f64 = rows
                .iter()
                .filter(|row| row.old_site_id == old_site_id)
                .map(|row| row.overlap_fraction)
                .sum();
            assert!((sum - 1.0).abs() <= 1.0e-12, "{old_site_id:?}: {sum}");
        }
        mesh.to_triangular_mesh()
            .expect("tombstones compact on export");
    }

    #[test]
    fn consecutive_retirement_maps_compose_past_tombstoned_targets() {
        let prior = [ConservativeRemapWeight {
            old_site_id: SiteId(1),
            new_site_id: SiteId(2),
            overlap_fraction: 1.0,
        }];
        let next = [
            ConservativeRemapWeight {
                old_site_id: SiteId(2),
                new_site_id: SiteId(3),
                overlap_fraction: 0.4,
            },
            ConservativeRemapWeight {
                old_site_id: SiteId(2),
                new_site_id: SiteId(4),
                overlap_fraction: 0.6,
            },
        ];

        let rows = compose_conservative_remap(&prior, &next);

        assert!(!rows.iter().any(|row| row.new_site_id == SiteId(2)));
        for old_site_id in [SiteId(1), SiteId(2)] {
            let sum: f64 = rows
                .iter()
                .filter(|row| row.old_site_id == old_site_id)
                .map(|row| row.overlap_fraction)
                .sum();
            assert!((sum - 1.0).abs() <= 1.0e-12);
        }
    }
}
