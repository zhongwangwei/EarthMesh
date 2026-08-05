use std::{collections::BTreeSet, io};

use super::*;

impl MethodCDelaunayMesh {
    #[cfg(test)]
    pub(crate) fn perim_map2_method_c(
        &self,
        nest_wd: &[MethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        Ok(self
            .perim_maps2_method_c_over(nest_wd, m_neighbors, None)?
            .into_iter()
            .flatten()
            .collect())
    }

    /// Perimeter walk restricted to a candidate set of start points.
    ///
    /// A start needs `nwdiv == 2`, so it must touch a subdivided face; the M
    /// corners of the selection are therefore a complete candidate set. Scanning
    /// all of `nmd` instead visits half a million M points to find a perimeter
    /// of a hundred, which is where the repair search spends most of its time.
    ///
    /// `candidates` must be ascending and deduplicated, so the perimeters come
    /// back in the order a full scan would produce them.
    pub(crate) fn perim_maps2_method_c_over(
        &self,
        nest_wd: &[MethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
        candidates: Option<&[usize]>,
    ) -> io::Result<Vec<Vec<MethodCPerimeterPoint>>> {
        require_method_c_len("Method-C nest_wd", nest_wd.len(), self.nwd + 1)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let mut perimeters = Vec::new();
        let mut seen = BTreeSet::new();
        match candidates {
            Some(candidates) => {
                for &im in candidates {
                    require_method_c_id("Method-C perimeter candidate M point", im, self.nmd)?;
                    self.perim_maps2_method_c_visit(
                        im,
                        nest_wd,
                        m_neighbors,
                        &mut seen,
                        &mut perimeters,
                    )?;
                }
            }
            None => {
                for im in 2..=self.nmd {
                    self.perim_maps2_method_c_visit(
                        im,
                        nest_wd,
                        m_neighbors,
                        &mut seen,
                        &mut perimeters,
                    )?;
                }
            }
        }
        if perimeters.is_empty() {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Method-C perimeter has no nwdiv == 2 convex start point",
            ))
        } else {
            Ok(perimeters)
        }
    }

    /// Start a perimeter at `im` if it qualifies and has not been walked.
    fn perim_maps2_method_c_visit(
        &self,
        im: usize,
        nest_wd: &[MethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
        seen: &mut BTreeSet<usize>,
        perimeters: &mut Vec<Vec<MethodCPerimeterPoint>>,
    ) -> io::Result<()> {
        let neighbors = m_neighbors[im];
        let mut nwdiv = 0usize;
        for &iw in neighbors.iw.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C perimeter W face", iw, self.nwd)?;
            if nest_wd[iw].is_subdivided() {
                nwdiv += 1;
            }
        }
        if nwdiv == 2 && !seen.contains(&im) {
            let perimeter = self.perim_map2_method_c_from(im, nest_wd, m_neighbors)?;
            seen.extend(perimeter.iter().map(|point| point.im));
            perimeters.push(perimeter);
        }
        Ok(())
    }

    fn perim_map2_method_c_from(
        &self,
        start: usize,
        nest_wd: &[MethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        let mut perimeter = Vec::new();
        let mut current = start;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Method-C perimeter loop revisited M point {current} before closing"),
                ));
            }

            let neighbors = m_neighbors[current];
            let mut nwdiv = 0usize;
            let mut near_pentagon = false;
            for j in 0..neighbors.npoly {
                let iw = neighbors.iw[j];
                let iu = neighbors.iu[j];
                require_method_c_id("Method-C perimeter W face", iw, self.nwd)?;
                require_method_c_id("Method-C perimeter U edge", iu, self.nud)?;
                if nest_wd[iw].is_subdivided() {
                    nwdiv += 1;
                }

                let edge = self.u_edges[iu];
                let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
                require_method_c_id("Method-C perimeter adjacent W face", iw1, self.nwd)?;
                require_method_c_id("Method-C perimeter adjacent W face", iw2, self.nwd)?;
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
            perimeter.push(MethodCPerimeterPoint {
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
        nest_wd: &[MethodCNestWd],
        m_neighbors: &[IcosahedronMPointNeighbors],
    ) -> io::Result<(usize, usize)> {
        require_method_c_id("Method-C perimeter M point", imstart, self.nmd)?;
        require_method_c_len(
            "Method-C perim M-neighbors",
            m_neighbors.len(),
            self.nmd + 1,
        )?;
        let neighbors = m_neighbors[imstart];
        for &iu in neighbors.iu.iter().take(neighbors.npoly) {
            require_method_c_id("Method-C perimeter U edge", iu, self.nud)?;
            let edge = self.u_edges[iu];
            let [iw1, iw2] = [edge.iw[0], edge.iw[1]];
            require_method_c_id("Method-C perimeter W face", iw1, self.nwd)?;
            require_method_c_id("Method-C perimeter W face", iw2, self.nwd)?;

            if edge.im[0] == imstart && nest_wd[iw1].flag() == 0 && nest_wd[iw2].is_subdivided() {
                return Ok((edge.im[1], iu));
            }
            if edge.im[1] == imstart && nest_wd[iw2].flag() == 0 && nest_wd[iw1].is_subdivided() {
                return Ok((edge.im[0], iu));
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Method-C perim_ngr could not advance from M point {imstart}"),
        ))
    }
}
