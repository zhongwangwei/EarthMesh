//! An ICON nest set as data: the global grid and the nests cut from it, each
//! a bisection of its parent. The ICON nest backend
//! (`earthmesh_refine_icon_nest`) plans a set; the ICON writer in the output
//! layer numbers and writes it. Both meet here, in the foundation layer.

/// A unit vector.
pub type P = [f64; 3];

/// Where a nest vertex comes from, in the parent domain's point ids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NestVertexOrigin {
    /// A vertex of the global grid (the global domain only).
    Base,
    /// A parent vertex kept by the bisection.
    Parent(usize),
    /// The midpoint of the parent edge between two parent vertices (`a < b`).
    Midpoint(usize, usize),
}

/// One ICON domain: the global grid or a nest.
#[derive(Clone, Debug)]
pub struct IconNestDomain {
    /// 1-based ICON domain id; the global grid is 1.
    pub id: usize,
    /// Domain id of the parent; 0 for the global grid.
    pub parent: usize,
    /// Bisections below the global grid (0 for the global grid).
    pub depth: u32,
    pub points: Vec<P>,
    /// Triangles, counter-clockwise seen from outside the sphere.
    pub triangles: Vec<[usize; 3]>,
    /// The parent triangle each triangle came from (empty for the global grid).
    pub parent_triangle: Vec<usize>,
    pub vertex_origin: Vec<NestVertexOrigin>,
    /// Distance of each vertex from the lateral boundary, in vertex rows:
    /// 1 on the boundary. `u32::MAX` for the global grid, which has none.
    pub vertex_row: Vec<u32>,
    /// Lateral boundary row of each triangle: the least row of its vertices.
    pub cell_row: Vec<u32>,
}
