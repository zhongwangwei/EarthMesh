use super::*;

impl TriangularMesh {
    pub(crate) fn method_c_thirdm_neighbors_canonical_with_neighbors(
        &self,
        im: usize,
        jdone: &mut [[bool; 6]],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<usize>> {
        require_method_c_id("Method-C thirdm start M point", im, self.nmd)?;
        require_method_c_len("Method-C thirdm jdone", jdone.len(), self.nmd + 1)?;
        require_method_c_len(
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
            require_method_c_id("Method-C thirdm far M point", immmm, self.nmd)?;
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

    pub(crate) fn opposite_ring_u_edge_with_neighbors(
        &self,
        im: usize,
        incoming_iu: usize,
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<usize> {
        require_method_c_id("Method-C thirdm M point", im, self.nmd)?;
        require_method_c_id("Method-C thirdm incoming U edge", incoming_iu, self.nud)?;
        require_method_c_len(
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
                            "Method-C thirdm incoming U edge {incoming_iu} has no valid opposite at M point {im}"
                        ),
                    ));
                }
                require_method_c_id("Method-C thirdm opposite U edge", opposite, self.nud)?;
                return Ok(opposite);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Method-C thirdm incoming U edge {incoming_iu} is not in M point {im}'s ring"),
        ))
    }

    pub(crate) fn other_m_endpoint(&self, iu: usize, im: usize) -> io::Result<usize> {
        require_method_c_id("Method-C U edge", iu, self.nud)?;
        require_method_c_id("Method-C M endpoint", im, self.nmd)?;
        let edge = self.u_edges[iu];
        if edge.im[0] == im {
            require_method_c_id("Method-C opposite M endpoint", edge.im[1], self.nmd)?;
            Ok(edge.im[1])
        } else if edge.im[1] == im {
            require_method_c_id("Method-C opposite M endpoint", edge.im[0], self.nmd)?;
            Ok(edge.im[0])
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Method-C U edge {iu} is not incident on M point {im}"),
            ))
        }
    }
}
