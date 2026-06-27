use std::{collections::BTreeSet, io};

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn perim_map2_method_c(
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

    pub(crate) fn perim_ngr_method_c(
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
}
