//! A boundary as topology, not as a list of points.
//!
//! A coastline read afresh each pass is a sequence of coordinates, and nothing
//! in it says which side is water, which loop is a lake inside an island, or
//! that this segment may be split but never crossed. Those are the facts a
//! refinement has to preserve, so they live here as a model the run holds onto
//! rather than as something rediscovered from geometry every time.
//!
//! Backend neutral by construction: no criterion, no data source, no refinement
//! policy. What a backend *does* about a boundary is the backend's business;
//! what the boundary *is* belongs here.
//!
//! # Scope
//!
//! The types and their invariants. The adaptation策略 -- encroachment, segment
//! splitting, sliding, narrow-feature policy -- belong to whichever backend is
//! doing the adapting.

pub mod equal_area;
pub mod rings;
pub mod segments;
pub use equal_area::{
    bounds_overlap, is_convex, local_equal_area_overlap_fraction,
    local_equal_area_overlap_fraction_lonlat, ring_bounds, LocalEqualArea, SphericalCap,
};
pub use rings::{closed_rings, RingError};
pub use segments::SegmentList;

use std::collections::BTreeMap;

use earthmesh_geometry::{
    try_spherical_polygon_area, try_spherical_polygon_signed_minor_excess_fast, Point,
    SphericalArea, SphericalPolygonError,
};

/// What a boundary means for the mesh that meets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryRole {
    /// A coastline, a basin outline. No cell may cross it.
    HardDomain,
    /// Land against sea, one material against another. Cells live on both
    /// sides; the curve is where they meet.
    MaterialInterface,
    /// A river, a levee, a fault. Has to appear in the mesh as edges.
    EmbeddedFeature,
    /// A regional ocean's open edge. Marker and ordering are part of the
    /// output, so both have to survive refinement.
    OpenBoundary,
    /// A storm track, a named corridor. Creates demand and constrains nothing.
    RefinementGuide,
    /// The two sides of a periodic domain, which must stay in correspondence.
    PeriodicSeam,
}

impl BoundaryRole {
    /// Whether the mesh is forbidden to cross this curve.
    pub fn is_impassable(self) -> bool {
        matches!(self, Self::HardDomain | Self::PeriodicSeam)
    }

    /// Whether an edge on this curve may be removed by an ordinary flip.
    ///
    /// Only a guide may: it constrains nothing, so nothing is lost by
    /// reshaping across it.
    pub fn permits_edge_flip(self) -> bool {
        matches!(self, Self::RefinementGuide)
    }
}

/// Which side of the domain a loop encloses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoopType {
    /// The outside of a region.
    Outer,
    /// A lake, or the sea inside an atoll. Held as its own loop rather than
    /// joined to the outer one by a cut, because a cut is a lie about the
    /// topology that later passes cannot tell from a real edge.
    Hole,
}

/// A point on the boundary, and how free it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryVertex {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
    /// A corner or a junction of curves. Refinement may not move it.
    pub pinned: bool,
}

/// Conservative evidence for a valid spherical cell against this domain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolygonOverlap {
    pub positive_area: bool,
    pub intersects_boundary: bool,
}

/// One closed ring of boundary vertices.
///
/// # Why `vertices` is not public
///
/// The order of this list *is* the loop's orientation, and orientation is what
/// decides which side of the ring is inside -- see [`SphericalBoundaryModel::contains`].
/// A ring walked off a mesh arrives with no orientation at all: the same edges
/// collected in a different order come back the other way round. So a loop
/// built by filling in this field would have an inside that depended on the
/// order its edges happened to be found in, silently and consistently.
///
/// The two constructors are the two honest ways to supply what the data does
/// not carry: [`Self::counter_clockwise`] for a caller that already knows the
/// order is right, and [`Self::enclosing`] for one that knows a point the loop
/// must contain and would rather be told than assert.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundaryLoop {
    pub loop_type: LoopType,
    pub role: BoundaryRole,
    /// Vertex indices in order. The ring closes implicitly; the first index is
    /// not repeated at the end.
    vertices: Vec<usize>,
    /// Which outer loop this hole sits in. `None` for an outer loop.
    pub parent: Option<usize>,
}

impl BoundaryLoop {
    /// A loop whose vertex order is already counter-clockwise seen from outside
    /// the sphere, so the region it encloses is the one on its left.
    ///
    /// The caller is asserting that. Nothing here can check it -- both sides of
    /// a closed curve on a sphere are enclosed by it, so there is no property
    /// of the ring alone that says which side was meant. Use [`Self::enclosing`]
    /// where the caller has a point instead of a conviction.
    pub fn counter_clockwise(
        loop_type: LoopType,
        role: BoundaryRole,
        vertices: Vec<usize>,
        parent: Option<usize>,
    ) -> Self {
        Self {
            loop_type,
            role,
            vertices,
            parent,
        }
    }

    /// A loop oriented so that it encloses `interior`, whichever way its
    /// vertices were given.
    ///
    /// This is the constructor to reach for after [`crate::closed_rings`],
    /// which cannot supply an orientation and says so. `model_vertices` are the
    /// positions the indices refer to; `interior` is a point the loop is known
    /// to contain -- a site inside the region, not a point on the ring.
    ///
    /// Returns `None` when the ring is degenerate or `interior` lies on it, in
    /// which case neither direction encloses the point and the caller has not
    /// said what it meant.
    pub fn enclosing(
        loop_type: LoopType,
        role: BoundaryRole,
        vertices: Vec<usize>,
        parent: Option<usize>,
        model_vertices: &[BoundaryVertex],
        interior: (f64, f64),
    ) -> Option<Self> {
        if vertices.len() < 3 {
            return None;
        }
        let probe = |order: &[usize]| SphericalBoundaryModel {
            vertices: model_vertices.to_vec(),
            loops: vec![Self {
                loop_type: LoopType::Outer,
                role,
                vertices: order.to_vec(),
                parent: None,
            }],
        };
        let forward = probe(&vertices).contains(interior.0, interior.1);
        let reversed: Vec<usize> = vertices.iter().rev().copied().collect();
        let backward = probe(&reversed).contains(interior.0, interior.1);
        // Exactly one direction must enclose the point. Both, or neither, means
        // the point is on the ring, and orienting against it would be picking a
        // side the caller never chose.
        match (forward, backward) {
            (true, false) => Some(Self::counter_clockwise(loop_type, role, vertices, parent)),
            (false, true) => Some(Self::counter_clockwise(loop_type, role, reversed, parent)),
            _ => None,
        }
    }

    /// A loop oriented so that it bounds the *smaller* of the two regions it
    /// divides the sphere into.
    ///
    /// The constructor for a ring walked off a mesh, where the caller has the
    /// ring and no point to orient it against -- a coastline's rings and the
    /// lakes inside them all want this, and a lake's interior point is exactly
    /// what a carve does not have to hand.
    ///
    /// "Smaller" is what makes an island an island rather than the ocean around
    /// it, and it is what makes nesting mean anything: an outer ring bounding
    /// the rest of the globe would not contain the lake sitting in it.
    ///
    /// Returns `None` for a degenerate ring, or one whose two sides come out
    /// equal, where there is no smaller side to pick.
    pub fn bounding_smaller_side(
        loop_type: LoopType,
        role: BoundaryRole,
        vertices: Vec<usize>,
        parent: Option<usize>,
        model_vertices: &[BoundaryVertex],
    ) -> Option<Self> {
        let points = points_for_ring(&vertices, model_vertices)?;
        let area =
            try_spherical_polygon_signed_minor_excess_fast(&points, |point| (point.x, point.y))
                .ok()?;
        if (area.abs() - std::f64::consts::TAU).abs() <= 64.0 * f64::EPSILON {
            return None;
        }
        // The signed-minor excess is already normalized by `earthmesh_geometry`,
        // so this does not depend on which vertex the fan starts at.
        let smaller_side_is_left = area > 0.0;
        let ordered = if smaller_side_is_left {
            vertices
        } else {
            vertices.into_iter().rev().collect()
        };
        Some(Self::counter_clockwise(loop_type, role, ordered, parent))
    }

    /// The ring, in traversal order.
    pub fn vertices(&self) -> &[usize] {
        &self.vertices
    }

    /// The ring the other way round, which encloses the complementary side.
    pub fn reversed(&self) -> Self {
        Self {
            vertices: self.vertices.iter().rev().copied().collect(),
            ..self.clone()
        }
    }
}

fn points_for_ring(ring: &[usize], vertices: &[BoundaryVertex]) -> Option<Vec<Point>> {
    ring.iter()
        .map(|&index| {
            vertices
                .get(index)
                .map(|point| Point::new(point.lon_degrees, point.lat_degrees))
        })
        .collect()
}

fn area_for_ring(
    ring: &[usize],
    vertices: &[BoundaryVertex],
) -> Result<SphericalArea, SphericalPolygonError> {
    let points = points_for_ring(ring, vertices)
        .ok_or(SphericalPolygonError::TooFewVertices { found: ring.len() })?;
    try_spherical_polygon_area(&points)
}

/// The signed minor area a ring encloses on the unit sphere, in steradians.
///
/// Positive when the ring runs counter-clockwise seen from outside, which is
/// the same convention [`SphericalBoundaryModel::contains`] reads. The geometry
/// crate owns the fan triangulation and `4π` normalization; this crate only
/// interprets the signed result as boundary topology.
fn signed_area_on_unit_sphere(ring: &[usize], vertices: &[BoundaryVertex]) -> Option<f64> {
    area_for_ring(ring, vertices)
        .ok()
        .map(|area| area.signed_minor_sr)
}

/// Whether a geodesic ring contains a point on its smaller spherical side.
///
/// Unlike a longitude ray-cast, the tangent-plane winding used here is valid
/// across the antimeridian and at either pole. Input order does not choose the
/// complementary side: reversing the ring still describes the same smaller
/// region. Points on a ring vertex count as contained.
pub fn spherical_ring_contains_minor<T>(
    ring: &[T],
    lon_degrees: f64,
    lat_degrees: f64,
    coordinates: impl Copy + Fn(&T) -> (f64, f64),
) -> bool {
    if !valid_lon_lat(lon_degrees, lat_degrees) {
        return false;
    }
    let Some(signed_minor_area) = signed_minor_area_for_coordinates(ring, coordinates) else {
        return false;
    };
    let Some(turn) = spherical_winding_turn(ring, lon_degrees, lat_degrees, coordinates) else {
        return true;
    };
    let desired_sign = signed_minor_area.signum();
    if desired_sign > 0.0 {
        turn > std::f64::consts::PI
    } else {
        turn < -std::f64::consts::PI
    }
}

/// A ring prepared for many [`spherical_ring_contains_minor`] queries, with
/// the same answers.
///
/// That function reads the whole ring per query -- its area, then a winding
/// sum with trigonometry at every vertex -- and a refinement index asked it
/// for every cell of a mesh: a 130,000-vertex basin outline against 36,000
/// cells. Prepared, a cap under 90 degrees holding every vertex turns away
/// points outside it with one dot product; inside it, the ring projected
/// gnomonically about the cap's centre (where great circles are straight
/// lines) gives the winding number by counting crossings in one band of
/// edges. Next to an edge, where the two could round apart, the query goes to
/// the original function.
#[derive(Clone, Debug)]
pub struct PreparedMinorRing {
    ring: Vec<(f64, f64)>,
    side: Option<f64>,
    cap: Option<([f64; 3], f64)>,
    plane: Option<GnomonicRing>,
}

impl PreparedMinorRing {
    pub fn new<T>(ring: &[T], coordinates: impl Copy + Fn(&T) -> (f64, f64)) -> Self {
        let ring: Vec<(f64, f64)> = ring.iter().map(coordinates).collect();
        let side = signed_minor_area_for_coordinates(&ring, |&p| p).map(f64::signum);
        let units: Vec<[f64; 3]> = ring
            .iter()
            .map(|&(lon, lat)| unit_from_lonlat(lon, lat))
            .collect();
        let cap = side.and_then(|_| minor_ring_cap(&units));
        let plane = cap.map(|(centre, _)| GnomonicRing::new(&units, centre));
        Self {
            ring,
            side,
            cap,
            plane,
        }
    }

    pub fn contains(&self, lon_degrees: f64, lat_degrees: f64) -> bool {
        if !valid_lon_lat(lon_degrees, lat_degrees) {
            return false;
        }
        let Some(side) = self.side else {
            return false;
        };
        let here = unit_from_lonlat(lon_degrees, lat_degrees);
        if self
            .cap
            .is_some_and(|(centre, cos_radius)| dot(here, centre) < cos_radius)
        {
            return false;
        }
        match self.plane.as_ref().and_then(|plane| plane.winding(here)) {
            Some(winding) if side > 0.0 => winding >= 1,
            Some(winding) => winding <= -1,
            None => spherical_ring_contains_minor(&self.ring, lon_degrees, lat_degrees, |&p| p),
        }
    }
}

fn unit_from_lonlat(lon: f64, lat: f64) -> [f64; 3] {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

/// A cap under 90 degrees about the vertices' mean direction holding them
/// all -- convex, so it holds the edges and the smaller side of the ring --
/// grown by 1e-5 radians, well past the 1.4e-6 within which a vertex counts
/// its neighbourhood as inside.
fn minor_ring_cap(units: &[[f64; 3]]) -> Option<([f64; 3], f64)> {
    let sum = units.iter().fold([0.0; 3], |acc, u| {
        [acc[0] + u[0], acc[1] + u[1], acc[2] + u[2]]
    });
    let length = norm(sum);
    if length <= 1.0e-12 {
        return None;
    }
    let centre = [sum[0] / length, sum[1] / length, sum[2] / length];
    let radius = units
        .iter()
        .map(|&u| angle(centre, u))
        .fold(0.0_f64, f64::max)
        + 1.0e-5;
    (radius < 89.0_f64.to_radians()).then(|| (centre, radius.cos()))
}

/// A ring in the gnomonic projection about `centre`, its edges bucketed into
/// bands of `y`.
#[derive(Clone, Debug)]
struct GnomonicRing {
    centre: [f64; 3],
    east: [f64; 3],
    north: [f64; 3],
    xy: Vec<(f64, f64)>,
    y_min: f64,
    band_height: f64,
    edges_in: Vec<Vec<u32>>,
}

impl GnomonicRing {
    fn new(units: &[[f64; 3]], centre: [f64; 3]) -> Self {
        let axis = if centre[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let east = cross(centre, axis);
        let length = norm(east);
        let east = [east[0] / length, east[1] / length, east[2] / length];
        let north = cross(centre, east);
        let mut ring = Self {
            centre,
            east,
            north,
            xy: Vec::new(),
            y_min: 0.0,
            band_height: 1.0,
            edges_in: Vec::new(),
        };
        ring.xy = units.iter().map(|&u| ring.project(u)).collect();
        let (y_min, y_max) = ring
            .xy
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &(_, y)| {
                (lo.min(y), hi.max(y))
            });
        let bands = ((units.len() as f64).sqrt().ceil() as usize).max(1);
        ring.y_min = y_min;
        ring.band_height = ((y_max - y_min) / bands as f64).max(f64::MIN_POSITIVE);
        ring.edges_in = vec![Vec::new(); bands];
        for k in 0..ring.xy.len() {
            let (a, b) = (ring.xy[k], ring.xy[(k + 1) % ring.xy.len()]);
            for band in ring.band_of(a.1.min(b.1))..=ring.band_of(a.1.max(b.1)) {
                ring.edges_in[band].push(k as u32);
            }
        }
        ring
    }

    fn project(&self, u: [f64; 3]) -> (f64, f64) {
        let z = dot(u, self.centre);
        (dot(u, self.east) / z, dot(u, self.north) / z)
    }

    fn band_of(&self, y: f64) -> usize {
        (((y - self.y_min) / self.band_height).floor().max(0.0) as usize)
            .min(self.edges_in.len() - 1)
    }

    /// The winding number about `here`, or `None` next to an edge.
    fn winding(&self, here: [f64; 3]) -> Option<i64> {
        let (x, y) = self.project(here);
        let near = 1.0e-5 * (1.0 + x * x + y * y);
        let band = self.band_of(y);
        let mut winding = 0i64;
        for b in band.saturating_sub(1)..=(band + 1).min(self.edges_in.len() - 1) {
            for &k in &self.edges_in[b] {
                let k = k as usize;
                let (p, q) = (self.xy[k], self.xy[(k + 1) % self.xy.len()]);
                let (dx, dy) = (q.0 - p.0, q.1 - p.1);
                let length = dx * dx + dy * dy;
                let t = if length > 0.0 {
                    (((x - p.0) * dx + (y - p.1) * dy) / length).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                if (x - p.0 - t * dx).hypot(y - p.1 - t * dy) <= near {
                    return None;
                }
                if b == band && (p.1 > y) != (q.1 > y) {
                    let s = (y - p.1) / (q.1 - p.1);
                    if p.0 + s * (q.0 - p.0) > x {
                        winding += if q.1 > p.1 { 1 } else { -1 };
                    }
                }
            }
        }
        Some(winding)
    }
}

fn signed_minor_area_for_coordinates<T>(
    ring: &[T],
    coordinates: impl Copy + Fn(&T) -> (f64, f64),
) -> Option<f64> {
    try_spherical_polygon_signed_minor_excess_fast(ring, coordinates).ok()
}

fn spherical_winding_turn<T>(
    ring: &[T],
    lon_degrees: f64,
    lat_degrees: f64,
    coordinates: impl Copy + Fn(&T) -> (f64, f64),
) -> Option<f64> {
    if ring.len() < 3 || !lon_degrees.is_finite() || !lat_degrees.is_finite() {
        return Some(0.0);
    }
    let to_unit = |lon: f64, lat: f64| {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let here = to_unit(lon_degrees, lat_degrees);
    let east = [-here[1], here[0], 0.0];
    let east_length = (east[0] * east[0] + east[1] * east[1]).sqrt();
    let east = if east_length > 1.0e-12 {
        [east[0] / east_length, east[1] / east_length, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let north = [
        here[1] * east[2] - here[2] * east[1],
        here[2] * east[0] - here[0] * east[2],
        here[0] * east[1] - here[1] * east[0],
    ];
    let tangent = |point: [f64; 3]| -> Option<(f64, f64)> {
        let dot = here[0] * point[0] + here[1] * point[1] + here[2] * point[2];
        let flat = [
            point[0] - here[0] * dot,
            point[1] - here[1] * dot,
            point[2] - here[2] * dot,
        ];
        let length = (flat[0] * flat[0] + flat[1] * flat[1] + flat[2] * flat[2]).sqrt();
        if length <= 1.0e-12 {
            return None;
        }
        Some((
            (flat[0] * east[0] + flat[1] * east[1] + flat[2] * east[2]) / length,
            (flat[0] * north[0] + flat[1] * north[1] + flat[2] * north[2]) / length,
        ))
    };
    let mut turned = 0.0;
    for step in 0..ring.len() {
        let (a_lon, a_lat) = coordinates(&ring[step]);
        let (b_lon, b_lat) = coordinates(&ring[(step + 1) % ring.len()]);
        let a_unit = to_unit(a_lon, a_lat);
        let b_unit = to_unit(b_lon, b_lat);
        let query_dot =
            |point: [f64; 3]| here[0] * point[0] + here[1] * point[1] + here[2] * point[2];
        if query_dot(a_unit) > 1.0 - 1.0e-12 || query_dot(b_unit) > 1.0 - 1.0e-12 {
            return None;
        }
        if query_dot(a_unit) < -1.0 + 1.0e-12 || query_dot(b_unit) < -1.0 + 1.0e-12 {
            return Some(0.0);
        }
        let (Some(a), Some(b)) = (tangent(a_unit), tangent(b_unit)) else {
            return Some(0.0);
        };
        turned += (a.0 * b.1 - a.1 * b.0).atan2(a.0 * b.0 + a.1 * b.1);
    }
    turned.is_finite().then_some(turned)
}

/// Every boundary the run has to respect, as one model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SphericalBoundaryModel {
    pub vertices: Vec<BoundaryVertex>,
    pub loops: Vec<BoundaryLoop>,
}

/// What is wrong with a boundary model, said precisely enough to fix.
#[derive(Clone, Debug, PartialEq)]
pub enum BoundaryError {
    /// A boundary vertex is not a finite lon/lat point on Earth.
    InvalidVertexCoordinate {
        vertex: usize,
        lon_degrees: f64,
        lat_degrees: f64,
    },
    /// A loop names a vertex that is not in the model.
    UnknownVertex { loop_index: usize, vertex: usize },
    /// A ring of fewer than three vertices does not enclose anything.
    DegenerateLoop { loop_index: usize, vertices: usize },
    /// A hole with no outer loop to be inside of.
    OrphanHole { loop_index: usize },
    /// A hole whose parent is not an outer loop.
    HoleInsideHole { loop_index: usize, parent: usize },
    /// An outer loop that names a parent.
    OuterLoopWithParent { loop_index: usize },
    /// The same vertex twice in one ring, which makes it pinch rather than
    /// close.
    RepeatedVertex { loop_index: usize, vertex: usize },
    /// A hole declares an outer parent, but one of its vertices is outside that parent.
    HoleOutsideParent {
        loop_index: usize,
        parent: usize,
        vertex: usize,
    },
    /// A ring crosses itself before it can enclose a single side.
    RingSelfIntersection {
        loop_index: usize,
        first_edge: usize,
        second_edge: usize,
    },
    /// A hole's direction describes the complement, not a finite void inside its parent.
    HoleWrongOrientation { loop_index: usize },
    /// A usable ring still has no unambiguous spherical area.
    UnresolvedRingArea { loop_index: usize },
    /// A boundary edge has the same physical point at both ends.
    CoincidentEdgeEndpoints {
        loop_index: usize,
        edge: usize,
        from: usize,
        to: usize,
    },
    /// A boundary edge joins antipodal points, so there is no unique minor arc.
    AntipodalEdgeEndpoints {
        loop_index: usize,
        edge: usize,
        from: usize,
        to: usize,
    },
    /// A hole touches or crosses its declared outer parent.
    HoleIntersectsParent { loop_index: usize, parent: usize },
    /// Two holes under the same outer parent touch or cross.
    SiblingHolesIntersect {
        parent: usize,
        first_loop: usize,
        second_loop: usize,
    },
    /// One hole under an outer parent sits inside another hole.
    SiblingHoleNested {
        parent: usize,
        outer_loop: usize,
        inner_loop: usize,
    },
}

impl std::fmt::Display for BoundaryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVertexCoordinate {
                vertex,
                lon_degrees,
                lat_degrees,
            } => write!(
                formatter,
                "vertex {vertex} is not a valid finite lon/lat point: ({lon_degrees}, {lat_degrees})"
            ),
            Self::UnknownVertex { loop_index, vertex } => write!(
                formatter,
                "loop {loop_index} names vertex {vertex}, which the model does not carry"
            ),
            Self::DegenerateLoop {
                loop_index,
                vertices,
            } => write!(
                formatter,
                "loop {loop_index} has {vertices} vertices; a ring needs at least three"
            ),
            Self::OrphanHole { loop_index } => write!(
                formatter,
                "loop {loop_index} is a hole with no outer loop to sit in"
            ),
            Self::HoleInsideHole { loop_index, parent } => write!(
                formatter,
                "loop {loop_index} is a hole whose parent {parent} is itself a hole"
            ),
            Self::OuterLoopWithParent { loop_index } => {
                write!(formatter, "loop {loop_index} is outer and names a parent")
            }
            Self::RepeatedVertex { loop_index, vertex } => write!(
                formatter,
                "loop {loop_index} visits vertex {vertex} twice, so it pinches rather than closes"
            ),
            Self::HoleOutsideParent {
                loop_index,
                parent,
                vertex,
            } => write!(
                formatter,
                "hole loop {loop_index} names vertex {vertex}, which is outside parent outer loop {parent}"
            ),
            Self::RingSelfIntersection {
                loop_index,
                first_edge,
                second_edge,
            } => write!(
                formatter,
                "loop {loop_index} crosses itself between edges {first_edge} and {second_edge}"
            ),
            Self::HoleWrongOrientation { loop_index } => write!(
                formatter,
                "hole loop {loop_index} is oriented as the complement, not as a finite void"
            ),
            Self::UnresolvedRingArea { loop_index } => write!(
                formatter,
                "loop {loop_index} has no unambiguous spherical area"
            ),
            Self::CoincidentEdgeEndpoints {
                loop_index,
                edge,
                from,
                to,
            } => write!(
                formatter,
                "loop {loop_index} edge {edge} has coincident endpoints {from} and {to}"
            ),
            Self::AntipodalEdgeEndpoints {
                loop_index,
                edge,
                from,
                to,
            } => write!(
                formatter,
                "loop {loop_index} edge {edge} joins antipodal endpoints {from} and {to}, so the minor great-circle arc is undefined"
            ),
            Self::HoleIntersectsParent { loop_index, parent } => write!(
                formatter,
                "hole loop {loop_index} touches or crosses parent outer loop {parent}"
            ),
            Self::SiblingHolesIntersect {
                parent,
                first_loop,
                second_loop,
            } => write!(
                formatter,
                "hole loops {first_loop} and {second_loop} under parent {parent} touch or cross"
            ),
            Self::SiblingHoleNested {
                parent,
                outer_loop,
                inner_loop,
            } => write!(
                formatter,
                "hole loop {inner_loop} sits inside sibling hole {outer_loop} under parent {parent}"
            ),
        }
    }
}

impl std::error::Error for BoundaryError {}

fn valid_lon_lat(lon_degrees: f64, lat_degrees: f64) -> bool {
    lon_degrees.is_finite() && lat_degrees.is_finite() && (-90.0..=90.0).contains(&lat_degrees)
}

fn unit_from_vertex(point: &BoundaryVertex) -> [f64; 3] {
    let (lon, lat) = (
        point.lon_degrees.to_radians(),
        point.lat_degrees.to_radians(),
    );
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn scale(a: [f64; 3], factor: f64) -> [f64; 3] {
    [a[0] * factor, a[1] * factor, a[2] * factor]
}

fn angle(a: [f64; 3], b: [f64; 3]) -> f64 {
    norm(cross(a, b)).atan2(dot(a, b))
}

fn point_on_minor_arc(a: [f64; 3], b: [f64; 3], p: [f64; 3]) -> bool {
    let ab = angle(a, b);
    if ab <= 1.0e-12 || (std::f64::consts::PI - ab).abs() <= 1.0e-10 {
        return false;
    }
    let normal = cross(a, b);
    if dot(normal, p).abs() > 1.0e-10 * norm(normal).max(1.0) {
        return false;
    }
    angle(a, p) + angle(p, b) <= ab + 1.0e-10
}

fn edge_endpoint_error(
    loop_index: usize,
    edge: usize,
    from: usize,
    to: usize,
    vertices: &[BoundaryVertex],
) -> Option<BoundaryError> {
    let a = unit_from_vertex(&vertices[from]);
    let b = unit_from_vertex(&vertices[to]);
    let separation = angle(a, b);
    if separation <= 1.0e-12 {
        Some(BoundaryError::CoincidentEdgeEndpoints {
            loop_index,
            edge,
            from,
            to,
        })
    } else if (std::f64::consts::PI - separation).abs() <= 1.0e-10 {
        Some(BoundaryError::AntipodalEdgeEndpoints {
            loop_index,
            edge,
            from,
            to,
        })
    } else {
        None
    }
}

fn spherical_segments_intersect(a0: [f64; 3], a1: [f64; 3], b0: [f64; 3], b1: [f64; 3]) -> bool {
    if point_on_minor_arc(a0, a1, b0)
        || point_on_minor_arc(a0, a1, b1)
        || point_on_minor_arc(b0, b1, a0)
        || point_on_minor_arc(b0, b1, a1)
    {
        return true;
    }
    let na = cross(a0, a1);
    let nb = cross(b0, b1);
    let line = cross(na, nb);
    let length = norm(line);
    if length <= 1.0e-12 {
        return false;
    }
    let p = scale(line, 1.0 / length);
    (point_on_minor_arc(a0, a1, p) && point_on_minor_arc(b0, b1, p))
        || (point_on_minor_arc(a0, a1, scale(p, -1.0))
            && point_on_minor_arc(b0, b1, scale(p, -1.0)))
}

/// Visit every pair of arcs whose boxes overlap -- the only pairs any arc
/// test here can report touching -- until `visit` returns true.
///
/// Rings and pairs of rings were compared edge against every edge, which a
/// 130,000-vertex basin outline made hours of trigonometry. A box is the
/// chord's, grown by the arc's sagitta (how far a minor arc leaves its
/// chord) and by 1e-4, far above the 1e-10 tolerances of the arc tests; the
/// boxes are swept along x.
fn sweep_overlapping_arcs(
    arcs: &[([f64; 3], [f64; 3])],
    mut visit: impl FnMut(usize, usize) -> bool,
) {
    let boxes: Vec<([f64; 3], [f64; 3])> = arcs
        .iter()
        .map(|&(a, b)| {
            let pad = (1.0 - (0.5 * angle(a, b)).cos()) + 1.0e-4;
            let mut low = [0.0; 3];
            let mut high = [0.0; 3];
            for k in 0..3 {
                low[k] = a[k].min(b[k]) - pad;
                high[k] = a[k].max(b[k]) + pad;
            }
            (low, high)
        })
        .collect();
    let mut order: Vec<usize> = (0..arcs.len()).collect();
    order.sort_by(|&i, &j| boxes[i].0[0].total_cmp(&boxes[j].0[0]).then(i.cmp(&j)));
    let mut active: Vec<usize> = Vec::new();
    for &arc in &order {
        let (low, high) = boxes[arc];
        active.retain(|&other| boxes[other].1[0] >= low[0]);
        for &other in &active {
            let (other_low, other_high) = boxes[other];
            if other_low[1] > high[1]
                || other_high[1] < low[1]
                || other_low[2] > high[2]
                || other_high[2] < low[2]
            {
                continue;
            }
            if visit(other, arc) {
                return;
            }
        }
        active.push(arc);
    }
}

fn spherical_segments_cross_strictly(
    a0: [f64; 3],
    a1: [f64; 3],
    b0: [f64; 3],
    b1: [f64; 3],
) -> bool {
    let na = cross(a0, a1);
    let nb = cross(b0, b1);
    let line = cross(na, nb);
    let length = norm(line);
    if length <= 1.0e-12 {
        return false;
    }
    let strictly_on_arc = |from, to, point| {
        let whole = angle(from, to);
        let start = angle(from, point);
        let end = angle(point, to);
        start > 1.0e-10 && end > 1.0e-10 && start + end <= whole + 1.0e-10
    };
    let intersection = scale(line, 1.0 / length);
    [intersection, scale(intersection, -1.0)]
        .into_iter()
        .any(|point| strictly_on_arc(a0, a1, point) && strictly_on_arc(b0, b1, point))
}

fn point_on_ring_boundary(point: [f64; 3], ring: &[[f64; 3]]) -> bool {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
        .any(|(&from, &to)| point_on_minor_arc(from, to, point))
}

impl SphericalBoundaryModel {
    /// Check every invariant the rest of the system is entitled to assume.
    ///
    /// Run once when the model is built. A boundary that is wrong here is wrong
    /// in a way that surfaces much later as a mesh with a hole nobody asked for.
    pub fn validate(&self) -> Result<(), Vec<BoundaryError>> {
        let mut errors = Vec::new();
        for (vertex_index, vertex) in self.vertices.iter().enumerate() {
            if !valid_lon_lat(vertex.lon_degrees, vertex.lat_degrees) {
                errors.push(BoundaryError::InvalidVertexCoordinate {
                    vertex: vertex_index,
                    lon_degrees: vertex.lon_degrees,
                    lat_degrees: vertex.lat_degrees,
                });
            }
        }
        for (loop_index, ring) in self.loops.iter().enumerate() {
            if ring.vertices().len() < 3 {
                errors.push(BoundaryError::DegenerateLoop {
                    loop_index,
                    vertices: ring.vertices().len(),
                });
            }
            let mut seen = BTreeMap::new();
            for &vertex in ring.vertices() {
                if vertex >= self.vertices.len() {
                    errors.push(BoundaryError::UnknownVertex { loop_index, vertex });
                    continue;
                }
                if seen.insert(vertex, ()).is_some() {
                    errors.push(BoundaryError::RepeatedVertex { loop_index, vertex });
                }
            }
            if self.ring_vertices_usable(ring) {
                for edge in 0..ring.vertices().len() {
                    let from = ring.vertices()[edge];
                    let to = ring.vertices()[(edge + 1) % ring.vertices().len()];
                    if let Some(error) =
                        edge_endpoint_error(loop_index, edge, from, to, &self.vertices)
                    {
                        errors.push(error);
                    }
                }
                if let Some((first_edge, second_edge)) = self.ring_self_intersection(ring) {
                    errors.push(BoundaryError::RingSelfIntersection {
                        loop_index,
                        first_edge,
                        second_edge,
                    });
                }
                if matches!(
                    area_for_ring(ring.vertices(), &self.vertices),
                    Err(SphericalPolygonError::DegenerateArea
                        | SphericalPolygonError::AmbiguousTriangulation { .. })
                ) {
                    errors.push(BoundaryError::UnresolvedRingArea { loop_index });
                }
            }
            match (ring.loop_type, ring.parent) {
                (LoopType::Hole, None) => errors.push(BoundaryError::OrphanHole { loop_index }),
                (LoopType::Hole, Some(parent)) => {
                    match self.loops.get(parent).map(|outer| outer.loop_type) {
                        Some(LoopType::Outer) => {}
                        Some(LoopType::Hole) => {
                            errors.push(BoundaryError::HoleInsideHole { loop_index, parent })
                        }
                        None => errors.push(BoundaryError::UnknownVertex {
                            loop_index,
                            vertex: parent,
                        }),
                    }
                }
                (LoopType::Outer, Some(_)) => {
                    errors.push(BoundaryError::OuterLoopWithParent { loop_index })
                }
                (LoopType::Outer, None) => {}
            }
        }
        for (loop_index, ring) in self.loops.iter().enumerate() {
            let (LoopType::Hole, Some(parent)) = (ring.loop_type, ring.parent) else {
                continue;
            };
            let Some(parent_ring) = self.loops.get(parent) else {
                continue;
            };
            if parent_ring.loop_type != LoopType::Outer
                || !self.ring_vertices_usable(ring)
                || !self.ring_vertices_usable(parent_ring)
            {
                continue;
            }
            if signed_area_on_unit_sphere(ring.vertices(), &self.vertices)
                .is_some_and(|area| area <= 0.0)
            {
                errors.push(BoundaryError::HoleWrongOrientation { loop_index });
            }
            for &vertex in ring.vertices() {
                let point = &self.vertices[vertex];
                if self.point_on_ring(parent_ring, point)
                    || !self.loop_winds_around(parent_ring, point.lon_degrees, point.lat_degrees)
                {
                    errors.push(BoundaryError::HoleOutsideParent {
                        loop_index,
                        parent,
                        vertex,
                    });
                    break;
                }
            }
            if self.rings_intersect(parent_ring, ring) {
                errors.push(BoundaryError::HoleIntersectsParent { loop_index, parent });
            }
        }
        let mut holes_by_parent: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (loop_index, ring) in self.loops.iter().enumerate() {
            if ring.loop_type == LoopType::Hole {
                if let Some(parent) = ring.parent {
                    if self
                        .loops
                        .get(parent)
                        .is_some_and(|parent_ring| parent_ring.loop_type == LoopType::Outer)
                        && self.ring_vertices_usable(ring)
                    {
                        holes_by_parent.entry(parent).or_default().push(loop_index);
                    }
                }
            }
        }
        for (parent, holes) in holes_by_parent {
            for first in 0..holes.len() {
                for second in first + 1..holes.len() {
                    let first_loop = holes[first];
                    let second_loop = holes[second];
                    let a = &self.loops[first_loop];
                    let b = &self.loops[second_loop];
                    if self.rings_intersect(a, b) {
                        errors.push(BoundaryError::SiblingHolesIntersect {
                            parent,
                            first_loop,
                            second_loop,
                        });
                        continue;
                    }
                    if self.ring_contains_ring(a, b) {
                        errors.push(BoundaryError::SiblingHoleNested {
                            parent,
                            outer_loop: first_loop,
                            inner_loop: second_loop,
                        });
                    } else if self.ring_contains_ring(b, a) {
                        errors.push(BoundaryError::SiblingHoleNested {
                            parent,
                            outer_loop: second_loop,
                            inner_loop: first_loop,
                        });
                    }
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn ring_vertices_usable(&self, ring: &BoundaryLoop) -> bool {
        ring.vertices().len() >= 3
            && ring.vertices().iter().all(|&index| {
                self.vertices
                    .get(index)
                    .is_some_and(|vertex| valid_lon_lat(vertex.lon_degrees, vertex.lat_degrees))
            })
    }

    fn point_on_ring(&self, ring: &BoundaryLoop, point: &BoundaryVertex) -> bool {
        let p = unit_from_vertex(point);
        let count = ring.vertices().len();
        for step in 0..count {
            let a = unit_from_vertex(&self.vertices[ring.vertices()[step]]);
            let b = unit_from_vertex(&self.vertices[ring.vertices()[(step + 1) % count]]);
            if point_on_minor_arc(a, b, p) {
                return true;
            }
        }
        false
    }

    fn ring_units(&self, ring: &BoundaryLoop) -> Vec<[f64; 3]> {
        ring.vertices()
            .iter()
            .map(|&vertex| unit_from_vertex(&self.vertices[vertex]))
            .collect()
    }

    /// The first pair of non-adjacent edges of `ring` that touch, as the full
    /// scan over every pair would find it (lowest first edge, then second).
    fn ring_self_intersection(&self, ring: &BoundaryLoop) -> Option<(usize, usize)> {
        let units = self.ring_units(ring);
        let count = units.len();
        let arcs: Vec<_> = (0..count)
            .map(|edge| (units[edge], units[(edge + 1) % count]))
            .collect();
        let mut first: Option<(usize, usize)> = None;
        sweep_overlapping_arcs(&arcs, |i, j| {
            let (first_edge, second_edge) = (i.min(j), i.max(j));
            if first_edge + 1 == second_edge || (first_edge == 0 && second_edge + 1 == count) {
                return false;
            }
            if first.is_some_and(|pair| pair <= (first_edge, second_edge)) {
                return false;
            }
            let (a0, a1) = arcs[first_edge];
            let (b0, b1) = arcs[second_edge];
            if spherical_segments_intersect(a0, a1, b0, b1) {
                first = Some((first_edge, second_edge));
            }
            false
        });
        first
    }

    fn rings_intersect(&self, a: &BoundaryLoop, b: &BoundaryLoop) -> bool {
        let (a_units, b_units) = (self.ring_units(a), self.ring_units(b));
        let a_count = a_units.len();
        let mut arcs: Vec<_> = (0..a_count)
            .map(|step| (a_units[step], a_units[(step + 1) % a_count]))
            .collect();
        arcs.extend(
            (0..b_units.len()).map(|step| (b_units[step], b_units[(step + 1) % b_units.len()])),
        );
        let mut found = false;
        sweep_overlapping_arcs(&arcs, |i, j| {
            let (i, j) = (i.min(j), i.max(j));
            if i < a_count && j >= a_count {
                let ((a0, a1), (b0, b1)) = (arcs[i], arcs[j]);
                found = spherical_segments_intersect(a0, a1, b0, b1);
            }
            found
        });
        found
    }

    fn ring_contains_ring(&self, outer: &BoundaryLoop, inner: &BoundaryLoop) -> bool {
        inner.vertices().iter().any(|&vertex| {
            let point = &self.vertices[vertex];
            self.loop_winds_around(outer, point.lon_degrees, point.lat_degrees)
        })
    }

    /// Shortest angular distance from a point to any boundary segment.
    ///
    /// The result is in radians on the unit sphere. Multiply by the mesh's
    /// sphere radius for a physical distance. `None` means the query or model
    /// has no usable boundary segment.
    pub fn distance_to_boundary_radians(&self, lon_degrees: f64, lat_degrees: f64) -> Option<f64> {
        if !valid_lon_lat(lon_degrees, lat_degrees) {
            return None;
        }
        let query = unit_from_vertex(&BoundaryVertex {
            lon_degrees,
            lat_degrees,
            pinned: false,
        });
        let mut best = f64::INFINITY;
        for ring in &self.loops {
            for edge in 0..ring.vertices().len() {
                let a = unit_from_vertex(self.vertices.get(ring.vertices()[edge])?);
                let b = unit_from_vertex(
                    self.vertices
                        .get(ring.vertices()[(edge + 1) % ring.vertices().len()])?,
                );
                best = best.min(angle(query, a)).min(angle(query, b));

                let normal = cross(a, b);
                let normal_length = norm(normal);
                if normal_length <= 1.0e-12 {
                    continue;
                }
                let normal = scale(normal, 1.0 / normal_length);
                let projected = [
                    query[0] - normal[0] * dot(query, normal),
                    query[1] - normal[1] * dot(query, normal),
                    query[2] - normal[2] * dot(query, normal),
                ];
                let projected_length = norm(projected);
                if projected_length <= 1.0e-12 {
                    continue;
                }
                let projected = scale(projected, 1.0 / projected_length);
                for candidate in [projected, scale(projected, -1.0)] {
                    if point_on_minor_arc(a, b, candidate) {
                        best = best.min(angle(query, candidate));
                    }
                }
            }
        }
        best.is_finite().then_some(best)
    }

    /// Whether a point is inside the domain this model describes.
    ///
    /// Inside an outer loop and not inside any of its holes. A lake in an
    /// island is outside; the sea around the island is outside; the land
    /// between them is inside. Getting that right is the whole reason holes are
    /// their own loops rather than joined to the outer ring by a cut.
    ///
    /// # Why this is here and not in a backend
    ///
    /// It answers "what is this boundary", which is this crate's subject. What
    /// a backend *does* with the answer -- refine inside it, refuse to cross
    /// it, split a segment that encroaches -- stays with the backend.
    ///
    /// # On the sphere, "inside" is a choice, and the ring's direction makes it
    ///
    /// A closed curve on a plane has an inside and an outside. On a sphere it
    /// has two sides and neither is smaller by nature -- the winding sum is
    /// `+2*pi` on one side and `-2*pi` on the other, so its *magnitude* calls
    /// both of them enclosed. Testing `abs(turn) > pi` therefore reports the
    /// far side of the globe as inside, which is what the dateline test caught.
    ///
    /// So the sign is what decides, and the convention is:
    ///
    /// **every ring runs counter-clockwise seen from outside the sphere, and
    /// the region it encloses is the one on its left.** An outer ring's left is
    /// the domain; a hole's left is the void inside it -- the lake, not the
    /// island around it. A ring given the other way round describes the
    /// complementary region, and does so deliberately rather than by accident.
    ///
    /// Winding is summed on the sphere rather than cast as a ray in longitude,
    /// so the dateline and the poles need no special case: a ring spanning 170
    /// east to 170 west is a twenty-degree strip, not almost the whole globe.
    pub fn contains(&self, lon_degrees: f64, lat_degrees: f64) -> bool {
        if !valid_lon_lat(lon_degrees, lat_degrees) {
            return false;
        }
        let mut inside = false;
        for (index, ring) in self.loops.iter().enumerate() {
            if ring.loop_type != LoopType::Outer
                || !self.loop_winds_around(ring, lon_degrees, lat_degrees)
            {
                continue;
            }
            let in_a_hole = self.loops.iter().any(|hole| {
                hole.loop_type == LoopType::Hole
                    && hole.parent == Some(index)
                    && self.loop_winds_around(hole, lon_degrees, lat_degrees)
            });
            if !in_a_hole {
                inside = true;
                break;
            }
        }
        inside
    }

    /// Conservative overlap evidence for a small spherical cell ring.
    ///
    /// This checks containment in both directions and every minor-arc edge,
    /// so a cell crossing or containing a domain boundary is not missed.
    /// Boundary-only contact does not count as positive-area target overlap.
    /// Callers must validate the boundary model once before querying it.
    pub fn polygon_overlap(
        &self,
        polygon: &[(f64, f64)],
    ) -> Result<PolygonOverlap, SphericalPolygonError> {
        let polygon_points = polygon
            .iter()
            .map(|&(lon, lat)| Point::new(lon, lat))
            .collect::<Vec<_>>();
        if try_spherical_polygon_area(&polygon_points)?.minor_sr <= 1.0e-14 {
            return Err(SphericalPolygonError::DegenerateArea);
        }
        let polygon_units = polygon
            .iter()
            .map(|&(lon, lat)| {
                unit_from_vertex(&BoundaryVertex {
                    lon_degrees: lon,
                    lat_degrees: lat,
                    pinned: false,
                })
            })
            .collect::<Vec<_>>();
        let strictly_inside_domain = |lon, lat| {
            self.contains(lon, lat)
                && self
                    .distance_to_boundary_radians(lon, lat)
                    .is_some_and(|distance| distance > 1.0e-10)
        };
        let mut overlap = PolygonOverlap::default();
        if polygon
            .iter()
            .any(|&(lon, lat)| strictly_inside_domain(lon, lat))
        {
            overlap.positive_area = true;
        }
        if polygon.iter().any(|&(lon, lat)| {
            self.distance_to_boundary_radians(lon, lat)
                .is_some_and(|distance| distance <= 1.0e-10)
        }) {
            overlap.intersects_boundary = true;
        }
        if self
            .loops
            .iter()
            .flat_map(|ring| ring.vertices())
            .filter_map(|&vertex| self.vertices.get(vertex))
            .any(|vertex| {
                let point = unit_from_vertex(vertex);
                spherical_ring_contains_minor(
                    polygon,
                    vertex.lon_degrees,
                    vertex.lat_degrees,
                    |p| *p,
                ) && !point_on_ring_boundary(point, &polygon_units)
            })
        {
            overlap.positive_area = true;
            overlap.intersects_boundary = true;
        }

        for polygon_edge in 0..polygon.len() {
            let a = polygon[polygon_edge];
            let b = polygon[(polygon_edge + 1) % polygon.len()];
            let a = unit_from_vertex(&BoundaryVertex {
                lon_degrees: a.0,
                lat_degrees: a.1,
                pinned: false,
            });
            let b = unit_from_vertex(&BoundaryVertex {
                lon_degrees: b.0,
                lat_degrees: b.1,
                pinned: false,
            });
            for ring in &self.loops {
                for edge in 0..ring.vertices().len() {
                    let Some(from) = self.vertices.get(ring.vertices()[edge]) else {
                        continue;
                    };
                    let Some(to) = self
                        .vertices
                        .get(ring.vertices()[(edge + 1) % ring.vertices().len()])
                    else {
                        continue;
                    };
                    if spherical_segments_cross_strictly(
                        a,
                        b,
                        unit_from_vertex(from),
                        unit_from_vertex(to),
                    ) {
                        overlap.positive_area = true;
                        overlap.intersects_boundary = true;
                    }
                }
            }
        }

        let sum = polygon_units.iter().fold([0.0; 3], |sum, point| {
            [sum[0] + point[0], sum[1] + point[1], sum[2] + point[2]]
        });
        let length = norm(sum);
        if length <= 1.0e-12 {
            return Ok(overlap);
        }
        let center = scale(sum, 1.0 / length);
        overlap.positive_area |= strictly_inside_domain(
            center[1].atan2(center[0]).to_degrees(),
            center[2].clamp(-1.0, 1.0).asin().to_degrees(),
        );
        overlap.intersects_boundary &= overlap.positive_area;
        Ok(overlap)
    }

    /// Whether a valid cell has positive-area target overlap.
    pub fn overlaps_polygon(&self, polygon: &[(f64, f64)]) -> bool {
        self.polygon_overlap(polygon)
            .is_ok_and(|overlap| overlap.positive_area)
    }

    /// Whether this ring encloses the point, by spherical winding.
    fn loop_winds_around(&self, ring: &BoundaryLoop, lon_degrees: f64, lat_degrees: f64) -> bool {
        let to_unit = |lon: f64, lat: f64| {
            let (lon, lat) = (lon.to_radians(), lat.to_radians());
            [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
        };
        let here = to_unit(lon_degrees, lat_degrees);
        // Each edge is projected into the plane tangent at the test point, and
        // the angles it turns through there are summed. A ring that encloses
        // the point turns a full circle; one that does not returns to zero.
        let tangent = |point: [f64; 3]| -> Option<(f64, f64)> {
            let dot = here[0] * point[0] + here[1] * point[1] + here[2] * point[2];
            let flat = [
                point[0] - here[0] * dot,
                point[1] - here[1] * dot,
                point[2] - here[2] * dot,
            ];
            let length = (flat[0] * flat[0] + flat[1] * flat[1] + flat[2] * flat[2]).sqrt();
            if length <= 1.0e-12 {
                // The test point sits on this vertex. Counting it as enclosed
                // is the choice that keeps a boundary vertex inside its own
                // domain rather than in neither.
                return None;
            }
            // Any fixed basis in the tangent plane does; the sum of turns is
            // independent of which.
            let east = [-here[1], here[0], 0.0];
            let east_length = (east[0] * east[0] + east[1] * east[1]).sqrt();
            let east = if east_length > 1.0e-12 {
                [east[0] / east_length, east[1] / east_length, 0.0]
            } else {
                // At a pole, east is undefined; any perpendicular will do.
                [1.0, 0.0, 0.0]
            };
            let north = [
                here[1] * east[2] - here[2] * east[1],
                here[2] * east[0] - here[0] * east[2],
                here[0] * east[1] - here[1] * east[0],
            ];
            Some((
                (flat[0] * east[0] + flat[1] * east[1] + flat[2] * east[2]) / length,
                (flat[0] * north[0] + flat[1] * north[1] + flat[2] * north[2]) / length,
            ))
        };

        let mut turned = 0.0_f64;
        let count = ring.vertices().len();
        for step in 0..count {
            let Some(&from) = ring.vertices().get(step) else {
                return false;
            };
            let Some(&to) = ring.vertices().get((step + 1) % count) else {
                return false;
            };
            let (Some(a), Some(b)) = (self.vertices.get(from), self.vertices.get(to)) else {
                return false;
            };
            let a_unit = to_unit(a.lon_degrees, a.lat_degrees);
            let b_unit = to_unit(b.lon_degrees, b.lat_degrees);
            let query_dot =
                |point: [f64; 3]| here[0] * point[0] + here[1] * point[1] + here[2] * point[2];
            if query_dot(a_unit) > 1.0 - 1.0e-12 || query_dot(b_unit) > 1.0 - 1.0e-12 {
                return true;
            }
            if query_dot(a_unit) < -1.0 + 1.0e-12 || query_dot(b_unit) < -1.0 + 1.0e-12 {
                return false;
            }
            let (Some(a), Some(b)) = (tangent(a_unit), tangent(b_unit)) else {
                return false;
            };
            let cross = a.0 * b.1 - a.1 * b.0;
            let dot = a.0 * b.0 + a.1 * b.1;
            turned += cross.atan2(dot);
        }
        // Signed, not absolute: see the convention on `contains`. The far side
        // of the ring sums to the negative of this and must not count.
        turned > std::f64::consts::PI
    }

    /// Every boundary edge, as ordered pairs of vertex indices.
    ///
    /// Rings close implicitly, so the last pair joins the final vertex to the
    /// first. A backend that has to place these on a mesh -- as Ruppert's
    /// segments, as edges no flip may remove -- starts here.
    pub fn segments(&self) -> Vec<(usize, usize)> {
        let mut segments = Vec::new();
        for ring in &self.loops {
            let count = ring.vertices().len();
            for step in 0..count {
                let (Some(&from), Some(&to)) = (
                    ring.vertices.get(step),
                    ring.vertices.get((step + 1) % count),
                ) else {
                    continue;
                };
                segments.push((from, to));
            }
        }
        segments
    }

    /// Outer loops and holes, counted.
    ///
    /// The pair a refinement has to leave unchanged. A run that ends with a
    /// different count has removed an island or closed a channel, whatever else
    /// it reports.
    pub fn topology_counts(&self) -> (usize, usize) {
        let holes = self
            .loops
            .iter()
            .filter(|ring| ring.loop_type == LoopType::Hole)
            .count();
        (self.loops.len() - holes, holes)
    }
}

#[cfg(test)]
mod tests;
