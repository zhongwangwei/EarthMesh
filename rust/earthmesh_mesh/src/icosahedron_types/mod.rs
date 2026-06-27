use crate::{CartesianPoint, SpringDiagnosticMaxDisplacement};

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
