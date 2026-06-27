use std::io;

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn apply_olam_perimeter_mrows(
        &mut self,
        ngr: usize,
        max_mrows: usize,
    ) -> io::Result<()> {
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
            for &iw in self.m_neighbors[im]
                .iw
                .iter()
                .take(self.m_neighbors[im].npoly)
            {
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
}
