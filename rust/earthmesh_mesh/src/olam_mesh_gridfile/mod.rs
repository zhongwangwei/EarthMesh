use std::io;

use super::*;

impl OlamDelaunayMesh {
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
}
