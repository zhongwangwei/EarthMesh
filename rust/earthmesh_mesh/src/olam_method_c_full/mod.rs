use std::io;

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn fill_method_c_full_subdivision(
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
}
