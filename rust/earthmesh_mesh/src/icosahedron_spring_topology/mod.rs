use super::*;

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
