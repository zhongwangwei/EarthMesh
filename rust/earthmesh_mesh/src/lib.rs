//! Rust mesh kernels migrated from EarthMesh Fortran.

use std::{
    collections::{BTreeMap, BTreeSet},
    io,
};

use earthmesh_core::{deg_to_rad, rad_to_deg, GridMemory, IjTabs, ItabM, ItabW};

/// Earth-centered Cartesian point using the same axis convention as `mkgrd.F90`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl CartesianPoint {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Single-precision Earth-centered Cartesian point for `icosahedron.F90:de_ps`
/// and `ps_de` compatibility.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CartesianPointF32 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl CartesianPointF32 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

/// Longitude/latitude pair in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatDegrees {
    pub lon_degrees: f64,
    pub lat_degrees: f64,
}

impl LonLatDegrees {
    pub const fn new(lon_degrees: f64, lat_degrees: f64) -> Self {
        Self {
            lon_degrees,
            lat_degrees,
        }
    }
}

/// Port of `MOD_grid_preprocess:lonlat2xyz` for a single unit-sphere point.
///
/// The Fortran routine intentionally returns unit vectors; callers multiply by
/// `erad8` when Earth-radius-scaled coordinates are required.
pub fn lonlat_degrees_to_unit_xyz(lonlat: LonLatDegrees) -> CartesianPoint {
    let lon_rad = deg_to_rad(lonlat.lon_degrees);
    let lat_rad = deg_to_rad(lonlat.lat_degrees);
    CartesianPoint::new(
        lat_rad.cos() * lon_rad.cos(),
        lat_rad.cos() * lon_rad.sin(),
        lat_rad.sin(),
    )
}

/// Batch port of `MOD_grid_preprocess:lonlat2xyz`, preserving input order.
pub fn lonlat_points_to_unit_xyz(points: &[LonLatDegrees]) -> Vec<CartesianPoint> {
    points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect()
}

/// Convert Earth-centered Cartesian coordinates to lon/lat degrees.
///
/// This ports the scalar formula used by `mkgrd.F90:grid_xyz2lonlat`:
///
/// - `raxis = sqrt(x ** 2 + y ** 2)`
/// - `lat = atan2(z, raxis) * piu180`
/// - `lon = atan2(y, x) * piu180`
#[inline]
pub fn xyz_to_lonlat_degrees(point: CartesianPoint) -> LonLatDegrees {
    let raxis = point.x.hypot(point.y);
    LonLatDegrees {
        lon_degrees: rad_to_deg(point.y.atan2(point.x)),
        lat_degrees: rad_to_deg(point.z.atan2(raxis)),
    }
}

/// Convert a slice of Earth-centered Cartesian points to lon/lat degrees while
/// preserving point order.
pub fn xyz_points_to_lonlat_degrees(points: &[CartesianPoint]) -> Vec<LonLatDegrees> {
    points.iter().copied().map(xyz_to_lonlat_degrees).collect()
}

/// State-level port of `mkgrd.F90:grid_xyz2lonlat`.
///
/// The legacy routine allocates `GLONM/GLATM/GLONW/GLATW` for the full
/// one-based grid footprint and fills entries up to `nma` and `nwa`. The Rust
/// state keeps the same placeholder-inclusive layout using zero-based vectors.
pub fn grid_xyz2lonlat_state(grid: &mut GridMemory) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma)?;
    require_grid_coordinate_len("zem", grid.zem.len(), grid.nma)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa)?;
    require_grid_coordinate_len("zew", grid.zew.len(), grid.nwa)?;

    grid.allocate_grid_lonlatmw(grid.nma, grid.nva, grid.nwa);
    for im in 0..grid.nma {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xem[im]),
            f64::from(grid.yem[im]),
            f64::from(grid.zem[im]),
        ));
        grid.glonm[im] = lonlat.lon_degrees as f32;
        grid.glatm[im] = lonlat.lat_degrees as f32;
    }
    for iw in 0..grid.nwa {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xew[iw]),
            f64::from(grid.yew[iw]),
            f64::from(grid.zew[iw]),
        ));
        grid.glonw[iw] = lonlat.lon_degrees as f32;
        grid.glatw[iw] = lonlat.lat_degrees as f32;
    }
    Ok(())
}

/// State-level `grid_xyz2lonlat` for direct Fortran one-based arrays.
///
/// Index `0` is kept unused and records `1..=nma` / `1..=nwa` are filled.
pub fn grid_xyz2lonlat_fortran_indexed_state(grid: &mut GridMemory) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("zem", grid.zem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("zew", grid.zew.len(), grid.nwa + 1)?;

    grid.allocate_grid_lonlatmw(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for im in 1..=grid.nma {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xem[im]),
            f64::from(grid.yem[im]),
            f64::from(grid.zem[im]),
        ));
        grid.glonm[im] = lonlat.lon_degrees as f32;
        grid.glatm[im] = lonlat.lat_degrees as f32;
    }
    for iw in 1..=grid.nwa {
        let lonlat = xyz_to_lonlat_degrees(CartesianPoint::new(
            f64::from(grid.xew[iw]),
            f64::from(grid.yew[iw]),
            f64::from(grid.zew[iw]),
        ));
        grid.glonw[iw] = lonlat.lon_degrees as f32;
        grid.glatw[iw] = lonlat.lat_degrees as f32;
    }
    Ok(())
}

pub fn grid_cartesian_xy_to_lonlat_placeholders_fortran_indexed_state(
    grid: &mut GridMemory,
) -> io::Result<()> {
    require_grid_coordinate_len("xem", grid.xem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("yem", grid.yem.len(), grid.nma + 1)?;
    require_grid_coordinate_len("xew", grid.xew.len(), grid.nwa + 1)?;
    require_grid_coordinate_len("yew", grid.yew.len(), grid.nwa + 1)?;

    grid.allocate_grid_lonlatmw(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for im in 1..=grid.nma {
        grid.glonm[im] = grid.xem[im];
        grid.glatm[im] = grid.yem[im];
    }
    for iw in 1..=grid.nwa {
        grid.glonw[iw] = grid.xew[iw];
        grid.glatw[iw] = grid.yew[iw];
    }
    Ok(())
}

fn require_grid_coordinate_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required grid length {required}"),
        ));
    }
    Ok(())
}

/// Count metadata from `icosahedron.F90:icosahedron`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcosahedronCounts {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
}

/// Four corner coordinates for one of the ten OLAM/EarthMesh big diamonds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IcosahedronDiamondCorners {
    pub south: CartesianPoint,
    pub north: CartesianPoint,
    pub west: CartesianPoint,
    pub east: CartesianPoint,
}

/// Initial point-only state from `icosahedron.F90:icosahedron` before
/// `tri_neighbors` and `spring_dynamics1` mutate connectivity/coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct IcosahedronInitialGrid {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub impent: [usize; 12],
    pub diamond_corners: [IcosahedronDiamondCorners; 10],
    pub m_points: Vec<CartesianPoint>,
}

/// Integrated icosahedron grid state after connectivity derivation and optional
/// `spring_dynamics1` relaxation.
#[derive(Debug, Clone, PartialEq)]
pub struct IcosahedronRelaxedGrid {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub impent: [usize; 12],
    pub m_points: Vec<CartesianPoint>,
    pub connectivity: IcosahedronDiamondConnectivity,
    pub m_neighbors: Vec<IcosahedronMPointNeighbors>,
    pub spring: IcosahedronSpringDynamicsOutput,
}

/// One-based grid/connectivity state after `mkgrd.F90:voronoi`, before `pcvt`.
#[derive(Debug, Clone, PartialEq)]
pub struct VoronoiGridState {
    pub grid: GridMemory,
    pub tabs: IjTabs,
    pub impent: [usize; 12],
}

/// Port the global icosahedron branch of `mkgrd.F90:voronoi`.
///
/// The returned vectors intentionally keep Fortran-compatible one-based slots:
/// index `0` is unused, and valid records live in `1..=nma` and `1..=nwa`.
pub fn voronoi_grid_from_icosahedron_relaxed(
    relaxed: &IcosahedronRelaxedGrid,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    let mesh = OlamDelaunayMesh::from_relaxed_icosahedron(relaxed);
    voronoi_grid_from_olam_delaunay_mesh(&mesh, radius)
}

/// Convert a generic OLAM Delaunay mesh to the Voronoi grid state used by the
/// existing EarthMesh gridfile writers.
///
/// This is the OLAM replacement seam for `mkgrd.F90:voronoi`: callers should
/// produce or refine an [`OlamDelaunayMesh`], validate it, then call this
/// adapter at the output boundary.
pub fn voronoi_grid_from_olam_delaunay_mesh(
    mesh: &OlamDelaunayMesh,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    voronoi_grid_from_olam_delaunay_mesh_with_projection(mesh, radius, true)
}

pub fn voronoi_grid_from_olam_delaunay_mesh_cartesian(
    mesh: &OlamDelaunayMesh,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    voronoi_grid_from_olam_delaunay_mesh_with_projection(mesh, radius, false)
}

fn voronoi_grid_from_olam_delaunay_mesh_with_projection(
    mesh: &OlamDelaunayMesh,
    radius: f64,
    project_cell_centers_to_radius: bool,
) -> io::Result<VoronoiGridState> {
    mesh.validate_topology()?;

    let mut grid = GridMemory {
        nma: mesh.nwd,
        nua: mesh.nud,
        nva: mesh.nud,
        nwa: mesh.nmd,
        mma: mesh.nwd,
        mua: mesh.nud,
        mva: mesh.nud,
        mwa: mesh.nmd,
        ..GridMemory::default()
    };
    grid.allocate_xyzem(grid.nma + 1);
    grid.allocate_xyzew(grid.nwa + 1);

    for iw in 1..=grid.nwa {
        let point = mesh.m_points[iw];
        grid.xew[iw] = point.x as f32;
        grid.yew[iw] = point.y as f32;
        grid.zew[iw] = point.z as f32;
    }

    for im in 2..=grid.nma {
        let face = &mesh.w_faces[im];
        if face
            .im
            .iter()
            .any(|&idx| idx < 2 || idx >= mesh.m_points.len())
        {
            continue;
        }
        let p1 = mesh.m_points[face.im[0]];
        let p2 = mesh.m_points[face.im[1]];
        let p3 = mesh.m_points[face.im[2]];
        let barycenter = CartesianPoint::new(
            (p1.x + p2.x + p3.x) / 3.0,
            (p1.y + p2.y + p3.y) / 3.0,
            (p1.z + p2.z + p3.z) / 3.0,
        );
        let point = if project_cell_centers_to_radius {
            normalize_cartesian_to_radius(barycenter, radius)?
        } else {
            barycenter
        };
        grid.xem[im] = point.x as f32;
        grid.yem[im] = point.y as f32;
        grid.zem[im] = point.z as f32;
    }

    let mut tabs = IjTabs::allocate(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for iw in 2..=grid.nwa {
        let neighbor = &mesh.m_neighbors[iw];
        tabs.w[iw] = ItabW {
            iwp: iw as i32,
            iwglobe: iw as i32,
            npoly: neighbor.npoly as i32,
            im: neighbor.iw.map(|value| value as i32),
            iv: neighbor.iu.map(|value| value as i32),
            ..ItabW::default()
        };
    }
    for im in 2..=grid.nma {
        let face = &mesh.w_faces[im];
        tabs.m[im] = ItabM {
            imp: im as i32,
            imglobe: im as i32,
            npoly: face.npoly as i32,
            mrlm: face.mrlw as i32,
            mrlm_orig: face.mrlw_orig as i32,
            ngr: face.ngr as i32,
            iv: face.iu.map(|value| value as i32),
            iw: face.im.map(|value| value as i32),
            ..ItabM::default()
        };
    }

    Ok(VoronoiGridState {
        grid,
        tabs,
        impent: mesh.impent,
    })
}

/// Port of `mkgrd.F90:pcvt` for the one-based Voronoi grid state.
///
/// The input state is the direct output of `voronoi_grid_from_icosahedron_relaxed`:
/// M points are initialized as triangle barycenters and `tabs.m[im].iw[0..3]`
/// points to the three surrounding W vertices.  This routine mirrors the
/// Fortran loop over `im = 2, nma`: invalid placeholder triangles are skipped;
/// valid triangles are replaced by spherical circumcenters and normalized back
/// to the Earth radius by `spherical_circumcenter_from_barycenter`.
pub fn pcvt_adjust_voronoi_grid_state(state: &mut VoronoiGridState) -> io::Result<()> {
    require_grid_coordinate_len("xem", state.grid.xem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("yem", state.grid.yem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("zem", state.grid.zem.len(), state.grid.nma + 1)?;
    require_grid_coordinate_len("xew", state.grid.xew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("yew", state.grid.yew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("zew", state.grid.zew.len(), state.grid.nwa + 1)?;
    require_grid_coordinate_len("tabs.m", state.tabs.m.len(), state.grid.nma + 1)?;

    for im in 2..=state.grid.nma {
        let vertex_ids = state.tabs.m[im].iw;
        if vertex_ids.iter().any(|&iw| iw < 2) {
            continue;
        }
        let vertex_ids = [
            usize::try_from(vertex_ids[0])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
            usize::try_from(vertex_ids[1])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
            usize::try_from(vertex_ids[2])
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative W vertex id"))?,
        ];
        if vertex_ids.iter().any(|&iw| iw > state.grid.nwa) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("M point {im} references W vertex beyond nwa"),
            ));
        }

        let barycenter = CartesianPoint::new(
            f64::from(state.grid.xem[im]),
            f64::from(state.grid.yem[im]),
            f64::from(state.grid.zem[im]),
        );
        let vertices = vertex_ids.map(|iw| {
            CartesianPoint::new(
                f64::from(state.grid.xew[iw]),
                f64::from(state.grid.yew[iw]),
                f64::from(state.grid.zew[iw]),
            )
        });
        let circumcenter = spherical_circumcenter_from_barycenter(barycenter, vertices)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("M point {im} has degenerate spherical circumcenter"),
                )
            })?;
        state.grid.xem[im] = circumcenter.x as f32;
        state.grid.yem[im] = circumcenter.y as f32;
        state.grid.zem[im] = circumcenter.z as f32;
    }

    Ok(())
}

/// In-memory Rust orchestration for the global `mkgrd.F90:gridinit` mesh path.
///
/// This composes the migrated deterministic kernels without writing NetCDF:
/// `olam_gridinit_factorization_fortran` -> `OlamDelaunayMesh` expansion
/// -> `voronoi_grid_from_olam_delaunay_mesh`
/// -> `pcvt_adjust_voronoi_grid_state` -> `grid_xyz2lonlat_fortran_indexed_state`.
/// The returned state intentionally remains one-based so callers can pass it to
/// `earthmesh_cli::write_gridfile_from_fortran_indexed_state` at the I/O boundary.
pub fn gridinit_voronoi_state_fortran(
    nxp0: usize,
    nspring: usize,
    beta: f64,
    spring_relax: f64,
    max_tris: usize,
) -> io::Result<VoronoiGridState> {
    let factors = olam_gridinit_factorization_fortran(nxp0).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid OLAM gridinit NXP {nxp0}"),
        )
    })?;
    let mut mesh =
        OlamDelaunayMesh::from_icosahedron(factors.base_nxp, nspring, beta, spring_relax, max_tris)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "failed to build validated OLAM icosahedron grid",
                )
            })?;
    if factors.expansion_factor > 1 {
        mesh = mesh.expand_by_factor(factors.expansion_factor)?;
    }

    let mut state =
        voronoi_grid_from_olam_delaunay_mesh(&mesh, earthmesh_core::EARTH_RADIUS_METERS)?;
    pcvt_adjust_voronoi_grid_state(&mut state)?;
    grid_xyz2lonlat_fortran_indexed_state(&mut state.grid)?;
    Ok(state)
}

fn normalize_cartesian_to_radius(point: CartesianPoint, radius: f64) -> io::Result<CartesianPoint> {
    let norm = magnitude(point);
    if norm == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot normalize a zero-length Cartesian point",
        ));
    }
    let expansion = radius / norm;
    Ok(CartesianPoint::new(
        point.x * expansion,
        point.y * expansion,
        point.z * expansion,
    ))
}

/// Minimal Rust equivalent of the `itab_ud` fields written by
/// `icosahedron.F90:fill_diamond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcosahedronUEdge {
    pub im: [usize; 2],
    pub iw: [usize; 6],
    pub iu: [usize; 12],
    pub mrlu: usize,
}

impl Default for IcosahedronUEdge {
    fn default() -> Self {
        Self {
            im: [1, 1],
            iw: [1; 6],
            iu: [1; 12],
            mrlu: 0,
        }
    }
}

/// Minimal Rust equivalent of the `itab_wd` fields written by
/// `icosahedron.F90:fill_diamond`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcosahedronWFace {
    pub iu: [usize; 3],
    pub npoly: usize,
    pub im: [usize; 3],
    pub iw: [usize; 9],
    pub mrlw: usize,
    pub mrlw_orig: usize,
    pub ngr: usize,
    pub mrow: isize,
}

impl Default for IcosahedronWFace {
    fn default() -> Self {
        Self {
            iu: [1, 1, 1],
            npoly: 0,
            im: [1, 1, 1],
            iw: [1; 9],
            mrlw: 0,
            mrlw_orig: 0,
            ngr: 0,
            mrow: 0,
        }
    }
}

/// Minimal Rust equivalent of the `itab_md` neighbor fields completed by
/// `icosahedron.F90:tri_neighbors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcosahedronMPointNeighbors {
    pub npoly: usize,
    pub iu: [usize; 7],
    pub iw: [usize; 7],
}

impl Default for IcosahedronMPointNeighbors {
    fn default() -> Self {
        Self {
            npoly: 0,
            iu: [1; 7],
            iw: [1; 7],
        }
    }
}

/// Minimal Rust equivalent of OLAM `itab_md` refinement/grid metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IcosahedronMPointMetadata {
    pub mrlm: usize,
    pub mrlm_orig: usize,
    pub ngr: usize,
}

impl Default for IcosahedronMPointMetadata {
    fn default() -> Self {
        Self {
            mrlm: 1,
            mrlm_orig: 1,
            ngr: 1,
        }
    }
}

/// Precomputed topology tables built at the start of
/// `icosahedron.F90:spring_dynamics1`.
#[derive(Debug, Clone, PartialEq)]
pub struct IcosahedronSpringTopology {
    pub edge_m_points: Vec<[usize; 2]>,
    pub edge_neighbor_u: Vec<[usize; 4]>,
    pub m_npoly: Vec<usize>,
    pub m_u_edges: Vec<[usize; 7]>,
    pub directions: Vec<[f64; 7]>,
}

/// Output from one iteration of `icosahedron.F90:spring_dynamics1`.
#[derive(Debug, Clone, PartialEq)]
pub struct IcosahedronSpringIterationOutput {
    pub updated_m_points: Vec<CartesianPoint>,
    pub edge_displacements: Vec<CartesianPoint>,
    pub edge_distances: Vec<f64>,
}

/// Output from the multi-iteration `icosahedron.F90:spring_dynamics1` wrapper.
#[derive(Debug, Clone, PartialEq)]
pub struct IcosahedronSpringDynamicsOutput {
    pub updated_m_points: Vec<CartesianPoint>,
    pub last_edge_displacements: Vec<CartesianPoint>,
    pub diagnostic_max_displacements: Vec<SpringDiagnosticMaxDisplacement>,
}

/// Connectivity arrays after the `fill_diamond` calls inside
/// `icosahedron.F90:icosahedron`, before `tri_neighbors` fills the remaining
/// reciprocal neighbors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IcosahedronDiamondConnectivity {
    pub u_edges: Vec<IcosahedronUEdge>,
    pub w_faces: Vec<IcosahedronWFace>,
}

/// Generic OLAM-style Delaunay mesh state.
///
/// OLAM carries the triangular mesh as three reciprocal tables:
///
/// - M: Delaunay vertices / future Voronoi cell centers.
/// - U: Delaunay edges.
/// - W: Delaunay triangle faces / future Voronoi vertices.
///
/// This type is the replacement boundary for new grid construction work.  It
/// currently wraps the migrated icosahedron tables, but its validation rules are
/// intentionally generic so global expansion and `spawn_nest` can plug into the
/// same invariant checks instead of patching local connectivity by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct OlamDelaunayMesh {
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub impent: [usize; 12],
    pub m_points: Vec<CartesianPoint>,
    m_metadata: Vec<IcosahedronMPointMetadata>,
    pub u_edges: Vec<IcosahedronUEdge>,
    pub w_faces: Vec<IcosahedronWFace>,
    pub m_neighbors: Vec<IcosahedronMPointNeighbors>,
    pub m_prognostic: Vec<usize>,
    pub u_prognostic: Vec<usize>,
    pub w_prognostic: Vec<usize>,
    boundary_rows: Vec<usize>,
}

/// Summary returned after checking an [`OlamDelaunayMesh`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlamTopologyValidation {
    pub checked_m_points: usize,
    pub checked_u_edges: usize,
    pub checked_w_faces: usize,
}

/// Result of OLAM `gridinit:get_factors`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OlamGridinitFactors {
    pub base_nxp: usize,
    pub expansion_factor: usize,
}

/// User-facing specified-region refinement request for the OLAM mesh layer.
#[derive(Debug, Clone, PartialEq)]
pub enum OlamRefinementRegion {
    Circle {
        center: LonLatDegrees,
        radius_meters: f64,
        level: usize,
    },
    Bbox {
        west_degrees: f64,
        east_degrees: f64,
        south_degrees: f64,
        north_degrees: f64,
        level: usize,
    },
    Corridor {
        points: Vec<LonLatDegrees>,
        radius_meters: Vec<f64>,
        level: usize,
    },
    Polygon {
        points: Vec<LonLatDegrees>,
        level: usize,
    },
}

#[derive(Debug, Clone, Copy, Default)]
struct OlamMethodCNestUd {
    im: usize,
    iu: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct OlamMethodCNestWd {
    iu: [usize; 3],
    iw: [isize; 3],
}

impl OlamMethodCNestWd {
    fn flag(self) -> isize {
        self.iw[2]
    }

    fn is_subdivided(self) -> bool {
        self.flag() > 0
    }

    fn is_suppressed(self) -> bool {
        self.flag() < 0
    }

    fn child_iw(self, slot: usize) -> io::Result<usize> {
        let value = self.iw[slot];
        if value <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM Method-C child W slot {slot} is not allocated"),
            ));
        }
        Ok(value as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OlamMethodCPerimeterPoint {
    im: usize,
    iu: usize,
    npoly: usize,
    nwdiv: usize,
    near_pentagon: bool,
}

const OLAM_METHOD_C_MIN_GRID_SPACING_METERS: f64 = 0.001;

fn scale_olam_refinement_region_radius(
    region: &OlamRefinementRegion,
    factor: f64,
) -> Option<OlamRefinementRegion> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    match region {
        OlamRefinementRegion::Circle {
            center,
            radius_meters,
            level,
        } => Some(OlamRefinementRegion::Circle {
            center: *center,
            radius_meters: radius_meters * factor,
            level: *level,
        }),
        OlamRefinementRegion::Corridor {
            points,
            radius_meters,
            level,
        } => Some(OlamRefinementRegion::Corridor {
            points: points.clone(),
            radius_meters: radius_meters
                .iter()
                .map(|radius| radius * factor)
                .collect(),
            level: *level,
        }),
        OlamRefinementRegion::Bbox { .. } | OlamRefinementRegion::Polygon { .. } => None,
    }
}

impl OlamRefinementRegion {
    pub fn level(&self) -> usize {
        match self {
            Self::Circle { level, .. }
            | Self::Bbox { level, .. }
            | Self::Corridor { level, .. }
            | Self::Polygon { level, .. } => *level,
        }
    }

    pub fn validate(&self) -> io::Result<()> {
        let level = self.level();
        if !(1..=5).contains(&level) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("OLAM refinement level {level} must be in 1..=5"),
            ));
        }
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                validate_lonlat(*center)?;
                validate_olam_method_c_radius("circle radius", *radius_meters)?;
            }
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                if !west_degrees.is_finite()
                    || !east_degrees.is_finite()
                    || !south_degrees.is_finite()
                    || !north_degrees.is_finite()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bbox coordinates must be finite",
                    ));
                }
                if *south_degrees < -90.0
                    || *south_degrees > 90.0
                    || *north_degrees < -90.0
                    || *north_degrees > 90.0
                    || south_degrees > north_degrees
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bbox latitude bounds are invalid",
                    ));
                }
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires at least two points",
                    ));
                }
                if radius_meters.len() != points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires one radius per point",
                    ));
                }
                for &point in points {
                    validate_lonlat(point)?;
                }
                for &radius in radius_meters {
                    validate_olam_method_c_radius("corridor radius", radius)?;
                }
            }
            Self::Polygon { points, .. } => {
                if points.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "polygon refinement requires at least three points",
                    ));
                }
                for &point in points {
                    validate_lonlat(point)?;
                }
            }
        }
        Ok(())
    }

    pub fn validate_cartesian_xy(&self) -> io::Result<()> {
        let level = self.level();
        if !(1..=5).contains(&level) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("OLAM refinement level {level} must be in 1..=5"),
            ));
        }
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                if !center.lon_degrees.is_finite() || !center.lat_degrees.is_finite() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "circle Cartesian coordinates must be finite",
                    ));
                }
                validate_olam_method_c_radius("circle radius", *radius_meters)?;
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires at least two points",
                    ));
                }
                if radius_meters.len() != points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires one radius per point",
                    ));
                }
                for point in points {
                    if !point.lon_degrees.is_finite() || !point.lat_degrees.is_finite() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "corridor Cartesian coordinates must be finite",
                        ));
                    }
                }
                for &radius in radius_meters {
                    validate_olam_method_c_radius("corridor radius", radius)?;
                }
            }
            Self::Bbox { .. } | Self::Polygon { .. } => self.validate()?,
        }
        Ok(())
    }

    fn anchor_lonlat(&self) -> LonLatDegrees {
        match self {
            Self::Circle { center, .. } => *center,
            Self::Bbox {
                west_degrees,
                south_degrees,
                ..
            } => LonLatDegrees::new(*west_degrees, *south_degrees),
            Self::Corridor { points, .. } | Self::Polygon { points, .. } => points[0],
        }
    }

    fn contains_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => olam_ec_ps_distance_meters(point, *center, radius) < *radius_meters,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                let corners = [
                    LonLatDegrees::new(*west_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *north_degrees),
                    LonLatDegrees::new(*west_degrees, *north_degrees),
                ];
                olam_closed_corridor_contains_cartesian(
                    point,
                    &corners,
                    radius,
                    2_000_000.0,
                )
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return false;
                }
                points.windows(2).enumerate().any(|(idx, segment)| {
                    let (distance, t) =
                        olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius);
                    distance < olam_corridor_radius_at_segment(radius_meters, idx, t)
                })
            }
            Self::Polygon { points, .. } => olam_open_corridor_contains_cartesian(
                point,
                points,
                radius,
                2_000_000.0,
            ),
        }
    }

    fn close_to_cartesian(&self, point: CartesianPoint, radius: f64) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => olam_ec_ps_distance_meters(point, *center, radius) < radius_meters * 1.5,
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                let corners = [
                    LonLatDegrees::new(*west_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *south_degrees),
                    LonLatDegrees::new(*east_degrees, *north_degrees),
                    LonLatDegrees::new(*west_degrees, *north_degrees),
                ];
                olam_closed_corridor_contains_cartesian(
                    point,
                    &corners,
                    radius,
                    2_000_000.0 * 1.2,
                )
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) =
                    olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Polygon { points, .. } => olam_open_corridor_contains_cartesian(
                point,
                points,
                radius,
                2_000_000.0 * 1.2,
            ),
        }
    }

    fn contains_cartesian_xy(&self, point: CartesianPoint) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                let dx = point.x - center.lon_degrees;
                let dy = point.y - center.lat_degrees;
                dx.hypot(dy) < *radius_meters
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) = olam_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t)
            }),
            Self::Bbox { .. } | Self::Polygon { .. } => false,
        }
    }

    fn close_to_cartesian_xy(&self, point: CartesianPoint) -> bool {
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                let dx = point.x - center.lon_degrees;
                let dy = point.y - center.lat_degrees;
                dx.hypot(dy) < radius_meters * 1.5
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => points.windows(2).enumerate().any(|(idx, segment)| {
                let (distance, t) = olam_cartesian_xy_segment_distance(point, segment[0], segment[1]);
                distance < olam_corridor_radius_at_segment(radius_meters, idx, t) * 1.2
            }),
            Self::Bbox { .. } | Self::Polygon { .. } => false,
        }
    }
}

fn validate_olam_method_c_radius(name: &str, value: f64) -> io::Result<()> {
    validate_positive_distance(name, value)?;
    if value < OLAM_METHOD_C_MIN_GRID_SPACING_METERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} must be at least {OLAM_METHOD_C_MIN_GRID_SPACING_METERS} to match Fortran Method-C dzxmin"
            ),
        ));
    }
    Ok(())
}

fn olam_closed_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    }) || points
        .last()
        .zip(points.first())
        .is_some_and(|(&last, &first)| {
            olam_corridor_segment_distance_meters(point, last, first, radius).0
                < corridor_radius_meters
        })
}

fn olam_open_corridor_contains_cartesian(
    point: CartesianPoint,
    points: &[LonLatDegrees],
    radius: f64,
    corridor_radius_meters: f64,
) -> bool {
    if points.len() < 2 {
        return false;
    }
    points.windows(2).any(|segment| {
        olam_corridor_segment_distance_meters(point, segment[0], segment[1], radius).0
            < corridor_radius_meters
    })
}

fn olam_corridor_radius_at_segment(radius_meters: &[f64], idx: usize, t: f64) -> f64 {
    let start = radius_meters
        .get(idx)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(0.0);
    let end = radius_meters
        .get(idx + 1)
        .copied()
        .or_else(|| radius_meters.last().copied())
        .unwrap_or(start);
    (1.0 - t) * start + t * end
}

fn olam_cartesian_xy_segment_distance(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> (f64, f64) {
    plane_segment_distance(
        PlanePoint::new(point.x, point.y),
        PlanePoint::new(start.lon_degrees, start.lat_degrees),
        PlanePoint::new(end.lon_degrees, end.lat_degrees),
    )
}

fn olam_region_contains_method_c(
    region: &OlamRefinementRegion,
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    if use_cartesian_xy {
        region.contains_cartesian_xy(point)
    } else {
        region.contains_cartesian(point, radius)
    }
}

fn olam_regions_contain_method_c(
    regions: &[OlamRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| olam_region_contains_method_c(region, point, radius, use_cartesian_xy))
}

fn olam_region_close_to_method_c(
    region: &OlamRefinementRegion,
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    if use_cartesian_xy {
        region.close_to_cartesian_xy(point)
    } else {
        region.close_to_cartesian(point, radius)
    }
}

fn olam_regions_close_to_method_c(
    regions: &[OlamRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| olam_region_close_to_method_c(region, point, radius, use_cartesian_xy))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OlamTriangleSeed {
    im: [usize; 3],
    mrlw: usize,
    mrlw_orig: usize,
    ngr: usize,
    mrow: isize,
    target_iw: usize,
    target_iu: [usize; 3],
}
/// Port of `olam_grid.f90:get_factors`.
///
/// OLAM does not always build the initial icosahedron at the requested `NXP`.
/// It may choose a coarser base grid and later call `expand_delaunay_mesh`.
/// The selection rule tries 3-first and 2-first reductions down to
/// `nxpmin = 24`, then selects the largest candidate below `46` when more than
/// one such candidate exists; otherwise it selects the minimum candidate.
pub fn olam_gridinit_factorization_fortran(nxp: usize) -> Option<OlamGridinitFactors> {
    if nxp == 0 {
        return None;
    }

    const NXP_MIN: usize = 24;
    let mut candidates = [OlamGridinitFactors {
        base_nxp: nxp,
        expansion_factor: 1,
    }; 4];

    reduce_gridinit_candidate(&mut candidates[0], 3, NXP_MIN);
    reduce_gridinit_candidate(&mut candidates[0], 2, NXP_MIN);

    reduce_gridinit_candidate(&mut candidates[1], 2, NXP_MIN);
    reduce_gridinit_candidate(&mut candidates[1], 3, NXP_MIN);

    let threshold = (NXP_MIN - 1) * 2;
    let under_threshold = candidates
        .iter()
        .filter(|candidate| candidate.base_nxp < threshold)
        .count();

    let mut selected = candidates[0];
    if under_threshold > 1 {
        for candidate in candidates.iter().copied().skip(1) {
            if candidate.base_nxp < threshold && candidate.base_nxp > selected.base_nxp {
                selected = candidate;
            }
        }
    } else {
        for candidate in candidates.iter().copied().skip(1) {
            if candidate.base_nxp < selected.base_nxp {
                selected = candidate;
            }
        }
    }

    Some(selected)
}

fn reduce_gridinit_candidate(candidate: &mut OlamGridinitFactors, factor: usize, nxp_min: usize) {
    while candidate.base_nxp % factor == 0 && candidate.base_nxp / factor >= nxp_min {
        candidate.base_nxp /= factor;
        candidate.expansion_factor *= factor;
    }
}

impl OlamDelaunayMesh {
    /// Surface (non-atmosphere) perimeter-row expansion width used by
    /// OLAM Method-C `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_SURFACE: usize = 7;

    /// Atmosphere perimeter-row expansion width used by OLAM Method-C
    /// `perim_mrow`.
    pub const METHOD_C_MAX_MROWS_ATMOS: usize = 13;

    /// Build the generic OLAM Delaunay mesh wrapper from an already-relaxed
    /// global icosahedron.
    pub fn from_relaxed_icosahedron(relaxed: &IcosahedronRelaxedGrid) -> Self {
        Self {
            nmd: relaxed.nmd,
            nud: relaxed.nud,
            nwd: relaxed.nwd,
            impent: relaxed.impent,
            m_points: relaxed.m_points.clone(),
            m_metadata: default_olam_m_metadata(relaxed.nmd),
            u_edges: relaxed.connectivity.u_edges.clone(),
            w_faces: relaxed.connectivity.w_faces.clone(),
            m_neighbors: relaxed.m_neighbors.clone(),
            m_prognostic: olam_identity_prognostic_map(relaxed.nmd),
            u_prognostic: olam_identity_prognostic_map(relaxed.nud),
            w_prognostic: olam_identity_prognostic_map(relaxed.nwd),
            boundary_rows: Vec::new(),
        }
    }

    /// Build OLAM's local Cartesian hexagonal base grid used by
    /// `cart_hex.F90:cart_hex` for `MDOMAIN = 5`.
    pub fn from_cart_hex(nxp: usize, deltax_meters: f64) -> io::Result<Self> {
        if !deltax_meters.is_finite() || deltax_meters < OLAM_METHOD_C_MIN_GRID_SPACING_METERS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "OLAM cart_hex DELTAX must be at least {OLAM_METHOD_C_MIN_GRID_SPACING_METERS} meters"
                ),
            ));
        }

        let mut nmd = 1;
        let mut nud = 1;
        let mut nwd = 1;
        let tab_width = nxp + 2;
        let tab_plane = tab_width * tab_width;
        let tab_len = 4 * tab_plane;
        let tab_idx = |i: usize, j: usize, ir: usize| ir * tab_plane + j * tab_width + i;
        let mut jm1 = vec![1usize; tab_len];
        let mut ju1 = vec![1usize; tab_len];
        let mut ju2 = vec![1usize; tab_len];
        let mut ju3 = vec![1usize; tab_len];
        let mut jw1 = vec![1usize; tab_len];
        let mut jw2 = vec![1usize; tab_len];

        for ir in 1..=3 {
            for j in 1..=nxp {
                for i in 1..=nxp + 1 {
                    let idx = tab_idx(i, j, ir);
                    jm1[idx] = nmd + 1;
                    ju1[idx] = nud + 1;
                    ju2[idx] = nud + 2;
                    ju3[idx] = nud + 3;
                    jw1[idx] = nwd + 1;
                    jw2[idx] = nwd + 2;
                    nmd += 1;
                    nud += 3;
                    nwd += 2;
                }
                jw1[tab_idx(0, j, ir)] = nwd + 1;
                nwd += 1;
            }

            for i in 1..=nxp + 1 {
                let idx = tab_idx(i, nxp + 1, ir);
                jm1[idx] = nmd + 1;
                ju1[idx] = nud + 1;
                jw2[tab_idx(i, 0, ir)] = nwd + 1;
                nmd += 1;
                nud += 1;
                nwd += 1;
            }
        }
        let jw0 = nwd + 1;
        nwd += 1;

        let zero = CartesianPoint::new(0.0, 0.0, 0.0);
        let mut m_points = vec![zero; nmd + 1];
        let mut u_edges = vec![IcosahedronUEdge::default(); nud + 1];
        let mut w_faces = vec![IcosahedronWFace::default(); nwd + 1];
        let mut m_prognostic = olam_identity_prognostic_map(nmd);
        let mut u_prognostic = olam_identity_prognostic_map(nud);
        let mut w_prognostic = olam_identity_prognostic_map(nwd);
        for face in w_faces.iter_mut().take(nwd + 1).skip(2) {
            face.mrlw = 1;
            face.mrlw_orig = 1;
            face.ngr = 1;
        }

        let unit_dist = (4.0_f64 / 3.0).sqrt().sqrt() * deltax_meters;
        let xstart = -((nxp + 1) as f64) * 0.5 * unit_dist;
        let ystart = -((nxp as f64) + 1.0 / 3.0) * 0.5 * 3.0_f64.sqrt() * unit_dist;

        for ir in 1..=3 {
            let irm = if ir == 1 { 3 } else { ir - 1 };
            let irp = if ir == 3 { 1 } else { ir + 1 };
            let (rxx, rxy, ryx, ryy) = match ir {
                1 => (1.0, 0.0, 0.0, 1.0),
                2 => (-0.5, -0.5 * 3.0_f64.sqrt(), 0.5 * 3.0_f64.sqrt(), -0.5),
                _ => (-0.5, 0.5 * 3.0_f64.sqrt(), -0.5 * 3.0_f64.sqrt(), -0.5),
            };

            for j in 1..=nxp {
                for i in 1..=nxp + 1 {
                    let idx = tab_idx(i, j, ir);
                    let im1 = jm1[idx];
                    let xm = xstart + ((i - 1) as f64 - 0.5 * (j - 1) as f64) * unit_dist;
                    let ym = ystart + (j - 1) as f64 * 0.5 * 3.0_f64.sqrt() * unit_dist;
                    m_points[im1] =
                        CartesianPoint::new(rxx * xm + rxy * ym, ryx * xm + ryy * ym, 0.0);

                    let iu1 = ju1[idx];
                    let iu2 = ju2[idx];
                    let iu3 = ju3[idx];
                    let iw1 = jw1[idx];
                    let iw2 = jw2[idx];
                    let iw3 = jw2[tab_idx(i, j - 1, ir)];
                    let iw4 = jw1[tab_idx(i - 1, j, ir)];
                    let im3 = jm1[tab_idx(i, j + 1, ir)];
                    let iu5 = ju1[tab_idx(i, j + 1, ir)];

                    let (im2, im4, iu4) = if i <= nxp {
                        (
                            jm1[tab_idx(i + 1, j, ir)],
                            jm1[tab_idx(i + 1, j + 1, ir)],
                            ju3[tab_idx(i + 1, j, ir)],
                        )
                    } else {
                        (
                            jm1[tab_idx(j, nxp + 1, irp)],
                            jm1[tab_idx(j + 1, nxp + 1, irp)],
                            ju1[tab_idx(j, nxp + 1, irp)],
                        )
                    };

                    u_edges[iu1] = if ir == 1 {
                        IcosahedronUEdge {
                            im: [im1, im2],
                            iw: set_first_two([1; 6], iw3, iw1),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im2, im1],
                            iw: set_first_two([1; 6], iw1, iw3),
                            ..IcosahedronUEdge::default()
                        }
                    };
                    u_edges[iu2] = if ir == 1 || ir == 3 {
                        IcosahedronUEdge {
                            im: [im1, im4],
                            iw: set_first_two([1; 6], iw1, iw2),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im4, im1],
                            iw: set_first_two([1; 6], iw2, iw1),
                            ..IcosahedronUEdge::default()
                        }
                    };
                    u_edges[iu3] = if ir == 3 {
                        IcosahedronUEdge {
                            im: [im1, im3],
                            iw: set_first_two([1; 6], iw2, iw4),
                            ..IcosahedronUEdge::default()
                        }
                    } else {
                        IcosahedronUEdge {
                            im: [im3, im1],
                            iw: set_first_two([1; 6], iw4, iw2),
                            ..IcosahedronUEdge::default()
                        }
                    };

                    w_faces[iw1].npoly = 3;
                    w_faces[iw1].iu = [iu1, iu4, iu2];
                    w_faces[iw1].im = [im1, im2, im4];
                    w_faces[iw2].npoly = 3;
                    w_faces[iw2].iu = [iu2, iu5, iu3];
                    w_faces[iw2].im = [im1, im4, im3];

                    if i == 1 && j == 1 {
                        w_faces[iw3].iu[0] = iu1;
                        w_faces[iw4].iu[0] = iu3;
                        m_prognostic[im1] = jm1[tab_idx(2, 2, irp)];
                        u_prognostic[iu1] = if ir == 2 {
                            ju3[tab_idx(2, 1, irm)]
                        } else {
                            ju2[tab_idx(1, 1, irp)]
                        };
                        if ir == 3 {
                            u_prognostic[iu2] = ju3[tab_idx(2, 1, irp)];
                        }
                        u_prognostic[iu3] = ju1[tab_idx(2, 2, irp)];
                        w_prognostic[iw3] = jw2[tab_idx(2, 1, irm)];
                        w_prognostic[iw4] = jw1[tab_idx(2, 2, irp)];
                    } else if i == 1 {
                        w_faces[iw4].iu[0] = iu3;
                        m_prognostic[im1] = jm1[tab_idx(j + 1, 2, irp)];
                        if ir != 2 {
                            u_prognostic[iu1] = ju2[tab_idx(j, 1, irp)];
                        }
                        if ir == 3 {
                            u_prognostic[iu2] = ju3[tab_idx(j + 1, 1, irp)];
                        }
                        u_prognostic[iu3] = ju1[tab_idx(j + 1, 2, irp)];
                        w_prognostic[iw4] = jw1[tab_idx(j + 1, 2, irp)];
                    } else if j == 1 {
                        w_faces[iw3].iu[0] = iu1;
                        m_prognostic[im1] = jm1[tab_idx(2, i, irm)];
                        u_prognostic[iu1] = if i == nxp + 1 && ir == 2 {
                            ju1[tab_idx(1, nxp + 1, ir)]
                        } else if i == nxp + 1 {
                            ju2[tab_idx(nxp, 1, irp)]
                        } else {
                            ju3[tab_idx(2, i, irm)]
                        };
                        if ir == 3 {
                            u_prognostic[iu2] = ju1[tab_idx(1, i, irm)];
                        }
                        if ir != 1 {
                            u_prognostic[iu3] = ju2[tab_idx(1, i - 1, irm)];
                        }
                        w_prognostic[iw3] = if i == nxp + 1 {
                            jw2[tab_idx(nxp + 1, 1, irp)]
                        } else {
                            jw2[tab_idx(2, i, irm)]
                        };
                    }
                }
            }

            for i in 1..=nxp + 1 {
                let idx = tab_idx(i, nxp + 1, ir);
                let im1 = jm1[idx];
                let iu1 = ju1[idx];
                let iw3 = jw2[tab_idx(i, nxp, ir)];
                let xm = xstart + ((i - 1) as f64 - 0.5 * nxp as f64) * unit_dist;
                let ym = ystart + nxp as f64 * 0.5 * 3.0_f64.sqrt() * unit_dist;
                m_points[im1] =
                    CartesianPoint::new(rxx * xm + rxy * ym, ryx * xm + ryy * ym, 0.0);

                let (im2, iw1) = if i <= nxp {
                    (jm1[tab_idx(i + 1, nxp + 1, ir)], jw1[tab_idx(nxp + 1, i, irm)])
                } else {
                    w_faces[jw0].iu[ir - 1] = iu1;
                    (jm1[tab_idx(i, nxp + 1, irp)], jw0)
                };
                u_edges[iu1] = if ir == 1 {
                    IcosahedronUEdge {
                        im: [im1, im2],
                        iw: set_first_two([1; 6], iw3, iw1),
                        ..IcosahedronUEdge::default()
                    }
                } else {
                    IcosahedronUEdge {
                        im: [im2, im1],
                        iw: set_first_two([1; 6], iw1, iw3),
                        ..IcosahedronUEdge::default()
                    }
                };
                if i == 1 {
                    m_prognostic[im1] = jm1[tab_idx(2, nxp + 1, irm)];
                    if ir != 2 {
                        u_prognostic[iu1] = ju2[tab_idx(nxp + 1, 1, irp)];
                    }
                }
            }
        }

        for edge in u_edges.iter_mut().take(nud + 1).skip(2) {
            edge.mrlu = 1;
        }

        let jw0_edges = w_faces[jw0].iu;
        let mut jw0_m = Vec::<usize>::new();
        for &iu in &jw0_edges {
            if iu <= 1 {
                continue;
            }
            for &im in &u_edges[iu].im {
                if im > 1 && !jw0_m.contains(&im) {
                    jw0_m.push(im);
                }
            }
        }
        if jw0_m.len() == 3 {
            w_faces[jw0].npoly = 3;
            w_faces[jw0].im = [jw0_m[0], jw0_m[1], jw0_m[2]];
        }
        fill_cart_hex_w_face_neighbors_from_edges(&u_edges, &mut w_faces, &w_prognostic)?;
        let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
        derive_icosahedron_u_neighbors_fortran(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive OLAM cart_hex U-edge neighbors",
            )
        })?;
        let IcosahedronDiamondConnectivity { u_edges, w_faces } = connectivity;

        let m_neighbors =
            derive_cart_hex_m_neighbors_from_active_faces(nmd, &u_edges, &w_faces, &w_prognostic)?;

        Ok(Self {
            nmd,
            nud,
            nwd,
            impent: [1; 12],
            m_points,
            m_metadata: default_olam_m_metadata(nmd),
            u_edges,
            w_faces,
            m_neighbors,
            m_prognostic,
            u_prognostic,
            w_prognostic,
            boundary_rows: Vec::new(),
        })
    }

    /// Build a validated OLAM Delaunay mesh from the migrated global
    /// icosahedron path.
    pub fn from_icosahedron(
        nxp0: usize,
        niter: usize,
        beta: f64,
        relax: f64,
        _diagnostic_every: usize,
    ) -> Option<Self> {
        let initial = icosahedron_initial_grid_fortran(nxp0)?;
        let mut connectivity = icosahedron_fill_diamonds_fortran(nxp0)?;
        let m_neighbors = derive_icosahedron_tri_neighbors_fortran(initial.nmd, &mut connectivity)?;
        let mesh = Self {
            nmd: initial.nmd,
            nud: initial.nud,
            nwd: initial.nwd,
            impent: initial.impent,
            m_points: initial.m_points,
            m_metadata: default_olam_m_metadata(initial.nmd),
            u_edges: connectivity.u_edges,
            w_faces: connectivity.w_faces,
            m_neighbors,
            m_prognostic: olam_identity_prognostic_map(initial.nmd),
            u_prognostic: olam_identity_prognostic_map(initial.nud),
            w_prognostic: olam_identity_prognostic_map(initial.nwd),
            boundary_rows: Vec::new(),
        };
        mesh.validate_topology().ok()?;
        if niter == 0 {
            Some(mesh)
        } else {
            mesh.spring_global_with_controls(nxp0, niter, beta, relax)
                .ok()
        }
    }

    /// Rebuild an OLAM Delaunay mesh from the compact EarthMesh gridfile
    /// tables written at the Voronoi output boundary.
    ///
    /// In that schema, `GLONW/GLATW` rows are the OLAM Delaunay M points and
    /// `itab_m%iw` rows are the OLAM W-face M-point triplets. Row `0`
    /// corresponds to Fortran/OLAM id `1`; active records start at id `2`.
    pub fn from_voronoi_gridfile_tables(
        m_point_lonlat: &[LonLatDegrees],
        w_face_m_points: &[[usize; 3]],
        m_face_counts: &[usize],
    ) -> io::Result<Self> {
        let nmd = m_point_lonlat.len();
        let nwd = w_face_m_points.len();
        require_olam_len("OLAM gridfile M point valences", m_face_counts.len(), nmd)?;
        if nmd < 2 || nwd < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM gridfile tables must include placeholder row 1 and at least one active row",
            ));
        }

        let radius = earthmesh_core::EARTH_RADIUS_METERS;
        let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); nmd + 1];
        for (row, &lonlat) in m_point_lonlat.iter().enumerate() {
            let id = row + 1;
            let unit = lonlat_degrees_to_unit_xyz(lonlat);
            m_points[id] = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);
        }

        let pentagons = m_face_counts
            .iter()
            .enumerate()
            .filter_map(|(row, &count)| (row > 0 && count == 5).then_some(row + 1))
            .collect::<Vec<_>>();
        if pentagons.len() != 12 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "OLAM gridfile source must expose 12 pentagonal M points, found {}",
                    pentagons.len()
                ),
            ));
        }
        let mut impent = [1usize; 12];
        impent.copy_from_slice(&pentagons);

        let face_seeds = w_face_m_points
            .iter()
            .enumerate()
            .filter_map(|(row, &im)| {
                let iw = row + 1;
                (iw > 1).then_some(
                    OlamTriangleSeed::new(im, (1, 1, 1))
                        .with_target_iw(iw)
                        .with_mrow(0),
                )
            })
            .collect::<Vec<_>>();

        match olam_mesh_from_triangle_seeds(nmd, impent, m_points.clone(), &face_seeds) {
            Ok(mesh) => Ok(mesh),
            Err(forward_err) => {
                let reversed = face_seeds
                    .iter()
                    .map(|seed| {
                        OlamTriangleSeed::new(
                            [seed.im[0], seed.im[2], seed.im[1]],
                            (seed.mrlw, seed.mrlw_orig, seed.ngr),
                        )
                        .with_mrow(seed.mrow)
                        .with_target_iw(seed.target_iw)
                    })
                    .collect::<Vec<_>>();
                olam_mesh_from_triangle_seeds(nmd, impent, m_points, &reversed).map_err(
                    |reverse_err| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "failed to rebuild OLAM mesh from gridfile tables; forward orientation: {forward_err}; reversed orientation: {reverse_err}"
                            ),
                        )
                    },
                )
            }
        }
    }

    /// Apply OLAM `spring_dynamics_globe` to the active Delaunay M points.
    ///
    /// OLAM's global spring is a Delaunay-edge relaxation pass: U-edge lengths
    /// are pushed toward `beta * 2*pi*R / (5*nxp) / 1.2`, the target is adjusted
    /// by the two opposite triangle angles, all M points are projected back to
    /// the sphere, and the twelve original pentagon points (`impent`) are kept
    /// fixed.
    pub fn spring_global(&self, nxp: usize, niter: usize) -> io::Result<Self> {
        self.spring_global_with_controls(nxp, niter, 1.25, 0.035)
    }

    /// Same as [`Self::spring_global`], but exposes OLAM's two scalar controls
    /// so callers that still carry namelist values can opt into them explicitly.
    pub fn spring_global_with_controls(
        &self,
        nxp: usize,
        niter: usize,
        beta: f64,
        relax: f64,
    ) -> io::Result<Self> {
        self.spring_global_with_dist00_and_projection(nxp, niter, beta, relax, None, true)
    }

    /// Apply OLAM `spring_dynamics_globe` for Cartesian/regional native
    /// coordinates (`mdomain >= 2`): target spacing comes from `deltax`, and M
    /// points are not projected back to Earth radius.
    pub fn spring_global_cartesian_with_controls(
        &self,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
        relax: f64,
    ) -> io::Result<Self> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Cartesian global spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spring_global_with_dist00_and_projection(
            nxp,
            niter,
            1.0,
            relax,
            Some(cartesian_dist00),
            false,
        )
    }

    fn spring_global_with_dist00_and_projection(
        &self,
        nxp: usize,
        niter: usize,
        beta: f64,
        relax: f64,
        dist00_override: Option<f64>,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        if nxp == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring requires positive NXP",
            ));
        }
        if !beta.is_finite() || beta <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring beta must be positive and finite",
            ));
        }
        if !relax.is_finite() || relax <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM global spring relax must be positive and finite",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let radius = active_mesh_radius(self)?;
        let topology =
            icosahedron_spring_topology_fortran(self.nmd, &self.u_edges, &self.m_neighbors, relax)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to build OLAM global spring topology",
                    )
                })?;
        let dist00 = dist00_override.unwrap_or(olam_fortran_global_dist00(beta, radius, nxp));
        let mut m_points = self.m_points.clone();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 20 == 0)
                && !earthmesh_core::progress::report("spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM global spring was cancelled",
                ));
            }
            m_points =
                olam_global_spring_iteration(
                    &m_points,
                    &topology,
                    &self.impent,
                    dist00,
                    if project_to_radius { Some(radius) } else { None },
                )
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "failed to run OLAM global spring iteration",
                        )
                    })?;
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok(adjusted)
    }

    /// Apply the core OLAM `spring_dynamics_nest` relaxation to a refined nest.
    ///
    /// With `move_interior=false` this mirrors OLAM's atmospheric nest call:
    /// only M points adjacent to transition-row faces with nonzero `mrow` move. With
    /// `move_interior=true`, M points adjacent to faces on `ngr` are also moved.
    pub fn spring_nest(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
    ) -> io::Result<Self> {
        self.spring_nest_with_radius_projection(nxp, niter, ngr, move_interior, true, None)
    }

    fn spring_nest_with_radius_projection(
        &self,
        nxp: usize,
        niter: usize,
        ngr: usize,
        move_interior: bool,
        project_to_radius: bool,
        dist00_override: Option<f64>,
    ) -> io::Result<Self> {
        if nxp == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM nest spring requires positive NXP",
            ));
        }
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM nest spring NGR must be greater than one",
            ));
        }

        self.validate_topology()?;
        if niter == 0 {
            return Ok(self.clone());
        }

        let movable_m_points = olam_nest_movable_m_points(self, ngr, move_interior)?;
        if movable_m_points.iter().skip(2).all(|movable| !*movable) {
            return Ok(self.clone());
        }

        let radius = active_mesh_radius(self)?;
        let topology =
            icosahedron_spring_topology_fortran(self.nmd, &self.u_edges, &self.m_neighbors, 0.035)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to build OLAM nest spring topology",
                    )
                })?;
        let dist00 = dist00_override.unwrap_or(olam_fortran_global_dist00(1.0, radius, nxp));
        let mut m_points = self.m_points.clone();

        for iteration in 1..=niter {
            if (iteration == 1 || iteration == niter || iteration % 100 == 0)
                && !earthmesh_core::progress::report("olam-nest-spring", iteration, niter)
            {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "OLAM nest spring was cancelled",
                ));
            }
            m_points =
                olam_nest_spring_iteration(
                    &m_points,
                    self,
                    &topology,
                    &movable_m_points,
                    dist00,
                    project_to_radius,
                )
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "failed to run OLAM nest spring iteration",
                    )
                })?;
        }

        for point in m_points.iter_mut().skip(2) {
            point.x = point.x as f32 as f64;
            point.y = point.y as f32 as f64;
            point.z = point.z as f32 as f64;
        }

        let adjusted = Self {
            nmd: self.nmd,
            nud: self.nud,
            nwd: self.nwd,
            impent: self.impent,
            m_points,
            m_metadata: self.m_metadata.clone(),
            u_edges: self.u_edges.clone(),
            w_faces: self.w_faces.clone(),
            m_neighbors: self.m_neighbors.clone(),
            m_prognostic: self.m_prognostic.clone(),
            u_prognostic: self.u_prognostic.clone(),
            w_prognostic: self.w_prognostic.clone(),
            boundary_rows: self.boundary_rows.clone(),
        };
        adjusted.validate_topology()?;
        Ok(adjusted)
    }

    /// Final W-face ids that were generated as transition rows by the most
    /// recent specified-region refinement pass.
    pub fn boundary_rows(&self) -> &[usize] {
        &self.boundary_rows
    }

    /// One-based OLAM `itab_md` refinement/grid metadata.
    pub fn m_point_metadata(&self) -> &[IcosahedronMPointMetadata] {
        &self.m_metadata
    }

    /// Spawn specified OLAM refinement regions with independent per-region
    /// levels using OLAM Method-C. Each pass follows the legacy perimeter
    /// grouping and transition-patch table updates instead of a generic local
    /// triangulation.
    ///
    /// This defaults to surface-style Method-C transition width (`max_mrows = 7`)
    /// and is therefore intended for non-atmosphere meshes unless callers pass
    /// an explicit width.
    pub fn spawn_nest(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_with_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_SURFACE,
        )
    }

    /// Spawn OLAM Method-C refinement with atmosphere-style transition width
    /// (`max_mrows = 13`).
    pub fn spawn_nest_as_atmosmesh(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_with_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_ATMOS,
        )
    }

    /// Spawn OLAM Method-C refinement with surface-style transition width
    /// (`max_mrows = 7`).
    pub fn spawn_nest_as_surface(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
    ) -> io::Result<Self> {
        self.spawn_nest(regions, max_level)
    }

    /// Canonical text dump of the migrated OLAM Delaunay M/U/W topology tables.
    ///
    /// This is intentionally exhaustive for fields owned by `earthmesh_mesh` and
    /// stable across platforms, so external Fortran parity harnesses can compare
    /// full table contents without carrying large golden fixture files.
    pub fn olam_delaunay_topology_dump(&self) -> String {
        let mut dump = String::new();
        dump.push_str(&format!(
            "counts nmd={} nud={} nwd={}\n",
            self.nmd, self.nud, self.nwd
        ));
        for im in 2..=self.nmd {
            let neighbors = self.m_neighbors[im];
            let metadata = self.m_metadata[im];
            let stored_m_neighbors = [1usize; 7];
            dump.push_str(&format!(
                "M {im} npoly={} mrlm={} mrlm_orig={} ngr={}",
                neighbors.npoly,
                metadata.mrlm,
                metadata.mrlm_orig,
                metadata.ngr
            ));
            push_usize_fields(&mut dump, " im", &stored_m_neighbors);
            push_usize_fields(&mut dump, " iu", &neighbors.iu);
            push_usize_fields(&mut dump, " iw", &neighbors.iw);
            dump.push('\n');
        }
        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            dump.push_str(&format!(
                "U {iu} mrlu={}",
                edge.mrlu
            ));
            push_usize_fields(&mut dump, " im", &edge.im);
            push_usize_fields(&mut dump, " iu", &edge.iu);
            push_usize_fields(&mut dump, " iw", &edge.iw);
            dump.push('\n');
        }
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            dump.push_str(&format!(
                "W {iw} npoly={} mrlw={} mrlw_orig={} mrow={} ngr={}",
                face.npoly,
                face.mrlw,
                face.mrlw_orig,
                face.mrow,
                face.ngr
            ));
            push_usize_fields(&mut dump, " im", &face.im);
            push_usize_fields(&mut dump, " iu", &face.iu);
            push_usize_fields(&mut dump, " iw", &face.iw);
            dump.push('\n');
        }
        dump
    }

    /// OLAM refinement using an explicit perimeter transition width.
    ///
    /// `max_mrows` controls the `perim_mrow` propagation width and allows callers
    /// to select atmosphere-like (13) or surface-like (7) boundary behavior.
    pub fn spawn_nest_with_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_internal(regions, max_level, max_mrows, None, false)
            .map(|(mesh, _)| mesh)
    }

    /// OLAM Method-C refinement for Cartesian/native XY coordinates used by
    /// Fortran `ngr_area` when a Method-C spawn is actually active.
    pub fn spawn_nest_cartesian_xy_with_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
    ) -> io::Result<Self> {
        self.spawn_nest_internal(regions, max_level, max_mrows, None, true)
            .map(|(mesh, _)| mesh)
    }

    /// Spawn specified OLAM refinement regions and run OLAM nest spring after
    /// each pass that actually refines faces. The returned counter is the
    /// number of spring passes executed.
    pub fn spawn_nest_with_spring(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_with_spring_and_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_SURFACE,
            nxp,
            niter,
        )
    }

    /// Spawn OLAM Method-C refinement with atmosphere-style transition width
    /// (`max_mrows = 13`) and run OLAM nest spring after each pass.
    pub fn spawn_nest_with_spring_as_atmosmesh(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_with_spring_and_max_mrows(
            regions,
            max_level,
            Self::METHOD_C_MAX_MROWS_ATMOS,
            nxp,
            niter,
        )
    }

    /// Spawn specified OLAM refinement regions with explicit perimeter row width and
    /// optional springing.
    pub fn spawn_nest_with_spring_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            false,
        )
    }

    /// OLAM Method-C refinement with springing for Cartesian/native XY
    /// coordinates used by Fortran `ngr_area` when a Method-C spawn is active.
    pub fn spawn_nest_cartesian_xy_with_spring_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, None)),
            true,
        )
    }

    /// OLAM Method-C refinement with springing for Cartesian/native XY
    /// coordinates, using Fortran `spring_dynamics_nest` target spacing:
    /// `deltax * sqrt(2 / sqrt(3))`.
    pub fn spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        nxp: usize,
        niter: usize,
        deltax_meters: f64,
    ) -> io::Result<(Self, usize)> {
        if !deltax_meters.is_finite() || deltax_meters < 0.001 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Cartesian nest spring deltax must be at least 0.001",
            ));
        }
        let cartesian_dist00 = deltax_meters * (2.0 / 3.0_f64.sqrt()).sqrt();
        self.spawn_nest_internal(
            regions,
            max_level,
            max_mrows,
            Some((nxp, niter, Some(cartesian_dist00))),
            true,
        )
    }

    fn spawn_nest_internal(
        &self,
        regions: &[OlamRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
    ) -> io::Result<(Self, usize)> {
        self.validate_topology()?;
        if regions.is_empty() || max_level == 0 {
            return Ok((self.clone(), 0));
        }
        if max_mrows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM spawn_nest max_mrows must be greater than zero",
            ));
        }
        if max_level > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("OLAM refinement max_level {max_level} must be in 1..=5"),
            ));
        }
        for region in regions {
            if use_cartesian_xy {
                region.validate_cartesian_xy()?;
            } else {
                region.validate()?;
            }
        }

        let mut mesh = self.clone();
        let mut spring_passes = 0usize;
        let mut next_grid_number = self
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.ngr)
            .chain(self.m_metadata.iter().skip(2).map(|metadata| metadata.ngr))
            .max()
            .unwrap_or(1)
            .max(1)
            + 1;
        let mut previous_pass_checkpoint: Option<(
            Self,
            Vec<bool>,
            usize,
            OlamRefinementRegion,
            bool,
        )> = None;
        for region in regions.iter().filter(|region| region.level() <= max_level) {
            let pass = region.level();
            if pass > 1 && matches!(region, OlamRefinementRegion::Polygon { .. }) {
                let has_nested_parent = mesh
                    .w_faces
                    .iter()
                    .skip(2)
                    .any(|face| face.ngr > 1);
                let has_parent_level_region =
                    regions.iter().any(|region| region.level() == pass - 1);
                if !has_nested_parent && !has_parent_level_region {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C perimeter length invalid: pass {pass} polygon regions require explicit parent-level halo"
                        ),
                    ));
                }
            }

            let selected_faces =
                mesh.selected_regions_faces(std::slice::from_ref(region), pass, use_cartesian_xy)?;
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "OLAM Method-C selected no active W faces for pass {pass}; refusing to replace a local nest with global expansion"
                    ),
                ));
            }
            let grid_number = next_grid_number;
            let mesh_before_pass = mesh.clone();
            let pass_requires_repair = mesh
                .spawn_nest_pass_method_c_without_mask_repair(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    !use_cartesian_xy,
                )
                .is_err();
            match mesh.spawn_nest_pass_with_max_mrows(
                &selected_faces,
                grid_number,
                max_mrows,
                !use_cartesian_xy,
            ) {
                Ok(refined) => mesh = refined,
                Err(error) => match mesh.spawn_nest_pass_with_mask_annealing(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    !use_cartesian_xy,
                    pass > 1,
                )? {
                    Some(refined) => mesh = refined,
                    None => {
                        if pass > 1 && spring.is_none() {
                            if let Some((
                                parent_base,
                                parent_selected,
                                parent_grid_number,
                                parent_region,
                                parent_required_repair,
                            )) =
                                previous_pass_checkpoint.as_ref()
                            {
                                if *parent_required_repair {
                                    if let Some(refined) = parent_base
                                        .retry_child_with_eroded_parent_mask(
                                        parent_selected,
                                        *parent_grid_number,
                                        parent_region,
                                        region,
                                        grid_number,
                                        max_mrows,
                                        !use_cartesian_xy,
                                        use_cartesian_xy,
                                    )?
                                    {
                                        mesh = refined;
                                        previous_pass_checkpoint = Some((
                                            mesh_before_pass,
                                            selected_faces.clone(),
                                            grid_number,
                                            region.clone(),
                                            pass_requires_repair,
                                        ));
                                        next_grid_number += 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        return Err(io::Error::new(
                            error.kind(),
                            format!("OLAM spawn_nest pass {pass} failed: {error}"),
                        ));
                    }
                },
            }
            previous_pass_checkpoint = Some((
                mesh_before_pass,
                selected_faces.clone(),
                grid_number,
                region.clone(),
                pass_requires_repair,
            ));
            next_grid_number += 1;

            if let Some((nxp, niter, cartesian_dist00)) = spring {
                if niter > 0 {
                    mesh = mesh.spring_nest_with_radius_projection(
                        nxp,
                        niter,
                        grid_number,
                        false,
                        !use_cartesian_xy,
                        cartesian_dist00,
                    )?;
                    spring_passes += 1;
                }
            }
        }

        Ok((mesh, spring_passes))
    }

    fn spawn_nest_pass_with_mask_annealing(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
        strict: bool,
    ) -> io::Result<Option<Self>> {
        let mut selected = selected_faces.to_vec();
        for _ in 0..32 {
            let eroded = if strict {
                self.erode_method_c_selected_m_boundary(&selected)?
            } else {
                self.erode_method_c_selected_boundary(&selected)?
            };
            let Some(eroded) = eroded else {
                return Ok(None);
            };
            selected = eroded;
            if selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
            }
            let attempt = if strict {
                self.spawn_nest_pass_method_c_without_mask_repair(
                    &selected,
                    child_level,
                    max_mrows,
                    project_to_radius,
                )
            } else {
                self.spawn_nest_pass_with_max_mrows(
                    &selected,
                    child_level,
                    max_mrows,
                    project_to_radius,
                )
            };
            if let Ok(refined) = attempt {
                return Ok(Some(refined));
            }
        }
        Ok(None)
    }

    fn erode_method_c_selected_boundary(&self, selected: &[bool]) -> io::Result<Option<Vec<bool>>> {
        require_olam_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let mut eroded = selected.to_vec();
        let mut removed = false;
        for iw in 2..=self.nwd {
            if !selected[iw] {
                continue;
            }
            let face = self.w_faces[iw];
            for &neighbor in face.iw.iter().take(3) {
                if neighbor <= 1 || neighbor > self.nwd || !selected[neighbor] {
                    eroded[iw] = false;
                    removed = true;
                    break;
                }
            }
        }
        if removed {
            Ok(Some(eroded))
        } else {
            Ok(None)
        }
    }

    fn erode_method_c_selected_m_boundary(
        &self,
        selected: &[bool],
    ) -> io::Result<Option<Vec<bool>>> {
        require_olam_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let m_neighbors = self.method_c_m_neighbors()?;
        let mut eroded = selected.to_vec();
        let mut removed = false;
        for im in 2..=self.nmd {
            let neighbors = m_neighbors[im];
            let mut selected_count = 0usize;
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_olam_id("OLAM Method-C M-boundary erosion W face", iw, self.nwd)?;
                selected_count += usize::from(selected[iw]);
            }
            if selected_count == 0 || selected_count == neighbors.npoly {
                continue;
            }
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                if selected[iw] {
                    eroded[iw] = false;
                    removed = true;
                }
            }
        }
        if removed {
            Ok(Some(eroded))
        } else {
            Ok(None)
        }
    }

    fn retry_child_with_eroded_parent_mask(
        &self,
        parent_selected_faces: &[bool],
        parent_grid_number: usize,
        parent_region: &OlamRefinementRegion,
        child_region: &OlamRefinementRegion,
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        if let Some(refined) = self.retry_child_with_scaled_parent_region(
            parent_region,
            parent_grid_number,
            child_region,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )? {
            return Ok(Some(refined));
        }
        if let Some(refined) = self.retry_child_with_parent_mask_sequence(
            parent_selected_faces.to_vec(),
            true,
            parent_grid_number,
            child_region,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )? {
            return Ok(Some(refined));
        }
        self.retry_child_with_parent_mask_sequence(
            parent_selected_faces.to_vec(),
            false,
            parent_grid_number,
            child_region,
            child_grid_number,
            max_mrows,
            project_to_radius,
            use_cartesian_xy,
        )
    }

    fn retry_child_with_scaled_parent_region(
        &self,
        parent_region: &OlamRefinementRegion,
        parent_grid_number: usize,
        child_region: &OlamRefinementRegion,
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        for step in 1..=12 {
            let factor = 1.0 - (step as f64 * 0.05);
            let Some(scaled_parent_region) =
                scale_olam_refinement_region_radius(parent_region, factor)
            else {
                return Ok(None);
            };
            let parent_selected = self.selected_regions_faces(
                std::slice::from_ref(&scaled_parent_region),
                scaled_parent_region.level(),
                use_cartesian_xy,
            )?;
            if parent_selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
            }
            let Ok(parent_mesh) = self.spawn_nest_pass_with_max_mrows(
                &parent_selected,
                parent_grid_number,
                max_mrows,
                project_to_radius,
            ) else {
                continue;
            };
            let child_selected = parent_mesh.selected_regions_faces(
                std::slice::from_ref(child_region),
                child_region.level(),
                use_cartesian_xy,
            )?;
            if child_selected.iter().skip(2).all(|selected| !*selected) {
                continue;
            }
            if let Ok(refined) = parent_mesh.spawn_nest_pass_with_max_mrows(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
            ) {
                return Ok(Some(refined));
            }
            if let Some(refined) = parent_mesh.spawn_nest_pass_with_mask_annealing(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
                true,
            )? {
                return Ok(Some(refined));
            }
        }
        Ok(None)
    }

    fn retry_child_with_parent_mask_sequence(
        &self,
        mut parent_selected: Vec<bool>,
        grow_parent: bool,
        parent_grid_number: usize,
        child_region: &OlamRefinementRegion,
        child_grid_number: usize,
        max_mrows: usize,
        project_to_radius: bool,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<Self>> {
        for _ in 0..32 {
            let next_parent = if grow_parent {
                self.grow_method_c_selected_boundary(&parent_selected)?
            } else {
                self.erode_method_c_selected_boundary(&parent_selected)?
            };
            let Some(next_parent) = next_parent else {
                return Ok(None);
            };
            parent_selected = next_parent;
            if parent_selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
            }

            let Ok(parent_mesh) = self.spawn_nest_pass_method_c_without_mask_repair(
                &parent_selected,
                parent_grid_number,
                max_mrows,
                project_to_radius,
            ) else {
                continue;
            };
            let child_selected = parent_mesh.selected_regions_faces(
                std::slice::from_ref(child_region),
                child_region.level(),
                use_cartesian_xy,
            )?;
            if child_selected.iter().skip(2).all(|selected| !*selected) {
                continue;
            }

            if let Ok(refined) = parent_mesh.spawn_nest_pass_with_max_mrows(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
            ) {
                return Ok(Some(refined));
            }
            if let Some(refined) = parent_mesh.spawn_nest_pass_with_mask_annealing(
                &child_selected,
                child_grid_number,
                max_mrows,
                project_to_radius,
                true,
            )? {
                return Ok(Some(refined));
            }
        }
        Ok(None)
    }

    fn grow_method_c_selected_boundary(&self, selected: &[bool]) -> io::Result<Option<Vec<bool>>> {
        require_olam_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let parent_mrlw = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, is_selected)| is_selected.then_some(self.w_faces[iw].mrlw));
        let Some(parent_mrlw) = parent_mrlw else {
            return Ok(None);
        };
        let mut grown = selected.to_vec();
        let mut added = false;
        for iw in 2..=self.nwd {
            if !selected[iw] {
                continue;
            }
            let face = self.w_faces[iw];
            for &neighbor in face.iw.iter().take(3) {
                if neighbor <= 1 || neighbor > self.nwd {
                    continue;
                }
                if !selected[neighbor] && self.w_faces[neighbor].mrlw == parent_mrlw {
                    grown[neighbor] = true;
                    added = true;
                }
            }
        }
        if added {
            Ok(Some(grown))
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    fn selected_region_faces(
        &self,
        region: &OlamRefinementRegion,
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        self.selected_regions_faces(std::slice::from_ref(region), pass, use_cartesian_xy)
    }

    fn selected_regions_faces(
        &self,
        regions: &[OlamRefinementRegion],
        pass: usize,
        use_cartesian_xy: bool,
    ) -> io::Result<Vec<bool>> {
        let radius = active_mesh_radius(self)?;
        require_olam_len("m_points", self.m_points.len(), self.nmd + 1)?;
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        let mut selected = vec![false; self.nwd + 1];
        if regions.is_empty() {
            return Ok(selected);
        }
        let seed_points =
            self.selected_region_thirdm_seed_points_with_neighbors(
                regions,
                pass,
                radius,
                &method_c_m_neighbors,
                use_cartesian_xy,
        )?;
        for im in seed_points {
            let mrlo = self.m_metadata[im].mrlm;
            let mut footprint = vec![false; self.nwd + 1];
            self.mark_fill_rad3_faces_with_neighbors(im, &mut footprint, &method_c_m_neighbors)?;
            for iw in 2..=self.nwd {
                if footprint[iw] && self.w_faces[iw].mrlw == mrlo {
                    selected[iw] = true;
                }
            }
        }
        if selected.iter().skip(2).all(|selected| !*selected) {
            return Ok(selected);
        }
        Ok(selected)
    }

    fn method_c_m_neighbors(&self) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
        require_olam_len(
            "Method-C M-neighbor table",
            self.m_neighbors.len(),
            self.nmd + 1,
        )?;
        Ok(self.m_neighbors.clone())
    }

    #[cfg(test)]
    fn derive_icosahedron_m_neighbors_fortran(&self) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
        derive_icosahedron_m_neighbors_fortran_checked(self.nmd, &self.u_edges, &self.w_faces)
    }

    fn selected_region_thirdm_seed_points_with_neighbors(
        &self,
        regions: &[OlamRefinementRegion],
        pass: usize,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<BTreeSet<usize>> {
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let mut seeds = BTreeSet::new();
        let active_regions = regions
            .iter()
            .filter(|region| region.level() >= pass)
            .cloned()
            .collect::<Vec<_>>();
        if active_regions.is_empty() {
            return Ok(seeds);
        }
        let start = self.olam_refinement_start_point_for_regions_with_neighbors(
            &active_regions,
            radius,
            m_neighbors,
            use_cartesian_xy,
        )?;
        let mrlo = self.m_metadata[start].mrlm;

        let mut jdone = vec![[false; 6]; self.nmd + 1];
        let mut lista = vec![start];
        while let Some(im) = lista.pop() {
            let neighbors = m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                require_olam_id("OLAM refinement boundary U edge", iu, self.nud)?;
                if self.u_edges[iu].mrlu != mrlo {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C perimeter length invalid: Current nested grid crosses the parent boundary / next coarser grid boundary at M point {im}"
                        ),
                    ));
                }
            }
            seeds.insert(im);

            for neighbor in self.olam_thirdm_neighbors_fortran_with_neighbors(im, &mut jdone, m_neighbors)?
            {
                let point = self.m_points[neighbor];
                let traversed_count = jdone[neighbor].iter().filter(|&&done| done).count();
                if traversed_count < 2
                    && olam_regions_contain_method_c(
                        &active_regions,
                        point,
                        radius,
                        use_cartesian_xy,
                    )
                {
                    lista.push(neighbor);
                }
            }
        }
        Ok(seeds)
    }

    #[cfg(test)]
    fn olam_refinement_start_point_with_neighbors(
        &self,
        region: &OlamRefinementRegion,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        self.olam_refinement_start_point_for_regions_with_neighbors(
            std::slice::from_ref(region),
            radius,
            m_neighbors,
            use_cartesian_xy,
        )
    }

    fn olam_refinement_start_point_for_regions_with_neighbors(
        &self,
        regions: &[OlamRefinementRegion],
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let Some(first_region) = regions.first() else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM refinement start requires at least one region",
            ));
        };
        let imcent = self.closest_m_point_to_region_anchor(first_region, use_cartesian_xy)?;
        if use_cartesian_xy {
            return Ok(imcent);
        }
        for &pentagon_id in &self.impent {
            if pentagon_id <= 1 {
                continue;
            }
            require_olam_id("OLAM refinement pentagon M point", pentagon_id, self.nmd)?;
            if olam_regions_contain_method_c(
                regions,
                self.m_points[pentagon_id],
                radius,
                use_cartesian_xy,
            ) {
                return Ok(pentagon_id);
            }
        }
        let mut nearby_pentagon = None;
        for &pentagon_id in &self.impent {
            if pentagon_id <= 1 {
                continue;
            }
            require_olam_id("OLAM refinement pentagon M point", pentagon_id, self.nmd)?;
            if olam_regions_close_to_method_c(
                regions,
                self.m_points[pentagon_id],
                radius,
                use_cartesian_xy,
            )
                && self.m_metadata[pentagon_id].mrlm == self.m_metadata[imcent].mrlm
            {
                nearby_pentagon = Some(pentagon_id);
            }
        }
        if let Some(pentagon_id) = nearby_pentagon {
            if let Some(start) =
                self.olam_march_from_nearby_pentagon_to_regions_with_neighbors(
                    pentagon_id,
                    regions,
                    radius,
                    m_neighbors,
                    use_cartesian_xy,
                )?
            {
                return Ok(start);
            }
        }
        Ok(imcent)
    }

    #[cfg(test)]
    fn olam_march_from_nearby_pentagon_to_region_with_neighbors(
        &self,
        pentagon_id: usize,
        region: &OlamRefinementRegion,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        self.olam_march_from_nearby_pentagon_to_regions_with_neighbors(
            pentagon_id,
            std::slice::from_ref(region),
            radius,
            m_neighbors,
            use_cartesian_xy,
        )
    }

    fn olam_march_from_nearby_pentagon_to_regions_with_neighbors(
        &self,
        pentagon_id: usize,
        regions: &[OlamRefinementRegion],
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        require_olam_id(
            "OLAM refinement nearby pentagon M point",
            pentagon_id,
            self.nmd,
        )?;
        let Some(nearest_inside) =
            self.nearest_inside_m_point_to_regions(pentagon_id, regions, radius, use_cartesian_xy)?
        else {
            return Ok(None);
        };

        let mut current = pentagon_id;
        let mut visited = BTreeSet::new();
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        for _ in 0..self.nmd {
            if !visited.insert(current) {
                return Ok(None);
            }

            let mut best_neighbor = 0usize;
            let mut best_distance = f64::INFINITY;
            jdone[current] = [false; 6];
            for neighbor in
                self.olam_thirdm_neighbors_fortran_with_neighbors(current, &mut jdone, m_neighbors)?
            {
                let point = self.m_points[neighbor];
                if olam_regions_contain_method_c(regions, point, radius, use_cartesian_xy) {
                    return Ok(Some(neighbor));
                }
                let distance = euclidean_distance(point, self.m_points[nearest_inside]);
                if distance < best_distance {
                    best_distance = distance;
                    best_neighbor = neighbor;
                }
            }
            if best_neighbor <= 1 {
                return Ok(None);
            }
            current = best_neighbor;
        }

        Ok(None)
    }

    #[cfg(test)]
    fn nearest_inside_m_point_to(
        &self,
        source_im: usize,
        region: &OlamRefinementRegion,
        radius: f64,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        self.nearest_inside_m_point_to_regions(
            source_im,
            std::slice::from_ref(region),
            radius,
            use_cartesian_xy,
        )
    }

    fn nearest_inside_m_point_to_regions(
        &self,
        source_im: usize,
        regions: &[OlamRefinementRegion],
        radius: f64,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        require_olam_id("OLAM refinement source M point", source_im, self.nmd)?;
        let mut nearest_inside = None;
        let mut nearest_distance = f64::INFINITY;
        for im in 2..=self.nmd {
            if !olam_regions_contain_method_c(
                regions,
                self.m_points[im],
                radius,
                use_cartesian_xy,
            ) {
                continue;
            }
            let distance = euclidean_distance(self.m_points[im], self.m_points[source_im]);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_inside = Some(im);
            }
        }
        Ok(nearest_inside)
    }

    fn closest_m_point_to_region_anchor(
        &self,
        region: &OlamRefinementRegion,
        use_cartesian_xy: bool,
    ) -> io::Result<usize> {
        if use_cartesian_xy {
            let anchor = region.anchor_lonlat();
            let mut best_im = 0usize;
            let mut best_distance = f64::INFINITY;
            for im in 2..=self.nmd {
                let point = self.m_points[im];
                let distance = (point.x - anchor.lon_degrees).hypot(point.y - anchor.lat_degrees);
                if distance < best_distance {
                    best_distance = distance;
                    best_im = im;
                }
            }
            return require_olam_id("OLAM refinement anchor M point", best_im, self.nmd)
                .map(|_| best_im);
        }
        let anchor = lonlat_degrees_to_unit_xyz(region.anchor_lonlat());
        let mut best_im = 0usize;
        let mut best_score = f64::NEG_INFINITY;
        for im in 2..=self.nmd {
            let point = self.m_points[im];
            let point_radius = magnitude(point);
            if point_radius == 0.0 {
                continue;
            }
            let score = dot(point, anchor) / point_radius;
            if score > best_score {
                best_score = score;
                best_im = im;
            }
        }
        require_olam_id("OLAM refinement anchor M point", best_im, self.nmd)?;
        Ok(best_im)
    }

    fn olam_thirdm_neighbors_fortran_with_neighbors(
        &self,
        im: usize,
        jdone: &mut [[bool; 6]],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_olam_id("OLAM thirdm start M point", im, self.nmd)?;
        require_olam_len("OLAM thirdm jdone", jdone.len(), self.nmd + 1)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let neighbors = m_neighbors[im];
        let mut third_neighbors = Vec::new();
        let max_edges = neighbors.npoly.min(6);
        for j in 0..max_edges {
            if jdone[im][j] {
                continue;
            }
            let iu = neighbors.iu[j];
            jdone[im][j] = true;
            let imm = self.other_m_endpoint(iu, im)?;
            let iuu = match self.opposite_ring_u_edge_with_neighbors(imm, iu, m_neighbors) {
                Ok(iuu) => iuu,
                Err(_) => continue,
            };
            let immm = match self.other_m_endpoint(iuu, imm) {
                Ok(immm) => immm,
                Err(_) => continue,
            };
            let iuuu = match self.opposite_ring_u_edge_with_neighbors(immm, iuu, m_neighbors) {
                Ok(iuuu) => iuuu,
                Err(_) => continue,
            };
            let immmm = match self.other_m_endpoint(iuuu, immm) {
                Ok(immmm) => immmm,
                Err(_) => continue,
            };
            require_olam_id("OLAM thirdm far M point", immmm, self.nmd)?;
            let far_neighbors = m_neighbors[immmm];
            for jj in 0..6 {
                let far_iu = far_neighbors.iu[jj];
                if far_iu < 2 || far_iu > self.nud {
                    continue;
                }
                if far_iu == iuuu {
                    jdone[immmm][jj] = true;
                    break;
                }
            }
            third_neighbors.push(immmm);
        }
        Ok(third_neighbors)
    }

    fn opposite_ring_u_edge_with_neighbors(
        &self,
        im: usize,
        incoming_iu: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<usize> {
        require_olam_id("OLAM thirdm M point", im, self.nmd)?;
        require_olam_id("OLAM thirdm incoming U edge", incoming_iu, self.nud)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let neighbors = m_neighbors[im];
        for j in 0..6 {
            let iu = neighbors.iu[j];
            if iu < 2 || iu > self.nud {
                continue;
            }
            if iu == incoming_iu {
                let opposite = neighbors.iu[(j + 3) % 6];
                if opposite < 2 || opposite > self.nud {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "OLAM thirdm incoming U edge {incoming_iu} has no valid opposite at M point {im}"
                        ),
                    ));
                }
                require_olam_id("OLAM thirdm opposite U edge", opposite, self.nud)?;
                return Ok(opposite);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM thirdm incoming U edge {incoming_iu} is not in M point {im}'s ring"),
        ))
    }

    fn other_m_endpoint(&self, iu: usize, im: usize) -> io::Result<usize> {
        require_olam_id("OLAM U edge", iu, self.nud)?;
        require_olam_id("OLAM M endpoint", im, self.nmd)?;
        let edge = self.u_edges[iu];
        if edge.im[0] == im {
            require_olam_id("OLAM opposite M endpoint", edge.im[1], self.nmd)?;
            Ok(edge.im[1])
        } else if edge.im[1] == im {
            require_olam_id("OLAM opposite M endpoint", edge.im[0], self.nmd)?;
            Ok(edge.im[0])
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM U edge {iu} is not incident on M point {im}"),
            ))
        }
    }

    fn mark_fill_rad3_faces_with_neighbors(
        &self,
        im: usize,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<bool> {
        require_olam_id("OLAM fill_rad3 M point", im, self.nmd)?;
        require_olam_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;

        let mut changed = false;
        let neighbors = m_neighbors[im];

        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_olam_id("OLAM fill_rad3 sector W face", iw, self.nwd)?;
            changed |= !selected_faces[iw];
            selected_faces[iw] = true;

            let face = self.w_faces[iw];
            let (imx, iwx, iwy) = if im == face.im[0] {
                (face.im[1], face.iw[3], face.iw[4])
            } else if im == face.im[1] {
                (face.im[2], face.iw[5], face.iw[6])
            } else if im == face.im[2] {
                (face.im[0], face.iw[7], face.iw[8])
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("OLAM fill_rad3 M point {im} is not on W face {iw}"),
                ));
            };
            require_olam_id("OLAM fill_rad3 sector M point", imx, self.nmd)?;
            require_olam_id("OLAM fill_rad3 outer W face", iwx, self.nwd)?;
            require_olam_id("OLAM fill_rad3 outer W face", iwy, self.nwd)?;

            let (im1, im2) =
                face_following_two_vertices(self.w_faces[iwx], imx, iwx).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "fill_rad3 im={im} iw={iw} imx={imx} iwx={iwx} face={:?}/{:?}: {error}",
                            face.im, face.iw
                        ),
                    )
                })?;
            require_olam_id("OLAM fill_rad3 distant M point", im1, self.nmd)?;
            require_olam_id("OLAM fill_rad3 distant M point", im2, self.nmd)?;
            let im3 = face_following_vertex(self.w_faces[iwy], im2, iwy).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!(
                        "fill_rad3 im={im} iw={iw} imx={imx} im2={im2} iwy={iwy} face={:?}/{:?}: {error}",
                        face.im, face.iw
                    ),
                )
            })?;
            require_olam_id("OLAM fill_rad3 distant M point", im3, self.nmd)?;

            for far_im in [im1, im2, im3] {
                let far_neighbors = m_neighbors[far_im];
                for &far_iw in far_neighbors.iw.iter().take(6) {
                    if far_iw > self.nwd {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("OLAM fill_rad3 distant W face {far_iw} is out of range"),
                        ));
                    }
                    changed |= !selected_faces[far_iw];
                    selected_faces[far_iw] = true;
                }
            }
        }

        Ok(changed)
    }

    #[cfg(test)]
    fn method_c_w_face_is_active(&self, iw: usize) -> bool {
        if iw > self.nwd || self.w_prognostic.get(iw).copied().unwrap_or(iw) != iw {
            return false;
        }
        self.w_faces[iw]
            .im
            .iter()
            .all(|&im| im > 1 && self.m_prognostic.get(im).copied().unwrap_or(im) == im)
    }

    #[cfg(test)]
    fn close_olam_selected_face_concavities(&self, selected_faces: &mut [bool]) -> io::Result<()> {
        self.close_olam_method_c_concavities(selected_faces)
    }

    fn ensure_method_c_selected_faces_share_parent_mrlw(
        &self,
        selected_faces: &[bool],
        child_level: usize,
    ) -> io::Result<()> {
        require_olam_len(
            "Method-C selected faces",
            selected_faces.len(),
            self.nwd + 1,
        )?;

        let radius = active_mesh_radius(self)?;
        let mut parent_mrlw = None;
        for iw in 2..=self.nwd {
            if !selected_faces[iw] {
                continue;
            }

            let face = self.w_faces[iw];
            if let Some(expected_mrlw) = parent_mrlw {
                if face.mrlw != expected_mrlw {
                    let center = normalized_face_center(
                        self.m_points[face.im[0]],
                        self.m_points[face.im[1]],
                        self.m_points[face.im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Current nested grid {child_level} crosses (or is too close to) the next coarser grid boundary at W face {iw} (mrlw={}, expected_mrlw={}, lon={:.3}, lat={:.3})",
                            face.mrlw, expected_mrlw, ll.lon_degrees, ll.lat_degrees
                        ),
                    ));
                }
            } else {
                parent_mrlw = Some(face.mrlw);
            }
        }

        Ok(())
    }

    fn spawn_nest_pass_with_max_mrows(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.spawn_nest_pass_method_c(selected_faces, child_level, max_mrows, project_to_radius)
    }

    fn spawn_nest_pass_method_c(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_olam_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Method-C child level must be greater than one",
            ));
        }

        let mut selected = selected_faces.to_vec();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;

        let mut last_repairable_error = None;
        for _ in 0..64 {
            let perimeter = self.repair_method_c_non_triplet_perimeter(
                &mut selected,
                &method_c_m_neighbors,
                child_level,
            )?;
            let mut nest_wd =
                self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
            match self.emit_method_c_tables(
                &perimeter,
                &method_c_m_neighbors,
                &mut nest_wd,
                child_level,
                max_mrows,
                project_to_radius,
            ) {
                Ok(mesh) => return Ok(mesh),
                Err(error) if Self::is_repairable_method_c_transition_error(&error) => {
                    let valence_m = Self::method_c_valence_error_m_point(&error);
                    let mut repaired = if valence_m.is_some() {
                        self.try_shrink_method_c_perimeter_once(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?
                    } else {
                        None
                    };
                    if repaired.is_none() {
                        repaired = if let Some(im) = valence_m {
                            self.try_fill_method_c_specific_m_point(
                                &selected,
                                &method_c_m_neighbors,
                                child_level,
                                im,
                            )?
                        } else {
                            None
                        };
                    }
                    if repaired.is_none() {
                        repaired = self.try_fill_method_c_perimeter_boundary(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?;
                    }
                    if repaired.is_none() {
                        repaired = self.try_grow_method_c_non_triplet_perimeter_once(
                            &selected,
                            &method_c_m_neighbors,
                            child_level,
                            Some(&perimeter),
                        )?;
                    }
                    let Some((repaired, _)) = repaired else {
                        return Err(error);
                    };
                    selected.clone_from_slice(&repaired);
                    last_repairable_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_repairable_error.unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C automatic perimeter repair exceeded its iteration limit",
            )
        }))
    }

    fn spawn_nest_pass_method_c_without_mask_repair(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        self.validate_topology()?;
        require_olam_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        if child_level <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM Method-C child level must be greater than one",
            ));
        }

        let mut selected = selected_faces.to_vec();
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )?;
        self.ensure_method_c_selected_faces_share_parent_mrlw(&selected, child_level)?;

        let perimeter =
            self.method_c_perimeter_from_selected_faces(&selected, &method_c_m_neighbors)?;
        if perimeter.len() % 3 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Method-C perimeter length invalid: perimeter length {} cannot be grouped into transition triples",
                    perimeter.len()
                ),
            ));
        }
        let mut nest_wd =
            self.method_c_nest_wd_from_selected_and_perimeter(&selected, &perimeter)?;
        self.emit_method_c_tables(
            &perimeter,
            &method_c_m_neighbors,
            &mut nest_wd,
            child_level,
            max_mrows,
            project_to_radius,
        )
    }

    fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        let message = error.to_string();
        message.contains("transition patch")
            || message.contains("exceeds 7-edge OLAM ring")
            || message.contains("cannot be grouped into transition triples")
    }

    fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let message = error.to_string();
        let start = message.find("M point ")? + "M point ".len();
        let rest = &message[start..];
        let digit_count = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digit_count == 0 || !rest[digit_count..].starts_with(" exceeds 7-edge") {
            return None;
        }
        rest[..digit_count].parse().ok()
    }

    fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<OlamMethodCPerimeterPoint>> {
        const MAX_REPAIR_PASSES: usize = 12;

        let mut last_error = None;
        for _ in 0..MAX_REPAIR_PASSES {
            let perimeter = match self.method_c_perimeter_from_selected_faces(selected, m_neighbors)
            {
                Ok(perimeter) if perimeter.len() % 3 == 0 => return Ok(perimeter),
                Ok(perimeter) => Some(perimeter),
                Err(error) => {
                    last_error = Some(error);
                    None
                }
            };
            let Some((repaired, repaired_perimeter)) = self
                .try_grow_method_c_non_triplet_perimeter_once(
                    selected,
                    m_neighbors,
                    child_level,
                    perimeter.as_deref(),
                )?
            else {
                break;
            };
            selected.clone_from_slice(&repaired);
            if repaired_perimeter.len() % 3 == 0 {
                return Ok(repaired_perimeter);
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        let perimeter = self.method_c_perimeter_from_selected_faces(selected, m_neighbors)?;
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Method-C perimeter length invalid: perimeter length {} cannot be grouped into transition triples without crossing the parent boundary",
                perimeter.len()
            ),
        ))
    }

    fn try_grow_method_c_non_triplet_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[OlamMethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<OlamMethodCPerimeterPoint>)>> {
        let parent_mrlw = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, is_selected)| is_selected.then_some(self.w_faces[iw].mrlw))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "OLAM Method-C cannot repair an empty selected face mask",
                )
            })?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut candidates = BTreeSet::new();

        if let Some(perimeter) = perimeter {
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C repair candidate W face", iw, self.nwd)?;
                    if !selected[iw] && self.w_faces[iw].mrlw == parent_mrlw {
                        candidates.insert(iw);
                    }
                }
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C repair boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m == 0 || selected_count_at_m == neighbors.npoly {
                    continue;
                }
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    if !selected[iw] && self.w_faces[iw].mrlw == parent_mrlw {
                        candidates.insert(iw);
                    }
                }
            }
        }

        let mut best: Option<(usize, usize, usize, Vec<bool>, Vec<OlamMethodCPerimeterPoint>)> =
            None;
        for candidate in candidates {
            let mut trial = selected.to_vec();
            trial[candidate] = true;
            self.close_olam_method_c_concavities_for_level_with_neighbors(
                &mut trial,
                m_neighbors,
            )?;
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeter) =
                self.method_c_perimeter_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            let added = trial.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
                continue;
            }
            let remainder = trial_perimeter.len() % 3;
            if remainder == 0 {
                return Ok(Some((trial, trial_perimeter)));
            }
            let score = (added, remainder, trial_perimeter.len(), trial, trial_perimeter);
            if best
                .as_ref()
                .is_none_or(|current| (score.0, score.1, score.2) < (current.0, current.1, current.2))
            {
                best = Some(score);
            }
        }

        Ok(best.map(|(_, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }

    fn try_shrink_method_c_perimeter_once(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[OlamMethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<OlamMethodCPerimeterPoint>)>> {
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut candidates = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            for point in perimeter {
                let neighbors = m_neighbors[point.im];
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C shrink candidate W face", iw, self.nwd)?;
                    if selected[iw] {
                        candidates.insert(iw);
                    }
                }
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C shrink boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m > 0 && selected_count_at_m < neighbors.npoly {
                    for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                        if selected[iw] {
                            candidates.insert(iw);
                        }
                    }
                }
            }
        }

        let mut best: Option<(usize, usize, usize, Vec<bool>, Vec<OlamMethodCPerimeterPoint>)> =
            None;
        for candidate in candidates {
            let mut trial = selected.to_vec();
            trial[candidate] = false;
            self.close_olam_method_c_concavities_for_level_with_neighbors(
                &mut trial,
                m_neighbors,
            )?;
            let trial_count = trial.iter().filter(|&&item| item).count();
            if trial_count == 0 || trial_count >= selected_count {
                continue;
            }
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeter) =
                self.method_c_perimeter_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            let removed = selected_count - trial_count;
            let remainder = trial_perimeter.len() % 3;
            let score = (removed, remainder, trial_perimeter.len(), trial, trial_perimeter);
            if best
                .as_ref()
                .is_none_or(|current| (score.0, score.1, score.2) < (current.0, current.1, current.2))
            {
                best = Some(score);
            }
        }

        Ok(best.map(|(_, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }

    fn try_fill_method_c_specific_m_point(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        im: usize,
    ) -> io::Result<Option<(Vec<bool>, Vec<OlamMethodCPerimeterPoint>)>> {
        require_olam_id("OLAM Method-C valence repair M point", im, self.nmd)?;
        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut trial = selected.to_vec();
        self.mark_fill_rad3_faces_with_neighbors(im, &mut trial, m_neighbors)?;
        self.close_olam_method_c_concavities_for_level_with_neighbors(&mut trial, m_neighbors)?;
        if trial.iter().filter(|&&item| item).count() == selected_count {
            return Ok(None);
        }
        if self
            .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
            .is_err()
        {
            return Ok(None);
        }
        let Ok(trial_perimeter) =
            self.method_c_perimeter_from_selected_faces(&trial, m_neighbors)
        else {
            return Ok(None);
        };
        Ok(Some((trial, trial_perimeter)))
    }

    fn try_fill_method_c_perimeter_boundary(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
        perimeter: Option<&[OlamMethodCPerimeterPoint]>,
    ) -> io::Result<Option<(Vec<bool>, Vec<OlamMethodCPerimeterPoint>)>> {
        let mut boundary_m = BTreeSet::new();
        if let Some(perimeter) = perimeter {
            for point in perimeter {
                boundary_m.insert(point.im);
            }
        } else {
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count_at_m = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C repair boundary W face", iw, self.nwd)?;
                    selected_count_at_m += usize::from(selected[iw]);
                }
                if selected_count_at_m > 0 && selected_count_at_m < neighbors.npoly {
                    boundary_m.insert(im);
                }
            }
        }
        if boundary_m.is_empty() {
            return Ok(None);
        }

        let selected_count = selected.iter().filter(|&&item| item).count();
        let mut best: Option<(usize, usize, usize, Vec<bool>, Vec<OlamMethodCPerimeterPoint>)> =
            None;
        for im in boundary_m {
            let mut trial = selected.to_vec();
            self.mark_fill_rad3_faces_with_neighbors(im, &mut trial, m_neighbors)?;
            self.close_olam_method_c_concavities_for_level_with_neighbors(
                &mut trial,
                m_neighbors,
            )?;
            let added = trial.iter().filter(|&&item| item).count() - selected_count;
            if added == 0 {
                continue;
            }
            if self
                .ensure_method_c_selected_faces_share_parent_mrlw(&trial, child_level)
                .is_err()
            {
                continue;
            }
            let Ok(trial_perimeter) =
                self.method_c_perimeter_from_selected_faces(&trial, m_neighbors)
            else {
                continue;
            };
            let remainder = trial_perimeter.len() % 3;
            let score = (added, remainder, trial_perimeter.len(), trial, trial_perimeter);
            if best
                .as_ref()
                .is_none_or(|current| (score.0, score.1, score.2) < (current.0, current.1, current.2))
            {
                best = Some(score);
            }
        }

        Ok(best.map(|(_, _, _, trial, trial_perimeter)| (trial, trial_perimeter)))
    }

    fn method_c_perimeter_from_selected_faces(
        &self,
        selected: &[bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<OlamMethodCPerimeterPoint>> {
        let mut probe_nest_wd = vec![OlamMethodCNestWd::default(); self.nwd + 1];
        for iw in 2..=self.nwd {
            if selected[iw] {
                probe_nest_wd[iw].iw[2] = 1;
            }
        }

        let perimeter = self.perim_map2_method_c(&probe_nest_wd, m_neighbors)?;
        if perimeter.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "OLAM Method-C perimeter is empty",
            ));
        }
        Ok(perimeter)
    }

    fn method_c_nest_wd_from_selected_and_perimeter(
        &self,
        selected: &[bool],
        perimeter: &[OlamMethodCPerimeterPoint],
    ) -> io::Result<Vec<OlamMethodCNestWd>> {
        let mut nest_wd = vec![OlamMethodCNestWd::default(); self.nwd + 1];
        for iw in 2..=self.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }

        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = self.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            require_olam_id("OLAM Method-C suppressed W face", suppressed_w, self.nwd)?;
            nest_wd[suppressed_w].iw[2] = -1;
        }
        Ok(nest_wd)
    }

    #[cfg(test)]
    fn close_olam_method_c_concavities(&self, selected_faces: &mut [bool]) -> io::Result<()> {
        let method_c_m_neighbors = self.method_c_m_neighbors()?;
        self.close_olam_method_c_concavities_with_neighbors(selected_faces, &method_c_m_neighbors)
    }

    #[cfg(test)]
    fn close_olam_method_c_concavities_with_neighbors(
        &self,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<()> {
        self.close_olam_method_c_concavities_for_level_with_neighbors(
            selected_faces,
            m_neighbors,
        )
    }

    fn close_olam_method_c_concavities_for_level_with_neighbors(
        &self,
        selected_faces: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<()> {
        require_olam_len("selected_faces", selected_faces.len(), self.nwd + 1)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        loop {
            let mut changed = false;
            for im in 2..=self.nmd {
                let neighbors = m_neighbors[im];
                let mut selected_count = 0usize;
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    require_olam_id("OLAM Method-C concavity W face", iw, self.nwd)?;
                    selected_count += usize::from(selected_faces[iw]);
                }
                if selected_count == 0 || selected_count == neighbors.npoly {
                    continue;
                }
                // Fortran behavior: fill when the selected incidence is at least
                // (npoly - 1), including pentagons when exactly one face is
                // missing and when all faces are selected.
                if selected_count < neighbors.npoly.saturating_sub(1) {
                    continue;
                }
                changed |= self.mark_fill_rad3_faces_with_neighbors(
                    im,
                    selected_faces,
                    m_neighbors,
                )?;
            }
            if !changed {
                return Ok(());
            }
        }
    }

    fn perim_map2_method_c(
        &self,
        nest_wd: &[OlamMethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<OlamMethodCPerimeterPoint>> {
        require_olam_len("OLAM Method-C nest_wd", nest_wd.len(), self.nwd + 1)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        for im in 2..=self.nmd {
            let neighbors = m_neighbors[im];
            let mut nwdiv = 0usize;
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_olam_id("OLAM Method-C perimeter W face", iw, self.nwd)?;
                if nest_wd[iw].is_subdivided() {
                    nwdiv += 1;
                }
            }
            if nwdiv == 2 {
                return self.perim_map2_method_c_from(im, nest_wd, m_neighbors);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "OLAM Method-C perimeter has no nwdiv == 2 convex start point",
        ))
    }

    fn perim_map2_method_c_from(
        &self,
        start: usize,
        nest_wd: &[OlamMethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<OlamMethodCPerimeterPoint>> {
        let mut perimeter = Vec::new();
        let mut current = start;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "OLAM Method-C perimeter loop revisited M point {current} before closing"
                    ),
                ));
            }

            let neighbors = m_neighbors[current];
            let mut nwdiv = 0usize;
            let mut near_pentagon = false;
            for j in 0..neighbors.npoly {
                let iw = neighbors.iw[j];
                let iu = neighbors.iu[j];
                require_olam_id("OLAM Method-C perimeter W face", iw, self.nwd)?;
                require_olam_id("OLAM Method-C perimeter U edge", iu, self.nud)?;
                if nest_wd[iw].is_subdivided() {
                    nwdiv += 1;
                }

                let edge = self.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                require_olam_id("OLAM Method-C perimeter adjacent W face", iw1, self.nwd)?;
                require_olam_id("OLAM Method-C perimeter adjacent W face", iw2, self.nwd)?;
                if nest_wd[iw1].flag() == 0 && nest_wd[iw2].flag() == 0 {
                    if current == edge.im[0] && m_neighbors[edge.im[1]].npoly == 5 {
                        near_pentagon = true;
                    }
                    if current == edge.im[1] && m_neighbors[edge.im[0]].npoly == 5 {
                        near_pentagon = true;
                    }
                }
            }

            let (next, edge) = self.perim_ngr_method_c(current, nest_wd, m_neighbors)?;
            perimeter.push(OlamMethodCPerimeterPoint {
                im: current,
                iu: edge,
                npoly: neighbors.npoly,
                nwdiv,
                near_pentagon,
            });

            if next == start {
                break;
            }
            current = next;
        }

        Ok(perimeter)
    }

    fn perim_ngr_method_c(
        &self,
        imstart: usize,
        nest_wd: &[OlamMethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<(usize, usize)> {
        require_olam_id("OLAM Method-C perimeter M point", imstart, self.nmd)?;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let neighbors = m_neighbors[imstart];
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            require_olam_id("OLAM Method-C perimeter U edge", iu, self.nud)?;
            let edge = self.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            require_olam_id("OLAM Method-C perimeter W face", iw1, self.nwd)?;
            require_olam_id("OLAM Method-C perimeter W face", iw2, self.nwd)?;

            if edge.im[0] == imstart && nest_wd[iw1].flag() == 0 && nest_wd[iw2].is_subdivided() {
                return Ok((edge.im[1], iu));
            }
            if edge.im[1] == imstart && nest_wd[iw2].flag() == 0 && nest_wd[iw1].is_subdivided() {
                return Ok((edge.im[0], iu));
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C perim_ngr could not advance from M point {imstart}"),
        ))
    }

    fn emit_method_c_tables(
        &self,
        perimeter: &[OlamMethodCPerimeterPoint],
        m_neighbors: &[IcosahedronMPointNeighbors],
        nest_wd: &mut [OlamMethodCNestWd],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
    ) -> io::Result<Self> {
        let radius = active_mesh_radius(self)?;
        let parent_level = child_level - 1;
        require_olam_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;

        let mut iwnew = vec![1usize; self.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=self.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                nest_wd[iw].iw[0] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[1] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[2] = iwnext as isize;
            }
            iwnext += 1;
        }
        let nwd0 = iwnext - 1;

        let mut nest_ud = vec![OlamMethodCNestUd::default(); self.nud + 1];
        let mut iunew = vec![1usize; self.nud + 1];
        let mut iwdiv = vec![false; self.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=self.nud {
            iunew[iu] = iunext;
            let edge = self.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    nest_ud[iu].iu = iunew[iu];
                } else {
                    iunext += 1;
                    nest_ud[iu].iu = iunext;
                }
            }

            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 1;
                        nest_wd[iw].iu[0] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[1] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[2] = iunext;
                    }
                }
            }
            iunext += 1;
        }
        let nud0 = iunext - 1;

        let mut imnew = vec![1usize; self.nmd + 1];
        let mut iudiv = vec![false; self.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=self.nmd {
            imnew[im] = imnext;
            let neighbors = m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                if !iudiv[iu] {
                    iudiv[iu] = true;
                    let edge = self.u_edges[iu];
                    let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                    if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                        if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                            nest_ud[iu].im = 1;
                        } else {
                            imnext += 1;
                            nest_ud[iu].im = imnext;
                        }
                    }
                }
            }
            imnext += 1;
        }
        let nmd0 = imnext - 1;

        let mut impent = [1usize; 12];
        for (slot, &old_im) in self.impent.iter().enumerate() {
            if old_im <= 1 {
                continue;
            }
            require_olam_id("OLAM Method-C impent", old_im, self.nmd)?;
            impent[slot] = imnew[old_im];
        }

        let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); nmd0 + 1];
        let mut m_metadata = default_olam_m_metadata(nmd0);
        let mut u_edges = vec![IcosahedronUEdge::default(); nud0 + 1];
        let mut w_faces = vec![IcosahedronWFace::default(); nwd0 + 1];

        for im in 2..=self.nmd {
            let imn = imnew[im];
            m_points[imn] = self.m_points[im];
            m_metadata[imn] = self.m_metadata[im];
        }

        let mut parent_mrlm = 0usize;
        for iu in 2..=self.nud {
            let iun = iunew[iu];
            let old = self.u_edges[iu];
            u_edges[iun] = IcosahedronUEdge {
                im: old.im.map(|im| imnew[im]),
                iw: old.iw.map(|iw| iwnew[iw]),
                iu: old.iu.map(|iu2| iunew[iu2]),
                mrlu: old.mrlu,
            };

            if nest_ud[iu].im > 1 {
                let im_mid = nest_ud[iu].im;
                let im1 = u_edges[iun].im[0];
                let im2 = u_edges[iun].im[1];
                if parent_mrlm == 0 {
                    parent_mrlm = m_metadata[im1].mrlm;
                }
                let refined_mrlm = parent_mrlm + 1;
                m_points[im_mid] = weighted_point(m_points[im1], 1.0, m_points[im2], 1.0)?;
                m_metadata[im1].mrlm = refined_mrlm;
                m_metadata[im2].mrlm = refined_mrlm;
                m_metadata[im_mid].mrlm = refined_mrlm;
                m_metadata[im_mid].mrlm_orig = refined_mrlm;
                m_metadata[im1].ngr = child_level;
                m_metadata[im2].ngr = child_level;
                m_metadata[im_mid].ngr = child_level;
            }
        }

        let mut parent_mrlw = 0usize;
        for iw in 2..=self.nwd {
            let iwn = iwnew[iw];
            let old = self.w_faces[iw];
            w_faces[iwn] = IcosahedronWFace {
                npoly: old.npoly,
                im: old.im.map(|im| imnew[im]),
                iu: old.iu.map(|iu| iunew[iu]),
                iw: old.iw.map(|iw2| iwnew[iw2]),
                mrlw: old.mrlw,
                mrlw_orig: old.mrlw_orig,
                ngr: old.ngr,
                mrow: old.mrow,
            };

            if nest_wd[iw].is_subdivided() {
                if parent_mrlw == 0 {
                    parent_mrlw = old.mrlw;
                }
                if old.mrlw != parent_mrlw {
                    let center = normalized_face_center(
                        m_points[old.im[0]],
                        m_points[old.im[1]],
                        m_points[old.im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Current nested grid {child_level} crosses the parent boundary / crosses (or is too close to) the next coarser grid boundary at W face {iw} (mrlw={}, lon={:.3}, lat={:.3})",
                            old.mrlw, ll.lon_degrees, ll.lat_degrees
                        ),
                    ));
                }
                self.fill_method_c_full_subdivision(
                    iw,
                    &iwnew,
                    &iunew,
                    &imnew,
                    child_level,
                    nest_wd,
                    &nest_ud,
                    &mut u_edges,
                    &mut w_faces,
                )?;
            }
        }

        let transition_parent_mrlw = if parent_mrlw == 0 {
            parent_level
        } else {
            parent_mrlw
        };
        self.perim_fill3_method_c(
            perimeter,
            transition_parent_mrlw,
            &iwnew,
            &iunew,
            &imnew,
            nest_wd,
            &mut nest_ud,
            &mut u_edges,
            &mut w_faces,
            &mut m_points,
            &mut m_metadata,
            radius,
            child_level,
        )?;

        if project_to_radius {
            for point in m_points.iter_mut().take(nmd0 + 1).skip(2) {
                *point = normalize_cartesian_to_radius(*point, radius)?;
            }
        }

        let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
        derive_icosahedron_w_neighbors_fortran(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive OLAM Method-C W-face neighbors",
            )
        })?;
        derive_icosahedron_u_neighbors_fortran(&mut connectivity).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "failed to derive OLAM Method-C U-edge neighbors",
            )
        })?;
        require_olam_len(
            "OLAM Method-C M prognostic map",
            self.m_prognostic.len(),
            self.nmd + 1,
        )?;
        require_olam_len(
            "OLAM Method-C U prognostic map",
            self.u_prognostic.len(),
            self.nud + 1,
        )?;
        require_olam_len(
            "OLAM Method-C W prognostic map",
            self.w_prognostic.len(),
            self.nwd + 1,
        )?;
        let mut m_prognostic = olam_identity_prognostic_map(nmd0);
        for old_im in 2..=self.nmd {
            let partner = self.m_prognostic[old_im];
            if partner > 1 {
                require_olam_id("OLAM Method-C M prognostic partner", partner, self.nmd)?;
                m_prognostic[imnew[old_im]] = imnew[partner];
            }
        }
        let mut u_prognostic = olam_identity_prognostic_map(nud0);
        for old_iu in 2..=self.nud {
            let partner = self.u_prognostic[old_iu];
            if partner > 1 {
                require_olam_id("OLAM Method-C U prognostic partner", partner, self.nud)?;
                u_prognostic[iunew[old_iu]] = iunew[partner];
            }
        }
        let mut w_prognostic = olam_identity_prognostic_map(nwd0);
        for old_iw in 2..=self.nwd {
            let partner = self.w_prognostic[old_iw];
            if partner > 1 {
                require_olam_id("OLAM Method-C W prognostic partner", partner, self.nwd)?;
                w_prognostic[iwnew[old_iw]] = iwnew[partner];
            }
        }
        let has_prognostic_w_faces = w_prognostic
            .iter()
            .enumerate()
            .skip(2)
            .any(|(iw, &partner)| partner > 1 && partner != iw);
        let m_neighbors = if has_prognostic_w_faces {
            derive_cart_hex_m_neighbors_from_active_faces(
                nmd0,
                &connectivity.u_edges,
                &connectivity.w_faces,
                &w_prognostic,
            )?
        } else {
            derive_icosahedron_m_neighbors_fortran_checked_with_prognostic(
                nmd0,
                &connectivity.u_edges,
                &connectivity.w_faces,
                None,
            )?
        };

        let mut mesh = OlamDelaunayMesh {
            nmd: nmd0,
            nud: nud0,
            nwd: nwd0,
            impent,
            m_points,
            m_metadata,
            u_edges: connectivity.u_edges,
            w_faces: connectivity.w_faces,
            m_neighbors,
            m_prognostic,
            u_prognostic,
            w_prognostic,
            boundary_rows: Vec::new(),
        };
        mesh.apply_olam_perimeter_mrows(child_level, max_mrows)?;
        Ok(mesh)
    }

    fn fill_method_c_full_subdivision(
        &self,
        iw: usize,
        iwnew: &[usize],
        iunew: &[usize],
        imnew: &[usize],
        child_level: usize,
        nest_wd: &[OlamMethodCNestWd],
        nest_ud: &[OlamMethodCNestUd],
        u_edges: &mut [IcosahedronUEdge],
        w_faces: &mut [IcosahedronWFace],
    ) -> io::Result<()> {
        let iwn = iwnew[iw];
        let old_face = self.w_faces[iw];
        let [iu1o, iu2o, iu3o] = old_face.iu;
        let [iu1n, iu2n, iu3n] = [iunew[iu1o], iunew[iu2o], iunew[iu3o]];
        let mrlo = old_face.mrlw;

        let [iu1, iu2, iu3] = nest_wd[iw].iu;
        let iu4 = nest_ud[iu1o].iu;
        let iu5 = nest_ud[iu2o].iu;
        let iu6 = nest_ud[iu3o].iu;
        let iw1 = nest_wd[iw].child_iw(0)?;
        let iw2 = nest_wd[iw].child_iw(1)?;
        let iw3 = nest_wd[iw].child_iw(2)?;

        for child_iw in [iw1, iw2, iw3] {
            w_faces[child_iw].npoly = 3;
            w_faces[child_iw].mrlw = mrlo + 1;
            w_faces[child_iw].mrlw_orig = mrlo + 1;
            w_faces[child_iw].ngr = child_level;
        }
        w_faces[iwn].mrlw = mrlo + 1;
        w_faces[iwn].ngr = child_level;
        w_faces[iwn].iu = [iu1, iu2, iu3];
        w_faces[iw1].iu[0] = iu1;
        w_faces[iw2].iu[0] = iu2;
        w_faces[iw3].iu[0] = iu3;

        if nest_ud[iu1o].im > 1 {
            u_edges[iu1n].im[1] = nest_ud[iu1o].im;
            u_edges[iu4].im[0] = nest_ud[iu1o].im;
            u_edges[iu4].im[1] = imnew[self.u_edges[iu1o].im[1]];
        }
        if nest_ud[iu2o].im > 1 {
            u_edges[iu2n].im[1] = nest_ud[iu2o].im;
            u_edges[iu5].im[0] = nest_ud[iu2o].im;
            u_edges[iu5].im[1] = imnew[self.u_edges[iu2o].im[1]];
        }
        if nest_ud[iu3o].im > 1 {
            u_edges[iu3n].im[1] = nest_ud[iu3o].im;
            u_edges[iu6].im[0] = nest_ud[iu3o].im;
            u_edges[iu6].im[1] = imnew[self.u_edges[iu3o].im[1]];
        }

        let [iu1o_iw1, iu2o_iw1, iu3o_iw1] = [
            self.u_edges[iu1o].iw[0],
            self.u_edges[iu2o].iw[0],
            self.u_edges[iu3o].iw[0],
        ];

        if iw == iu1o_iw1 {
            w_faces[iw3].iu[1] = iu1n;
            w_faces[iw2].iu[2] = iu4;
            u_edges[iu1].im = [nest_ud[iu2o].im, nest_ud[iu3o].im];
            u_edges[iu1].iw = set_first_two(u_edges[iu1].iw, iw1, iwn);
            u_edges[iu1n].iw[0] = iw3;
            u_edges[iu4].iw[0] = iw2;
        } else {
            w_faces[iw3].iu[1] = iu4;
            w_faces[iw2].iu[2] = iu1n;
            u_edges[iu1].im = [nest_ud[iu3o].im, nest_ud[iu2o].im];
            u_edges[iu1].iw = set_first_two(u_edges[iu1].iw, iwn, iw1);
            u_edges[iu1n].iw[1] = iw2;
            u_edges[iu4].iw[1] = iw3;
        }

        if iw == iu2o_iw1 {
            w_faces[iw1].iu[1] = iu2n;
            w_faces[iw3].iu[2] = iu5;
            u_edges[iu2].im = [nest_ud[iu3o].im, nest_ud[iu1o].im];
            u_edges[iu2].iw = set_first_two(u_edges[iu2].iw, iw2, iwn);
            u_edges[iu2n].iw[0] = iw1;
            u_edges[iu5].iw[0] = iw3;
        } else {
            w_faces[iw1].iu[1] = iu5;
            w_faces[iw3].iu[2] = iu2n;
            u_edges[iu2].im = [nest_ud[iu1o].im, nest_ud[iu3o].im];
            u_edges[iu2].iw = set_first_two(u_edges[iu2].iw, iwn, iw2);
            u_edges[iu2n].iw[1] = iw3;
            u_edges[iu5].iw[1] = iw1;
        }

        if iw == iu3o_iw1 {
            w_faces[iw2].iu[1] = iu3n;
            w_faces[iw1].iu[2] = iu6;
            u_edges[iu3].im = [nest_ud[iu1o].im, nest_ud[iu2o].im];
            u_edges[iu3].iw = set_first_two(u_edges[iu3].iw, iw3, iwn);
            u_edges[iu3n].iw[0] = iw2;
            u_edges[iu6].iw[0] = iw1;
        } else {
            w_faces[iw2].iu[1] = iu6;
            w_faces[iw1].iu[2] = iu3n;
            u_edges[iu3].im = [nest_ud[iu2o].im, nest_ud[iu1o].im];
            u_edges[iu3].iw = set_first_two(u_edges[iu3].iw, iwn, iw3);
            u_edges[iu3n].iw[1] = iw1;
            u_edges[iu6].iw[1] = iw2;
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn perim_fill3_method_c(
        &self,
        perimeter: &[OlamMethodCPerimeterPoint],
        parent_level: usize,
        iwnew: &[usize],
        iunew: &[usize],
        imnew: &[usize],
        nest_wd: &[OlamMethodCNestWd],
        nest_ud: &mut [OlamMethodCNestUd],
        u_edges: &mut [IcosahedronUEdge],
        w_faces: &mut [IcosahedronWFace],
        m_points: &mut [CartesianPoint],
        m_metadata: &mut [IcosahedronMPointMetadata],
        radius: f64,
        child_level: usize,
    ) -> io::Result<()> {
        for triple in perimeter.chunks_exact(3) {
            let [p1, p2, p3] = [triple[0], triple[1], triple[2]];
            let [jm1, jm2, jm3] = [p1.im, p2.im, p3.im];
            let [ju1, ju2, ju3] = [p1.iu, p2.iu, p3.iu];

            let (iu41, iu42, iu46, iw26, iw27) = if jm1 == self.u_edges[ju1].im[0] {
                (
                    iunew[ju1],
                    nest_ud[ju1].iu,
                    iunew[self.u_edges[ju1].iu[4]],
                    iwnew[self.u_edges[ju1].iw[2]],
                    iwnew[self.u_edges[ju1].iw[0]],
                )
            } else {
                (
                    nest_ud[ju1].iu,
                    iunew[ju1],
                    iunew[self.u_edges[ju1].iu[11]],
                    iwnew[self.u_edges[ju1].iw[5]],
                    iwnew[self.u_edges[ju1].iw[1]],
                )
            };

            let (iu49, iu50, iu34, iu35, iu48, iu51, iw6o, iw9o, iw6, iw9, iw29, iw20, iw28, iw30) =
                if jm2 == self.u_edges[ju2].im[0] {
                    (
                        iunew[self.u_edges[ju2].iu[0]],
                        iunew[self.u_edges[ju2].iu[1]],
                        iunew[self.u_edges[ju2].iu[2]],
                        iunew[self.u_edges[ju2].iu[3]],
                        iunew[self.u_edges[ju2].iu[4]],
                        iunew[self.u_edges[ju2].iu[7]],
                        self.u_edges[ju2].iw[4],
                        self.u_edges[ju2].iw[5],
                        iwnew[self.u_edges[ju2].iw[4]],
                        iwnew[self.u_edges[ju2].iw[5]],
                        iwnew[self.u_edges[ju2].iw[0]],
                        iwnew[self.u_edges[ju2].iw[1]],
                        iwnew[self.u_edges[ju2].iw[2]],
                        iwnew[self.u_edges[ju2].iw[3]],
                    )
                } else {
                    (
                        iunew[self.u_edges[ju2].iu[3]],
                        iunew[self.u_edges[ju2].iu[2]],
                        iunew[self.u_edges[ju2].iu[1]],
                        iunew[self.u_edges[ju2].iu[0]],
                        iunew[self.u_edges[ju2].iu[11]],
                        iunew[self.u_edges[ju2].iu[8]],
                        self.u_edges[ju2].iw[3],
                        self.u_edges[ju2].iw[2],
                        iwnew[self.u_edges[ju2].iw[3]],
                        iwnew[self.u_edges[ju2].iw[2]],
                        iwnew[self.u_edges[ju2].iw[1]],
                        iwnew[self.u_edges[ju2].iw[0]],
                        iwnew[self.u_edges[ju2].iw[5]],
                        iwnew[self.u_edges[ju2].iw[4]],
                    )
                };

            let (im21, iu44, iu45, iu53, iw31, iw32) = if jm3 == self.u_edges[ju3].im[0] {
                (
                    imnew[self.u_edges[ju3].im[1]],
                    iunew[ju3],
                    nest_ud[ju3].iu,
                    iunew[self.u_edges[ju3].iu[7]],
                    iwnew[self.u_edges[ju3].iw[0]],
                    iwnew[self.u_edges[ju3].iw[3]],
                )
            } else {
                (
                    imnew[self.u_edges[ju3].im[0]],
                    nest_ud[ju3].iu,
                    iunew[ju3],
                    iunew[self.u_edges[ju3].iu[8]],
                    iwnew[self.u_edges[ju3].iw[1]],
                    iwnew[self.u_edges[ju3].iw[4]],
                )
            };

            let im16 = imnew[jm1];
            let im17 = nest_ud[ju1].im;
            let im18 = imnew[jm2];
            let im19 = imnew[jm3];
            let im20 = nest_ud[ju3].im;
            let iu43 = iunew[ju2];

            let [iu25, iu15] = method_c_split_outer_edges(nest_wd[iw6o].iu, u_edges, "iw6")?;
            let iw7 = other_edge_face(u_edges[iu15], iw6)?;
            let (iw19, im12) = if u_edges[iu25].iw[0] == iw6 {
                (u_edges[iu25].iw[1], u_edges[iu25].im[1])
            } else {
                (u_edges[iu25].iw[0], u_edges[iu25].im[0])
            };

            let [iu16, iu26] = method_c_split_outer_edges(nest_wd[iw9o].iu, u_edges, "iw9")?;
            let iw8 = other_edge_face(u_edges[iu16], iw9)?;
            let (iw21, im13) = if u_edges[iu26].iw[0] == iw9 {
                (u_edges[iu26].iw[1], u_edges[iu26].im[0])
            } else {
                (u_edges[iu26].iw[0], u_edges[iu26].im[1])
            };

            let im22 = fortran_other_endpoint_by_first(u_edges[iu46], im16);
            let im23 = fortran_other_endpoint_by_first(u_edges[iu48], im18);
            let im24 = fortran_other_endpoint_by_first(u_edges[iu49], im18);
            let im25 = fortran_other_endpoint_by_first(u_edges[iu51], im19);
            let im26 = fortran_other_endpoint_by_first(u_edges[iu53], im21);

            fill_missing_endpoint(&mut u_edges[iu15], im18);
            fill_missing_endpoint(&mut u_edges[iu16], im18);
            fill_missing_endpoint(&mut u_edges[iu25], im18);
            fill_missing_endpoint(&mut u_edges[iu26], im18);

            let im5 = if u_edges[iu34].im[0] == im18 {
                u_edges[iu34].iw = set_first_two(u_edges[iu34].iw, iw8, iw7);
                u_edges[iu34].im[1]
            } else {
                u_edges[iu34].iw = set_first_two(u_edges[iu34].iw, iw7, iw8);
                u_edges[iu34].im[0]
            };

            if u_edges[iu35].im[0] == im19 {
                u_edges[iu35].iw[1] = iw19;
                u_edges[iu35].iw[0] = iw21;
                u_edges[iu35].im[1] = im18;
            } else {
                u_edges[iu35].iw[0] = iw19;
                u_edges[iu35].iw[1] = iw21;
                u_edges[iu35].im[0] = im18;
            }

            if u_edges[iu41].im[1] == im17 {
                u_edges[iu41].iw[0] = iw27;
            } else {
                u_edges[iu41].iw[1] = iw27;
            }
            if u_edges[iu42].im[0] == im17 {
                u_edges[iu42].im[1] = im19;
                u_edges[iu42].iw[0] = iw20;
            } else {
                u_edges[iu42].im[0] = im19;
                u_edges[iu42].iw[1] = iw20;
            }
            if u_edges[iu43].im[1] == im19 {
                u_edges[iu43].im[0] = im24;
            } else {
                u_edges[iu43].im[1] = im24;
            }
            if u_edges[iu44].im[0] == im19 {
                u_edges[iu44].iw[0] = iw29;
            } else {
                u_edges[iu44].iw[1] = iw29;
            }
            if u_edges[iu45].im[0] == im20 {
                u_edges[iu45].iw[0] = iw31;
            } else {
                u_edges[iu45].iw[1] = iw31;
            }
            if u_edges[iu48].iw[1] == iw27 {
                u_edges[iu48].im[1] = im17;
            } else {
                u_edges[iu48].im[0] = im17;
            }
            if u_edges[iu49].im[1] == im24 {
                u_edges[iu49].im[0] = im17;
                u_edges[iu49].iw[1] = iw20;
            } else {
                u_edges[iu49].im[1] = im17;
                u_edges[iu49].iw[0] = iw20;
            }
            if u_edges[iu50].im[0] == im24 {
                u_edges[iu50].im[1] = im20;
            } else {
                u_edges[iu50].im[0] = im20;
            }
            if u_edges[iu51].iw[1] == iw31 {
                u_edges[iu51].im[0] = im20;
            } else {
                u_edges[iu51].im[1] = im20;
            }

            replace_w_face_edge_after(w_faces, iw8, iu16, iu34, "iw8/iu16->iu34")?;
            let iu33 =
                replace_w_face_edge_with_side_return(w_faces, iw19, iu25, iu35, "iw19/iu25->iu35")?;
            if u_edges[iu33].iw[1] == iw19 {
                u_edges[iu33].im[1] = im19;
            } else {
                u_edges[iu33].im[0] = im19;
            }

            replace_w_face_edges_at(w_faces, iw20, iu43, [iu42, iu49], "iw20/iu43")?;
            replace_w_face_edge_before(w_faces, iw27, iu48, iu41, "iw27/iu48->iu41")?;
            replace_w_face_edges_at(w_faces, iw29, iu50, [iu44, iu43], "iw29/iu50")?;
            replace_w_face_edge_after(w_faces, iw31, iu51, iu45, "iw31/iu51->iu45")?;

            for im in [im22, im23, im24, im25, im26] {
                m_metadata[im].ngr = child_level;
            }
            let transition_w_faces = [iw20, iw26, iw27, iw28, iw29, iw30, iw31, iw32];
            for iw in transition_w_faces.iter().copied() {
                w_faces[iw].ngr = child_level;
            }
            for iw in transition_w_faces {
                if w_faces[iw].mrlw != parent_level {
                    let center = normalized_face_center(
                        m_points[w_faces[iw].im[0]],
                        m_points[w_faces[iw].im[1]],
                        m_points[w_faces[iw].im[2]],
                        radius,
                    )?;
                    let ll = xyz_to_lonlat_degrees(center);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                            "Method-C perimeter length invalid: Current nested grid {child_level} crosses the parent boundary in Method-C transition at W face {iw} (mrlw={}, lon={:.3}, lat={:.3})",
                            w_faces[iw].mrlw,
                            ll.lon_degrees,
                            ll.lat_degrees
                        ),
                    ));
                }
            }

            m_metadata[im17].mrlm_orig = m_metadata[im18].mrlm_orig;
            m_metadata[im20].mrlm_orig = m_metadata[im19].mrlm_orig;
            m_metadata[im18].mrlm_orig = parent_level + 1;
            m_metadata[im19].mrlm_orig = parent_level + 1;

            m_points[im19] = weighted_point(m_points[im24], 1.0, m_points[im5], 1.0)?;
            m_points[im18] = weighted_point(m_points[im19], 1.0, m_points[im5], 1.0)?;
            m_points[im17] = weighted_point(m_points[im17], 0.75, m_points[im19], 0.25)?;
            m_points[im20] = weighted_point(m_points[im20], 0.75, m_points[im19], 0.25)?;
            m_points[im12] = weighted_point(m_points[im12], 0.833, m_points[im18], 0.167)?;
            m_points[im13] = weighted_point(m_points[im13], 0.833, m_points[im18], 0.167)?;
        }

        Ok(())
    }

    fn apply_olam_perimeter_mrows(&mut self, ngr: usize, max_mrows: usize) -> io::Result<()> {
        self.validate_topology()?;
        if ngr <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM perimeter mrow NGR must be greater than one",
            ));
        }

        let mut mrow_temp = vec![0isize; self.nwd + 1];
        let mut mrow_temp2 = vec![0isize; self.nwd + 1];

        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            let [iw1, iw2, iw3] = [face.iw[0], face.iw[1], face.iw[2]];
            require_olam_id("OLAM perimeter W neighbor", iw1, self.nwd)?;
            require_olam_id("OLAM perimeter W neighbor", iw2, self.nwd)?;
            require_olam_id("OLAM perimeter W neighbor", iw3, self.nwd)?;

            if face.ngr == ngr {
                if face.mrlw < self.w_faces[iw1].mrlw
                    || face.mrlw < self.w_faces[iw2].mrlw
                    || face.mrlw < self.w_faces[iw3].mrlw
                {
                    mrow_temp[iw] = 1;
                } else if face.mrlw > self.w_faces[iw1].mrlw
                    || face.mrlw > self.w_faces[iw2].mrlw
                    || face.mrlw > self.w_faces[iw3].mrlw
                {
                    mrow_temp[iw] = -1;
                }
            }
        }

        mrow_temp2.clone_from(&mrow_temp);
        for irow in 2..=(2 * max_mrows) {
            let jrow = (irow % 2) as isize;
            for iw in 2..=self.nwd {
                if mrow_temp[iw] != 0 {
                    continue;
                }

                let [iw1, iw2, iw3] = [
                    self.w_faces[iw].iw[0],
                    self.w_faces[iw].iw[1],
                    self.w_faces[iw].iw[2],
                ];
                require_olam_id("OLAM perimeter W neighbor", iw1, self.nwd)?;
                require_olam_id("OLAM perimeter W neighbor", iw2, self.nwd)?;
                require_olam_id("OLAM perimeter W neighbor", iw3, self.nwd)?;

                let positive_row = mrow_temp[iw1].max(mrow_temp[iw2]).max(mrow_temp[iw3]);
                if positive_row > 0 {
                    mrow_temp2[iw] = positive_row + jrow;
                }

                let negative_row = mrow_temp[iw1].min(mrow_temp[iw2]).min(mrow_temp[iw3]);
                if negative_row < 0 {
                    mrow_temp2[iw] = negative_row - jrow;
                }
            }
            mrow_temp.clone_from(&mrow_temp2);
        }

        let mut boundary_rows = Vec::new();
        for iw in 2..=self.nwd {
            let row = mrow_temp[iw];
            if row == 0 {
                continue;
            }

            let old_row = self.w_faces[iw].mrow;
            if row < 2 && old_row != 0 && old_row > -3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Current nested grid {ngr} crosses the parent boundary in Method-C mrow at W face {iw} (new mrow={row}, old mrow={old_row})"
                    ),
                ));
            }
            if old_row == 0 || old_row < -2 {
                self.w_faces[iw].mrow = row;
            }
            self.w_faces[iw].ngr = ngr;
            boundary_rows.push(iw);
        }

        for im in 2..=self.nmd {
            let mut on_grid = false;
            for &iw in self.m_neighbors[im].iw.iter().take(self.m_neighbors[im].npoly) {
                require_olam_id("OLAM perimeter M W neighbor", iw, self.nwd)?;
                if self.w_faces[iw].ngr == ngr {
                    on_grid = true;
                    break;
                }
            }
            if on_grid {
                self.m_metadata[im].ngr = ngr;
            }
        }

        self.boundary_rows = boundary_rows;
        self.validate_topology()?;
        Ok(())
    }

    /// Port of OLAM `expand_global2`: insert one M point on every active
    /// Delaunay edge and subdivide every triangular W face into four children.
    ///
    /// The Fortran routine preserves/copies many atmosphere-loop fields while
    /// rebuilding the same triangular topology. This Rust path keeps the mesh
    /// fields currently owned by `earthmesh_mesh`, then performs a full M/U/W
    /// neighbor rebuild rather than depending on local edge-number patches.
    pub fn expand_global2(&self) -> io::Result<Self> {
        self.validate_topology()?;

        let radius = active_mesh_radius(self)?;
        let mut m_points = self.m_points.clone();
        let mut midpoint_by_edge = BTreeMap::new();

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            let midpoint = CartesianPoint::new(
                0.5 * (self.m_points[im1].x + self.m_points[im2].x),
                0.5 * (self.m_points[im1].y + self.m_points[im2].y),
                0.5 * (self.m_points[im1].z + self.m_points[im2].z),
            );
            let midpoint = normalize_cartesian_to_radius(midpoint, radius)?;
            let midpoint_id = m_points.len();
            m_points.push(midpoint);
            midpoint_by_edge.insert(olam_edge_key(im1, im2), midpoint_id);
        }

        let mut child_faces = Vec::with_capacity((self.nwd - 1) * 4);
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            let [a, b, c] = face.im;
            let ab = lookup_olam_midpoint(&midpoint_by_edge, a, b, iw)?;
            let bc = lookup_olam_midpoint(&midpoint_by_edge, b, c, iw)?;
            let ca = lookup_olam_midpoint(&midpoint_by_edge, c, a, iw)?;
            let metadata = (face.mrlw, face.mrlw_orig, face.ngr);

            child_faces.push(OlamTriangleSeed::new([a, ab, ca], metadata).with_mrow(face.mrow));
            child_faces.push(OlamTriangleSeed::new([b, bc, ab], metadata).with_mrow(face.mrow));
            child_faces.push(OlamTriangleSeed::new([c, ca, bc], metadata).with_mrow(face.mrow));
            child_faces.push(OlamTriangleSeed::new([ab, bc, ca], metadata).with_mrow(face.mrow));
        }

        olam_mesh_from_triangle_seeds(m_points.len() - 1, self.impent, m_points, &child_faces)
    }

    /// Apply OLAM global expansion factors in the same 3-first, then 2-second
    /// order used by OLAM `expand_delaunay_mesh`.
    pub fn expand_by_factor(&self, factor: usize) -> io::Result<Self> {
        if factor == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "OLAM expansion factor must be positive",
            ));
        }

        let mut reduced = factor;
        while reduced % 3 == 0 {
            reduced /= 3;
        }
        while reduced % 2 == 0 {
            reduced /= 2;
        }
        if reduced != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("OLAM expansion factor {factor} must contain only factors of 2 and 3"),
            ));
        }

        let mut expanded = self.clone();
        let mut remaining = factor;
        while remaining % 3 == 0 {
            expanded = expanded.expand_global3()?;
            remaining /= 3;
        }
        while remaining > 1 {
            expanded = expanded.expand_global2()?;
            remaining /= 2;
        }
        Ok(expanded)
    }

    /// Port of OLAM `expand_global3`: insert two M points on every active
    /// Delaunay edge, one M point inside every active W face, and subdivide
    /// each triangular face into nine children.
    pub fn expand_global3(&self) -> io::Result<Self> {
        self.validate_topology()?;

        let radius = active_mesh_radius(self)?;
        let mut m_points = self.m_points.clone();
        let mut thirds_by_edge = BTreeMap::new();

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            let point1 = self.m_points[im1];
            let point2 = self.m_points[im2];
            let first_from_im1 = normalized_weighted_point(point1, 2.0, point2, 1.0, radius)?;
            let second_from_im1 = normalized_weighted_point(point1, 1.0, point2, 2.0, radius)?;
            let first_id = m_points.len();
            m_points.push(first_from_im1);
            let second_id = m_points.len();
            m_points.push(second_from_im1);
            let ids_from_low_to_high = if im1 <= im2 {
                [first_id, second_id]
            } else {
                [second_id, first_id]
            };
            thirds_by_edge.insert(olam_edge_key(im1, im2), ids_from_low_to_high);
        }

        let mut child_faces = Vec::with_capacity((self.nwd - 1) * 9);
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            let [a, b, c] = face.im;
            let [ab1, ab2] = lookup_olam_thirds(&thirds_by_edge, a, b, iw)?;
            let [bc1, bc2] = lookup_olam_thirds(&thirds_by_edge, b, c, iw)?;
            let [ac1, ac2] = lookup_olam_thirds(&thirds_by_edge, a, c, iw)?;
            let center = normalized_face_center(
                self.m_points[a],
                self.m_points[b],
                self.m_points[c],
                radius,
            )?;
            let center_id = m_points.len();
            m_points.push(center);
            let metadata = (face.mrlw, face.mrlw_orig, face.ngr);

            child_faces.push(OlamTriangleSeed::new([a, ab1, ac1], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([ab1, ab2, center_id], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([ac1, center_id, ac2], metadata).with_mrow(face.mrow));
            child_faces.push(OlamTriangleSeed::new([ab2, b, bc1], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([center_id, bc1, bc2], metadata).with_mrow(face.mrow));
            child_faces.push(OlamTriangleSeed::new([ac2, bc2, c], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([center_id, ac1, ab1], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([bc1, center_id, ab2], metadata).with_mrow(face.mrow));
            child_faces
                .push(OlamTriangleSeed::new([bc2, ac2, center_id], metadata).with_mrow(face.mrow));
        }

        olam_mesh_from_triangle_seeds(m_points.len() - 1, self.impent, m_points, &child_faces)
    }

    /// Check reciprocal `M/U/W` topology invariants for the active OLAM slots.
    ///
    /// Slot `0` is Rust's unused vector slot and slot `1` mirrors OLAM's
    /// sentinel record. Active records are `2..=nmd`, `2..=nud`, and `2..=nwd`.
    pub fn validate_topology(&self) -> io::Result<OlamTopologyValidation> {
        require_olam_len("m_points", self.m_points.len(), self.nmd + 1)?;
        require_olam_len("u_edges", self.u_edges.len(), self.nud + 1)?;
        require_olam_len("w_faces", self.w_faces.len(), self.nwd + 1)?;
        require_olam_len("m_neighbors", self.m_neighbors.len(), self.nmd + 1)?;
        require_olam_len("m_prognostic", self.m_prognostic.len(), self.nmd + 1)?;
        require_olam_len("u_prognostic", self.u_prognostic.len(), self.nud + 1)?;
        require_olam_len("w_prognostic", self.w_prognostic.len(), self.nwd + 1)?;

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            require_olam_id("U edge M endpoint", im1, self.nmd)?;
            require_olam_id("U edge M endpoint", im2, self.nmd)?;
            if im1 == im2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("U edge {iu} has duplicate M endpoints {im1}"),
                ));
            }

            let adjacent_faces = [edge.iw[0], edge.iw[1]];
            if adjacent_faces[0] == adjacent_faces[1] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "U edge {iu} has duplicate adjacent W face {}",
                        adjacent_faces[0]
                    ),
                ));
            }
            for &iw in &adjacent_faces {
                require_olam_id("U edge adjacent W face", iw, self.nwd)?;
                let w_partner = self.w_prognostic[iw];
                if w_partner > 1 && w_partner != iw {
                    require_olam_id("U edge periodic W face partner", w_partner, self.nwd)?;
                    continue;
                }
                let face = self.w_faces[iw];
                if !face.iu.contains(&iu) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "U edge {iu} points to W face {iw}, but the face does not point back"
                        ),
                    ));
                }
                if !face.im.contains(&im1) || !face.im.contains(&im2) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("U edge {iu} endpoints [{im1}, {im2}] are not both on W face {iw}"),
                    ));
                }
            }
        }

        for iw in 2..=self.nwd {
            let w_partner = self.w_prognostic[iw];
            if w_partner > 1 && w_partner != iw {
                require_olam_id("periodic W face partner", w_partner, self.nwd)?;
                continue;
            }
            let face = self.w_faces[iw];
            if face.npoly != 3 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("W face {iw} must be triangular, got npoly {}", face.npoly),
                ));
            }
            require_unique_active_triplet("W face M vertices", iw, face.im, self.nmd)?;
            require_unique_active_triplet("W face U edges", iw, face.iu, self.nud)?;

            for &iu in &face.iu {
                let edge = self.u_edges[iu];
                if edge.iw[0] != iw && edge.iw[1] != iw {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "W face {iw} points to U edge {iu}, but the edge does not point back"
                        ),
                    ));
                }
                if !face.im.contains(&edge.im[0]) || !face.im.contains(&edge.im[1]) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("W face {iw} references U edge {iu} outside its M vertices"),
                    ));
                }
            }
        }

        for im in 2..=self.nmd {
            let m_partner = self.m_prognostic[im];
            if m_partner > 1 && m_partner != im {
                require_olam_id("periodic M point partner", m_partner, self.nmd)?;
                continue;
            }
            let neighbors = self.m_neighbors[im];
            if !(3..=7).contains(&neighbors.npoly) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("M point {im} has invalid npoly {}", neighbors.npoly),
                ));
            }
            for j in 0..neighbors.npoly {
                let iu = neighbors.iu[j];
                let iw = neighbors.iw[j];
                require_olam_id("M point U edge", iu, self.nud)?;
                require_olam_id("M point W face", iw, self.nwd)?;
                if !self.u_edges[iu].im.contains(&im) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "M point {im} points to U edge {iu}, but the edge does not point back"
                        ),
                    ));
                }
                if !self.w_faces[iw].im.contains(&im) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "M point {im} points to W face {iw}, but the face does not point back"
                        ),
                    ));
                }
            }
        }

        Ok(OlamTopologyValidation {
            checked_m_points: self.nmd.saturating_sub(1),
            checked_u_edges: self.nud.saturating_sub(1),
            checked_w_faces: self.nwd.saturating_sub(1),
        })
    }
}

fn set_first_two(mut values: [usize; 6], first: usize, second: usize) -> [usize; 6] {
    values[0] = first;
    values[1] = second;
    values
}

fn other_edge_face(edge: IcosahedronUEdge, iw: usize) -> io::Result<usize> {
    if edge.iw[0] == iw {
        Ok(edge.iw[1])
    } else if edge.iw[1] == iw {
        Ok(edge.iw[0])
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C edge does not touch W face {iw}"),
        ))
    }
}

fn fortran_other_endpoint_by_first(edge: IcosahedronUEdge, im: usize) -> usize {
    if edge.im[0] == im {
        edge.im[1]
    } else {
        edge.im[0]
    }
}

fn fill_missing_endpoint(edge: &mut IcosahedronUEdge, im: usize) {
    if edge.im[0] == 1 {
        edge.im[0] = im;
    } else {
        edge.im[1] = im;
    }
}

fn method_c_split_outer_edges(
    candidates: [usize; 3],
    u_edges: &[IcosahedronUEdge],
    label: &str,
) -> io::Result<[usize; 2]> {
    let [ku1, ku2, ku3] = candidates;
    for (solid, first_open, second_open) in [(ku1, ku2, ku3), (ku2, ku3, ku1), (ku3, ku1, ku2)] {
        let edge = u_edges.get(solid).copied().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM Method-C {label} candidate U edge {solid} is out of range"),
            )
        })?;
        if edge.im[0] > 1 && edge.im[1] > 1 {
            return Ok([first_open, second_open]);
        }
    }
    let edge_summary = [ku1, ku2, ku3]
        .map(|iu| {
            u_edges
                .get(iu)
                .map(|edge| format!("{iu}:{:?}", edge.im))
                .unwrap_or_else(|| format!("{iu}:<missing>"))
        })
        .join(", ");
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "OLAM Method-C {label} transition patch has no solid split edge ({edge_summary})"
        ),
    ))
}

fn replace_w_face_edge_after(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[2] = new_iu;
    } else if face.iu[1] == old_iu {
        face.iu[0] = new_iu;
    } else {
        face.iu[1] = new_iu;
    }
    Ok(())
}

fn replace_w_face_edge_before(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[1] = new_iu;
    } else if face.iu[1] == old_iu {
        face.iu[2] = new_iu;
    } else {
        face.iu[0] = new_iu;
    }
    Ok(())
}

fn replace_w_face_edge_with_side_return(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    new_iu: usize,
    _label: &str,
) -> io::Result<usize> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        let side = face.iu[1];
        face.iu[2] = new_iu;
        Ok(side)
    } else if face.iu[1] == old_iu {
        let side = face.iu[2];
        face.iu[0] = new_iu;
        Ok(side)
    } else {
        let side = face.iu[0];
        face.iu[1] = new_iu;
        Ok(side)
    }
}

fn replace_w_face_edges_at(
    w_faces: &mut [IcosahedronWFace],
    iw: usize,
    old_iu: usize,
    replacements: [usize; 2],
    _label: &str,
) -> io::Result<()> {
    let face = w_faces.get_mut(iw).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("OLAM Method-C W face {iw} is out of range"),
        )
    })?;
    if face.iu[0] == old_iu {
        face.iu[1] = replacements[0];
        face.iu[2] = replacements[1];
    } else if face.iu[1] == old_iu {
        face.iu[2] = replacements[0];
        face.iu[0] = replacements[1];
    } else {
        face.iu[0] = replacements[0];
        face.iu[1] = replacements[1];
    }
    Ok(())
}

impl OlamTriangleSeed {
    fn new(im: [usize; 3], metadata: (usize, usize, usize)) -> Self {
        Self {
            im,
            mrlw: metadata.0,
            mrlw_orig: metadata.1,
            ngr: metadata.2,
            mrow: 0,
            target_iw: 0,
            target_iu: [0; 3],
        }
    }

    fn with_mrow(mut self, mrow: isize) -> Self {
        self.mrow = mrow;
        self
    }

    fn with_target_iw(mut self, target_iw: usize) -> Self {
        self.target_iw = target_iw;
        self
    }
}

fn validate_lonlat(point: LonLatDegrees) -> io::Result<()> {
    if !point.lon_degrees.is_finite()
        || !point.lat_degrees.is_finite()
        || point.lat_degrees < -90.0
        || point.lat_degrees > 90.0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid lon/lat point {:?}", point),
        ));
    }
    Ok(())
}

fn validate_positive_distance(name: &str, value: f64) -> io::Result<()> {
    if !value.is_finite() || value <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be positive and finite"),
        ));
    }
    Ok(())
}

fn active_mesh_radius(mesh: &OlamDelaunayMesh) -> io::Result<f64> {
    for point in mesh.m_points.iter().skip(2) {
        let radius = magnitude(*point);
        if radius.is_finite() && radius > 0.0 {
            return Ok(radius);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "OLAM mesh has no active point with a positive radius",
    ))
}

fn push_usize_fields<const N: usize>(output: &mut String, label: &str, values: &[usize; N]) {
    output.push_str(label);
    for value in values {
        output.push_str(&format!(" {value}"));
    }
}

fn euclidean_distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(vector_between(a, b))
}

fn olam_ec_ps_distance_meters(point: CartesianPoint, pole: LonLatDegrees, radius: f64) -> f64 {
    let projected = olam_ec_ps_project_fortran_real(point, pole, radius);
    projected.x.hypot(projected.y)
}

fn olam_ec_ps_project_fortran_real(
    point: CartesianPoint,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let radius = radius as f32;
    let point_radius =
        ((point.x as f32).powi(2) + (point.y as f32).powi(2) + (point.z as f32).powi(2)).sqrt();
    if point_radius == 0.0 {
        return PlanePoint::new(f64::INFINITY, f64::INFINITY);
    }
    let scale = radius / point_radius;
    let xeq = point.x as f32 * scale;
    let yeq = point.y as f32 * scale;
    let zeq = point.z as f32 * scale;
    let pole_lat = deg_to_rad(pole.lat_degrees) as f32;
    let pole_lon = deg_to_rad(pole.lon_degrees) as f32;
    let sinplat = pole_lat.sin();
    let cosplat = pole_lat.cos();
    let sinplon = pole_lon.sin();
    let cosplon = pole_lon.cos();

    let xep = radius * cosplat * cosplon;
    let yep = radius * cosplat * sinplon;
    let zep = radius * sinplat;
    let dxe = xeq - xep;
    let dye = yeq - yep;
    let dze = zeq - zep;

    let xq = -sinplon * dxe + cosplon * dye;
    let yq = cosplat * dze - sinplat * (cosplon * dxe + sinplon * dye);
    let zq = sinplat * dze + cosplat * (cosplon * dxe + sinplon * dye);
    let earth_diameter = 2.0 * radius;
    let t = earth_diameter / (earth_diameter + zq).max(1.0);

    PlanePoint::new((xq * t) as f64, (yq * t) as f64)
}

fn olam_ll_ps_project_fortran_real(
    point: LonLatDegrees,
    pole: LonLatDegrees,
    radius: f64,
) -> PlanePoint {
    let radius = radius as f32;
    let qlat = deg_to_rad(point.lat_degrees) as f32;
    let qlon = deg_to_rad(point.lon_degrees) as f32;
    let cartesian = CartesianPoint::new(
        (radius * qlat.cos() * qlon.cos()) as f64,
        (radius * qlat.cos() * qlon.sin()) as f64,
        (radius * qlat.sin()) as f64,
    );
    olam_ec_ps_project_fortran_real(cartesian, pole, radius as f64)
}

fn plane_segment_distance_fortran_real(
    point: PlanePoint,
    start: PlanePoint,
    end: PlanePoint,
) -> (f64, f64) {
    let x0 = point.x as f32;
    let y0 = point.y as f32;
    let x1 = start.x as f32;
    let y1 = start.y as f32;
    let x2 = end.x as f32;
    let y2 = end.y as f32;
    let dx = x2 - x1;
    let dy = y2 - y1;
    let xp = x0 - x1;
    let yp = y0 - y1;
    let t = ((xp * dx + yp * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    let dist = ((xp - t * dx).powi(2) + (yp - t * dy).powi(2)).sqrt();
    (dist as f64, t as f64)
}

fn olam_corridor_segment_distance_meters(
    point: CartesianPoint,
    start: LonLatDegrees,
    end: LonLatDegrees,
    radius: f64,
) -> (f64, f64) {
    let mut segment_lon = 0.5 * (start.lon_degrees + end.lon_degrees);
    if (start.lon_degrees - end.lon_degrees).abs() > 180.0 {
        if segment_lon <= 0.0 {
            segment_lon += 180.0;
        } else {
            segment_lon -= 180.0;
        }
    }
    let pole = LonLatDegrees::new(segment_lon, 0.5 * (start.lat_degrees + end.lat_degrees));
    let a = olam_ll_ps_project_fortran_real(start, pole, radius);
    let b = olam_ll_ps_project_fortran_real(end, pole, radius);
    let p = olam_ec_ps_project_fortran_real(point, pole, radius);
    plane_segment_distance_fortran_real(p, a, b)
}

fn plane_segment_distance(point: PlanePoint, start: PlanePoint, end: PlanePoint) -> (f64, f64) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let denom = dx * dx + dy * dy;
    let t = if denom == 0.0 {
        0.0
    } else {
        (((point.x - start.x) * dx + (point.y - start.y) * dy) / denom).clamp(0.0, 1.0)
    };
    let closest_x = start.x + t * dx;
    let closest_y = start.y + t * dy;
    ((point.x - closest_x).hypot(point.y - closest_y), t)
}

fn face_following_two_vertices(
    face: IcosahedronWFace,
    im: usize,
    iw: usize,
) -> io::Result<(usize, usize)> {
    if face.im[0] == im {
        Ok((face.im[1], face.im[2]))
    } else if face.im[1] == im {
        Ok((face.im[2], face.im[0]))
    } else if face.im[2] == im {
        Ok((face.im[0], face.im[1]))
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} vertices {:?} do not contain M point {im}",
                face.im
            ),
        ))
    }
}

fn face_following_vertex(face: IcosahedronWFace, im: usize, iw: usize) -> io::Result<usize> {
    if face.im[0] == im {
        Ok(face.im[1])
    } else if face.im[1] == im {
        Ok(face.im[2])
    } else if face.im[2] == im {
        Ok(face.im[0])
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} vertices {:?} do not contain M point {im}",
                face.im
            ),
        ))
    }
}

fn olam_edge_key(im1: usize, im2: usize) -> (usize, usize) {
    if im1 <= im2 {
        (im1, im2)
    } else {
        (im2, im1)
    }
}

fn lookup_olam_midpoint(
    midpoint_by_edge: &BTreeMap<(usize, usize), usize>,
    im1: usize,
    im2: usize,
    owner_iw: usize,
) -> io::Result<usize> {
    midpoint_by_edge
        .get(&olam_edge_key(im1, im2))
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("W face {owner_iw} references edge [{im1}, {im2}] without a midpoint"),
            )
        })
}

fn lookup_olam_thirds(
    thirds_by_edge: &BTreeMap<(usize, usize), [usize; 2]>,
    im1: usize,
    im2: usize,
    owner_iw: usize,
) -> io::Result<[usize; 2]> {
    let points_from_low_to_high = thirds_by_edge
        .get(&olam_edge_key(im1, im2))
        .copied()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("W face {owner_iw} references edge [{im1}, {im2}] without thirds"),
            )
        })?;
    if im1 <= im2 {
        Ok(points_from_low_to_high)
    } else {
        Ok([points_from_low_to_high[1], points_from_low_to_high[0]])
    }
}

fn normalized_weighted_point(
    point1: CartesianPoint,
    weight1: f64,
    point2: CartesianPoint,
    weight2: f64,
    radius: f64,
) -> io::Result<CartesianPoint> {
    let total = weight1 + weight2;
    if total == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot interpolate OLAM point with zero total weight",
        ));
    }
    normalize_cartesian_to_radius(
        CartesianPoint::new(
            (point1.x * weight1 + point2.x * weight2) / total,
            (point1.y * weight1 + point2.y * weight2) / total,
            (point1.z * weight1 + point2.z * weight2) / total,
        ),
        radius,
    )
}

fn weighted_point(
    point1: CartesianPoint,
    weight1: f64,
    point2: CartesianPoint,
    weight2: f64,
) -> io::Result<CartesianPoint> {
    let total = weight1 + weight2;
    if total == 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot interpolate OLAM point with zero total weight",
        ));
    }
    Ok(CartesianPoint::new(
        (point1.x * weight1 + point2.x * weight2) / total,
        (point1.y * weight1 + point2.y * weight2) / total,
        (point1.z * weight1 + point2.z * weight2) / total,
    ))
}

fn normalized_face_center(
    point1: CartesianPoint,
    point2: CartesianPoint,
    point3: CartesianPoint,
    radius: f64,
) -> io::Result<CartesianPoint> {
    normalize_cartesian_to_radius(
        CartesianPoint::new(
            (point1.x + point2.x + point3.x) / 3.0,
            (point1.y + point2.y + point3.y) / 3.0,
            (point1.z + point2.z + point3.z) / 3.0,
        ),
        radius,
    )
}

fn olam_mesh_from_triangle_seeds(
    nmd: usize,
    impent: [usize; 12],
    m_points: Vec<CartesianPoint>,
    face_seeds: &[OlamTriangleSeed],
) -> io::Result<OlamDelaunayMesh> {
    olam_mesh_from_triangle_seeds_with_boundary_rows(nmd, impent, m_points, face_seeds, Vec::new())
}

fn olam_mesh_from_triangle_seeds_with_boundary_rows(
    nmd: usize,
    impent: [usize; 12],
    m_points: Vec<CartesianPoint>,
    face_seeds: &[OlamTriangleSeed],
    boundary_rows: Vec<usize>,
) -> io::Result<OlamDelaunayMesh> {
    require_olam_len("m_points", m_points.len(), nmd + 1)?;

    let face_iw = assign_olam_triangle_seed_w_ids(face_seeds)?;
    let nwd = face_iw.iter().copied().max().unwrap_or(1);
    let mut u_edges = vec![IcosahedronUEdge::default(); 2];
    let mut w_faces = vec![IcosahedronWFace::default(); nwd + 1];
    let mut edge_by_key = BTreeMap::<(usize, usize), usize>::new();
    let reserved_u_ids = face_seeds
        .iter()
        .flat_map(|seed| seed.target_iu)
        .filter(|&iu| iu > 1)
        .collect::<BTreeSet<_>>();
    let mut occupied_u_ids = BTreeSet::<usize>::new();
    let mut next_auto_iu = 2usize;

    for (&iw, seed) in face_iw.iter().zip(face_seeds.iter()) {
        require_unique_active_triplet("OLAM W seed M vertices", iw, seed.im, nmd)?;

        let mut face = IcosahedronWFace {
            npoly: 3,
            im: seed.im,
            mrlw: seed.mrlw.max(1),
            mrlw_orig: seed.mrlw_orig.max(1),
            ngr: seed.ngr.max(1),
            mrow: seed.mrow,
            ..IcosahedronWFace::default()
        };

        let directed_sides = [
            (seed.im[2], seed.im[1]),
            (seed.im[0], seed.im[2]),
            (seed.im[1], seed.im[0]),
        ];
        for (slot, (from, to)) in directed_sides.into_iter().enumerate() {
            let iu = insert_or_attach_olam_edge(
                &mut u_edges,
                &mut edge_by_key,
                &reserved_u_ids,
                &mut occupied_u_ids,
                &mut next_auto_iu,
                iw,
                from,
                to,
                face.iu[slot],
                seed.target_iu[slot],
            )?;
            face.iu[slot] = iu;
        }

        w_faces[iw] = face;
    }

    let nud = u_edges.len() - 1;
    for iu in 2..=nud {
        let edge = u_edges[iu];
        if edge.iw[0] <= 1 || edge.iw[1] <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("U edge {iu} is not shared by two W faces"),
            ));
        }
    }

    let mut connectivity = IcosahedronDiamondConnectivity { u_edges, w_faces };
    fill_olam_w_face_neighbors_from_edges(&mut connectivity.u_edges, &mut connectivity.w_faces)?;
    derive_icosahedron_u_neighbors_fortran(&mut connectivity).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "failed to derive OLAM U-edge neighbors from rebuilt triangle mesh",
        )
    })?;
    let m_neighbors =
        derive_olam_m_neighbors_from_incidence(nmd, &connectivity.u_edges, &connectivity.w_faces)?;
    let m_metadata = derive_olam_m_metadata_from_w_faces(nmd, &connectivity.w_faces)?;

    let mesh = OlamDelaunayMesh {
        nmd,
        nud,
        nwd,
        impent,
        m_points,
        m_metadata,
        u_edges: connectivity.u_edges,
        w_faces: connectivity.w_faces,
        m_neighbors,
        m_prognostic: olam_identity_prognostic_map(nmd),
        u_prognostic: olam_identity_prognostic_map(nud),
        w_prognostic: olam_identity_prognostic_map(nwd),
        boundary_rows,
    };
    mesh.validate_topology()?;
    Ok(mesh)
}

fn default_olam_m_metadata(nmd: usize) -> Vec<IcosahedronMPointMetadata> {
    vec![IcosahedronMPointMetadata::default(); nmd + 1]
}

fn olam_identity_prognostic_map(max_id: usize) -> Vec<usize> {
    (0..=max_id).collect()
}

fn derive_olam_m_metadata_from_w_faces(
    nmd: usize,
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointMetadata>> {
    let mut metadata = default_olam_m_metadata(nmd);
    let mut seen = vec![false; nmd + 1];
    for face in w_faces.iter().skip(2) {
        for &im in &face.im {
            require_olam_id("OLAM M metadata face vertex", im, nmd)?;
            seen[im] = true;
            metadata[im].mrlm = metadata[im].mrlm.max(face.mrlw.max(1));
            metadata[im].mrlm_orig = metadata[im].mrlm_orig.max(face.mrlw_orig.max(1));
            metadata[im].ngr = metadata[im].ngr.max(face.ngr.max(1));
        }
    }
    for (im, &has_face) in seen.iter().enumerate().skip(2) {
        if !has_face {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM M metadata point {im} is not incident on any W face"),
            ));
        }
    }
    Ok(metadata)
}

fn fill_olam_w_face_neighbors_from_edges(
    u_edges: &mut [IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
) -> io::Result<()> {
    let nwd = w_faces.len().saturating_sub(1);
    for iw in 2..=nwd {
        for slot in 0..3 {
            let iu = w_faces[iw].iu[slot];
            let edge = u_edges[iu];
            let other_iw = if edge.iw[0] == iw {
                edge.iw[1]
            } else if edge.iw[1] == iw {
                edge.iw[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("W face {iw} edge slot {slot} points at U edge {iu}, but edge does not point back"),
                ));
            };
            require_olam_id("OLAM W neighbor face", other_iw, nwd)?;
            w_faces[iw].iw[slot] = other_iw;
        }
    }

    for iw in 2..=nwd {
        let [iw1, iw2, iw3] = [w_faces[iw].iw[0], w_faces[iw].iw[1], w_faces[iw].iw[2]];
        require_olam_id("OLAM W inner neighbor", iw1, nwd)?;
        require_olam_id("OLAM W inner neighbor", iw2, nwd)?;
        require_olam_id("OLAM W inner neighbor", iw3, nwd)?;

        let pair1 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw1].iw[0], w_faces[iw1].iw[1], w_faces[iw1].iw[2]],
        );
        let pair2 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw2].iw[0], w_faces[iw2].iw[1], w_faces[iw2].iw[2]],
        );
        let pair3 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw3].iw[0], w_faces[iw3].iw[1], w_faces[iw3].iw[2]],
        );

        w_faces[iw].iw[3] = pair1[0];
        w_faces[iw].iw[4] = pair1[1];
        w_faces[iw].iw[5] = pair2[0];
        w_faces[iw].iw[6] = pair2[1];
        w_faces[iw].iw[7] = pair3[0];
        w_faces[iw].iw[8] = pair3[1];
    }

    Ok(())
}

fn order_olam_outer_w_pair_for_fill_rad3(
    w_faces: &[IcosahedronWFace],
    pair: [usize; 2],
    outer_candidates: [usize; 6],
    imx: usize,
) -> io::Result<[usize; 2]> {
    if let Some(ordered) =
        order_olam_outer_w_pair_candidate(w_faces, pair, outer_candidates, imx)?
    {
        return Ok(ordered);
    }
    if let Some(ordered) =
        order_olam_outer_w_pair_candidate(w_faces, [pair[1], pair[0]], outer_candidates, imx)?
    {
        return Ok(ordered);
    }
    Ok(pair)
}

fn order_olam_outer_w_pair_candidate(
    w_faces: &[IcosahedronWFace],
    pair: [usize; 2],
    outer_candidates: [usize; 6],
    imx: usize,
) -> io::Result<Option<[usize; 2]>> {
    let nwd = w_faces.len().saturating_sub(1);
    require_olam_id("OLAM cart_hex outer W pair", pair[0], nwd)?;
    require_olam_id("OLAM cart_hex outer W pair", pair[1], nwd)?;
    if !w_faces[pair[0]].im.contains(&imx) {
        return Ok(None);
    }
    let (im1, im2) = face_following_two_vertices(w_faces[pair[0]], imx, pair[0])?;
    if w_faces[pair[1]].im.contains(&im2) {
        let im3 = face_following_vertex(w_faces[pair[1]], im2, pair[1])?;
        if im3 != im1 {
            return Ok(Some(pair));
        }
    }
    for iwy in w_faces[pair[0]].iw {
        if iwy <= 1 {
            continue;
        }
        require_olam_id("OLAM cart_hex iwx W neighbor", iwy, nwd)?;
        if iwy != pair[0] && w_faces[iwy].im.contains(&im2) {
            let im3 = face_following_vertex(w_faces[iwy], im2, iwy)?;
            if im3 != im1 {
                return Ok(Some([pair[0], iwy]));
            }
        }
    }
    for iwy in outer_candidates {
        require_olam_id("OLAM cart_hex outer W candidate", iwy, nwd)?;
        if iwy != pair[0] && w_faces[iwy].im.contains(&im2) {
            let im3 = face_following_vertex(w_faces[iwy], im2, iwy)?;
            if im3 != im1 {
                return Ok(Some([pair[0], iwy]));
            }
        }
    }
    Ok(Some(pair))
}

fn fill_cart_hex_w_face_neighbors_from_edges(
    u_edges: &[IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
    w_prognostic: &[usize],
) -> io::Result<()> {
    let nwd = w_faces.len().saturating_sub(1);
    for iw in 2..=nwd {
        if w_prognostic[iw] != iw {
            continue;
        }
        for slot in 0..3 {
            let iu = w_faces[iw].iu[slot];
            require_olam_id("OLAM cart_hex W face U edge", iu, u_edges.len().saturating_sub(1))?;
            let edge = u_edges[iu];
            let other_iw = if edge.iw[0] == iw {
                edge.iw[1]
            } else if edge.iw[1] == iw {
                edge.iw[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("cart_hex W face {iw} edge slot {slot} points at U edge {iu}, but edge does not point back"),
                ));
            };
            require_olam_id("OLAM cart_hex W neighbor face", other_iw, nwd)?;
            w_faces[iw].iw[slot] = other_iw;
        }
    }

    for iw in 2..=nwd {
        let partner = w_prognostic[iw];
        if partner > 1 && partner != iw {
            require_olam_id("OLAM cart_hex periodic W face partner", partner, nwd)?;
            let boundary_iu = w_faces[iw].iu[0];
            w_faces[iw] = w_faces[partner];
            if boundary_iu > 1 {
                w_faces[iw].iu[0] = boundary_iu;
            }
        }
    }

    for iw in 2..=nwd {
        if w_prognostic[iw] != iw {
            continue;
        }
        let [iw1, iw2, iw3] = [w_faces[iw].iw[0], w_faces[iw].iw[1], w_faces[iw].iw[2]];
        require_olam_id("OLAM cart_hex W inner neighbor", iw1, nwd)?;
        require_olam_id("OLAM cart_hex W inner neighbor", iw2, nwd)?;
        require_olam_id("OLAM cart_hex W inner neighbor", iw3, nwd)?;

        let raw_pair1 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw1].iw[0], w_faces[iw1].iw[1], w_faces[iw1].iw[2]],
        );
        let raw_pair2 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw2].iw[0], w_faces[iw2].iw[1], w_faces[iw2].iw[2]],
        );
        let raw_pair3 = tri_neighbors_outer_w_pair(
            iw,
            [w_faces[iw3].iw[0], w_faces[iw3].iw[1], w_faces[iw3].iw[2]],
        );
        let outer_candidates = [
            raw_pair1[0],
            raw_pair1[1],
            raw_pair2[0],
            raw_pair2[1],
            raw_pair3[0],
            raw_pair3[1],
        ];
        let pair1 = order_olam_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair1,
            outer_candidates,
            w_faces[iw].im[1],
        )?;
        let pair2 = order_olam_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair2,
            outer_candidates,
            w_faces[iw].im[2],
        )?;
        let pair3 = order_olam_outer_w_pair_for_fill_rad3(
            w_faces,
            raw_pair3,
            outer_candidates,
            w_faces[iw].im[0],
        )?;

        w_faces[iw].iw[3] = pair1[0];
        w_faces[iw].iw[4] = pair1[1];
        w_faces[iw].iw[5] = pair2[0];
        w_faces[iw].iw[6] = pair2[1];
        w_faces[iw].iw[7] = pair3[0];
        w_faces[iw].iw[8] = pair3[1];
    }

    Ok(())
}

fn assign_olam_triangle_seed_w_ids(face_seeds: &[OlamTriangleSeed]) -> io::Result<Vec<usize>> {
    let mut assigned = vec![0usize; face_seeds.len()];
    let mut occupied = BTreeSet::<usize>::new();

    for (idx, seed) in face_seeds.iter().enumerate() {
        if seed.target_iw <= 1 {
            continue;
        }
        if !occupied.insert(seed.target_iw) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate OLAM target W id {}", seed.target_iw),
            ));
        }
        assigned[idx] = seed.target_iw;
    }

    let mut next_iw = 2usize;
    for iw in &mut assigned {
        if *iw > 1 {
            continue;
        }
        while occupied.contains(&next_iw) {
            next_iw += 1;
        }
        *iw = next_iw;
        occupied.insert(next_iw);
    }

    Ok(assigned)
}

fn derive_olam_m_neighbors_from_incidence(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut incident_w = vec![Vec::<usize>::new(); nmd + 1];

    for (iw, face) in w_faces.iter().enumerate().skip(2) {
        let mut unique_im = face.im.to_vec();
        unique_im.sort_unstable();
        unique_im.dedup();
        for &im in &unique_im {
            require_olam_id("W face M vertex", im, nmd)?;
        }
        if unique_im.len() < 2 {
            continue;
        }
        for &im in &unique_im {
            incident_w[im].push(iw);
        }
    }

    let mut m_neighbors = vec![IcosahedronMPointNeighbors::default(); nmd + 1];
    for im in 2..=nmd {
        let mut w_list = incident_w[im].clone();
        w_list.sort_unstable();
        w_list.dedup();

        if w_list.is_empty() {
            continue;
        }
        if !(3..=7).contains(&w_list.len()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("M point {im} has unsupported OLAM incident face count {}", w_list.len()),
            ));
        }

        let mut edge_hits = BTreeMap::<usize, usize>::new();
        let mut valid_w_list = Vec::<usize>::new();
        for &iw in &w_list {
            let Ok(incident) = olam_face_incident_edges_for_m(im, iw, u_edges, w_faces) else {
                continue;
            };
            valid_w_list.push(iw);
            for iu in incident {
                *edge_hits.entry(iu).or_insert(0usize) += 1;
            }
        }
        if valid_w_list.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "M point {im} has too few incident W faces after filtering malformed ring entries: {}",
                    valid_w_list.len()
                ),
            ));
        }
        let mut u_list = edge_hits
            .into_iter()
            .filter(|(_, hits)| *hits >= 2)
            .map(|(iu, _)| iu)
            .collect::<Vec<_>>();
        u_list.sort_unstable();
        u_list.dedup();

        if !(3..=7).contains(&u_list.len()) {
            let edge_vertices = u_list
                .iter()
                .map(|&iu| (iu, u_edges[iu].im, u_edges[iu].iw))
                .collect::<Vec<_>>();
            let face_vertices = w_list
                .iter()
                .map(|&iw| (iw, w_faces[iw].im))
                .collect::<Vec<_>>();
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "M point {im} has unsupported OLAM incidence: {} U edges {:?}, {} W faces {:?}",
                    u_list.len(),
                    edge_vertices,
                    w_list.len(),
                    face_vertices
                ),
            ));
        }

        let (ordered_u, ordered_w) =
            order_olam_m_ring_from_incidence(im, &u_list, &valid_w_list, u_edges, w_faces)?;
        let mut neighbor = IcosahedronMPointNeighbors {
            npoly: ordered_u.len(),
            ..IcosahedronMPointNeighbors::default()
        };
        for (slot, (&iu, &iw)) in ordered_u.iter().zip(ordered_w.iter()).enumerate() {
            neighbor.iu[slot] = iu;
            neighbor.iw[slot] = iw;
        }
        m_neighbors[im] = neighbor;
    }

    Ok(m_neighbors)
}

fn derive_cart_hex_m_neighbors_from_active_faces(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
    w_prognostic: &[usize],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut incident_w = vec![Vec::<usize>::new(); nmd + 1];
    for (iw, face) in w_faces.iter().enumerate().skip(2) {
        if w_prognostic.get(iw).copied().unwrap_or(iw) != iw {
            continue;
        }
        if face.npoly != 3 || face.im.iter().any(|&im| im <= 1) {
            continue;
        }
        for &im in &face.im {
            require_olam_id("OLAM cart_hex W face M vertex", im, nmd)?;
            incident_w[im].push(iw);
        }
    }

    let mut m_neighbors = vec![IcosahedronMPointNeighbors::default(); nmd + 1];
    for im in 2..=nmd {
        let mut w_list = incident_w[im].clone();
        w_list.sort_unstable();
        w_list.dedup();
        if !(3..=7).contains(&w_list.len()) {
            continue;
        }

        let mut edge_hits = BTreeMap::<usize, usize>::new();
        for &iw in &w_list {
            for iu in olam_face_incident_edges_for_m(im, iw, u_edges, w_faces)? {
                *edge_hits.entry(iu).or_insert(0usize) += 1;
            }
        }
        let mut u_list = edge_hits
            .into_iter()
            .filter(|(_, hits)| *hits >= 2)
            .map(|(iu, _)| iu)
            .collect::<Vec<_>>();
        u_list.sort_unstable();
        u_list.dedup();
        if !(3..=7).contains(&u_list.len()) {
            continue;
        }

        let (ordered_u, ordered_w) =
            order_olam_m_ring_from_incidence(im, &u_list, &w_list, u_edges, w_faces)?;
        let mut neighbor = IcosahedronMPointNeighbors {
            npoly: ordered_u.len(),
            ..IcosahedronMPointNeighbors::default()
        };
        for (slot, (&iu, &iw)) in ordered_u.iter().zip(ordered_w.iter()).enumerate() {
            neighbor.iu[slot] = iu;
            neighbor.iw[slot] = iw;
        }
        m_neighbors[im] = neighbor;
    }

    Ok(m_neighbors)
}

fn order_olam_m_ring_from_incidence(
    im: usize,
    u_list: &[usize],
    w_list: &[usize],
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<(Vec<usize>, Vec<usize>)> {
    if u_list.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let expected_u = u_list.iter().copied().collect::<BTreeSet<_>>();
    let expected_w = w_list.iter().copied().collect::<BTreeSet<_>>();
    let mut best_actual_u = BTreeSet::<usize>::new();
    let mut best_actual_w = BTreeSet::<usize>::new();
    let mut best_ordered_u = Vec::<usize>::new();
    let mut best_ordered_w = Vec::<usize>::new();
    let mut best_error = String::new();

    for &start_face in w_list {
        let incident = match olam_face_incident_edges_for_m(im, start_face, u_edges, w_faces) {
            Ok(incident) => incident,
            Err(err) => {
                best_error = err.to_string();
                continue;
            }
        };
        for &start_edge in &incident {
            let mut ordered_u = Vec::with_capacity(u_list.len());
            let mut ordered_w = Vec::with_capacity(w_list.len());
            let mut current_face = start_face;
            let mut incoming_edge = start_edge;
            let mut candidate_error = None::<String>;

            for _ in 0..w_list.len() {
                if ordered_w.contains(&current_face) {
                    break;
                }
                if !expected_w.contains(&current_face) {
                    candidate_error = Some(format!(
                        "walk reached non-incident W face {current_face} from start W {start_face}, U {start_edge}"
                    ));
                    break;
                }
                if !expected_u.contains(&incoming_edge) {
                    candidate_error = Some(format!(
                        "walk reached non-incident U edge {incoming_edge} from start W {start_face}, U {start_edge}"
                    ));
                    break;
                }
                ordered_u.push(incoming_edge);
                ordered_w.push(current_face);

                let face_edges =
                    olam_face_incident_edges_for_m(im, current_face, u_edges, w_faces)?;
                let outgoing_edge = if face_edges[0] == incoming_edge {
                    face_edges[1]
                } else if face_edges[1] == incoming_edge {
                    face_edges[0]
                } else {
                    candidate_error = Some(format!(
                        "face {current_face} does not contain incoming U edge {incoming_edge}"
                    ));
                    break;
                };
                let edge = u_edges[outgoing_edge];
                let next_face = if edge.iw[0] == current_face {
                    edge.iw[1]
                } else if edge.iw[1] == current_face {
                    edge.iw[0]
                } else {
                    candidate_error = Some(format!(
                        "outgoing U edge {outgoing_edge} does not contain W face {current_face}"
                    ));
                    break;
                };
                current_face = next_face;
                incoming_edge = outgoing_edge;
            }

            let actual_u = ordered_u.iter().copied().collect::<BTreeSet<_>>();
            let actual_w = ordered_w.iter().copied().collect::<BTreeSet<_>>();
            if actual_u == expected_u {
                return Ok((ordered_u, ordered_w));
            }
            if actual_u.len() + actual_w.len() > best_actual_u.len() + best_actual_w.len() {
                best_actual_u = actual_u;
                best_actual_w = actual_w;
                best_ordered_u = ordered_u;
                best_ordered_w = ordered_w;
                best_error = candidate_error.unwrap_or_else(|| "walk closed early".to_string());
            }
        }
    }

    let edge_rows = u_list
        .iter()
        .map(|&iu| (iu, u_edges[iu].im, u_edges[iu].iw))
        .collect::<Vec<_>>();
    let face_rows = w_list
        .iter()
        .map(|&iw| (iw, w_faces[iw].im, w_faces[iw].iu))
        .collect::<Vec<_>>();
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "M point {im} incidence ring did not close over the same U/W sets: best U {:?} vs {:?}, best W {:?} vs {:?}; ordered U {:?}, ordered W {:?}; last walk error {}; U rows {:?}; W rows {:?}",
            best_actual_u,
            expected_u,
            best_actual_w,
            expected_w,
            best_ordered_u,
            best_ordered_w,
            best_error,
            edge_rows,
            face_rows
        ),
    ))
}

fn olam_face_incident_edges_for_m(
    im: usize,
    iw: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<[usize; 2]> {
    let face = w_faces[iw];
    if !face.im.contains(&im) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("W face {iw} does not contain M point {im}"),
        ));
    }
    let mut edges = Vec::with_capacity(2);
    for &iu in &face.iu {
        if u_edges[iu].im.contains(&im) {
            edges.push(iu);
        }
    }
    if edges.len() != 2 {
        let edge_rows = face
            .iu
            .iter()
            .map(|&iu| (iu, u_edges[iu].im))
            .collect::<Vec<_>>();
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "W face {iw} has {edges_len} incident U edges for M point {im}, expected 2; face.im={:?}, face.iu={:?}, edge.im={:?}",
                face.im,
                face.iu,
                edge_rows,
                edges_len = edges.len(),
            ),
        ));
    }
    Ok([edges[0], edges[1]])
}

fn insert_or_attach_olam_edge(
    u_edges: &mut Vec<IcosahedronUEdge>,
    edge_by_key: &mut BTreeMap<(usize, usize), usize>,
    reserved_u_ids: &BTreeSet<usize>,
    occupied_u_ids: &mut BTreeSet<usize>,
    next_auto_iu: &mut usize,
    iw: usize,
    from: usize,
    to: usize,
    existing_face_edge: usize,
    target_iu: usize,
) -> io::Result<usize> {
    debug_assert_eq!(existing_face_edge, 1);
    let key = olam_edge_key(from, to);
    if let Some(&iu) = edge_by_key.get(&key) {
        if target_iu > 1 && target_iu != iu {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "W face {iw} target U edge {target_iu} conflicts with existing shared U edge {iu}"
                ),
            ));
        }
        let edge = u_edges.get_mut(iu).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing U edge {iu} while attaching W face {iw}"),
            )
        })?;
        if edge.iw[1] > 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("U edge {iu} has more than two adjacent W faces"),
            ));
        }
        if edge.im != [to, from] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "W face {iw} shares U edge {iu} with inconsistent orientation [{from}, {to}]"
                ),
            ));
        }
        edge.iw[1] = iw;
        return Ok(iu);
    }

    let iu = if target_iu > 1 {
        if !occupied_u_ids.insert(target_iu) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate target U edge id {target_iu} while inserting W face {iw}"),
            ));
        }
        target_iu
    } else {
        while reserved_u_ids.contains(next_auto_iu) || occupied_u_ids.contains(next_auto_iu) {
            *next_auto_iu += 1;
        }
        let iu = *next_auto_iu;
        occupied_u_ids.insert(iu);
        *next_auto_iu += 1;
        iu
    };

    if u_edges.len() <= iu {
        u_edges.resize(iu + 1, IcosahedronUEdge::default());
    }
    let mut edge = IcosahedronUEdge::default();
    edge.im = [from, to];
    edge.iw[0] = iw;
    edge.mrlu = 1;
    u_edges[iu] = edge;
    edge_by_key.insert(key, iu);
    Ok(iu)
}

fn require_olam_len(name: &str, actual: usize, required: usize) -> io::Result<()> {
    if actual < required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} length {actual} is shorter than required {required}"),
        ));
    }
    Ok(())
}

fn require_olam_id(label: &str, id: usize, max: usize) -> io::Result<()> {
    if id <= 1 || id > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} id {id} is outside active OLAM range 2..={max}"),
        ));
    }
    Ok(())
}

fn require_unique_active_triplet(
    label: &str,
    owner: usize,
    values: [usize; 3],
    max: usize,
) -> io::Result<()> {
    for &value in &values {
        require_olam_id(label, value, max)?;
    }
    if values[0] == values[1] || values[0] == values[2] || values[1] == values[2] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} for owner {owner} contains duplicates: {values:?}"),
        ));
    }
    Ok(())
}

/// `mem_ijtabs:mloops` used by `mdloopf`, `udloopf`, and `wdloopf`.
pub const ICOSAHEDRON_MLOOPS: usize = 7;
const OLAM_FORTRAN_EARTH_RADIUS_METERS: f64 = 6_371_220.0;
const OLAM_FORTRAN_PI2: f32 = 3.1415927_f32 * 2.0;

fn olam_fortran_global_dist00(beta: f64, radius: f64, nxp: usize) -> f64 {
    ((beta as f32) * OLAM_FORTRAN_PI2 * (radius as f32) / (5.0 * nxp as f32)) as f64
}

/// Port of the `nmd/nud/nwd` sizing formulas in
/// `icosahedron.F90:icosahedron`.
pub fn icosahedron_counts_fortran(nxp0: usize) -> Option<IcosahedronCounts> {
    if nxp0 == 0 {
        return None;
    }
    let nn10 = nxp0.checked_mul(nxp0)?.checked_mul(10)?;
    Some(IcosahedronCounts {
        nmd: nn10 + 3,
        nud: 3 * nn10 + 1,
        nwd: 2 * nn10 + 1,
    })
}

/// Port of the big-diamond corner coordinate initialization in
/// `icosahedron.F90:icosahedron`.
pub fn icosahedron_diamond_corners_fortran() -> [IcosahedronDiamondCorners; 10] {
    let radius = OLAM_FORTRAN_EARTH_RADIUS_METERS as f32;
    let erador5 = radius / 5.0_f32.sqrt();
    let full_turn = OLAM_FORTRAN_PI2;

    std::array::from_fn(|slot| {
        let id = slot + 1;
        if id <= 5 {
            let angle_n = 0.2_f32 * (id - 1) as f32 * full_turn;
            let angle_w = angle_n - 0.1_f32 * full_turn;
            let angle_e = angle_n + 0.1_f32 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(0.0, 0.0, -radius as f64),
                north: CartesianPoint::new(
                    (erador5 * 2.0 * angle_n.cos()) as f64,
                    (erador5 * 2.0 * angle_n.sin()) as f64,
                    erador5 as f64,
                ),
                west: CartesianPoint::new(
                    (erador5 * 2.0 * angle_w.cos()) as f64,
                    (erador5 * 2.0 * angle_w.sin()) as f64,
                    -erador5 as f64,
                ),
                east: CartesianPoint::new(
                    (erador5 * 2.0 * angle_e.cos()) as f64,
                    (erador5 * 2.0 * angle_e.sin()) as f64,
                    -erador5 as f64,
                ),
            }
        } else {
            let angle_s = 0.2_f32 * (id - 6) as f32 * full_turn + 0.1_f32 * full_turn;
            let angle_w = angle_s - 0.1_f32 * full_turn;
            let angle_e = angle_s + 0.1_f32 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(
                    (erador5 * 2.0 * angle_s.cos()) as f64,
                    (erador5 * 2.0 * angle_s.sin()) as f64,
                    -erador5 as f64,
                ),
                north: CartesianPoint::new(0.0, 0.0, radius as f64),
                west: CartesianPoint::new(
                    (erador5 * 2.0 * angle_w.cos()) as f64,
                    (erador5 * 2.0 * angle_w.sin()) as f64,
                    erador5 as f64,
                ),
                east: CartesianPoint::new(
                    (erador5 * 2.0 * angle_e.cos()) as f64,
                    (erador5 * 2.0 * angle_e.sin()) as f64,
                    erador5 as f64,
                ),
            }
        }
    })
}

/// Point-coordinate portion of `icosahedron.F90:icosahedron`.
///
/// This initializes the allocated point counts, the 12 pentagonal M-point
/// indices, the 10 big-diamond corner coordinates, and the pre-spring M-point
/// coordinates. Connectivity construction (`fill_diamond`/`tri_neighbors`) and
/// spring relaxation remain separate migration surfaces.
pub fn icosahedron_initial_grid_fortran(nxp0: usize) -> Option<IcosahedronInitialGrid> {
    let counts = icosahedron_counts_fortran(nxp0)?;
    let diamond_corners = icosahedron_diamond_corners_fortran();
    let mut impent = [0usize; 12];
    let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); counts.nmd + 1];
    let pwrd = 0.9_f32;
    let radius = OLAM_FORTRAN_EARTH_RADIUS_METERS as f32;

    impent[0] = 2;
    impent[11] = counts.nmd;

    for ibigd in 1..=10 {
        let corners = diamond_corners[ibigd - 1];
        for j in 1..=nxp0 {
            for i in 1..=nxp0 {
                let idiamond = (ibigd - 1) * nxp0 * nxp0 + (j - 1) * nxp0 + i;
                let im_left = idiamond + 2;
                if i == 1 && j == nxp0 {
                    impent[ibigd] = im_left;
                }

                let (mut wts, mut wtn, wtw0, wte0) = if i + j <= nxp0 {
                    (
                        ((nxp0 + 1 - i - j) as f32 / nxp0 as f32).clamp(0.0, 1.0),
                        0.0,
                        (j as f32 / (i + j - 1) as f32).clamp(0.0, 1.0),
                        1.0 - (j as f32 / (i + j - 1) as f32).clamp(0.0, 1.0),
                    )
                } else {
                    let wte0 = ((nxp0 - j) as f32
                        / (2 * nxp0 + 1 - i - j) as f32)
                        .clamp(0.0, 1.0);
                    (
                        0.0,
                        ((i + j - nxp0 - 1) as f32 / nxp0 as f32).clamp(0.0, 1.0),
                        1.0 - wte0,
                        wte0,
                    )
                };

                let mut wtw = (1.0 - wts - wtn) * wtw0;
                let mut wte = (1.0 - wts - wtn) * wte0;
                let sumwt = wts.powf(pwrd) + wtn.powf(pwrd) + wtw.powf(pwrd) + wte.powf(pwrd);
                if sumwt == 0.0 {
                    return None;
                }
                wts = wts.powf(pwrd) / sumwt;
                wtn = wtn.powf(pwrd) / sumwt;
                wtw = wtw.powf(pwrd) / sumwt;
                wte = wte.powf(pwrd) / sumwt;

                let point = CartesianPoint::new(
                    (wts * corners.south.x as f32
                        + wtn * corners.north.x as f32
                        + wtw * corners.west.x as f32
                        + wte * corners.east.x as f32) as f64,
                    (wts * corners.south.y as f32
                        + wtn * corners.north.y as f32
                        + wtw * corners.west.y as f32
                        + wte * corners.east.y as f32) as f64,
                    (wts * corners.south.z as f32
                        + wtn * corners.north.z as f32
                        + wtw * corners.west.z as f32
                        + wte * corners.east.z as f32) as f64,
                );
                let norm = ((point.x as f32).powi(2)
                    + (point.y as f32).powi(2)
                    + (point.z as f32).powi(2))
                .sqrt();
                if norm == 0.0 {
                    return None;
                }
                let expansion = radius / norm;
                m_points[im_left] = CartesianPoint::new(
                    (point.x as f32 * expansion) as f64,
                    (point.y as f32 * expansion) as f64,
                    (point.z as f32 * expansion) as f64,
                );
            }
        }
    }

    m_points[2] = CartesianPoint::new(0.0, 0.0, -radius as f64);
    m_points[counts.nmd] = CartesianPoint::new(0.0, 0.0, radius as f64);

    Some(IcosahedronInitialGrid {
        nmd: counts.nmd,
        nud: counts.nud,
        nwd: counts.nwd,
        impent,
        diamond_corners,
        m_points,
    })
}

fn to_usize_index(value: isize) -> Option<usize> {
    if value < 0 {
        None
    } else {
        Some(value as usize)
    }
}

fn fill_diamond_fortran_indexed(
    u_edges: &mut [IcosahedronUEdge],
    w_faces: &mut [IcosahedronWFace],
    im_left: usize,
    im_right: usize,
    im_top: usize,
    im_bot: usize,
    iu0: usize,
    iu1: usize,
    iu2: usize,
    iu3: usize,
    iu4: usize,
    iw1: usize,
    iw2: usize,
) -> Option<()> {
    let edge0 = u_edges.get_mut(iu0)?;
    edge0.im = [im_left, im_right];
    edge0.iw[0] = iw1;
    edge0.iw[1] = iw2;
    edge0.mrlu = 1;

    let edge1 = u_edges.get_mut(iu1)?;
    edge1.im = [im_left, im_bot];
    edge1.iw[1] = iw1;
    edge1.mrlu = 1;

    u_edges.get_mut(iu2)?.iw[0] = iw1;

    let edge3 = u_edges.get_mut(iu3)?;
    edge3.im = [im_top, im_left];
    edge3.iw[1] = iw2;
    edge3.mrlu = 1;

    u_edges.get_mut(iu4)?.iw[0] = iw2;

    let face1 = w_faces.get_mut(iw1)?;
    face1.iu = [iu0, iu1, iu2];
    face1.mrlw = 1;
    face1.mrlw_orig = 1;
    face1.ngr = 1;

    let face2 = w_faces.get_mut(iw2)?;
    face2.iu = [iu0, iu4, iu3];
    face2.mrlw = 1;
    face2.mrlw_orig = 1;
    face2.ngr = 1;

    Some(())
}

/// Port of the `fill_diamond` invocation loop inside
/// `icosahedron.F90:icosahedron`.
///
/// This preserves Fortran's 1-based allocated-array convention by returning
/// vectors with indices `0` and `1` unused/defaulted. It only covers the fields
/// explicitly written by `fill_diamond`; `tri_neighbors` is responsible for
/// later reciprocal U/W/M neighbor completion.
pub fn icosahedron_fill_diamonds_fortran(nxp0: usize) -> Option<IcosahedronDiamondConnectivity> {
    let counts = icosahedron_counts_fortran(nxp0)?;
    let mut u_edges = vec![IcosahedronUEdge::default(); counts.nud + 1];
    let mut w_faces = vec![IcosahedronWFace::default(); counts.nwd + 1];
    let ibigd_ne = [6isize, 7, 8, 9, 10, 7, 8, 9, 10, 6];
    let ibigd_se = [2isize, 3, 4, 5, 1, 2, 3, 4, 5, 1];
    let n = nxp0 as isize;
    let n2 = n * n;

    for ibigd in 1..=10isize {
        for j in 1..=n {
            for i in 1..=n {
                let idiamond = (ibigd - 1) * n2 + (j - 1) * n + i;
                let im_left = to_usize_index(idiamond + 2)?;
                let iu0 = to_usize_index(3 * idiamond)?;
                let iu1 = to_usize_index(3 * idiamond - 1)?;
                let iu3 = to_usize_index(3 * idiamond + 1)?;
                let iw1 = to_usize_index(2 * idiamond)?;
                let iw2 = to_usize_index(2 * idiamond + 1)?;

                let (im_right, im_top, im_bot, iu2, iu4) = if ibigd < 6 {
                    let idiamond_top = if i < n {
                        idiamond + 1
                    } else {
                        (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (j - 1) * n + 1
                    };
                    let im_top = idiamond_top + 2;
                    let iu4 = 3 * idiamond_top - 1;

                    let (idiamond_right, mut iu2) = if j > 1 && i < n {
                        (idiamond - n + 1, 0)
                    } else if j == 1 {
                        let right = (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (i - 1) * n + 1;
                        (right, 3 * right - 1)
                    } else {
                        (
                            (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (j - 2) * n + 1,
                            0,
                        )
                    };
                    let im_right = idiamond_right + 2;

                    let idiamond_bot = if j > 1 {
                        let bottom = idiamond - n;
                        iu2 = 3 * bottom + 1;
                        bottom
                    } else {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (i - 2) * n + 1
                    };
                    let mut im_bot = idiamond_bot + 2;
                    if i == 1 && j == 1 {
                        im_bot = 2;
                    }
                    (im_right, im_top, im_bot, iu2, iu4)
                } else {
                    let (idiamond_top, mut iu4) = if i < n {
                        let top = idiamond + 1;
                        (top, 3 * top - 1)
                    } else {
                        (
                            (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + j + 1,
                            0,
                        )
                    };
                    let mut im_top = idiamond_top + 2;

                    let idiamond_right = if j > 1 && i < n {
                        idiamond - n + 1
                    } else if j == 1 && i < n {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + i + 1
                    } else {
                        let right = (ibigd_ne[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + j;
                        iu4 = 3 * right + 1;
                        right
                    };
                    let im_right = idiamond_right + 2;

                    let idiamond_bot = if j > 1 {
                        idiamond - n
                    } else {
                        (ibigd_se[(ibigd - 1) as usize] - 1) * n2 + (n - 1) * n + i
                    };
                    let im_bot = idiamond_bot + 2;
                    let iu2 = 3 * idiamond_bot + 1;

                    if i == n && j == n {
                        im_top = 10 * n2 + 3;
                    }
                    (im_right, im_top, im_bot, iu2, iu4)
                };

                fill_diamond_fortran_indexed(
                    &mut u_edges,
                    &mut w_faces,
                    im_left,
                    to_usize_index(im_right)?,
                    to_usize_index(im_top)?,
                    to_usize_index(im_bot)?,
                    iu0,
                    iu1,
                    to_usize_index(iu2)?,
                    iu3,
                    to_usize_index(iu4)?,
                    iw1,
                    iw2,
                )?;
            }
        }
    }

    Some(IcosahedronDiamondConnectivity { u_edges, w_faces })
}

fn tri_neighbors_outer_w_pair(current_iw: usize, neighbor_inner: [usize; 3]) -> [usize; 2] {
    if current_iw == neighbor_inner[0] {
        [neighbor_inner[1], neighbor_inner[2]]
    } else if current_iw == neighbor_inner[1] {
        [neighbor_inner[2], neighbor_inner[0]]
    } else {
        [neighbor_inner[0], neighbor_inner[1]]
    }
}

/// Port of the W-face portions of `icosahedron.F90:tri_neighbors`.
///
/// This fills `itab_wd(iw)%npoly`, the three surrounding M points, the three
/// inner W neighbors, and the six outer W neighbors for every active W face
/// (`iw = 2..nwd`). U-edge and M-point reciprocal connectivity remain separate
/// migration surfaces.
pub fn derive_icosahedron_w_neighbors_fortran(
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<()> {
    for iw in 2..connectivity.w_faces.len() {
        let [iu1, iu2, iu3] = connectivity.w_faces.get(iw)?.iu;

        let mut face = *connectivity.w_faces.get(iw)?;
        face.npoly = usize::from(iu1 > 1) + usize::from(iu2 > 1) + usize::from(iu3 > 1);

        if iu1 > 1 {
            let edge = connectivity.u_edges.get(iu1)?;
            if iw == edge.iw[0] {
                face.im[2] = edge.im[0];
                face.im[1] = edge.im[1];
                face.iw[0] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[2] = edge.im[1];
                face.im[1] = edge.im[0];
                face.iw[0] = edge.iw[0];
            }
        }

        if iu2 > 1 {
            let edge = connectivity.u_edges.get(iu2)?;
            if iw == edge.iw[0] {
                face.im[0] = edge.im[0];
                face.im[2] = edge.im[1];
                face.iw[1] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[0] = edge.im[1];
                face.im[2] = edge.im[0];
                face.iw[1] = edge.iw[0];
            }
        }

        if iu3 > 1 {
            let edge = connectivity.u_edges.get(iu3)?;
            if iw == edge.iw[0] {
                face.im[1] = edge.im[0];
                face.im[0] = edge.im[1];
                face.iw[2] = edge.iw[1];
            } else if iw == edge.iw[1] {
                face.im[1] = edge.im[1];
                face.im[0] = edge.im[0];
                face.iw[2] = edge.iw[0];
            }
        }

        *connectivity.w_faces.get_mut(iw)? = face;
    }

    for iw in 2..connectivity.w_faces.len() {
        let [iw1, iw2, iw3] = [
            connectivity.w_faces.get(iw)?.iw[0],
            connectivity.w_faces.get(iw)?.iw[1],
            connectivity.w_faces.get(iw)?.iw[2],
        ];
        let neighbor1 = connectivity.w_faces.get(iw1)?.iw;
        let neighbor2 = connectivity.w_faces.get(iw2)?.iw;
        let neighbor3 = connectivity.w_faces.get(iw3)?.iw;

        let pair1 = tri_neighbors_outer_w_pair(iw, [neighbor1[0], neighbor1[1], neighbor1[2]]);
        let pair2 = tri_neighbors_outer_w_pair(iw, [neighbor2[0], neighbor2[1], neighbor2[2]]);
        let pair3 = tri_neighbors_outer_w_pair(iw, [neighbor3[0], neighbor3[1], neighbor3[2]]);

        let face = connectivity.w_faces.get_mut(iw)?;
        face.iw[3] = pair1[0];
        face.iw[4] = pair1[1];
        face.iw[5] = pair2[0];
        face.iw[6] = pair2[1];
        face.iw[7] = pair3[0];
        face.iw[8] = pair3[1];
    }

    Some(())
}

/// Port of the U-edge portion of `icosahedron.F90:tri_neighbors`.
///
/// This fills each active U edge's refinement level, four same-ring U
/// neighbors, four outer W neighbors, and eight second-ring U neighbors from
/// already-populated W-face inner-neighbor tables. W-face and M-point
/// derivation are intentionally kept as separate migration surfaces.
pub fn derive_icosahedron_u_neighbors_fortran(
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<()> {
    for iu in 2..connectivity.u_edges.len() {
        let mut edge = *connectivity.u_edges.get(iu)?;
        let iw1 = edge.iw[0];
        let iw2 = edge.iw[1];

        let w1 = *connectivity.w_faces.get(iw1)?;
        let w2 = *connectivity.w_faces.get(iw2)?;
        edge.mrlu = w1.mrlw.max(w2.mrlw);

        if w1.iu[0] == iu {
            edge.iu[0] = w1.iu[1];
            edge.iu[1] = w1.iu[2];
        } else if w1.iu[1] == iu {
            edge.iu[0] = w1.iu[2];
            edge.iu[1] = w1.iu[0];
        } else {
            edge.iu[0] = w1.iu[0];
            edge.iu[1] = w1.iu[1];
        }

        if w2.iu[0] == iu {
            edge.iu[2] = w2.iu[2];
            edge.iu[3] = w2.iu[1];
        } else if w2.iu[1] == iu {
            edge.iu[2] = w2.iu[0];
            edge.iu[3] = w2.iu[2];
        } else {
            edge.iu[2] = w2.iu[1];
            edge.iu[3] = w2.iu[0];
        }

        let iu1 = edge.iu[0];
        let iu2 = edge.iu[1];
        let iu3 = edge.iu[2];
        let iu4 = edge.iu[3];

        let neighbor1 = *connectivity.u_edges.get(iu1)?;
        edge.iw[2] = if neighbor1.iw[0] == iw1 {
            neighbor1.iw[1]
        } else {
            neighbor1.iw[0]
        };

        let neighbor2 = *connectivity.u_edges.get(iu2)?;
        edge.iw[3] = if neighbor2.iw[0] == iw1 {
            neighbor2.iw[1]
        } else {
            neighbor2.iw[0]
        };

        let neighbor3 = *connectivity.u_edges.get(iu3)?;
        edge.iw[4] = if neighbor3.iw[0] == iw2 {
            neighbor3.iw[1]
        } else {
            neighbor3.iw[0]
        };

        let neighbor4 = *connectivity.u_edges.get(iu4)?;
        edge.iw[5] = if neighbor4.iw[0] == iw2 {
            neighbor4.iw[1]
        } else {
            neighbor4.iw[0]
        };

        let iw3 = edge.iw[2];
        let iw4 = edge.iw[3];
        let iw5 = edge.iw[4];
        let iw6 = edge.iw[5];

        let w3 = *connectivity.w_faces.get(iw3)?;
        if iu1 == w3.iu[0] {
            edge.iu[4] = w3.iu[1];
            edge.iu[5] = w3.iu[2];
        } else if iu1 == w3.iu[1] {
            edge.iu[4] = w3.iu[2];
            edge.iu[5] = w3.iu[0];
        } else {
            edge.iu[4] = w3.iu[0];
            edge.iu[5] = w3.iu[1];
        }

        let w4 = *connectivity.w_faces.get(iw4)?;
        if iu2 == w4.iu[0] {
            edge.iu[6] = w4.iu[1];
            edge.iu[7] = w4.iu[2];
        } else if iu2 == w4.iu[1] {
            edge.iu[6] = w4.iu[2];
            edge.iu[7] = w4.iu[0];
        } else {
            edge.iu[6] = w4.iu[0];
            edge.iu[7] = w4.iu[1];
        }

        let w5 = *connectivity.w_faces.get(iw5)?;
        if iu3 == w5.iu[0] {
            edge.iu[8] = w5.iu[2];
            edge.iu[9] = w5.iu[1];
        } else if iu3 == w5.iu[1] {
            edge.iu[8] = w5.iu[0];
            edge.iu[9] = w5.iu[2];
        } else {
            edge.iu[8] = w5.iu[1];
            edge.iu[9] = w5.iu[0];
        }

        let w6 = *connectivity.w_faces.get(iw6)?;
        if iu4 == w6.iu[0] {
            edge.iu[10] = w6.iu[2];
            edge.iu[11] = w6.iu[1];
        } else if iu4 == w6.iu[1] {
            edge.iu[10] = w6.iu[0];
            edge.iu[11] = w6.iu[2];
        } else {
            edge.iu[10] = w6.iu[1];
            edge.iu[11] = w6.iu[0];
        }

        *connectivity.u_edges.get_mut(iu)? = edge;
    }

    Some(())
}

/// Port of the M-point polygon assembly portion of
/// `icosahedron.F90:tri_neighbors`.
///
/// Returns a Fortran-indexed table (`0` and `1` are placeholders) with each M
/// point's surrounding U and W rings. The original routine stops when a ring
/// exceeds seven sides; this Rust boundary returns `None` instead.
pub fn derive_icosahedron_m_neighbors_fortran(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> Option<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_m_neighbors_fortran_checked(nmd, u_edges, w_faces).ok()
}

fn derive_icosahedron_m_neighbors_fortran_checked(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_m_neighbors_fortran_checked_with_prognostic(nmd, u_edges, w_faces, None)
}

fn derive_icosahedron_m_neighbors_fortran_checked_with_prognostic(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    w_faces: &[IcosahedronWFace],
    m_prognostic: Option<&[usize]>,
) -> io::Result<Vec<IcosahedronMPointNeighbors>> {
    let mut m_points = vec![IcosahedronMPointNeighbors::default(); nmd + 1];

    for iu in 2..u_edges.len() {
        for j in 0..2 {
            let im = u_edges.get(iu).map(|edge| edge.im[j]).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} is out of range while deriving M neighbors"),
                )
            })?;
            let iw = u_edges.get(iu).map(|edge| edge.iw[j]).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} is out of range while deriving M neighbors"),
                )
            })?;
            if im >= m_points.len() || iw >= w_faces.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("U edge {iu} endpoint/face out of range: im={im}, iw={iw}"),
                ));
            }
            if m_prognostic
                .and_then(|map| map.get(im))
                .copied()
                .is_some_and(|partner| partner > 1 && partner != im)
            {
                continue;
            }

            if m_points[im].npoly != 0 && w_faces[iw].npoly >= 3 {
                continue;
            }

            let mut m_point = m_points[im];
            let start_iu = iu;
            let mut iunow = iu;
            let mut npoly = 0usize;
            let mut walk_trace = Vec::<(usize, [usize; 2], [usize; 6], [usize; 12])>::new();

            while iunow > 1 {
                npoly += 1;
                let edge_now = *u_edges.get(iunow).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("U edge {iunow} is out of range in M point {im} ring"),
                    )
                })?;
                walk_trace.push((iunow, edge_now.im, edge_now.iw, edge_now.iu));
                if npoly > 7 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Method-C perimeter length invalid: Current nested grid crosses (or is too close to) the next coarser grid boundary; M point {im} exceeds 7-edge OLAM ring while walking from U edge {iu}; trace {:?}",
                            walk_trace
                        ),
                    ));
                }

                let ring_slot = npoly - 1;
                m_point.iu[ring_slot] = iunow;

                if edge_now.im[0] == im {
                    if edge_now.iw[1] > 1 {
                        m_point.iw[ring_slot] = edge_now.iw[1];
                        iunow = edge_now.iu[2];
                    } else {
                        iunow = start_iu;
                    }
                } else {
                    if edge_now.iw[0] > 1 {
                        m_point.iw[ring_slot] = edge_now.iw[0];
                        iunow = edge_now.iu[1];
                    } else {
                        iunow = start_iu;
                    }
                }

                m_point.npoly = npoly;
                if iunow == start_iu {
                    break;
                }
            }

            m_points[im] = m_point;
        }
    }

    Ok(m_points)
}

/// Integrated Rust wrapper for `icosahedron.F90:tri_neighbors`.
///
/// The mutable U/W tables are updated in the same high-level sequence as the
/// Fortran subroutine: W-face neighbors, U-edge reciprocal neighbors, then
/// M-point polygon rings. The returned M table is Fortran-indexed.
pub fn derive_icosahedron_tri_neighbors_fortran(
    nmd: usize,
    connectivity: &mut IcosahedronDiamondConnectivity,
) -> Option<Vec<IcosahedronMPointNeighbors>> {
    derive_icosahedron_w_neighbors_fortran(connectivity)?;
    derive_icosahedron_u_neighbors_fortran(connectivity)?;
    derive_icosahedron_m_neighbors_fortran(nmd, &connectivity.u_edges, &connectivity.w_faces)
}

/// Port of the setup table construction before the main iteration loop in
/// `icosahedron.F90:spring_dynamics1`.
///
/// It snapshots U-edge endpoint/neighbor ids plus per-M-point polygon edge ids
/// and direction signs. Fortran stores `+relax` when `itab_ud(iu)%im(2) == im`
/// and `-relax` otherwise.
pub fn icosahedron_spring_topology_fortran(
    nmd: usize,
    u_edges: &[IcosahedronUEdge],
    m_neighbors: &[IcosahedronMPointNeighbors],
    relax: f64,
) -> Option<IcosahedronSpringTopology> {
    if m_neighbors.len() <= nmd {
        return None;
    }

    let mut edge_m_points = vec![[1usize; 2]; u_edges.len()];
    let mut edge_neighbor_u = vec![[1usize; 4]; u_edges.len()];
    for iu in 2..u_edges.len() {
        let edge = *u_edges.get(iu)?;
        edge_m_points[iu] = edge.im;
        edge_neighbor_u[iu] = [edge.iu[0], edge.iu[1], edge.iu[2], edge.iu[3]];
    }

    let mut m_npoly = vec![0usize; nmd + 1];
    let mut m_u_edges = vec![[1usize; 7]; nmd + 1];
    let mut directions = vec![[0.0_f64; 7]; nmd + 1];
    for im in 2..=nmd {
        let m_point = *m_neighbors.get(im)?;
        if m_point.npoly > 7 {
            return None;
        }
        m_npoly[im] = m_point.npoly;
        for j in 0..m_point.npoly {
            let iu = m_point.iu[j];
            let edge = *u_edges.get(iu)?;
            m_u_edges[im][j] = iu;
            directions[im][j] = if edge.im[1] == im { relax } else { -relax };
        }
    }

    Some(IcosahedronSpringTopology {
        edge_m_points,
        edge_neighbor_u,
        m_npoly,
        m_u_edges,
        directions,
    })
}

/// Port of one main-loop iteration in `icosahedron.F90:spring_dynamics1`.
///
/// `dist00` is the coarse target segment length computed by Fortran as
/// `beta * pi2_r8 * erad8 / (5 * nxp)`. The routine applies the OLAM-6.4
/// `dist00 / 1.2` target scaling, opposite-angle ratio clamp, per-M-point
/// direction signs from `IcosahedronSpringTopology`, and radius normalization.
pub fn icosahedron_spring_iteration_fortran(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    dist00: f64,
    radius: f64,
) -> Option<IcosahedronSpringIterationOutput> {
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
    {
        return None;
    }

    let mut edge_vectors = vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut edge_distances = vec![0.0_f64; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [im1, im2] = topology.edge_m_points[edge_id];
        let point1 = *m_points.get(im1)?;
        let point2 = *m_points.get(im2)?;
        let edge_vector = vector_between(point1, point2);
        let distance = magnitude(edge_vector);
        if distance == 0.0 {
            return None;
        }
        edge_vectors[edge_id] = edge_vector;
        edge_distances[edge_id] = distance;
    }

    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = edge_distances[edge_id];
        let dist1 = *edge_distances.get(iu1)?;
        let dist2 = *edge_distances.get(iu2)?;
        let dist3 = *edge_distances.get(iu3)?;
        let dist4 = *edge_distances.get(iu4)?;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        let target_distance = dist00 / 1.2 * ratio;
        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            edge_vector.x * frac_change,
            edge_vector.y * frac_change,
            edge_vector.z * frac_change,
        );
    }

    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        let npoly = topology.m_npoly[im];
        if npoly > 7 {
            return None;
        }
        let mut point = updated_m_points[im];
        for j in 0..npoly {
            let edge_id = topology.m_u_edges[im][j];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = topology.directions[im][j];
            point.x += direction * displacement.x;
            point.y += direction * displacement.y;
            point.z += direction * displacement.z;
        }

        let norm = magnitude(point);
        if norm == 0.0 {
            return None;
        }
        let expansion = radius / norm;
        updated_m_points[im] = CartesianPoint::new(
            point.x * expansion,
            point.y * expansion,
            point.z * expansion,
        );
    }

    Some(IcosahedronSpringIterationOutput {
        updated_m_points,
        edge_displacements,
        edge_distances,
    })
}

fn olam_global_spring_iteration(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    impent: &[usize; 12],
    dist00: f64,
    radius: Option<f64>,
) -> Option<Vec<CartesianPoint>> {
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
    {
        return None;
    }

    let mut edge_vectors = vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut edge_distances = vec![0.0_f64; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [im1, im2] = topology.edge_m_points[edge_id];
        let point1 = *m_points.get(im1)?;
        let point2 = *m_points.get(im2)?;
        let dx = (point2.x - point1.x) as f32;
        let dy = (point2.y - point1.y) as f32;
        let dz = (point2.z - point1.z) as f32;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance == 0.0 || !distance.is_finite() {
            return None;
        }
        edge_vectors[edge_id] = CartesianPoint::new(dx as f64, dy as f64, dz as f64);
        edge_distances[edge_id] = distance as f64;
    }

    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let dist00_f32 = dist00 as f32;
    for edge_id in 2..topology.edge_m_points.len() {
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = edge_distances[edge_id] as f32;
        let dist1 = *edge_distances.get(iu1)? as f32;
        let dist2 = *edge_distances.get(iu2)? as f32;
        let dist3 = *edge_distances.get(iu3)? as f32;
        let dist4 = *edge_distances.get(iu4)? as f32;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        if !ratio.is_finite() {
            return None;
        }
        let target_distance = dist00_f32 / 1.2 * ratio;
        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            (edge_vector.x as f32 * frac_change) as f64,
            (edge_vector.y as f32 * frac_change) as f64,
            (edge_vector.z as f32 * frac_change) as f64,
        );
    }

    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        if impent.contains(&im) {
            continue;
        }

        let npoly = topology.m_npoly[im];
        if npoly > 7 {
            return None;
        }
        let mut point = updated_m_points[im];
        for j in 0..npoly {
            let edge_id = topology.m_u_edges[im][j];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = topology.directions[im][j] as f32;
            point.x += (direction * displacement.x as f32) as f64;
            point.y += (direction * displacement.y as f32) as f64;
            point.z += (direction * displacement.z as f32) as f64;
        }

        if let Some(radius) = radius {
            let norm = magnitude(point);
            if norm == 0.0 || !norm.is_finite() {
                return None;
            }
            let expansion = radius / norm;
            updated_m_points[im] = CartesianPoint::new(
                point.x * expansion,
                point.y * expansion,
                point.z * expansion,
            );
        } else {
            updated_m_points[im] = point;
        }
    }

    Some(updated_m_points)
}

fn olam_nest_movable_m_points(
    mesh: &OlamDelaunayMesh,
    ngr: usize,
    move_interior: bool,
) -> io::Result<Vec<bool>> {
    let mut movable = vec![false; mesh.nmd + 1];

    for im in 2..=mesh.nmd {
        if mesh.m_metadata[im].ngr != ngr {
            continue;
        }

        if move_interior {
            movable[im] = true;
            continue;
        }

        let neighbors = mesh.m_neighbors[im];
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_olam_id("OLAM nest spring movable W face", iw, mesh.nwd)?;
            if mesh.w_faces[iw].mrow != 0 {
                movable[im] = true;
                break;
            }
        }
    }

    Ok(movable)
}

fn olam_nest_mrow_distance_multiplier(mrow1: isize, mrow2: isize) -> f64 {
    let mrmax = mrow1.max(mrow2);
    let mrmin = mrow1.min(mrow2);
    match (mrmax, mrmin) {
        (-2, -2) => 7.0 / 6.0,
        (-1, -2) => 8.0 / 6.0,
        (-1, -1) => 9.0 / 6.0,
        (1, -1) => 10.0 / 6.0,
        (1, 1) => 11.0 / 12.0,
        _ => 1.0,
    }
}

fn olam_nest_spring_iteration(
    m_points: &[CartesianPoint],
    mesh: &OlamDelaunayMesh,
    topology: &IcosahedronSpringTopology,
    movable_m_points: &[bool],
    dist00: f64,
    project_to_radius: bool,
) -> Option<Vec<CartesianPoint>> {
    if topology.m_npoly.len() != m_points.len()
        || topology.m_u_edges.len() != m_points.len()
        || topology.directions.len() != m_points.len()
        || topology.edge_neighbor_u.len() != topology.edge_m_points.len()
        || movable_m_points.len() != m_points.len()
    {
        return None;
    }

    let mut moveu = vec![false; topology.edge_m_points.len()];
    let mut compu = vec![false; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        let [im1, im2] = topology.edge_m_points[edge_id];
        moveu[edge_id] = movable_m_points[im1] || movable_m_points[im2];
        let [iu1, _, iu3, _] = topology.edge_neighbor_u[edge_id];
        let [iu1_im1, iu1_im2] = *topology.edge_m_points.get(iu1)?;
        let im3 = if iu1_im1 == im1 { iu1_im2 } else { iu1_im1 };
        let [iu3_im1, iu3_im2] = *topology.edge_m_points.get(iu3)?;
        let im4 = if iu3_im1 == im1 { iu3_im2 } else { iu3_im1 };
        compu[edge_id] =
            moveu[edge_id] || movable_m_points[im3] || movable_m_points[im4];
    }

    let mut edge_vectors = vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut edge_distances = vec![0.0_f64; topology.edge_m_points.len()];
    for edge_id in 2..topology.edge_m_points.len() {
        if !compu[edge_id] {
            continue;
        }
        let [im1, im2] = topology.edge_m_points[edge_id];
        let point1 = *m_points.get(im1)?;
        let point2 = *m_points.get(im2)?;
        let dx = (point2.x - point1.x) as f32;
        let dy = (point2.y - point1.y) as f32;
        let dz = (point2.z - point1.z) as f32;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance == 0.0 || !distance.is_finite() {
            return None;
        }
        edge_vectors[edge_id] = CartesianPoint::new(dx as f64, dy as f64, dz as f64);
        edge_distances[edge_id] = distance as f64;
    }

    let max_mrlu = (2..topology.edge_m_points.len())
        .filter_map(|edge_id| {
            if moveu[edge_id] {
                Some(mesh.u_edges.get(edge_id)?.mrlu.max(1))
            } else {
                None
            }
        })
        .max()
        .unwrap_or(1);
    let dist00_f32 = dist00 as f32;
    let dmin = dist00_f32 / 2.0_f32.powi(max_mrlu.saturating_sub(1) as i32);
    let min_area_squared = 0.1875_f32 * dmin.powi(4);
    let mut edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];

    for edge_id in 2..topology.edge_m_points.len() {
        if !moveu[edge_id] {
            continue;
        }
        let edge = *mesh.u_edges.get(edge_id)?;
        let [iu1, iu2, iu3, iu4] = topology.edge_neighbor_u[edge_id];
        let dist = *edge_distances.get(edge_id)? as f32;
        let dist1 = *edge_distances.get(iu1)? as f32;
        let dist2 = *edge_distances.get(iu2)? as f32;
        let dist3 = *edge_distances.get(iu3)? as f32;
        let dist4 = *edge_distances.get(iu4)? as f32;
        if dist1 == 0.0 || dist2 == 0.0 || dist3 == 0.0 || dist4 == 0.0 {
            return None;
        }

        let twocosphi3 = (dist1.powi(2) + dist2.powi(2) - dist.powi(2)) / (dist1 * dist2);
        let twocosphi4 = (dist3.powi(2) + dist4.powi(2) - dist.powi(2)) / (dist3 * dist4);
        let angle_ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
        if !angle_ratio.is_finite() {
            return None;
        }

        let edge_level = edge.mrlu.max(1);
        let mut target_distance =
            (dist00_f32 / 1.2) / 2.0_f32.powi(edge_level.saturating_sub(1) as i32)
                * angle_ratio;
        let face1 = *mesh.w_faces.get(edge.iw[0])?;
        let face2 = *mesh.w_faces.get(edge.iw[1])?;
        target_distance *= olam_nest_mrow_distance_multiplier(face1.mrow, face2.mrow) as f32;

        let s1 = 0.5 * (dist + dist1 + dist2);
        let s2 = 0.5 * (dist + dist3 + dist4);
        let area1_squared = s1 * (s1 - dist) * (s1 - dist1) * (s1 - dist2);
        let area2_squared = s2 * (s2 - dist) * (s2 - dist3) * (s2 - dist4);
        let min_local_area_squared = area1_squared.min(area2_squared);
        if min_local_area_squared <= 0.0 || !min_local_area_squared.is_finite() {
            return None;
        }
        let area_ratio = (min_area_squared / min_local_area_squared).max(1.0);
        target_distance *= area_ratio;

        let frac_change = (target_distance - dist) / dist;
        let edge_vector = edge_vectors[edge_id];
        edge_displacements[edge_id] = CartesianPoint::new(
            (edge_vector.x as f32 * frac_change) as f64,
            (edge_vector.y as f32 * frac_change) as f64,
            (edge_vector.z as f32 * frac_change) as f64,
        );
    }

    let radius = if project_to_radius {
        Some(active_mesh_radius(mesh).ok()?)
    } else {
        None
    };
    let mut updated_m_points = m_points.to_vec();
    for im in 2..m_points.len() {
        if !movable_m_points[im] {
            continue;
        }

        let npoly = topology.m_npoly[im];
        if npoly > 7 {
            return None;
        }
        let mut point = updated_m_points[im];
        for j in 0..npoly {
            let edge_id = topology.m_u_edges[im][j];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = topology.directions[im][j] as f32;
            point.x += (direction * displacement.x as f32) as f64;
            point.y += (direction * displacement.y as f32) as f64;
            point.z += (direction * displacement.z as f32) as f64;
        }

        if let Some(radius) = radius {
            let norm = magnitude(point);
            if norm == 0.0 || !norm.is_finite() {
                return None;
            }
            let expansion = radius / norm;
            updated_m_points[im] = CartesianPoint::new(
                point.x * expansion,
                point.y * expansion,
                point.z * expansion,
            );
        } else {
            updated_m_points[im] = point;
        }
    }

    Some(updated_m_points)
}

/// Multi-iteration wrapper for `icosahedron.F90:spring_dynamics1`.
///
/// It repeatedly applies `icosahedron_spring_iteration_fortran` and records the
/// Fortran-style periodic Max-DS diagnostic for `iter == 1` or
/// `iter % diagnostic_every == 0`, comparing each diagnostic iteration against
/// the coordinates at the start of that same iteration.
pub fn icosahedron_spring_dynamics1_fortran(
    m_points: &[CartesianPoint],
    topology: &IcosahedronSpringTopology,
    niter: usize,
    dist00: f64,
    radius: f64,
    diagnostic_every: usize,
) -> Option<IcosahedronSpringDynamicsOutput> {
    if diagnostic_every == 0 {
        return None;
    }

    let mut current_m_points = m_points.to_vec();
    let mut last_edge_displacements =
        vec![CartesianPoint::new(0.0, 0.0, 0.0); topology.edge_m_points.len()];
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter {
        if (iteration == 1 || iteration == niter || iteration % 20 == 0)
            && !earthmesh_core::progress::report("spring", iteration, niter)
        {
            return None;
        }
        let record_diagnostic = iteration == 1 || iteration % diagnostic_every == 0;
        let diagnostic_reference = if record_diagnostic {
            Some(current_m_points.clone())
        } else {
            None
        };

        let iteration_output =
            icosahedron_spring_iteration_fortran(&current_m_points, topology, dist00, radius)?;
        current_m_points = iteration_output.updated_m_points;
        last_edge_displacements = iteration_output.edge_displacements;

        if let Some(reference) = diagnostic_reference {
            let mut max_displacement = 0.0_f64;
            for im in 2..current_m_points.len() {
                let displacement = magnitude(vector_between(reference[im], current_m_points[im]));
                max_displacement = max_displacement.max(displacement);
            }
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
    }

    Some(IcosahedronSpringDynamicsOutput {
        updated_m_points: current_m_points,
        last_edge_displacements,
        diagnostic_max_displacements,
    })
}

/// Integrated Rust port of the deterministic in-memory portions of
/// `icosahedron.F90:icosahedron`.
///
/// This creates initial M-point coordinates, fills diamond U/W connectivity,
/// derives `tri_neighbors`, builds `spring_dynamics1` topology, computes the
/// Fortran coarse target distance `beta * pi2_r8 * erad8 / (5 * nxp0)`, and
/// applies the migrated spring loop for `niter` iterations.
pub fn icosahedron_relaxed_grid_fortran(
    nxp0: usize,
    niter: usize,
    beta: f64,
    relax: f64,
    diagnostic_every: usize,
) -> Option<IcosahedronRelaxedGrid> {
    let initial = icosahedron_initial_grid_fortran(nxp0)?;
    let mut connectivity = icosahedron_fill_diamonds_fortran(nxp0)?;
    let m_neighbors = derive_icosahedron_tri_neighbors_fortran(initial.nmd, &mut connectivity)?;
    let topology = icosahedron_spring_topology_fortran(
        initial.nmd,
        &connectivity.u_edges,
        &m_neighbors,
        relax,
    )?;
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let dist00 = beta * earthmesh_core::PI2 * radius / (5.0 * nxp0 as f64);
    let spring = icosahedron_spring_dynamics1_fortran(
        &initial.m_points,
        &topology,
        niter,
        dist00,
        radius,
        diagnostic_every,
    )?;

    Some(IcosahedronRelaxedGrid {
        nmd: initial.nmd,
        nud: initial.nud,
        nwd: initial.nwd,
        impent: initial.impent,
        m_points: spring.updated_m_points.clone(),
        connectivity,
        m_neighbors,
        spring,
    })
}

/// Shared Rust port of `icosahedron.F90:mdloopf`, `udloopf`, and `wdloopf`.
///
/// The three Fortran routines have identical flag semantics: `init == 'f'`
/// clears all loop flags, negative ids clear the selected loop, positive ids
/// set it, and zero ids are ignored. Input ids are Fortran 1-based.
pub fn apply_icosahedron_loop_flags_fortran(
    loop_flags: &mut [bool; ICOSAHEDRON_MLOOPS],
    initialize_false: bool,
    loop_ids: &[isize],
) -> Option<()> {
    if initialize_false {
        loop_flags.fill(false);
    }

    for &loop_id in loop_ids {
        if loop_id == 0 {
            continue;
        }
        let index = loop_id.unsigned_abs().checked_sub(1)?;
        let slot = loop_flags.get_mut(index)?;
        *slot = loop_id > 0;
    }

    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_conversion_preserves_order() {
        let points = [
            CartesianPoint::new(1.0, 0.0, 0.0),
            CartesianPoint::new(0.0, 1.0, 0.0),
        ];
        let lonlat = xyz_points_to_lonlat_degrees(&points);
        assert_eq!(lonlat.len(), 2);
        assert_eq!(lonlat[0].lon_degrees, 0.0);
        assert_eq!(lonlat[1].lon_degrees, 90.0);
    }

    #[test]
    fn olam_circle_region_uses_fortran_polar_stereographic_distance() {
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 5_000_000.0,
            level: 1,
        };
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(rad_to_deg(0.75), 0.0));

        assert!(
            !region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "Fortran ngr_area uses ec_ps distance, which rejects this point even though great-circle distance accepts it"
        );
    }

    #[test]
    fn olam_region_boundaries_use_fortran_strict_less_than_radius() {
        let radius = earthmesh_core::EARTH_RADIUS_METERS;
        let center = LonLatDegrees::new(0.0, 0.0);
        let circle_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(1.0, 0.0));
        let circle_distance = olam_ec_ps_distance_meters(circle_boundary, center, radius);
        let circle = OlamRefinementRegion::Circle {
            center,
            radius_meters: circle_distance,
            level: 1,
        };
        let circle_close = OlamRefinementRegion::Circle {
            center,
            radius_meters: circle_distance / 1.5,
            level: 1,
        };

        assert!(!circle.contains_cartesian(circle_boundary, radius));
        assert!(!circle_close.close_to_cartesian(circle_boundary, radius));

        let corridor_points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
        let corridor_boundary = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
        let corridor_distance = olam_corridor_segment_distance_meters(
            corridor_boundary,
            corridor_points[0],
            corridor_points[1],
            radius,
        )
        .0;
        let corridor = OlamRefinementRegion::Corridor {
            points: corridor_points.clone(),
            radius_meters: vec![corridor_distance],
            level: 1,
        };
        let zero_radius_corridor = OlamRefinementRegion::Corridor {
            points: corridor_points.clone(),
            radius_meters: vec![0.0],
            level: 1,
        };
        let on_corridor_line = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));

        assert!(!corridor.contains_cartesian(corridor_boundary, radius));
        assert!(!zero_radius_corridor.contains_cartesian(on_corridor_line, radius));
        assert!(!zero_radius_corridor.close_to_cartesian(on_corridor_line, radius));
    }

    #[test]
    fn olam_refinement_region_rejects_radius_below_fortran_dzxmin() {
        let circle = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(0.0, 0.0),
            radius_meters: 0.0005,
            level: 1,
        };
        assert!(
            circle.validate().is_err(),
            "Fortran Method-C rejects grdrad below dzxmin=0.001"
        );

        let corridor = OlamRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
            radius_meters: vec![0.001, 0.0005],
            level: 1,
        };
        assert!(
            corridor.validate().is_err(),
            "Fortran Method-C rejects any corridor grdrad below dzxmin=0.001"
        );
    }

    #[test]
    fn olam_corridor_region_uses_fortran_segment_polar_stereographic_distance() {
        let region = OlamRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(-80.0, 40.0), LonLatDegrees::new(80.0, 40.0)],
            radius_meters: vec![1_000_000.0, 1_000_000.0],
            level: 1,
        };
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

        assert!(
            !region.contains_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "Fortran ngr_area projects each segment to local PS space before linesegdist2"
        );
    }

    #[test]
    fn olam_corridor_region_interpolates_segment_radius_like_fortran() {
        let radius = earthmesh_core::EARTH_RADIUS_METERS;
        let points = vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)];
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 1.0));
        let (distance, t) = olam_corridor_segment_distance_meters(point, points[0], points[1], radius);
        let region = OlamRefinementRegion::Corridor {
            points,
            radius_meters: vec![distance * 0.5, distance * 3.0],
            level: 1,
        };

        assert!(distance > distance * 0.5);
        assert!(distance < olam_corridor_radius_at_segment(&[distance * 0.5, distance * 3.0], 0, t));
        assert!(
            region.contains_cartesian(point, radius),
            "Fortran ngr_area interpolates grdrad between segment endpoints using t"
        );
    }

    #[test]
    fn olam_corridor_region_requires_radius_per_fortran_endpoint() {
        let region = OlamRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(-1.0, 0.0), LonLatDegrees::new(1.0, 0.0)],
            radius_meters: vec![1_000_000.0],
            level: 1,
        };

        assert!(
            region.validate().is_err(),
            "Fortran ngr_area interpolates grdrad(ipt) and grdrad(jpt), so each corridor endpoint must provide a radius"
        );
    }

    #[test]
    fn olam_native_cartesian_circle_uses_fortran_mdomain_ge_two_distance() {
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10.0, 20.0),
            radius_meters: 5.0,
            level: 1,
        };

        assert!(region.contains_cartesian_xy(CartesianPoint::new(12.0, 23.0, 999.0)));
        assert!(!region.contains_cartesian_xy(CartesianPoint::new(13.0, 24.0, 999.0)));
        assert!(region.close_to_cartesian_xy(CartesianPoint::new(10.0, 27.0, 999.0)));
    }

    #[test]
    fn olam_native_cartesian_region_validation_allows_fortran_mdomain_ge_two_coordinates() {
        let circle = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(250.0, 200.0),
            radius_meters: 5.0,
            level: 1,
        };
        let corridor = OlamRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(250.0, 200.0), LonLatDegrees::new(260.0, 210.0)],
            radius_meters: vec![2.0, 6.0],
            level: 1,
        };

        assert!(circle.validate().is_err());
        assert!(corridor.validate().is_err());
        circle
            .validate_cartesian_xy()
            .expect("Fortran mdomain >= 2 accepts Cartesian native circle coordinates");
        corridor
            .validate_cartesian_xy()
            .expect("Fortran mdomain >= 2 accepts Cartesian native corridor coordinates");
    }

    #[test]
    fn olam_native_cartesian_start_uses_imcent_not_global_pentagon_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
        let pentagon = mesh.impent[0];
        let non_pentagon = (2..=mesh.nmd)
            .find(|im| !mesh.impent.contains(im))
            .expect("non-pentagon M point");
        let pentagon_xy = mesh.m_points[pentagon];
        let anchor_xy = mesh.m_points[non_pentagon];
        let radius_meters =
            (anchor_xy.x - pentagon_xy.x).hypot(anchor_xy.y - pentagon_xy.y) * 1.01;
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(anchor_xy.x, anchor_xy.y),
            radius_meters,
            level: 1,
        };

        assert!(region.contains_cartesian_xy(pentagon_xy));
        let start = mesh
            .olam_refinement_start_point_with_neighbors(
                &region,
                active_mesh_radius(&mesh).expect("mesh radius"),
                &method_c_m_neighbors,
                true,
            )
            .expect("cartesian Method-C start");

        assert_eq!(
            start, non_pentagon,
            "Fortran mdomain >= 2 skips impent logic and starts from imcent"
        );
    }

    #[test]
    fn olam_selected_faces_do_not_pre_expand_for_future_levels_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region_level_one = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(105.0, 35.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let region_level_two = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(105.0, 35.0),
            radius_meters: 2_500_000.0,
            level: 2,
        };

        let selected_level_one = mesh
            .selected_region_faces(&region_level_one, 1, false)
            .expect("level-one selected faces");
        let selected_level_two = mesh
            .selected_region_faces(&region_level_two, 1, false)
            .expect("level-two pass-one selected faces");

        assert_eq!(
            selected_level_one, selected_level_two,
            "Fortran spawn_nest selects each NN independently and does not pre-expand pass 1 for future nested grids"
        );
    }

    #[test]
    fn olam_native_cartesian_corridor_uses_fortran_linesegdist2_radius_interpolation() {
        let region = OlamRefinementRegion::Corridor {
            points: vec![LonLatDegrees::new(0.0, 0.0), LonLatDegrees::new(10.0, 0.0)],
            radius_meters: vec![2.0, 6.0],
            level: 1,
        };

        assert!(region.contains_cartesian_xy(CartesianPoint::new(5.0, 3.0, 999.0)));
        assert!(!region.contains_cartesian_xy(CartesianPoint::new(5.0, 4.0, 999.0)));
        assert!(region.close_to_cartesian_xy(CartesianPoint::new(5.0, 4.7, 999.0)));
    }

    #[test]
    fn olam_polygon_near_edge_uses_fortran_segment_polar_stereographic_distance() {
        let region = OlamRefinementRegion::Polygon {
            points: vec![
                LonLatDegrees::new(-80.0, 40.0),
                LonLatDegrees::new(80.0, 40.0),
                LonLatDegrees::new(80.0, -40.0),
                LonLatDegrees::new(-80.0, -40.0),
            ],
            level: 1,
        };
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

        assert!(
            !region.close_to_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "polygon near-edge halo should use the same Fortran PS segment distance as ngr_area"
        );
    }

    #[test]
    fn olam_bbox_near_edge_uses_fortran_segment_polar_stereographic_distance() {
        let region = OlamRefinementRegion::Bbox {
            west_degrees: -80.0,
            east_degrees: 80.0,
            south_degrees: -40.0,
            north_degrees: 40.0,
            level: 1,
        };
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 45.0));

        assert!(
            !region.close_to_cartesian(point, earthmesh_core::EARTH_RADIUS_METERS),
            "bbox near-edge halo should use the same Fortran PS segment distance as ngr_area"
        );
    }

    #[test]
    fn olam_bbox_and_polygon_regions_use_closed_corridor_not_lonlat_interior() {
        let radius = earthmesh_core::EARTH_RADIUS_METERS;
        let point = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(0.0, 0.0));
        let polygon = OlamRefinementRegion::Polygon {
            points: vec![
                LonLatDegrees::new(-40.0, -40.0),
                LonLatDegrees::new(40.0, -40.0),
                LonLatDegrees::new(40.0, 40.0),
                LonLatDegrees::new(-40.0, 40.0),
            ],
            level: 1,
        };
        let bbox = OlamRefinementRegion::Bbox {
            west_degrees: -40.0,
            east_degrees: 40.0,
            south_degrees: -40.0,
            north_degrees: 40.0,
            level: 1,
        };

        assert!(
            !polygon.contains_cartesian(point, radius),
            "Fortran ngr_area has no point-in-polygon interior fill; closed masks are treated as corridor segments"
        );
        assert!(
            !bbox.contains_cartesian(point, radius),
            "Fortran ngr_area has no lon/lat bbox interior fill; bbox input is reduced to closed corridor segments"
        );
    }

    #[test]
    fn olam_polygon_region_does_not_close_last_point_to_first_unless_explicit() {
        let radius = earthmesh_core::EARTH_RADIUS_METERS;
        let polygon = OlamRefinementRegion::Polygon {
            points: vec![
                LonLatDegrees::new(0.0, 0.0),
                LonLatDegrees::new(60.0, 0.0),
                LonLatDegrees::new(60.0, 60.0),
            ],
            level: 1,
        };
        let point_on_implicit_closing_segment =
            lonlat_degrees_to_unit_xyz(LonLatDegrees::new(30.0, 30.0));

        assert!(
            !polygon.contains_cartesian(point_on_implicit_closing_segment, radius),
            "Fortran ngr_area only checks connected input segments 1..ngrdll-1; it does not add an implicit last-to-first closing segment"
        );
    }

    #[test]
    fn olam_multipoint_region_anchor_uses_first_specified_point_like_fortran() {
        let first = LonLatDegrees::new(-40.0, -30.0);
        let corridor = OlamRefinementRegion::Corridor {
            points: vec![first, LonLatDegrees::new(20.0, 30.0)],
            radius_meters: vec![500_000.0, 500_000.0],
            level: 1,
        };
        let polygon = OlamRefinementRegion::Polygon {
            points: vec![
                first,
                LonLatDegrees::new(40.0, -30.0),
                LonLatDegrees::new(40.0, 30.0),
                LonLatDegrees::new(-40.0, 30.0),
            ],
            level: 1,
        };

        assert_eq!(
            corridor.anchor_lonlat(),
            first,
            "Fortran chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
        );
        assert_eq!(
            polygon.anchor_lonlat(),
            first,
            "Fortran chooses imcent from grdlat/grdlon index 1 for multi-point NGR regions"
        );
    }

    #[test]
    fn olam_bbox_region_anchor_uses_first_closed_corridor_corner_like_fortran() {
        let bbox = OlamRefinementRegion::Bbox {
            west_degrees: -40.0,
            east_degrees: 40.0,
            south_degrees: -30.0,
            north_degrees: 30.0,
            level: 1,
        };

        assert_eq!(
            bbox.anchor_lonlat(),
            LonLatDegrees::new(-40.0, -30.0),
            "bbox regions are reduced to closed Fortran corridor segments, so anchor must be the first generated corner"
        );
    }

    #[test]
    fn olam_nest_mrow_distance_multiplier_matches_fortran_transition_rows() {
        let cases = [
            ((-2, -2), 7.0 / 6.0),
            ((-1, -2), 8.0 / 6.0),
            ((-1, -1), 9.0 / 6.0),
            ((1, -1), 10.0 / 6.0),
            ((1, 1), 11.0 / 12.0),
            ((0, 0), 1.0),
            ((2, -3), 1.0),
        ];

        for ((mrow1, mrow2), expected) in cases {
            let actual = olam_nest_mrow_distance_multiplier(mrow1, mrow2);
            assert!(
                (actual - expected).abs() <= f64::EPSILON,
                "mrow pair ({mrow1}, {mrow2}) expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn olam_perim_mrow_preserves_existing_adjacent_rows_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh
            .spawn_nest_with_max_mrows(
                &[region],
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            )
            .expect("Method-C nest");
        let preserved_iw = (2..=refined.nwd)
            .find(|&iw| refined.w_faces[iw].mrow >= 2)
            .expect("transition row that Fortran may preserve");

        for iw in 2..=refined.nwd {
            refined.w_faces[iw].mrow = 0;
        }
        refined.w_faces[preserved_iw].mrow = 1;
        refined
            .apply_olam_perimeter_mrows(2, OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
            .expect("Fortran perim_mrow preserves old -2/-1/1 rows when not crossing");

        assert_eq!(
            refined.w_faces[preserved_iw].mrow, 1,
            "Fortran perim_mrow preserves existing -2, -1, and 1 rows unless they cross the new border"
        );
    }

    #[test]
    fn olam_perim_mrow_rejects_crossing_existing_border_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh
            .spawn_nest_with_max_mrows(
                &[region],
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            )
            .expect("Method-C nest");
        let crossing_iw = (2..=refined.nwd)
            .find(|&iw| refined.w_faces[iw].mrow == 1)
            .expect("current border row that should reject an existing adjacent row");

        for iw in 2..=refined.nwd {
            refined.w_faces[iw].mrow = 0;
        }
        refined.w_faces[crossing_iw].mrow = 1;

        let err = refined
            .apply_olam_perimeter_mrows(2, OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
            .expect_err("Fortran perim_mrow rejects crossing or too-close nested boundaries");
        assert!(
            err.to_string().contains("crosses the parent boundary"),
            "unexpected perim_mrow error: {err}"
        );
    }

    #[test]
    fn olam_perim_mrow_overwrites_old_outer_rows_below_minus_two_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh
            .spawn_nest_with_max_mrows(
                &[region],
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
            )
            .expect("Method-C nest");
        let overwritten_iw = (2..=refined.nwd)
            .find(|&iw| refined.w_faces[iw].mrow >= 2)
            .expect("outer transition row that Fortran may overwrite");
        let expected_row = refined.w_faces[overwritten_iw].mrow;

        for iw in 2..=refined.nwd {
            refined.w_faces[iw].mrow = 0;
        }
        refined.w_faces[overwritten_iw].mrow = -3;
        refined
            .apply_olam_perimeter_mrows(2, OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE)
            .expect("Fortran perim_mrow overwrites old rows below -2");

        assert_eq!(
            refined.w_faces[overwritten_iw].mrow, expected_row,
            "Fortran perim_mrow overwrites existing mrow values below -2 with the new transition row"
        );
    }

    #[test]
    fn olam_perim_mrow_uses_fortran_half_step_row_growth() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let refined = mesh
            .spawn_nest_with_max_mrows(&[region], 1, 3)
            .expect("Method-C nest with explicit mrow width");
        let max_abs_mrow = refined
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrow.unsigned_abs())
            .max()
            .expect("mrow values");

        assert_eq!(
            max_abs_mrow, 3,
            "Fortran perim_mrow propagates through 2*max_mrows passes but only increments row magnitude on alternating passes"
        );
    }

    #[test]
    fn olam_nest_movable_points_match_fortran_transition_rule() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
        let actual =
            olam_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
        let mut expected = vec![false; refined.nmd + 1];

        for im in 2..=refined.nmd {
            if refined.m_metadata[im].ngr != 2 {
                continue;
            }
            let neighbors = refined.m_neighbors[im];
            expected[im] = neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .any(|&iw| refined.w_faces[iw].mrow != 0);
        }

        let mismatched = (2..=refined.nmd)
            .filter(|&im| actual[im] != expected[im])
            .collect::<Vec<_>>();
        assert!(
            mismatched.is_empty(),
            "Fortran spring_dynamics_nest only moves M points on ngr that touch mrow != 0; mismatched M ids: {mismatched:?}"
        );
    }

    #[test]
    fn olam_nest_movable_points_use_mrow_not_boundary_row_cache() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
        refined.boundary_rows.clear();

        let actual =
            olam_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
        let mut expected = vec![false; refined.nmd + 1];

        for im in 2..=refined.nmd {
            if refined.m_metadata[im].ngr != 2 {
                continue;
            }
            let neighbors = refined.m_neighbors[im];
            expected[im] = neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .any(|&iw| refined.w_faces[iw].mrow != 0);
        }

        let missed = (2..=refined.nmd)
            .filter(|&im| expected[im] && !actual[im])
            .collect::<Vec<_>>();
        assert!(
            missed.is_empty(),
            "Fortran spring_dynamics_nest reads itab_wd%mrow directly, not a cached boundary-row list; missed M ids: {missed:?}"
        );
    }

    #[test]
    fn olam_nest_move_interior_keeps_parent_grid_m_points_stationary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
        let actual =
            olam_nest_movable_m_points(&refined, 2, true).expect("movable M point mask");
        let mismatched = (2..=refined.nmd)
            .filter(|&im| actual[im] != (refined.m_metadata[im].ngr == 2))
            .collect::<Vec<_>>();

        assert!(
            mismatched.is_empty(),
            "Fortran moveint=1 moves all and only M points whose itab_md%ngr equals the current nest ngr; mismatched M ids: {mismatched:?}"
        );
    }

    #[test]
    fn olam_nest_transition_movement_filters_parent_grid_m_points() {
        let mut mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let boundary_iw = 2;
        mesh.boundary_rows = vec![boundary_iw];
        mesh.w_faces[boundary_iw].mrow = 1;
        mesh.w_faces[boundary_iw].ngr = 2;

        let actual =
            olam_nest_movable_m_points(&mesh, 2, false).expect("movable M point mask");
        let moved_parent_points = mesh.w_faces[boundary_iw]
            .im
            .iter()
            .copied()
            .filter(|&im| mesh.m_metadata[im].ngr != 2 && actual[im])
            .collect::<Vec<_>>();

        assert!(
            moved_parent_points.is_empty(),
            "Fortran spring_dynamics_nest skips transition-row M points whose itab_md%ngr is not the current ngr; moved parent-grid M ids: {moved_parent_points:?}"
        );
    }

    #[test]
    fn olam_nest_spring_ignores_mrlu_outside_moving_stencil() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
        let movable =
            olam_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
        let transition_face_id = *refined
            .boundary_rows()
            .first()
            .expect("transition row face should be recorded");
        let transition_point_id = refined.w_faces[transition_face_id]
            .im
            .iter()
            .copied()
            .find(|&im| movable[im])
            .expect("transition face has a movable M point");
        let transition_neighbors = refined.m_neighbors[transition_point_id];
        let neighbor_edge_id = transition_neighbors.iu[0];
        let neighbor_point_id = if refined.u_edges[neighbor_edge_id].im[0] == transition_point_id {
            refined.u_edges[neighbor_edge_id].im[1]
        } else {
            refined.u_edges[neighbor_edge_id].im[0]
        };
        let squeezed = CartesianPoint::new(
            refined.m_points[neighbor_point_id].x * 0.999
                + refined.m_points[transition_point_id].x * 0.001,
            refined.m_points[neighbor_point_id].y * 0.999
                + refined.m_points[transition_point_id].y * 0.001,
            refined.m_points[neighbor_point_id].z * 0.999
                + refined.m_points[transition_point_id].z * 0.001,
        );
        let scale = earthmesh_core::EARTH_RADIUS_METERS / magnitude(squeezed);
        refined.m_points[transition_point_id] = CartesianPoint::new(
            squeezed.x * scale,
            squeezed.y * scale,
            squeezed.z * scale,
        );
        let mut with_remote_level = refined.clone();
        let remote_edge_id = (2..=with_remote_level.nud)
            .find(|&iu| {
                let [im1, im2] = with_remote_level.u_edges[iu].im;
                !movable[im1] && !movable[im2]
            })
            .expect("remote non-moving U edge");
        with_remote_level.u_edges[remote_edge_id].mrlu = 16;

        let baseline = refined
            .spring_nest(6, 1, 2, false)
            .expect("baseline nest spring");
        let remote_changed = with_remote_level
            .spring_nest(6, 1, 2, false)
            .expect("remote-level nest spring");

        for im in 2..=baseline.nmd {
            let diff = magnitude(vector_between(
                baseline.m_points[im],
                remote_changed.m_points[im],
            ));
            assert!(
                diff <= 1.0e-7,
                "Fortran spring_dynamics_nest computes mrlmax only from nmoveu edges; remote edge {remote_edge_id} changed M point {im} by {diff}"
            );
        }
    }

    #[test]
    fn olam_nest_spring_ignores_degenerate_edge_outside_compu_stencil() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut refined = mesh.spawn_nest(&[region], 1).expect("local circle nest");
        let movable =
            olam_nest_movable_m_points(&refined, 2, false).expect("movable M point mask");
        let topology = icosahedron_spring_topology_fortran(
            refined.nmd,
            &refined.u_edges,
            &refined.m_neighbors,
            0.035,
        )
        .expect("spring topology");
        let mut compu = vec![false; refined.nud + 1];
        for edge_id in 2..=refined.nud {
            let [im1, im2] = refined.u_edges[edge_id].im;
            let [iu1, _, iu3, _] = topology.edge_neighbor_u[edge_id];
            let [iu1_im1, iu1_im2] = refined.u_edges[iu1].im;
            let im3 = if iu1_im1 == im1 { iu1_im2 } else { iu1_im1 };
            let [iu3_im1, iu3_im2] = refined.u_edges[iu3].im;
            let im4 = if iu3_im1 == im1 { iu3_im2 } else { iu3_im1 };
            compu[edge_id] = movable[im1] || movable[im2] || movable[im3] || movable[im4];
        }
        let remote_edge_id = (2..=refined.nud)
            .find(|&edge_id| !compu[edge_id])
            .expect("non-computational remote U edge");
        let [remote_im1, remote_im2] = refined.u_edges[remote_edge_id].im;
        refined.m_points[remote_im2] = refined.m_points[remote_im1];

        refined
            .spring_nest(6, 1, 2, false)
            .expect("Fortran spring_dynamics_nest should ignore non-compu remote edges");
    }

    #[test]
    fn olam_selected_faces_close_sharp_concavity_around_m_point() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let point_id = (2..=mesh.nmd)
            .find(|id| !mesh.impent.contains(id) && mesh.m_neighbors[*id].npoly == 6)
            .expect("six-sided non-pentagon M point");
        let neighbors = mesh.m_neighbors[point_id];
        let missing_face = neighbors.iw[neighbors.npoly - 1];
        let mut selected = vec![false; mesh.nwd + 1];
        for &iw in neighbors.iw.iter().take(neighbors.npoly - 1) {
            selected[iw] = true;
        }

        mesh.close_olam_selected_face_concavities(&mut selected)
            .expect("close sharp concavity");

        assert!(
            selected[missing_face],
            "OLAM sharp-concavity fill should add the only missing W face around M point {point_id}"
        );
    }

    #[test]
    fn olam_concavity_fill_keeps_fortran_npoly_minus_one_threshold_at_pentagons() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let point_id = mesh.impent[0];
        let neighbors = mesh.m_neighbors[point_id];
        assert_eq!(neighbors.npoly, 5, "test point should be an OLAM pentagon");

        let mut too_sparse = vec![false; mesh.nwd + 1];
        for &iw in neighbors.iw.iter().take(neighbors.npoly - 2) {
            too_sparse[iw] = true;
        }
        mesh.close_olam_selected_face_concavities(&mut too_sparse)
            .expect("close sparse pentagon case");
        assert_eq!(
            neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .filter(|&&iw| too_sparse[iw])
                .count(),
            neighbors.npoly - 2,
            "Fortran skips concavity fill while nw < npoly - 1"
        );

        let mut one_missing = vec![false; mesh.nwd + 1];
        for &iw in neighbors.iw.iter().take(neighbors.npoly - 1) {
            one_missing[iw] = true;
        }
        mesh.close_olam_selected_face_concavities(&mut one_missing)
            .expect("close one-missing pentagon case");
        assert!(
            neighbors
                .iw
                .iter()
                .take(neighbors.npoly)
                .all(|&iw| one_missing[iw]),
            "Fortran fills pentagon concavities only once nw reaches npoly - 1"
        );
    }

    #[test]
    fn olam_fill_rad3_marks_all_current_pentagon_faces_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
        let pentagon = mesh.impent[0];
        let neighbors = method_c_m_neighbors[pentagon];
        assert_eq!(neighbors.npoly, 5, "test requires an OLAM pentagon");

        let mut selected = vec![false; mesh.nwd + 1];
        mesh.mark_fill_rad3_faces_with_neighbors(
            pentagon,
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Fortran fill_rad3 around a pentagon");

        let missed = neighbors
            .iw
            .iter()
            .take(neighbors.npoly)
            .copied()
            .filter(|&iw| !selected[iw])
            .collect::<Vec<_>>();
        assert!(
            missed.is_empty(),
            "Fortran fill_rad3 loops over current M point npoly, not a hard-coded hexagon width; missed W faces: {missed:?}"
        );
    }

    #[test]
    fn olam_fill_rad3_marks_six_neighbors_of_three_distant_m_points_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        let im = (2..=mesh.nmd)
            .find(|&candidate| method_c_m_neighbors[candidate].npoly == 6)
            .expect("ordinary hexagonal M point");
        let immediate = method_c_m_neighbors[im]
            .iw
            .iter()
            .take(method_c_m_neighbors[im].npoly)
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let mut expected_far_w = std::collections::BTreeSet::new();

        for &iw in &immediate {
            let face = mesh.w_faces[iw];
            let (imx, iwx, iwy) = if im == face.im[0] {
                (face.im[1], face.iw[3], face.iw[4])
            } else if im == face.im[1] {
                (face.im[2], face.iw[5], face.iw[6])
            } else {
                (face.im[0], face.iw[7], face.iw[8])
            };
            let (im1, im2) =
                face_following_two_vertices(mesh.w_faces[iwx], imx, iwx).expect("Fortran im1/im2");
            let im3 = face_following_vertex(mesh.w_faces[iwy], im2, iwy).expect("Fortran im3");
            for far_im in [im1, im2, im3] {
                for &far_iw in method_c_m_neighbors[far_im].iw.iter().take(6) {
                    expected_far_w.insert(far_iw);
                }
            }
        }

        let mut selected = vec![false; mesh.nwd + 1];
        mesh.mark_fill_rad3_faces_with_neighbors(im, &mut selected, &method_c_m_neighbors)
            .expect("Fortran fill_rad3 around a hexagon");
        let missed = expected_far_w
            .iter()
            .copied()
            .filter(|&iw| !selected[iw])
            .collect::<Vec<_>>();

        assert!(
            missed.is_empty(),
            "Fortran fill_rad3 marks all six W neighbors of each im1/im2/im3 distant M point; missed W faces: {missed:?}"
        );
        assert!(
            expected_far_w.iter().any(|iw| !immediate.contains(iw)),
            "test must cover fill_rad3's distant M-point expansion beyond the immediate ring"
        );
    }

    #[test]
    fn olam_cart_hex_initializes_m_metadata_like_fortran() {
        let mesh = OlamDelaunayMesh::from_cart_hex(2, 1000.0).expect("cart_hex OLAM mesh");

        for im in 2..=mesh.nmd {
            assert_eq!(mesh.m_metadata[im].mrlm, 1);
            assert_eq!(mesh.m_metadata[im].mrlm_orig, 1);
            assert_eq!(mesh.m_metadata[im].ngr, 1);
        }
    }

    #[test]
    fn olam_method_c_skips_cart_hex_periodic_copy_faces_like_fortran() {
        let mesh = OlamDelaunayMesh::from_cart_hex(5, 1000.0).expect("cart_hex OLAM mesh");
        let ghost_iw = (2..=mesh.nwd)
            .find(|&iw| mesh.w_prognostic[iw] > 1 && mesh.w_prognostic[iw] != iw)
            .expect("Fortran cart_hex periodic W copy");
        let partner_iw = mesh.w_prognostic[ghost_iw];

        assert!(
            !mesh.method_c_w_face_is_active(ghost_iw),
            "Fortran Method-C must ignore cart_hex periodic W copies as active fill_rad3 faces"
        );
        assert!(
            mesh.method_c_w_face_is_active(partner_iw),
            "Fortran Method-C should keep the prognostic owner W face active"
        );

        let face_with_copy_m = (2..=mesh.nwd)
            .find(|&iw| {
                mesh.w_faces[iw]
                    .im
                    .iter()
                    .any(|&im| mesh.m_prognostic[im] > 1 && mesh.m_prognostic[im] != im)
            })
            .expect("Fortran cart_hex W face containing a periodic M copy");
        assert!(
            !mesh.method_c_w_face_is_active(face_with_copy_m),
            "Fortran Method-C must ignore W faces that contain cart_hex periodic M copies"
        );
    }

    #[test]
    fn olam_fill_rad3_skips_cart_hex_periodic_copy_faces_like_fortran() {
        let mesh = OlamDelaunayMesh::from_cart_hex(5, 1000.0).expect("cart_hex OLAM mesh");
        let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
        let exposed_periodic_copies = (2..=mesh.nmd)
            .flat_map(|im| {
                method_c_m_neighbors[im]
                    .iw
                    .iter()
                    .take(method_c_m_neighbors[im].npoly)
                    .copied()
            })
            .filter(|&iw| mesh.w_prognostic[iw] > 1 && mesh.w_prognostic[iw] != iw)
            .collect::<Vec<_>>();
        assert!(
            exposed_periodic_copies.is_empty(),
            "Fortran Method-C M-neighbor rings must not expose cart_hex periodic-copy W faces to fill_rad3: {exposed_periodic_copies:?}"
        );
    }

    #[test]
    fn olam_selected_regions_skip_cart_hex_periodic_copy_faces_like_fortran() {
        let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10_200_000.0, -310_000.0),
            radius_meters: 500_000.0,
            level: 1,
        };

        let selected = mesh
            .selected_regions_faces(&[region], 1, true)
            .expect("Fortran Method-C cart_hex region selection");
        let selected_periodic_copies = (2..=mesh.nwd)
            .filter(|&iw| selected[iw] && !mesh.method_c_w_face_is_active(iw))
            .collect::<Vec<_>>();

        assert!(
            selected_periodic_copies.is_empty(),
            "Fortran Method-C region selection must not include cart_hex periodic-copy W faces: {selected_periodic_copies:?}"
        );
    }

    #[test]
    fn olam_method_c_suppresses_center_perimeter_segment_faces_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        assert_eq!(
            perimeter.len() % 3,
            0,
            "Fortran Method-C suppression consumes perimeter points in triples"
        );
        let expected_start = (2..=mesh.nmd)
            .find(|&im| {
                let neighbors = method_c_m_neighbors[im];
                neighbors
                    .iw
                    .iter()
                    .take(neighbors.npoly)
                    .filter(|&&iw| nest_wd[iw].is_subdivided())
                    .count()
                    == 2
            })
            .expect("Fortran perim_map2 start point");
        assert_eq!(
            perimeter[0].im, expected_start,
            "Fortran perim_map2 starts from the first original M point with nwdiv == 2"
        );
        for index in 0..perimeter.len() {
            let point = perimeter[index];
            let next = perimeter[(index + 1) % perimeter.len()].im;
            let edge = mesh.u_edges[point.iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if edge.im[0] == point.im {
                assert!(
                    nest_wd[iw1].flag() == 0 && nest_wd[iw2].is_subdivided(),
                    "Fortran perim_ngr advances from im(1) only when iw(1) is outside and iw(2) is inside"
                );
                assert_eq!(
                    next, edge.im[1],
                    "Fortran perim_ngr next M point from im(1) is im(2)"
                );
            } else {
                assert_eq!(
                    edge.im[1], point.im,
                    "Fortran perim_ngr perimeter U edge must contain the current M point"
                );
                assert!(
                    nest_wd[iw2].flag() == 0 && nest_wd[iw1].is_subdivided(),
                    "Fortran perim_ngr advances from im(2) only when iw(2) is outside and iw(1) is inside"
                );
                assert_eq!(
                    next, edge.im[0],
                    "Fortran perim_ngr next M point from im(2) is im(1)"
                );
            }
        }
        for point in &perimeter {
            let neighbors = method_c_m_neighbors[point.im];
            let mut expected_nwdiv = 0usize;
            let mut expected_near_pentagon = false;
            for j in 0..neighbors.npoly {
                let iw = neighbors.iw[j];
                if nest_wd[iw].is_subdivided() {
                    expected_nwdiv += 1;
                }

                let iu = neighbors.iu[j];
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if nest_wd[iw1].flag() == 0 && nest_wd[iw2].flag() == 0 {
                    if point.im == edge.im[0] && method_c_m_neighbors[edge.im[1]].npoly == 5 {
                        expected_near_pentagon = true;
                    }
                    if point.im == edge.im[1] && method_c_m_neighbors[edge.im[0]].npoly == 5 {
                        expected_near_pentagon = true;
                    }
                }
            }

            assert_eq!(
                point.npoly, neighbors.npoly,
                "Fortran perim_map2 stores npolyper for each perimeter M point"
            );
            assert_eq!(
                point.nwdiv, expected_nwdiv,
                "Fortran perim_map2 stores nwdivper for each perimeter M point"
            );
            assert_eq!(
                point.near_pentagon, expected_near_pentagon,
                "Fortran perim_map2 stores nearpent from outside unsplit U edges"
            );
        }

        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            assert!(
                selected[suppressed_w],
                "suppressed W face {suppressed_w} should be an originally selected center-segment face"
            );
            nest_wd[suppressed_w].iw[2] = -1;
        }

        for face in nest_wd.iter().skip(2).filter(|face| face.is_suppressed()) {
            assert!(
                !face.is_subdivided(),
                "Fortran suppression flag -1 must prevent full subdivision allocation"
            );
        }
    }

    #[test]
    fn olam_method_c_repairs_non_triplet_perimeter_by_local_growth() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");

        let mut selected_case = None;
        'faces: for iw in 2..=mesh.nwd {
            for &adjacent in mesh.w_faces[iw].iw.iter().take(3) {
                if adjacent <= 1 || adjacent > mesh.nwd || mesh.w_faces[adjacent].mrlw != mesh.w_faces[iw].mrlw {
                    continue;
                }
                let mut selected = vec![false; mesh.nwd + 1];
                selected[iw] = true;
                selected[adjacent] = true;
                mesh.close_olam_method_c_concavities_for_level_with_neighbors(
                    &mut selected,
                    &method_c_m_neighbors,
                )
                .expect("Fortran concavity closure");

                let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
                for test_iw in 2..=mesh.nwd {
                    if selected[test_iw] {
                        nest_wd[test_iw].iw[2] = 1;
                    }
                }
                let Ok(perimeter) = mesh.perim_map2_method_c(&nest_wd, &method_c_m_neighbors) else {
                    continue;
                };
                if !perimeter.is_empty() && perimeter.len() % 3 != 0 {
                    selected_case = Some((selected, perimeter.len()));
                    break 'faces;
                }
            }
        }

        let (selected, perimeter_len) =
            selected_case.expect("test requires a non-triplet Fortran perimeter case");
        let refined = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                true,
            )
            .unwrap_or_else(|error| {
                panic!(
                    "non-triplet perimeter length {perimeter_len} should be locally repairable when same-MRL boundary faces are available: {error}"
                )
            });
        refined
            .validate_topology()
            .expect("locally repaired Method-C mesh topology");
    }

    #[test]
    fn olam_method_c_perim_ngr_matches_perimeter_next_point() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");

        let mut selected_case = None;
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let region = OlamRefinementRegion::Circle {
                center: xyz_to_lonlat_degrees(point),
                radius_meters: 2_000_000.0,
                level: 1,
            };
            let selected = mesh
                .selected_region_faces(&region, 1, false)
                .expect("selected Method-C faces");
            let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
            for iw in 2..=mesh.nwd {
                if selected[iw] {
                    nest_wd[iw].iw[2] = 1;
                }
            }
            let perimeter = match mesh.perim_map2_method_c(&nest_wd, &method_c_m_neighbors) {
                Ok(perimeter) => perimeter,
                Err(_) => continue,
            };
            if perimeter.is_empty() {
                continue;
            }
            selected_case = Some((perimeter, nest_wd));
            break;
        }
        let (perimeter, nest_wd) = selected_case
            .expect("perimeter case in selected Method-C region");

        for point in perimeter {
            let edge = mesh.u_edges[point.iu];
            let next_expected = if point.im == edge.im[0] {
                edge.im[1]
            } else {
                edge.im[0]
            };
            let (next, next_edge) = mesh
                .perim_ngr_method_c(point.im, &nest_wd, &method_c_m_neighbors)
                .expect("fortran perim_ngr");
            assert_eq!(
                next_edge, point.iu,
                "perim_map2 and perim_ngr must agree on boundary edge"
            );
            assert_eq!(
                next, next_expected,
                "perim_ngr should return the immediate perimeter neighbor without prognostic folding"
            );
        }
    }

    #[test]
    fn olam_method_c_full_subdivision_uses_grid_number_for_w_face_ngr() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(66, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let child_grid_number = 4;
        let mut refined_case = None;

        for radius_meters in [
            2_500_000.0,
            3_000_000.0,
            3_500_000.0,
            4_000_000.0,
            4_500_000.0,
            5_000_000.0,
            5_500_000.0,
            6_000_000.0,
        ] {
            let region = OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters,
                level: 1,
            };
            let selected = mesh
                .selected_region_faces(&region, 1, false)
                .expect("selected Method-C faces");
            let Ok(refined) = mesh.spawn_nest_pass_with_max_mrows(
                &selected,
                child_grid_number,
                7,
            true,
            ) else {
                continue;
            };
            if refined
                .w_faces
                .iter()
                .skip(2)
                .any(|face| face.mrlw == 2 && face.mrow == 0)
            {
                refined_case = Some(refined);
                break;
            }
        }

        let refined = refined_case.expect("test case with an interior full-subdivision W face");
        let mismatched = refined
            .w_faces
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, face)| {
                if face.mrlw == 2 && face.ngr != child_grid_number {
                    Some((iw, face.ngr))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        assert!(
            mismatched.is_empty(),
            "Fortran assigns full-subdivision W-face ngr from the current grid number, not mrlo + 1; mismatches: {mismatched:?}"
        );
    }

    #[test]
    fn olam_method_c_pass_uses_fortran_table_numbering_counts() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let selected_w_count = (2..=mesh.nwd)
            .filter(|&iw| nest_wd[iw].is_subdivided())
            .count();
        let split_u_count = (2..=mesh.nud)
            .filter(|&iu| {
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !nest_wd[iw1].is_suppressed()
                    && !nest_wd[iw2].is_suppressed()
            })
            .count();

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");

        assert_eq!(
            refined.nmd,
            mesh.nmd + split_u_count,
            "Fortran Method-C allocates one midpoint M only for non-suppressed split U edges"
        );
        assert_eq!(
            refined.nud,
            mesh.nud + split_u_count + 3 * selected_w_count,
            "Fortran Method-C allocates one split U plus three child-W internal U edges"
        );
        assert_eq!(
            refined.nwd,
            mesh.nwd + 3 * selected_w_count,
            "Fortran Method-C keeps the remapped parent W and adds three child W faces per subdivided W"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_single_circle_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("Method-C nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (435, 1297, 865),
            "reduced Fortran probe summary: nmd=435 nud=1297 nwd=865"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 864)]),
            "reduced Fortran probe summary: all 864 active W faces have ngr=2"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(12), 864),
            "reduced Fortran probe summary: mrow min=-6 max=12 count=864"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp7_single_circle_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("NXP7 Method-C nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (565, 1687, 1125),
            "reduced Fortran NXP7 probe summary: nmd=565 nud=1687 nwd=1125"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 57), (2, 1067)]),
            "reduced Fortran NXP7 probe summary: W-face ngr counts are ngr1=57 and ngr2=1067"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(13), 1067),
            "reduced Fortran NXP7 probe summary: mrow min=-6 max=13 count=1067"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![2_500_000.0, 2_500_000.0],
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("Method-C corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (474, 1414, 943),
            "reduced Fortran corridor probe summary: nmd=474 nud=1414 nwd=943"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 942)]),
            "reduced Fortran corridor probe summary: all 942 active W faces have ngr=2"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(12), 942),
            "reduced Fortran corridor probe summary: mrow min=-6 max=12 count=942"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp7_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![2_500_000.0, 2_500_000.0],
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("NXP7 Method-C corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (643, 1921, 1281),
            "reduced Fortran NXP7 corridor probe summary: nmd=643 nud=1921 nwd=1281"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 25), (2, 1255)]),
            "reduced Fortran NXP7 corridor probe summary: W-face ngr counts are ngr1=25 and ngr2=1255"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-8), Some(13), 1255),
            "reduced Fortran NXP7 corridor probe summary: mrow min=-8 max=13 count=1255"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_variable_radius_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![2_500_000.0, 1_250_000.0],
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("Method-C variable-radius corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (435, 1297, 865),
            "reduced Fortran variable-radius corridor probe summary: nmd=435 nud=1297 nwd=865"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 864)]),
            "reduced Fortran variable-radius corridor probe summary: all 864 active W faces have ngr=2"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(12), 864),
            "reduced Fortran variable-radius corridor probe summary: mrow min=-6 max=12 count=864"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_three_point_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
                LonLatDegrees::new(150.0, 0.0),
            ],
            radius_meters: vec![2_500_000.0, 2_500_000.0, 2_500_000.0],
            level: 1,
        };

        let refined = mesh
            .spawn_nest_as_atmosmesh(&[region], 1)
            .expect("Method-C three-point corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (552, 1648, 1099),
            "reduced Fortran three-point corridor probe summary: nmd=552 nud=1648 nwd=1099"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 1098)]),
            "reduced Fortran three-point corridor probe summary: all 1098 active W faces have ngr=2"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-9), Some(12), 1098),
            "reduced Fortran three-point corridor probe summary: mrow min=-9 max=12 count=1098"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_two_level_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(115.0, 25.0),
                    LonLatDegrees::new(130.0, 25.0),
                ],
                radius_meters: vec![6_000_000.0, 6_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(120.0, 25.0),
                    LonLatDegrees::new(125.0, 25.0),
                ],
                radius_meters: vec![1_000_000.0, 1_000_000.0],
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("two-level Method-C corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (783, 2341, 1561),
            "reduced Fortran two-level corridor probe summary: nmd=783 nud=2341 nwd=1561"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 294), (3, 1266)]),
            "reduced Fortran two-level corridor probe summary: W-face ngr counts are ngr2=294 and ngr3=1266"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(11), 1560),
            "reduced Fortran two-level corridor probe summary: mrow min=-6 max=11 count=1560"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp7_two_level_corridor_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(115.0, 25.0),
                    LonLatDegrees::new(130.0, 25.0),
                ],
                radius_meters: vec![2_500_000.0, 2_500_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(120.0, 25.0),
                    LonLatDegrees::new(125.0, 25.0),
                ],
                radius_meters: vec![500_000.0, 500_000.0],
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("NXP7 two-level Method-C corridor nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (715, 2137, 1425),
            "reduced Fortran NXP7 two-level corridor probe summary: nmd=715 nud=2137 nwd=1425"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 25), (2, 287), (3, 1112)]),
            "reduced Fortran NXP7 two-level corridor probe summary: W-face ngr counts are ngr1=25, ngr2=287, ngr3=1112"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(13), 1399),
            "reduced Fortran NXP7 two-level corridor probe summary: mrow min=-6 max=13 count=1399"
        );
    }

    #[test]
    fn olam_method_c_rejects_reduced_fortran_nxp6_two_level_corridor_too_close_boundary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(115.0, 25.0),
                    LonLatDegrees::new(130.0, 25.0),
                ],
                radius_meters: vec![6_000_000.0, 6_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(115.0, 25.0),
                    LonLatDegrees::new(130.0, 25.0),
                ],
                radius_meters: vec![1_000_000.0, 1_000_000.0],
                level: 2,
            },
        ];

        let error = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect_err("reduced Fortran probe rejects this same-length two-level corridor as too close to the parent boundary");
        let message = error.to_string();
        assert!(
            message.contains("crosses")
                || message.contains("too close")
                || message.contains("parent boundary")
                || message.contains("next coarser grid boundary"),
            "Rust should reject the same invalid two-level corridor as the reduced Fortran probe; got {error}"
        );
        assert!(
            !message.contains("cannot be grouped into transition triples"),
            "Rust should reject this invalid two-level corridor before Method-C perimeter triple grouping; got {error}"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp6_two_circle_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 4_000_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("two-level Method-C nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (624, 1864, 1243),
            "reduced Fortran probe summary: nmd=624 nud=1864 nwd=1243"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(2, 154), (3, 1088)]),
            "reduced Fortran probe summary: W-face ngr counts are ngr2=154 and ngr3=1088"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(11), 1242),
            "reduced Fortran probe summary: mrow min=-6 max=11 count=1242"
        );
    }

    #[test]
    fn olam_method_c_matches_reduced_fortran_nxp7_two_circle_summary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 3_000_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("NXP7 two-level Method-C circle nest matching reduced Fortran probe");
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        let mut mrow_values = Vec::new();
        for iw in 2..=refined.nwd {
            let face = refined.w_faces[iw];
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
            if face.mrow != 0 {
                mrow_values.push(face.mrow);
            }
        }
        mrow_values.sort_unstable();

        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (754, 2254, 1503),
            "reduced Fortran NXP7 two-circle probe summary: nmd=754 nud=2254 nwd=1503"
        );
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 3), (2, 335), (3, 1164)]),
            "reduced Fortran NXP7 two-circle probe summary: W-face ngr counts are ngr1=3, ngr2=335, ngr3=1164"
        );
        assert_eq!(
            (
                mrow_values.first().copied(),
                mrow_values.last().copied(),
                mrow_values.len()
            ),
            (Some(-6), Some(13), 1499),
            "reduced Fortran NXP7 two-circle probe summary: mrow min=-6 max=13 count=1499"
        );
    }

    #[test]
    fn olam_method_c_repairs_nxp7_circle_parent_radius_that_fortran_overruns_perimeter() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 4_000_000.0,
            level: 1,
        }];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 1)
            .expect("Rust should repair the non-triplet parent perimeter instead of reproducing Fortran's perim_fill3 overrun");
        refined
            .validate_topology()
            .expect("repaired nxp7 circle topology");
        for im in 2..=refined.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "repaired nxp7 circle M point {im} exceeds OLAM-supported valence"
            );
        }
    }

    #[test]
    fn olam_method_c_repairs_nxp7_corridor_parent_radius_that_fortran_overruns_perimeter() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [OlamRefinementRegion::Corridor {
            points: vec![
                LonLatDegrees::new(115.0, 25.0),
                LonLatDegrees::new(130.0, 25.0),
            ],
            radius_meters: vec![4_000_000.0, 4_000_000.0],
            level: 1,
        }];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 1)
            .expect("Rust should repair the non-triplet corridor parent perimeter instead of reproducing Fortran's perim_fill3 overrun");
        refined
            .validate_topology()
            .expect("repaired nxp7 corridor topology");
        for im in 2..=refined.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "repaired nxp7 corridor M point {im} exceeds OLAM-supported valence"
            );
        }
    }

    #[test]
    fn olam_method_c_anneals_nxp7_two_circle_after_repaired_parent() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 4_000_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("child mask should anneal after repaired nxp7 parent circle");
        refined
            .validate_topology()
            .expect("annealed nxp7 two-circle topology");
        for im in 2..=refined.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "annealed nxp7 two-circle M point {im} exceeds OLAM-supported valence"
            );
        }
    }

    #[test]
    fn olam_method_c_anneals_nxp7_two_corridor_after_repaired_parent() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(7, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(115.0, 25.0),
                    LonLatDegrees::new(130.0, 25.0),
                ],
                radius_meters: vec![4_000_000.0, 4_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(120.0, 25.0),
                    LonLatDegrees::new(125.0, 25.0),
                ],
                radius_meters: vec![500_000.0, 500_000.0],
                level: 2,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect("child mask should anneal after repaired nxp7 parent corridor");
        refined
            .validate_topology()
            .expect("annealed nxp7 two-corridor topology");
        for im in 2..=refined.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "annealed nxp7 two-corridor M point {im} exceeds OLAM-supported valence"
            );
        }
    }

    #[test]
    fn olam_method_c_rejects_reduced_fortran_nxp6_two_circle_too_close_boundary() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let regions = [
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 2_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ];

        let error = mesh
            .spawn_nest_as_atmosmesh(&regions, 2)
            .expect_err("reduced Fortran probe rejects this two-level circle as too close to the parent boundary");
        let message = error.to_string();
        assert!(
            message.contains("crosses")
                || message.contains("too close")
                || message.contains("parent boundary")
                || message.contains("next coarser grid boundary"),
            "Rust should reject the same invalid two-level circle as the reduced Fortran probe; got {error}"
        );
        assert!(
            !message.contains("cannot be grouped into transition triples"),
            "Rust should reject this invalid two-level circle before Method-C perimeter triple grouping; got {error}"
        );
    }

    #[test]
    fn olam_method_c_child_w_ids_follow_fortran_parent_then_three_children_order() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iwnew = vec![1usize; mesh.nwd + 1];
        let mut expected_child_w = vec![[1usize; 3]; mesh.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=mesh.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                expected_child_w[iw][0] = iwnext;
                iwnext += 1;
                expected_child_w[iw][1] = iwnext;
                iwnext += 1;
                expected_child_w[iw][2] = iwnext;
            }
            iwnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iw in 2..=mesh.nwd {
            if !nest_wd[iw].is_subdivided() {
                continue;
            }
            let parent_id = iwnew[iw];
            assert_eq!(
                expected_child_w[iw],
                [parent_id + 1, parent_id + 2, parent_id + 3],
                "Fortran iwnew places three child W ids immediately after parent W {iw}"
            );
            assert_eq!(refined.w_faces[parent_id].mrlw, mesh.w_faces[iw].mrlw + 1);
            assert_eq!(
                refined.w_faces[parent_id].mrlw_orig,
                mesh.w_faces[iw].mrlw_orig,
                "Fortran promotes remapped full-subdivision parent W mrlw but preserves mrlw_orig"
            );
            assert_eq!(refined.w_faces[parent_id].ngr, 2);
            for child_id in expected_child_w[iw] {
                assert_eq!(refined.w_faces[child_id].mrlw, mesh.w_faces[iw].mrlw + 1);
                assert_eq!(refined.w_faces[child_id].mrlw_orig, mesh.w_faces[iw].mrlw + 1);
                assert_eq!(refined.w_faces[child_id].ngr, 2);
                assert!(
                    refined.w_faces[child_id].im.iter().all(|&im| im > 1),
                    "Fortran tri_neighbors should rebuild child W {child_id} M vertices from Method-C U endpoints"
                );
                for &iu in &refined.w_faces[child_id].iu {
                    assert!(
                        refined.u_edges[iu]
                            .im
                            .iter()
                            .all(|endpoint| refined.w_faces[child_id].im.contains(endpoint)),
                        "child W {child_id} U edge {iu} should use only that W face's M vertices"
                    );
                }
                checked += 1;
            }
        }

        assert!(checked > 0, "test should exercise subdivided Method-C W faces");
    }

    #[test]
    fn olam_method_c_internal_u_ids_follow_fortran_first_seen_w_order() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iwnew = vec![1usize; mesh.nwd + 1];
        let mut expected_child_w = vec![[1usize; 3]; mesh.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=mesh.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                expected_child_w[iw][0] = iwnext;
                iwnext += 1;
                expected_child_w[iw][1] = iwnext;
                iwnext += 1;
                expected_child_w[iw][2] = iwnext;
            }
            iwnext += 1;
        }

        let mut expected_internal_u = vec![[1usize; 3]; mesh.nwd + 1];
        let mut iunew = vec![1usize; mesh.nud + 1];
        let mut expected_second_u = vec![1usize; mesh.nud + 1];
        let mut iwdiv = vec![false; mesh.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=mesh.nud {
            iunew[iu] = iunext;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if !nest_wd[iw1].is_suppressed() && !nest_wd[iw2].is_suppressed() {
                    iunext += 1;
                    expected_second_u[iu] = iunext;
                } else {
                    expected_second_u[iu] = iunew[iu];
                }
            }
            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 1;
                        expected_internal_u[iw][0] = iunext;
                        iunext += 1;
                        expected_internal_u[iw][1] = iunext;
                        iunext += 1;
                        expected_internal_u[iw][2] = iunext;
                    }
                }
            }
            iunext += 1;
        }

        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        for im in 2..=mesh.nmd {
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iw in 2..=mesh.nwd {
            if !nest_wd[iw].is_subdivided() {
                continue;
            }
            if !mesh.w_faces[iw]
                .iw
                .iter()
                .take(3)
                .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
            {
                continue;
            }
            let parent_id = iwnew[iw];
            assert_eq!(
                refined.w_faces[parent_id].iu,
                expected_internal_u[iw],
                "Fortran writes nest_wd(iw)%iu(1:3) to the remapped parent W face {iw}"
            );
            for (slot, child_id) in expected_child_w[iw].into_iter().enumerate() {
                assert_eq!(
                    refined.w_faces[child_id].iu[0],
                    expected_internal_u[iw][slot],
                    "Fortran writes internal U edge {} as child W {child_id}'s first U",
                    expected_internal_u[iw][slot]
                );
                checked += 1;
            }
            let midpoint_ids = mesh.w_faces[iw].iu.map(|iu| expected_midpoint_m[iu]);
            let mut actual_pairs = expected_internal_u[iw]
                .into_iter()
                .map(|iu| {
                    let mut endpoints = refined.u_edges[iu].im;
                    endpoints.sort_unstable();
                    endpoints
                })
                .collect::<Vec<_>>();
            actual_pairs.sort_unstable();
            let mut expected_pairs = [
                [midpoint_ids[0], midpoint_ids[1]],
                [midpoint_ids[0], midpoint_ids[2]],
                [midpoint_ids[1], midpoint_ids[2]],
            ];
            for pair in &mut expected_pairs {
                pair.sort_unstable();
            }
            expected_pairs.sort_unstable();
            assert_eq!(
                actual_pairs,
                expected_pairs,
                "Fortran full-subdivision internal U edges connect the three split-edge midpoint M ids for W face {iw}"
            );
            let mut actual_parent_vertices = refined.w_faces[parent_id].im;
            actual_parent_vertices.sort_unstable();
            let mut expected_parent_vertices = midpoint_ids;
            expected_parent_vertices.sort_unstable();
            assert_eq!(
                actual_parent_vertices,
                expected_parent_vertices,
                "Fortran full-subdivision remapped parent W face {parent_id} should be the central midpoint triangle for old W face {iw}"
            );
            for &iu in &refined.w_faces[parent_id].iu {
                assert!(
                    refined.u_edges[iu]
                        .im
                        .iter()
                        .all(|endpoint| refined.w_faces[parent_id].im.contains(endpoint)),
                    "central remapped W face {parent_id} U edge {iu} should use only midpoint vertices"
                );
            }
            let w_family = [
                parent_id,
                expected_child_w[iw][0],
                expected_child_w[iw][1],
                expected_child_w[iw][2],
            ];
            for &iu in &mesh.w_faces[iw].iu {
                assert!(
                    refined.u_edges[iunew[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|face| w_family.contains(face)),
                    "Fortran remapped first half of split-U {iu} should touch W face family for subdivided W {iw}"
                );
                assert!(
                    refined.u_edges[expected_second_u[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|face| w_family.contains(face)),
                    "Fortran second half of split-U {iu} should touch W face family for subdivided W {iw}"
                );
            }
            let expected_split_child_faces = [
                if iw == mesh.u_edges[mesh.w_faces[iw].iu[0]].iw[0] {
                    (expected_child_w[iw][2], expected_child_w[iw][1])
                } else {
                    (expected_child_w[iw][1], expected_child_w[iw][2])
                },
                if iw == mesh.u_edges[mesh.w_faces[iw].iu[1]].iw[0] {
                    (expected_child_w[iw][0], expected_child_w[iw][2])
                } else {
                    (expected_child_w[iw][2], expected_child_w[iw][0])
                },
                if iw == mesh.u_edges[mesh.w_faces[iw].iu[2]].iw[0] {
                    (expected_child_w[iw][1], expected_child_w[iw][0])
                } else {
                    (expected_child_w[iw][0], expected_child_w[iw][1])
                },
            ];
            for (slot, &iu) in mesh.w_faces[iw].iu.iter().enumerate() {
                let (first_half_child, second_half_child) = expected_split_child_faces[slot];
                assert!(
                    refined.u_edges[iunew[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|&face| face == first_half_child),
                    "Fortran full-subdivision split-U {iu} first half should touch child W {first_half_child} for old W {iw} edge slot {slot}"
                );
                assert!(
                    refined.u_edges[expected_second_u[iu]]
                        .iw
                        .iter()
                        .take(2)
                        .any(|&face| face == second_half_child),
                    "Fortran full-subdivision split-U {iu} second half should touch child W {second_half_child} for old W {iw} edge slot {slot}"
                );
            }
            let [iu1o, iu2o, iu3o] = mesh.w_faces[iw].iu;
            let expected_child_iu = [
                [
                    expected_internal_u[iw][0],
                    if iw == mesh.u_edges[iu2o].iw[0] {
                        iunew[iu2o]
                    } else {
                        expected_second_u[iu2o]
                    },
                    if iw == mesh.u_edges[iu3o].iw[0] {
                        expected_second_u[iu3o]
                    } else {
                        iunew[iu3o]
                    },
                ],
                [
                    expected_internal_u[iw][1],
                    if iw == mesh.u_edges[iu3o].iw[0] {
                        iunew[iu3o]
                    } else {
                        expected_second_u[iu3o]
                    },
                    if iw == mesh.u_edges[iu1o].iw[0] {
                        expected_second_u[iu1o]
                    } else {
                        iunew[iu1o]
                    },
                ],
                [
                    expected_internal_u[iw][2],
                    if iw == mesh.u_edges[iu1o].iw[0] {
                        iunew[iu1o]
                    } else {
                        expected_second_u[iu1o]
                    },
                    if iw == mesh.u_edges[iu2o].iw[0] {
                        expected_second_u[iu2o]
                    } else {
                        iunew[iu2o]
                    },
                ],
            ];
            for (slot, child_id) in expected_child_w[iw].into_iter().enumerate() {
                assert_eq!(
                    refined.w_faces[child_id].iu,
                    expected_child_iu[slot],
                    "Fortran ltab_wd child W {child_id} should preserve exact Method-C U-edge slot order for old W {iw}"
                );
            }
        }

        assert!(checked > 0, "test should exercise interior full-subdivision W faces");
    }

    #[test]
    fn olam_method_c_split_u_second_half_ids_follow_fortran_iunew_order() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iunew = vec![1usize; mesh.nud + 1];
        let mut expected_second_u = vec![1usize; mesh.nud + 1];
        let mut iwdiv = vec![false; mesh.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=mesh.nud {
            iunew[iu] = iunext;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    expected_second_u[iu] = iunew[iu];
                } else {
                    iunext += 1;
                    expected_second_u[iu] = iunext;
                }
            }

            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 3;
                    }
                }
            }
            iunext += 1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                    if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                        expected_midpoint_m[iu] = 1;
                    } else {
                        imnext += 1;
                        expected_midpoint_m[iu] = imnext;
                    }
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iu in 2..=mesh.nud {
            if expected_second_u[iu] == 1 || expected_second_u[iu] == iunew[iu] {
                continue;
            }
            let old = mesh.u_edges[iu];
            let [iw1, iw2] = [old.iw[0], old.iw[1]];
            if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
                continue;
            }
            let midpoint = expected_midpoint_m[iu];
            let remapped_im1 = imnew[old.im[0]];
            let remapped_im2 = imnew[old.im[1]];
            let first_half = refined.u_edges[iunew[iu]].im;
            let second_half = refined.u_edges[expected_second_u[iu]].im;
            assert!(
                first_half.contains(&midpoint) || second_half.contains(&midpoint),
                "Fortran split-U {iu} should connect a half-edge to midpoint M id {midpoint}"
            );
            assert!(
                first_half.contains(&remapped_im1)
                    || first_half.contains(&remapped_im2)
                    || second_half.contains(&remapped_im1)
                    || second_half.contains(&remapped_im2),
                "Fortran split-U {iu} half-edges should retain a remapped old endpoint"
            );
            let midpoint_count = first_half
                .into_iter()
                .chain(second_half)
                .filter(|&endpoint| endpoint == midpoint)
                .count();
            assert_eq!(
                midpoint_count, 2,
                "Fortran split-U {iu} half-edges should share midpoint M id {midpoint}"
            );
            checked += 1;
        }

        assert!(checked > 0, "test should exercise non-suppressed split-U second halves");
    }

    #[test]
    fn olam_method_c_split_u_m_metadata_marks_child_ownership() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iu in 2..=mesh.nud {
            let midpoint = expected_midpoint_m[iu];
            if midpoint <= 1 {
                continue;
            }
            let old = mesh.u_edges[iu];
            let [iw1, iw2] = [old.iw[0], old.iw[1]];
            if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
                continue;
            }
            for &old_im in &old.im {
                let remapped = imnew[old_im];
                assert_eq!(
                    refined.m_metadata[remapped].mrlm, 2,
                    "Fortran Method-C split-U {iu} raises old endpoint M {old_im} to child mrlm"
                );
                assert_eq!(
                    refined.m_metadata[remapped].ngr, 2,
                    "Fortran Method-C split-U {iu} marks old endpoint M {old_im} with child grid ownership"
                );
            }
            assert_eq!(
                refined.m_metadata[midpoint].mrlm, 2,
                "Fortran Method-C split-U {iu} gives new midpoint M {midpoint} child mrlm"
            );
            assert_eq!(
                refined.m_metadata[midpoint].mrlm_orig, 2,
                "Fortran Method-C split-U {iu} gives new midpoint M {midpoint} child original ownership"
            );
            assert_eq!(
                refined.m_metadata[midpoint].ngr, 2,
                "Fortran Method-C split-U {iu} marks new midpoint M {midpoint} with child grid ownership"
            );
            checked += 1;
        }

        assert!(checked > 0, "test should exercise non-suppressed split-U M metadata");
    }

    #[test]
    fn olam_method_c_split_u_midpoint_coordinates_match_fortran_edge_average_projection() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        for im in 2..=mesh.nmd {
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iu in 2..=mesh.nud {
            let midpoint = expected_midpoint_m[iu];
            if midpoint <= 1 {
                continue;
            }
            let old = mesh.u_edges[iu];
            let [iw1, iw2] = [old.iw[0], old.iw[1]];
            if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
                continue;
            }
            if ![iw1, iw2].into_iter().all(|iw| {
                mesh.w_faces[iw]
                    .iw
                    .iter()
                    .take(3)
                    .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
            }) {
                continue;
            }
            let linear_midpoint =
                weighted_point(mesh.m_points[old.im[0]], 1.0, mesh.m_points[old.im[1]], 1.0)
                    .expect("Fortran midpoint average");
            let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
                .expect("Fortran final radius projection");
            let actual = refined.m_points[midpoint];
            let delta = magnitude(CartesianPoint::new(
                actual.x - expected.x,
                actual.y - expected.y,
                actual.z - expected.z,
            ));
            assert!(
                delta < 1.0e-6,
                "Fortran Method-C split-U {iu} midpoint M {midpoint} should be edge-average projected to radius; delta={delta}"
            );
            checked += 1;
        }

        assert!(
            checked > 0,
            "test should exercise interior non-suppressed split-U midpoint coordinates"
        );
    }

    #[test]
    fn olam_method_c_cartesian_split_u_midpoint_coordinates_match_native_edge_average() {
        let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0)
            .expect("cart_hex OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10_200_000.0, -310_000.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, true)
            .expect("selected Cartesian Method-C faces");
        let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        for im in 2..=mesh.nmd {
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                false,
            )
            .expect("Cartesian Method-C pass");
        let mut checked = 0usize;
        for iu in 2..=mesh.nud {
            let midpoint = expected_midpoint_m[iu];
            if midpoint <= 1 {
                continue;
            }
            let old = mesh.u_edges[iu];
            let [iw1, iw2] = [old.iw[0], old.iw[1]];
            if !(nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()) {
                continue;
            }
            if ![iw1, iw2].into_iter().all(|iw| {
                mesh.w_faces[iw]
                    .iw
                    .iter()
                    .take(3)
                    .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
            }) {
                continue;
            }
            let expected =
                weighted_point(mesh.m_points[old.im[0]], 1.0, mesh.m_points[old.im[1]], 1.0)
                    .expect("Fortran Cartesian midpoint average");
            let actual = refined.m_points[midpoint];
            let delta = magnitude(CartesianPoint::new(
                actual.x - expected.x,
                actual.y - expected.y,
                actual.z - expected.z,
            ));
            assert!(
                delta < 1.0e-9,
                "Fortran Cartesian Method-C split-U {iu} midpoint M {midpoint} should be native edge-average without radius projection; delta={delta}"
            );
            checked += 1;
        }

        assert!(
            checked > 0,
            "test should exercise full-interior Cartesian split-U midpoint coordinates"
        );
    }

    #[test]
    fn olam_method_c_full_subdivision_child_w_vertices_match_fortran_geometry() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                    expected_midpoint_m[iu] = imnext;
                }
            }
            imnext += 1;
        }

        let mut expected_parent_w = vec![1usize; mesh.nwd + 1];
        let mut expected_child_w = vec![[1usize; 3]; mesh.nwd + 1];
        let mut iwnext = 2usize;
        for iw in 2..=mesh.nwd {
            expected_parent_w[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                expected_child_w[iw][0] = iwnext;
                iwnext += 1;
                expected_child_w[iw][1] = iwnext;
                iwnext += 1;
                expected_child_w[iw][2] = iwnext;
            }
            iwnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iw in 2..=mesh.nwd {
            if !nest_wd[iw].is_subdivided() {
                continue;
            }
            if !mesh.w_faces[iw]
                .iw
                .iter()
                .take(3)
                .all(|&neighbor| neighbor > 1 && nest_wd[neighbor].is_subdivided())
            {
                continue;
            }

            let original_vertices = mesh.w_faces[iw].im.map(|im| imnew[im]);
            let midpoint_vertices = mesh.w_faces[iw]
                .iu
                .map(|iu| expected_midpoint_m[iu]);
            let parent_w = expected_parent_w[iw];
            let mut actual_parent_vertices = refined.w_faces[parent_w].im;
            actual_parent_vertices.sort_unstable();
            let mut expected_parent_vertices = midpoint_vertices;
            expected_parent_vertices.sort_unstable();
            assert_eq!(
                actual_parent_vertices,
                expected_parent_vertices,
                "Fortran remapped parent W {parent_w} for old W {iw} should be the central split-midpoint triangle"
            );
            for &vertex in &refined.w_faces[parent_w].im {
                let old_iu = mesh.w_faces[iw]
                    .iu
                    .into_iter()
                    .find(|&iu| expected_midpoint_m[iu] == vertex)
                    .expect("central parent midpoint vertex should map to old U edge");
                let edge = mesh.u_edges[old_iu];
                let linear_midpoint = weighted_point(
                    mesh.m_points[edge.im[0]],
                    1.0,
                    mesh.m_points[edge.im[1]],
                    1.0,
                )
                .expect("Fortran midpoint average");
                let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
                    .expect("Fortran final radius projection");
                let actual = refined.m_points[vertex];
                let delta = magnitude(CartesianPoint::new(
                    actual.x - expected.x,
                    actual.y - expected.y,
                    actual.z - expected.z,
                ));
                assert!(
                    delta < 1.0e-6,
                    "Fortran remapped parent W {parent_w} midpoint vertex {vertex} should be edge-average projected to radius; delta={delta}"
                );
            }
            for child_w in expected_child_w[iw] {
                let child_vertices = refined.w_faces[child_w].im;
                let original_count = child_vertices
                    .iter()
                    .filter(|vertex| original_vertices.contains(vertex))
                    .count();
                let midpoint_count = child_vertices
                    .iter()
                    .filter(|vertex| midpoint_vertices.contains(vertex))
                    .count();
                assert_eq!(
                    original_count, 1,
                    "Fortran child W {child_w} for old W {iw} should keep exactly one old M vertex"
                );
                assert_eq!(
                    midpoint_count, 2,
                    "Fortran child W {child_w} for old W {iw} should use exactly two split-U midpoint M vertices"
                );
                for &vertex in &child_vertices {
                    if original_vertices.contains(&vertex) {
                        continue;
                    }
                    let old_iu = mesh.w_faces[iw]
                        .iu
                        .into_iter()
                        .find(|&iu| expected_midpoint_m[iu] == vertex)
                        .expect("child midpoint vertex should map to old U edge");
                    let edge = mesh.u_edges[old_iu];
                    let linear_midpoint = weighted_point(
                        mesh.m_points[edge.im[0]],
                        1.0,
                        mesh.m_points[edge.im[1]],
                        1.0,
                    )
                    .expect("Fortran midpoint average");
                    let expected = normalize_cartesian_to_radius(linear_midpoint, radius)
                        .expect("Fortran final radius projection");
                    let actual = refined.m_points[vertex];
                    let delta = magnitude(CartesianPoint::new(
                        actual.x - expected.x,
                        actual.y - expected.y,
                        actual.z - expected.z,
                    ));
                    assert!(
                        delta < 1.0e-6,
                        "Fortran child W {child_w} midpoint vertex {vertex} should be edge-average projected to radius; delta={delta}"
                    );
                }
                checked += 1;
            }
        }

        assert!(
            checked > 0,
            "test should exercise full-interior Method-C child W face geometry"
        );
    }

    #[test]
    fn olam_method_c_suppressed_split_u_reuses_original_u_and_skips_midpoint_like_fortran() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iunew = vec![1usize; mesh.nud + 1];
        let mut expected_second_u = vec![1usize; mesh.nud + 1];
        let mut iwdiv = vec![false; mesh.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=mesh.nud {
            iunew[iu] = iunext;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    expected_second_u[iu] = iunew[iu];
                } else {
                    iunext += 1;
                    expected_second_u[iu] = iunext;
                }
            }

            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 3;
                    }
                }
            }
            iunext += 1;
        }

        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        for im in 2..=mesh.nmd {
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                    if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                        expected_midpoint_m[iu] = 1;
                    } else {
                        imnext += 1;
                        expected_midpoint_m[iu] = imnext;
                    }
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let mut checked = 0usize;
        for iu in 2..=mesh.nud {
            let old = mesh.u_edges[iu];
            let [iw1, iw2] = [old.iw[0], old.iw[1]];
            if !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed()) {
                continue;
            }
            if !(nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided()) {
                continue;
            }
            assert_eq!(
                expected_second_u[iu], iunew[iu],
                "Fortran suppressed split-U {iu} reuses iunew(iu) instead of allocating a second half"
            );
            assert_eq!(
                expected_midpoint_m[iu], 1,
                "Fortran suppressed split-U {iu} sets nest_ud(iu)%im = 1"
            );
            assert!(
                !refined.u_edges[iunew[iu]].im.contains(&1),
                "suppressed split-U {iu} should not reference a new midpoint M id"
            );
            checked += 1;
        }

        assert!(checked > 0, "test should exercise suppressed Method-C split-U edges");
    }

    #[test]
    fn olam_method_c_remaps_impent_through_fortran_imnew_table() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                    if !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed()) {
                        imnext += 1;
                    }
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let expected_impent = mesh.impent.map(|im| imnew[im]);

        assert_eq!(
            refined.impent, expected_impent,
            "Fortran spawn_nest remaps impent through imnew after Method-C table allocation"
        );
    }

    #[test]
    fn olam_method_c_remaps_prognostic_partners_through_fortran_tables() {
        let mut mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iwnew = vec![1usize; mesh.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=mesh.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 3;
            }
            iwnext += 1;
        }

        let mut iunew = vec![1usize; mesh.nud + 1];
        let mut iwdiv = vec![false; mesh.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=mesh.nud {
            iunew[iu] = iunext;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
            {
                iunext += 1;
            }
            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 3;
                    }
                }
            }
            iunext += 1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                    && !(nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed())
                {
                    imnext += 1;
                }
            }
            imnext += 1;
        }

        let m_pair = (mesh.impent[0], mesh.impent[1]);
        let u_pair = (2usize, 3usize);
        let w_pair = (2usize, 3usize);
        mesh.m_prognostic[m_pair.0] = m_pair.1;
        mesh.u_prognostic[u_pair.0] = u_pair.1;
        mesh.w_prognostic[w_pair.0] = w_pair.1;

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");

        assert_eq!(
            refined.m_prognostic[imnew[m_pair.0]], imnew[m_pair.1],
            "Fortran Method-C remaps M prognostic partner through imnew"
        );
        assert_eq!(
            refined.u_prognostic[iunew[u_pair.0]], iunew[u_pair.1],
            "Fortran Method-C remaps U prognostic partner through iunew"
        );
        assert_eq!(
            refined.w_prognostic[iwnew[w_pair.0]], iwnew[w_pair.1],
            "Fortran Method-C remaps W prognostic partner through iwnew"
        );
    }

    #[test]
    fn olam_method_c_emits_closed_topology_without_placeholder_neighbor_ids() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");

        refined
            .validate_topology()
            .expect("Method-C output topology should be closed");
        for iu in 2..=refined.nud {
            assert!(
                refined.u_edges[iu].im.iter().all(|&im| im > 1),
                "U edge {iu} should not contain placeholder M endpoint"
            );
            assert!(
                refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
                "U edge {iu} should not contain placeholder adjacent W face"
            );
        }
        for iw in 2..=refined.nwd {
            assert!(
                refined.w_faces[iw].im.iter().all(|&im| im > 1),
                "W face {iw} should not contain placeholder M vertex"
            );
            assert!(
                refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
                "W face {iw} should not contain placeholder U edge"
            );
        }
        for im in 2..=refined.nmd {
            let neighbors = refined.m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                assert!(iu > 1, "M point {im} should not contain placeholder U neighbor");
            }
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                assert!(iw > 1, "M point {im} should not contain placeholder W neighbor");
            }
        }
    }

    #[test]
    fn olam_method_c_multiple_regions_emit_projected_closed_outputs() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        let cases = [
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 2_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(115.0, 25.0),
                radius_meters: 3_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(15.0, 45.0),
                radius_meters: 2_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: LonLatDegrees::new(-75.0, 10.0),
                radius_meters: 2_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Bbox {
                west_degrees: 110.0,
                east_degrees: 120.0,
                south_degrees: 20.0,
                north_degrees: 30.0,
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![
                    LonLatDegrees::new(110.0, 24.0),
                    LonLatDegrees::new(120.0, 26.0),
                ],
                radius_meters: vec![1_500_000.0, 1_500_000.0],
                level: 1,
            },
            OlamRefinementRegion::Polygon {
                points: vec![
                    LonLatDegrees::new(110.0, 20.0),
                    LonLatDegrees::new(120.0, 20.0),
                    LonLatDegrees::new(120.0, 30.0),
                    LonLatDegrees::new(110.0, 30.0),
                ],
                level: 1,
            },
        ];

        for region in cases {
            let selected = mesh
                .selected_region_faces(&region, 1, false)
                .expect("selected Method-C faces");
            let selected_parent_mrl = (2..=mesh.nwd)
                .find(|&iw| selected.get(iw).copied().unwrap_or(false))
                .map(|iw| mesh.w_faces[iw].mrlw);
            let mut expected_selected = selected.clone();
            mesh.close_olam_method_c_concavities_for_level_with_neighbors(
                &mut expected_selected,
                &method_c_m_neighbors,
            )
            .expect("Method-C closure");
            if let Some(parent_mrl) = selected_parent_mrl {
                for iw in 2..=mesh.nwd {
                    if mesh.w_faces[iw].mrlw != parent_mrl {
                        expected_selected[iw] = false;
                    }
                }
            }
            let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
            for iw in 2..=mesh.nwd {
                if expected_selected[iw] {
                    nest_wd[iw].iw[2] = 1;
                }
            }
            let perimeter = mesh
                .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
                .expect("Method-C perimeter");
            for triple in perimeter.chunks_exact(3) {
                let center = triple[1];
                let edge = mesh.u_edges[center.iu];
                let suppressed_w = if center.im == edge.im[0] {
                    edge.iw[1]
                } else {
                    edge.iw[0]
                };
                nest_wd[suppressed_w].iw[2] = -1;
            }
            let selected_w_count = (2..=mesh.nwd)
                .filter(|&iw| nest_wd[iw].is_subdivided())
                .count();
            let split_u_count = (2..=mesh.nud)
                .filter(|&iu| {
                    let edge = mesh.u_edges[iu];
                    let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                    (nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided())
                        && !nest_wd[iw1].is_suppressed()
                        && !nest_wd[iw2].is_suppressed()
                })
                .count();
            let refined = mesh
                .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
                .expect("Method-C pass");

            assert_eq!(
                refined.nmd,
                mesh.nmd + split_u_count,
                "Fortran Method-C allocates one midpoint M only for non-suppressed split U edges"
            );
            assert_eq!(
                refined.nud,
                mesh.nud + split_u_count + 3 * selected_w_count,
                "Fortran Method-C allocates one split U plus three child-W internal U edges"
            );
            assert_eq!(
                refined.nwd,
                mesh.nwd + 3 * selected_w_count,
                "Fortran Method-C keeps the remapped parent W and adds three child W faces per subdivided W"
            );
            refined
                .validate_topology()
                .expect("Method-C output topology should be closed");
            for im in 2..=refined.nmd {
                let delta = (magnitude(refined.m_points[im]) - radius).abs();
                assert!(
                    delta < 1.0e-6,
                    "Fortran spawn_nest final projection should place M point {im} on the active radius; delta={delta}"
                );
                let neighbors = refined.m_neighbors[im];
                for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                    assert!(iu > 1, "M point {im} should not contain placeholder U neighbor");
                }
                for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                    assert!(iw > 1, "M point {im} should not contain placeholder W neighbor");
                }
            }
            for iu in 2..=refined.nud {
                assert!(
                    refined.u_edges[iu].im.iter().all(|&im| im > 1),
                    "U edge {iu} should not contain placeholder M endpoint"
                );
                assert!(
                    refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
                    "U edge {iu} should not contain placeholder adjacent W face"
                );
            }
            for iw in 2..=refined.nwd {
                assert!(
                    refined.w_faces[iw].im.iter().all(|&im| im > 1),
                    "W face {iw} should not contain placeholder M vertex"
                );
                assert!(
                    refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
                    "W face {iw} should not contain placeholder U edge"
                );
            }
        }
    }

    #[test]
    fn olam_method_c_public_spawn_entrypoints_use_same_table_path() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let expected = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("direct Method-C pass");
        let expected_counts = (expected.nmd, expected.nud, expected.nwd);

        let surface = mesh
            .spawn_nest(std::slice::from_ref(&region), 1)
            .expect("public surface Method-C spawn");
        assert_eq!(
            (surface.nmd, surface.nud, surface.nwd),
            expected_counts,
            "spawn_nest should use the same Method-C table path as the direct pass"
        );
        surface.validate_topology().expect("surface Method-C topology");

        let surface_alias = mesh
            .spawn_nest_as_surface(std::slice::from_ref(&region), 1)
            .expect("public surface alias Method-C spawn");
        assert_eq!(
            (surface_alias.nmd, surface_alias.nud, surface_alias.nwd),
            expected_counts,
            "spawn_nest_as_surface should use the same Method-C table path as spawn_nest"
        );
        surface_alias
            .validate_topology()
            .expect("surface alias Method-C topology");

        let explicit = mesh
            .spawn_nest_with_max_mrows(std::slice::from_ref(&region), 1, 7)
            .expect("explicit-width Method-C spawn");
        assert_eq!(
            (explicit.nmd, explicit.nud, explicit.nwd),
            expected_counts,
            "spawn_nest_with_max_mrows should use the same Method-C table path"
        );
        explicit
            .validate_topology()
            .expect("explicit-width Method-C topology");

        let atmosphere = mesh
            .spawn_nest_as_atmosmesh(std::slice::from_ref(&region), 1)
            .expect("public atmosphere Method-C spawn");
        assert_eq!(
            (atmosphere.nmd, atmosphere.nud, atmosphere.nwd),
            expected_counts,
            "spawn_nest_as_atmosmesh should change mrow width without leaving the Method-C table path"
        );
        atmosphere
            .validate_topology()
            .expect("atmosphere Method-C topology");

        let (spring, spring_passes) = mesh
            .spawn_nest_with_spring(std::slice::from_ref(&region), 1, 16, 0)
            .expect("public spring Method-C spawn");
        assert_eq!(spring_passes, 0);
        assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            expected_counts,
            "spawn_nest_with_spring should use the same Method-C table path before optional springing"
        );
        spring.validate_topology().expect("spring Method-C topology");

        let cart_mesh =
            OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0).expect("cart_hex OLAM mesh");
        let cart_region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10_200_000.0, -310_000.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let cart_selected = cart_mesh
            .selected_region_faces(&cart_region, 1, true)
            .expect("selected Cartesian Method-C faces");
        let cart_expected = cart_mesh
            .spawn_nest_pass_with_max_mrows(
                &cart_selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                false,
            )
            .expect("direct Cartesian Method-C pass");
        let cart_expected_counts = (cart_expected.nmd, cart_expected.nud, cart_expected.nwd);
        let cart_public = cart_mesh
            .spawn_nest_cartesian_xy_with_max_mrows(
                std::slice::from_ref(&cart_region),
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
            )
            .expect("public Cartesian Method-C spawn");
        assert_eq!(
            (cart_public.nmd, cart_public.nud, cart_public.nwd),
            cart_expected_counts,
            "spawn_nest_cartesian_xy_with_max_mrows should use the same Method-C table path as the direct Cartesian pass"
        );
        cart_public
            .validate_topology()
            .expect("Cartesian Method-C topology");

        let (cart_spring, cart_spring_passes) = cart_mesh
            .spawn_nest_cartesian_xy_with_spring_and_max_mrows(
                std::slice::from_ref(&cart_region),
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                18,
                0,
            )
            .expect("public Cartesian spring Method-C spawn");
        assert_eq!(cart_spring_passes, 0);
        assert_eq!(
            (cart_spring.nmd, cart_spring.nud, cart_spring.nwd),
            cart_expected_counts,
            "spawn_nest_cartesian_xy_with_spring_and_max_mrows should use the same Method-C table path before optional springing"
        );
        cart_spring
            .validate_topology()
            .expect("Cartesian spring Method-C topology");
    }

    #[test]
    fn olam_method_c_spring_niter_keeps_table_path_and_closed_topology() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let expected = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("direct Method-C pass");

        let (spring, spring_passes) = mesh
            .spawn_nest_with_spring(std::slice::from_ref(&region), 1, 16, 1)
            .expect("public spring Method-C spawn with iterations");

        assert_eq!(
            spring_passes, 1,
            "niter > 0 should run one OLAM nest spring pass after the active Method-C refinement pass"
        );
        assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "spring_nest should relax the Method-C table output without changing its Fortran allocation counts"
        );
        spring
            .validate_topology()
            .expect("spring-relaxed Method-C topology");
        for im in 2..=spring.nmd {
            let delta = (magnitude(spring.m_points[im]) - radius).abs();
            assert!(
                delta < 0.5,
                "spring-relaxed Method-C M point {im} should stay projected on the Fortran real-valued active radius; delta={delta}"
            );
        }
    }

    #[test]
    fn olam_method_c_cartesian_spring_niter_keeps_table_path_and_closed_topology() {
        let mesh = OlamDelaunayMesh::from_cart_hex(18, 1_000_000.0)
            .expect("cart_hex OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10_200_000.0, -310_000.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, true)
            .expect("selected Cartesian Method-C faces");
        let expected = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                false,
            )
            .expect("direct Cartesian Method-C pass");

        let (spring, spring_passes) = mesh
            .spawn_nest_cartesian_xy_with_spring_and_max_mrows(
                std::slice::from_ref(&region),
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                18,
                1,
            )
            .expect("public Cartesian spring Method-C spawn with iterations");

        assert_eq!(
            spring_passes, 1,
            "Cartesian niter > 0 should run one OLAM nest spring pass after the active Method-C refinement pass"
        );
        assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "Cartesian spring_nest should relax the Method-C table output without changing Fortran allocation counts"
        );
        spring
            .validate_topology()
            .expect("Cartesian spring-relaxed Method-C topology");
        for im in 2..=spring.nmd {
            let point = spring.m_points[im];
            assert!(
                point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
                "Cartesian spring-relaxed Method-C M point {im} should remain finite"
            );
        }
    }

    #[test]
    fn olam_method_c_cartesian_deltax_spring_niter_keeps_table_path_and_closed_topology() {
        let deltax = 1_000_000.0;
        let mesh = OlamDelaunayMesh::from_cart_hex(18, deltax)
            .expect("cart_hex OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(10_200_000.0, -310_000.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, true)
            .expect("selected Cartesian Method-C faces");
        let expected = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                false,
            )
            .expect("direct Cartesian Method-C pass");

        let (spring, spring_passes) = mesh
            .spawn_nest_cartesian_xy_with_spring_deltax_and_max_mrows(
                std::slice::from_ref(&region),
                1,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_ATMOS,
                18,
                1,
                deltax,
            )
            .expect("public Cartesian deltax spring Method-C spawn with iterations");

        assert_eq!(
            spring_passes, 1,
            "Cartesian deltax niter > 0 should run one OLAM nest spring pass after the active Method-C refinement pass"
        );
        assert_eq!(
            (spring.nmd, spring.nud, spring.nwd),
            (expected.nmd, expected.nud, expected.nwd),
            "Cartesian deltax spring_nest should relax the Method-C table output without changing Fortran allocation counts"
        );
        spring
            .validate_topology()
            .expect("Cartesian deltax spring-relaxed Method-C topology");
        for im in 2..=spring.nmd {
            let point = spring.m_points[im];
            assert!(
                point.x.is_finite() && point.y.is_finite() && point.z.is_finite(),
                "Cartesian deltax spring-relaxed Method-C M point {im} should remain finite"
            );
        }
    }

    #[test]
    fn olam_method_c_olamin_style_multilevel_corridor_table_outputs_closed_mesh() {
        let mesh = OlamDelaunayMesh::from_icosahedron(33, 5000, 1.25, 0.035, 100)
            .expect("base OLAM mesh")
            .expand_by_factor(2)
            .expect("Fortran expand_global2 base OLAM mesh");
        let path = vec![
            LonLatDegrees::new(-94.0, 25.0),
            LonLatDegrees::new(-95.0, 26.0),
        ];
        let regions = [
            OlamRefinementRegion::Corridor {
                points: path.clone(),
                radius_meters: vec![3_000_000.0, 3_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: path.clone(),
                radius_meters: vec![1_800_000.0, 1_800_000.0],
                level: 2,
            },
            OlamRefinementRegion::Corridor {
                points: path,
                radius_meters: vec![1_200_000.0, 1_200_000.0],
                level: 3,
            },
        ];

        let refined = mesh
            .spawn_nest_as_atmosmesh(&regions, 3)
            .expect("OLAMIN-style atmosphere Method-C corridor table nest");
        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (84_099, 252_289, 168_193),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C HDF5 M/U/W table sizes"
        );
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        for face in refined.w_faces.iter().skip(2) {
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
        }
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 76_426), (2, 11_468), (3, 15_114), (4, 65_184)]),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C per-grid W-face counts"
        );
        let mut mrows = refined
            .w_faces
            .iter()
            .skip(2)
            .filter_map(|face| (face.mrow != 0).then_some(face.mrow))
            .collect::<Vec<_>>();
        mrows.sort_unstable();
        assert_eq!(
            (mrows.first().copied(), mrows.last().copied(), mrows.len()),
            (Some(-13), Some(13), 50_069),
            "OLAMIN-style atmosphere Method-C corridor table output should match the Fortran Method-C atmosphere mrow envelope"
        );
        refined
            .validate_topology()
            .expect("OLAMIN-style Method-C table topology");
    }

    #[test]
    #[ignore = "runs three 5000-iteration atmosphere spring passes; use the table-only OLAMIN corridor test for default Method-C count/topology coverage"]
    fn olam_method_c_olamin_style_multilevel_corridor_outputs_closed_mesh() {
        let mesh = OlamDelaunayMesh::from_icosahedron(33, 5000, 1.25, 0.035, 100)
            .expect("base OLAM mesh")
            .expand_by_factor(2)
            .expect("Fortran expand_global2 base OLAM mesh");
        let path = vec![
            LonLatDegrees::new(-94.0, 25.0),
            LonLatDegrees::new(-95.0, 26.0),
        ];
        let regions = [
            OlamRefinementRegion::Corridor {
                points: path.clone(),
                radius_meters: vec![3_000_000.0, 3_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: path.clone(),
                radius_meters: vec![1_800_000.0, 1_800_000.0],
                level: 2,
            },
            OlamRefinementRegion::Corridor {
                points: path,
                radius_meters: vec![1_200_000.0, 1_200_000.0],
                level: 3,
            },
        ];

        let (refined, spring_passes) = mesh
            .spawn_nest_with_spring_as_atmosmesh(&regions, 3, 66, 5000)
            .expect("OLAMIN-style atmosphere Method-C corridor nest");
        assert_eq!(
            spring_passes, 3,
            "Fortran MAKEGRID runs one atmosphere spring pass after each active Method-C nest"
        );
        assert_eq!(
            (refined.nmd, refined.nud, refined.nwd),
            (84_099, 252_289, 168_193),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C HDF5 M/U/W table sizes"
        );
        let mut ngr_counts = BTreeMap::<usize, usize>::new();
        for face in refined.w_faces.iter().skip(2) {
            *ngr_counts.entry(face.ngr).or_insert(0) += 1;
        }
        assert_eq!(
            ngr_counts,
            BTreeMap::from([(1, 76_426), (2, 11_468), (3, 15_114), (4, 65_184)]),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C per-grid W-face counts"
        );
        let mut mrows = refined
            .w_faces
            .iter()
            .skip(2)
            .filter_map(|face| (face.mrow != 0).then_some(face.mrow))
            .collect::<Vec<_>>();
        mrows.sort_unstable();
        assert_eq!(
            (mrows.first().copied(), mrows.last().copied(), mrows.len()),
            (Some(-13), Some(13), 50_069),
            "OLAMIN-style atmosphere Method-C corridor output should match the Fortran Method-C atmosphere mrow envelope"
        );

        refined
            .validate_topology()
            .expect("OLAMIN-style Method-C topology");
        let grid_numbers = refined
            .w_faces
            .iter()
            .skip(2)
            .filter_map(|face| (face.ngr > 1).then_some(face.ngr))
            .collect::<BTreeSet<_>>();
        assert_eq!(
            grid_numbers,
            BTreeSet::from([2, 3, 4]),
            "Fortran Method-C spawns one grid number per OLAMIN corridor refinement level"
        );
        for iu in 2..=refined.nud {
            assert!(
                refined.u_edges[iu].im.iter().all(|&im| im > 1),
                "U edge {iu} should not contain placeholder M endpoint"
            );
            assert!(
                refined.u_edges[iu].iw.iter().take(2).all(|&iw| iw > 1),
                "U edge {iu} should not contain placeholder adjacent W face"
            );
        }
        for iw in 2..=refined.nwd {
            assert!(
                refined.w_faces[iw].im.iter().all(|&im| im > 1),
                "W face {iw} should not contain placeholder M vertex"
            );
            assert!(
                refined.w_faces[iw].iu.iter().all(|&iu| iu > 1),
                "W face {iw} should not contain placeholder U edge"
            );
        }
        for im in 2..=mesh.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "old M point {im} exceeds OLAM-supported valence after OLAMIN-style Method-C nesting"
            );
        }

        let adapted = voronoi_grid_from_olam_delaunay_mesh(
            &refined,
            active_mesh_radius(&refined).expect("active mesh radius"),
        )
        .expect("OLAMIN-style Method-C Voronoi handoff");
        assert_eq!(adapted.grid.nma, refined.nwd);
        assert_eq!(adapted.grid.nua, refined.nud);
        assert_eq!(adapted.grid.nwa, refined.nmd);
        for iw in 2..=refined.nwd {
            assert_eq!(
                adapted.tabs.m[iw].npoly as usize,
                refined.w_faces[iw].npoly,
                "Voronoi handoff should preserve Method-C W-face npoly for face {iw}"
            );
            assert_eq!(
                adapted.tabs.m[iw].ngr as usize,
                refined.w_faces[iw].ngr,
                "Voronoi handoff should preserve Method-C W-face grid number for face {iw}"
            );
        }
        for im in 2..=refined.nmd {
            assert_eq!(
                adapted.tabs.w[im].npoly as usize,
                refined.m_neighbors[im].npoly,
                "Voronoi handoff should preserve Method-C M-neighbor npoly for point {im}"
            );
        }
    }

    #[test]
    fn olam_method_c_midpoint_m_ids_follow_fortran_first_seen_edge_order() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C closure");

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut expected_midpoint_m = vec![1usize; mesh.nud + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            for &iu in method_c_m_neighbors[im]
                .iu
                .iter()
                .take(method_c_m_neighbors[im].npoly)
            {
                if iudiv[iu] {
                    continue;
                }
                iudiv[iu] = true;
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                    if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                        expected_midpoint_m[iu] = 1;
                    } else {
                        imnext += 1;
                        expected_midpoint_m[iu] = imnext;
                    }
                }
            }
            imnext += 1;
        }

        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
            .expect("Method-C pass without final projection");
        let checked = (2..=mesh.nud)
            .filter(|&iu| expected_midpoint_m[iu] > 1)
            .filter(|&iu| {
                let edge = mesh.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                nest_wd[iw1].is_subdivided() && nest_wd[iw2].is_subdivided()
            })
            .map(|iu| {
                let old_edge = mesh.u_edges[iu];
                let midpoint = expected_midpoint_m[iu];
                let remapped_im1 = imnew[old_edge.im[0]];
                let remapped_im2 = imnew[old_edge.im[1]];
                let has_half_edge = |endpoint: usize| {
                    refined.u_edges.iter().skip(2).any(|edge| {
                        edge.im.contains(&midpoint) && edge.im.contains(&endpoint)
                    })
                };
                assert!(
                    has_half_edge(remapped_im1) && has_half_edge(remapped_im2),
                    "Fortran assigns split-U {iu} midpoint to first-seen M id {midpoint} and connects both remapped endpoints"
                );
                1usize
            })
            .sum::<usize>();

        assert!(checked > 0, "test should exercise Method-C split-U midpoint ids");
    }

    #[test]
    fn olam_method_c_refinement_level_is_not_grid_number() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let child_grid_number = 4;
        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, child_grid_number, 7, true)
            .expect("Method-C pass with non-level grid number");

        let max_mrlm = refined
            .m_metadata
            .iter()
            .skip(2)
            .map(|metadata| metadata.mrlm)
            .max()
            .expect("M metadata");
        let max_mrlm_orig = refined
            .m_metadata
            .iter()
            .skip(2)
            .map(|metadata| metadata.mrlm_orig)
            .max()
            .expect("M original metadata");
        let max_mrlw = refined
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrlw)
            .max()
            .expect("W metadata");
        let max_mrlw_orig = refined
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.mrlw_orig)
            .max()
            .expect("W original metadata");

        assert!(
            max_mrlm <= 2 && max_mrlm_orig <= 2,
            "Fortran writes M refinement levels as parent mrlo + 1 independently of grid number; got max mrlm={max_mrlm}, max mrlm_orig={max_mrlm_orig}"
        );
        assert!(
            max_mrlw <= 2 && max_mrlw_orig <= 2,
            "Fortran writes W refinement levels as parent mrlo + 1 independently of grid number; got max mrlw={max_mrlw}, max mrlw_orig={max_mrlw_orig}"
        );
    }

    #[test]
    fn olam_method_c_keeps_fortran_linear_coordinates_before_projection() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
            .expect("Method-C pass without final projection");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        let off_radius_points = (2..=refined.nmd)
            .filter(|&im| refined.m_metadata[im].mrlm_orig == 2)
            .filter(|&im| (magnitude(refined.m_points[im]) - radius).abs() > 1.0e-6)
            .collect::<Vec<_>>();

        assert!(
            !off_radius_points.is_empty(),
            "Fortran perim_fill3 writes ordinary linear M coordinates before the later spawn_nest radius projection"
        );
    }

    #[test]
    fn olam_method_c_projection_matches_fortran_radius_expansion() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let linear = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, false)
            .expect("Method-C pass without final projection");
        let projected = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass with final projection");
        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        assert_eq!(linear.nmd, projected.nmd);

        for im in 2..=linear.nmd {
            let expected = normalize_cartesian_to_radius(linear.m_points[im], radius)
                .expect("Fortran radius expansion");
            let actual = projected.m_points[im];
            let delta = magnitude(CartesianPoint::new(
                actual.x - expected.x,
                actual.y - expected.y,
                actual.z - expected.z,
            ));
            assert!(
                delta < 1.0e-6,
                "Fortran spawn_nest projects M point {im} by radial expansion; delta={delta}"
            );
        }
    }

    #[test]
    fn olam_perim_fill3_writes_fortran_weighted_transition_coordinates() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let mut selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let selected_parent_mrl = (2..=mesh.nwd)
            .find(|&iw| selected.get(iw).copied().unwrap_or(false))
            .map(|iw| mesh.w_faces[iw].mrlw);
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M-neighbor table should derive");
        mesh.close_olam_method_c_concavities_for_level_with_neighbors(
            &mut selected,
            &method_c_m_neighbors,
        )
        .expect("Method-C concavity closure");
        if let Some(parent_mrl) = selected_parent_mrl {
            for iw in 2..=mesh.nwd {
                if mesh.w_faces[iw].mrlw != parent_mrl {
                    selected[iw] = false;
                }
            }
        }

        let mut nest_wd = vec![OlamMethodCNestWd::default(); mesh.nwd + 1];
        for iw in 2..=mesh.nwd {
            if selected[iw] {
                nest_wd[iw].iw[2] = 1;
            }
        }
        let perimeter = mesh
            .perim_map2_method_c(&nest_wd, &method_c_m_neighbors)
            .expect("Method-C perimeter");
        for triple in perimeter.chunks_exact(3) {
            let center = triple[1];
            let edge = mesh.u_edges[center.iu];
            let suppressed_w = if center.im == edge.im[0] {
                edge.iw[1]
            } else {
                edge.iw[0]
            };
            nest_wd[suppressed_w].iw[2] = -1;
        }

        let mut iwnew = vec![1usize; mesh.nwd + 1];
        let mut iwnext = 2usize;
        iwnew[1] = 1;
        for iw in 2..=mesh.nwd {
            iwnew[iw] = iwnext;
            if nest_wd[iw].is_subdivided() {
                iwnext += 1;
                nest_wd[iw].iw[0] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[1] = iwnext as isize;
                iwnext += 1;
                nest_wd[iw].iw[2] = iwnext as isize;
            }
            iwnext += 1;
        }
        let nwd0 = iwnext - 1;

        let mut nest_ud = vec![OlamMethodCNestUd::default(); mesh.nud + 1];
        let mut iunew = vec![1usize; mesh.nud + 1];
        let mut iwdiv = vec![false; mesh.nwd + 1];
        let mut iunext = 2usize;
        iunew[1] = 1;
        for iu in 2..=mesh.nud {
            iunew[iu] = iunext;
            let edge = mesh.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                    nest_ud[iu].iu = iunew[iu];
                } else {
                    iunext += 1;
                    nest_ud[iu].iu = iunext;
                }
            }

            for &iw in &edge.iw[0..2] {
                if !iwdiv[iw] {
                    iwdiv[iw] = true;
                    if nest_wd[iw].is_subdivided() {
                        iunext += 1;
                        nest_wd[iw].iu[0] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[1] = iunext;
                        iunext += 1;
                        nest_wd[iw].iu[2] = iunext;
                    }
                }
            }
            iunext += 1;
        }
        let nud0 = iunext - 1;

        let mut imnew = vec![1usize; mesh.nmd + 1];
        let mut iudiv = vec![false; mesh.nud + 1];
        let mut imnext = 2usize;
        imnew[1] = 1;
        for im in 2..=mesh.nmd {
            imnew[im] = imnext;
            let neighbors = method_c_m_neighbors[im];
            for &iu in neighbors.iu.iter().take(neighbors.npoly) {
                if !iudiv[iu] {
                    iudiv[iu] = true;
                    let edge = mesh.u_edges[iu];
                    let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                    if nest_wd[iw1].is_subdivided() || nest_wd[iw2].is_subdivided() {
                        if nest_wd[iw1].is_suppressed() || nest_wd[iw2].is_suppressed() {
                            nest_ud[iu].im = 1;
                        } else {
                            imnext += 1;
                            nest_ud[iu].im = imnext;
                        }
                    }
                }
            }
            imnext += 1;
        }
        let nmd0 = imnext - 1;

        let mut m_points = vec![CartesianPoint::new(0.0, 0.0, 0.0); nmd0 + 1];
        let mut m_metadata = default_olam_m_metadata(nmd0);
        let mut u_edges = vec![IcosahedronUEdge::default(); nud0 + 1];
        let mut w_faces = vec![IcosahedronWFace::default(); nwd0 + 1];

        for im in 2..=mesh.nmd {
            let imn = imnew[im];
            m_points[imn] = mesh.m_points[im];
            m_metadata[imn] = mesh.m_metadata[im];
        }
        for iu in 2..=mesh.nud {
            let iun = iunew[iu];
            let old = mesh.u_edges[iu];
            u_edges[iun] = IcosahedronUEdge {
                im: old.im.map(|im| imnew[im]),
                iw: old.iw.map(|iw| iwnew[iw]),
                iu: old.iu.map(|iu2| iunew[iu2]),
                mrlu: old.mrlu,
            };
            if nest_ud[iu].im > 1 {
                let im_mid = nest_ud[iu].im;
                let im1 = u_edges[iun].im[0];
                let im2 = u_edges[iun].im[1];
                m_points[im_mid] =
                    weighted_point(m_points[im1], 1.0, m_points[im2], 1.0).unwrap();
            }
        }
        for iw in 2..=mesh.nwd {
            let iwn = iwnew[iw];
            let old = mesh.w_faces[iw];
            w_faces[iwn] = IcosahedronWFace {
                npoly: old.npoly,
                im: old.im.map(|im| imnew[im]),
                iu: old.iu.map(|iu| iunew[iu]),
                iw: old.iw.map(|iw2| iwnew[iw2]),
                mrlw: old.mrlw,
                mrlw_orig: old.mrlw_orig,
                ngr: old.ngr,
                mrow: old.mrow,
            };
            if nest_wd[iw].is_subdivided() {
                mesh.fill_method_c_full_subdivision(
                    iw,
                    &iwnew,
                    &iunew,
                    &imnew,
                    2,
                    &nest_wd,
                    &nest_ud,
                    &mut u_edges,
                    &mut w_faces,
                )
                .expect("full Method-C face subdivision");
            }
        }

        let [p1, p2, p3] = [perimeter[0], perimeter[1], perimeter[2]];
        let [jm1, jm2, jm3] = [p1.im, p2.im, p3.im];
        let [ju1, ju2, ju3] = [p1.iu, p2.iu, p3.iu];
        let im16 = imnew[jm1];
        let im17 = nest_ud[ju1].im;
        let im18 = imnew[jm2];
        let im19 = imnew[jm3];
        let im20 = nest_ud[ju3].im;
        let iu43 = iunew[ju2];
        assert!(im17 > 1 && im20 > 1, "perim_fill3 test triple should have split endpoint M ids");

        let (iu41, iu42, iu46, iw26, iw27) = if jm1 == mesh.u_edges[ju1].im[0] {
            (
                iunew[ju1],
                nest_ud[ju1].iu,
                iunew[mesh.u_edges[ju1].iu[4]],
                iwnew[mesh.u_edges[ju1].iw[2]],
                iwnew[mesh.u_edges[ju1].iw[0]],
            )
        } else {
            (
                nest_ud[ju1].iu,
                iunew[ju1],
                iunew[mesh.u_edges[ju1].iu[11]],
                iwnew[mesh.u_edges[ju1].iw[5]],
                iwnew[mesh.u_edges[ju1].iw[1]],
            )
        };
        let (iu49, iu50, iu34, iu35, iu48, iu51, iw6o, iw9o, iw6, iw9, iw29, iw20, iw28, iw30) =
            if jm2 == mesh.u_edges[ju2].im[0] {
            (
                iunew[mesh.u_edges[ju2].iu[0]],
                iunew[mesh.u_edges[ju2].iu[1]],
                iunew[mesh.u_edges[ju2].iu[2]],
                iunew[mesh.u_edges[ju2].iu[3]],
                iunew[mesh.u_edges[ju2].iu[4]],
                iunew[mesh.u_edges[ju2].iu[7]],
                mesh.u_edges[ju2].iw[4],
                mesh.u_edges[ju2].iw[5],
                iwnew[mesh.u_edges[ju2].iw[4]],
                iwnew[mesh.u_edges[ju2].iw[5]],
                iwnew[mesh.u_edges[ju2].iw[0]],
                iwnew[mesh.u_edges[ju2].iw[1]],
                iwnew[mesh.u_edges[ju2].iw[2]],
                iwnew[mesh.u_edges[ju2].iw[3]],
            )
        } else {
            (
                iunew[mesh.u_edges[ju2].iu[3]],
                iunew[mesh.u_edges[ju2].iu[2]],
                iunew[mesh.u_edges[ju2].iu[1]],
                iunew[mesh.u_edges[ju2].iu[0]],
                iunew[mesh.u_edges[ju2].iu[11]],
                iunew[mesh.u_edges[ju2].iu[8]],
                mesh.u_edges[ju2].iw[3],
                mesh.u_edges[ju2].iw[2],
                iwnew[mesh.u_edges[ju2].iw[3]],
                iwnew[mesh.u_edges[ju2].iw[2]],
                iwnew[mesh.u_edges[ju2].iw[1]],
                iwnew[mesh.u_edges[ju2].iw[0]],
                iwnew[mesh.u_edges[ju2].iw[5]],
                iwnew[mesh.u_edges[ju2].iw[4]],
            )
        };
        let (im21, iu44, iu45, iu53, iw31, iw32) = if jm3 == mesh.u_edges[ju3].im[0] {
            (
                imnew[mesh.u_edges[ju3].im[1]],
                iunew[ju3],
                nest_ud[ju3].iu,
                iunew[mesh.u_edges[ju3].iu[7]],
                iwnew[mesh.u_edges[ju3].iw[0]],
                iwnew[mesh.u_edges[ju3].iw[3]],
            )
        } else {
            (
                imnew[mesh.u_edges[ju3].im[0]],
                nest_ud[ju3].iu,
                iunew[ju3],
                iunew[mesh.u_edges[ju3].iu[8]],
                iwnew[mesh.u_edges[ju3].iw[1]],
                iwnew[mesh.u_edges[ju3].iw[4]],
            )
        };
        let im22 = fortran_other_endpoint_by_first(u_edges[iu46], im16);
        let im23 = fortran_other_endpoint_by_first(u_edges[iu48], im18);
        let im24 = fortran_other_endpoint_by_first(u_edges[iu49], im18);
        let im25 = fortran_other_endpoint_by_first(u_edges[iu51], im19);
        let im26 = fortran_other_endpoint_by_first(u_edges[iu53], im21);
        let im5 = if u_edges[iu34].im[0] == im18 {
            u_edges[iu34].im[1]
        } else {
            u_edges[iu34].im[0]
        };

        let [iu25, iu15] = method_c_split_outer_edges(nest_wd[iw6o].iu, &u_edges, "iw6")
            .expect("split outer edges for iw6");
        let iw19 = if u_edges[iu25].iw[0] == iw6 {
            u_edges[iu25].iw[1]
        } else {
            u_edges[iu25].iw[0]
        };
        let iw7 = if u_edges[iu15].iw[0] == iw6 {
            u_edges[iu15].iw[1]
        } else {
            u_edges[iu15].iw[0]
        };
        let iu33 = if w_faces[iw19].iu[0] == iu25 {
            w_faces[iw19].iu[1]
        } else if w_faces[iw19].iu[1] == iu25 {
            w_faces[iw19].iu[2]
        } else {
            w_faces[iw19].iu[0]
        };
        let im12 = if u_edges[iu25].iw[0] == iw6 {
            u_edges[iu25].im[1]
        } else {
            u_edges[iu25].im[0]
        };
        let [iu16, iu26] = method_c_split_outer_edges(nest_wd[iw9o].iu, &u_edges, "iw9")
            .expect("split outer edges for iw9");
        let iw8 = if u_edges[iu16].iw[0] == iw9 {
            u_edges[iu16].iw[1]
        } else {
            u_edges[iu16].iw[0]
        };
        let iw21 = if u_edges[iu26].iw[0] == iw9 {
            u_edges[iu26].iw[1]
        } else {
            u_edges[iu26].iw[0]
        };
        let im13 = if u_edges[iu26].iw[0] == iw9 {
            u_edges[iu26].im[0]
        } else {
            u_edges[iu26].im[1]
        };

        let pre_points = m_points.clone();
        let expected_im19 =
            weighted_point(pre_points[im24], 1.0, pre_points[im5], 1.0).unwrap();
        let expected_im18 =
            weighted_point(expected_im19, 1.0, pre_points[im5], 1.0).unwrap();
        let expected_im17 =
            weighted_point(pre_points[im17], 0.75, expected_im19, 0.25).unwrap();
        let expected_im20 =
            weighted_point(pre_points[im20], 0.75, expected_im19, 0.25).unwrap();
        let expected_im12 =
            weighted_point(pre_points[im12], 0.833, expected_im18, 0.167).unwrap();
        let expected_im13 =
            weighted_point(pre_points[im13], 0.833, expected_im18, 0.167).unwrap();
        let parent_level = selected_parent_mrl.unwrap_or(1);
        let expected_im17_mrlm_orig = m_metadata[im18].mrlm_orig;
        let expected_im20_mrlm_orig = m_metadata[im19].mrlm_orig;
        let expected_neighbor_ownership = [im22, im23, im24, im25, im26]
            .map(|im| (im, m_metadata[im].mrlm, m_metadata[im].mrlm_orig));
        let mut expected_iw8_iu = w_faces[iw8].iu;
        if expected_iw8_iu[0] == iu16 {
            expected_iw8_iu[2] = iu34;
        } else if expected_iw8_iu[1] == iu16 {
            expected_iw8_iu[0] = iu34;
        } else {
            expected_iw8_iu[1] = iu34;
        }
        let mut expected_iw19_iu = w_faces[iw19].iu;
        if expected_iw19_iu[0] == iu25 {
            expected_iw19_iu[2] = iu35;
        } else if expected_iw19_iu[1] == iu25 {
            expected_iw19_iu[0] = iu35;
        } else {
            expected_iw19_iu[1] = iu35;
        }
        let mut expected_iw20_iu = w_faces[iw20].iu;
        if expected_iw20_iu[0] == iu43 {
            expected_iw20_iu[1] = iu42;
            expected_iw20_iu[2] = iu49;
        } else if expected_iw20_iu[1] == iu43 {
            expected_iw20_iu[2] = iu42;
            expected_iw20_iu[0] = iu49;
        } else {
            expected_iw20_iu[0] = iu42;
            expected_iw20_iu[1] = iu49;
        }
        let mut expected_iw27_iu = w_faces[iw27].iu;
        if expected_iw27_iu[0] == iu48 {
            expected_iw27_iu[1] = iu41;
        } else if expected_iw27_iu[1] == iu48 {
            expected_iw27_iu[2] = iu41;
        } else {
            expected_iw27_iu[0] = iu41;
        }
        let mut expected_iw29_iu = w_faces[iw29].iu;
        if expected_iw29_iu[0] == iu50 {
            expected_iw29_iu[1] = iu44;
            expected_iw29_iu[2] = iu43;
        } else if expected_iw29_iu[1] == iu50 {
            expected_iw29_iu[2] = iu44;
            expected_iw29_iu[0] = iu43;
        } else {
            expected_iw29_iu[0] = iu44;
            expected_iw29_iu[1] = iu43;
        }
        let mut expected_iw31_iu = w_faces[iw31].iu;
        if expected_iw31_iu[0] == iu51 {
            expected_iw31_iu[2] = iu45;
        } else if expected_iw31_iu[1] == iu51 {
            expected_iw31_iu[0] = iu45;
        } else {
            expected_iw31_iu[1] = iu45;
        }
        let mut expected_iu34 = u_edges[iu34];
        if expected_iu34.im[0] == im18 {
            expected_iu34.iw = set_first_two(expected_iu34.iw, iw8, iw7);
        } else {
            expected_iu34.iw = set_first_two(expected_iu34.iw, iw7, iw8);
        }
        let mut expected_iu35 = u_edges[iu35];
        if expected_iu35.im[0] == im19 {
            expected_iu35.iw[1] = iw19;
            expected_iu35.iw[0] = iw21;
            expected_iu35.im[1] = im18;
        } else {
            expected_iu35.iw[0] = iw19;
            expected_iu35.iw[1] = iw21;
            expected_iu35.im[0] = im18;
        }
        let mut expected_iu41 = u_edges[iu41];
        if expected_iu41.im[1] == im17 {
            expected_iu41.iw[0] = iw27;
        } else {
            expected_iu41.iw[1] = iw27;
        }
        let mut expected_iu42 = u_edges[iu42];
        if expected_iu42.im[0] == im17 {
            expected_iu42.im[1] = im19;
            expected_iu42.iw[0] = iw20;
        } else {
            expected_iu42.im[0] = im19;
            expected_iu42.iw[1] = iw20;
        }
        let mut expected_iu43 = u_edges[iu43];
        if expected_iu43.im[1] == im19 {
            expected_iu43.im[0] = im24;
        } else {
            expected_iu43.im[1] = im24;
        }
        let mut expected_iu44 = u_edges[iu44];
        if expected_iu44.im[0] == im19 {
            expected_iu44.iw[0] = iw29;
        } else {
            expected_iu44.iw[1] = iw29;
        }
        let mut expected_iu45 = u_edges[iu45];
        if expected_iu45.im[0] == im20 {
            expected_iu45.iw[0] = iw31;
        } else {
            expected_iu45.iw[1] = iw31;
        }
        let mut expected_iu48 = u_edges[iu48];
        if expected_iu48.iw[1] == iw27 {
            expected_iu48.im[1] = im17;
        } else {
            expected_iu48.im[0] = im17;
        }
        let mut expected_iu49 = u_edges[iu49];
        if expected_iu49.im[1] == im24 {
            expected_iu49.im[0] = im17;
            expected_iu49.iw[1] = iw20;
        } else {
            expected_iu49.im[1] = im17;
            expected_iu49.iw[0] = iw20;
        }
        let mut expected_iu50 = u_edges[iu50];
        if expected_iu50.im[0] == im24 {
            expected_iu50.im[1] = im20;
        } else {
            expected_iu50.im[0] = im20;
        }
        let mut expected_iu51 = u_edges[iu51];
        if expected_iu51.iw[1] == iw31 {
            expected_iu51.im[0] = im20;
        } else {
            expected_iu51.im[1] = im20;
        }
        let mut expected_iu33 = u_edges[iu33];
        if expected_iu33.iw[1] == iw19 {
            expected_iu33.im[1] = im19;
        } else {
            expected_iu33.im[0] = im19;
        }

        let radius = active_mesh_radius(&mesh).expect("active mesh radius");
        mesh.perim_fill3_method_c(
            &perimeter[0..3],
            parent_level,
            &iwnew,
            &iunew,
            &imnew,
            &nest_wd,
            &mut nest_ud,
            &mut u_edges,
            &mut w_faces,
            &mut m_points,
            &mut m_metadata,
            radius,
            2,
        )
        .expect("perim_fill3 first transition triple");

        let assert_point = |label: &str, actual: CartesianPoint, expected: CartesianPoint| {
            let delta = magnitude(CartesianPoint::new(
                actual.x - expected.x,
                actual.y - expected.y,
                actual.z - expected.z,
            ));
            assert!(
                delta < 1.0e-9,
                "{label} should match Fortran perim_fill3 weighted coordinate formula; delta={delta}"
            );
        };
        assert_point("im19", m_points[im19], expected_im19);
        assert_point("im18", m_points[im18], expected_im18);
        assert_point("im17", m_points[im17], expected_im17);
        assert_point("im20", m_points[im20], expected_im20);
        assert_point("im12", m_points[im12], expected_im12);
        assert_point("im13", m_points[im13], expected_im13);
        assert_eq!(m_metadata[im17].mrlm_orig, expected_im17_mrlm_orig);
        assert_eq!(m_metadata[im20].mrlm_orig, expected_im20_mrlm_orig);
        assert_eq!(m_metadata[im18].mrlm_orig, parent_level + 1);
        assert_eq!(m_metadata[im19].mrlm_orig, parent_level + 1);
        for (im, expected_mrlm, expected_mrlm_orig) in expected_neighbor_ownership {
            assert_eq!(m_metadata[im].ngr, 2);
            assert_eq!(
                m_metadata[im].mrlm, expected_mrlm,
                "Fortran perim_fill3 sets ngr for transition neighbor M {im} without changing mrlm ownership"
            );
            assert_eq!(
                m_metadata[im].mrlm_orig, expected_mrlm_orig,
                "Fortran perim_fill3 sets ngr for transition neighbor M {im} without changing mrlm_orig ownership"
            );
        }
        for iw in [iw20, iw26, iw27, iw28, iw29, iw30, iw31, iw32] {
            assert_eq!(w_faces[iw].ngr, 2);
        }
        let has_edge = |iw: usize, iu: usize| w_faces[iw].iu.iter().take(3).any(|&edge| edge == iu);
        assert!(has_edge(iw8, iu34));
        assert!(has_edge(iw19, iu35));
        assert!(has_edge(iw20, iu42) && has_edge(iw20, iu49));
        assert!(has_edge(iw27, iu41));
        assert!(has_edge(iw29, iu44) && has_edge(iw29, iunew[ju2]));
        assert!(has_edge(iw31, iu45));
        assert_eq!(w_faces[iw8].iu, expected_iw8_iu);
        assert_eq!(w_faces[iw19].iu, expected_iw19_iu);
        assert_eq!(w_faces[iw20].iu, expected_iw20_iu);
        assert_eq!(w_faces[iw27].iu, expected_iw27_iu);
        assert_eq!(w_faces[iw29].iu, expected_iw29_iu);
        assert_eq!(w_faces[iw31].iu, expected_iw31_iu);
        for (iu, expected) in [
            (iu33, expected_iu33),
            (iu34, expected_iu34),
            (iu35, expected_iu35),
            (iu41, expected_iu41),
            (iu42, expected_iu42),
            (iu43, expected_iu43),
            (iu44, expected_iu44),
            (iu45, expected_iu45),
            (iu48, expected_iu48),
            (iu49, expected_iu49),
            (iu50, expected_iu50),
            (iu51, expected_iu51),
        ] {
            assert_eq!(
                u_edges[iu].im, expected.im,
                "Fortran perim_fill3 should preserve exact endpoint slot order for U edge {iu}"
            );
            assert_eq!(
                [u_edges[iu].iw[0], u_edges[iu].iw[1]],
                [expected.iw[0], expected.iw[1]],
                "Fortran perim_fill3 should preserve exact adjacent-W slot order for U edge {iu}"
            );
        }
        let has_m_endpoint = |iu: usize, im: usize| u_edges[iu].im.iter().any(|&endpoint| endpoint == im);
        assert!(has_m_endpoint(iu15, im18));
        assert!(has_m_endpoint(iu16, im18));
        assert!(has_m_endpoint(iu25, im18));
        assert!(has_m_endpoint(iu26, im18));
        assert!(has_m_endpoint(iu33, im19));
        assert!(has_m_endpoint(iu35, im18) && has_m_endpoint(iu35, im19));
        assert!(has_m_endpoint(iu42, im17) && has_m_endpoint(iu42, im19));
        assert!(has_m_endpoint(iunew[ju2], im19) && has_m_endpoint(iunew[ju2], im24));
        assert!(has_m_endpoint(iu48, im17));
        assert!(has_m_endpoint(iu49, im17) && has_m_endpoint(iu49, im24));
        assert!(has_m_endpoint(iu50, im24) && has_m_endpoint(iu50, im20));
        assert!(has_m_endpoint(iu51, im20));
        let has_w_face = |iu: usize, iw: usize| u_edges[iu].iw.iter().take(2).any(|&face| face == iw);
        assert!(has_w_face(iu41, iw27));
        assert!(has_w_face(iu42, iw20));
        assert!(has_w_face(iu44, iw29));
        assert!(has_w_face(iu45, iw31));
        assert!(has_w_face(iu49, iw20));
    }

    #[test]
    fn olam_method_c_projects_points_to_radius_before_neighbor_rebuild() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("selected Method-C faces");
        let refined = mesh
            .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true)
            .expect("Method-C pass");
        let radius = active_mesh_radius(&refined).expect("active mesh radius");
        let off_radius_points = (2..=refined.nmd)
            .filter(|&im| refined.m_metadata[im].mrlm_orig == 2)
            .filter(|&im| (magnitude(refined.m_points[im]) - radius).abs() > 1.0e-6)
            .collect::<Vec<_>>();

        assert!(
            off_radius_points.is_empty(),
            "Fortran spawn_nest projects all Method-C M coordinates back to Earth radius before tri_neighbors/perim_mrow/spring; off-radius M ids: {off_radius_points:?}"
        );
    }

    #[test]
    fn olam_selected_faces_use_current_parent_mrl_inside_existing_nest() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let first_region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(-120.0, 0.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let first = mesh
            .spawn_nest(&[first_region], 1)
            .expect("first Method-C nest");
        let nested_point = (2..=first.nmd)
            .find(|&im| {
                let neighbors = first.m_neighbors[im];
                first.m_metadata[im].mrlm == 2
                    && neighbors.npoly == 6
                    && neighbors
                        .iu
                        .iter()
                        .take(neighbors.npoly)
                        .all(|&iu| first.u_edges[iu].mrlu == 2)
            })
            .expect("first nest should create an interior level-2 M point");
        let region = OlamRefinementRegion::Circle {
            center: xyz_to_lonlat_degrees(first.m_points[nested_point]),
            radius_meters: 1.0,
            level: 1,
        };

        let selected = first
            .selected_region_faces(&region, 1, false)
            .expect("selected inner Method-C faces");
        let selected_levels = selected
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, selected)| selected.then_some(first.w_faces[iw].mrlw))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            selected_levels,
            BTreeSet::from([2]),
            "Fortran derives mrlo from the current starting M point, not from the pass counter"
        );
    }

    #[test]
    fn olam_selected_faces_parent_halo_keeps_current_parent_mrl() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let first_region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(-120.0, 0.0),
            radius_meters: 500_000.0,
            level: 1,
        };
        let first = mesh
            .spawn_nest(&[first_region], 1)
            .expect("first Method-C nest");
        let nested_point = (2..=first.nmd)
            .find(|&im| {
                let neighbors = first.m_neighbors[im];
                first.m_metadata[im].mrlm == 2
                    && neighbors.npoly == 6
                    && neighbors
                        .iu
                        .iter()
                        .take(neighbors.npoly)
                        .all(|&iu| first.u_edges[iu].mrlu == 2)
            })
            .expect("first nest should create an interior level-2 M point");
        let region = OlamRefinementRegion::Circle {
            center: xyz_to_lonlat_degrees(first.m_points[nested_point]),
            radius_meters: 1.0,
            level: 2,
        };

        let selected = first
            .selected_region_faces(&region, 1, false)
            .expect("selected inner Method-C faces with parent halo");
        let selected_levels = selected
            .iter()
            .enumerate()
            .skip(2)
            .filter_map(|(iw, selected)| selected.then_some(first.w_faces[iw].mrlw))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            selected_levels,
            BTreeSet::from([2]),
            "Fortran Method-C expands one current parent MRL at a time; selected levels were {selected_levels:?}"
        );
    }

    #[test]
    fn method_c_refines_locally_and_caps_old_m_valence() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let region = OlamRefinementRegion::Circle {
            center: LonLatDegrees::new(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        };

        let refined = mesh.spawn_nest(&[region], 1).expect("Method-C nest");
        let global_doubled = mesh.expand_global2().expect("global factor-2 expansion");

        assert!(refined.nmd > mesh.nmd);
        assert!(refined.nud > mesh.nud);
        assert!(refined.nwd > mesh.nwd);
        assert!(
            refined.nwd < global_doubled.nwd,
            "specified-region Method-C spawn should remain local, not refine the whole globe"
        );
        refined
            .validate_topology()
            .expect("valid Method-C refinement topology");
        for im in 2..=mesh.nmd {
            assert!(
                refined.m_neighbors[im].npoly <= 7,
                "old M point {im} exceeds OLAM-supported valence after Method-C closure"
            );
        }
    }

    #[test]
    fn spawn_nest_rejects_all_active_selection_instead_of_global_fallback() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let mut selected = vec![false; mesh.nwd + 1];
        for item in selected.iter_mut().take(mesh.nwd + 1).skip(2) {
            *item = true;
        }

        let err = mesh
            .spawn_nest_pass_with_max_mrows(
                &selected,
                2,
                OlamDelaunayMesh::METHOD_C_MAX_MROWS_SURFACE,
                true,
            )
            .expect_err("Method-C should not replace all-active selection with global expansion");

        assert!(
            err.to_string().contains("no nwdiv == 2 convex start point"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn olam_region_start_prefers_contained_global_pentagon() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("mesh radius");
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .expect("Method-C test neighbors");
        let pentagon_id = mesh.impent[0];
        let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);

        let mut chosen_region = None;
        for lon_offset in (-40..=40).step_by(4) {
            for lat_offset in (-20..=20).step_by(4) {
                if lon_offset == 0 && lat_offset == 0 {
                    continue;
                }
                let region = OlamRefinementRegion::Circle {
                    center: LonLatDegrees::new(
                        pentagon_lonlat.lon_degrees + lon_offset as f64,
                        pentagon_lonlat.lat_degrees + lat_offset as f64,
                    ),
                    radius_meters: 3_000_000.0,
                    level: 1,
                };
                if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                    && mesh
                        .closest_m_point_to_region_anchor(&region, false)
                        .expect("closest anchor")
                        != pentagon_id
                {
                    chosen_region = Some(region);
                    break;
                }
            }
            if chosen_region.is_some() {
                break;
            }
        }
        let region = chosen_region.expect("test region containing pentagon but centered elsewhere");

        let start = mesh
            .olam_refinement_start_point_with_neighbors(
                &region,
                radius,
                &method_c_m_neighbors,
                false,
            )
            .expect("OLAM start point");

        assert_eq!(
            start, pentagon_id,
            "OLAM spawn_nest should use a contained global impent as IMBEG before falling back to the nearest center point"
        );
    }

    #[test]
    fn olam_region_start_marches_from_nearby_global_pentagon() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("mesh radius");
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .expect("Method-C test neighbors");

        let mut selected_case = None;
        'search: for &pentagon_id in &mesh.impent {
            let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
            for lon_offset in (-36..=36).step_by(3) {
                for lat_offset in (-24..=24).step_by(3) {
                    if lon_offset == 0 && lat_offset == 0 {
                        continue;
                    }
                    for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                        let region = OlamRefinementRegion::Circle {
                            center: LonLatDegrees::new(
                                pentagon_lonlat.lon_degrees + lon_offset as f64,
                                pentagon_lonlat.lat_degrees + lat_offset as f64,
                            ),
                            radius_meters,
                            level: 1,
                        };
                        if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                            || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                        {
                            continue;
                        }
                        let Some(expected_start) =
                            olam_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                        else {
                            continue;
                        };
                        let closest = mesh
                            .closest_m_point_to_region_anchor(&region, false)
                            .expect("closest anchor");
                        if expected_start != closest {
                            selected_case = Some((region, expected_start));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (region, expected_start) =
            selected_case.expect("near-pentagon circle requiring OLAM impen march");

        let start = mesh
            .olam_refinement_start_point_with_neighbors(
                &region,
                radius,
                &method_c_m_neighbors,
                false,
            )
            .expect("OLAM start point");

        assert_eq!(
            start, expected_start,
            "OLAM spawn_nest should march from a nearby impent toward the nearest inside M point before falling back to the geometric center"
        );
    }

    #[test]
    fn olam_region_start_skips_nearby_global_pentagon_with_different_mrlm() {
        let mut mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("mesh radius");
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .expect("Method-C test neighbors");

        let mut selected_case = None;
        'search: for &pentagon_id in &mesh.impent {
            let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
            for lon_offset in (-36..=36).step_by(3) {
                for lat_offset in (-24..=24).step_by(3) {
                    if lon_offset == 0 && lat_offset == 0 {
                        continue;
                    }
                    for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                        let region = OlamRefinementRegion::Circle {
                            center: LonLatDegrees::new(
                                pentagon_lonlat.lon_degrees + lon_offset as f64,
                                pentagon_lonlat.lat_degrees + lat_offset as f64,
                            ),
                            radius_meters,
                            level: 1,
                        };
                        if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                            || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                        {
                            continue;
                        }
                        let closest = mesh
                            .closest_m_point_to_region_anchor(&region, false)
                            .expect("closest anchor");
                        if closest == pentagon_id {
                            continue;
                        }
                        if olam_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                            .is_some()
                        {
                            selected_case = Some((region, pentagon_id, closest));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (region, pentagon_id, closest) =
            selected_case.expect("near-pentagon case that would march with matching mrlm");
        mesh.m_metadata[pentagon_id].mrlm = mesh.m_metadata[closest].mrlm + 1;

        let start = mesh
            .olam_refinement_start_point_with_neighbors(
                &region,
                radius,
                &method_c_m_neighbors,
                false,
            )
            .expect("OLAM start point");

        assert_eq!(
            start, closest,
            "Fortran only uses the nearby impent march when impent mrlm matches imcent mrlm"
        );
    }

    #[test]
    fn olam_near_pentagon_march_uses_marched_start_mrlm_for_parent_ownership() {
        let mut mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("mesh radius");
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .expect("Method-C test neighbors");

        let mut selected_case = None;
        'search: for &pentagon_id in &mesh.impent {
            let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
            for lon_offset in (-36..=36).step_by(3) {
                for lat_offset in (-24..=24).step_by(3) {
                    if lon_offset == 0 && lat_offset == 0 {
                        continue;
                    }
                    for radius_meters in [500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                        let region = OlamRefinementRegion::Circle {
                            center: LonLatDegrees::new(
                                pentagon_lonlat.lon_degrees + lon_offset as f64,
                                pentagon_lonlat.lat_degrees + lat_offset as f64,
                            ),
                            radius_meters,
                            level: 1,
                        };
                        if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                            || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                        {
                            continue;
                        }
                        let Some(expected_start) =
                            olam_impen_march_start_for_test(&mesh, pentagon_id, &region, radius)
                        else {
                            continue;
                        };
                        let closest = mesh
                            .closest_m_point_to_region_anchor(&region, false)
                            .expect("closest anchor");
                        if expected_start != closest && expected_start != pentagon_id {
                            selected_case = Some((region, pentagon_id, closest, expected_start));
                            break 'search;
                        }
                    }
                }
            }
        }
        let (region, pentagon_id, closest, expected_start) =
            selected_case.expect("near-pentagon march case with distinct impen/imcent/imbeg");
        mesh.m_metadata[pentagon_id].mrlm = 3;
        mesh.m_metadata[pentagon_id].mrlm_orig = 3;
        mesh.m_metadata[closest].mrlm = 3;
        mesh.m_metadata[closest].mrlm_orig = 3;
        assert_eq!(
            mesh.m_metadata[expected_start].mrlm, 1,
            "test requires marched IMBEG to remain on the parent level"
        );

        let start = mesh
            .olam_refinement_start_point_with_neighbors(
                &region,
                radius,
                &method_c_m_neighbors,
                false,
            )
            .expect("OLAM start point");
        let selected = mesh
            .selected_region_faces(&region, 1, false)
            .expect("Method-C selection should use marched IMBEG mrlm as mrlo");

        assert_eq!(start, expected_start);
        assert!(
            selected.iter().skip(2).any(|selected| *selected),
            "Fortran sets mrlo from marched IMBEG, not from nearby impen or imcent"
        );
    }

    #[test]
    fn olam_near_pentagon_march_preserves_fortran_jdone_between_steps() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let radius = active_mesh_radius(&mesh).expect("mesh radius");
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .expect("Method-C test neighbors");
        let mut checked = 0usize;

        for &pentagon_id in &mesh.impent {
            let pentagon_lonlat = xyz_to_lonlat_degrees(mesh.m_points[pentagon_id]);
            for lon_offset in (-60..=60).step_by(3) {
                for lat_offset in (-36..=36).step_by(3) {
                    if lon_offset == 0 && lat_offset == 0 {
                        continue;
                    }
                    for radius_meters in [250_000.0, 500_000.0, 750_000.0, 1_000_000.0, 1_250_000.0] {
                        let region = OlamRefinementRegion::Circle {
                            center: LonLatDegrees::new(
                                pentagon_lonlat.lon_degrees + lon_offset as f64,
                                pentagon_lonlat.lat_degrees + lat_offset as f64,
                            ),
                            radius_meters,
                            level: 1,
                        };
                        if region.contains_cartesian(mesh.m_points[pentagon_id], radius)
                            || !region.close_to_cartesian(mesh.m_points[pentagon_id], radius)
                        {
                            continue;
                        }
                        let expected =
                            olam_impen_march_start_fortran_jdone_for_test(
                                &mesh,
                                pentagon_id,
                                &region,
                                radius,
                            );
                        let actual = mesh
                            .olam_march_from_nearby_pentagon_to_region_with_neighbors(
                                pentagon_id,
                                &region,
                                radius,
                                &method_c_m_neighbors,
                                false,
                            )
                            .expect("OLAM near-pentagon march");
                        assert_eq!(
                            actual, expected,
                            "Fortran spawn_nest keeps jdone marks between near-pentagon march steps while clearing only the current row"
                        );
                        checked += 1;
                    }
                }
            }
        }

        assert!(
            checked > 0,
            "test should exercise at least one near-pentagon march case"
        );
    }

    #[test]
    fn olam_thirdm_walks_straight_opposite_edges_and_marks_reciprocal_done_like_fortran() {
        let mesh = OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100)
            .expect("base mesh should build");
        let method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M-neighbor table should derive");
        let start = (2..=mesh.nmd)
            .find(|&im| method_c_m_neighbors[im].npoly == 6)
            .expect("base mesh should contain a 6-edge M point");

        let iu = method_c_m_neighbors[start].iu[0];
        let imm = mesh
            .other_m_endpoint(iu, start)
            .expect("first U edge should have opposite M endpoint");
        let iuu = mesh
            .opposite_ring_u_edge_with_neighbors(imm, iu, &method_c_m_neighbors)
            .expect("Fortran thirdm should choose opposite edge at first M");
        let immm = mesh
            .other_m_endpoint(iuu, imm)
            .expect("second U edge should have opposite M endpoint");
        let iuuu = mesh
            .opposite_ring_u_edge_with_neighbors(immm, iuu, &method_c_m_neighbors)
            .expect("Fortran thirdm should choose opposite edge at second M");
        let expected_immmm = mesh
            .other_m_endpoint(iuuu, immm)
            .expect("third U edge should have opposite M endpoint");

        let mut jdone = vec![[false; 6]; mesh.nmd + 1];
        let thirdm_neighbors = mesh
            .olam_thirdm_neighbors_fortran_with_neighbors(
                start,
                &mut jdone,
                &method_c_m_neighbors,
            )
            .expect("thirdm should traverse ordinary 6-edge topology");

        assert_eq!(thirdm_neighbors.first().copied(), Some(expected_immmm));
        assert!(jdone[start][0]);
        let reciprocal_edge = method_c_m_neighbors[expected_immmm]
            .iu
            .iter()
            .take(method_c_m_neighbors[expected_immmm].npoly.min(6))
            .position(|&far_iu| far_iu == iuuu)
            .expect("far M point should contain the incoming third U edge");
        assert!(jdone[expected_immmm][reciprocal_edge]);
    }

    #[test]
    fn olam_thirdm_rejects_broken_topology_instead_of_skipping_path() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let mut method_c_m_neighbors = mesh
            .derive_icosahedron_m_neighbors_fortran()
            .expect("Method-C M neighbors");
        let start = (2..=mesh.nmd)
            .find(|&im| method_c_m_neighbors[im].npoly >= 2)
            .expect("M point with multiple U edges");
        let non_incident_iu = (2..=mesh.nud)
            .find(|&iu| !mesh.u_edges[iu].im.contains(&start))
            .expect("U edge not incident on selected M point");
        method_c_m_neighbors[start].iu[0] = non_incident_iu;
        let mut jdone = vec![[false; 6]; mesh.nmd + 1];

        let err = mesh
            .olam_thirdm_neighbors_fortran_with_neighbors(
                start,
                &mut jdone,
                &method_c_m_neighbors,
            )
            .expect_err("Fortran thirdm should not silently skip an invalid straight path");
        assert!(
            err.to_string().contains("not incident")
                || err.to_string().contains("not in M point"),
            "unexpected thirdm topology error: {err}"
        );
    }

    #[test]
    fn olam_thirdm_skips_intermediate_zero_npoly_path() {
        let mesh =
            OlamDelaunayMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base OLAM mesh");
        let mut method_c_m_neighbors =
            mesh.derive_icosahedron_m_neighbors_fortran().expect("Method-C M neighbors");
        let start = (2..=mesh.nmd)
            .find(|&im| {
                let mut jdone = vec![[false; 6]; mesh.nmd + 1];
                if method_c_m_neighbors[im].npoly < 2 {
                    return false;
                }
                mesh.olam_thirdm_neighbors_fortran_with_neighbors(im, &mut jdone, &method_c_m_neighbors)
                    .map(|neighbors| !neighbors.is_empty())
                    .unwrap_or(false)
            })
            .expect("M point with at least one computed third-m path");
        let iu = method_c_m_neighbors[start].iu[0];
        let imm = mesh.other_m_endpoint(iu, start).expect("neighbor on valid edge");
        method_c_m_neighbors[imm].npoly = 0;
        let mut jdone = vec![[false; 6]; mesh.nmd + 1];
        let neighbors = mesh
            .olam_thirdm_neighbors_fortran_with_neighbors(start, &mut jdone, &method_c_m_neighbors)
            .expect("thirdm should ignore malformed intermediate npoly entries");
        assert!(
            !neighbors.is_empty(),
            "zero-npoly intermediate should still allow at least one straight third-m path"
        );
    }

    fn olam_impen_march_start_fortran_jdone_for_test(
        mesh: &OlamDelaunayMesh,
        pentagon_id: usize,
        region: &OlamRefinementRegion,
        radius: f64,
    ) -> Option<usize> {
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .ok()?;
        let nearest_inside = mesh
            .nearest_inside_m_point_to(pentagon_id, region, radius, false)
            .ok()??;
        let mut current = pentagon_id;
        let mut visited = BTreeSet::new();
        let mut jdone = vec![[false; 6]; mesh.nmd + 1];
        for _ in 0..mesh.nmd {
            if !visited.insert(current) {
                return None;
            }
            jdone[current] = [false; 6];
            let neighbors = mesh
                .olam_thirdm_neighbors_fortran_with_neighbors(
                    current,
                    &mut jdone,
                    &method_c_m_neighbors,
                )
                .ok()?;
            let mut best_neighbor = None;
            let mut best_distance = f64::INFINITY;
            for neighbor in neighbors {
                if region.contains_cartesian(mesh.m_points[neighbor], radius) {
                    return Some(neighbor);
                }
                let distance =
                    cartesian_distance(mesh.m_points[neighbor], mesh.m_points[nearest_inside]);
                if distance < best_distance {
                    best_distance = distance;
                    best_neighbor = Some(neighbor);
                }
            }
            current = best_neighbor?;
        }
        None
    }

    fn olam_impen_march_start_for_test(
        mesh: &OlamDelaunayMesh,
        pentagon_id: usize,
        region: &OlamRefinementRegion,
        radius: f64,
    ) -> Option<usize> {
        let method_c_m_neighbors =
            derive_icosahedron_m_neighbors_fortran_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
                .ok()?;
        let mut nearest_inside = None;
        let mut nearest_distance = f64::INFINITY;
        for im in 2..=mesh.nmd {
            if !region.contains_cartesian(mesh.m_points[im], radius) {
                continue;
            }
            let distance = cartesian_distance(mesh.m_points[im], mesh.m_points[pentagon_id]);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_inside = Some(im);
            }
        }
        let nearest_inside = nearest_inside?;
        let mut current = pentagon_id;
        let mut visited = BTreeSet::new();
        let mut jdone = vec![[false; 6]; mesh.nmd + 1];
        for _ in 0..mesh.nmd {
            if !visited.insert(current) {
                return None;
            }
            jdone[current] = [false; 6];
            let neighbors = mesh
                .olam_thirdm_neighbors_fortran_with_neighbors(current, &mut jdone, &method_c_m_neighbors)
                .ok()?;
            let mut best_neighbor = None;
            let mut best_distance = f64::INFINITY;
            for neighbor in neighbors {
                if region.contains_cartesian(mesh.m_points[neighbor], radius) {
                    return Some(neighbor);
                }
                let distance =
                    cartesian_distance(mesh.m_points[neighbor], mesh.m_points[nearest_inside]);
                if distance < best_distance {
                    best_distance = distance;
                    best_neighbor = Some(neighbor);
                }
            }
            current = best_neighbor?;
        }
        None
    }

    fn cartesian_distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

}

/// Precomputed sine/cosine basis for an icosahedron polar stereographic pole.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleBasis {
    pub cos_lat: f64,
    pub sin_lat: f64,
    pub cos_lon: f64,
    pub sin_lon: f64,
}

impl PoleBasis {
    pub fn from_lonlat_radians(lon_radians: f64, lat_radians: f64) -> Self {
        Self {
            cos_lat: lat_radians.cos(),
            sin_lat: lat_radians.sin(),
            cos_lon: lon_radians.cos(),
            sin_lon: lon_radians.sin(),
        }
    }
}

/// Single-precision pole basis for `icosahedron.F90` `real` projection calls.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleBasisF32 {
    pub cos_lat: f32,
    pub sin_lat: f32,
    pub cos_lon: f32,
    pub sin_lon: f32,
}

impl PoleBasisF32 {
    pub fn from_lonlat_radians(lon_radians: f32, lat_radians: f32) -> Self {
        Self {
            cos_lat: lat_radians.cos(),
            sin_lat: lat_radians.sin(),
            cos_lon: lon_radians.cos(),
            sin_lon: lon_radians.sin(),
        }
    }
}

/// Point on the polar stereographic plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePoint {
    pub x: f64,
    pub y: f64,
}

impl PlanePoint {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// Single-precision point on the polar stereographic plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanePointF32 {
    pub x: f32,
    pub y: f32,
}

impl PlanePointF32 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Port of `icosahedron.F90:de_ps_r8`.
pub fn project_to_polar_stereographic(point: CartesianPoint, pole: PoleBasis) -> PlanePoint {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq =
        pole.cos_lat * point.z - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq =
        pole.sin_lat * point.z + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePoint::new(xq * t, yq * t)
}

/// Port of the single-precision `icosahedron.F90:de_ps`.
pub fn project_to_polar_stereographic_f32(
    point: CartesianPointF32,
    pole: PoleBasisF32,
) -> PlanePointF32 {
    let xq = -pole.sin_lon * point.x + pole.cos_lon * point.y;
    let yq =
        pole.cos_lat * point.z - pole.sin_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);
    let zq =
        pole.sin_lat * point.z + pole.cos_lat * (pole.cos_lon * point.x + pole.sin_lon * point.y);

    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS as f32 * 2.0;
    let t = earth_diameter / (earth_diameter + zq);

    PlanePointF32::new(xq * t, yq * t)
}

/// Port of `icosahedron.F90:ps_de_r8`.
pub fn unproject_from_polar_stereographic(point: PlanePoint, pole: PoleBasis) -> CartesianPoint {
    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS * 2.0;
    let earth_diameter_sq = earth_diameter * earth_diameter;
    let t = earth_diameter_sq / (point.x * point.x + point.y * point.y + earth_diameter_sq);

    let xq = point.x * t;
    let yq = point.y * t;
    let zq = earth_diameter * (t - 1.0);

    CartesianPoint::new(
        -pole.sin_lon * xq + pole.cos_lon * (-pole.sin_lat * yq + pole.cos_lat * zq),
        pole.cos_lon * xq - pole.sin_lon * (pole.sin_lat * yq - pole.cos_lat * zq),
        pole.cos_lat * yq + pole.sin_lat * zq,
    )
}

/// Port of the single-precision `icosahedron.F90:ps_de`.
pub fn unproject_from_polar_stereographic_f32(
    point: PlanePointF32,
    pole: PoleBasisF32,
) -> CartesianPointF32 {
    let earth_diameter = earthmesh_core::EARTH_RADIUS_METERS as f32 * 2.0;
    let earth_diameter_sq = earth_diameter * earth_diameter;
    let t = earth_diameter_sq / (point.x * point.x + point.y * point.y + earth_diameter_sq);

    let xq = point.x * t;
    let yq = point.y * t;
    let zq = earth_diameter * (t - 1.0);

    CartesianPointF32::new(
        -pole.sin_lon * xq + pole.cos_lon * (-pole.sin_lat * yq + pole.cos_lat * zq),
        pole.cos_lon * xq - pole.sin_lon * (pole.sin_lat * yq - pole.cos_lat * zq),
        pole.cos_lat * yq + pole.sin_lat * zq,
    )
}

/// Port of `MOD_grid_preprocess:centroid_spherical_single`.
///
/// Converts lon/lat vertices to unit Cartesian vectors, averages components,
/// then converts the averaged vector back to lon/lat degrees.
pub fn spherical_centroid_degrees(points: &[LonLatDegrees]) -> Option<LonLatDegrees> {
    if points.is_empty() {
        return None;
    }

    let mut sx = 0.0;
    let mut sy = 0.0;
    let mut sz = 0.0;
    for point in points {
        let xyz = lonlat_degrees_to_unit_xyz(*point);
        sx += xyz.x;
        sy += xyz.y;
        sz += xyz.z;
    }
    let n = points.len() as f64;
    let centroid = CartesianPoint::new(sx / n, sy / n, sz / n);
    Some(xyz_to_lonlat_degrees(centroid))
}

/// Batch port of `MOD_grid_preprocess:centroid_spherical_calculation`.
///
/// Preserves the Fortran workflow where triangle ids start at `2`; slots `0`
/// and `1` remain initialized to `(0, 0)` just like an unwritten `mp` scratch
/// array in the migrated Rust call boundary.
pub fn centroid_spherical_mesh_fortran_indexed(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<LonLatDegrees>> {
    let mut centroids = vec![LonLatDegrees::new(0.0, 0.0); cells_on_triangle.len()];

    for triangle_id in 2..cells_on_triangle.len() {
        let cell_ids = cells_on_triangle[triangle_id];
        let triangle_points = [
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ];
        centroids[triangle_id] = spherical_centroid_degrees(&triangle_points)?;
    }

    Some(centroids)
}

/// Port of one iteration of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// `barycenter` and `vertices` are Earth-radius-scaled Cartesian coordinates.
/// The algorithm mirrors the Fortran global-domain branch: build a local polar
/// stereographic plane at the spherical barycenter, solve the 2-D circumcenter,
/// unproject it back to an Earth displacement, then renormalize to the Earth
/// radius.
pub fn spherical_circumcenter_from_barycenter(
    barycenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> Option<CartesianPoint> {
    let earth_radius = earthmesh_core::EARTH_RADIUS_METERS;
    let raxis = barycenter.x.hypot(barycenter.y);
    if raxis == 0.0 {
        return Some(barycenter);
    }

    let pole = PoleBasis {
        cos_lat: raxis / earth_radius,
        sin_lat: barycenter.z / earth_radius,
        cos_lon: barycenter.x / raxis,
        sin_lon: barycenter.y / raxis,
    };

    let mut projected = [PlanePoint::new(0.0, 0.0); 3];
    for (slot, vertex) in projected.iter_mut().zip(vertices) {
        let displacement = CartesianPoint::new(
            vertex.x - barycenter.x,
            vertex.y - barycenter.y,
            vertex.z - barycenter.z,
        );
        *slot = project_to_polar_stereographic(displacement, pole);
    }

    let [p1, p2, p3] = projected;
    let dx12 = p2.x - p1.x;
    let dx13 = p3.x - p1.x;
    let dx23 = p3.x - p2.x;
    let s1 = p1.x * p1.x + p1.y * p1.y;
    let s2 = p2.x * p2.x + p2.y * p2.y;
    let s3 = p3.x * p3.x + p3.y * p3.y;

    let y_denom = dx13 * p2.y - dx12 * p3.y - dx23 * p1.y;
    if y_denom == 0.0 {
        return Some(barycenter);
    }
    let ycc = 0.5 * (dx13 * s2 - dx12 * s3 - dx23 * s1) / y_denom;

    let xcc = if dx12.abs() > dx13.abs() {
        if dx12 == 0.0 {
            return Some(barycenter);
        }
        (s2 - s1 - ycc * 2.0 * (p2.y - p1.y)) / (2.0 * dx12)
    } else {
        if dx13 == 0.0 {
            return Some(barycenter);
        }
        (s3 - s1 - ycc * 2.0 * (p3.y - p1.y)) / (2.0 * dx13)
    };

    let displacement = unproject_from_polar_stereographic(PlanePoint::new(xcc, ycc), pole);
    let mut circumcenter = CartesianPoint::new(
        displacement.x + barycenter.x,
        displacement.y + barycenter.y,
        displacement.z + barycenter.z,
    );

    let radius = magnitude(circumcenter);
    if radius == 0.0 {
        return Some(barycenter);
    }
    let expansion = earth_radius / radius;
    circumcenter.x *= expansion;
    circumcenter.y *= expansion;
    circumcenter.z *= expansion;
    Some(circumcenter)
}

fn angular_distance_radians(a: CartesianPoint, b: CartesianPoint) -> Option<f64> {
    let mag = magnitude(a) * magnitude(b);
    if mag == 0.0 {
        return None;
    }
    Some((dot(a, b) / mag).clamp(-1.0, 1.0).acos())
}

fn circumcenter_is_local_enough(
    barycenter: CartesianPoint,
    circumcenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> bool {
    let Some(center_distance) = angular_distance_radians(barycenter, circumcenter) else {
        return false;
    };
    let max_vertex_distance = vertices
        .iter()
        .filter_map(|vertex| angular_distance_radians(barycenter, *vertex))
        .fold(0.0_f64, f64::max);

    if max_vertex_distance == 0.0 {
        return false;
    }

    (center_distance <= deg_to_rad(5.0) || center_distance <= 2.5 * max_vertex_distance)
        && circumcenter_fits_local_lonlat_envelope(barycenter, circumcenter, vertices)
}

fn unwrap_lon_around(lon_degrees: f64, reference_degrees: f64) -> f64 {
    if lon_degrees - reference_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees - reference_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

fn circumcenter_fits_local_lonlat_envelope(
    barycenter: CartesianPoint,
    circumcenter: CartesianPoint,
    vertices: [CartesianPoint; 3],
) -> bool {
    let barycenter_lonlat = xyz_to_lonlat_degrees(barycenter);
    let circumcenter_lonlat = xyz_to_lonlat_degrees(circumcenter);
    let circumcenter_lon = unwrap_lon_around(
        circumcenter_lonlat.lon_degrees,
        barycenter_lonlat.lon_degrees,
    );
    let mut min_lon = f64::MAX;
    let mut max_lon = f64::MIN;
    let mut min_lat = f64::MAX;
    let mut max_lat = f64::MIN;

    for vertex in vertices {
        let vertex_lonlat = xyz_to_lonlat_degrees(vertex);
        let vertex_lon =
            unwrap_lon_around(vertex_lonlat.lon_degrees, barycenter_lonlat.lon_degrees);
        min_lon = min_lon.min(vertex_lon);
        max_lon = max_lon.max(vertex_lon);
        min_lat = min_lat.min(vertex_lonlat.lat_degrees);
        max_lat = max_lat.max(vertex_lonlat.lat_degrees);
    }

    let lon_margin = ((max_lon - min_lon) * 1.5).max(1.0);
    let lat_margin = ((max_lat - min_lat) * 1.5).max(1.0);
    circumcenter_lon >= min_lon - lon_margin
        && circumcenter_lon <= max_lon + lon_margin
        && circumcenter_lonlat.lat_degrees >= min_lat - lat_margin
        && circumcenter_lonlat.lat_degrees <= max_lat + lat_margin
}

/// Batch port of `MOD_grid_preprocess:circumcenter_spherical_calculation`.
///
/// Returns a copy of the incoming M-point Cartesian centers with triangle ids
/// `2..len` replaced by spherical circumcenters, preserving the Fortran inout
/// behavior for slots not visited by the loop.
pub fn circumcenter_spherical_mesh_fortran_indexed(
    initial_centers: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    cells_on_triangle: &[[usize; 3]],
) -> Option<Vec<CartesianPoint>> {
    if cells_on_triangle.len() > initial_centers.len() {
        return None;
    }

    let mut centers = initial_centers.to_vec();
    for triangle_id in 2..cells_on_triangle.len() {
        let vertex_ids = cells_on_triangle[triangle_id];
        let vertices = [
            *vertex_points.get(vertex_ids[0])?,
            *vertex_points.get(vertex_ids[1])?,
            *vertex_points.get(vertex_ids[2])?,
        ];
        let barycenter = centers[triangle_id];
        let circumcenter =
            match spherical_circumcenter_from_barycenter(centers[triangle_id], vertices) {
                Some(center) => center,
                None => {
                    spring_global_debug(&format!("circumcenter failed for triangle {triangle_id}"));
                    return None;
                }
            };
        centers[triangle_id] = if circumcenter_is_local_enough(barycenter, circumcenter, vertices) {
            circumcenter
        } else {
            spring_global_debug(&format!(
                "circumcenter for triangle {triangle_id} is outside local triangle; using barycenter"
            ));
            barycenter
        };
    }

    Some(centers)
}

/// Result of `MOD_grid_preprocess:find_frac_index`.
///
/// `index` is intentionally 1-based to preserve the Fortran caller contract.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FortranFracIndex {
    pub index: usize,
    pub frac: f64,
}

/// Port of `MOD_grid_preprocess:find_frac_index` with explicit failure.
///
/// The Fortran subroutine supports monotonic ascending longitude grids and
/// monotonic descending latitude grids. The original error path is unreachable
/// after `return`; this Rust port returns `None` when the point is outside the
/// provided bounds or a zero-width cell is encountered.
pub fn find_frac_index_fortran(grid: &[f64], point: f64) -> Option<FortranFracIndex> {
    if grid.len() < 2 {
        return None;
    }

    let ascending = grid[0] < *grid.last()?;
    for i in 0..(grid.len() - 1) {
        let in_cell = if ascending {
            point >= grid[i] && point <= grid[i + 1]
        } else {
            point <= grid[i] && point >= grid[i + 1]
        };
        if !in_cell {
            continue;
        }

        let dx = grid[i + 1] - grid[i];
        if dx == 0.0 {
            return None;
        }
        let frac = ((point - grid[i]) / dx).clamp(0.0, 1.0);
        return Some(FortranFracIndex { index: i + 1, frac });
    }

    None
}

/// Rust representation of `refine_vars:set_dis_type` choices used by
/// `MOD_grid_preprocess:dist_layers_make`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceLayerSpacing {
    Linear,
    Power,
    Exponential,
    Logarithmic,
}

/// Port of `MOD_grid_preprocess:dist_layers_make`.
pub fn distance_layers(
    dist_len: usize,
    dist_select: f64,
    spacing: DistanceLayerSpacing,
) -> Option<Vec<f64>> {
    if dist_len == 0 {
        return None;
    }

    let mindist_select = dist_select / 2.0;
    let dist_len_f = dist_len as f64;
    let mut layers = Vec::with_capacity(dist_len);

    match spacing {
        DistanceLayerSpacing::Linear => {
            let a = mindist_select / dist_len_f;
            let b = mindist_select - a;
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0) + b);
            }
        }
        DistanceLayerSpacing::Power => {
            let a = mindist_select;
            let b = 2.0_f64.ln() / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).powf(b));
            }
        }
        DistanceLayerSpacing::Exponential => {
            let b = 2.0_f64.powf(1.0 / dist_len_f);
            let a = mindist_select / b;
            for i in 1..=dist_len {
                layers.push(a * b.powf(i as f64 + 1.0));
            }
        }
        DistanceLayerSpacing::Logarithmic => {
            let b = mindist_select;
            let a = b / (dist_len_f + 1.0).ln();
            for i in 1..=dist_len {
                layers.push(a * (i as f64 + 1.0).ln() + b);
            }
        }
    }

    Some(layers)
}

fn boundary_cells_from_triangle_flags(
    num_center_in: usize,
    triangles_on_cell: &[Vec<usize>],
    triangle_flags: &[bool],
) -> Option<Vec<bool>> {
    let mut boundary = vec![false; triangles_on_cell.len()];
    for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
        let triangles = &triangles_on_cell[cell_id];
        if triangles.is_empty() {
            continue;
        }
        let mut flagged = 0usize;
        let mut active_triangles = 0usize;
        for &triangle_id in triangles {
            if triangle_id <= 1 {
                continue;
            }
            active_triangles += 1;
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != active_triangles;
    }
    Some(boundary)
}

/// Port of the edge-length update rule in
/// `MOD_grid_preprocess:distsOnEdge_layers_make`.
///
/// The arrays preserve migrated Fortran indexing: slots `0` and `1` are
/// placeholders, triangle ids and edge ids are used directly, and the caller
/// provides `num_vertex_in`/`num_center_in` from `num_mp_step(iter)` and
/// `num_wp_step(iter)`.
pub fn dists_on_edge_layers_fortran_indexed(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    triangles_on_cell: &[Vec<usize>],
    edges_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_dists_on_edge: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < 2 * dist_len
        || refinement_flags.len() > edges_on_vertex.len()
        || initial_dists_on_edge.len() > cells_on_edge.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut dists_on_edge = initial_dists_on_edge.to_vec();
    let mut edge_moved = vec![false; initial_dists_on_edge.len()];
    let mindist00 = dist_layers[2 * dist_len - 1] / 2.0;

    for _ in 0..num_rc {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    triangle_flags[triangle_id] = false;
                }
            }
        }
    }
    spring_global_debug(&format!(
        "dists layers after_rc active_after_vertex={}",
        triangle_flags
            .iter()
            .enumerate()
            .filter(|(idx, flag)| **flag && *idx > num_vertex_in)
            .count()
    ));

    let mut direct_candidate_edges = 0usize;
    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &edge_id in edges_on_vertex.get(triangle_id)? {
            if edge_id == 0 {
                continue;
            }
            direct_candidate_edges += 1;
            *dists_on_edge.get_mut(edge_id)? = mindist00;
            *edge_moved.get_mut(edge_id)? = true;
        }
    }
    spring_global_debug(&format!(
        "dists layers direct_candidate_edges={direct_candidate_edges}"
    ));

    for layer_id in 0..=dist_len {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        if layer_id == dist_len {
            break;
        }

        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    continue;
                }
                *triangle_flags.get_mut(triangle_id)? = true;
                *triangle_in.get_mut(triangle_id)? = true;
            }
        }

        for triangle_id in 2..triangle_in.len() {
            if !triangle_in[triangle_id] {
                continue;
            }
            for &edge_id in edges_on_vertex.get(triangle_id)? {
                if edge_id == 0 || *edge_moved.get(edge_id)? {
                    continue;
                }
                let cells = *cells_on_edge.get(edge_id)?;
                let boundary_sum =
                    usize::from(*boundary.get(cells[0])?) + usize::from(*boundary.get(cells[1])?);
                let layer_index = if boundary_sum == 1 {
                    2 * layer_id
                } else {
                    2 * layer_id + 1
                };
                *dists_on_edge.get_mut(edge_id)? = *dist_layers.get(layer_index)?;
                *edge_moved.get_mut(edge_id)? = true;
            }
        }
        triangle_in.fill(false);
    }

    Some(dists_on_edge)
}

/// Port of the cell-width update rule in
/// `MOD_grid_preprocess:cellwidth_layers_make`.
///
/// `cells_on_triangle` corresponds to Fortran `ngrmw(:, i)` for triangle `i`,
/// while `triangles_on_cell` corresponds to `ngrwm(:, k)` for cell `k`.
pub fn cellwidth_layers_fortran_indexed(
    num_vertex_in: usize,
    num_center_in: usize,
    num_rc: usize,
    dist_len: usize,
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    dist_layers: &[f64],
    refinement_flags: &[bool],
    initial_cellwidth: &[f64],
) -> Option<Vec<f64>> {
    if dist_len == 0
        || dist_layers.len() < dist_len
        || refinement_flags.len() > cells_on_triangle.len()
        || initial_cellwidth.len() < triangles_on_cell.len()
    {
        return None;
    }

    let mut triangle_flags = refinement_flags.to_vec();
    let mut triangle_in = vec![false; triangle_flags.len()];
    let mut cellwidth = initial_cellwidth.to_vec();

    for _ in 0..num_rc {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    triangle_flags[triangle_id] = false;
                }
            }
        }
    }

    let inner_cellwidth = dist_layers[dist_len - 1] / 2.0;
    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &cell_id in cells_on_triangle.get(triangle_id)? {
            if cell_id == 0 {
                continue;
            }
            *cellwidth.get_mut(cell_id)? = inner_cellwidth;
        }
    }

    for layer_id in 0..=dist_len {
        let boundary =
            boundary_cells_from_triangle_flags(num_center_in, triangles_on_cell, &triangle_flags)?;
        if layer_id == dist_len {
            break;
        }

        for cell_id in (num_center_in + 1)..triangles_on_cell.len() {
            if !boundary[cell_id] {
                continue;
            }
            for &triangle_id in &triangles_on_cell[cell_id] {
                if *triangle_flags.get(triangle_id)? {
                    continue;
                }
                *triangle_flags.get_mut(triangle_id)? = true;
                *triangle_in.get_mut(triangle_id)? = true;
            }
        }

        for triangle_id in 2..triangle_in.len() {
            if !triangle_in[triangle_id] {
                continue;
            }
            for &cell_id in cells_on_triangle.get(triangle_id)? {
                if cell_id == 0 || *boundary.get(cell_id)? {
                    continue;
                }
                *cellwidth.get_mut(cell_id)? = *dist_layers.get(layer_id)?;
            }
        }
        triangle_in.fill(false);
    }

    Some(cellwidth)
}

/// One active or skipped refinement iteration for the Rust port of
/// `MOD_grid_preprocess:set_distsOnEdge_global`.
#[derive(Debug, Clone, Copy)]
pub struct GlobalDistanceStep<'a> {
    pub active: bool,
    pub halo: usize,
    pub refinement_flags: &'a [bool],
    pub num_vertex_in: usize,
    pub num_center_in: usize,
}

/// Borrowed inputs for the pure calculation side of
/// `MOD_grid_preprocess:set_distsOnEdge_global`.
#[derive(Debug, Clone, Copy)]
pub struct SetDistsOnEdgeGlobalInput<'a> {
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub num_rc: usize,
    pub spacing: DistanceLayerSpacing,
    pub triangles_on_cell: &'a [Vec<usize>],
    pub cells_on_triangle: Option<&'a [[usize; 3]]>,
    pub edges_on_vertex: &'a [[usize; 3]],
    pub cells_on_edge: &'a [[usize; 2]],
    pub steps: &'a [GlobalDistanceStep<'a>],
}

/// Output from `set_distsOnEdge_global` calculation orchestration.
#[derive(Debug, Clone, PartialEq)]
pub struct SetDistsOnEdgeGlobalOutput {
    pub dists_on_edge: Vec<f64>,
    pub cellwidth: Option<Vec<f64>>,
}

/// Rust orchestration wrapper for `MOD_grid_preprocess:set_distsOnEdge_global`.
///
/// The Fortran routine derives refined-region flags through
/// `refine_sjx_regional_make` and reads global `halo`, `step`, and
/// `exit_loop_step` state. This pure Rust wrapper keeps the same distance
/// update sequence but accepts each iteration's refinement flags explicitly:
/// initialize background values, halve the selected edge/cellwidth scale after
/// each active iteration, build transition layers, then call the migrated
/// `distsOnEdge_layers_make` and optional `cellwidth_layers_make` kernels.
pub fn set_dists_on_edge_global_fortran_indexed(
    input: SetDistsOnEdgeGlobalInput<'_>,
) -> Option<SetDistsOnEdgeGlobalOutput> {
    let mut dists_on_edge = vec![input.base_dists_on_edge; input.cells_on_edge.len()];
    let mut cellwidth = input
        .base_cellwidth
        .map(|base| vec![base; input.triangles_on_cell.len()]);

    if cellwidth.is_some() && input.cells_on_triangle.is_none() {
        return None;
    }

    let mut edge_scale = input.base_dists_on_edge;
    let mut cellwidth_scale = input.base_cellwidth;

    for step in input.steps {
        if !step.active {
            continue;
        }
        spring_global_debug(&format!(
            "distance step halo={} num_vertex_in={} num_center_in={} flags={} active_after_vertex={}",
            step.halo,
            step.num_vertex_in,
            step.num_center_in,
            step.refinement_flags.len(),
            step.refinement_flags
                .iter()
                .enumerate()
                .filter(|(idx, flag)| **flag && *idx > step.num_vertex_in)
                .count()
        ));
        let dist_len = step.halo + input.num_rc;
        if dist_len == 0 {
            return None;
        }

        let current_edge_scale = edge_scale;
        edge_scale = current_edge_scale / 2.0;
        let edge_layers = distance_layers(2 * dist_len, current_edge_scale, input.spacing)?;
        let before_changed = dists_on_edge
            .iter()
            .filter(|value| (**value - input.base_dists_on_edge).abs() > 1.0e-12)
            .count();
        dists_on_edge = dists_on_edge_layers_fortran_indexed(
            step.num_vertex_in,
            step.num_center_in,
            input.num_rc,
            dist_len,
            input.triangles_on_cell,
            input.edges_on_vertex,
            input.cells_on_edge,
            &edge_layers,
            step.refinement_flags,
            &dists_on_edge,
        )?;
        let after_changed = dists_on_edge
            .iter()
            .filter(|value| (**value - input.base_dists_on_edge).abs() > 1.0e-12)
            .count();
        spring_global_debug(&format!(
            "distance step changed_edges before={before_changed} after={after_changed}"
        ));

        if let (Some(current_cellwidth), Some(cells_on_triangle), Some(widths)) =
            (cellwidth_scale, input.cells_on_triangle, cellwidth.as_ref())
        {
            let next_cellwidth_scale = current_cellwidth / 2.0;
            let cellwidth_layers = distance_layers(dist_len, current_cellwidth, input.spacing)?;
            let updated = cellwidth_layers_fortran_indexed(
                step.num_vertex_in,
                step.num_center_in,
                input.num_rc,
                dist_len,
                cells_on_triangle,
                input.triangles_on_cell,
                &cellwidth_layers,
                step.refinement_flags,
                widths,
            )?;
            cellwidth = Some(updated);
            cellwidth_scale = Some(next_cellwidth_scale);
        }
    }

    Some(SetDistsOnEdgeGlobalOutput {
        dists_on_edge,
        cellwidth,
    })
}

/// Port of `MOD_grid_preprocess:CheckLon`.
///
/// The Fortran routine performs a single +/-360 adjustment rather than a full
/// modulo normalization. Preserve that behavior for parity.
pub fn normalize_lon_m180_180(lon_degrees: f64) -> f64 {
    if lon_degrees > 180.0 {
        lon_degrees - 360.0
    } else if lon_degrees < -180.0 {
        lon_degrees + 360.0
    } else {
        lon_degrees
    }
}

/// Port of the swap predicate in `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Fortran compares the 2-D cross product between the cell-center edge vector
/// and the current vertex-edge vector. If `res > 0`, ordering is kept; otherwise
/// `verticesOnEdge(1:2, i)` is swapped.
pub fn should_swap_vertices_on_edge(
    cell1: LonLatDegrees,
    cell2: LonLatDegrees,
    vertex1: LonLatDegrees,
    vertex2: LonLatDegrees,
) -> bool {
    let cell_delta_lon = normalize_lon_m180_180(cell2.lon_degrees - cell1.lon_degrees);
    let cell_delta_lat = cell2.lat_degrees - cell1.lat_degrees;
    let vertex_delta_lon = normalize_lon_m180_180(vertex2.lon_degrees - vertex1.lon_degrees);
    let vertex_delta_lat = vertex2.lat_degrees - vertex1.lat_degrees;

    let cross = cell_delta_lon * vertex_delta_lat - cell_delta_lat * vertex_delta_lon;
    cross <= 0.0
}

/// Port of `MOD_grid_preprocess:GetSort_verticesOnEdge`.
///
/// Returns a sorted copy of `verticesOnEdge`, preserving the Fortran convention
/// that edge ids start at `2`. Each edge is swapped when the migrated
/// cross-product predicate indicates Fortran would exchange
/// `verticesOnEdge(1:2, i)`.
pub fn order_vertices_on_edge_fortran_indexed(
    point_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
) -> Option<Vec<[usize; 2]>> {
    if cells_on_edge.len() != vertices_on_edge.len() {
        return None;
    }

    let mut ordered = vertices_on_edge.to_vec();
    for edge_id in 2..vertices_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let vertices = ordered[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let cell2 = *cell_lonlat.get(cells[1])?;
        let vertex1 = *point_lonlat.get(vertices[0])?;
        let vertex2 = *point_lonlat.get(vertices[1])?;

        if should_swap_vertices_on_edge(cell1, cell2, vertex1, vertex2) {
            ordered[edge_id].swap(0, 1);
        }
    }

    Some(ordered)
}

/// Port of one-vertex rotation logic from `MOD_grid_preprocess:normalizeRotation`.
///
/// The minimum positive cell id is rotated into slot 0, and the edge slots are
/// rotated in lockstep. If no positive cell id exists, arrays are unchanged.
pub fn normalize_vertex_rotation(
    cells_on_vertex: [usize; 3],
    edges_on_vertex: [usize; 3],
) -> ([usize; 3], [usize; 3]) {
    let mut min_cell = cells_on_vertex[0];
    let mut min_pos = 0usize;

    for pos in 1..3 {
        let cell = cells_on_vertex[pos];
        if cell > 0 && (min_cell == 0 || cell < min_cell) {
            min_cell = cell;
            min_pos = pos;
        }
    }

    if min_pos == 1 && min_cell > 0 {
        (
            [cells_on_vertex[1], cells_on_vertex[2], cells_on_vertex[0]],
            [edges_on_vertex[1], edges_on_vertex[2], edges_on_vertex[0]],
        )
    } else if min_pos == 2 && min_cell > 0 {
        (
            [cells_on_vertex[2], cells_on_vertex[0], cells_on_vertex[1]],
            [edges_on_vertex[2], edges_on_vertex[0], edges_on_vertex[1]],
        )
    } else {
        (cells_on_vertex, edges_on_vertex)
    }
}

/// Port of `MOD_grid_preprocess:standardizeVerticesOnCellRotation`.
///
/// Cell ids preserve the migrated Fortran indexing convention: slot `1` is
/// skipped and valid cells are visited from id `2`. Only the first
/// `n_edges_on_cell[cell_id]` entries are rotated; any storage tail is kept in
/// place, matching Fortran's fixed-width `verticesOnCell(:, i)` arrays.
pub fn standardize_vertices_on_cell_rotation_fortran_indexed(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        return None;
    }

    let mut standardized = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if standardized[cell_id].len() < ne {
            return None;
        }

        let mut min_vertex_id = usize::MAX;
        let mut min_pos = 0usize;
        for pos in 0..ne {
            let vertex_id = standardized[cell_id][pos];
            if vertex_id > 0 && vertex_id < min_vertex_id {
                min_vertex_id = vertex_id;
                min_pos = pos;
            }
        }

        if min_vertex_id != usize::MAX && min_pos != 0 {
            let current = standardized[cell_id][0..ne].to_vec();
            let rotated = current[min_pos..]
                .iter()
                .chain(current[..min_pos].iter())
                .copied()
                .collect::<Vec<_>>();
            standardized[cell_id][0..ne].copy_from_slice(&rotated);
        }
    }

    Some(standardized)
}

/// Output of `MOD_grid_preprocess:Get_ConnectOnCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellConnectivityOnCell {
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
}

/// Port of `MOD_grid_preprocess:Get_ConnectOnCell`.
///
/// The input `vertices_on_cell` must already be ordered around each cell. For
/// each consecutive vertex pair, this finds the shared edge from the two
/// `edgesOnVertex` triplets, then maps that edge to the neighboring cell via
/// `cellsOnEdge`.
pub fn connect_on_cell_fortran_indexed(
    n_edges_on_cell: &[usize],
    cells_on_edge: &[[usize; 2]],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_cell: &[Vec<usize>],
) -> Option<CellConnectivityOnCell> {
    let debug = std::env::var_os("EARTHMESH_MPAS_DEBUG").is_some();
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        if debug {
            eprintln!(
                "EARTHMESH_MPAS_DEBUG: n_edges_on_cell len {} < vertices_on_cell len {}",
                n_edges_on_cell.len(),
                vertices_on_cell.len()
            );
        }
        return None;
    }

    let mut edges_on_cell = vec![Vec::new(); vertices_on_cell.len()];
    let mut cells_on_cell = vec![Vec::new(); vertices_on_cell.len()];

    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if vertices_on_cell[cell_id].len() < ne {
            return None;
        }

        let mut cell_edges = Vec::with_capacity(ne);
        let mut neighbor_cells = Vec::with_capacity(ne);
        for vertex_slot in 0..ne {
            let vertex1 = vertices_on_cell[cell_id][vertex_slot];
            let vertex2 = vertices_on_cell[cell_id][(vertex_slot + 1) % ne];
            let edges_vertex1 = *edges_on_vertex.get(vertex1)?;
            let edges_vertex2 = *edges_on_vertex.get(vertex2)?;
            let edge_id = match edges_vertex1
                .iter()
                .copied()
                .find(|edge| *edge > 0 && edges_vertex2.contains(edge))
            {
                Some(edge_id) => edge_id,
                None => {
                    if debug {
                        eprintln!(
                            "EARTHMESH_MPAS_DEBUG: no shared edge cell={cell_id} slot={vertex_slot} vertex1={vertex1} vertex2={vertex2} edges1={edges_vertex1:?} edges2={edges_vertex2:?} vertices={:?}",
                            vertices_on_cell[cell_id]
                        );
                    }
                    return None;
                }
            };
            let cells = *cells_on_edge.get(edge_id)?;
            let neighbor = if cells[0] == cell_id {
                cells[1]
            } else if cells[1] == cell_id {
                cells[0]
            } else {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: edge cell mismatch cell={cell_id} slot={vertex_slot} edge={edge_id} cells_on_edge={cells:?} vertices={:?}",
                        vertices_on_cell[cell_id]
                    );
                }
                return None;
            };
            cell_edges.push(edge_id);
            neighbor_cells.push(neighbor);
        }
        edges_on_cell[cell_id] = cell_edges;
        cells_on_cell[cell_id] = neighbor_cells;
    }

    Some(CellConnectivityOnCell {
        edges_on_cell,
        cells_on_cell,
    })
}

pub fn order_vertices_on_cell_by_shared_edges_fortran_indexed(
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    edges_on_vertex: &[[usize; 3]],
) -> Option<Vec<Vec<usize>>> {
    let debug = std::env::var_os("EARTHMESH_MPAS_DEBUG").is_some();
    if n_edges_on_cell.len() < vertices_on_cell.len() {
        if debug {
            eprintln!(
                "EARTHMESH_MPAS_DEBUG: n_edges_on_cell len {} < vertices_on_cell len {}",
                n_edges_on_cell.len(),
                vertices_on_cell.len()
            );
        }
        return None;
    }
    let mut ordered = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne <= 2 {
            continue;
        }
        if vertices_on_cell[cell_id].len() < ne {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} vertices len {} < n_edges {ne}",
                    vertices_on_cell[cell_id].len()
                );
            }
            return None;
        }
        let active = vertices_on_cell[cell_id][0..ne]
            .iter()
            .copied()
            .filter(|vertex| *vertex > 0)
            .collect::<Vec<_>>();
        if active.len() != ne {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} has inactive vertices active={active:?} ne={ne} row={:?}",
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }

        let start = *active.iter().min()?;
        let mut start_neighbors = active
            .iter()
            .copied()
            .filter(|candidate| {
                *candidate != start
                    && vertices_share_edge(start, *candidate, edges_on_vertex).unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if start_neighbors.len() != 2 {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} start {start} neighbor count {} active={active:?} row={:?}",
                    start_neighbors.len(),
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }
        start_neighbors.sort_unstable();

        let mut cycle = vec![start, start_neighbors[0]];
        while cycle.len() < ne {
            let prev = cycle[cycle.len() - 2];
            let current = cycle[cycle.len() - 1];
            let mut next_candidates = active
                .iter()
                .copied()
                .filter(|candidate| {
                    *candidate != prev
                        && !cycle.contains(candidate)
                        && vertices_share_edge(current, *candidate, edges_on_vertex)
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>();
            if next_candidates.len() != 1 {
                if debug {
                    eprintln!(
                        "EARTHMESH_MPAS_DEBUG: cell {cell_id} current {current} next candidate count {} active={active:?} cycle={cycle:?} row={:?}",
                        next_candidates.len(),
                        vertices_on_cell[cell_id]
                    );
                }
                return None;
            }
            cycle.push(next_candidates.remove(0));
        }
        if !vertices_share_edge(*cycle.last()?, start, edges_on_vertex)? {
            if debug {
                eprintln!(
                    "EARTHMESH_MPAS_DEBUG: cell {cell_id} cycle does not close start={start} cycle={cycle:?} row={:?}",
                    vertices_on_cell[cell_id]
                );
            }
            return None;
        }
        ordered[cell_id][0..ne].copy_from_slice(&cycle);
    }
    Some(ordered)
}

fn vertices_share_edge(
    vertex1: usize,
    vertex2: usize,
    edges_on_vertex: &[[usize; 3]],
) -> Option<bool> {
    let edges_vertex1 = edges_on_vertex.get(vertex1)?;
    let edges_vertex2 = edges_on_vertex.get(vertex2)?;
    Some(
        edges_vertex1
            .iter()
            .any(|edge| *edge > 0 && edges_vertex2.contains(edge)),
    )
}

/// Port of `MOD_grid_preprocess:orderVerticesOnCell`.
///
/// Preserves the Fortran selection-sort approach: for each fixed vertex slot,
/// choose the remaining vertex with positive `cross(vec1, vec2) · normal` and
/// the smallest angle to the current reference vector.
pub fn order_vertices_on_cell_fortran_indexed(
    cell_points: &[CartesianPoint],
    vertex_points: &[CartesianPoint],
    vertices_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<Vec<usize>>> {
    if n_edges_on_cell.len() < vertices_on_cell.len() || cell_points.len() < vertices_on_cell.len()
    {
        return None;
    }

    let mut ordered = vertices_on_cell.to_vec();
    for cell_id in 2..vertices_on_cell.len() {
        let ne = n_edges_on_cell[cell_id];
        if ne == 0 {
            continue;
        }
        if ordered[cell_id].len() < ne {
            return None;
        }

        let cell_center = *cell_points.get(cell_id)?;
        let normal_mag = magnitude(cell_center);
        if normal_mag == 0.0 {
            return None;
        }
        let normal = CartesianPoint::new(
            cell_center.x / normal_mag,
            cell_center.y / normal_mag,
            cell_center.z / normal_mag,
        );

        for slot in 0..(ne - 1) {
            let vertex1_id = ordered[cell_id][slot];
            if vertex1_id == 0 {
                continue;
            }
            let vertex1 = *vertex_points.get(vertex1_id)?;
            let vec1 = vector_between(cell_center, vertex1);
            let mag1 = magnitude(vec1);
            if mag1 == 0.0 {
                continue;
            }

            let mut min_angle = std::f64::consts::PI * 2.0;
            let mut swap_slot = None;
            for candidate_slot in (slot + 1)..ne {
                let vertex2_id = ordered[cell_id][candidate_slot];
                if vertex2_id == 0 {
                    continue;
                }
                let vertex2 = *vertex_points.get(vertex2_id)?;
                let vec2 = vector_between(cell_center, vertex2);
                let mag2 = magnitude(vec2);
                if mag2 == 0.0 {
                    continue;
                }

                let cross_product = cross(vec1, vec2);
                if dot(cross_product, normal) <= 0.0 {
                    continue;
                }
                let angle = (dot(vec1, vec2) / (mag1 * mag2)).clamp(-1.0, 1.0).acos();
                if angle < min_angle {
                    min_angle = angle;
                    swap_slot = Some(candidate_slot);
                }
            }

            if let Some(candidate_slot) = swap_slot {
                if candidate_slot != slot + 1 {
                    ordered[cell_id].swap(slot + 1, candidate_slot);
                }
            }
        }
    }

    Some(ordered)
}

/// Port of `MOD_grid_preprocess:planeAngle`.
pub fn plane_angle_signed(
    point_a: CartesianPoint,
    point_b: CartesianPoint,
    point_c: CartesianPoint,
    normal: CartesianPoint,
) -> Option<f64> {
    let ab = vector_between(point_a, point_b);
    let ac = vector_between(point_a, point_c);
    let mab = magnitude(ab);
    let mac = magnitude(ac);
    if mab == 0.0 || mac == 0.0 {
        return None;
    }

    let cos_angle = (dot(ab, ac) / (mab * mac)).clamp(-1.0, 1.0);
    let signed = if dot(cross(ab, ac), normal) >= 0.0 {
        cos_angle.acos()
    } else {
        -cos_angle.acos()
    };
    Some(signed)
}

/// Output of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDistanceAngleOutput {
    pub dc_edge: Vec<f64>,
    pub dv_edge: Vec<f64>,
    pub angle_edge: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Edge_DIS_Angle`.
pub fn edge_distance_angle_fortran_indexed(
    vertices: &[CartesianPoint],
    cells: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    lat_vertex_degrees: &[f64],
    lon_edge_degrees: &[f64],
    lat_edge_degrees: &[f64],
) -> Option<EdgeDistanceAngleOutput> {
    if cells_on_edge.len() != vertices_on_edge.len()
        || edge_points.len() < vertices_on_edge.len()
        || lon_edge_degrees.len() < vertices_on_edge.len()
        || lat_edge_degrees.len() < vertices_on_edge.len()
    {
        return None;
    }

    let mut dc_edge = vec![0.0; vertices_on_edge.len()];
    let mut dv_edge = vec![0.0; vertices_on_edge.len()];
    let mut angle_edge = vec![0.0; vertices_on_edge.len()];
    let pi = std::f64::consts::PI;

    for edge_id in 2..vertices_on_edge.len() {
        let vertex_ids = vertices_on_edge[edge_id];
        let cell_ids = cells_on_edge[edge_id];
        let vertex1 = *vertices.get(vertex_ids[0])?;
        let vertex2 = *vertices.get(vertex_ids[1])?;
        let cell1 = *cells.get(cell_ids[0])?;
        let cell2 = *cells.get(cell_ids[1])?;

        dv_edge[edge_id] = arc_length_unit_sphere(vertex1, vertex2);
        dc_edge[edge_id] = arc_length_unit_sphere(cell1, cell2);
        if dv_edge[edge_id] == 0.0 {
            return None;
        }

        let mut angle = (deg_to_rad(*lat_vertex_degrees.get(vertex_ids[1])?)
            - deg_to_rad(*lat_vertex_degrees.get(vertex_ids[0])?))
            / dv_edge[edge_id];
        angle = angle.clamp(-1.0, 1.0).acos();

        let edge_point = *edge_points.get(edge_id)?;
        let lon_north = deg_to_rad(lon_edge_degrees[edge_id]);
        let lat_north = deg_to_rad(lat_edge_degrees[edge_id] + 0.05);
        let north_point = CartesianPoint::new(
            lat_north.cos() * lon_north.cos(),
            lat_north.cos() * lon_north.sin(),
            lat_north.sin(),
        );
        let mut sign = plane_angle_signed(edge_point, north_point, vertex2, edge_point)?;
        if sign.abs() > 1.0e-14 {
            sign /= sign.abs();
        } else {
            sign = 1.0;
        }

        angle *= sign;
        if angle > pi {
            angle -= 2.0 * pi;
        }
        if angle < -pi {
            angle += 2.0 * pi;
        }
        angle_edge[edge_id] = angle;
    }

    Some(EdgeDistanceAngleOutput {
        dc_edge,
        dv_edge,
        angle_edge,
    })
}

/// Output of `MOD_grid_preprocess:edgeIDSort`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeIdSortOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Port of `MOD_grid_preprocess:edgeIDSort`.
///
/// Edges from the current mesh are reordered to match
/// `cells_on_edge_reference`; `edges_on_vertex` is then rebuilt from the sorted
/// `vertices_on_edge` arrays.
pub fn edge_id_sort_fortran_indexed(
    num_vertices: usize,
    cells_on_edge_reference: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_edge: &[[usize; 2]],
    edge_points: &[LonLatDegrees],
) -> Option<EdgeIdSortOutput> {
    let num_edges = cells_on_edge_reference.len();
    if cells_on_edge.len() != num_edges
        || vertices_on_edge.len() != num_edges
        || edge_points.len() != num_edges
    {
        return None;
    }

    let mut sorted_cells_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_vertices_on_edge = vec![[0usize; 2]; num_edges];
    let mut sorted_edge_points = vec![LonLatDegrees::new(0.0, 0.0); num_edges];

    for target_edge_id in 2..num_edges {
        let reference_cells = cells_on_edge_reference[target_edge_id];
        let source_edge_id = (2..num_edges).find(|&candidate| {
            cells_on_edge[candidate][0] == reference_cells[0]
                && cells_on_edge[candidate][1] == reference_cells[1]
        })?;
        sorted_cells_on_edge[target_edge_id] = cells_on_edge[source_edge_id];
        sorted_vertices_on_edge[target_edge_id] = vertices_on_edge[source_edge_id];
        sorted_edge_points[target_edge_id] = edge_points[source_edge_id];
    }

    let mut edges_on_vertex = vec![[0usize; 3]; num_vertices];
    let mut edge_counts = vec![0usize; num_vertices];
    for edge_id in 2..num_edges {
        for &vertex_id in &sorted_vertices_on_edge[edge_id] {
            if vertex_id == 0 {
                continue;
            }
            let count = edge_counts.get_mut(vertex_id)?;
            if *count >= 3 {
                return None;
            }
            edges_on_vertex.get_mut(vertex_id)?[*count] = edge_id;
            *count += 1;
        }
    }

    Some(EdgeIdSortOutput {
        cells_on_edge: sorted_cells_on_edge,
        vertices_on_edge: sorted_vertices_on_edge,
        edges_on_vertex,
        edge_points: sorted_edge_points,
    })
}

/// Output of `MOD_grid_preprocess:set_weightsOnEdge`.
#[derive(Debug, Clone, PartialEq)]
pub struct WeightsOnEdgeOutput {
    pub weights_on_edge: Vec<Vec<f64>>,
    pub edges_on_edge: Vec<Vec<usize>>,
    pub n_edges_on_edge: Vec<usize>,
    pub error_segment: Vec<f64>,
}

fn find_index_in_prefix(index: usize, indices: &[usize], n_indices: usize) -> Option<usize> {
    indices
        .iter()
        .take(n_indices)
        .position(|candidate| *candidate == index)
}

/// Port of `MOD_grid_preprocess:set_weightsOnEdge`.
///
/// The routine computes MPAS-compatible edge stencils and reconstruction
/// weights for Fortran-indexed mesh arrays. Weight rows are stored compactly per
/// edge rather than in a fixed `maxEdges2 x num_edge` matrix.
pub fn set_weights_on_edge_fortran_indexed(
    area_cell: &[f64],
    angle_edge: &[f64],
    dc_edge: &[f64],
    dv_edge: &[f64],
    kite_areas_on_vertex: &[[f64; 3]],
    edges_on_cell: &[Vec<usize>],
    cells_on_vertex: &[[usize; 3]],
    cells_on_edge: &[[usize; 2]],
    vertices_on_cell: &[Vec<usize>],
    vertices_on_edge: &[[usize; 2]],
    n_edges_on_cell: &[usize],
) -> Option<WeightsOnEdgeOutput> {
    let num_edges = cells_on_edge.len();
    if vertices_on_edge.len() != num_edges
        || angle_edge.len() < num_edges
        || dc_edge.len() < num_edges
        || dv_edge.len() < num_edges
    {
        return None;
    }

    let mut weights_on_edge = vec![Vec::new(); num_edges];
    let mut edges_on_edge = vec![Vec::new(); num_edges];
    let mut n_edges_on_edge = vec![0usize; num_edges];
    let mut error_segment = vec![0.0; num_edges];

    for edge_id in 2..num_edges {
        let [cell1, cell2] = cells_on_edge[edge_id];
        let edge_vertices = vertices_on_edge[edge_id];
        if cell1 == 0
            || cell2 == 0
            || edge_vertices[0] == 0
            || edge_vertices[1] == 0
            || cell1 >= n_edges_on_cell.len()
            || cell2 >= n_edges_on_cell.len()
        {
            continue;
        }
        let mut nw1 = 0usize;

        for side in 0..2 {
            let (cell_id, vertex_start, tev2) = if side == 0 {
                (cell1, vertices_on_edge[edge_id][1], -1.0)
            } else {
                (cell2, vertices_on_edge[edge_id][0], 1.0)
            };
            let ne = *n_edges_on_cell.get(cell_id)?;
            if ne == 0
                || vertices_on_cell.get(cell_id)?.len() < ne
                || edges_on_cell.get(cell_id)?.len() < ne
            {
                return None;
            }
            let area = *area_cell.get(cell_id)?;
            if area == 0.0 {
                return None;
            }

            let mut riv_cell = Vec::with_capacity(ne);
            for vertex_id in vertices_on_cell[cell_id].iter().copied().take(ne) {
                let cells_for_vertex = *cells_on_vertex.get(vertex_id)?;
                let kite_slot = cells_for_vertex
                    .iter()
                    .position(|candidate| *candidate == cell_id)?;
                riv_cell.push(kite_areas_on_vertex.get(vertex_id)?[kite_slot] / area);
            }

            let vertex_index = find_index_in_prefix(vertex_start, &vertices_on_cell[cell_id], ne)?;
            let mut riv_wrap = riv_cell.clone();
            riv_wrap.extend_from_slice(&riv_cell);

            for wrapped_index in vertex_index..=(vertex_index + ne - 2) {
                let mut kahan_sum = 0.0;
                let mut kahan_c = 0.0;
                for value in &riv_wrap[vertex_index..=wrapped_index] {
                    let kahan_y = *value - kahan_c;
                    let kahan_t = kahan_sum + kahan_y;
                    kahan_c = (kahan_t - kahan_sum) - kahan_y;
                    kahan_sum = kahan_t;
                }
                weights_on_edge[edge_id].push((kahan_sum - 0.5) * tev2);
            }

            let edge_index_cell = find_index_in_prefix(edge_id, &edges_on_cell[cell_id], ne)?;
            let mut edge_index = edges_on_cell[cell_id][0..ne].to_vec();
            edge_index.extend_from_within(0..ne);
            for local_edge_slot in 0..(ne - 1) {
                let output_slot = nw1 + local_edge_slot;
                let contributing_edge_id = edge_index[edge_index_cell + local_edge_slot + 1];
                edges_on_edge[edge_id].push(contributing_edge_id);
                let factor = *dv_edge.get(contributing_edge_id)? / *dc_edge.get(edge_id)?;
                let mut weight = *weights_on_edge[edge_id].get(output_slot)? * factor;
                if cells_on_edge.get(contributing_edge_id)?[1] == cell_id {
                    weight = -weight;
                }
                weights_on_edge[edge_id][output_slot] = weight;
            }

            nw1 = ne - 1;
            n_edges_on_edge[edge_id] += nw1;
        }
    }

    for edge_id in 2..num_edges {
        let mut v_edge = 0.0;
        for (contributing_edge_id, weight) in edges_on_edge[edge_id]
            .iter()
            .copied()
            .zip(weights_on_edge[edge_id].iter().copied())
        {
            v_edge += angle_edge.get(contributing_edge_id)?.cos() * weight;
        }
        let ve = -angle_edge[edge_id].sin();
        error_segment[edge_id] = (v_edge - ve).abs();
    }

    Some(WeightsOnEdgeOutput {
        weights_on_edge,
        edges_on_edge,
        n_edges_on_edge,
        error_segment,
    })
}

fn vector_between(from: CartesianPoint, to: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(to.x - from.x, to.y - from.y, to.z - from.z)
}

fn dot(a: CartesianPoint, b: CartesianPoint) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn cross(a: CartesianPoint, b: CartesianPoint) -> CartesianPoint {
    CartesianPoint::new(
        a.y * b.z - a.z * b.y,
        a.z * b.x - a.x * b.z,
        a.x * b.y - a.y * b.x,
    )
}

fn magnitude(a: CartesianPoint) -> f64 {
    (a.x * a.x + a.y * a.y + a.z * a.z).sqrt()
}

/// One-edge correction term from `MOD_grid_preprocess:spring_dynamics_global`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpringEdgeAdjustment {
    pub displacement: CartesianPoint,
    pub distance: f64,
    pub ratio: f64,
    pub target_distance: f64,
    pub frac_change: f64,
    pub frac_change_squared: f64,
}

/// Port of the per-edge spring correction formula in
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// `neighbor_distance_1..4` correspond to `dist(iu1..iu4)` from
/// `EdgesOnedge_tri(:, iu)`. The returned displacement is the Fortran-updated
/// `(dx, dy, dz)` after multiplying the edge vector by `frac_change`.
pub fn spring_edge_adjustment_fortran(
    cell1: CartesianPoint,
    cell2: CartesianPoint,
    target_edge_distance: f64,
    neighbor_distance_1: f64,
    neighbor_distance_2: f64,
    neighbor_distance_3: f64,
    neighbor_distance_4: f64,
) -> Option<SpringEdgeAdjustment> {
    // Fortran assigns the edge vector with `real(...)` and no kind argument
    // even though `dx/dy/dz` are real(r8), so each component is rounded through
    // default real before distance and displacement calculations.
    let edge_vector = CartesianPoint::new(
        (cell2.x - cell1.x) as f32 as f64,
        (cell2.y - cell1.y) as f32 as f64,
        (cell2.z - cell1.z) as f32 as f64,
    );
    let distance = magnitude(edge_vector);
    if distance == 0.0
        || neighbor_distance_1 == 0.0
        || neighbor_distance_2 == 0.0
        || neighbor_distance_3 == 0.0
        || neighbor_distance_4 == 0.0
    {
        return None;
    }

    let twocosphi3 = (neighbor_distance_1.powi(2) + neighbor_distance_2.powi(2) - distance.powi(2))
        / (neighbor_distance_1 * neighbor_distance_2);
    let twocosphi4 = (neighbor_distance_3.powi(2) + neighbor_distance_4.powi(2) - distance.powi(2))
        / (neighbor_distance_3 * neighbor_distance_4);
    let ratio = (twocosphi3 + twocosphi4).clamp(0.15, 1.2);
    let target_distance = target_edge_distance / 1.2 * ratio;
    let frac_change = (target_distance - distance) / distance;
    let displacement = CartesianPoint::new(
        edge_vector.x * frac_change,
        edge_vector.y * frac_change,
        edge_vector.z * frac_change,
    );

    Some(SpringEdgeAdjustment {
        displacement,
        distance,
        ratio,
        target_distance,
        frac_change,
        frac_change_squared: frac_change * frac_change,
    })
}

/// Port of the `dirs(j, iw)` sign setup in
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// For each cell edge, Fortran assigns `+relax` when the current cell is
/// `CellsOnEdge(2, edge)` and `-relax` otherwise. Rows preserve the compact
/// `edgesOnCell` row length supplied for each Fortran-indexed cell id.
pub fn spring_edge_directions_fortran_indexed(
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    relax: f64,
) -> Option<Vec<Vec<f64>>> {
    if n_edges_on_cell.len() != edges_on_cell.len() {
        return None;
    }

    let mut directions = vec![Vec::<f64>::new(); n_edges_on_cell.len()];
    for cell_id in 2..n_edges_on_cell.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_edges = edges_on_cell.get(cell_id)?;
        if edge_count > cell_edges.len() {
            return None;
        }
        let mut row = Vec::with_capacity(edge_count);
        for &edge_id in cell_edges.iter().take(edge_count) {
            let cells = *cells_on_edge.get(edge_id)?;
            if cells[1] == cell_id {
                row.push(relax);
            } else {
                row.push(-relax);
            }
        }
        directions[cell_id] = row;
    }

    Some(directions)
}

/// Port of the cell accumulation and spherical renormalization steps inside
/// `MOD_grid_preprocess:spring_dynamics_global`.
///
/// The caller supplies the per-edge displacements already produced by
/// `spring_edge_adjustment_fortran` and the compact per-cell direction rows
/// produced by `spring_edge_directions_fortran_indexed`. This helper performs
/// the Fortran update:
/// `xew8(iw) += dirs(j, iw) * dx(edge)` for each cell edge, followed by
/// normalization back to `radius`.
pub fn spring_apply_cell_displacements_fortran_indexed(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    directions: &[Vec<f64>],
    edge_displacements: &[CartesianPoint],
    radius: f64,
) -> Option<Vec<CartesianPoint>> {
    if n_edges_on_cell.len() != cell_points.len()
        || edges_on_cell.len() != cell_points.len()
        || directions.len() != cell_points.len()
    {
        return None;
    }

    let mut updated = cell_points.to_vec();
    for cell_id in 2..cell_points.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_edges = edges_on_cell.get(cell_id)?;
        let cell_directions = directions.get(cell_id)?;
        if edge_count > cell_edges.len() || edge_count > cell_directions.len() {
            return None;
        }

        let mut point = updated[cell_id];
        for slot in 0..edge_count {
            let edge_id = cell_edges[slot];
            let displacement = *edge_displacements.get(edge_id)?;
            let direction = cell_directions[slot];
            point.x += direction * displacement.x;
            point.y += direction * displacement.y;
            point.z += direction * displacement.z;
        }

        let norm = magnitude(point);
        if norm == 0.0 {
            return None;
        }
        let expansion = radius / norm;
        updated[cell_id] = CartesianPoint::new(
            point.x * expansion,
            point.y * expansion,
            point.z * expansion,
        );
    }

    Some(updated)
}

/// Output from one `spring_dynamics_global` iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringGlobalIterationOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub edge_displacements: Vec<CartesianPoint>,
    pub frac_change_squared: Vec<f64>,
}

/// Periodic displacement diagnostic printed by `spring_dynamics_global`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDiagnosticMaxDisplacement {
    pub iteration: usize,
    pub max_displacement: f64,
}

/// Output from the multi-iteration `spring_dynamics_global` wrapper.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDynamicsGlobalOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub last_edge_displacements: Vec<CartesianPoint>,
    pub last_frac_change_squared: Vec<f64>,
    pub diagnostic_max_displacements: Vec<SpringDiagnosticMaxDisplacement>,
}

/// Output from the regional move-mask smoother in `spring_dynamics_regionalv2`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringDynamicsRegionalOutput {
    pub updated_cell_points: Vec<CartesianPoint>,
    pub calculated_cells: Vec<usize>,
    pub moved_cells: Vec<usize>,
    pub diagnostic_max_displacements: Vec<SpringDiagnosticMaxDisplacement>,
}

/// Borrowed inputs for the pure mask-derivation core of
/// `MOD_grid_preprocess:set_dbxMove_regional_step`.
#[derive(Debug, Clone, Copy)]
pub struct RegionalMoveMaskInput<'a> {
    pub set_dis: usize,
    pub refined_triangles: &'a [bool],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
}

/// Output from the migrated `set_dbxMove_regional_step` mask derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionalMoveMaskOutput {
    pub move_mask: Vec<bool>,
    pub boundary_mask: Vec<bool>,
    pub expanded_refined_triangles: Vec<bool>,
    pub protected_triangles: Vec<bool>,
}

/// Borrowed inputs for the pure classification core of
/// `MOD_grid_preprocess:refine_sjx_regional_make`.
#[derive(Debug, Clone, Copy)]
pub struct RefineRegionalMaskInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub source_lon_vertices: &'a [f64],
    pub source_lat_vertices: &'a [f64],
    pub mask_patch: &'a [Vec<bool>],
    pub first_triangle_id: usize,
}

/// Borrowed inputs for the pure in-memory calculation side of
/// `MOD_grid_preprocess:Springjustment_global`.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentGlobalCoreInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub base_dists_on_edge: f64,
    pub base_cellwidth: Option<f64>,
    pub distance_num_rc: usize,
    pub distance_spacing: DistanceLayerSpacing,
    pub distance_steps: &'a [GlobalDistanceStep<'a>],
    pub niter_refine: usize,
    pub relax: f64,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the pure in-memory `Springjustment_global` adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentGlobalCoreOutput {
    pub updated_triangle_lonlat: Vec<LonLatDegrees>,
    pub updated_cell_lonlat: Vec<LonLatDegrees>,
    pub triangle_neighbors: Vec<[usize; 3]>,
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
    pub edges_on_edge_tri: Vec<[usize; 4]>,
    pub dists_on_edge: Vec<f64>,
    pub cellwidth: Option<Vec<f64>>,
    pub edge_lonlat: Vec<LonLatDegrees>,
    pub spring: SpringDynamicsGlobalOutput,
}

/// Borrowed inputs for the pure in-memory calculation side of
/// `MOD_grid_preprocess:Springjustment_regional_step`.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalCoreInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub move_mask: &'a [bool],
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the pure in-memory `Springjustment_regional_step` adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalCoreOutput {
    pub updated_triangle_lonlat: Vec<LonLatDegrees>,
    pub updated_cell_lonlat: Vec<LonLatDegrees>,
    pub triangle_neighbors: Vec<[usize; 3]>,
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub edges_on_cell: Vec<Vec<usize>>,
    pub cells_on_cell: Vec<Vec<usize>>,
    pub regional: SpringDynamicsRegionalOutput,
}

/// Borrowed inputs for the pure in-memory calculation side of
/// `Springjustment_regional_step` when the upstream refinement source has
/// already been resolved to triangle flags.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalFromRefinementInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub refined_triangles: &'a [bool],
    pub set_dis: usize,
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from `Springjustment_regional_step` after mask derivation and the
/// migrated regional spring core have both run.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalFromRefinementOutput {
    pub mask: RegionalMoveMaskOutput,
    pub core: SpringjustmentRegionalCoreOutput,
}

/// Borrowed inputs for the pure in-memory regional Springjustment path when
/// the upstream refinement source is an already-loaded source mask grid.
#[derive(Debug, Clone, Copy)]
pub struct SpringjustmentRegionalFromSourceMaskInput<'a> {
    pub triangle_lonlat: &'a [LonLatDegrees],
    pub cell_lonlat: &'a [LonLatDegrees],
    pub cells_on_triangle: &'a [[usize; 3]],
    pub triangles_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
    pub source_lon_vertices: &'a [f64],
    pub source_lat_vertices: &'a [f64],
    pub mask_patch: &'a [Vec<bool>],
    pub first_triangle_id: usize,
    pub set_dis: usize,
    pub protected_seed_cells: &'a [usize],
    pub vertex_protect_layers: usize,
    pub niter_refine: usize,
    pub radius: f64,
    pub diagnostic_every: usize,
}

/// Output from the source-mask regional Springjustment adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct SpringjustmentRegionalFromSourceMaskOutput {
    pub refined_triangles: Vec<bool>,
    pub regional: SpringjustmentRegionalFromRefinementOutput,
}

/// One-iteration Rust wrapper for `MOD_grid_preprocess:spring_dynamics_global`.
///
/// This ports the calculation order inside one Fortran iteration: compute all
/// current edge distances, update per-edge correction vectors from
/// `EdgesOnedge_tri`, build/apply per-cell direction signs, then renormalize
/// cell coordinates back to `radius`.
pub fn spring_global_iteration_fortran_indexed(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    edges_on_edge_tri: &[[usize; 4]],
    dists_on_edge: &[f64],
    relax: f64,
    radius: f64,
) -> Option<SpringGlobalIterationOutput> {
    if cells_on_edge.len() != edges_on_edge_tri.len()
        || cells_on_edge.len() != dists_on_edge.len()
        || n_edges_on_cell.len() != cell_points.len()
        || edges_on_cell.len() != cell_points.len()
    {
        return None;
    }

    let mut edge_distances = vec![0.0; cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let cell1 = *cell_points.get(cells[0])?;
        let cell2 = *cell_points.get(cells[1])?;
        edge_distances[edge_id] = magnitude(vector_between(cell1, cell2));
    }

    let mut edge_displacements = vec![CartesianPoint::new(0.0, 0.0, 0.0); cells_on_edge.len()];
    let mut frac_change_squared = vec![0.0; cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let neighbor_edges = edges_on_edge_tri[edge_id];
        let adjustment = spring_edge_adjustment_fortran(
            *cell_points.get(cells[0])?,
            *cell_points.get(cells[1])?,
            dists_on_edge[edge_id],
            *edge_distances.get(neighbor_edges[0])?,
            *edge_distances.get(neighbor_edges[1])?,
            *edge_distances.get(neighbor_edges[2])?,
            *edge_distances.get(neighbor_edges[3])?,
        )?;
        edge_displacements[edge_id] = adjustment.displacement;
        frac_change_squared[edge_id] = adjustment.frac_change_squared;
    }

    let directions = spring_edge_directions_fortran_indexed(
        n_edges_on_cell,
        edges_on_cell,
        cells_on_edge,
        relax,
    )?;
    let updated_cell_points = spring_apply_cell_displacements_fortran_indexed(
        cell_points,
        n_edges_on_cell,
        edges_on_cell,
        &directions,
        &edge_displacements,
        radius,
    )?;

    Some(SpringGlobalIterationOutput {
        updated_cell_points,
        edge_displacements,
        frac_change_squared,
    })
}

/// Multi-iteration Rust wrapper for `MOD_grid_preprocess:spring_dynamics_global`.
///
/// This keeps only the current coordinate arrays, matching the Fortran memory
/// model, and records the periodic `Max DS` diagnostics for `iter == 1` or
/// `iter % diagnostic_every == 0`.
pub fn spring_dynamics_global_fortran_indexed(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    edges_on_cell: &[Vec<usize>],
    cells_on_edge: &[[usize; 2]],
    edges_on_edge_tri: &[[usize; 4]],
    dists_on_edge: &[f64],
    niter_refine: usize,
    relax: f64,
    radius: f64,
    diagnostic_every: usize,
) -> Option<SpringDynamicsGlobalOutput> {
    if diagnostic_every == 0 {
        return None;
    }

    let mut current_cell_points = cell_points.to_vec();
    let mut diagnostic_reference = cell_points.to_vec();
    let mut last_edge_displacements = vec![CartesianPoint::new(0.0, 0.0, 0.0); cells_on_edge.len()];
    let mut last_frac_change_squared = vec![0.0; cells_on_edge.len()];
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter_refine {
        let record_diagnostic = iteration == 1 || iteration % diagnostic_every == 0;
        if record_diagnostic {
            diagnostic_reference = current_cell_points.clone();
        }

        let iteration_output = spring_global_iteration_fortran_indexed(
            &current_cell_points,
            n_edges_on_cell,
            edges_on_cell,
            cells_on_edge,
            edges_on_edge_tri,
            dists_on_edge,
            relax,
            radius,
        )?;

        current_cell_points = iteration_output.updated_cell_points;
        last_edge_displacements = iteration_output.edge_displacements;
        last_frac_change_squared = iteration_output.frac_change_squared;

        if record_diagnostic {
            let mut max_displacement = 0.0_f64;
            for cell_id in 2..current_cell_points.len() {
                let before = *diagnostic_reference.get(cell_id)?;
                let after = current_cell_points[cell_id];
                let displacement = magnitude(vector_between(before, after));
                max_displacement = max_displacement.max(displacement);
            }
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
    }

    Some(SpringDynamicsGlobalOutput {
        updated_cell_points: current_cell_points,
        last_edge_displacements,
        last_frac_change_squared,
        diagnostic_max_displacements,
    })
}

/// Rust port of `MOD_grid_preprocess:spring_dynamics_regionalv2`.
///
/// The Fortran routine builds a compact calculation set from every movable
/// cell plus its neighbor cells, but only cells flagged by `IsdbxMove` are
/// updated. Each moved cell is replaced by the average of its neighboring cell
/// coordinates from the previous iteration and then projected back to `radius`.
pub fn spring_dynamics_regional_fortran_indexed(
    cell_points: &[CartesianPoint],
    n_edges_on_cell: &[usize],
    cells_on_cell: &[Vec<usize>],
    move_mask: &[bool],
    niter_refine: usize,
    radius: f64,
    diagnostic_every: usize,
) -> Option<SpringDynamicsRegionalOutput> {
    if diagnostic_every == 0
        || n_edges_on_cell.len() != cell_points.len()
        || cells_on_cell.len() != cell_points.len()
        || move_mask.len() != cell_points.len()
    {
        return None;
    }

    let mut calculated_mask = move_mask.to_vec();
    for cell_id in 2..cell_points.len() {
        if !move_mask[cell_id] {
            continue;
        }
        let edge_count = n_edges_on_cell[cell_id];
        let neighbors = cells_on_cell.get(cell_id)?;
        if edge_count == 0 || edge_count > neighbors.len() {
            return None;
        }
        for &neighbor_id in neighbors.iter().take(edge_count) {
            *calculated_mask.get_mut(neighbor_id)? = true;
        }
    }

    let calculated_cells = (2..cell_points.len())
        .filter(|&cell_id| calculated_mask[cell_id])
        .collect::<Vec<_>>();
    let moved_cells = (2..cell_points.len())
        .filter(|&cell_id| move_mask[cell_id])
        .collect::<Vec<_>>();

    let mut current_cell_points = cell_points.to_vec();
    let mut diagnostic_max_displacements = Vec::new();

    for iteration in 1..=niter_refine {
        let previous_cell_points = current_cell_points.clone();
        for &cell_id in &moved_cells {
            let edge_count = n_edges_on_cell[cell_id];
            let neighbors = cells_on_cell.get(cell_id)?;
            if edge_count == 0 || edge_count > neighbors.len() {
                return None;
            }

            let mut averaged = CartesianPoint::new(0.0, 0.0, 0.0);
            for &neighbor_id in neighbors.iter().take(edge_count) {
                let neighbor = *previous_cell_points.get(neighbor_id)?;
                averaged.x += neighbor.x / edge_count as f64;
                averaged.y += neighbor.y / edge_count as f64;
                averaged.z += neighbor.z / edge_count as f64;
            }

            let norm = magnitude(averaged);
            if norm == 0.0 {
                return None;
            }
            let expansion = radius / norm;
            current_cell_points[cell_id] = CartesianPoint::new(
                averaged.x * expansion,
                averaged.y * expansion,
                averaged.z * expansion,
            );
        }

        if iteration == 1 || iteration % diagnostic_every == 0 {
            let mut max_displacement = 0.0_f64;
            for &cell_id in &moved_cells {
                let before = previous_cell_points[cell_id];
                let after = current_cell_points[cell_id];
                let displacement = magnitude(vector_between(before, after));
                max_displacement = max_displacement.max(displacement);
            }
            diagnostic_max_displacements.push(SpringDiagnosticMaxDisplacement {
                iteration,
                max_displacement,
            });
        }
    }

    Some(SpringDynamicsRegionalOutput {
        updated_cell_points: current_cell_points,
        calculated_cells,
        moved_cells,
        diagnostic_max_displacements,
    })
}

fn regional_boundary_mask_fortran_indexed(
    triangle_flags: &[bool],
    triangles_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
) -> Option<Vec<bool>> {
    if triangles_on_cell.len() != n_edges_on_cell.len() {
        return None;
    }
    let mut boundary = vec![false; triangles_on_cell.len()];
    for cell_id in 2..triangles_on_cell.len() {
        let edge_count = n_edges_on_cell[cell_id];
        let cell_triangles = triangles_on_cell.get(cell_id)?;
        if edge_count == 0 {
            continue;
        }
        if edge_count > cell_triangles.len() {
            return None;
        }
        let mut flagged = 0usize;
        for &triangle_id in cell_triangles.iter().take(edge_count) {
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != edge_count;
    }
    Some(boundary)
}

fn expand_triangles_from_boundary_fortran_indexed(
    mut triangle_flags: Vec<bool>,
    triangles_on_cell: &[Vec<usize>],
    n_edges_on_cell: &[usize],
    expansion_layers: usize,
) -> Option<(Vec<bool>, Vec<bool>)> {
    let mut boundary = regional_boundary_mask_fortran_indexed(
        &triangle_flags,
        triangles_on_cell,
        n_edges_on_cell,
    )?;
    for _ in 0..expansion_layers {
        for cell_id in 2..boundary.len() {
            if !boundary[cell_id] {
                continue;
            }
            let edge_count = n_edges_on_cell[cell_id];
            let cell_triangles = triangles_on_cell.get(cell_id)?;
            if edge_count > cell_triangles.len() {
                return None;
            }
            for &triangle_id in cell_triangles.iter().take(edge_count) {
                *triangle_flags.get_mut(triangle_id)? = true;
            }
        }
        boundary = regional_boundary_mask_fortran_indexed(
            &triangle_flags,
            triangles_on_cell,
            n_edges_on_cell,
        )?;
    }
    Some((triangle_flags, boundary))
}

/// Axis selector for the `MOD_Area_judge:Source_Find` source-grid lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaJudgeAxis {
    Longitude,
    Latitude,
}

/// One-based source-grid bounds returned by
/// `MOD_Area_judge:minmax_range_make`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeSourceBounds {
    pub minlon_source: usize,
    pub maxlon_source: usize,
    pub maxlat_source: usize,
    pub minlat_source: usize,
}

/// Source cells selected by the closed-curve ray-crossing fill in
/// `MOD_Area_judge:IsInArea_close_Calculation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaJudgeClosedCurveFill {
    pub cells: Vec<(usize, usize)>,
    pub patch_count: usize,
}

/// Summary from the pure `mask_patch_modify` sea/land update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaJudgeMaskPatchReport {
    pub patched_cells: usize,
}

fn area_judge_source_window_fortran_indexed(
    temp: f64,
    axis: AreaJudgeAxis,
    gridnum_perdegree: usize,
    n_source: usize,
    max_index: usize,
) -> Option<(usize, usize)> {
    if !temp.is_finite() || gridnum_perdegree == 0 || n_source == 0 || max_index < 1 {
        return None;
    }

    let gridnum = gridnum_perdegree as isize;
    let (minsource, maxsource) = match axis {
        AreaJudgeAxis::Longitude => (
            ((temp.floor() as isize) + 180) * gridnum,
            ((temp.ceil() as isize) + 180) * gridnum,
        ),
        AreaJudgeAxis::Latitude => (
            (90 - temp.ceil() as isize) * gridnum,
            (90 - temp.floor() as isize) * gridnum,
        ),
    };

    let start = (minsource - 10).max(1) as usize;
    let end = (maxsource + 10).min((1 + n_source) as isize) as usize;
    if start > end {
        return None;
    }
    Some((start.min(max_index), end.min(max_index)))
}

/// Pure Rust port of `MOD_Area_judge:Source_Find`.
///
/// The routine keeps the Fortran one-based indexing convention: callers pass
/// a placeholder at index 0, source vertices occupy `1..=n_source+1`, longitude
/// vertices ascend from -180 to 180, and latitude vertices descend from 90 to
/// -90.  The search is bounded by the same degree-derived ±10-cell window used
/// in Fortran before scanning for the first matching vertex.
pub fn area_judge_source_find_fortran_indexed(
    temp: f64,
    seq_lonlat: &[f64],
    axis: AreaJudgeAxis,
    gridnum_perdegree: usize,
    n_source: usize,
) -> Option<usize> {
    let max_index = seq_lonlat.len().checked_sub(1)?;
    let (start, end) = area_judge_source_window_fortran_indexed(
        temp,
        axis,
        gridnum_perdegree,
        n_source,
        max_index,
    )?;
    match axis {
        AreaJudgeAxis::Longitude => (start..=end).find(|&index| temp <= seq_lonlat[index]),
        AreaJudgeAxis::Latitude => (start..=end).find(|&index| temp >= seq_lonlat[index]),
    }
}

/// Pure Rust return-value form of `MOD_Area_judge:minmax_range_make`.
///
/// The Fortran subroutine also mutates one of three global range accumulators
/// depending on `type_select`.  This kernel intentionally returns just the
/// source bounds; the later `Area_judge` orchestration can merge these bounds
/// into domain/refine/patch accumulators without reimplementing the lookup.
pub fn area_judge_minmax_range_make_fortran_indexed(
    edgew_temp: f64,
    edgee_temp: f64,
    edgen_temp: f64,
    edges_temp: f64,
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
) -> Option<AreaJudgeSourceBounds> {
    let minlon_source = area_judge_source_find_fortran_indexed(
        edgew_temp,
        lon_vertex,
        AreaJudgeAxis::Longitude,
        gridnum_perdegree,
        nlons_source,
    )?;
    let mut maxlon_source = area_judge_source_find_fortran_indexed(
        edgee_temp,
        lon_vertex,
        AreaJudgeAxis::Longitude,
        gridnum_perdegree,
        nlons_source,
    )?
    .checked_sub(2)?;
    let maxlat_source = area_judge_source_find_fortran_indexed(
        edgen_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    let mut minlat_source = area_judge_source_find_fortran_indexed(
        edges_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?
    .checked_sub(2)?;

    if maxlon_source == nlons_source.saturating_sub(1) {
        maxlon_source += 1;
    }
    if minlat_source == nlats_source.saturating_sub(1) {
        minlat_source += 1;
    }

    Some(AreaJudgeSourceBounds {
        minlon_source,
        maxlon_source,
        maxlat_source,
        minlat_source,
    })
}

fn area_judge_ray_segment_intersection_lon(
    ray_lat: f64,
    start: LonLatDegrees,
    end: LonLatDegrees,
) -> Option<f64> {
    let lat1 = start.lat_degrees;
    let lat2 = end.lat_degrees;
    if lat1 == lat2 {
        return None;
    }
    if (lat1 > ray_lat && lat2 > ray_lat) || (lat1 < ray_lat && lat2 < ray_lat) {
        return None;
    }

    let m = (lat2 - lat1) / (end.lon_degrees - start.lon_degrees);
    Some(start.lon_degrees + (ray_lat - lat1) / m)
}

/// Pure Rust source-cell fill for the closed-curve branch in
/// `MOD_Area_judge:IsInArea_close_Calculation`.
///
/// The helper mirrors the Fortran row scan after `minmax_range_make`: for each
/// source latitude row between the polygon north/south bounds, intersect a
/// left-to-right ray with every polygon segment, sort the intersection
/// longitudes, then mark cells between odd/even intersection pairs.  When
/// `restore_dateline_shift` is true, filled longitude indices are remapped with
/// the same half-world shift that Fortran applies after `CheckCrossing`.
pub fn area_judge_closed_curve_fill_fortran_indexed(
    close_points: &[LonLatDegrees],
    lon_vertex: &[f64],
    lat_vertex: &[f64],
    gridnum_perdegree: usize,
    nlons_source: usize,
    nlats_source: usize,
    restore_dateline_shift: bool,
) -> Option<AreaJudgeClosedCurveFill> {
    if close_points.len() < 3 || lat_vertex.len() < 2 {
        return None;
    }

    let edgen_temp = close_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::NEG_INFINITY, f64::max);
    let edges_temp = close_points
        .iter()
        .map(|point| point.lat_degrees)
        .fold(f64::INFINITY, f64::min);
    let maxlat_source = area_judge_source_find_fortran_indexed(
        edgen_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    let minlat_source = area_judge_source_find_fortran_indexed(
        edges_temp,
        lat_vertex,
        AreaJudgeAxis::Latitude,
        gridnum_perdegree,
        nlats_source,
    )?;
    if minlat_source > lat_vertex.len() {
        return None;
    }
    if maxlat_source > minlat_source {
        return None;
    }
    if maxlat_source == minlat_source {
        return Some(AreaJudgeClosedCurveFill {
            cells: Vec::new(),
            patch_count: 0,
        });
    }

    let mut cells = Vec::new();
    let mut patch_count = 0usize;
    for lat_index in maxlat_source..minlat_source {
        let ray_lat = 0.5 * (lat_vertex[lat_index] + lat_vertex[lat_index + 1]);
        let mut intersections = Vec::new();
        for edge_index in 0..close_points.len() {
            let start = close_points[edge_index];
            let end = close_points[(edge_index + 1) % close_points.len()];
            if let Some(lon_intersect) =
                area_judge_ray_segment_intersection_lon(ray_lat, start, end)
            {
                intersections.push(lon_intersect);
            }
        }
        intersections.sort_by(f64::total_cmp);

        for pair in intersections.chunks_exact(2) {
            let minlon_source = area_judge_source_find_fortran_indexed(
                pair[0],
                lon_vertex,
                AreaJudgeAxis::Longitude,
                gridnum_perdegree,
                nlons_source,
            )?;
            let maxlon_source = area_judge_source_find_fortran_indexed(
                pair[1],
                lon_vertex,
                AreaJudgeAxis::Longitude,
                gridnum_perdegree,
                nlons_source,
            )?;
            if minlon_source > maxlon_source {
                return None;
            }
            patch_count += maxlon_source - minlon_source;
            for lon_index in minlon_source..maxlon_source {
                let restored_lon_index =
                    if restore_dateline_shift && lon_index < nlons_source / 2 + 1 {
                        lon_index + nlons_source / 2
                    } else if restore_dateline_shift {
                        lon_index - nlons_source / 2
                    } else {
                        lon_index
                    };
                cells.push((restored_lon_index, lat_index));
            }
        }
    }

    Some(AreaJudgeClosedCurveFill { cells, patch_count })
}

fn area_judge_grid_covers_bounds_fortran_indexed<T>(
    grid: &[Vec<T>],
    bounds: AreaJudgeSourceBounds,
) -> bool {
    if bounds.minlon_source > bounds.maxlon_source
        || bounds.maxlat_source > bounds.minlat_source
        || bounds.maxlon_source >= grid.len()
    {
        return false;
    }
    (bounds.minlon_source..=bounds.maxlon_source)
        .all(|lon_index| bounds.minlat_source < grid[lon_index].len())
}

/// Pure Rust core of `MOD_Area_judge:mask_patch_modify`.
///
/// Fortran first builds an `IsInPaArea_grid` patch mask, then scans the
/// inclusive patch bounds and sets `seaorland(i, j) = 0` wherever that patch
/// mask is nonzero.  This helper keeps the same one-based array convention and
/// returns the number of nonzero patch cells applied; area construction and
/// NetCDF restart I/O remain in the higher-level orchestration layer.
pub fn area_judge_apply_mask_patch_fortran_indexed(
    seaorland: &mut [Vec<i32>],
    patch_mask: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> Option<AreaJudgeMaskPatchReport> {
    if !area_judge_grid_covers_bounds_fortran_indexed(seaorland, bounds)
        || !area_judge_grid_covers_bounds_fortran_indexed(patch_mask, bounds)
    {
        return None;
    }

    let mut patched_cells = 0usize;
    for lat_index in bounds.maxlat_source..=bounds.minlat_source {
        for lon_index in bounds.minlon_source..=bounds.maxlon_source {
            if patch_mask[lon_index][lat_index] != 0 {
                seaorland[lon_index][lat_index] = 0;
                patched_cells += 1;
            }
        }
    }

    Some(AreaJudgeMaskPatchReport { patched_cells })
}

fn source_find_lon_fortran_indexed(source_lon_vertices: &[f64], lon: f64) -> Option<usize> {
    (1..source_lon_vertices.len()).find(|&index| lon <= source_lon_vertices[index])
}

fn source_find_lat_fortran_indexed(source_lat_vertices: &[f64], lat: f64) -> Option<usize> {
    (1..source_lat_vertices.len()).find(|&index| lat >= source_lat_vertices[index])
}

/// Pure Rust port of the source-mask classification core in
/// `MOD_grid_preprocess:refine_sjx_regional_make`.
///
/// The original routine reads the `mask_patch` NetCDF/file state before this
/// classification loop. This kernel accepts that mask and the source lon/lat
/// vertex arrays explicitly, then mirrors the Fortran `Source_Find` lookup and
/// subsequent `max(1, source - 1)` cell-index shift for each triangle center
/// from `num_mp_step(iter)` onward.
pub fn refine_sjx_regional_make_fortran_indexed(
    input: RefineRegionalMaskInput<'_>,
) -> Option<Vec<bool>> {
    if input.source_lon_vertices.len() < 2
        || input.source_lat_vertices.len() < 2
        || input.mask_patch.is_empty()
    {
        return None;
    }

    let mut refined_triangles = vec![false; input.triangle_lonlat.len()];
    for triangle_id in input.first_triangle_id..input.triangle_lonlat.len() {
        let center = input.triangle_lonlat[triangle_id];
        let lon_source =
            source_find_lon_fortran_indexed(input.source_lon_vertices, center.lon_degrees)?
                .saturating_sub(1)
                .max(1);
        let lat_source =
            source_find_lat_fortran_indexed(input.source_lat_vertices, center.lat_degrees)?
                .saturating_sub(1)
                .max(1);
        if *input.mask_patch.get(lon_source)?.get(lat_source)? {
            refined_triangles[triangle_id] = true;
        }
    }

    Some(refined_triangles)
}

/// Pure Rust port of `MOD_grid_preprocess:set_dbxMove_regional_step`.
///
/// The original routine derives initial refinement flags either from
/// `num_sjx_ref` or `refine_sjx_regional_make`. This core accepts those flags
/// explicitly, expands them through `set_dis` boundary layers, marks cells on
/// refined triangles as movable, freezes mixed boundary cells, then optionally
/// removes cells in protected seed-vertex neighborhoods for
/// `vertex_protect_layers`.
pub fn set_dbx_move_regional_step_fortran_indexed(
    input: RegionalMoveMaskInput<'_>,
) -> Option<RegionalMoveMaskOutput> {
    if input.refined_triangles.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
    {
        return None;
    }

    let (expanded_refined_triangles, boundary_mask) =
        expand_triangles_from_boundary_fortran_indexed(
            input.refined_triangles.to_vec(),
            input.triangles_on_cell,
            input.n_edges_on_cell,
            input.set_dis,
        )?;

    let mut move_mask = vec![false; input.triangles_on_cell.len()];
    for triangle_id in 2..expanded_refined_triangles.len() {
        if !expanded_refined_triangles[triangle_id] {
            continue;
        }
        for &cell_id in input.cells_on_triangle.get(triangle_id)? {
            if cell_id == 0 {
                continue;
            }
            *move_mask.get_mut(cell_id)? = true;
        }
    }
    for cell_id in 2..boundary_mask.len() {
        if boundary_mask[cell_id] {
            move_mask[cell_id] = false;
        }
    }

    let mut protected_triangles = vec![false; input.refined_triangles.len()];
    if input.vertex_protect_layers > 0 && !input.protected_seed_cells.is_empty() {
        let mut active_protected_seed_cells = Vec::new();
        for &cell_id in input.protected_seed_cells {
            let edge_count = *input.n_edges_on_cell.get(cell_id)?;
            let cell_triangles = input.triangles_on_cell.get(cell_id)?;
            if edge_count > cell_triangles.len() {
                return None;
            }
            let touches_refinement = cell_triangles.iter().take(edge_count).any(|&triangle_id| {
                *expanded_refined_triangles
                    .get(triangle_id)
                    .unwrap_or(&false)
            });
            if touches_refinement {
                active_protected_seed_cells.push(cell_id);
            }
        }

        if !active_protected_seed_cells.is_empty() {
            for cell_id in active_protected_seed_cells {
                let edge_count = input.n_edges_on_cell[cell_id];
                let cell_triangles = input.triangles_on_cell.get(cell_id)?;
                for &triangle_id in cell_triangles.iter().take(edge_count) {
                    *protected_triangles.get_mut(triangle_id)? = true;
                }
            }
            protected_triangles = expand_triangles_from_boundary_fortran_indexed(
                protected_triangles,
                input.triangles_on_cell,
                input.n_edges_on_cell,
                input.vertex_protect_layers,
            )?
            .0;

            for triangle_id in 2..protected_triangles.len() {
                if !protected_triangles[triangle_id] {
                    continue;
                }
                for &cell_id in input.cells_on_triangle.get(triangle_id)? {
                    if cell_id == 0 {
                        continue;
                    }
                    *move_mask.get_mut(cell_id)? = false;
                }
            }
        }
    }

    Some(RegionalMoveMaskOutput {
        move_mask,
        boundary_mask,
        expanded_refined_triangles,
        protected_triangles,
    })
}

/// Pure Rust adapter for the in-memory calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_global`.
///
/// This deliberately excludes NetCDF/file side effects. It wires the migrated
/// kernels in the same order as the Fortran workflow: triangle neighbors,
/// edge/connectivity construction, edge-neighbor topology, global spring
/// dynamics, cell lon/lat refresh, triangle centroid/circumcenter refresh, and
/// final MPAS-style vertex-array ordering.
pub fn springjustment_global_core_fortran_indexed(
    input: SpringjustmentGlobalCoreInput<'_>,
) -> Option<SpringjustmentGlobalCoreOutput> {
    if input.triangle_lonlat.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
        || input.cell_lonlat.len() != input.n_edges_on_cell.len()
    {
        spring_global_debug("input dimension check failed");
        return None;
    }

    let triangle_neighbors = match triangle_neighbors_from_cell_membership_fortran_indexed(
        input.cells_on_triangle,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("triangle_neighbors_from_cell_membership failed");
            return None;
        }
    };
    let edge_output = match get_edge_production_fortran_indexed(
        &triangle_neighbors,
        input.cells_on_triangle,
        input.triangle_lonlat,
        input.cell_lonlat,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("get_edge_production failed");
            return None;
        }
    };
    let triangle_points_for_order = input
        .triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let cell_points_for_order = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let geometric_order = order_vertices_on_cell_fortran_indexed(
        &cell_points_for_order,
        &triangle_points_for_order,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
    });
    let topological_order = || {
        order_vertices_on_cell_by_shared_edges_fortran_indexed(
            input.triangles_on_cell,
            input.n_edges_on_cell,
            &edge_output.edges_on_vertex,
        )
        .and_then(|ordered| {
            standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
        })
    };
    let cell_connectivity = match connect_on_cell_fortran_indexed(
        input.n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        input.triangles_on_cell,
    )
    .or_else(|| {
        geometric_order.as_ref().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_output.cells_on_edge,
                &edge_output.edges_on_vertex,
                ordered,
            )
        })
    })
    .or_else(|| {
        topological_order().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_output.cells_on_edge,
                &edge_output.edges_on_vertex,
                &ordered,
            )
        })
    }) {
        Some(value) => value,
        None => {
            spring_global_debug("connect_on_cell failed");
            return None;
        }
    };
    let edges_on_edge_tri = match edges_on_edge_tri_fortran_indexed(
        &edge_output.vertices_on_edge,
        &edge_output.edges_on_vertex,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("edges_on_edge_tri failed");
            return None;
        }
    };
    let distance_output =
        match set_dists_on_edge_global_fortran_indexed(SetDistsOnEdgeGlobalInput {
            base_dists_on_edge: input.base_dists_on_edge,
            base_cellwidth: input.base_cellwidth,
            num_rc: input.distance_num_rc,
            spacing: input.distance_spacing,
            triangles_on_cell: input.triangles_on_cell,
            cells_on_triangle: Some(input.cells_on_triangle),
            edges_on_vertex: &edge_output.edges_on_vertex,
            cells_on_edge: &edge_output.cells_on_edge,
            steps: input.distance_steps,
        }) {
            Some(value) => value,
            None => {
                spring_global_debug("set_dists_on_edge_global failed");
                return None;
            }
        };
    let dists_on_edge = distance_output.dists_on_edge;
    let cellwidth = distance_output.cellwidth;

    let cell_points = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let spring_output = match spring_dynamics_global_fortran_indexed(
        &cell_points,
        input.n_edges_on_cell,
        &cell_connectivity.edges_on_cell,
        &edge_output.cells_on_edge,
        &edges_on_edge_tri,
        &dists_on_edge,
        input.niter_refine,
        input.relax,
        input.radius,
        input.diagnostic_every,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("spring_dynamics_global failed");
            return None;
        }
    };
    let updated_cell_lonlat = spring_output
        .updated_cell_points
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let centroid_lonlat = match centroid_spherical_mesh_fortran_indexed(
        &updated_cell_lonlat,
        input.cells_on_triangle,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("centroid_spherical_mesh failed");
            return None;
        }
    };
    let centroid_cartesian = centroid_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let circumcenters = match circumcenter_spherical_mesh_fortran_indexed(
        &centroid_cartesian,
        &spring_output.updated_cell_points,
        input.cells_on_triangle,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("circumcenter_spherical_mesh failed");
            return None;
        }
    };
    let updated_triangle_lonlat = circumcenters
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let updated_triangle_points = updated_triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let edge_points_cartesian = edge_output
        .edge_points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let final_ordered = match order_vertex_arrays_fortran_indexed(
        &updated_triangle_points,
        &edge_points_cartesian,
        &edge_output.edges_on_vertex,
        &edge_output.vertices_on_edge,
        &edge_output.cells_on_edge,
    ) {
        Some(value) => value,
        None => {
            spring_global_debug("order_vertex_arrays failed");
            return None;
        }
    };

    Some(SpringjustmentGlobalCoreOutput {
        updated_triangle_lonlat,
        updated_cell_lonlat,
        triangle_neighbors,
        cells_on_edge: edge_output.cells_on_edge,
        vertices_on_edge: edge_output.vertices_on_edge,
        edges_on_vertex: final_ordered.edges_on_vertex,
        cells_on_vertex: final_ordered.cells_on_vertex,
        edges_on_cell: cell_connectivity.edges_on_cell,
        cells_on_cell: cell_connectivity.cells_on_cell,
        edges_on_edge_tri,
        dists_on_edge,
        cellwidth,
        edge_lonlat: edge_output.edge_points,
        spring: spring_output,
    })
}

/// Pure Rust adapter for the in-memory calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_regional_step`.
pub fn springjustment_regional_core_fortran_indexed(
    input: SpringjustmentRegionalCoreInput<'_>,
) -> Option<SpringjustmentRegionalCoreOutput> {
    if input.triangle_lonlat.len() != input.cells_on_triangle.len()
        || input.triangles_on_cell.len() != input.n_edges_on_cell.len()
        || input.cell_lonlat.len() != input.n_edges_on_cell.len()
        || input.move_mask.len() != input.n_edges_on_cell.len()
    {
        return None;
    }

    let triangle_neighbors = triangle_neighbors_from_cell_membership_fortran_indexed(
        input.cells_on_triangle,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )?;
    let edge_connectivity =
        get_edge_connectivity_fortran_indexed(&triangle_neighbors, input.cells_on_triangle)?;
    let vertices_on_edge = order_vertices_on_edge_fortran_indexed(
        input.triangle_lonlat,
        input.cell_lonlat,
        &edge_connectivity.cells_on_edge,
        &edge_connectivity.vertices_on_edge,
    )?;
    let triangle_points_for_order = input
        .triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let cell_points_for_order = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let geometric_order = order_vertices_on_cell_fortran_indexed(
        &cell_points_for_order,
        &triangle_points_for_order,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )
    .and_then(|ordered| {
        standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
    });
    let topological_order = || {
        order_vertices_on_cell_by_shared_edges_fortran_indexed(
            input.triangles_on_cell,
            input.n_edges_on_cell,
            &edge_connectivity.edges_on_vertex,
        )
        .and_then(|ordered| {
            standardize_vertices_on_cell_rotation_fortran_indexed(&ordered, input.n_edges_on_cell)
        })
    };
    let cell_connectivity = connect_on_cell_fortran_indexed(
        input.n_edges_on_cell,
        &edge_connectivity.cells_on_edge,
        &edge_connectivity.edges_on_vertex,
        input.triangles_on_cell,
    )
    .or_else(|| {
        geometric_order.as_ref().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_connectivity.cells_on_edge,
                &edge_connectivity.edges_on_vertex,
                ordered,
            )
        })
    })
    .or_else(|| {
        topological_order().and_then(|ordered| {
            connect_on_cell_fortran_indexed(
                input.n_edges_on_cell,
                &edge_connectivity.cells_on_edge,
                &edge_connectivity.edges_on_vertex,
                &ordered,
            )
        })
    })?;

    let cell_points = input
        .cell_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let regional = spring_dynamics_regional_fortran_indexed(
        &cell_points,
        input.n_edges_on_cell,
        &cell_connectivity.cells_on_cell,
        input.move_mask,
        input.niter_refine,
        input.radius,
        input.diagnostic_every,
    )?;
    let updated_cell_lonlat = regional
        .updated_cell_points
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();
    let centroid_lonlat =
        centroid_spherical_mesh_fortran_indexed(&updated_cell_lonlat, input.cells_on_triangle)?;
    let centroid_cartesian = centroid_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .map(|point| {
            CartesianPoint::new(
                point.x * input.radius,
                point.y * input.radius,
                point.z * input.radius,
            )
        })
        .collect::<Vec<_>>();
    let circumcenters = circumcenter_spherical_mesh_fortran_indexed(
        &centroid_cartesian,
        &regional.updated_cell_points,
        input.cells_on_triangle,
    )?;
    let updated_triangle_lonlat = circumcenters
        .iter()
        .copied()
        .map(xyz_to_lonlat_degrees)
        .collect::<Vec<_>>();

    Some(SpringjustmentRegionalCoreOutput {
        triangle_neighbors,
        cells_on_edge: edge_connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: edge_connectivity.edges_on_vertex,
        edges_on_cell: cell_connectivity.edges_on_cell,
        cells_on_cell: cell_connectivity.cells_on_cell,
        updated_cell_lonlat,
        updated_triangle_lonlat,
        regional,
    })
}

fn spring_global_debug(message: &str) {
    if std::env::var_os("EARTHMESH_SPRING_DEBUG").is_some() {
        eprintln!("EARTHMESH_SPRING_DEBUG: {message}");
    }
}

/// Pure Rust adapter for the in-memory mask + calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_regional_step`.
///
/// This keeps NetCDF/file persistence and the original upstream
/// `refine_sjx_regional_make` source classification outside the kernel, but
/// wires the already-migrated `set_dbxMove_regional_step` mask derivation into
/// the regional spring core so callers do not have to manually compose them.
pub fn springjustment_regional_from_refinement_fortran_indexed(
    input: SpringjustmentRegionalFromRefinementInput<'_>,
) -> Option<SpringjustmentRegionalFromRefinementOutput> {
    let mask = set_dbx_move_regional_step_fortran_indexed(RegionalMoveMaskInput {
        set_dis: input.set_dis,
        refined_triangles: input.refined_triangles,
        cells_on_triangle: input.cells_on_triangle,
        triangles_on_cell: input.triangles_on_cell,
        n_edges_on_cell: input.n_edges_on_cell,
        protected_seed_cells: input.protected_seed_cells,
        vertex_protect_layers: input.vertex_protect_layers,
    })?;
    let core = springjustment_regional_core_fortran_indexed(SpringjustmentRegionalCoreInput {
        triangle_lonlat: input.triangle_lonlat,
        cell_lonlat: input.cell_lonlat,
        cells_on_triangle: input.cells_on_triangle,
        triangles_on_cell: input.triangles_on_cell,
        n_edges_on_cell: input.n_edges_on_cell,
        move_mask: &mask.move_mask,
        niter_refine: input.niter_refine,
        radius: input.radius,
        diagnostic_every: input.diagnostic_every,
    })?;

    Some(SpringjustmentRegionalFromRefinementOutput { mask, core })
}

/// Pure Rust adapter for the in-memory source-mask branch of
/// `MOD_grid_preprocess:Springjustment_regional_step`.
///
/// This composes `refine_sjx_regional_make`, `set_dbxMove_regional_step`, and
/// the migrated regional spring/circumcenter core while still leaving NetCDF
/// mask loading and final persistence outside this deterministic kernel.
pub fn springjustment_regional_from_source_mask_fortran_indexed(
    input: SpringjustmentRegionalFromSourceMaskInput<'_>,
) -> Option<SpringjustmentRegionalFromSourceMaskOutput> {
    let refined_triangles = refine_sjx_regional_make_fortran_indexed(RefineRegionalMaskInput {
        triangle_lonlat: input.triangle_lonlat,
        source_lon_vertices: input.source_lon_vertices,
        source_lat_vertices: input.source_lat_vertices,
        mask_patch: input.mask_patch,
        first_triangle_id: input.first_triangle_id,
    })?;
    let regional = springjustment_regional_from_refinement_fortran_indexed(
        SpringjustmentRegionalFromRefinementInput {
            triangle_lonlat: input.triangle_lonlat,
            cell_lonlat: input.cell_lonlat,
            cells_on_triangle: input.cells_on_triangle,
            triangles_on_cell: input.triangles_on_cell,
            n_edges_on_cell: input.n_edges_on_cell,
            refined_triangles: &refined_triangles,
            set_dis: input.set_dis,
            protected_seed_cells: input.protected_seed_cells,
            vertex_protect_layers: input.vertex_protect_layers,
            niter_refine: input.niter_refine,
            radius: input.radius,
            diagnostic_every: input.diagnostic_every,
        },
    )?;

    Some(SpringjustmentRegionalFromSourceMaskOutput {
        refined_triangles,
        regional,
    })
}

/// Port of the candidate-selection core in `MOD_grid_preprocess:orderVertexArrays`.
///
/// From one reference edge vector, choose the candidate edge with positive CCW
/// orientation around the vertex normal and the smallest angle to the reference
/// vector. The returned index is the zero-based slot in `candidate_edges`.
pub fn next_ccw_edge_candidate_slot(
    vertex: CartesianPoint,
    reference_edge: CartesianPoint,
    candidate_edges: &[CartesianPoint],
) -> Option<usize> {
    let normal = vertex;
    let normal_mag = magnitude(normal);
    let reference_vec = vector_between(vertex, reference_edge);
    let reference_mag = magnitude(reference_vec);
    let mut min_angle = std::f64::consts::PI * 2.0;
    let mut best_slot = None;

    for (slot, candidate_edge) in candidate_edges.iter().copied().enumerate() {
        let candidate_vec = vector_between(vertex, candidate_edge);
        let candidate_mag = magnitude(candidate_vec);
        let cross_prod = cross(reference_vec, candidate_vec);
        let cross_mag = magnitude(cross_prod);

        if cross_mag > 1.0e-15 && normal_mag > 1.0e-15 {
            let dot_val = dot(cross_prod, normal) / (cross_mag * normal_mag);
            if dot_val > 0.0 {
                let denom = reference_mag * candidate_mag;
                if denom == 0.0 {
                    continue;
                }
                let cos_angle = (dot(reference_vec, candidate_vec) / denom).clamp(-1.0, 1.0);
                let angle = cos_angle.acos();
                if angle < min_angle {
                    min_angle = angle;
                    best_slot = Some(slot);
                }
            }
        }
    }

    best_slot
}

/// Single-vertex output from `orderVertexArrays`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderedVertexArrays {
    pub edges_on_vertex: [usize; 3],
    pub cells_on_vertex: [usize; 3],
}

/// Array-level output from the Fortran-indexed `orderVertexArrays` port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderedVertexArraysOutput {
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
}

/// Port of the per-vertex mutation/rebuild workflow in `MOD_grid_preprocess:orderVertexArrays`.
///
/// This preserves the Fortran algorithm: mutate `edgesOnVertex` by repeatedly
/// swapping the next smallest positive-CCW edge into the following slot, then
/// rebuild `cellsOnVertex` from `verticesOnEdge` and `cellsOnEdge`.
pub fn order_vertex_arrays_for_vertex(
    vertex_id: usize,
    vertex: CartesianPoint,
    edges_on_vertex: [usize; 3],
    edge_points: &[CartesianPoint],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArrays> {
    let mut ordered_edges = edges_on_vertex;

    for j in 0..3 {
        let edge1 = ordered_edges[j];
        if edge1 == 0 {
            continue;
        }
        let reference_edge = *edge_points.get(edge1)?;
        let candidate_slots = ((j + 1)..3)
            .filter(|slot| ordered_edges[*slot] > 0)
            .collect::<Vec<_>>();
        let candidate_points = candidate_slots
            .iter()
            .map(|slot| edge_points.get(ordered_edges[*slot]).copied())
            .collect::<Option<Vec<_>>>()?;
        let Some(relative_slot) =
            next_ccw_edge_candidate_slot(vertex, reference_edge, &candidate_points)
        else {
            continue;
        };
        let swap_slot = candidate_slots[relative_slot];
        if swap_slot != j + 1 {
            ordered_edges.swap(j + 1, swap_slot);
        }
    }

    let mut ordered_cells = [0usize; 3];
    for j in 0..3 {
        let edge = ordered_edges[j];
        if edge == 0 {
            continue;
        }
        let vertices = *vertices_on_edge.get(edge)?;
        let cells = *cells_on_edge.get(edge)?;
        ordered_cells[j] = if vertex_id == vertices[0] {
            cells[0]
        } else {
            cells[1]
        };
    }

    Some(OrderedVertexArrays {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}

/// Fortran-indexed array wrapper for `MOD_grid_preprocess:orderVertexArrays`.
///
/// Indices `0` and `1` are preserved/skipped so existing Fortran-style ids can
/// be used directly while the rest of the mesh workflow is migrated.
pub fn order_vertex_arrays_fortran_indexed(
    vertex_points: &[CartesianPoint],
    edge_points: &[CartesianPoint],
    edges_on_vertex: &[[usize; 3]],
    vertices_on_edge: &[[usize; 2]],
    cells_on_edge: &[[usize; 2]],
) -> Option<OrderedVertexArraysOutput> {
    if edges_on_vertex.len() < vertex_points.len() {
        return None;
    }

    let mut ordered_edges = edges_on_vertex.to_vec();
    let mut ordered_cells = vec![[0usize; 3]; vertex_points.len()];

    for vertex_id in 2..vertex_points.len() {
        let ordered = order_vertex_arrays_for_vertex(
            vertex_id,
            vertex_points[vertex_id],
            ordered_edges[vertex_id],
            edge_points,
            vertices_on_edge,
            cells_on_edge,
        )?;
        ordered_edges[vertex_id] = ordered.edges_on_vertex;
        ordered_cells[vertex_id] = ordered.cells_on_vertex;
    }

    Some(OrderedVertexArraysOutput {
        edges_on_vertex: ordered_edges,
        cells_on_vertex: ordered_cells,
    })
}

/// Port of `MOD_grid_preprocess:arc_length`.
///
/// Computes spherical arc length from Cartesian coordinates using the same
/// haversine form and float32 squaring emulation described in the Fortran code.
pub fn arc_length_unit_sphere(a: CartesianPoint, b: CartesianPoint) -> f64 {
    let r_a = (a.x * a.x + a.y * a.y + a.z * a.z).sqrt();
    let r_b = (b.x * b.x + b.y * b.y + b.z * b.z).sqrt();

    let lon_a = a.y.atan2(a.x);
    let lat_a = (a.z / r_a).asin();
    let lon_b = b.y.atan2(b.x);
    let lat_b = (b.z / r_b).asin();

    let dlat_half = 0.5 * (lat_a - lat_b);
    let dlon_half = 0.5 * (lon_a - lon_b);

    let sin_dlat_half_f32 = dlat_half.sin() as f32;
    let sin_dlon_half_f32 = dlon_half.sin() as f32;
    let term1 = (sin_dlat_half_f32 * sin_dlat_half_f32) as f64;
    let term2 = lat_b.cos() * lat_a.cos() * (sin_dlon_half_f32 * sin_dlon_half_f32) as f64;

    let arg = (term1 + term2).sqrt();
    r_a * 2.0 * arg.asin()
}

/// Port of `MOD_grid_preprocess:triangle_signed_area_sphere`.
///
/// Despite the Fortran name, the l'Huilier implementation returns a
/// non-negative spherical excess for the three input points. It deliberately
/// reuses `arc_length_unit_sphere` so the same mixed-precision haversine
/// behavior is preserved.
pub fn spherical_triangle_area_unit(points: [CartesianPoint; 3]) -> f64 {
    let a = arc_length_unit_sphere(points[2], points[1]);
    let b = arc_length_unit_sphere(points[2], points[0]);
    let c = arc_length_unit_sphere(points[0], points[1]);
    let semiperimeter = (a + b + c) / 2.0;
    let tan_quarter_excess = (semiperimeter / 2.0).tan()
        * ((semiperimeter - a) / 2.0).tan()
        * ((semiperimeter - b) / 2.0).tan()
        * ((semiperimeter - c) / 2.0).tan();

    4.0 * tan_quarter_excess.max(0.0).sqrt().atan()
}

/// Port of the MPAS kite area primitive inside `MOD_grid_preprocess:GetArea`.
///
/// For one vertex/cell pair, Fortran computes the kite as the absolute area of
/// triangle `(vertex, edge1, cell)` plus triangle `(vertex, edge2, cell)`.
pub fn spherical_kite_area_unit(
    vertex: CartesianPoint,
    edge1: CartesianPoint,
    edge2: CartesianPoint,
    cell: CartesianPoint,
) -> f64 {
    spherical_triangle_area_unit([vertex, edge1, cell]).abs()
        + spherical_triangle_area_unit([vertex, edge2, cell]).abs()
}

/// Port of the `areaCell` fan triangulation inside `MOD_grid_preprocess:GetArea`.
///
/// Fortran pins `verticesOnCell(1, i)` and sums triangles
/// `(v1, vj+1, vj+2)` for `j = 1..num_edges-2`.
pub fn spherical_cell_area_from_vertices_unit(
    vertices: &[CartesianPoint],
    num_edges: usize,
) -> Option<f64> {
    if num_edges < 3 || num_edges > vertices.len() {
        return None;
    }

    let anchor = vertices[0];
    let mut area = 0.0;
    for j in 0..(num_edges - 2) {
        area += spherical_triangle_area_unit([anchor, vertices[j + 1], vertices[j + 2]]);
    }
    Some(area)
}

/// Port of the shared-cell lookup in `MOD_grid_preprocess:GetArea`.
///
/// Fortran checks all four combinations from `cellsOnEdge(:, edge1)` and
/// `cellsOnEdge(:, edge2)` and keeps the maximum matching positive cell id.
/// Zero is the no-cell sentinel and is returned as `None`.
pub fn shared_cell_for_edge_pair(
    edge1_cells: [usize; 2],
    edge2_cells: [usize; 2],
) -> Option<usize> {
    let mut shared_cell = 0usize;
    for cell1 in edge1_cells {
        for cell2 in edge2_cells {
            if cell1 == cell2 {
                shared_cell = shared_cell.max(cell1);
            }
        }
    }

    (shared_cell > 0).then_some(shared_cell)
}

/// Port of the `cellsOnVertex(:, i)` scan in `MOD_grid_preprocess:GetArea`.
///
/// Returns a zero-based Rust index for the matching Fortran `icv` slot.
pub fn vertex_cell_position(cells_on_vertex: [usize; 3], cell: usize) -> Option<usize> {
    cells_on_vertex
        .iter()
        .position(|candidate| *candidate == cell)
}

/// Port of `MOD_grid_preprocess:IsNgrmm`.
///
/// Returns the one-based Fortran code for the vertex in `a` opposite the shared
/// edge with `b`: `1`, `2`, or `3`. Non-neighbor triangles return `None`
/// instead of Fortran's `0` sentinel.
pub fn is_ngrmm(a: [usize; 3], b: [usize; 3]) -> Option<usize> {
    if b.contains(&a[0]) {
        if b.contains(&a[1]) {
            Some(3)
        } else if b.contains(&a[2]) {
            Some(2)
        } else {
            None
        }
    } else if b.contains(&a[1]) && b.contains(&a[2]) {
        Some(1)
    } else {
        None
    }
}

/// Port of the `GetEdge` `cellsOnEdge(:, k)` mapping after `IsNgrmm`.
///
/// The two shared polygon-cell ids are selected from `a` according to the
/// Fortran opposite-vertex code and sorted ascending before return.
pub fn cells_on_edge_from_neighbor_cells(a: [usize; 3], b: [usize; 3]) -> Option<[usize; 2]> {
    let mut cells = match is_ngrmm(a, b)? {
        1 => [a[1], a[2]],
        2 => [a[2], a[0]],
        3 => [a[0], a[1]],
        _ => return None,
    };
    if cells[0] > cells[1] {
        cells.swap(0, 1);
    }
    Some(cells)
}

/// Port of `MOD_grid_preprocess:set_ngrmm`.
///
/// Builds triangle-neighbor slots from triangle-to-cell membership
/// (`cells_on_triangle`) and the inverse cell-to-triangle membership
/// (`triangles_on_cell`). Slots preserve the Fortran `IsNgrmm` meaning:
/// neighbor slot `0`, `1`, or `2` is opposite the corresponding triangle cell.
pub fn triangle_neighbors_from_cell_membership_fortran_indexed(
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    triangle_counts_on_cell: &[usize],
) -> Option<Vec<[usize; 3]>> {
    if triangles_on_cell.len() != triangle_counts_on_cell.len() {
        return None;
    }

    let mut triangle_neighbors = vec![[0usize; 3]; cells_on_triangle.len()];
    for triangle_id in 2..cells_on_triangle.len() {
        let mut neighbor_count = 0usize;
        for &cell_id in &cells_on_triangle[triangle_id] {
            if cell_id == 0 {
                continue;
            }
            let count = *triangle_counts_on_cell.get(cell_id)?;
            let cell_triangles = triangles_on_cell.get(cell_id)?;
            if count > cell_triangles.len() {
                return None;
            }
            if neighbor_count == 3 {
                break;
            }
            for &candidate_triangle_id in cell_triangles.iter().take(count) {
                if candidate_triangle_id == 0 || candidate_triangle_id == triangle_id {
                    continue;
                }
                let candidate_cells = *cells_on_triangle.get(candidate_triangle_id)?;
                let Some(opposite_slot) = is_ngrmm(cells_on_triangle[triangle_id], candidate_cells)
                else {
                    continue;
                };
                triangle_neighbors[triangle_id][opposite_slot - 1] = candidate_triangle_id;
                neighbor_count += 1;
            }
        }
    }

    Some(triangle_neighbors)
}

/// Port of `MOD_grid_preprocess:set_edgesOnEdge_tri`.
///
/// For each edge, returns the two cyclic neighboring edges at the first
/// endpoint followed by the two cyclic neighboring edges at the second endpoint.
/// Indices preserve the Fortran convention that edge ids start at `2`.
pub fn edges_on_edge_tri_fortran_indexed(
    vertices_on_edge: &[[usize; 2]],
    edges_on_vertex: &[[usize; 3]],
) -> Option<Vec<[usize; 4]>> {
    let mut edges_on_edge = vec![[0usize; 4]; vertices_on_edge.len()];

    for edge_id in 2..vertices_on_edge.len() {
        let vertices = vertices_on_edge[edge_id];
        for (endpoint_slot, vertex_id) in vertices.iter().copied().enumerate() {
            let vertex_edges = *edges_on_vertex.get(vertex_id)?;
            let edge_slot = vertex_edges
                .iter()
                .position(|candidate_edge| *candidate_edge == edge_id)?;
            let adjacent_slots = match edge_slot {
                0 => [1, 2],
                1 => [2, 0],
                2 => [0, 1],
                _ => return None,
            };
            edges_on_edge[edge_id][endpoint_slot * 2] = vertex_edges[adjacent_slots[0]];
            edges_on_edge[edge_id][endpoint_slot * 2 + 1] = vertex_edges[adjacent_slots[1]];
        }
    }

    Some(edges_on_edge)
}

/// Output from the core connectivity part of `MOD_grid_preprocess:GetEdge`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEdgeConnectivity {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
}

/// Production-facing `GetEdge` output after the same post-processing sequence
/// used by the global mesh workflow.
#[derive(Debug, Clone, PartialEq)]
pub struct GetEdgeProductionOutput {
    pub cells_on_edge: Vec<[usize; 2]>,
    pub vertices_on_edge: Vec<[usize; 2]>,
    pub edges_on_vertex: Vec<[usize; 3]>,
    pub cells_on_vertex: Vec<[usize; 3]>,
    pub edge_points: Vec<LonLatDegrees>,
}

/// Port of the core connectivity loop in `MOD_grid_preprocess:GetEdge`.
///
/// The optional midpoint calculation is intentionally separate; this function
/// ports edge-id creation/reuse, `verticesOnEdge`, `cellsOnEdge`, and
/// `edgesOnVertex` for Fortran-indexed arrays.
pub fn get_edge_connectivity_fortran_indexed(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
) -> Option<GetEdgeConnectivity> {
    if cells_on_vertex.len() != triangle_neighbors.len() || triangle_neighbors.len() < 2 {
        return None;
    }

    let mut edges_on_vertex = vec![[0usize; 3]; triangle_neighbors.len()];
    let mut cells_on_edge = vec![[0usize; 2]; 2];
    let mut vertices_on_edge = vec![[0usize; 2]; 2];
    let mut triangle_used = vec![false; triangle_neighbors.len()];
    let mut edge_id = 1usize;

    for triangle_id in 2..triangle_neighbors.len() {
        for neighbor_slot in 0..3 {
            let neighbor_id = triangle_neighbors[triangle_id][neighbor_slot];
            if neighbor_id == 0 {
                continue;
            }
            if neighbor_id >= triangle_neighbors.len() {
                return None;
            }

            if triangle_used[neighbor_id] {
                let reuse_slot = triangle_neighbors[neighbor_id]
                    .iter()
                    .position(|candidate| *candidate == triangle_id)?;
                edges_on_vertex[triangle_id][neighbor_slot] =
                    edges_on_vertex[neighbor_id][reuse_slot];
                continue;
            }

            edge_id += 1;
            if cells_on_edge.len() <= edge_id {
                cells_on_edge.resize(edge_id + 1, [0usize; 2]);
                vertices_on_edge.resize(edge_id + 1, [0usize; 2]);
            }

            edges_on_vertex[triangle_id][neighbor_slot] = edge_id;
            vertices_on_edge[edge_id] = [triangle_id, neighbor_id];
            cells_on_edge[edge_id] = cells_on_edge_from_neighbor_cells(
                cells_on_vertex[triangle_id],
                cells_on_vertex[neighbor_id],
            )?;
        }
        triangle_used[triangle_id] = true;
    }

    Some(GetEdgeConnectivity {
        cells_on_edge,
        vertices_on_edge,
        edges_on_vertex,
    })
}

/// Production wrapper for `MOD_grid_preprocess:GetEdge` plus the immediate
/// post-processing used before MPAS-style mesh outputs are consumed.
///
/// The sequence matches the migrated workflow surfaces:
/// `GetEdge`, `GetSort_verticesOnEdge`, optional `vp` midpoint generation, and
/// `orderVertexArrays`.
pub fn get_edge_production_fortran_indexed(
    triangle_neighbors: &[[usize; 3]],
    cells_on_vertex: &[[usize; 3]],
    triangle_lonlat: &[LonLatDegrees],
    cell_lonlat: &[LonLatDegrees],
) -> Option<GetEdgeProductionOutput> {
    let connectivity = get_edge_connectivity_fortran_indexed(triangle_neighbors, cells_on_vertex)?;
    let vertices_on_edge = order_vertices_on_edge_fortran_indexed(
        triangle_lonlat,
        cell_lonlat,
        &connectivity.cells_on_edge,
        &connectivity.vertices_on_edge,
    )?;
    let edge_points =
        edge_midpoints_from_cells_fortran_indexed(&connectivity.cells_on_edge, cell_lonlat)?;
    let triangle_points = triangle_lonlat
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let edge_points_cartesian = edge_points
        .iter()
        .copied()
        .map(lonlat_degrees_to_unit_xyz)
        .collect::<Vec<_>>();
    let ordered_vertex_arrays = order_vertex_arrays_fortran_indexed(
        &triangle_points,
        &edge_points_cartesian,
        &connectivity.edges_on_vertex,
        &vertices_on_edge,
        &connectivity.cells_on_edge,
    )?;

    Some(GetEdgeProductionOutput {
        cells_on_edge: connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: ordered_vertex_arrays.edges_on_vertex,
        cells_on_vertex: ordered_vertex_arrays.cells_on_vertex,
        edge_points,
    })
}

/// Port of the optional `vp` midpoint calculation in `MOD_grid_preprocess:GetEdge`.
///
/// For each Fortran-indexed edge id from `2..`, the edge point is the spherical
/// centroid of the two neighboring polygon cell centers `wp(cellsOnEdge(:, k), :)`.
pub fn edge_midpoints_from_cells_fortran_indexed(
    cells_on_edge: &[[usize; 2]],
    cell_lonlat: &[LonLatDegrees],
) -> Option<Vec<LonLatDegrees>> {
    let mut midpoints = vec![LonLatDegrees::new(0.0, 0.0); cells_on_edge.len()];
    for edge_id in 2..cells_on_edge.len() {
        let cells = cells_on_edge[edge_id];
        let cell1 = *cell_lonlat.get(cells[0])?;
        let cell2 = *cell_lonlat.get(cells[1])?;
        midpoints[edge_id] = spherical_centroid_degrees(&[cell1, cell2])?;
    }
    Some(midpoints)
}

/// Borrowed inputs for the Fortran-indexed subset of `MOD_grid_preprocess:GetArea`.
///
/// Index `0` is unused and index `1` is skipped to mirror the Fortran loops
/// that run from `2` through the allocated counts. Positive connectivity ids
/// are therefore used directly as Rust vector indices.
#[derive(Debug, Clone, Copy)]
pub struct GetAreaUnitInput<'a> {
    pub vertices: &'a [CartesianPoint],
    pub edge_points: &'a [CartesianPoint],
    pub cell_points: &'a [CartesianPoint],
    pub cells_on_vertex: &'a [[usize; 3]],
    pub edges_on_vertex: &'a [[usize; 3]],
    pub cells_on_edge: &'a [[usize; 2]],
    pub vertices_on_cell: &'a [Vec<usize>],
    pub n_edges_on_cell: &'a [usize],
}

/// Unit-sphere area outputs from the Fortran-indexed `GetArea` subset.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaUnitOutput {
    pub kite_areas_on_vertex: Vec<[f64; 3]>,
    pub area_triangle: Vec<f64>,
    pub area_cell: Vec<f64>,
}

/// Relative reconstruction error summary printed by `MOD_grid_preprocess:GetArea`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AreaTriangleReconstructionError {
    pub max_relative: f64,
    pub avg_relative: f64,
}

/// Port of the core array workflow in `MOD_grid_preprocess:GetArea`.
///
/// This keeps the Fortran indexing convention and computes:
///
/// - `kiteAreasOnVertex(icv, i)` from consecutive edge pairs around a vertex.
/// - `areaTriangle(i)` as the sum of the three kite slots for each vertex.
/// - `areaCell(i)` by fan-triangulating `verticesOnCell(:, i)`.
pub fn get_area_unit_fortran_indexed(input: GetAreaUnitInput<'_>) -> Option<GetAreaUnitOutput> {
    if input.cells_on_vertex.len() < input.vertices.len()
        || input.edges_on_vertex.len() < input.vertices.len()
    {
        return None;
    }

    let mut kite_areas_on_vertex = vec![[0.0; 3]; input.vertices.len()];
    let mut area_triangle = vec![0.0; input.vertices.len()];
    if input.n_edges_on_cell.len() < input.cell_points.len() {
        return None;
    }

    let mut area_cell = vec![0.0; input.cell_points.len()];

    for vertex_id in 2..input.vertices.len() {
        let vertex = input.vertices[vertex_id];
        let cells_on_vertex = input.cells_on_vertex[vertex_id];
        let edges_on_vertex = input.edges_on_vertex[vertex_id];

        for edge_slot in 0..3 {
            let next_edge_slot = (edge_slot + 1) % 3;
            let edge1 = edges_on_vertex[edge_slot];
            let edge2 = edges_on_vertex[next_edge_slot];
            if edge1 == 0 || edge2 == 0 {
                continue;
            }

            let edge1_cells = *input.cells_on_edge.get(edge1)?;
            let edge2_cells = *input.cells_on_edge.get(edge2)?;
            let Some(cell_id) = shared_cell_for_edge_pair(edge1_cells, edge2_cells) else {
                continue;
            };
            let Some(vertex_cell_slot) = vertex_cell_position(cells_on_vertex, cell_id) else {
                continue;
            };

            let edge1_point = *input.edge_points.get(edge1)?;
            let edge2_point = *input.edge_points.get(edge2)?;
            let cell_point = *input.cell_points.get(cell_id)?;
            kite_areas_on_vertex[vertex_id][vertex_cell_slot] =
                spherical_kite_area_unit(vertex, edge1_point, edge2_point, cell_point);
        }
    }

    for vertex_id in 2..input.vertices.len() {
        area_triangle[vertex_id] = kite_areas_on_vertex[vertex_id].iter().sum();
    }

    for cell_id in 2..input.cell_points.len() {
        let Some(vertex_ids) = input.vertices_on_cell.get(cell_id) else {
            continue;
        };
        if vertex_ids.len() < 3 {
            continue;
        }
        let num_edges = *input.n_edges_on_cell.get(cell_id)?;
        if num_edges < 3 || num_edges > vertex_ids.len() {
            continue;
        }
        let vertices = vertex_ids
            .iter()
            .map(|vertex_id| input.vertices.get(*vertex_id).copied())
            .collect::<Option<Vec<_>>>()?;
        area_cell[cell_id] = spherical_cell_area_from_vertices_unit(&vertices, num_edges)?;
    }

    Some(GetAreaUnitOutput {
        kite_areas_on_vertex,
        area_triangle,
        area_cell,
    })
}

/// Production-facing `GetArea` output with the diagnostic summary printed by
/// the Fortran routine.
#[derive(Debug, Clone, PartialEq)]
pub struct GetAreaProductionOutput {
    pub unit: GetAreaUnitOutput,
    pub reconstruction_error: AreaTriangleReconstructionError,
}

/// Production wrapper for `MOD_grid_preprocess:GetArea`.
///
/// This combines the migrated unit-sphere area workflow with the reconstruction
/// relative-error diagnostic that the Fortran routine prints after computing
/// `areaTriangle`.
pub fn get_area_production_fortran_indexed(
    input: GetAreaUnitInput<'_>,
) -> Option<GetAreaProductionOutput> {
    let unit = get_area_unit_fortran_indexed(input)?;
    let reconstruction_error = area_triangle_reconstruction_error_fortran_indexed(
        &unit.area_triangle,
        input.cell_points,
        input.cells_on_vertex,
    )?;

    Some(GetAreaProductionOutput {
        unit,
        reconstruction_error,
    })
}

/// Port of the `GetArea` area-triangle reconstruction error summary.
///
/// For each Fortran-indexed vertex id from `2..`, the routine recomputes the
/// triangle area from `cellsOnVertex(:, i)` cell centers and compares it with
/// the reconstructed `areaTriangle(i)`.
pub fn area_triangle_reconstruction_error_fortran_indexed(
    area_triangle: &[f64],
    cell_points: &[CartesianPoint],
    cells_on_vertex: &[[usize; 3]],
) -> Option<AreaTriangleReconstructionError> {
    if area_triangle.len() < 3 || cells_on_vertex.len() < area_triangle.len() {
        return None;
    }

    let mut max_relative = 0.0;
    let mut sum_relative = 0.0;
    let mut count = 0usize;

    for vertex_id in 2..area_triangle.len() {
        let cell_ids = cells_on_vertex[vertex_id];
        if cell_ids.contains(&0) {
            return None;
        }
        let exact = spherical_triangle_area_unit([
            *cell_points.get(cell_ids[0])?,
            *cell_points.get(cell_ids[1])?,
            *cell_points.get(cell_ids[2])?,
        ]);
        if exact == 0.0 {
            return None;
        }
        let relative = (area_triangle[vertex_id] - exact).abs() / exact;
        max_relative = f64::max(max_relative, relative);
        sum_relative += relative;
        count += 1;
    }

    Some(AreaTriangleReconstructionError {
        max_relative,
        avg_relative: sum_relative / count as f64,
    })
}

/// Output of `MOD_grid_preprocess:Get_Length_Angle`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonLengthAngleMetrics {
    pub angles_degrees: Vec<f64>,
    pub edge_lengths_meters: Vec<f64>,
}

/// Port of `MOD_grid_preprocess:Get_Length_Angle`.
///
/// For each polygon vertex, this builds the same `(previous, current, next)`
/// triplet as the Fortran cyclic buffer, computes the spherical angle using the
/// half-angle formula, and records the current-to-next edge length scaled by
/// `erad8`.
pub fn polygon_length_angle_metrics(points: &[LonLatDegrees]) -> Option<PolygonLengthAngleMetrics> {
    let num_edges = points.len();
    if num_edges < 3 {
        return None;
    }

    let mut angles_degrees = Vec::with_capacity(num_edges);
    let mut edge_lengths_meters = Vec::with_capacity(num_edges);

    for i in 0..num_edges {
        let previous = points[(i + num_edges - 1) % num_edges];
        let current = points[i];
        let next = points[(i + 1) % num_edges];

        let previous_xyz = lonlat_degrees_to_unit_xyz(previous);
        let current_xyz = lonlat_degrees_to_unit_xyz(current);
        let next_xyz = lonlat_degrees_to_unit_xyz(next);

        let length1 = arc_length_unit_sphere(next_xyz, current_xyz);
        let length2 = arc_length_unit_sphere(next_xyz, previous_xyz);
        let length3 = arc_length_unit_sphere(previous_xyz, current_xyz);
        let semiperimeter = 0.5 * (length1 + length2 + length3);
        let angle_arg = ((semiperimeter - length1).sin() * (semiperimeter - length3).sin()
            / (length1.sin() * length3.sin()))
        .sqrt();
        angles_degrees.push(rad_to_deg(2.0 * angle_arg.asin()));
        edge_lengths_meters.push(length1 * earthmesh_core::EARTH_RADIUS_METERS);
    }

    Some(PolygonLengthAngleMetrics {
        angles_degrees,
        edge_lengths_meters,
    })
}

/// Mesh-quality aggregate produced by Fortran `TriMeshQuality`/`PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshQualitySummary {
    pub cell_metrics: Vec<PolygonLengthAngleMetrics>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

fn polygon_quality_summary(
    cells: &[Vec<LonLatDegrees>],
    regular_angle_degrees: f64,
    lower_threshold_degrees: f64,
    upper_threshold_degrees: f64,
) -> Option<MeshQualitySummary> {
    if cells.is_empty() {
        return None;
    }

    let mut cell_metrics = Vec::with_capacity(cells.len());
    let mut angle_less_flags = Vec::with_capacity(cells.len());
    let mut angle_more_flags = Vec::with_capacity(cells.len());
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut angle_count = 0usize;

    for cell in cells {
        let metrics = polygon_length_angle_metrics(cell)?;
        let cell_min = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let cell_max = metrics
            .angles_degrees
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        global_min = global_min.min(cell_min);
        global_max = global_max.max(cell_max);
        sum_min += cell_min;
        sum_max += cell_max;
        sum_squared += metrics
            .angles_degrees
            .iter()
            .map(|angle| (angle - regular_angle_degrees).powi(2))
            .sum::<f64>();
        angle_count += metrics.angles_degrees.len();
        angle_less_flags.push(cell_min < lower_threshold_degrees);
        angle_more_flags.push(cell_max > upper_threshold_degrees);
        cell_metrics.push(metrics);
    }

    let cell_count = cells.len() as f64;
    Some(MeshQualitySummary {
        cell_metrics,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / cell_count, sum_max / cell_count),
        angle_stddev_degrees: (sum_squared / angle_count as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:TriMeshQuality`.
pub fn triangle_mesh_quality(triangles: &[[LonLatDegrees; 3]]) -> Option<MeshQualitySummary> {
    let cells: Vec<Vec<LonLatDegrees>> =
        triangles.iter().map(|triangle| triangle.to_vec()).collect();
    polygon_quality_summary(&cells, 60.0, 45.0, 75.0)
}

/// Fortran-style cache/update output for `MOD_grid_preprocess:TriMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleMeshQualityFortranOutput {
    pub length_cache: Vec<[f64; 3]>,
    pub angle_cache: Vec<[f64; 3]>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:TriMeshQuality`.
///
/// Inputs use the repository's Rust convention for migrated Fortran-indexed
/// arrays: slots `0` and `1` are placeholders and triangle ids start at `2`.
/// Adjusted triangles are recalculated from `cell_points`/`cells_on_triangle`;
/// unadjusted triangles reuse the provided angle/length caches.
pub fn triangle_mesh_quality_fortran_indexed(
    cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    adjust_flags: &[bool],
    length_cache: &[[f64; 3]],
    angle_cache: &[[f64; 3]],
) -> Option<TriangleMeshQualityFortranOutput> {
    let len = cells_on_triangle.len();
    if len < 3 || adjust_flags.len() != len || length_cache.len() != len || angle_cache.len() != len
    {
        return None;
    }

    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; len];
    let mut angle_more_flags = vec![false; len];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut count = 0usize;

    for triangle_id in 2..len {
        if adjust_flags[triangle_id] {
            let cell_ids = cells_on_triangle[triangle_id];
            let triangle = [
                *cell_points.get(cell_ids[0])?,
                *cell_points.get(cell_ids[1])?,
                *cell_points.get(cell_ids[2])?,
            ];
            let metrics = polygon_length_angle_metrics(&triangle)?;
            updated_angles[triangle_id] = [
                metrics.angles_degrees[0],
                metrics.angles_degrees[1],
                metrics.angles_degrees[2],
            ];
            updated_lengths[triangle_id] = [
                metrics.edge_lengths_meters[0],
                metrics.edge_lengths_meters[1],
                metrics.edge_lengths_meters[2],
            ];
        }

        let angles = updated_angles[triangle_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - 60.0).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[triangle_id] = min_angle < 45.0;
        angle_more_flags[triangle_id] = max_angle > 75.0;
        count += 1;
    }

    Some(TriangleMeshQualityFortranOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (sum_min / count as f64, sum_max / count as f64),
        angle_stddev_degrees: (sum_squared / (3 * count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Port of the aggregation core in `MOD_grid_preprocess:PolyMeshQuality`.
///
/// All cells in the input should have the same edge count, matching each
/// Fortran call for pentagons, hexagons, or heptagons. The regular angle is
/// `(num_edges - 2) * 180 / num_edges`, with 0.9/1.1 threshold bands.
pub fn polygon_mesh_quality(cells: &[Vec<LonLatDegrees>]) -> Option<MeshQualitySummary> {
    let first = cells.first()?;
    let num_edges = first.len();
    if num_edges < 3 || cells.iter().any(|cell| cell.len() != num_edges) {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    polygon_quality_summary(
        cells,
        regular_angle,
        regular_angle * 0.9,
        regular_angle * 1.1,
    )
}

/// Fortran-style compact cache/update output for `MOD_grid_preprocess:PolyMeshQuality`.
#[derive(Debug, Clone, PartialEq)]
pub struct PolygonMeshQualityFortranOutput {
    pub length_cache: Vec<Vec<f64>>,
    pub angle_cache: Vec<Vec<f64>>,
    pub extreme_angles_degrees: (f64, f64),
    pub average_min_max_angles_degrees: (f64, f64),
    pub angle_stddev_degrees: f64,
    pub angle_less_flags: Vec<bool>,
    pub angle_more_flags: Vec<bool>,
}

/// Cache-aware port of `MOD_grid_preprocess:PolyMeshQuality`.
///
/// Fortran iterates over cell ids from `2`, skips cells whose `n_ngrwm` does not
/// match `num_edges`, and stores quality caches in a compact `j` index for only
/// the matching cells. This Rust port preserves that compact-cache contract.
pub fn polygon_mesh_quality_fortran_indexed(
    num_edges: usize,
    cell_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
    length_cache: &[Vec<f64>],
    angle_cache: &[Vec<f64>],
) -> Option<PolygonMeshQualityFortranOutput> {
    let len = cells_on_polygon.len();
    if num_edges < 3 || len < 3 || polygon_edge_counts.len() != len || adjust_flags.len() != len {
        return None;
    }

    let matching_count = (2..len)
        .filter(|&cell_id| polygon_edge_counts[cell_id] == num_edges)
        .count();
    if matching_count == 0
        || length_cache.len() != matching_count
        || angle_cache.len() != matching_count
        || length_cache.iter().any(|row| row.len() != num_edges)
        || angle_cache.iter().any(|row| row.len() != num_edges)
    {
        return None;
    }

    let regular_angle = (num_edges as f64 - 2.0) * 180.0 / num_edges as f64;
    let angle_regularless = regular_angle * 0.9;
    let angle_regularmore = regular_angle * 1.1;
    let mut updated_lengths = length_cache.to_vec();
    let mut updated_angles = angle_cache.to_vec();
    let mut angle_less_flags = vec![false; matching_count];
    let mut angle_more_flags = vec![false; matching_count];
    let mut sum_min = 0.0;
    let mut sum_max = 0.0;
    let mut sum_squared = 0.0;
    let mut global_min = f64::INFINITY;
    let mut global_max = f64::NEG_INFINITY;
    let mut compact_id = 0usize;

    for cell_id in 2..len {
        if polygon_edge_counts[cell_id] != num_edges {
            continue;
        }

        if adjust_flags[cell_id] {
            let polygon_indices = cells_on_polygon.get(cell_id)?;
            if polygon_indices.len() < num_edges {
                return None;
            }
            let mut polygon = Vec::with_capacity(num_edges);
            for &point_id in polygon_indices.iter().take(num_edges) {
                polygon.push(*cell_points.get(point_id)?);
            }
            let metrics = polygon_length_angle_metrics(&polygon)?;
            updated_angles[compact_id] = metrics.angles_degrees;
            updated_lengths[compact_id] = metrics.edge_lengths_meters;
        }

        let angles = &updated_angles[compact_id];
        let min_angle = angles.iter().copied().fold(f64::INFINITY, f64::min);
        let max_angle = angles.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        sum_min += min_angle;
        sum_max += max_angle;
        sum_squared += angles
            .iter()
            .map(|angle| (angle - regular_angle).powi(2))
            .sum::<f64>();
        global_min = global_min.min(min_angle);
        global_max = global_max.max(max_angle);
        angle_less_flags[compact_id] = min_angle < angle_regularless;
        angle_more_flags[compact_id] = max_angle > angle_regularmore;
        compact_id += 1;
    }

    Some(PolygonMeshQualityFortranOutput {
        length_cache: updated_lengths,
        angle_cache: updated_angles,
        extreme_angles_degrees: (global_min, global_max),
        average_min_max_angles_degrees: (
            sum_min / matching_count as f64,
            sum_max / matching_count as f64,
        ),
        angle_stddev_degrees: (sum_squared / (num_edges * matching_count) as f64).sqrt(),
        angle_less_flags,
        angle_more_flags,
    })
}

/// Polygon edge-count classes reported by
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolygonEdgeClassCounts {
    pub pentagons: usize,
    pub hexagons: usize,
    pub heptagons: usize,
    pub less_than_five: usize,
    pub greater_than_seven: usize,
}

/// Quality summaries produced by the Rust orchestration wrapper for
/// `MOD_grid_preprocess:Grid_Quality_Check_Global`.
#[derive(Debug, Clone, PartialEq)]
pub struct GridQualityGlobalOutput {
    pub edge_class_counts: PolygonEdgeClassCounts,
    pub triangle: TriangleMeshQualityFortranOutput,
    pub pentagon: Option<PolygonMeshQualityFortranOutput>,
    pub hexagon: Option<PolygonMeshQualityFortranOutput>,
    pub heptagon: Option<PolygonMeshQualityFortranOutput>,
}

fn polygon_edge_class_counts_fortran_indexed(
    polygon_edge_counts: &[usize],
) -> PolygonEdgeClassCounts {
    let mut counts = PolygonEdgeClassCounts {
        pentagons: 0,
        hexagons: 0,
        heptagons: 0,
        less_than_five: 0,
        greater_than_seven: 0,
    };

    for edge_count in polygon_edge_counts.iter().copied().skip(2) {
        match edge_count {
            5 => counts.pentagons += 1,
            6 => counts.hexagons += 1,
            7 => counts.heptagons += 1,
            count if count < 5 => counts.less_than_five += 1,
            _ => counts.greater_than_seven += 1,
        }
    }

    counts
}

fn polygon_quality_or_none_fortran_indexed(
    num_edges: usize,
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
    adjust_flags: &[bool],
) -> Option<Option<PolygonMeshQualityFortranOutput>> {
    let matching_count = polygon_edge_counts
        .iter()
        .copied()
        .skip(2)
        .filter(|edge_count| *edge_count == num_edges)
        .count();

    if matching_count == 0 {
        return Some(None);
    }

    let length_cache = vec![vec![0.0; num_edges]; matching_count];
    let angle_cache = vec![vec![0.0; num_edges]; matching_count];
    polygon_mesh_quality_fortran_indexed(
        num_edges,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        adjust_flags,
        &length_cache,
        &angle_cache,
    )
    .map(Some)
}

/// Rust orchestration wrapper for `MOD_grid_preprocess:Grid_Quality_Check_Global`.
///
/// This ports the calculation side of the Fortran routine: polygon edge-class
/// counting, all-true initial adjust flags, triangle quality, and 5/6/7-sided
/// polygon quality groups. The NetCDF `quality_save_global` side effect remains
/// an adapter/output-layer responsibility.
pub fn grid_quality_check_global_fortran_indexed(
    triangle_cell_points: &[LonLatDegrees],
    cells_on_triangle: &[[usize; 3]],
    polygon_points: &[LonLatDegrees],
    cells_on_polygon: &[Vec<usize>],
    polygon_edge_counts: &[usize],
) -> Option<GridQualityGlobalOutput> {
    if cells_on_polygon.len() != polygon_edge_counts.len() {
        return None;
    }

    let edge_class_counts = polygon_edge_class_counts_fortran_indexed(polygon_edge_counts);
    let triangle_adjust_flags = vec![true; cells_on_triangle.len()];
    let triangle_length_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle_angle_cache = vec![[0.0; 3]; cells_on_triangle.len()];
    let triangle = triangle_mesh_quality_fortran_indexed(
        triangle_cell_points,
        cells_on_triangle,
        &triangle_adjust_flags,
        &triangle_length_cache,
        &triangle_angle_cache,
    )?;

    let polygon_adjust_flags = vec![true; cells_on_polygon.len()];
    let pentagon = polygon_quality_or_none_fortran_indexed(
        5,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let hexagon = polygon_quality_or_none_fortran_indexed(
        6,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;
    let heptagon = polygon_quality_or_none_fortran_indexed(
        7,
        polygon_points,
        cells_on_polygon,
        polygon_edge_counts,
        &polygon_adjust_flags,
    )?;

    Some(GridQualityGlobalOutput {
        edge_class_counts,
        triangle,
        pentagon,
        hexagon,
        heptagon,
    })
}

/// Port of `MOD_grid_preprocess:robust_spherical_area`.
///
/// Returns signed area on the unit sphere. The caller can multiply by radius²
/// when physical area is needed. The formula preserves Fortran's dateline-aware
/// `delta_lon` adjustment and does not take an absolute value.
pub fn robust_spherical_area_unit(points: &[LonLatDegrees]) -> Option<f64> {
    let num_inter = points.len();
    if num_inter < 3 {
        return None;
    }

    let mut area = 0.0;
    for i in 0..num_inter {
        let j = (i + 1) % num_inter;
        let lon_i = deg_to_rad(points[i].lon_degrees);
        let lon_j = deg_to_rad(points[j].lon_degrees);
        let lat_i = deg_to_rad(points[i].lat_degrees);
        let lat_j = deg_to_rad(points[j].lat_degrees);
        let mut delta_lon = lon_j - lon_i;
        if delta_lon > std::f64::consts::PI {
            delta_lon -= 2.0 * std::f64::consts::PI;
        } else if delta_lon < -std::f64::consts::PI {
            delta_lon += 2.0 * std::f64::consts::PI;
        }
        area += delta_lon * (2.0 + lat_i.sin() + lat_j.sin());
    }

    Some(area / 2.0)
}

/// Result of `MOD_mask_postproc.F90:sort_and_reindex`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VertexReindex {
    pub sorted_vertices: Vec<usize>,
    pub vertex_mapping: Vec<usize>,
}

/// Result of `MOD_mask_postproc.F90:Data_Renew`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskPostprocRenewedData {
    pub points_next: usize,
    pub bounds_next: usize,
    pub center_neighbors_next: Vec<Vec<usize>>,
    pub vertex_neighbors_next: Vec<Vec<usize>>,
    pub center_neighbor_counts_next: Vec<usize>,
    pub vertex_neighbor_counts_next: Vec<usize>,
}

/// Result of `MOD_mask_postproc.F90:Data_Finial`.
#[derive(Debug, Clone, PartialEq)]
pub struct MaskPostprocFinalData {
    pub points_final: usize,
    pub bounds_final: usize,
    pub center_coordinates_final: Vec<[f64; 2]>,
    pub vertex_coordinates_final: Vec<[f64; 2]>,
    pub center_neighbors_final: Vec<Vec<usize>>,
    pub vertex_neighbors_final: Vec<Vec<usize>>,
    pub center_neighbor_counts_final: Vec<usize>,
    pub vertex_neighbor_counts_final: Vec<usize>,
}

/// Result of `MOD_mask_postproc.F90:bdy_connection_closed_curve`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryClosedCurves {
    pub num_closed_curve: usize,
    pub num_bdy_long: [usize; 3],
    pub close_curves: Vec<Vec<usize>>,
    pub n_close_curve: Vec<usize>,
}

/// Result of `MOD_mask_postproc.F90:bdy_connection` before NetCDF output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryConnection {
    pub bdy_num_in: usize,
    pub boundary_order: Vec<usize>,
    pub boundary_neighbors: Vec<Vec<usize>>,
    pub curves: BoundaryClosedCurves,
}

/// Result of `MOD_refine.F90:bdy_refine_segment_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineBoundarySegments {
    pub num_bdy_refine_segment: usize,
    pub bdy_refine_segment: Vec<Vec<usize>>,
    pub n_bdy_refine_segment: Vec<usize>,
}

/// Result of `MOD_refine.F90:weak_concav_segment_make`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefineWeakConcavitySegments {
    pub num_ref_weak_concav: usize,
    pub num_weak_concav_segment: usize,
    pub num_weak_concav_pair: usize,
    pub bdy_refine_segment: Vec<Vec<usize>>,
    pub n_bdy_refine_segment: Vec<usize>,
    pub weak_concav_segment: Vec<Vec<usize>>,
    pub n_weak_concav_segment: Vec<usize>,
    pub weak_concav_pair: Vec<[usize; 2]>,
}

/// Result of `MOD_mask_postproc.F90:Isolated_Ocean_Renew`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedOceanRenewal {
    pub num_bdy_long: [usize; 3],
    pub bdy_long_order: Vec<usize>,
    pub removed_curve_ids: Vec<usize>,
    pub n_close_curve_after: Vec<usize>,
}

/// Result of the pure classification part of `MOD_mask_postproc.F90:bdy_calculation`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryOrders {
    pub bdy_order: Vec<usize>,
    pub obc_order: Vec<usize>,
    pub ibc_order: Vec<usize>,
    pub rotation_start: Option<usize>,
}

/// Port of `MOD_mask_postproc.F90:extract_unique_vertices`.
///
/// The input is Rust row-major by center id: `center_neighbors[j][i]` mirrors
/// Fortran `ustr_ngr_center_f(i, j)`. Slot `1` is preserved as the legacy empty
/// vertex placeholder and the scan starts at center id `2`.
pub fn extract_unique_vertices_fortran_indexed(
    center_neighbors: &[Vec<usize>],
    neighbor_counts: &[usize],
    max_vertex_id: usize,
) -> io::Result<Vec<usize>> {
    if neighbor_counts.len() < center_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "neighbor_counts length must cover center_neighbors",
        ));
    }

    let mut is_selected = vec![true; max_vertex_id + 1];
    let mut unique_vertices = vec![1];
    for center_id in 2..center_neighbors.len() {
        let count = neighbor_counts[center_id];
        if count > center_neighbors[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "neighbor count {count} exceeds center {center_id} row length {}",
                    center_neighbors[center_id].len()
                ),
            ));
        }
        for &vertex_id in center_neighbors[center_id].iter().take(count) {
            if vertex_id > max_vertex_id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {vertex_id}, outside 0..={max_vertex_id}"
                    ),
                ));
            }
            if is_selected[vertex_id] {
                unique_vertices.push(vertex_id);
                is_selected[vertex_id] = false;
            }
        }
    }

    Ok(unique_vertices)
}

/// Port of `MOD_mask_postproc.F90:sort_and_reindex`.
///
/// Returns the sorted unique vertex list and the Fortran-style old vertex id to
/// new compact id mapping. Mapping slot `0` is retained but unused.
pub fn sort_and_reindex_vertices(
    unique_vertices: &[usize],
    max_vertex_id: usize,
) -> io::Result<VertexReindex> {
    let mut sorted_vertices = unique_vertices.to_vec();
    sorted_vertices.sort_unstable();

    let mut vertex_mapping = vec![0; max_vertex_id + 1];
    for (new_id, &old_vertex_id) in sorted_vertices.iter().enumerate() {
        if old_vertex_id > max_vertex_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {old_vertex_id} outside 0..={max_vertex_id}"),
            ));
        }
        vertex_mapping[old_vertex_id] = new_id + 1;
    }

    Ok(VertexReindex {
        sorted_vertices,
        vertex_mapping,
    })
}

/// Port of the final `ustr_ngr_center_f = vertex_mapping(ustr_ngr_center_f)`
/// loop in `MOD_mask_postproc.F90:mask_postproc_*`.
///
/// The scan preserves Fortran indexing by leaving rows `0` and `1` untouched
/// and only remapping slots covered by `center_neighbor_counts`.
pub fn reindex_final_center_vertices_fortran_indexed(
    center_neighbors_final: &[Vec<usize>],
    center_neighbor_counts_final: &[usize],
    vertex_mapping: &[usize],
) -> io::Result<Vec<Vec<usize>>> {
    if center_neighbor_counts_final.len() < center_neighbors_final.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts_final must cover center_neighbors_final",
        ));
    }

    let mut reindexed = center_neighbors_final.to_vec();
    for center_id in 2..center_neighbors_final.len() {
        let count = center_neighbor_counts_final[center_id];
        if count > center_neighbors_final[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center_id} neighbor count exceeds available row width"),
            ));
        }
        for slot in 0..count {
            let old_vertex_id = center_neighbors_final[center_id][slot];
            let Some(&new_vertex_id) = vertex_mapping.get(old_vertex_id) else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {old_vertex_id}, outside vertex_mapping"
                    ),
                ));
            };
            if new_vertex_id == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("center {center_id} references unmapped vertex {old_vertex_id}"),
                ));
            }
            reindexed[center_id][slot] = new_vertex_id;
        }
    }

    Ok(reindexed)
}

/// Port of `MOD_mask_postproc.F90:Data_Renew`.
///
/// The function compacts active centers (`IsInDmArea_ustr(i)==1`) into a new
/// center-neighbor table, then rebuilds vertex-to-center adjacency.  It
/// deliberately writes the original source center id into `vertex_neighbors_next`
/// to preserve the Fortran branch highlighted by the in-source comment.
pub fn renew_mask_postproc_data_fortran_indexed(
    mode_grid: &str,
    active_centers: &[bool],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    ustr_bounds: usize,
) -> io::Result<MaskPostprocRenewedData> {
    let (center_width, vertex_width) = mask_postproc_neighbor_widths(mode_grid)?;
    if active_centers.len() < center_neighbors.len()
        || center_neighbor_counts.len() < center_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "active_centers and center_neighbor_counts must cover center_neighbors",
        ));
    }

    let points_next = 1 + active_centers
        .iter()
        .take(center_neighbors.len())
        .skip(2)
        .filter(|&&is_active| is_active)
        .count();

    let mut center_neighbors_next = vec![vec![1; center_width]; points_next + 1];
    let mut vertex_neighbors_next = vec![vec![1; vertex_width]; ustr_bounds + 1];
    let mut center_neighbor_counts_next = vec![0; points_next + 1];
    let mut vertex_neighbor_counts_next = vec![0; ustr_bounds + 1];

    let mut compact_center_id = 1;
    for source_center_id in 2..center_neighbors.len() {
        if !active_centers[source_center_id] {
            continue;
        }
        compact_center_id += 1;
        let count = center_neighbor_counts[source_center_id];
        if count > center_width || count > center_neighbors[source_center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {source_center_id} neighbor count {count} exceeds available width"),
            ));
        }

        for (slot, &vertex_id) in center_neighbors[source_center_id]
            .iter()
            .take(center_width)
            .enumerate()
        {
            center_neighbors_next[compact_center_id][slot] = vertex_id;
        }
        center_neighbor_counts_next[compact_center_id] = count;

        for &vertex_id in center_neighbors_next[compact_center_id].iter().take(count) {
            if vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {source_center_id} references vertex {vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            let slot = vertex_neighbor_counts_next[vertex_id];
            if slot >= vertex_width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("vertex {vertex_id} has more than {vertex_width} neighboring centers"),
                ));
            }
            vertex_neighbor_counts_next[vertex_id] += 1;
            vertex_neighbors_next[vertex_id][slot] = source_center_id;
        }
    }

    let mut bounds_next = ustr_bounds;
    for vertex_id in 2..=ustr_bounds {
        if vertex_neighbor_counts_next[vertex_id] == 0 {
            bounds_next -= 1;
        }
    }

    Ok(MaskPostprocRenewedData {
        points_next,
        bounds_next,
        center_neighbors_next,
        vertex_neighbors_next,
        center_neighbor_counts_next,
        vertex_neighbor_counts_next,
    })
}

/// Port of `MOD_mask_postproc.F90:Data_Finial`.
///
/// This is the final placeholder-preserving compaction after domain-mask edits:
/// active centers are copied to compact ids, vertex adjacency is rebuilt using
/// those compact center ids (`k` in the Fortran comment), then only vertices
/// that still have adjacent centers are copied to the final vertex arrays.
pub fn finalize_mask_postproc_data_fortran_indexed(
    mode_grid: &str,
    active_centers: &[bool],
    center_coordinates: &[[f64; 2]],
    vertex_coordinates: &[[f64; 2]],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    ustr_bounds: usize,
) -> io::Result<MaskPostprocFinalData> {
    let (center_width, vertex_width) = mask_postproc_neighbor_widths(mode_grid)?;
    if active_centers.len() < center_neighbors.len()
        || center_coordinates.len() < center_neighbors.len()
        || center_neighbor_counts.len() < center_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "active_centers, center_coordinates, and center_neighbor_counts must cover center_neighbors",
        ));
    }
    if vertex_coordinates.len() <= ustr_bounds {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "vertex_coordinates length {} must cover Fortran vertex ids 1..={ustr_bounds}",
                vertex_coordinates.len()
            ),
        ));
    }

    let points_final = 1 + active_centers
        .iter()
        .take(center_neighbors.len())
        .skip(2)
        .filter(|&&is_active| is_active)
        .count();

    let mut center_coordinates_final = vec![[0.0, 0.0]; points_final + 1];
    let mut center_neighbors_final = vec![vec![1; center_width]; points_final + 1];
    let mut center_neighbor_counts_final = vec![0; points_final + 1];
    let mut vertex_neighbors_work = vec![vec![1; vertex_width]; ustr_bounds + 1];
    let mut vertex_neighbor_counts_work = vec![0; ustr_bounds + 1];

    let mut compact_center_id = 1;
    for source_center_id in 2..center_neighbors.len() {
        if !active_centers[source_center_id] {
            continue;
        }
        compact_center_id += 1;
        let count = center_neighbor_counts[source_center_id];
        if count > center_width || count > center_neighbors[source_center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {source_center_id} neighbor count {count} exceeds available width"),
            ));
        }

        center_coordinates_final[compact_center_id] = center_coordinates[source_center_id];
        for (slot, &vertex_id) in center_neighbors[source_center_id]
            .iter()
            .take(center_width)
            .enumerate()
        {
            center_neighbors_final[compact_center_id][slot] = vertex_id;
        }
        center_neighbor_counts_final[compact_center_id] = count;

        for &vertex_id in center_neighbors_final[compact_center_id].iter().take(count) {
            if vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {source_center_id} references vertex {vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            let slot = vertex_neighbor_counts_work[vertex_id];
            if slot >= vertex_width {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("vertex {vertex_id} has more than {vertex_width} neighboring centers"),
                ));
            }
            vertex_neighbor_counts_work[vertex_id] += 1;
            vertex_neighbors_work[vertex_id][slot] = compact_center_id;
        }
    }

    let bounds_final = 1 + vertex_neighbor_counts_work
        .iter()
        .take(ustr_bounds + 1)
        .skip(2)
        .filter(|&&count| count > 0)
        .count();

    let mut vertex_coordinates_final = vec![[0.0, 0.0]; bounds_final + 1];
    let mut vertex_neighbors_final = vec![vec![1; vertex_width]; bounds_final + 1];
    let mut vertex_neighbor_counts_final = vec![0; bounds_final + 1];

    let mut compact_vertex_id = 1;
    for source_vertex_id in 2..=ustr_bounds {
        if vertex_neighbor_counts_work[source_vertex_id] == 0 {
            continue;
        }
        compact_vertex_id += 1;
        vertex_coordinates_final[compact_vertex_id] = vertex_coordinates[source_vertex_id];
        vertex_neighbors_final[compact_vertex_id] = vertex_neighbors_work[source_vertex_id].clone();
        vertex_neighbor_counts_final[compact_vertex_id] =
            vertex_neighbor_counts_work[source_vertex_id];
    }

    Ok(MaskPostprocFinalData {
        points_final,
        bounds_final,
        center_coordinates_final,
        vertex_coordinates_final,
        center_neighbors_final,
        vertex_neighbors_final,
        center_neighbor_counts_final,
        vertex_neighbor_counts_final,
    })
}

/// Port of `MOD_mask_postproc.F90:IsInDmArea_ustr_Renew`.
///
/// `is_in_domain` mirrors the global Fortran `IsInDmArea_ustr` array: `1` is
/// active/ocean, negative values are inactive/land, and slot `1` is the legacy
/// placeholder.  The routine first removes triangles whose three vertices are
/// all solid-boundary vertices (`n_ustr_ngr == 6`), then applies the legacy
/// one-missing-triangle refill rule and updates `points_new` with the same
/// per-vertex increments/decrements as the Fortran code.
pub fn renew_mask_postproc_domain_triangles_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    points_new: &mut isize,
) -> io::Result<()> {
    if is_in_domain.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "is_in_domain must preserve at least the Fortran placeholder slots",
        ));
    }
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
        || vertex_neighbors_new.len() < vertex_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor tables and count arrays must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len() - 1;
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    let mut solid_boundary_vertex_tally = vec![3usize; is_in_domain.len()];

    for vertex_id in 2..=ustr_bounds {
        let count_new = vertex_neighbor_counts_new[vertex_id];
        let count_original = vertex_neighbor_counts[vertex_id];
        if count_new > vertex_neighbors_new[vertex_id].len()
            || count_original > vertex_neighbors[vertex_id].len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} neighbor count exceeds available row width"),
            ));
        }
        if count_new == 0 || count_new == count_original {
            continue;
        }
        for &center_id in vertex_neighbors_new[vertex_id].iter().take(count_new) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} references center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            solid_boundary_vertex_tally[center_id] += 1;
        }
    }

    for center_id in 2..=ustr_points {
        if is_in_domain[center_id] != 1 {
            continue;
        }
        if solid_boundary_vertex_tally[center_id] == 6 {
            is_in_domain[center_id] = -1;
            *points_new -= 1;
        }
    }

    for vertex_id in 2..=ustr_bounds {
        let count_original = vertex_neighbor_counts[vertex_id];
        let count_new = vertex_neighbor_counts_new[vertex_id];
        if count_original < count_new {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "vertex {vertex_id} renewed count {count_new} exceeds original count {count_original}"
                ),
            ));
        }
        if count_original - count_new != 1 {
            continue;
        }
        for &center_id in vertex_neighbors[vertex_id].iter().take(count_original) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} references center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            is_in_domain[center_id] = 1;
        }
        *points_new += 1;
    }

    Ok(())
}

/// Port of `MOD_mask_postproc.F90:IsInDmArea_ustr_Renew_v2`.
///
/// For vertices with exactly two missing neighboring triangles, the legacy code
/// checks opposite slots (`j` and `j+3`) and refills both when both are
/// currently outside the active domain.
pub fn renew_mask_postproc_opposite_domain_triangles_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    points_new: &mut isize,
) -> io::Result<()> {
    if is_in_domain.len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "is_in_domain must preserve at least the Fortran placeholder slots",
        ));
    }
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor table and count arrays must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len() - 1;
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    for vertex_id in 2..=ustr_bounds {
        let count_original = vertex_neighbor_counts[vertex_id];
        let count_new = vertex_neighbor_counts_new[vertex_id];
        if count_original > vertex_neighbors[vertex_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {vertex_id} neighbor count exceeds available row width"),
            ));
        }
        if count_original < count_new {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "vertex {vertex_id} renewed count {count_new} exceeds original count {count_original}"
                ),
            ));
        }
        if count_original - count_new != 2 {
            continue;
        }
        for slot in 0..count_original.saturating_sub(3) {
            let left_center_id = vertex_neighbors[vertex_id][slot];
            let right_center_id = vertex_neighbors[vertex_id][slot + 3];
            if left_center_id > ustr_points || right_center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {vertex_id} references centers {left_center_id}/{right_center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            if is_in_domain[left_center_id] != 1 && is_in_domain[right_center_id] != 1 {
                is_in_domain[left_center_id] = 1;
                is_in_domain[right_center_id] = 1;
                *points_new += 2;
            }
        }
    }

    Ok(())
}

/// Port of `MOD_mask_postproc.F90:narrow_waterway_widen`.
///
/// The helper builds the temporary boundary vertex-to-vertex graph from compact
/// center rows, detects the legacy four-connection narrow-waterway signature,
/// then activates every original center adjacent to the duplicated neighbor.
pub fn widen_narrow_waterway_fortran_indexed(
    is_in_domain: &mut [i32],
    vertex_neighbors: &[Vec<usize>],
    center_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
    center_neighbor_counts_new: &[usize],
) -> io::Result<()> {
    if vertex_neighbor_counts.len() < vertex_neighbors.len()
        || vertex_neighbor_counts_new.len() < vertex_neighbors.len()
        || center_neighbor_counts_new.len() < center_neighbors_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "neighbor tables and count arrays must have matching Fortran-indexed lengths",
        ));
    }

    let ustr_points = is_in_domain.len().saturating_sub(1);
    let ustr_bounds = vertex_neighbors.len().saturating_sub(1);
    let mut boundary_vertex_neighbors = vec![Vec::<usize>::new(); ustr_bounds + 1];

    for center_id in 2..center_neighbors_new.len() {
        let count = center_neighbor_counts_new[center_id];
        if count > center_neighbors_new[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center_id} neighbor count exceeds available row width"),
            ));
        }
        if count == 0 {
            continue;
        }
        for slot in 0..count {
            let left_vertex_id = center_neighbors_new[center_id][slot];
            if left_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {left_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[left_vertex_id] == vertex_neighbor_counts[left_vertex_id]
            {
                continue;
            }

            let right_vertex_id = center_neighbors_new[center_id][(slot + 1) % count];
            if right_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {right_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[right_vertex_id]
                == vertex_neighbor_counts[right_vertex_id]
            {
                continue;
            }

            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                left_vertex_id,
                right_vertex_id,
            )?;
            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                right_vertex_id,
                left_vertex_id,
            )?;
            break;
        }
    }

    for vertex_id in 2..=ustr_bounds {
        if boundary_vertex_neighbors[vertex_id].len() != 4 {
            continue;
        }
        let Some(duplicated_neighbor) =
            first_duplicate_neighbor(&boundary_vertex_neighbors[vertex_id])
        else {
            continue;
        };
        let count = vertex_neighbor_counts[duplicated_neighbor];
        if count > vertex_neighbors[duplicated_neighbor].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("vertex {duplicated_neighbor} neighbor count exceeds available row width"),
            ));
        }
        for &center_id in vertex_neighbors[duplicated_neighbor].iter().take(count) {
            if center_id > ustr_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "vertex {duplicated_neighbor} references center {center_id}, outside 0..={ustr_points}"
                    ),
                ));
            }
            is_in_domain[center_id] = 1;
        }
    }

    Ok(())
}

fn push_boundary_neighbor(
    boundary_vertex_neighbors: &mut [Vec<usize>],
    vertex_id: usize,
    neighbor_id: usize,
) -> io::Result<()> {
    if boundary_vertex_neighbors[vertex_id].len() >= 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("boundary vertex {vertex_id} has more than four boundary connections"),
        ));
    }
    boundary_vertex_neighbors[vertex_id].push(neighbor_id);
    Ok(())
}

fn first_duplicate_neighbor(neighbors: &[usize]) -> Option<usize> {
    for (left, &left_value) in neighbors.iter().enumerate() {
        for &right_value in neighbors.iter().skip(left + 1) {
            if left_value == right_value {
                return Some(left_value);
            }
        }
    }
    None
}

/// Port of `MOD_mask_postproc.F90:bdy_connection_closed_curve`.
///
/// `boundary_order[0]` and output curve slot `0` are placeholders matching the
/// Fortran convention that useful records start at index `1`/`2` depending on
/// the source array.  `num_bdy_long[0..2]` preserves the legacy final `+1` on
/// longest/second-longest lengths because downstream allocation expects the
/// extra placeholder space.
pub fn boundary_closed_curves_fortran_indexed(
    boundary_order: &[usize],
    boundary_neighbors: &[Vec<usize>],
) -> io::Result<BoundaryClosedCurves> {
    if boundary_order.len() < 2 {
        return Ok(BoundaryClosedCurves {
            num_closed_curve: 0,
            num_bdy_long: [1, 1, 0],
            close_curves: vec![Vec::new()],
            n_close_curve: vec![0],
        });
    }

    let mut boundary_available = vec![true; boundary_order.len()];
    let mut num_bdy_long = [0usize; 3];
    let mut close_curves = vec![Vec::new()];
    let mut n_close_curve = vec![0usize];

    while boundary_available
        .iter()
        .skip(1)
        .any(|&available| available)
    {
        let start_pos = boundary_available
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(pos, &available)| available.then_some(pos))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing boundary start"))?;

        let start_vertex = boundary_order[start_pos];
        require_boundary_neighbor_row(start_vertex, boundary_neighbors)?;
        let mut boundary_queue = vec![start_vertex];
        boundary_available[start_pos] = false;

        let boundary_end = boundary_neighbors[start_vertex][1];
        let mut selected_neighbor = boundary_neighbors[start_vertex][0];
        while selected_neighbor != boundary_end {
            require_boundary_neighbor_row(selected_neighbor, boundary_neighbors)?;
            let previous_vertex = *boundary_queue.last().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "empty boundary queue")
            })?;
            boundary_queue.push(selected_neighbor);
            let selected_pos = boundary_order
                .iter()
                .enumerate()
                .skip(1)
                .find_map(|(pos, &vertex)| (vertex == selected_neighbor).then_some(pos))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary vertex {selected_neighbor} not found in boundary_order"),
                    )
                })?;
            boundary_available[selected_pos] = false;

            selected_neighbor = if boundary_neighbors[selected_neighbor][0] == previous_vertex {
                boundary_neighbors[selected_neighbor][1]
            } else {
                boundary_neighbors[selected_neighbor][0]
            };
        }

        boundary_queue.push(boundary_end);
        let end_pos = boundary_order
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(pos, &vertex)| (vertex == boundary_end).then_some(pos))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary end vertex {boundary_end} not found in boundary_order"),
                )
            })?;
        boundary_available[end_pos] = false;
        if boundary_queue.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "boundary closed curve has fewer than three points",
            ));
        }

        let curve_id = close_curves.len();
        let num_points = boundary_queue.len();
        close_curves.push(boundary_queue);
        n_close_curve.push(num_points);

        if num_points > num_bdy_long[0] {
            num_bdy_long[0] = num_points;
            num_bdy_long[2] = curve_id;
        }
        if curve_id != 1 && num_points > num_bdy_long[1] && num_points < num_bdy_long[0] {
            num_bdy_long[1] = num_points;
        }
    }

    num_bdy_long[0] += 1;
    num_bdy_long[1] += 1;

    Ok(BoundaryClosedCurves {
        num_closed_curve: close_curves.len() - 1,
        num_bdy_long,
        close_curves,
        n_close_curve,
    })
}

/// Pure-data port of `MOD_mask_postproc.F90:bdy_connection`.
///
/// NetCDF writing of `obcv2.nc4` remains an adapter concern; this helper
/// returns the boundary order, the two-neighbor boundary graph, and the
/// closed-curve metadata needed by isolated-ocean removal.
pub fn boundary_connection_fortran_indexed(
    center_neighbors_new: &[Vec<usize>],
    center_neighbor_counts_new: &[usize],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &[usize],
) -> io::Result<BoundaryConnection> {
    if center_neighbor_counts_new.len() < center_neighbors_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts_new must cover center_neighbors_new",
        ));
    }
    if vertex_neighbor_counts_new.len() != vertex_neighbor_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor count arrays must have matching lengths",
        ));
    }

    let ustr_bounds = vertex_neighbor_counts.len().saturating_sub(1);
    let mut boundary_vertex_neighbors = vec![Vec::<usize>::new(); ustr_bounds + 1];

    for center_id in 2..center_neighbors_new.len() {
        let count = center_neighbor_counts_new[center_id];
        if count > center_neighbors_new[center_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("center {center_id} neighbor count exceeds available row width"),
            ));
        }
        if count == 0 {
            continue;
        }

        for slot in 0..count {
            let left_vertex_id = center_neighbors_new[center_id][slot];
            if left_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {left_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[left_vertex_id] == vertex_neighbor_counts[left_vertex_id]
            {
                continue;
            }

            let right_vertex_id = center_neighbors_new[center_id][(slot + 1) % count];
            if right_vertex_id > ustr_bounds {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "center {center_id} references vertex {right_vertex_id}, outside 0..={ustr_bounds}"
                    ),
                ));
            }
            if vertex_neighbor_counts_new[right_vertex_id]
                == vertex_neighbor_counts[right_vertex_id]
            {
                continue;
            }

            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                left_vertex_id,
                right_vertex_id,
            )?;
            push_boundary_neighbor(
                &mut boundary_vertex_neighbors,
                right_vertex_id,
                left_vertex_id,
            )?;
            break;
        }
    }

    let mut boundary_order = vec![1usize];
    let mut boundary_neighbors = vec![vec![1usize, 1usize]; ustr_bounds + 1];
    for vertex_id in 2..=ustr_bounds {
        match boundary_vertex_neighbors[vertex_id].len() {
            0 => {}
            2 => {
                boundary_order.push(vertex_id);
                boundary_neighbors[vertex_id][0] = boundary_vertex_neighbors[vertex_id][0];
                boundary_neighbors[vertex_id][1] = boundary_vertex_neighbors[vertex_id][1];
            }
            count => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary vertex {vertex_id} has {count} connections, expected 0 or 2"),
                ));
            }
        }
    }

    let curves = boundary_closed_curves_fortran_indexed(&boundary_order, &boundary_neighbors)?;
    Ok(BoundaryConnection {
        bdy_num_in: boundary_order.len(),
        boundary_order,
        boundary_neighbors,
        curves,
    })
}

/// Pure-data port of `MOD_mask_postproc.F90:Isolated_Ocean_Renew`.
///
/// The caller supplies the already-built boundary connection so this helper can
/// focus on the legacy closed-curve classification and inward peeling rule.
pub fn remove_isolated_ocean_fortran_indexed(
    is_in_domain: &mut [i32],
    center_neighbors: &[Vec<usize>],
    center_neighbor_counts: &[usize],
    vertex_neighbors_new: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_neighbor_counts_new: &mut [usize],
    boundary: &BoundaryConnection,
) -> io::Result<IsolatedOceanRenewal> {
    if center_neighbor_counts.len() < center_neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "center_neighbor_counts must cover center_neighbors",
        ));
    }
    if vertex_neighbor_counts_new.len() != vertex_neighbor_counts.len()
        || vertex_neighbors_new.len() > vertex_neighbor_counts.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vertex neighbor tables/counts must share a compatible Fortran-indexed domain",
        ));
    }

    let curves = &boundary.curves;
    let longest_curve_id = curves.num_bdy_long[2];
    let mut bdy_long_order = vec![1usize; curves.num_bdy_long[0]];
    if longest_curve_id > 0 {
        let longest_curve = curves.close_curves.get(longest_curve_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("longest curve id {longest_curve_id} is missing"),
            )
        })?;
        if longest_curve.len() + 1 > bdy_long_order.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "longest boundary curve does not fit bdy_long_order",
            ));
        }
        for (offset, &vertex_id) in longest_curve.iter().enumerate() {
            bdy_long_order[offset + 1] = vertex_id;
        }
    }

    let mut close_curves = curves.close_curves.clone();
    let mut n_close_curve = curves.n_close_curve.clone();
    let mut removed_curve_ids = Vec::new();
    for curve_id in 1..=curves.num_closed_curve {
        if curve_id == longest_curve_id {
            continue;
        }
        let curve = curves.close_curves.get(curve_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("closed curve id {curve_id} is missing"),
            )
        })?;
        let mut num_diff = 0isize;
        for &vertex_id in curve {
            require_vertex_count(vertex_id, vertex_neighbor_counts)?;
            num_diff += 2 * vertex_neighbor_counts_new[vertex_id] as isize
                - vertex_neighbor_counts[vertex_id] as isize;
        }
        if num_diff >= 0 {
            continue;
        }

        removed_curve_ids.push(curve_id);
        let mut num_add = 1usize;
        while num_add != 0 {
            let isolated_order = close_curves[curve_id].clone();
            let isolated_count = n_close_curve[curve_id];
            close_curves[curve_id].clear();
            n_close_curve[curve_id] = 0;

            for &boundary_vertex_id in isolated_order.iter().take(isolated_count) {
                let adjacent_center_count = vertex_neighbor_counts_new[boundary_vertex_id];
                vertex_neighbor_counts_new[boundary_vertex_id] = 0;
                let center_row = vertex_neighbors_new
                    .get(boundary_vertex_id)
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("vertex {boundary_vertex_id} missing vertex_neighbors_new row"),
                        )
                    })?;
                if adjacent_center_count > center_row.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "vertex {boundary_vertex_id} renewed count exceeds available row width"
                        ),
                    ));
                }
                for &center_id in center_row.iter().take(adjacent_center_count) {
                    if center_id >= is_in_domain.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "vertex {boundary_vertex_id} references center {center_id}, outside is_in_domain"
                            ),
                        ));
                    }
                    is_in_domain[center_id] = -1;
                    let center_count = *center_neighbor_counts.get(center_id).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} missing neighbor count"),
                        )
                    })?;
                    let center_row = center_neighbors.get(center_id).ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} missing neighbor row"),
                        )
                    })?;
                    if center_count > center_row.len() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("center {center_id} neighbor count exceeds row width"),
                        ));
                    }
                    for &next_boundary_vertex_id in center_row.iter().take(center_count) {
                        require_vertex_count(next_boundary_vertex_id, vertex_neighbor_counts)?;
                        if vertex_neighbor_counts_new[next_boundary_vertex_id]
                            != vertex_neighbor_counts[next_boundary_vertex_id]
                        {
                            continue;
                        }
                        if !close_curves[curve_id].contains(&next_boundary_vertex_id) {
                            close_curves[curve_id].push(next_boundary_vertex_id);
                            n_close_curve[curve_id] += 1;
                        }
                    }
                }
            }

            num_add = n_close_curve[curve_id];
            if num_add == 1 {
                num_add = 0;
            }
        }
    }

    Ok(IsolatedOceanRenewal {
        num_bdy_long: curves.num_bdy_long,
        bdy_long_order,
        removed_curve_ids,
        n_close_curve_after: n_close_curve,
    })
}

/// Pure-data port of `MOD_mask_postproc.F90:bdy_calculation`.
///
/// This helper classifies the retained longest boundary into OBC/IBC order
/// arrays and performs the legacy order rotation. Writing `obc.nc4` is kept in
/// the adapter layer.
pub fn classify_boundary_orders_fortran_indexed(
    num_bdy_long: [usize; 3],
    bdy_long_order: &[usize],
    vertex_neighbors: &[Vec<usize>],
    vertex_neighbor_counts: &[usize],
    vertex_mapping: &[usize],
    is_in_domain: &[i32],
) -> io::Result<BoundaryOrders> {
    let bdy_num = num_bdy_long[0];
    if bdy_num == 0 || bdy_long_order.len() < bdy_num {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bdy_long_order must cover num_bdy_long[0]",
        ));
    }

    let mut bdy_order = bdy_long_order[..bdy_num].to_vec();
    let mut obc_order = vec![1usize; bdy_num];
    let mut ibc_order = vec![1usize; bdy_num];

    for idx in 1..bdy_num {
        let vertex_id = bdy_long_order[idx];
        require_vertex_count(vertex_id, vertex_neighbor_counts)?;
        if vertex_id >= vertex_neighbors.len() || vertex_id >= vertex_mapping.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex_id} is outside vertex tables"),
            ));
        }
        let count = vertex_neighbor_counts[vertex_id];
        if count > vertex_neighbors[vertex_id].len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex_id} neighbor count exceeds row width"),
            ));
        }

        let mut all_adjacent_centers_active = true;
        for &center_id in vertex_neighbors[vertex_id].iter().take(count) {
            if center_id >= is_in_domain.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "boundary vertex {vertex_id} references center {center_id}, outside is_in_domain"
                    ),
                ));
            }
            if is_in_domain[center_id] == -1 {
                all_adjacent_centers_active = false;
                break;
            }
        }

        bdy_order[idx] = vertex_mapping[vertex_id];
        if all_adjacent_centers_active {
            obc_order[idx] = bdy_order[idx];
        } else {
            ibc_order[idx] = bdy_order[idx];
        }
    }

    if bdy_num >= 4 {
        for idx in 2..bdy_num - 1 {
            if obc_order[idx] != 1 && obc_order[idx - 1] == 1 && obc_order[idx + 1] == 1 {
                ibc_order[idx] = obc_order[idx];
                obc_order[idx] = 1;
            }
        }
    }

    let mut rotation_start = None;
    if bdy_num >= 4 {
        for idx in 1..=bdy_num - 3 {
            if obc_order[idx] == 1 {
                continue;
            }
            if obc_order[idx + 1] != 1 && obc_order[idx + 2] == 1 {
                rotate_boundary_order_like_fortran(&mut bdy_order, idx);
                rotate_boundary_order_like_fortran(&mut obc_order, idx);
                rotate_boundary_order_like_fortran(&mut ibc_order, idx);
                rotation_start = Some(idx + 1);
                break;
            }
        }
    }

    Ok(BoundaryOrders {
        bdy_order,
        obc_order,
        ibc_order,
        rotation_start,
    })
}

fn rotate_boundary_order_like_fortran(values: &mut [usize], split_idx: usize) {
    let original = values.to_vec();
    let mut write_idx = 1;
    for &value in original.iter().skip(split_idx + 1) {
        values[write_idx] = value;
        write_idx += 1;
    }
    for &value in original[1..=split_idx].iter().rev() {
        values[write_idx] = value;
        write_idx += 1;
    }
}

fn require_vertex_count(vertex_id: usize, vertex_neighbor_counts: &[usize]) -> io::Result<()> {
    if vertex_id >= vertex_neighbor_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("vertex {vertex_id} is outside vertex count array"),
        ));
    }
    Ok(())
}

fn require_boundary_neighbor_row(
    vertex_id: usize,
    boundary_neighbors: &[Vec<usize>],
) -> io::Result<()> {
    if vertex_id >= boundary_neighbors.len() || boundary_neighbors[vertex_id].len() < 2 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("boundary vertex {vertex_id} must have two neighbor entries"),
        ));
    }
    Ok(())
}

fn mask_postproc_neighbor_widths(mode_grid: &str) -> io::Result<(usize, usize)> {
    match mode_grid {
        "tri" => Ok((3, 7)),
        "hex" => Ok((7, 3)),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported mask_postproc mode_grid {other}"),
        )),
    }
}

/// Port of `MOD_refine.F90:iterB_judge`.
///
/// Inputs preserve Fortran indexing: row 0 is unused, `ngrmm[cell]` contains
/// the three neighboring triangle ids for `cell`, and `mrl_new[cell] == 4`
/// means the triangle has already been one-into-four refined.  The returned
/// `ref_sjx` has the same placeholder-inclusive length as `mrl_new`.
pub fn refine_iter_b_judge_fortran_indexed(
    set_dis_in: usize,
    num_vertex: usize,
    ngrmm: &[Vec<usize>],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterB_judge",
        ));
    }
    if ngrmm.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "ngrmm rows {} must match mrl_new length {}",
                ngrmm.len(),
                mrl_new.len()
            ),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    for (cell, neighbors) in ngrmm.iter().enumerate().skip(num_vertex.saturating_add(1)) {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("ngrmm row {cell} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor == 0 || neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("ngrmm row {cell} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }

    let mut mrl_in = vec![0_i32; mrl_new.len()];
    for i in (num_vertex + 1)..=sjx_points {
        if mrl_new[i] != 4 {
            continue;
        }
        for &neighbor in &ngrmm[i] {
            if mrl_new[neighbor] == 4 {
                continue;
            }
            mrl_in[neighbor] += 2;
        }
    }

    const HHH: [usize; 5] = [0, 1, 2, 0, 1];
    for _ in 1..set_dis_in {
        let mut mrl_bk = mrl_in.clone();
        for i in (num_vertex + 1)..=sjx_points {
            if mrl_new[i] == 4 || mrl_in[i] != 0 {
                continue;
            }
            let neighbors = &ngrmm[i];
            let transition_sum: i32 = neighbors.iter().map(|&neighbor| mrl_in[neighbor]).sum();
            if transition_sum != 4 {
                continue;
            }
            for j in 0..3 {
                let m1 = neighbors[HHH[j]];
                let m2 = neighbors[HHH[j + 1]];
                let m3 = neighbors[HHH[j + 2]];
                if mrl_in[m1] == 2 && mrl_in[m2] == 2 {
                    mrl_bk[i] += 2;
                    mrl_bk[m3] += 2;
                    break;
                }
            }
        }
        mrl_in = mrl_bk;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for i in (num_vertex + 1)..=sjx_points {
        if mrl_new[i] == 4 {
            continue;
        }
        if mrl_in[i] >= 4 {
            ref_sjx[i] = 1;
        }
    }
    Ok(ref_sjx)
}

/// Port of the empty `MOD_refine.F90:orial_vertices_protect` placeholder.
///
/// The Fortran subroutine has no executable statements, so the Rust migration
/// intentionally preserves all caller-owned refinement markers unchanged.
pub fn refine_orial_vertices_protect_fortran_indexed(_ref_sjx: &mut [i32]) {}

/// Port of `MOD_refine.F90:iterG_judge`.
///
/// Inputs preserve Fortran indexing: row 0 is unused, polygon/cell ids start
/// after `num_center`, `triangles_on_cell[cell]` corresponds to
/// `ngrwm(1:n_ngrwm(cell), cell)`, and `mrl_new[triangle] == 1` means the
/// triangle is still unrefined.  A six-edge polygon with refinement-state sum
/// 18 marks its unrefined adjacent triangles as weak-concavity refinements.
pub fn refine_iter_g_judge_fortran_indexed(
    num_center: usize,
    lbx_points: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "lbx_points {lbx_points} must be addressable in triangles_on_cell ({}) and edge_counts ({})",
                triangles_on_cell.len(),
                edge_counts.len()
            ),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    let mut ref_sjx = vec![0_i32; mrl_new.len()];

    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell];
        if num_edges > neighbors.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "cell {cell} edge_count {num_edges} exceeds neighbor row length {}",
                    neighbors.len()
                ),
            ));
        }
        for &triangle in &neighbors[..num_edges] {
            if triangle == 0 || triangle > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} has invalid triangle neighbor {triangle}"),
                ));
            }
        }
        if num_edges != 6 {
            continue;
        }
        let state_sum: i32 = neighbors[..num_edges]
            .iter()
            .map(|&triangle| mrl_new[triangle])
            .sum();
        if state_sum != 18 {
            continue;
        }
        for &triangle in &neighbors[..num_edges] {
            if mrl_new[triangle] == 1 {
                ref_sjx[triangle] = 1;
            }
        }
    }

    Ok(ref_sjx)
}

/// Port of `MOD_refine.F90:iterE_judge`.
///
/// Finds adjacent refined-triangle pairs around polygons (`ngrwm`) that form a
/// convex refinement region.  If either opposite polygon across the pair has a
/// matching convex region, the Fortran routine marks one neighboring triangle
/// in `ref_sjx` to avoid the conflicting convex transition.  Inputs preserve
/// one-based Fortran indexing and placeholder row 0.
pub fn refine_iter_e_judge_fortran_indexed(
    num_center: usize,
    lbx_points: usize,
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    ref_lbx: &[i32],
) -> io::Result<Vec<i32>> {
    if lbx_points >= triangles_on_cell.len()
        || lbx_points >= edge_counts.len()
        || lbx_points >= ref_lbx.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("lbx_points {lbx_points} must be addressable in all cell arrays"),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    let mut lbx_refine1 = vec![0_i32; lbx_points + 1];
    let mut lbx_refine2 = vec![0_usize; lbx_points + 1];
    let mut lbx_refine = vec![[0_usize; 2]; lbx_points + 1];

    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(
            cell,
            triangles_on_cell,
            edge_counts,
            sjx_points,
            Some(cells_on_triangle.len().saturating_sub(1)),
        )?;
        if ref_lbx[cell] == 0 {
            continue;
        }
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if state_sum != num_edges as i32 + 6 {
            continue;
        }
        for pos in 0..num_edges {
            let m1 = neighbors[pos];
            if mrl_new[m1] != 4 {
                continue;
            }
            let m2 = neighbors[(pos + 1) % num_edges];
            if mrl_new[m2] != 4 {
                continue;
            }
            lbx_refine1[cell] = 1;
            lbx_refine2[cell] = pos;
            lbx_refine[cell] = [m1, m2];
        }
    }

    if lbx_refine1.iter().sum::<i32>() == 0 {
        return Ok(vec![0_i32; mrl_new.len()]);
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for cell in (num_center + 1)..=lbx_points {
        if lbx_refine1[cell] == 0 {
            continue;
        }
        let [m1, m2] = lbx_refine[cell];
        let w1 = unique_triangle_cell(m1, m2, cells_on_triangle)?;
        let w2 = unique_triangle_cell(m2, m1, cells_on_triangle)?;
        let w1_refines = w1 <= lbx_points && lbx_refine1[w1] == 1;
        let w2_refines = w2 <= lbx_points && lbx_refine1[w2] == 1;
        if w1_refines || w2_refines {
            let num_edges = edge_counts[cell];
            let pos = lbx_refine2[cell];
            let mark_pos = if w1_refines {
                if pos == 0 {
                    num_edges - 1
                } else {
                    pos - 1
                }
            } else {
                (pos + 2) % num_edges
            };
            let triangle = triangles_on_cell[cell][mark_pos];
            ref_sjx[triangle] = 1;
            if w1_refines {
                lbx_refine1[w1] = 0;
            } else {
                lbx_refine1[w2] = 0;
            }
        }
        lbx_refine1[cell] = 0;
    }

    Ok(ref_sjx)
}

fn validate_refine_cell_neighbors(
    cell: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    sjx_points: usize,
    max_triangle_connectivity: Option<usize>,
) -> io::Result<()> {
    let num_edges = edge_counts[cell];
    let neighbors = &triangles_on_cell[cell];
    if num_edges > neighbors.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "cell {cell} edge_count {num_edges} exceeds neighbor row length {}",
                neighbors.len()
            ),
        ));
    }
    for &triangle in &neighbors[..num_edges] {
        if triangle == 0 || triangle > sjx_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} has invalid triangle neighbor {triangle}"),
            ));
        }
        if let Some(max_triangle_connectivity) = max_triangle_connectivity {
            if triangle > max_triangle_connectivity {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "cell {cell} triangle {triangle} missing cells_on_triangle connectivity"
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn unique_triangle_cell(
    triangle: usize,
    other_triangle: usize,
    cells_on_triangle: &[[usize; 3]],
) -> io::Result<usize> {
    let cells = cells_on_triangle.get(triangle).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangle {triangle} missing cells_on_triangle connectivity"),
        )
    })?;
    let other_cells = cells_on_triangle.get(other_triangle).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangle {other_triangle} missing cells_on_triangle connectivity"),
        )
    })?;
    let mut unique = None;
    for &cell in cells {
        if cell != 0 && !other_cells.contains(&cell) {
            unique = Some(cell);
        }
    }
    unique.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("triangles {triangle} and {other_triangle} do not expose an opposite cell"),
        )
    })
}

/// Port of `MOD_refine.F90:iterF_judge`.
///
/// Builds the protection halo around the original icosahedron vertices
/// (`impent`) using Fortran one-based `ngrwm/n_ngrwm` connectivity.  If a
/// protected region still contains an `mrl_new == 1` triangle, all protected
/// `mrl_new == 0` triangles are marked for refinement.
pub fn refine_iter_f_judge_fortran_indexed(
    num_sjx: usize,
    num_dbx: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    impent: &[usize],
    vertex_protect_layers: usize,
) -> io::Result<Vec<i32>> {
    if num_sjx >= mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_sjx {num_sjx} must be addressable in mrl_new length {}",
                mrl_new.len()
            ),
        ));
    }
    if num_dbx >= triangles_on_cell.len() || num_dbx >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_dbx {num_dbx} must be addressable in cell connectivity arrays"),
        ));
    }
    if impent.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "impent must include at least one protected original vertex cell",
        ));
    }

    for cell in 2..=num_dbx {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, num_sjx, None)?;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for &protected_cell in impent {
        if protected_cell == 0 || protected_cell > num_dbx {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("impent protected cell {protected_cell} is outside 1..={num_dbx}"),
            ));
        }
        if edge_counts[protected_cell] < 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "impent protected cell {protected_cell} must expose at least five triangles"
                ),
            ));
        }

        let mut protected_triangles = vec![0_i32; mrl_new.len()];
        for &triangle in &triangles_on_cell[protected_cell][..5] {
            if triangle == 0 || triangle > num_sjx {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "impent protected cell {protected_cell} has invalid triangle {triangle}"
                    ),
                ));
            }
            protected_triangles[triangle] = 1;
        }

        for _ in 0..vertex_protect_layers {
            let mut boundary_cells = vec![0_i32; edge_counts.len()];
            for cell in 2..=num_dbx {
                let num_edges = edge_counts[cell];
                let count: i32 = triangles_on_cell[cell][..num_edges]
                    .iter()
                    .map(|&triangle| protected_triangles[triangle])
                    .sum();
                if count == 0 || count == num_edges as i32 {
                    continue;
                }
                boundary_cells[cell] = 1;
            }

            for cell in 2..=num_dbx {
                if boundary_cells[cell] != 1 {
                    continue;
                }
                let num_edges = edge_counts[cell];
                for &triangle in &triangles_on_cell[cell][..num_edges] {
                    protected_triangles[triangle] = 1;
                }
            }
        }

        let has_unrefined_one = (2..=num_sjx)
            .any(|triangle| protected_triangles[triangle] == 1 && mrl_new[triangle] == 1);
        if !has_unrefined_one {
            continue;
        }
        for triangle in 2..=num_sjx {
            if protected_triangles[triangle] == 1 && mrl_new[triangle] == 0 {
                ref_sjx[triangle] = 1;
            }
        }
    }

    Ok(ref_sjx)
}

/// Port of `MOD_refine.F90:iterC_judge`.
///
/// Combines weak-concavity cleanup around already-refined polygons with the
/// `ref_lbx_in` transition propagation used to keep 5/6-edge cells from
/// exceeding the seven-edge refinement cap.  Inputs preserve Fortran one-based
/// indexing: row 0 is unused, triangle rows after `num_vertex` contain exactly
/// three `ngrmm` neighbors, and polygon rows after `num_center` use
/// `edge_counts[cell]` entries from `triangles_on_cell[cell]`.
pub fn refine_iter_c_judge_fortran_indexed(
    set_dis_in: usize,
    num_vertex: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
    ref_lbx: &[i32],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterC_judge",
        ));
    }
    if triangle_neighbors.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "triangle neighbor rows {} must match mrl_new length {}",
                triangle_neighbors.len(),
                mrl_new.len()
            ),
        ));
    }
    if lbx_points >= triangles_on_cell.len()
        || lbx_points >= edge_counts.len()
        || lbx_points >= ref_lbx.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("lbx_points {lbx_points} must be addressable in all cell arrays"),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;
    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, sjx_points, None)?;
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];

    for cell in (num_center + 1)..=lbx_points {
        if ref_lbx[cell] == 0 {
            continue;
        }
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if num_edges == 5 {
            if state_sum > 10 {
                for &triangle in neighbors {
                    if mrl_new[triangle] == 1 {
                        ref_sjx[triangle] = 1;
                    }
                }
            }
        } else if num_edges == 6 && state_sum == 12 {
            for pos in 0..3 {
                let refined_a = neighbors[pos];
                let refined_b = neighbors[pos + 3];
                let gap_a = neighbors[pos + 1];
                let gap_b = neighbors[pos + 2];
                if mrl_new[refined_a] == 4
                    && mrl_new[refined_b] == 4
                    && mrl_new[gap_a] == 1
                    && mrl_new[gap_b] == 1
                {
                    ref_sjx[gap_a] = 1;
                    ref_sjx[gap_b] = 1;
                }
            }
        }
    }

    let mut mrl_in = vec![0_i32; mrl_new.len()];
    let mut mrl_bk = vec![0_i32; mrl_new.len()];
    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_new[triangle] != 4 {
            continue;
        }
        for &neighbor in &triangle_neighbors[triangle] {
            if mrl_new[neighbor] == 4 {
                continue;
            }
            mrl_in[neighbor] = 2;
        }
    }

    const HHH: [usize; 5] = [0, 1, 2, 0, 1];
    for _ in 1..set_dis_in {
        mrl_bk.fill(0);
        for triangle in (num_vertex + 1)..=sjx_points {
            if mrl_new[triangle] == 4 || mrl_in[triangle] != 0 {
                continue;
            }
            let neighbors = &triangle_neighbors[triangle];
            let transition_sum: i32 = neighbors.iter().map(|&neighbor| mrl_in[neighbor]).sum();
            if transition_sum != 4 {
                continue;
            }
            for pos in 0..3 {
                let m1 = neighbors[HHH[pos]];
                let m2 = neighbors[HHH[pos + 1]];
                let m3 = neighbors[HHH[pos + 2]];
                if mrl_in[m1] == 2 && mrl_in[m2] == 2 {
                    mrl_bk[triangle] += 2;
                    mrl_bk[m3] += 2;
                    break;
                }
            }
        }
        mrl_in.clone_from(&mrl_bk);
    }

    let mut ref_lbx_in = vec![vec![0_i32; 7]; lbx_points + 1];
    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        for (pos, &triangle) in triangles_on_cell[cell][..num_edges].iter().enumerate() {
            if mrl_bk[triangle] == 0 {
                continue;
            }
            let neighbor_state_sum: i32 = triangle_neighbors[triangle]
                .iter()
                .map(|&neighbor| mrl_new[neighbor])
                .sum();
            if neighbor_state_sum == 6 {
                ref_lbx_in[cell][pos] = 1;
            }
        }
    }

    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        let neighbors = &triangles_on_cell[cell][..num_edges];
        let state_sum: i32 = neighbors.iter().map(|&triangle| mrl_new[triangle]).sum();
        if ref_lbx[cell] != 0 {
            if num_edges == 6 && state_sum == 9 {
                let incoming_count: i32 = ref_lbx_in[cell][..num_edges].iter().sum();
                if 2 + incoming_count > 3 {
                    for &triangle in neighbors {
                        if mrl_new[triangle] == 1 {
                            ref_sjx[triangle] = 1;
                        }
                    }
                }
            }
        } else if num_edges == 5 || num_edges == 6 {
            let mut num_ref_lbx: Vec<f64> = ref_lbx_in[cell][..num_edges]
                .iter()
                .map(|&value| f64::from(value))
                .collect();
            for pos in 0..num_edges {
                let next = (pos + 1) % num_edges;
                if ref_lbx_in[cell][pos] == 1 && ref_lbx_in[cell][next] == 1 {
                    num_ref_lbx[pos] = 0.5;
                    num_ref_lbx[next] = 0.5;
                }
            }
            if num_ref_lbx.iter().sum::<f64>() + num_edges as f64 > 7.0 {
                for (pos, &triangle) in neighbors.iter().enumerate() {
                    if ref_lbx_in[cell][pos] != 0 && mrl_new[triangle] == 1 {
                        ref_sjx[triangle] = 1;
                    }
                }
            }
        }
    }

    Ok(ref_sjx)
}

fn validate_triangle_neighbor_rows(
    num_vertex: usize,
    sjx_points: usize,
    triangle_neighbors: &[Vec<usize>],
) -> io::Result<()> {
    for (triangle, neighbors) in triangle_neighbors
        .iter()
        .enumerate()
        .take(sjx_points + 1)
        .skip(num_vertex + 1)
    {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle neighbor row {triangle} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle neighbor row {triangle} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }
    Ok(())
}

/// Port of `MOD_refine.F90:iterD_judge`.
///
/// Finds weak-concavity boundary segment pairs where one side has one
/// transition triangle and the neighboring side has more than one (`1+n`).
/// Such pairs are marked for extra refinement by setting both boundary
/// triangles in `ref_sjx`.  Inputs preserve Fortran one-based indexing:
/// triangle row 0 is unused, active triangle rows after `num_vertex` have
/// exactly three `triangle_neighbors`, and polygon rows after `num_center`
/// expose `triangles_on_cell[cell][..edge_counts[cell]]`.
pub fn refine_iter_d_judge_fortran_indexed(
    set_dis_in: usize,
    num_vertex: usize,
    sjx_points: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<i32>> {
    if sjx_points >= mrl_new.len()
        || sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sjx_points {sjx_points} must be addressable in all triangle arrays"),
        ));
    }
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("lbx_points {lbx_points} must be addressable in all cell arrays"),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_center {num_center} exceeds lbx_points {lbx_points}"),
        ));
    }

    let mut ref_sjx = vec![0_i32; sjx_points + 1];
    if set_dis_in == 1 {
        return Ok(ref_sjx);
    }
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:iterD_judge",
        ));
    }

    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;
    for cell in (num_center + 1)..=lbx_points {
        validate_refine_cell_neighbors(cell, triangles_on_cell, edge_counts, sjx_points, None)?;
    }

    let closed_curves = refine_boundary_closed_curves_fortran_indexed(
        num_vertex,
        sjx_points,
        num_center,
        lbx_points,
        triangle_neighbors,
        cells_on_triangle,
        mrl_new,
    )?;
    let bdy_refine_segments = refine_boundary_segments_fortran_indexed(
        set_dis_in,
        &closed_curves,
        triangles_on_cell,
        edge_counts,
        mrl_new,
    )?;

    let num_bdy_refine_segment = bdy_refine_segments.len();
    if num_bdy_refine_segment == 0 {
        return Ok(ref_sjx);
    }

    for i in 0..num_bdy_refine_segment {
        let j = (i + 1) % num_bdy_refine_segment;
        let segment_i = &bdy_refine_segments[i];
        let segment_j = &bdy_refine_segments[j];
        if segment_i.is_empty() || segment_j.is_empty() {
            continue;
        }
        let m1 = *segment_i.last().expect("non-empty segment");
        let m2 = segment_j[0];
        if is_ngrmm(cells_on_triangle[m1], cells_on_triangle[m2]).is_none() {
            continue;
        }
        let num_max = segment_i.len().max(segment_j.len());
        let num_min = segment_i.len().min(segment_j.len());
        if num_min == 1 && num_max > 1 {
            ref_sjx[m1] = 1;
            ref_sjx[m2] = 1;
        }
    }

    Ok(ref_sjx)
}

fn refine_boundary_closed_curves_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
    num_center: usize,
    lbx_points: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    mrl_new: &[i32],
) -> io::Result<Vec<Vec<usize>>> {
    let mut boundary_neighbors = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut boundary_triangle_count = 1_usize; // Fortran keeps slot 1 empty.

    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_new[triangle] != 1 {
            continue;
        }
        let neighbor_state_sum: i32 = triangle_neighbors[triangle]
            .iter()
            .map(|&neighbor| mrl_new[neighbor])
            .sum();
        if neighbor_state_sum != 6 {
            continue;
        }
        let refined_neighbor = triangle_neighbors[triangle]
            .iter()
            .copied()
            .find(|&neighbor| mrl_new[neighbor] == 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary triangle {triangle} has no refined neighbor"),
                )
            })?;
        let triangle_cells = cells_on_triangle[triangle];
        let refined_cells = cells_on_triangle[refined_neighbor];
        let unique_pos = triangle_cells
            .iter()
            .position(|cell| !refined_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} does not expose a unique boundary-opposite cell"),
                )
            })?;
        let mut shared_cells = Vec::with_capacity(2);
        for offset in 1..=2 {
            let cell = triangle_cells[(unique_pos + offset) % 3];
            if cell == 0 || cell > lbx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary triangle {triangle} references invalid boundary cell {cell}"),
                ));
            }
            shared_cells.push(cell);
        }
        let w1 = shared_cells[0];
        let w2 = shared_cells[1];
        boundary_neighbors[w1].push(w2);
        boundary_neighbors[w2].push(w1);
        boundary_triangle_count += 1;
    }

    let mut boundary_order = vec![1_usize];
    for (cell, neighbors) in boundary_neighbors
        .iter()
        .enumerate()
        .take(lbx_points + 1)
        .skip(num_center + 1)
    {
        match neighbors.len() {
            0 => {}
            2 => boundary_order.push(cell),
            n => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary cell {cell} has {n} connections; expected 0 or 2"),
                ));
            }
        }
    }
    if boundary_triangle_count != boundary_order.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "boundary triangle count with empty slot {boundary_triangle_count} differs from boundary vertex count {}",
                boundary_order.len()
            ),
        ));
    }

    let mut available = vec![false; boundary_order.len()];
    for item in available.iter_mut().skip(1) {
        *item = true;
    }
    let mut closed_curves = Vec::new();
    while available.iter().skip(1).any(|&value| value) {
        let start_pos = available
            .iter()
            .enumerate()
            .skip(1)
            .find_map(|(idx, &value)| value.then_some(idx))
            .expect("at least one available boundary vertex");
        let start = boundary_order[start_pos];
        available[start_pos] = false;
        let mut curve = vec![start];
        let end = boundary_neighbors[start][1];
        let mut selected = boundary_neighbors[start][0];
        let mut previous = start;
        while selected != end {
            curve.push(selected);
            let selected_pos = boundary_order
                .iter()
                .position(|&cell| cell == selected)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary cell {selected} is not present in boundary order"),
                    )
                })?;
            if !available[selected_pos] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary curve revisits cell {selected} before closure"),
                ));
            }
            available[selected_pos] = false;
            let neighbors = &boundary_neighbors[selected];
            let next = if neighbors[0] == previous {
                neighbors[1]
            } else if neighbors[1] == previous {
                neighbors[0]
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary cell {selected} is not connected back to {previous}"),
                ));
            };
            previous = selected;
            selected = next;
        }
        curve.push(end);
        let end_pos = boundary_order
            .iter()
            .position(|&cell| cell == end)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary end cell {end} is not present in boundary order"),
                )
            })?;
        if !available[end_pos] {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary curve end cell {end} was already consumed"),
            ));
        }
        available[end_pos] = false;
        if curve.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "boundary closed curve must contain at least three cells",
            ));
        }
        closed_curves.push(curve);
    }

    Ok(closed_curves)
}

fn refine_boundary_segments_fortran_indexed(
    set_dis_in: usize,
    closed_curves: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<Vec<Vec<usize>>> {
    let mut segments = Vec::new();
    for curve in closed_curves {
        if curve.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "closed curve must contain at least three cells",
            ));
        }
        let mut closed = curve.clone();
        closed.push(curve[0]);

        let mut segment_start_end = vec![(0_usize, 0_usize); closed.len()];
        if set_dis_in == 1 {
            for j in 0..(closed.len() - 1) {
                segment_start_end[j] = (j, j + 1);
            }
        } else {
            let first_turn = (0..(closed.len() - 1))
                .find(|&idx| {
                    refined_count_around_cell(closed[idx], triangles_on_cell, edge_counts, mrl_new)
                        != 3
                })
                .unwrap_or(0);
            if first_turn != 0 {
                let unique_len = closed.len() - 1;
                let mut rotated = Vec::with_capacity(unique_len + 1);
                rotated.extend_from_slice(&closed[first_turn..unique_len]);
                rotated.extend_from_slice(&closed[..first_turn]);
                rotated.push(rotated[0]);
                closed = rotated;
            }

            let mut start = 0_usize;
            segment_start_end[start].0 = 0;
            for j in 1..(closed.len() - 1) {
                let refined_count =
                    refined_count_around_cell(closed[j], triangles_on_cell, edge_counts, mrl_new);
                if refined_count == 3 {
                    continue;
                }
                segment_start_end[start].1 = j;
                segment_start_end[j].0 = j;
                start = j;
            }
            segment_start_end[start].1 = closed.len() - 1;

            let original_ranges: Vec<(usize, usize)> = segment_start_end
                .iter()
                .copied()
                .filter(|(range_start, range_end)| range_end > range_start)
                .collect();
            segment_start_end.fill((0, 0));
            for (range_start, range_end) in original_ranges {
                let num = range_end - range_start;
                if num <= set_dis_in {
                    segment_start_end[range_start] = (range_start, range_end);
                    continue;
                }
                let mut num_segment = ((num + 1) as f64 / set_dis_in as f64).floor() as usize;
                if (num + 1) % set_dis_in != 0 {
                    num_segment += 1;
                }
                if num % set_dis_in == 0 {
                    num_segment = num_segment.saturating_sub(1);
                }
                num_segment = num_segment.max(1);
                let mut subranges = Vec::with_capacity(num_segment);
                let mut sub_start = range_start;
                for _ in 0..(num_segment - 1) {
                    let sub_end = sub_start + set_dis_in;
                    subranges.push((sub_start, sub_end));
                    sub_start = sub_end;
                }
                subranges.push((sub_start, range_end));
                if set_dis_in >= 3 && subranges.len() >= 2 {
                    let min_len = (set_dis_in + 1) / 2;
                    let last_idx = subranges.len() - 1;
                    let (last_start, last_end) = subranges[last_idx];
                    if last_end - last_start < min_len {
                        let adjusted_start = last_end - min_len;
                        subranges[last_idx].0 = adjusted_start;
                        subranges[last_idx - 1].1 = adjusted_start;
                    }
                }
                for (sub_start, sub_end) in subranges {
                    segment_start_end[sub_start] = (sub_start, sub_end);
                }
            }
        }

        let total_edges: usize = segment_start_end
            .iter()
            .filter_map(|&(start, end)| (end > start).then_some(end - start))
            .sum();
        if total_edges != closed.len() - 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "boundary segment edge total {total_edges} differs from closed curve edge count {}",
                    closed.len() - 1
                ),
            ));
        }

        for (start, end) in segment_start_end {
            if end <= start {
                continue;
            }
            let mut segment = Vec::with_capacity(end - start);
            for k in start..end {
                let cell_a = closed[k];
                let cell_b = closed[k + 1];
                segment.push(common_unrefined_triangle_between_cells(
                    cell_a,
                    cell_b,
                    triangles_on_cell,
                    edge_counts,
                    mrl_new,
                )?);
            }
            segments.push(segment);
        }
    }
    Ok(segments)
}

/// Public pure-data port of `MOD_refine.F90:bdy_refine_segment_make`.
///
/// `closed_curves` are the unique boundary vertices per curve, without the
/// repeated tail slot.  The helper applies the same Fortran rotation rule for
/// `set_dis_in > 1`, splits long straight runs, and returns the unrefined
/// triangle id shared by each adjacent boundary-cell pair.
pub fn refine_boundary_segments_make_fortran_indexed(
    set_dis_in: usize,
    closed_curves: &[Vec<usize>],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<RefineBoundarySegments> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:bdy_refine_segment_make",
        ));
    }
    let bdy_refine_segment = refine_boundary_segments_fortran_indexed(
        set_dis_in,
        closed_curves,
        triangles_on_cell,
        edge_counts,
        mrl_new,
    )?;
    let n_bdy_refine_segment = bdy_refine_segment.iter().map(Vec::len).collect::<Vec<_>>();
    Ok(RefineBoundarySegments {
        num_bdy_refine_segment: bdy_refine_segment.len(),
        bdy_refine_segment,
        n_bdy_refine_segment,
    })
}

/// Public pure-data port of `MOD_refine.F90:weak_concav_segment_make`.
///
/// The input boundary segments are Rust vectors in Fortran traversal order
/// (`bdy_refine_segment[:, i]` becomes one `Vec<usize>`).  Adjacent segment
/// pairs whose boundary triangles are opposite neighbors by `IsNgrmm` are
/// removed from the ordinary boundary segment list and emitted either as
/// singleton weak-concavity pairs (`weak_concav_pair`) or as weak-concavity
/// transition segments (`weak_concav_segment`).
pub fn refine_weak_concav_segment_make_fortran_indexed(
    set_dis_in: usize,
    num_ref_weak_concav: usize,
    cells_on_triangle: &[[usize; 3]],
    bdy_refine_segment: &[Vec<usize>],
) -> io::Result<RefineWeakConcavitySegments> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:weak_concav_segment_make",
        ));
    }

    let mut bdy_refine_segment_next = bdy_refine_segment.to_vec();
    let mut weak_concav_segment = Vec::<Vec<usize>>::new();
    let mut weak_concav_pair = Vec::<[usize; 2]>::new();
    let num_bdy_refine_segment = bdy_refine_segment.len();

    for i in 0..num_bdy_refine_segment {
        let j = (i + 1) % num_bdy_refine_segment;
        if bdy_refine_segment_next[i].is_empty() || bdy_refine_segment_next[j].is_empty() {
            continue;
        }
        let segment_i = &bdy_refine_segment[i];
        let segment_j = &bdy_refine_segment[j];
        if segment_i.is_empty() || segment_j.is_empty() {
            continue;
        }
        let m1 = *segment_i.last().expect("non-empty segment");
        let m2 = segment_j[0];
        if m1 >= cells_on_triangle.len() || m2 >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak-concavity pair references triangles {m1} and {m2} outside cells_on_triangle"),
            ));
        }
        if is_ngrmm(cells_on_triangle[m1], cells_on_triangle[m2]).is_none() {
            continue;
        }

        let len_i = segment_i.len();
        let len_j = segment_j.len();
        let num_max = len_i.max(len_j);
        let num_min = len_i.min(len_j);
        let num_diff = num_max - num_min;

        if num_diff == 0 {
            if len_i == 1 {
                weak_concav_pair.push([m1, 0]);
                weak_concav_pair.push([m2, 0]);
            } else {
                weak_concav_segment.push(segment_i.clone());
                weak_concav_segment.push(segment_j.clone());
            }
            bdy_refine_segment_next[i].clear();
            bdy_refine_segment_next[j].clear();
        } else if num_diff == 1 {
            if num_min < 3 {
                weak_concav_segment.push(vec![m1]);
                weak_concav_segment.push(vec![m2]);
                if num_min == 2 {
                    if len_i > len_j {
                        bdy_refine_segment_next[i].pop();
                    } else if !bdy_refine_segment_next[j].is_empty() {
                        bdy_refine_segment_next[j].remove(0);
                    }
                }
            } else {
                weak_concav_pair.push([m1, 0]);
                weak_concav_pair.push([m2, 0]);
                bdy_refine_segment_next[i].pop();
                if !bdy_refine_segment_next[j].is_empty() {
                    bdy_refine_segment_next[j].remove(0);
                }
            }
        } else if num_min == 1 {
            weak_concav_pair.push([m1, 0]);
            weak_concav_pair.push([m2, 0]);
            bdy_refine_segment_next[i].pop();
            if !bdy_refine_segment_next[j].is_empty() {
                bdy_refine_segment_next[j].remove(0);
            }
        } else {
            let common_len = num_min;
            let weak_i_start = len_i.saturating_sub(common_len);
            weak_concav_segment.push(segment_i[weak_i_start..].to_vec());
            weak_concav_segment.push(segment_j[..common_len].to_vec());
            bdy_refine_segment_next[i].truncate(weak_i_start);
            bdy_refine_segment_next[j] = segment_j[common_len..].to_vec();
        }
    }

    let num_weak_concav_segment = weak_concav_segment.len();
    let num_weak_concav_pair = weak_concav_pair.len();
    let mut all_weak_concav_segment = weak_concav_segment.clone();
    all_weak_concav_segment.extend(weak_concav_pair.iter().map(|pair| vec![pair[0]]));
    let n_weak_concav_segment = all_weak_concav_segment
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>();
    let num_ref_weak_concav = num_ref_weak_concav.max(all_weak_concav_segment.len());
    let n_bdy_refine_segment = bdy_refine_segment_next
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>();

    Ok(RefineWeakConcavitySegments {
        num_ref_weak_concav,
        num_weak_concav_segment,
        num_weak_concav_pair,
        bdy_refine_segment: bdy_refine_segment_next,
        n_bdy_refine_segment,
        weak_concav_segment: all_weak_concav_segment,
        n_weak_concav_segment,
        weak_concav_pair,
    })
}

fn refined_count_around_cell(
    cell: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> i32 {
    let num_edges = edge_counts[cell];
    let state_sum: i32 = triangles_on_cell[cell][..num_edges]
        .iter()
        .map(|&triangle| mrl_new[triangle])
        .sum();
    (state_sum - num_edges as i32) / 3
}

fn common_unrefined_triangle_between_cells(
    cell_a: usize,
    cell_b: usize,
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    mrl_new: &[i32],
) -> io::Result<usize> {
    for &triangle_a in &triangles_on_cell[cell_a][..edge_counts[cell_a]] {
        if mrl_new[triangle_a] == 4 {
            continue;
        }
        for &triangle_b in &triangles_on_cell[cell_b][..edge_counts[cell_b]] {
            if mrl_new[triangle_b] == 4 {
                continue;
            }
            if triangle_a == triangle_b {
                return Ok(triangle_a);
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("cells {cell_a} and {cell_b} do not share an unrefined boundary triangle"),
    ))
}

/// Port of `MOD_refine.F90:OnedivideFour_connection`.
///
/// Applies the refinement marker `ref_sjx` to the current refinement state:
/// each requested, still-unrefined triangle (`mrl_new == 1`) marks its three
/// parent polygon cells in `ref_lbx` and promotes the triangle state to `4`.
/// Inputs and mutable outputs preserve Fortran one-based indexing.
pub fn refine_onedivide_four_connection_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
    cells_on_triangle: &[[usize; 3]],
    ref_sjx: &[i32],
    ref_lbx: &mut [i32],
    mrl_new: &mut [i32],
) -> io::Result<()> {
    if sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx.len()
        || sjx_points >= mrl_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sjx_points {sjx_points} must be addressable in all triangle arrays"),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }

    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx[triangle] == 0 || mrl_new[triangle] != 1 {
            continue;
        }
        for &cell in &cells_on_triangle[triangle] {
            if cell == 0 || cell >= ref_lbx.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid parent cell {cell}"),
                ));
            }
            ref_lbx[cell] = 1;
        }
        mrl_new[triangle] = 4;
    }

    Ok(())
}

/// Port of `MOD_refine.F90:OnedivideFour_renew`.
///
/// For each marked triangle, generates three new polygon-center points on the
/// original triangle edges, four child triangle-center points, and the
/// `ngrmw_new` child connectivity stencil used later by `NGR_RENEW`.
/// `num_mp` and `num_wp` preserve the Fortran iteration-count arrays, so
/// `num_mp[iter - 1]`/`num_wp[iter - 1]` are the previous endpoints and
/// `num_mp[iter]`/`num_wp[iter]` are the required output endpoints.
pub fn refine_onedivide_four_renew_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    cells_on_triangle: &[[usize; 3]],
    ref_sjx_segment: &[i32],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let sjx_points = num_mp[iter - 1];
    if sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx_segment.len()
        || sjx_points >= cells_on_triangle_new.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("previous sjx_points {sjx_points} must be addressable in triangle arrays"),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {} exceeds triangle output storage",
                num_mp[iter]
            ),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_wp[{iter}] {} exceeds cell output storage",
                num_wp[iter]
            ),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds previous sjx_points {sjx_points}"),
        ));
    }

    let mut refed_iter = 0_usize;
    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx_segment[triangle] == 0 {
            continue;
        }
        let parent_cells = cells_on_triangle[triangle];
        for &cell in &parent_cells {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid parent cell {cell}"),
                ));
            }
        }

        let mut parent_points = [
            cell_points[parent_cells[0]],
            cell_points[parent_cells[1]],
            cell_points[parent_cells[2]],
        ];
        let crosses_dateline = parent_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - parent_points
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut parent_points);
        }

        let mut new_cell_points = [
            midpoint_lonlat(parent_points[1], parent_points[2]),
            midpoint_lonlat(parent_points[0], parent_points[2]),
            midpoint_lonlat(parent_points[0], parent_points[1]),
        ];
        let mut new_triangle_points = [
            average_lonlat3(parent_points[0], new_cell_points[1], new_cell_points[2]),
            average_lonlat3(parent_points[1], new_cell_points[0], new_cell_points[2]),
            average_lonlat3(parent_points[2], new_cell_points[0], new_cell_points[1]),
            average_lonlat3(new_cell_points[2], new_cell_points[0], new_cell_points[1]),
        ];
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut new_triangle_points);
            check_crossing_fortran_lonlat(&mut new_cell_points);
        }

        let m0 = num_mp[iter - 1] + refed_iter * 4;
        let w0 = num_wp[iter - 1] + refed_iter * 3;
        if m0 + 4 >= triangle_points.len()
            || m0 + 4 >= cells_on_triangle_new.len()
            || w0 + 3 >= cell_points.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("refined triangle {triangle} exceeds allocated child storage"),
            ));
        }

        triangle_points[(m0 + 1)..=(m0 + 4)].copy_from_slice(&new_triangle_points);
        cell_points[(w0 + 1)..=(w0 + 3)].copy_from_slice(&new_cell_points);

        cells_on_triangle_new[m0 + 1][1] = w0 + 3;
        cells_on_triangle_new[m0 + 1][2] = w0 + 2;
        cells_on_triangle_new[m0 + 2][1] = w0 + 1;
        cells_on_triangle_new[m0 + 2][2] = w0 + 3;
        cells_on_triangle_new[m0 + 3][1] = w0 + 2;
        cells_on_triangle_new[m0 + 3][2] = w0 + 1;
        cells_on_triangle_new[m0 + 4] = [w0 + 1, w0 + 2, w0 + 3];
        for k in 0..3 {
            cells_on_triangle_new[triangle][k] = 1;
            cells_on_triangle_new[m0 + 1 + k][0] = parent_cells[k];
        }

        refed_iter += 1;
    }

    crossline_check_fortran(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}

fn midpoint_lonlat(a: LonLatDegrees, b: LonLatDegrees) -> LonLatDegrees {
    LonLatDegrees::new(
        (a.lon_degrees + b.lon_degrees) / 2.0,
        (a.lat_degrees + b.lat_degrees) / 2.0,
    )
}

fn average_lonlat3(a: LonLatDegrees, b: LonLatDegrees, c: LonLatDegrees) -> LonLatDegrees {
    LonLatDegrees::new(
        (a.lon_degrees + b.lon_degrees + c.lon_degrees) / 3.0,
        (a.lat_degrees + b.lat_degrees + c.lat_degrees) / 3.0,
    )
}

fn check_crossing_fortran_lonlat(points: &mut [LonLatDegrees]) {
    for point in points {
        if point.lon_degrees < 0.0 {
            point.lon_degrees += 180.0;
        } else {
            point.lon_degrees -= 180.0;
        }
    }
}

fn crossline_check_fortran(
    iter: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "crossline_check range exceeds point storage",
        ));
    }
    for point in triangle_points
        .iter_mut()
        .take(num_mp[iter] + 1)
        .skip(num_mp[iter - 1] + 1)
    {
        if point.lon_degrees == -180.0 {
            point.lon_degrees = 180.0;
        }
    }
    for point in cell_points
        .iter_mut()
        .take(num_wp[iter] + 1)
        .skip(num_wp[iter - 1] + 1)
    {
        if point.lon_degrees == -180.0 {
            point.lon_degrees = 180.0;
        }
    }
    Ok(())
}

/// Port of `MOD_refine.F90:ref_sjx_isreverse_judge`.
///
/// For each active boundary/weak-concavity segment, adjacent segment
/// triangles determine the shared neighbor that must be refined by reverse
/// one-into-two.  The segment is rewritten in-place to contain the next round
/// forward one-into-two candidates, preserving Fortran's placeholder `1`
/// behavior.
pub fn refine_isreverse_judge_fortran_indexed(
    set_dis_in: usize,
    num_segment: usize,
    triangle_neighbors: &[Vec<usize>],
    mrl_new: &[i32],
    segments: &mut [Vec<usize>],
    n_segments: &[usize],
) -> io::Result<Vec<i32>> {
    if set_dis_in == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "set_dis_in must be positive like MOD_refine:ref_sjx_isreverse_judge",
        ));
    }
    if num_segment > segments.len() || num_segment > n_segments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_segment {num_segment} exceeds segment arrays"),
        ));
    }
    if triangle_neighbors.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "triangle neighbor rows {} must match mrl_new length {}",
                triangle_neighbors.len(),
                mrl_new.len()
            ),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    for (triangle, neighbors) in triangle_neighbors.iter().enumerate().skip(2) {
        if neighbors.len() != 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle neighbor row {triangle} must contain exactly three neighbors"),
            ));
        }
        for &neighbor in neighbors {
            if neighbor > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle neighbor row {triangle} has invalid neighbor {neighbor}"),
                ));
            }
        }
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for segment_id in 0..num_segment {
        if n_segments[segment_id] == 0 {
            continue;
        }
        if segments[segment_id].len() < set_dis_in {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("segment {segment_id} length is shorter than set_dis_in {set_dis_in}"),
            ));
        }
        let segment_select = segments[segment_id][..set_dis_in].to_vec();
        segments[segment_id][..set_dis_in].fill(1);
        let mut next_segment_pos = 0usize;
        for j in 0..(set_dis_in - 1) {
            if segment_select[j + 1] == 1 {
                break;
            }
            let m0 = segment_select[j];
            let w0 = segment_select[j + 1];
            if m0 == 0 || m0 > sjx_points || w0 == 0 || w0 > sjx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("segment {segment_id} references invalid triangle pair {m0}, {w0}"),
                ));
            }
            let Some(shared_neighbor) = triangle_neighbors[m0]
                .iter()
                .copied()
                .find(|candidate| *candidate > 1 && triangle_neighbors[w0].contains(candidate))
            else {
                break;
            };
            let next_triangle = triangle_neighbors[shared_neighbor]
                .iter()
                .copied()
                .filter(|&candidate| candidate > 1 && mrl_new[candidate] != 4)
                .last();
            let Some(next_triangle) = next_triangle else {
                continue;
            };
            ref_sjx[shared_neighbor] = 1;
            segments[segment_id][next_segment_pos] = next_triangle;
            next_segment_pos += 1;
        }
    }

    Ok(ref_sjx)
}

/// Port of `MOD_refine.F90:OnedivideTwo`.
///
/// Splits each marked transition triangle into two child triangles.  Forward
/// mode chooses the neighboring already-refined triangle (`mrl_new == 4`) to
/// identify the shared edge; reverse mode chooses the neighboring unrefined
/// triangle (`mrl_new == 1`).  The parent triangle connectivity is cleared to
/// Fortran placeholder `1`, child connectivity and `sjx_child` are filled, and
/// dateline-crossing coordinates follow the Fortran `CheckCrossing` and
/// `crossline_check` rules.
pub fn refine_onedivide_two_fortran_indexed(
    iter: usize,
    is_reverse: bool,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    ref_sjx: &[i32],
    mrl_new: &[i32],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle_new: &mut [[usize; 3]],
    sjx_child: &mut [[usize; 2]],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let previous_sjx_points = num_mp[iter - 1];
    let sjx_points = *num_mp
        .get(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "num_mp[1] is required"))?;
    if sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx.len()
        || sjx_points >= mrl_new.len()
        || sjx_points >= sjx_child.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("base sjx_points {sjx_points} must be addressable in triangle arrays"),
        ));
    }
    if previous_sjx_points >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "previous sjx_points {previous_sjx_points} must be addressable in renewed triangle connectivity"
            ),
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_mp[{iter}] {} exceeds triangle output storage",
                num_mp[iter]
            ),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "num_wp[{iter}] {} exceeds cell output storage",
                num_wp[iter]
            ),
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds previous sjx_points {sjx_points}"),
        ));
    }
    validate_triangle_neighbor_rows(num_vertex, sjx_points, triangle_neighbors)?;

    let mut refed_iter = 0_usize;
    for triangle in (num_vertex + 1)..=sjx_points {
        if ref_sjx[triangle] == 0 {
            continue;
        }
        let required_state = if is_reverse { 1 } else { 4 };
        let split_neighbor = triangle_neighbors[triangle]
            .iter()
            .copied()
            .filter(|&neighbor| mrl_new[neighbor] == required_state)
            .last()
            .ok_or_else(|| {
                let neighbor_states: Vec<(usize, i32)> = triangle_neighbors[triangle]
                    .iter()
                    .copied()
                    .map(|neighbor| (neighbor, mrl_new.get(neighbor).copied().unwrap_or_default()))
                    .collect();
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "triangle {triangle} has no {} neighbor for one-into-two split \
                         (iter={iter}, num_mp[1]={sjx_points}, previous_num_mp={previous_sjx_points}, neighbors={neighbor_states:?})",
                        if is_reverse { "unrefined" } else { "refined" }
                    ),
                )
            })?;
        let neighbor_cells = cells_on_triangle[split_neighbor];
        let parent_cells = cells_on_triangle_new[triangle];
        let unique_pos = parent_cells
            .iter()
            .position(|cell| !neighbor_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "triangle {triangle} has no vertex opposite split neighbor {split_neighbor}"
                    ),
                )
            })?;
        let w1 = parent_cells[unique_pos];
        let w2 = parent_cells[(unique_pos + 1) % 3];
        let w3 = parent_cells[(unique_pos + 2) % 3];
        for &cell in &[w1, w2, w3] {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid cell {cell}"),
                ));
            }
        }
        let mut split_points = [cell_points[w1], cell_points[w2], cell_points[w3]];
        let crosses_dateline = split_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - split_points
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut split_points);
        }

        let mut new_cell_point = midpoint_lonlat(split_points[1], split_points[2]);
        let mut child_point_a = average_lonlat3(split_points[0], new_cell_point, split_points[1]);
        let mut child_point_b = average_lonlat3(split_points[0], new_cell_point, split_points[2]);
        let m1 = num_mp[iter - 1] + refed_iter * 2 + 1;
        let m2 = num_mp[iter - 1] + refed_iter * 2 + 2;
        let w4 = num_wp[iter - 1] + refed_iter + 1;
        if m2 >= triangle_points.len()
            || m2 >= cells_on_triangle_new.len()
            || w4 >= cell_points.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle {triangle} exceeds allocated one-into-two child storage"),
            ));
        }

        cells_on_triangle_new[m1] = [w1, w2, w4];
        cells_on_triangle_new[m2] = [w1, w3, w4];
        if crosses_dateline {
            check_crossing_fortran_lonlat(std::slice::from_mut(&mut child_point_a));
            check_crossing_fortran_lonlat(std::slice::from_mut(&mut child_point_b));
            check_crossing_fortran_lonlat(std::slice::from_mut(&mut new_cell_point));
        }
        triangle_points[m1] = child_point_a;
        triangle_points[m2] = child_point_b;
        cell_points[w4] = new_cell_point;
        cells_on_triangle_new[triangle] = [1, 1, 1];
        sjx_child[triangle] = [m1, m2];
        refed_iter += 1;
    }

    crossline_check_fortran(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}

/// Port of `MOD_refine.F90:m1w1_to_m11w11`.
///
/// Given two parent triangles (`m1`, `w1`) and their two recorded children in
/// `sjx_child`, returns the first child pair that shares an edge according to
/// the Fortran `IsNgrmm` test.  Missing child adjacency is represented as
/// `Ok(None)`, matching the modern Fortran optional `found=.false.` path used by
/// weak-concavity LOP handling.
pub fn refine_m1w1_to_m11w11_fortran_indexed(
    m1: usize,
    w1: usize,
    sjx_child: &[[usize; 2]],
    ngrmw_new: &[[usize; 3]],
) -> io::Result<Option<(usize, usize)>> {
    if m1 == 0 || m1 >= sjx_child.len() || w1 == 0 || w1 >= sjx_child.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("parent ids m1={m1}, w1={w1} must address sjx_child"),
        ));
    }

    for &m11 in &sjx_child[m1] {
        for &w11 in &sjx_child[w1] {
            if m11 == 0 || w11 == 0 {
                continue;
            }
            if m11 >= ngrmw_new.len() || w11 >= ngrmw_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("child ids m11={m11}, w11={w11} must address ngrmw_new"),
                ));
            }
            if is_ngrmm(ngrmw_new[w11], ngrmw_new[m11]).is_some() {
                return Ok(Some((m11, w11)));
            }
        }
    }

    Ok(None)
}

/// Port of `MOD_refine.F90:weak_concav_pair_special`.
///
/// Handles weak-concavity pairs whose adjacent segment length is one: records
/// each weak triangle's outward transition triangle, marks that outward triangle
/// for reverse one-into-two refinement, writes the paired weak-concavity segment
/// entry for triangles sharing the paired weak triangle, and defers disjoint
/// neighbors for an `mrl_new=4` renewal after the scan.
pub fn refine_weak_concav_pair_special_fortran_indexed(
    num_weak_concav_pair: usize,
    num_ref_weak_concav: usize,
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    mrl_new: &mut [i32],
    ref_sjx: &mut [i32],
    weak_concav_pair: &mut [[usize; 2]],
    weak_concav_segment: &mut [Vec<usize>],
) -> io::Result<()> {
    if num_weak_concav_pair >= weak_concav_pair.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_weak_concav_pair {num_weak_concav_pair} must address weak_concav_pair"),
        ));
    }
    if num_ref_weak_concav < num_weak_concav_pair {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_ref_weak_concav must be at least num_weak_concav_pair",
        ));
    }

    let mut mrl_renew = vec![1_usize; num_weak_concav_pair + 1];
    for k in 1..=num_weak_concav_pair {
        let m1 = weak_concav_pair[k][0];
        let pair_index = if k % 2 == 0 {
            k.checked_sub(1)
        } else {
            k.checked_add(1)
        }
        .filter(|&idx| idx >= 1 && idx <= num_weak_concav_pair)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak concavity pair {k} has no paired triangle"),
            )
        })?;
        let m2 = weak_concav_pair[pair_index][0];
        if m1 == 0
            || m1 >= triangle_neighbors.len()
            || m1 >= mrl_new.len()
            || m1 >= ref_sjx.len()
            || m1 >= cells_on_triangle.len()
            || m2 == 0
            || m2 >= cells_on_triangle.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak concavity triangles m1={m1}, m2={m2} must address inputs"),
            ));
        }

        let m3 = triangle_neighbors[m1]
            .iter()
            .copied()
            .find(|&neighbor| neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak concavity triangle {m1} has no outward non-refined neighbor"),
                )
            })?;
        if m3 >= triangle_neighbors.len()
            || m3 >= cells_on_triangle.len()
            || m3 >= ref_sjx.len()
            || m3 >= mrl_new.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("outward triangle {m3} must address refinement inputs"),
            ));
        }
        weak_concav_pair[k][1] = m3;
        ref_sjx[m3] = 1;

        for &m4 in &triangle_neighbors[m3] {
            if m4 == 0 || m4 >= mrl_new.len() || m4 >= cells_on_triangle.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("neighbor {m4} of outward triangle {m3} must address inputs"),
                ));
            }
            if mrl_new[m4] == 4 {
                continue;
            }
            let shares_vertex_with_pair = cells_on_triangle[m4]
                .iter()
                .any(|vertex| cells_on_triangle[m2].contains(vertex));
            if shares_vertex_with_pair {
                let segment_id = num_ref_weak_concav - num_weak_concav_pair + k;
                if segment_id >= weak_concav_segment.len()
                    || weak_concav_segment[segment_id].is_empty()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("weak_concav_segment {segment_id} must have a first slot"),
                    ));
                }
                weak_concav_segment[segment_id][0] = m4;
            } else {
                mrl_renew[k] = m4;
            }
        }
    }

    for &triangle in mrl_renew.iter().skip(1) {
        if triangle >= mrl_new.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("deferred renewal triangle {triangle} must address mrl_new"),
            ));
        }
        mrl_new[triangle] = 4;
    }

    Ok(())
}

/// Port of `MOD_refine.F90:sharp_concav_lop_judge`.
///
/// Builds LOP transition segment pairs for sharp-concavity boundary segments.
/// Segment matrices are represented as Fortran-indexed rows (`segment[i][j]`
/// corresponds to Fortran `segment(j, i)`).
pub fn refine_sharp_concav_lop_judge_fortran_indexed(
    num_ref: &mut usize,
    num_bdy_refine_segment: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    ngrmw_new: &[[usize; 3]],
    sjx_child: &[[usize; 2]],
    bdy_refine_segment: &[Vec<usize>],
    bdy_refine_segment_old: &[Vec<usize>],
    _n_bdy_refine_segment: &[usize],
    ref_sjx_segment_temp: &mut [Vec<usize>],
    n_ref_sjx_segment_temp: &mut [usize],
) -> io::Result<()> {
    if num_bdy_refine_segment >= bdy_refine_segment.len()
        || num_bdy_refine_segment >= bdy_refine_segment_old.len()
        || num_bdy_refine_segment >= ref_sjx_segment_temp.len()
        || num_bdy_refine_segment >= n_ref_sjx_segment_temp.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_bdy_refine_segment must address sharp-concavity segment arrays",
        ));
    }

    for segment_id in 1..=num_bdy_refine_segment {
        let tran_degree = n_ref_sjx_segment_temp[segment_id] + 1;
        if tran_degree == 1 {
            continue;
        }
        if bdy_refine_segment[segment_id].len() <= tran_degree - 1
            || bdy_refine_segment_old[segment_id].len() <= tran_degree
            || ref_sjx_segment_temp[segment_id].len() <= 4 * (tran_degree - 1)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("segment {segment_id} storage is shorter than tran_degree {tran_degree}"),
            ));
        }

        let mut valid_pairs = 0_usize;
        for j in 1..=(tran_degree - 1) {
            let m1 = bdy_refine_segment_old[segment_id][j];
            let w0 = bdy_refine_segment[segment_id][j];
            let m2 = bdy_refine_segment_old[segment_id][j + 1];
            if m1 <= 1 || w0 <= 1 || m2 <= 1 {
                break;
            }
            if w0 == 0 || w0 >= triangle_neighbors.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("segment {segment_id} w0={w0} must address triangle_neighbors"),
                ));
            }
            let Some(w1) = triangle_neighbors[w0].iter().copied().find(|&neighbor| {
                neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 1
            }) else {
                break;
            };

            let Some((m11, w11)) =
                refine_m1w1_to_m11w11_fortran_indexed(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };

            if w1 >= sjx_child.len() || m2 >= sjx_child.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("w1={w1} and m2={m2} must address sjx_child"),
                ));
            }
            let mut w22 = sjx_child[w1][0];
            if w22 == w11 {
                w22 = sjx_child[w1][1];
            }
            if w22 == 0 || w22 >= ngrmw_new.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("w22={w22} must address child connectivity"),
                ));
            }
            let m22 = sjx_child[m2]
                .iter()
                .copied()
                .filter(|&candidate| candidate > 0 && candidate < ngrmw_new.len())
                .find(|&candidate| is_ngrmm(ngrmw_new[w22], ngrmw_new[candidate]).is_some())
                .unwrap_or(0);
            if m22 == 0 {
                continue;
            }

            valid_pairs += 1;
            let out = 4 * valid_pairs - 3;
            ref_sjx_segment_temp[segment_id][out] = m11;
            ref_sjx_segment_temp[segment_id][out + 1] = w11;
            ref_sjx_segment_temp[segment_id][out + 2] = w22;
            ref_sjx_segment_temp[segment_id][out + 3] = m22;
        }

        let effective_tran_degree = valid_pairs + 1;
        if effective_tran_degree == 1 {
            n_ref_sjx_segment_temp[segment_id] = 0;
            continue;
        }
        let num_end = 4 * valid_pairs;
        n_ref_sjx_segment_temp[segment_id] = (effective_tran_degree / 2) * 4;
        *num_ref += n_ref_sjx_segment_temp[segment_id];
        if effective_tran_degree == 2 {
            continue;
        }
        for k in (1..=n_ref_sjx_segment_temp[segment_id]).step_by(4) {
            let src = num_end.checked_sub(k).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid sharp-concavity mirror source",
                )
            })?;
            ref_sjx_segment_temp[segment_id][k + 2] = ref_sjx_segment_temp[segment_id][src];
            ref_sjx_segment_temp[segment_id][k + 3] = ref_sjx_segment_temp[segment_id][src + 1];
        }
    }

    Ok(())
}

/// Port of `MOD_refine.F90:weak_concav_lop_judge`.
///
/// Weak-concavity segment matrices use zero-based inner slots for the Fortran
/// first dimension (`weak_concav_segment[i][0]` is Fortran
/// `weak_concav_segment(1, i)`), matching `weak_concav_pair_special` output.
/// `ref_sjx_segment_temp` remains one-based in the inner slot to match the LOP
/// segment consumers and `sharp_concav_lop_judge`.
pub fn refine_weak_concav_lop_judge_fortran_indexed(
    num_ref: &mut usize,
    num_bdy_refine_segment: usize,
    num_ref_weak_concav: usize,
    num_weak_concav_segment: usize,
    num_weak_concav_pair: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    ngrmw_new: &[[usize; 3]],
    sjx_child: &[[usize; 2]],
    weak_concav_segment: &mut [Vec<usize>],
    weak_concav_segment_old: &[Vec<usize>],
    n_weak_concav_segment: &[usize],
    weak_concav_pair: &[[usize; 2]],
    ref_sjx_segment_temp: &mut [Vec<usize>],
    n_ref_sjx_segment_temp: &mut [usize],
) -> io::Result<()> {
    if num_weak_concav_pair >= weak_concav_pair.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_weak_concav_pair must address weak_concav_pair",
        ));
    }

    let num_end = if num_weak_concav_pair != 0 {
        for pair_id in 1..=num_weak_concav_pair {
            let [m1, w1] = weak_concav_pair[pair_id];
            let Some((m11, w11)) =
                refine_m1w1_to_m11w11_fortran_indexed(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };
            let segment_id = num_bdy_refine_segment + num_weak_concav_segment + pair_id;
            if segment_id >= n_ref_sjx_segment_temp.len()
                || segment_id >= ref_sjx_segment_temp.len()
                || ref_sjx_segment_temp[segment_id].len() <= 2
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("pair segment {segment_id} must address ref_sjx_segment_temp"),
                ));
            }
            n_ref_sjx_segment_temp[segment_id] = 2;
            *num_ref += 2;
            ref_sjx_segment_temp[segment_id][1] = m11;
            ref_sjx_segment_temp[segment_id][2] = w11;
        }
        num_weak_concav_segment
    } else {
        num_ref_weak_concav
    };

    if num_weak_concav_segment == 0 {
        return Ok(());
    }
    if num_end >= weak_concav_segment.len()
        || num_end >= weak_concav_segment_old.len()
        || num_end >= n_weak_concav_segment.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "weak concavity segment counts must address segment arrays",
        ));
    }

    for segment_id_weak in 1..=num_end {
        if weak_concav_segment[segment_id_weak].is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("weak_concav_segment {segment_id_weak} must have a first slot"),
            ));
        }
        if weak_concav_segment[segment_id_weak][0] == 1 {
            continue;
        }
        let segment_id = segment_id_weak + num_bdy_refine_segment;
        if segment_id >= ref_sjx_segment_temp.len() || segment_id >= n_ref_sjx_segment_temp.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP segment {segment_id} must address ref_sjx_segment_temp"),
            ));
        }
        let mut kk = 0_usize;
        let n_segment = n_weak_concav_segment[segment_id_weak];

        if segment_id_weak % 2 != 0 {
            if segment_id_weak + 1 >= weak_concav_segment_old.len()
                || weak_concav_segment_old[segment_id_weak].len() <= n_segment
                || weak_concav_segment_old[segment_id_weak + 1].is_empty()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("odd weak segment {segment_id_weak} lacks intersegment old endpoints"),
                ));
            }
            let m1 = weak_concav_segment_old[segment_id_weak][n_segment];
            let w1 = weak_concav_segment_old[segment_id_weak + 1][0];
            if let Some((m11, w11)) =
                refine_m1w1_to_m11w11_fortran_indexed(m1, w1, sjx_child, ngrmw_new)?
            {
                if ref_sjx_segment_temp[segment_id].len() <= kk + 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("LOP segment {segment_id} lacks intersegment output slots"),
                    ));
                }
                n_ref_sjx_segment_temp[segment_id] = 2;
                *num_ref += 2;
                ref_sjx_segment_temp[segment_id][kk + 1] = m11;
                ref_sjx_segment_temp[segment_id][kk + 2] = w11;
                kk += 2;
            } else {
                continue;
            }
            if n_segment == 0 {
                for offset in 0..=1 {
                    let row = segment_id_weak + offset;
                    if row < weak_concav_segment.len() {
                        for value in &mut weak_concav_segment[row] {
                            *value = 1;
                        }
                    }
                }
                continue;
            }
        }

        for j in 1..=n_segment {
            let old_slot = if segment_id_weak % 2 == 0 { j } else { j - 1 };
            if weak_concav_segment_old[segment_id_weak].len() <= old_slot
                || weak_concav_segment[segment_id_weak].len() < j
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak segment {segment_id_weak} lacks internal slot {j}"),
                ));
            }
            let m1 = weak_concav_segment_old[segment_id_weak][old_slot];
            let w0 = weak_concav_segment[segment_id_weak][j - 1];
            if w0 == 0 || w0 >= triangle_neighbors.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("weak segment {segment_id_weak} w0={w0} must address neighbors"),
                ));
            }
            let w1 = triangle_neighbors[w0]
                .iter()
                .copied()
                .find(|&neighbor| {
                    neighbor > 0 && neighbor < mrl_new.len() && mrl_new[neighbor] != 1
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "weak segment {segment_id_weak} w0={w0} has no reverse split neighbor"
                        ),
                    )
                })?;
            let Some((m11, w11)) =
                refine_m1w1_to_m11w11_fortran_indexed(m1, w1, sjx_child, ngrmw_new)?
            else {
                continue;
            };
            if ref_sjx_segment_temp[segment_id].len() <= kk + 2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOP segment {segment_id} lacks internal output slots"),
                ));
            }
            n_ref_sjx_segment_temp[segment_id] += 2;
            *num_ref += 2;
            ref_sjx_segment_temp[segment_id][kk + 1] = m11;
            ref_sjx_segment_temp[segment_id][kk + 2] = w11;
            kk += 2;
        }
    }

    Ok(())
}

/// Port of `MOD_refine.F90:Delaunay_Lop`.
///
/// Applies diagonal flips for adjacent triangle pairs listed in a one-based
/// `ref_sjx_segment` array, writes replacement triangles after
/// `num_mp[iter-1]`, clears old triangle connectivity to Fortran placeholder
/// `1`, and preserves the Fortran dateline/crossline cleanup behavior.
pub fn refine_delaunay_lop_fortran_indexed(
    iter: usize,
    num_ref: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points: &mut [LonLatDegrees],
    cell_points: &mut [LonLatDegrees],
    cells_on_triangle: &mut [[usize; 3]],
    ref_sjx_segment: &[usize],
) -> io::Result<()> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    if num_ref >= ref_sjx_segment.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_ref must address one-based ref_sjx_segment entries",
        ));
    }
    if num_mp[iter] >= triangle_points.len() || num_mp[iter] >= cells_on_triangle.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_mp[{iter}] {} exceeds triangle storage", num_mp[iter]),
        ));
    }
    if num_wp[iter] >= cell_points.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_wp[{iter}] {} exceeds cell storage", num_wp[iter]),
        ));
    }

    let mut refed_iter = 0_usize;
    for k in 1..=(num_ref / 2) {
        let i = ref_sjx_segment[2 * k - 1];
        let j = ref_sjx_segment[2 * k];
        if i == 0 || j == 0 {
            continue;
        }
        if i >= cells_on_triangle.len() || j >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP pair i={i}, j={j} must address triangle connectivity"),
            ));
        }
        let tri_i = cells_on_triangle[i];
        let tri_j = cells_on_triangle[j];
        let w1 = tri_i
            .iter()
            .copied()
            .find(|vertex| !tri_j.contains(vertex))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} has no vertex opposite {j}"),
                )
            })?;
        let w2 = tri_i
            .iter()
            .copied()
            .find(|&vertex| vertex != w1)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} lacks a first shared vertex"),
                )
            })?;
        let w4 = tri_i
            .iter()
            .copied()
            .find(|&vertex| vertex != w1 && vertex != w2)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {i} lacks a second shared vertex"),
                )
            })?;
        let w3 = tri_j
            .iter()
            .copied()
            .find(|vertex| !tri_i.contains(vertex))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {j} has no vertex opposite {i}"),
                )
            })?;
        for &cell in &[w1, w2, w3, w4] {
            if cell == 0 || cell >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("LOP pair i={i}, j={j} references invalid cell {cell}"),
                ));
            }
        }

        let m1 = num_mp[iter - 1] + refed_iter * 2 + 1;
        let m2 = num_mp[iter - 1] + refed_iter * 2 + 2;
        if m2 >= triangle_points.len() || m2 >= cells_on_triangle.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("LOP output m2={m2} exceeds triangle storage"),
            ));
        }

        cells_on_triangle[m1] = [w1, w2, w3];
        cells_on_triangle[m2] = [w1, w4, w3];

        let mut quad_points = [
            cell_points[w1],
            cell_points[w2],
            cell_points[w3],
            cell_points[w4],
        ];
        let crosses_dateline = quad_points
            .iter()
            .map(|point| point.lon_degrees)
            .fold(f64::NEG_INFINITY, f64::max)
            - quad_points
                .iter()
                .map(|point| point.lon_degrees)
                .fold(f64::INFINITY, f64::min)
            > 180.0;
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut quad_points);
        }
        let mut new_triangles = [
            average_lonlat3(quad_points[0], quad_points[1], quad_points[2]),
            average_lonlat3(quad_points[0], quad_points[3], quad_points[2]),
        ];
        if crosses_dateline {
            check_crossing_fortran_lonlat(&mut new_triangles);
        }
        triangle_points[m1] = new_triangles[0];
        triangle_points[m2] = new_triangles[1];
        cells_on_triangle[i] = [1, 1, 1];
        cells_on_triangle[j] = [1, 1, 1];
        refed_iter += 1;
    }

    crossline_check_fortran(iter, num_mp, num_wp, triangle_points, cell_points)?;

    Ok(())
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefineNgrRenewCore {
    pub num_sjx: usize,
    pub num_dbx: usize,
    pub triangle_points: Vec<LonLatDegrees>,
    pub cell_points: Vec<LonLatDegrees>,
    pub cells_on_triangle: Vec<[usize; 3]>,
    pub triangles_on_cell: Vec<Vec<usize>>,
    pub n_triangles_on_cell: Vec<usize>,
    pub boundary_refine: Vec<usize>,
    pub boundary_refine_transition: Vec<usize>,
    pub vertex_mapping: Vec<usize>,
}

/// Pure Rust core for `MOD_refine.F90:NGR_RENEW` before `GetSortNew` and file IO.
///
/// This preserves the Fortran-indexed data model: slot `0` is a placeholder,
/// original vertices `1..=num_wp[1]` are copied directly, new vertices are
/// deduplicated only against previously accepted new vertices, deleted
/// triangles have connectivity `[1, 1, 1]`, and triangle-to-vertex adjacency is
/// rebuilt from final compacted triangle ids starting at triangle `2`.
pub fn refine_ngr_renew_core_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points_new: &[LonLatDegrees],
    cell_points_new: &[LonLatDegrees],
    cells_on_triangle_new: &[[usize; 3]],
    boundary_refine: &[usize],
    boundary_refine_transition: &[usize],
) -> io::Result<RefineNgrRenewCore> {
    if iter == 0 || iter >= num_mp.len() || iter >= num_wp.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("iter {iter} must address num_mp/num_wp previous and current slots"),
        ));
    }
    let original_wp = num_wp[1];
    let final_wp = num_wp[iter];
    let final_mp = num_mp[iter];
    if original_wp > final_wp || final_wp >= cell_points_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_wp bounds must address cell_points_new",
        ));
    }
    if final_mp >= triangle_points_new.len() || final_mp >= cells_on_triangle_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_mp bounds must address triangle inputs",
        ));
    }
    if num_vertex > final_mp {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_vertex must not exceed final triangle count",
        ));
    }

    let mut vertex_mapping = vec![0_usize; final_wp + 1];
    let mut cell_points = vec![LonLatDegrees::new(9999.0, 9999.0); final_wp + 1];
    let mut num_dbx = original_wp;
    cell_points[1..=original_wp].copy_from_slice(&cell_points_new[1..=original_wp]);
    for (idx, mapping) in vertex_mapping
        .iter_mut()
        .enumerate()
        .take(original_wp + 1)
        .skip(1)
    {
        *mapping = idx;
    }

    for source_vertex in (original_wp + 1)..=final_wp {
        let duplicate = ((original_wp + 1)..=num_dbx).find(|&candidate| {
            cell_points[candidate].lon_degrees == cell_points_new[source_vertex].lon_degrees
                && cell_points[candidate].lat_degrees == cell_points_new[source_vertex].lat_degrees
        });
        if let Some(mapped) = duplicate {
            vertex_mapping[source_vertex] = mapped;
        } else {
            num_dbx += 1;
            if num_dbx >= cell_points.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "deduplicated vertex count exceeds allocated final cell storage",
                ));
            }
            cell_points[num_dbx] = cell_points_new[source_vertex];
            vertex_mapping[source_vertex] = num_dbx;
        }
    }
    cell_points.truncate(num_dbx + 1);
    let max_mapping = vertex_mapping.iter().copied().max().unwrap_or(0);
    if max_mapping != num_dbx {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max vertex_mapping does not match deduplicated vertex count",
        ));
    }

    let deleted_triangles = ((num_vertex + 1)..=final_mp)
        .filter(|&triangle| cells_on_triangle_new[triangle][0] == 1)
        .count();
    let num_sjx = final_mp - deleted_triangles;
    let mut triangle_points = vec![LonLatDegrees::new(0.0, 0.0); num_sjx + 1];
    let mut cells_on_triangle = vec![[1_usize, 1, 1]; num_sjx + 1];
    triangle_points[1..=num_vertex].copy_from_slice(&triangle_points_new[1..=num_vertex]);
    cells_on_triangle[1..=num_vertex].copy_from_slice(&cells_on_triangle_new[1..=num_vertex]);

    let mut out_triangle = num_vertex;
    for source_triangle in (num_vertex + 1)..=final_mp {
        if cells_on_triangle_new[source_triangle][0] == 1 {
            continue;
        }
        out_triangle += 1;
        triangle_points[out_triangle] = triangle_points_new[source_triangle];
        cells_on_triangle[out_triangle] = cells_on_triangle_new[source_triangle];
    }
    if out_triangle != num_sjx {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "compacted triangle count does not match expected num_sjx",
        ));
    }

    let mut n_triangles_on_cell = vec![0_usize; num_dbx + 1];
    for tri_cells in cells_on_triangle.iter_mut().take(num_sjx + 1).skip(2) {
        for cell in tri_cells.iter_mut() {
            if *cell == 0 || *cell >= vertex_mapping.len() || vertex_mapping[*cell] == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle references cell {cell} without final vertex mapping"),
                ));
            }
            *cell = vertex_mapping[*cell];
            n_triangles_on_cell[*cell] += 1;
        }
    }

    let mut triangles_on_cell = vec![Vec::<usize>::new(); num_dbx + 1];
    for (triangle, tri_cells) in cells_on_triangle
        .iter()
        .enumerate()
        .take(num_sjx + 1)
        .skip(2)
    {
        for &cell in tri_cells {
            triangles_on_cell[cell].push(triangle);
        }
    }

    let remap_boundary = |values: &[usize], vertex_mapping: &[usize]| -> io::Result<Vec<usize>> {
        values
            .iter()
            .map(|&value| {
                if value == 0 || value >= vertex_mapping.len() || vertex_mapping[value] == 0 {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("boundary vertex {value} has no final mapping"),
                    ))
                } else {
                    Ok(vertex_mapping[value])
                }
            })
            .collect()
    };

    Ok(RefineNgrRenewCore {
        num_sjx,
        num_dbx,
        triangle_points,
        cell_points,
        cells_on_triangle,
        triangles_on_cell,
        n_triangles_on_cell,
        boundary_refine: remap_boundary(boundary_refine, &vertex_mapping)?,
        boundary_refine_transition: remap_boundary(boundary_refine_transition, &vertex_mapping)?,
        vertex_mapping,
    })
}

/// File-I/O-free port of `MOD_refine.F90:NGR_RENEW` including the final
/// `GetSortNew` adjacency ordering pass.
pub fn refine_ngr_renew_fortran_indexed(
    iter: usize,
    num_vertex: usize,
    num_mp: &[usize],
    num_wp: &[usize],
    triangle_points_new: &[LonLatDegrees],
    cell_points_new: &[LonLatDegrees],
    cells_on_triangle_new: &[[usize; 3]],
    boundary_refine: &[usize],
    boundary_refine_transition: &[usize],
) -> io::Result<RefineNgrRenewCore> {
    let mut renewed = refine_ngr_renew_core_fortran_indexed(
        iter,
        num_vertex,
        num_mp,
        num_wp,
        triangle_points_new,
        cell_points_new,
        cells_on_triangle_new,
        boundary_refine,
        boundary_refine_transition,
    )?;
    get_sort_new_fortran_indexed(
        renewed.num_dbx,
        &renewed.n_triangles_on_cell,
        &renewed.cells_on_triangle,
        &renewed.triangle_points,
        &mut renewed.triangles_on_cell,
    )?;
    Ok(renewed)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefineArrayLengthHalo {
    pub expanded_mrl: Vec<i32>,
    pub initial_boundary_mask: Vec<i32>,
    pub transition_boundary_mask: Vec<i32>,
    pub boundary_refine: Vec<usize>,
    pub boundary_refine_transition: Vec<usize>,
    pub num_transition_row_triangles: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RefineArrayLengthCalculation {
    pub halo: RefineArrayLengthHalo,
    pub boundary: BoundaryConnection,
}

/// Pure halo-sizing core of `MOD_refine.F90:Array_length_calculation`.
///
/// This excludes `bdy_connection_make` close-curve generation and NetCDF side
/// effects, but preserves the Fortran boundary criterion and outward halo
/// expansion that updates `num_tranrow_sjx`, `isbdy_refine`, `bdy_refine`, and
/// `bdy_refine_tran`.
pub fn refine_array_length_halo_fortran_indexed(
    set_dis_in: usize,
    num_center: usize,
    _sjx_points: usize,
    lbx_points: usize,
    mrl_new: &[i32],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    initial_num_transition_row_triangles: usize,
) -> io::Result<RefineArrayLengthHalo> {
    if lbx_points >= triangles_on_cell.len() || lbx_points >= edge_counts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lbx_points must address triangles_on_cell and edge_counts",
        ));
    }
    if num_center > lbx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_center must not exceed lbx_points",
        ));
    }
    let mut expanded_mrl = mrl_new.to_vec();
    let mut boundary_mask = refine_boundary_mask_from_mrl(
        num_center,
        lbx_points,
        &expanded_mrl,
        triangles_on_cell,
        edge_counts,
    )?;
    let initial_boundary_mask = boundary_mask.clone();
    let mut num_transition_row_triangles = initial_num_transition_row_triangles;

    for _ in 0..set_dis_in {
        for cell in (num_center + 1)..=lbx_points {
            if boundary_mask[cell] != 1 {
                continue;
            }
            let num_edges = edge_counts[cell];
            for &triangle in triangles_on_cell[cell].iter().take(num_edges) {
                if triangle == 0 || triangle >= expanded_mrl.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("cell {cell} references invalid triangle {triangle}"),
                    ));
                }
                if expanded_mrl[triangle] == 4 {
                    continue;
                }
                expanded_mrl[triangle] = 4;
                num_transition_row_triangles += 1;
            }
        }
        boundary_mask = refine_boundary_mask_from_mrl(
            num_center,
            lbx_points,
            &expanded_mrl,
            triangles_on_cell,
            edge_counts,
        )?;
    }

    let boundary_refine = ((num_center + 1)..=lbx_points)
        .filter(|&cell| initial_boundary_mask[cell] == 1)
        .collect::<Vec<_>>();
    let boundary_refine_transition = ((num_center + 1)..=lbx_points)
        .filter(|&cell| boundary_mask[cell] == 1)
        .collect::<Vec<_>>();

    Ok(RefineArrayLengthHalo {
        expanded_mrl,
        initial_boundary_mask,
        transition_boundary_mask: boundary_mask,
        boundary_refine,
        boundary_refine_transition,
        num_transition_row_triangles,
    })
}

/// File-I/O-free wrapper for `MOD_refine.F90:Array_length_calculation`.
///
/// This composes the already migrated halo sizing with
/// `bdy_connection_make` close-curve construction.  The Fortran
/// `close_Mesh_Save` NetCDF writes remain an adapter concern; callers can use
/// `boundary.curves.close_curves` plus their coordinate table to write the same
/// files.
pub fn refine_array_length_calculation_fortran_indexed(
    set_dis_in: usize,
    num_vertex: usize,
    num_center: usize,
    sjx_points: usize,
    lbx_points: usize,
    mrl_new: &[i32],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
    initial_num_transition_row_triangles: usize,
) -> io::Result<RefineArrayLengthCalculation> {
    let halo = refine_array_length_halo_fortran_indexed(
        set_dis_in,
        num_center,
        sjx_points,
        lbx_points,
        mrl_new,
        triangles_on_cell,
        edge_counts,
        initial_num_transition_row_triangles,
    )?;
    let boundary = refine_boundary_connection_make_fortran_indexed(
        num_vertex,
        sjx_points,
        lbx_points,
        mrl_new,
        triangle_neighbors,
        cells_on_triangle,
    )?;
    Ok(RefineArrayLengthCalculation { halo, boundary })
}

fn refine_boundary_mask_from_mrl(
    num_center: usize,
    lbx_points: usize,
    mrl: &[i32],
    triangles_on_cell: &[Vec<usize>],
    edge_counts: &[usize],
) -> io::Result<Vec<i32>> {
    let mut mask = vec![0_i32; lbx_points + 1];
    for cell in (num_center + 1)..=lbx_points {
        let num_edges = edge_counts[cell];
        if triangles_on_cell[cell].len() < num_edges {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} edge count {num_edges} exceeds triangles_on_cell row"),
            ));
        }
        let mut state_sum = 0_i32;
        for &triangle in triangles_on_cell[cell].iter().take(num_edges) {
            if triangle == 0 || triangle >= mrl.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} references invalid triangle {triangle}"),
                ));
            }
            state_sum += mrl[triangle];
        }
        if state_sum == num_edges as i32 || state_sum == (num_edges as i32) * 4 {
            continue;
        }
        mask[cell] = 1;
    }
    Ok(mask)
}

/// Port of `MOD_grid_preprocess.F90:GetSortNew` for final cell adjacency order.
///
/// For each cell `2..=num_dbx`, walks adjacent triangles using `IsNgrmm`, falls
/// back to the next unused input triangle when the walk is disconnected, then
/// reverses clockwise orders according to `robust_spherical_area_unit`.
pub fn get_sort_new_fortran_indexed(
    num_dbx: usize,
    n_triangles_on_cell: &[usize],
    cells_on_triangle: &[[usize; 3]],
    triangle_points: &[LonLatDegrees],
    triangles_on_cell: &mut [Vec<usize>],
) -> io::Result<()> {
    if num_dbx >= n_triangles_on_cell.len() || num_dbx >= triangles_on_cell.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_dbx must address cell adjacency arrays",
        ));
    }

    for cell in 2..=num_dbx {
        let num_inter = n_triangles_on_cell[cell];
        if triangles_on_cell[cell].len() < num_inter {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("cell {cell} adjacency row shorter than n_triangles_on_cell"),
            ));
        }
        if num_inter <= 1 {
            triangles_on_cell[cell].truncate(num_inter);
            continue;
        }
        let input = triangles_on_cell[cell][..num_inter].to_vec();
        for &triangle in &input {
            if triangle == 0
                || triangle >= cells_on_triangle.len()
                || triangle >= triangle_points.len()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cell {cell} references invalid triangle {triangle}"),
                ));
            }
        }

        let mut neighbor_degree = vec![0_usize; num_inter];
        for j in 0..num_inter {
            for next_pos in 0..num_inter {
                if next_pos == j {
                    continue;
                }
                if is_ngrmm(
                    cells_on_triangle[input[j]],
                    cells_on_triangle[input[next_pos]],
                )
                .is_some()
                {
                    neighbor_degree[j] += 1;
                }
            }
        }

        let start_pos = neighbor_degree
            .iter()
            .position(|&degree| degree == 1)
            .unwrap_or(0);
        let mut sorted = Vec::with_capacity(num_inter);
        let mut used = vec![false; num_inter];
        let mut ref_triangle = input[start_pos];
        sorted.push(ref_triangle);
        used[start_pos] = true;

        while sorted.len() < num_inter {
            let mut found = false;
            for j in 1..num_inter {
                if used[j] {
                    continue;
                }
                let candidate = input[j];
                if is_ngrmm(
                    cells_on_triangle[ref_triangle],
                    cells_on_triangle[candidate],
                )
                .is_none()
                {
                    continue;
                }
                ref_triangle = candidate;
                sorted.push(ref_triangle);
                used[j] = true;
                found = true;
                break;
            }
            if !found {
                if let Some((j, &candidate)) = input.iter().enumerate().find(|(idx, _)| !used[*idx])
                {
                    ref_triangle = candidate;
                    sorted.push(ref_triangle);
                    used[j] = true;
                } else {
                    break;
                }
            }
        }

        let polygon = sorted
            .iter()
            .map(|&triangle| triangle_points[triangle])
            .collect::<Vec<_>>();
        if let Some(area) = robust_spherical_area_unit(&polygon) {
            if area < 0.0 {
                sorted.reverse();
            }
        }
        triangles_on_cell[cell] = sorted;
    }

    Ok(())
}

/// Pure-data port of `MOD_refine.F90:bdy_connection_make`.
///
/// Builds boundary vertex-vertex connections from unrefined triangles that have
/// exactly one refined neighbor (`sum(mrl_bk(ngrmm(:, i))) == 6`), validates the
/// closed boundary degree invariant, then reuses the shared closed-curve walker.
pub fn refine_boundary_connection_make_fortran_indexed(
    num_vertex: usize,
    sjx_points: usize,
    lbx_points: usize,
    mrl_bk: &[i32],
    triangle_neighbors: &[Vec<usize>],
    cells_on_triangle: &[[usize; 3]],
) -> io::Result<BoundaryConnection> {
    if sjx_points >= mrl_bk.len()
        || sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "sjx_points must address refinement and triangle arrays",
        ));
    }
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "num_vertex must not exceed sjx_points",
        ));
    }
    let mut boundary_neighbors = vec![Vec::<usize>::new(); lbx_points + 1];
    let mut bdy_num_in_save = 1_usize;

    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_bk[triangle] != 1 {
            continue;
        }
        if triangle_neighbors[triangle].len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("triangle {triangle} must have three neighbors"),
            ));
        }
        let mut neighbor_state_sum = 0_i32;
        for &neighbor in triangle_neighbors[triangle].iter().take(3) {
            if neighbor == 0 {
                continue;
            }
            if neighbor >= mrl_bk.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} references invalid neighbor {neighbor}"),
                ));
            }
            neighbor_state_sum += mrl_bk[neighbor];
        }
        if neighbor_state_sum != 6 {
            continue;
        }
        let refined_neighbor = triangle_neighbors[triangle]
            .iter()
            .take(3)
            .copied()
            .find(|&neighbor| mrl_bk[neighbor] == 4)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} has boundary sum 6 but no refined neighbor"),
                )
            })?;
        bdy_num_in_save += 1;

        let parent_cells = cells_on_triangle[triangle];
        let refined_cells = cells_on_triangle[refined_neighbor];
        let free_pos = parent_cells
            .iter()
            .position(|cell| !refined_cells.contains(cell))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("triangle {triangle} has no vertex opposite refined neighbor"),
                )
            })?;
        let w1 = parent_cells[(free_pos + 1) % 3];
        let w2 = parent_cells[(free_pos + 2) % 3];
        for &vertex in &[w1, w2] {
            if vertex == 0 || vertex > lbx_points {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("boundary vertex {vertex} must be in 1..={lbx_points}"),
                ));
            }
        }
        push_boundary_neighbor(&mut boundary_neighbors, w1, w2)?;
        push_boundary_neighbor(&mut boundary_neighbors, w2, w1)?;
    }

    for vertex in (num_vertex + 1)..=lbx_points {
        if boundary_neighbors[vertex].len() == 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex} has open degree 1"),
            ));
        }
        if boundary_neighbors[vertex].len() > 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("boundary vertex {vertex} has more than two refine boundary neighbors"),
            ));
        }
    }

    let mut boundary_order = vec![1_usize];
    for vertex in (num_vertex + 1)..=lbx_points {
        if boundary_neighbors[vertex].len() == 2 {
            boundary_order.push(vertex);
        }
    }
    let bdy_num_in = boundary_order.len();
    if bdy_num_in_save != bdy_num_in {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "refine boundary triangle count {bdy_num_in_save} does not match boundary vertex count {bdy_num_in}"
            ),
        ));
    }

    let curves = boundary_closed_curves_fortran_indexed(&boundary_order, &boundary_neighbors)?;
    Ok(BoundaryConnection {
        bdy_num_in,
        boundary_order,
        boundary_neighbors,
        curves,
    })
}
