//! Rust mesh kernels migrated from EarthMesh Fortran.

use std::io;

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
}

/// Port the global icosahedron branch of `mkgrd.F90:voronoi`.
///
/// The returned vectors intentionally keep Fortran-compatible one-based slots:
/// index `0` is unused, and valid records live in `1..=nma` and `1..=nwa`.
pub fn voronoi_grid_from_icosahedron_relaxed(
    relaxed: &IcosahedronRelaxedGrid,
    radius: f64,
) -> io::Result<VoronoiGridState> {
    require_grid_coordinate_len("relaxed.m_points", relaxed.m_points.len(), relaxed.nmd + 1)?;
    require_grid_coordinate_len(
        "relaxed.connectivity.w_faces",
        relaxed.connectivity.w_faces.len(),
        relaxed.nwd + 1,
    )?;
    require_grid_coordinate_len(
        "relaxed.m_neighbors",
        relaxed.m_neighbors.len(),
        relaxed.nmd + 1,
    )?;

    let mut grid = GridMemory {
        nma: relaxed.nwd,
        nua: relaxed.nud,
        nva: relaxed.nud,
        nwa: relaxed.nmd,
        mma: relaxed.nwd,
        mua: relaxed.nud,
        mva: relaxed.nud,
        mwa: relaxed.nmd,
        ..GridMemory::default()
    };
    grid.allocate_xyzem(grid.nma + 1);
    grid.allocate_xyzew(grid.nwa + 1);

    for iw in 1..=grid.nwa {
        let point = relaxed.m_points[iw];
        grid.xew[iw] = point.x as f32;
        grid.yew[iw] = point.y as f32;
        grid.zew[iw] = point.z as f32;
    }

    for im in 2..=grid.nma {
        let face = &relaxed.connectivity.w_faces[im];
        if face
            .im
            .iter()
            .any(|&idx| idx < 2 || idx >= relaxed.m_points.len())
        {
            continue;
        }
        let p1 = relaxed.m_points[face.im[0]];
        let p2 = relaxed.m_points[face.im[1]];
        let p3 = relaxed.m_points[face.im[2]];
        let barycenter = CartesianPoint::new(
            (p1.x + p2.x + p3.x) / 3.0,
            (p1.y + p2.y + p3.y) / 3.0,
            (p1.z + p2.z + p3.z) / 3.0,
        );
        let normalized = normalize_cartesian_to_radius(barycenter, radius)?;
        grid.xem[im] = normalized.x as f32;
        grid.yem[im] = normalized.y as f32;
        grid.zem[im] = normalized.z as f32;
    }

    let mut tabs = IjTabs::allocate(grid.nma + 1, grid.nva + 1, grid.nwa + 1);
    for iw in 2..=grid.nwa {
        let neighbor = &relaxed.m_neighbors[iw];
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
        let face = &relaxed.connectivity.w_faces[im];
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

    Ok(VoronoiGridState { grid, tabs })
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
/// `icosahedron_relaxed_grid_fortran` -> `voronoi_grid_from_icosahedron_relaxed`
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
    let relaxed = icosahedron_relaxed_grid_fortran(nxp0, nspring, beta, spring_relax, max_tris)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "failed to build relaxed icosahedron grid",
            )
        })?;
    let mut state =
        voronoi_grid_from_icosahedron_relaxed(&relaxed, earthmesh_core::EARTH_RADIUS_METERS)?;
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

/// `mem_ijtabs:mloops` used by `mdloopf`, `udloopf`, and `wdloopf`.
pub const ICOSAHEDRON_MLOOPS: usize = 7;

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
    let radius = earthmesh_core::EARTH_RADIUS_METERS;
    let erador5 = radius / 5.0_f64.sqrt();
    let full_turn = earthmesh_core::PI2;

    std::array::from_fn(|slot| {
        let id = slot + 1;
        if id <= 5 {
            let angle_n = 0.2 * (id - 1) as f64 * full_turn;
            let angle_w = angle_n - 0.1 * full_turn;
            let angle_e = angle_n + 0.1 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(0.0, 0.0, -radius),
                north: CartesianPoint::new(
                    erador5 * 2.0 * angle_n.cos(),
                    erador5 * 2.0 * angle_n.sin(),
                    erador5,
                ),
                west: CartesianPoint::new(
                    erador5 * 2.0 * angle_w.cos(),
                    erador5 * 2.0 * angle_w.sin(),
                    -erador5,
                ),
                east: CartesianPoint::new(
                    erador5 * 2.0 * angle_e.cos(),
                    erador5 * 2.0 * angle_e.sin(),
                    -erador5,
                ),
            }
        } else {
            let angle_s = 0.2 * (id - 6) as f64 * full_turn + 0.1 * full_turn;
            let angle_w = angle_s - 0.1 * full_turn;
            let angle_e = angle_s + 0.1 * full_turn;
            IcosahedronDiamondCorners {
                south: CartesianPoint::new(
                    erador5 * 2.0 * angle_s.cos(),
                    erador5 * 2.0 * angle_s.sin(),
                    -erador5,
                ),
                north: CartesianPoint::new(0.0, 0.0, radius),
                west: CartesianPoint::new(
                    erador5 * 2.0 * angle_w.cos(),
                    erador5 * 2.0 * angle_w.sin(),
                    erador5,
                ),
                east: CartesianPoint::new(
                    erador5 * 2.0 * angle_e.cos(),
                    erador5 * 2.0 * angle_e.sin(),
                    erador5,
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
    let pwrd = 0.9_f64;
    let radius = earthmesh_core::EARTH_RADIUS_METERS;

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
                        ((nxp0 + 1 - i - j) as f64 / nxp0 as f64).clamp(0.0, 1.0),
                        0.0,
                        (j as f64 / (i + j - 1) as f64).clamp(0.0, 1.0),
                        1.0 - (j as f64 / (i + j - 1) as f64).clamp(0.0, 1.0),
                    )
                } else {
                    let wte0 = ((nxp0 - j) as f64 / (2 * nxp0 + 1 - i - j) as f64).clamp(0.0, 1.0);
                    (
                        0.0,
                        ((i + j - nxp0 - 1) as f64 / nxp0 as f64).clamp(0.0, 1.0),
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
                    wts * corners.south.x
                        + wtn * corners.north.x
                        + wtw * corners.west.x
                        + wte * corners.east.x,
                    wts * corners.south.y
                        + wtn * corners.north.y
                        + wtw * corners.west.y
                        + wte * corners.east.y,
                    wts * corners.south.z
                        + wtn * corners.north.z
                        + wtw * corners.west.z
                        + wte * corners.east.z,
                );
                let norm = (point.x * point.x + point.y * point.y + point.z * point.z).sqrt();
                if norm == 0.0 {
                    return None;
                }
                let expansion = radius / norm;
                m_points[im_left] = CartesianPoint::new(
                    point.x * expansion,
                    point.y * expansion,
                    point.z * expansion,
                );
            }
        }
    }

    m_points[2] = CartesianPoint::new(0.0, 0.0, -radius);
    m_points[counts.nmd] = CartesianPoint::new(0.0, 0.0, radius);

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
    let mut m_points = vec![IcosahedronMPointNeighbors::default(); nmd + 1];

    for iu in 2..u_edges.len() {
        for j in 0..2 {
            let im = u_edges.get(iu)?.im[j];
            let iw = u_edges.get(iu)?.iw[j];
            if im >= m_points.len() || iw >= w_faces.len() {
                return None;
            }

            if m_points[im].npoly != 0 && w_faces[iw].npoly >= 3 {
                continue;
            }

            let mut m_point = m_points[im];
            let start_iu = iu;
            let mut iunow = iu;
            let mut npoly = 0usize;

            while iunow > 1 {
                npoly += 1;
                if npoly > 7 {
                    return None;
                }

                let ring_slot = npoly - 1;
                let edge_now = *u_edges.get(iunow)?;
                m_point.iu[ring_slot] = iunow;

                if edge_now.im[0] == im {
                    if edge_now.iw[1] > 1 {
                        m_point.iw[ring_slot] = edge_now.iw[1];
                        iunow = edge_now.iu[2];
                    } else {
                        iunow = start_iu;
                    }
                } else if edge_now.iw[0] > 1 {
                    m_point.iw[ring_slot] = edge_now.iw[0];
                    iunow = edge_now.iu[1];
                } else {
                    iunow = start_iu;
                }

                m_point.npoly = npoly;
                if iunow == start_iu {
                    break;
                }
            }

            m_points[im] = m_point;
        }
    }

    Some(m_points)
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
        return None;
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
        return None;
    }
    let ycc = 0.5 * (dx13 * s2 - dx12 * s3 - dx23 * s1) / y_denom;

    let xcc = if dx12.abs() > dx13.abs() {
        if dx12 == 0.0 {
            return None;
        }
        (s2 - s1 - ycc * 2.0 * (p2.y - p1.y)) / (2.0 * dx12)
    } else {
        if dx13 == 0.0 {
            return None;
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
        return None;
    }
    let expansion = earth_radius / radius;
    circumcenter.x *= expansion;
    circumcenter.y *= expansion;
    circumcenter.z *= expansion;
    Some(circumcenter)
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
        centers[triangle_id] =
            spherical_circumcenter_from_barycenter(centers[triangle_id], vertices)?;
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
        for &triangle_id in triangles {
            if *triangle_flags.get(triangle_id)? {
                flagged += 1;
            }
        }
        boundary[cell_id] = flagged != 0 && flagged != triangles.len();
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

    for triangle_id in (num_vertex_in + 1)..triangle_flags.len() {
        if !triangle_flags[triangle_id] {
            continue;
        }
        for &edge_id in edges_on_vertex.get(triangle_id)? {
            if edge_id == 0 {
                continue;
            }
            *dists_on_edge.get_mut(edge_id)? = mindist00;
            *edge_moved.get_mut(edge_id)? = true;
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
        let dist_len = step.halo + input.num_rc;
        if dist_len == 0 {
            return None;
        }

        let current_edge_scale = edge_scale;
        edge_scale = current_edge_scale / 2.0;
        let edge_layers = distance_layers(2 * dist_len, current_edge_scale, input.spacing)?;
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
    if n_edges_on_cell.len() < vertices_on_cell.len() {
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
            let edge_id = edges_vertex1
                .iter()
                .copied()
                .find(|edge| *edge > 0 && edges_vertex2.contains(edge))?;
            let cells = *cells_on_edge.get(edge_id)?;
            let neighbor = if cells[0] == cell_id {
                cells[1]
            } else if cells[1] == cell_id {
                cells[0]
            } else {
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
    let edge_vector = vector_between(cell1, cell2);
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
    if maxlat_source >= minlat_source || minlat_source > lat_vertex.len() {
        return None;
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
        || input.mask_patch.len() < input.source_lon_vertices.len()
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
        return None;
    }

    let triangle_neighbors = triangle_neighbors_from_cell_membership_fortran_indexed(
        input.cells_on_triangle,
        input.triangles_on_cell,
        input.n_edges_on_cell,
    )?;
    let edge_output = get_edge_production_fortran_indexed(
        &triangle_neighbors,
        input.cells_on_triangle,
        input.triangle_lonlat,
        input.cell_lonlat,
    )?;
    let cell_connectivity = connect_on_cell_fortran_indexed(
        input.n_edges_on_cell,
        &edge_output.cells_on_edge,
        &edge_output.edges_on_vertex,
        input.triangles_on_cell,
    )?;
    let edges_on_edge_tri = edges_on_edge_tri_fortran_indexed(
        &edge_output.vertices_on_edge,
        &edge_output.edges_on_vertex,
    )?;
    let distance_output = set_dists_on_edge_global_fortran_indexed(SetDistsOnEdgeGlobalInput {
        base_dists_on_edge: input.base_dists_on_edge,
        base_cellwidth: input.base_cellwidth,
        num_rc: input.distance_num_rc,
        spacing: input.distance_spacing,
        triangles_on_cell: input.triangles_on_cell,
        cells_on_triangle: Some(input.cells_on_triangle),
        edges_on_vertex: &edge_output.edges_on_vertex,
        cells_on_edge: &edge_output.cells_on_edge,
        steps: input.distance_steps,
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
    let spring = spring_dynamics_global_fortran_indexed(
        &cell_points,
        input.n_edges_on_cell,
        &cell_connectivity.edges_on_cell,
        &edge_output.cells_on_edge,
        &edges_on_edge_tri,
        &distance_output.dists_on_edge,
        input.niter_refine,
        input.relax,
        input.radius,
        input.diagnostic_every,
    )?;
    let updated_cell_lonlat = spring
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
        &spring.updated_cell_points,
        input.cells_on_triangle,
    )?;
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
    let final_ordered = order_vertex_arrays_fortran_indexed(
        &updated_triangle_points,
        &edge_points_cartesian,
        &edge_output.edges_on_vertex,
        &edge_output.vertices_on_edge,
        &edge_output.cells_on_edge,
    )?;

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
        dists_on_edge: distance_output.dists_on_edge,
        cellwidth: distance_output.cellwidth,
        edge_lonlat: edge_output.edge_points,
        spring,
    })
}

/// Pure Rust adapter for the in-memory calculation sequence inside
/// `MOD_grid_preprocess:Springjustment_regional_step`.
///
/// This excludes `set_dbxMove_regional_step` and file side effects by accepting
/// the regional move mask explicitly. It wires the migrated topology,
/// `spring_dynamics_regionalv2`, cell lon/lat refresh, and triangle
/// centroid/circumcenter refresh sequence used by the Fortran routine.
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
    let cell_connectivity = connect_on_cell_fortran_indexed(
        input.n_edges_on_cell,
        &edge_connectivity.cells_on_edge,
        &edge_connectivity.edges_on_vertex,
        input.triangles_on_cell,
    )?;

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
        updated_triangle_lonlat,
        updated_cell_lonlat,
        triangle_neighbors,
        cells_on_edge: edge_connectivity.cells_on_edge,
        vertices_on_edge,
        edges_on_vertex: edge_connectivity.edges_on_vertex,
        edges_on_cell: cell_connectivity.edges_on_cell,
        cells_on_cell: cell_connectivity.cells_on_cell,
        regional,
    })
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
pub fn spherical_cell_area_from_vertices_unit(vertices: &[CartesianPoint]) -> Option<f64> {
    if vertices.len() < 3 {
        return None;
    }

    let anchor = vertices[0];
    let mut area = 0.0;
    for j in 0..(vertices.len() - 2) {
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
        let vertices = vertex_ids
            .iter()
            .map(|vertex_id| input.vertices.get(*vertex_id).copied())
            .collect::<Option<Vec<_>>>()?;
        area_cell[cell_id] = spherical_cell_area_from_vertices_unit(&vertices)?;
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
            if neighbor == 0 || neighbor > sjx_points {
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
            if neighbor == 0 || neighbor > sjx_points {
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
            let shared_neighbor = triangle_neighbors[m0]
                .iter()
                .copied()
                .find(|candidate| triangle_neighbors[w0].contains(candidate))
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!(
                            "segment {segment_id} triangle pair {m0}, {w0} has no shared neighbor"
                        ),
                    )
                })?;
            ref_sjx[shared_neighbor] = 1;

            for &candidate in &triangle_neighbors[shared_neighbor] {
                if mrl_new[candidate] == 4 {
                    continue;
                }
                segments[segment_id][j] = candidate;
            }
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
    let sjx_points = num_mp[iter - 1];
    if sjx_points >= triangle_neighbors.len()
        || sjx_points >= cells_on_triangle.len()
        || sjx_points >= ref_sjx.len()
        || sjx_points >= mrl_new.len()
        || sjx_points >= cells_on_triangle_new.len()
        || sjx_points >= sjx_child.len()
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
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "triangle {triangle} has no {} neighbor for one-into-two split",
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
