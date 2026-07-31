use std::io;

use super::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct MethodCGridfileMetadata<'a> {
    /// Final triangular M-cell rows, which map to Delaunay W faces.
    pub m_lineage: Option<&'a [i64]>,
    pub m_refine_level: Option<&'a [i32]>,
    pub m_refine_level_orig: Option<&'a [i32]>,
    pub m_ngr: Option<&'a [i32]>,
    /// Final polygonal W-cell rows, which map to Delaunay M points.
    pub w_lineage: Option<&'a [i64]>,
    pub w_refine_level: Option<&'a [i32]>,
    pub w_refine_level_orig: Option<&'a [i32]>,
    pub w_ngr: Option<&'a [i32]>,
}

impl MethodCDelaunayMesh {
    /// Rebuild an Method-C Delaunay mesh from the compact EarthMesh gridfile
    /// tables written at the Voronoi output boundary.
    ///
    /// In that schema, `GLONW/GLATW` rows are the Method-C Delaunay M points and
    /// `itab_m%iw` rows are the Method-C W-face M-point triplets. Row `0`
    /// corresponds to Canonical/Method-C id `1`; active records start at id `2`.
    pub fn from_voronoi_gridfile_tables(
        m_point_lonlat: &[LonLatDegrees],
        w_face_m_points: &[[usize; 3]],
        m_face_counts: &[usize],
    ) -> io::Result<Self> {
        Self::from_voronoi_gridfile_tables_with_refine_levels(
            m_point_lonlat,
            w_face_m_points,
            m_face_counts,
            None,
            None,
        )
    }

    /// Rebuild a Method-C mesh while restoring the zero-based refinement
    /// ownership levels persisted by EarthMesh gridfiles.
    pub fn from_voronoi_gridfile_tables_with_refine_levels(
        m_point_lonlat: &[LonLatDegrees],
        w_face_m_points: &[[usize; 3]],
        m_face_counts: &[usize],
        gridfile_m_refine_level: Option<&[i32]>,
        gridfile_w_refine_level: Option<&[i32]>,
    ) -> io::Result<Self> {
        Self::from_voronoi_gridfile_tables_with_metadata(
            m_point_lonlat,
            w_face_m_points,
            m_face_counts,
            MethodCGridfileMetadata {
                m_refine_level: gridfile_m_refine_level,
                w_refine_level: gridfile_w_refine_level,
                ..Default::default()
            },
        )
    }

    pub fn from_voronoi_gridfile_tables_with_metadata(
        m_point_lonlat: &[LonLatDegrees],
        w_face_m_points: &[[usize; 3]],
        m_face_counts: &[usize],
        metadata: MethodCGridfileMetadata<'_>,
    ) -> io::Result<Self> {
        let nmd = m_point_lonlat.len();
        let nwd = w_face_m_points.len();
        require_method_c_len(
            "Method-C gridfile M point valences",
            m_face_counts.len(),
            nmd,
        )?;
        for (name, values, expected) in [
            ("M refinement level", metadata.m_refine_level, nwd),
            (
                "M original refinement level",
                metadata.m_refine_level_orig,
                nwd,
            ),
            ("M ngr", metadata.m_ngr, nwd),
            ("W refinement level", metadata.w_refine_level, nmd),
            (
                "W original refinement level",
                metadata.w_refine_level_orig,
                nmd,
            ),
            ("W ngr", metadata.w_ngr, nmd),
        ] {
            validate_gridfile_metadata(name, values, expected)?;
        }
        for (name, values, expected) in [
            ("M lineage", metadata.m_lineage, nwd),
            ("W lineage", metadata.w_lineage, nmd),
        ] {
            validate_gridfile_lineages(name, values, expected)?;
        }
        if nmd < 2 || nwd < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C gridfile tables must include placeholder row 1 and at least one active row",
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
                    "Method-C gridfile source must expose 12 pentagonal M points, found {}",
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
                (iw > 1).then(|| {
                    let level = metadata
                        .m_refine_level
                        .and_then(|levels| levels.get(row))
                        .copied()
                        .unwrap_or(0);
                    let level = level as usize + 1;
                    let original = metadata
                        .m_refine_level_orig
                        .and_then(|levels| levels.get(row))
                        .copied()
                        .map(|value| value as usize + 1)
                        .unwrap_or(level);
                    let ngr = metadata
                        .m_ngr
                        .and_then(|values| values.get(row))
                        .copied()
                        .map(|value| value as usize)
                        .unwrap_or(1);
                    MethodCTriangleSeed::new(im, (level, original, ngr))
                        .with_target_iw(iw)
                        .with_mrow(0)
                })
            })
            .collect::<Vec<_>>();

        let mut mesh = match method_c_mesh_from_triangle_seeds(
            nmd,
            impent,
            m_points.clone(),
            &face_seeds,
        ) {
            Ok(mesh) => mesh,
            Err(forward_err) => {
                let reversed = face_seeds
                    .iter()
                    .map(|seed| {
                        MethodCTriangleSeed::new(
                            [seed.im[0], seed.im[2], seed.im[1]],
                            (seed.mrlw, seed.mrlw_orig, seed.ngr),
                        )
                        .with_mrow(seed.mrow)
                        .with_target_iw(seed.target_iw)
                    })
                    .collect::<Vec<_>>();
                method_c_mesh_from_triangle_seeds(nmd, impent, m_points, &reversed).map_err(
                    |reverse_err| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "failed to rebuild Method-C mesh from gridfile tables; forward orientation: {forward_err}; reversed orientation: {reverse_err}"
                            ),
                        )
                    },
                )?
            }
        };
        for row in 1..nmd {
            if let Some(level) = metadata
                .w_refine_level
                .and_then(|values| values.get(row))
                .copied()
            {
                mesh.m_metadata[row + 1].mrlm = level as usize + 1;
            }
            if let Some(level) = metadata
                .w_refine_level_orig
                .and_then(|values| values.get(row))
                .copied()
            {
                mesh.m_metadata[row + 1].mrlm_orig = level as usize + 1;
            } else if metadata.w_refine_level.is_some() {
                mesh.m_metadata[row + 1].mrlm_orig = mesh.m_metadata[row + 1].mrlm;
            }
            if let Some(ngr) = metadata.w_ngr.and_then(|values| values.get(row)).copied() {
                mesh.m_metadata[row + 1].ngr = ngr as usize;
            }
        }
        if let Some(lineages) = metadata.m_lineage {
            mesh.w_lineage = gridfile_lineages_to_method_c("M", lineages, nwd)?;
            mesh.next_w_lineage = mesh
                .w_lineage
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .checked_add(1)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Method-C imported W-face lineage space exhausted",
                    )
                })?;
        }
        if let Some(lineages) = metadata.w_lineage {
            mesh.m_lineage = gridfile_lineages_to_method_c("W", lineages, nmd)?;
            mesh.next_m_lineage = mesh
                .m_lineage
                .iter()
                .copied()
                .max()
                .unwrap_or(1)
                .checked_add(1)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Method-C imported M-point lineage space exhausted",
                    )
                })?;
        }
        mesh.validate_topology()?;
        Ok(mesh)
    }
}

fn validate_gridfile_lineages(
    name: &str,
    values: Option<&[i64]>,
    expected: usize,
) -> io::Result<()> {
    let Some(values) = values else {
        return Ok(());
    };
    require_method_c_len(&format!("Method-C gridfile {name}"), values.len(), expected)?;
    let mut seen = BTreeSet::new();
    for (row, &value) in values.iter().enumerate().skip(1) {
        if value <= 1 || !seen.insert(value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Method-C gridfile {name} at row {} must be a unique lineage greater than one, got {value}",
                    row + 1
                ),
            ));
        }
    }
    Ok(())
}

fn gridfile_lineages_to_method_c(
    role: &str,
    values: &[i64],
    active_count: usize,
) -> io::Result<Vec<usize>> {
    let mut lineages = vec![0; active_count + 1];
    lineages[1] = 1;
    for (row, &value) in values.iter().enumerate().skip(1) {
        lineages[row + 1] = usize::try_from(value).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Method-C gridfile {role} lineage {value} at row {} exceeds the platform usize range",
                    row + 1
                ),
            )
        })?;
    }
    Ok(lineages)
}

fn validate_gridfile_metadata(
    name: &str,
    values: Option<&[i32]>,
    expected: usize,
) -> io::Result<()> {
    let Some(values) = values else {
        return Ok(());
    };
    require_method_c_len(&format!("Method-C gridfile {name}"), values.len(), expected)?;
    if let Some((row, value)) = values.iter().enumerate().find(|(row, value)| {
        if name.ends_with("ngr") {
            **value < 0 || (*row > 0 && **value == 0)
        } else {
            **value < 0
        }
    }) {
        let reason = if *value < 0 {
            "negative"
        } else {
            "non-positive"
        };
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Method-C gridfile {name} {value} is {reason} at row {row}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gridfile_rebuild_restores_persisted_refinement_ownership() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(3, 0, 1.0, 0.25, 100)
            .expect("base Method-C mesh");
        let points = (1..=mesh.nmd)
            .map(|im| xyz_to_lonlat_degrees(mesh.m_points[im]))
            .collect::<Vec<_>>();
        let faces = (1..=mesh.nwd)
            .map(|iw| mesh.w_faces[iw].im)
            .collect::<Vec<_>>();
        let counts = (1..=mesh.nmd)
            .map(|im| mesh.m_neighbors[im].npoly)
            .collect::<Vec<_>>();
        let mut m_levels = vec![0; mesh.nwd];
        let mut w_levels = vec![0; mesh.nmd];
        m_levels[1] = 2;
        w_levels[1] = 3;

        let rebuilt = MethodCDelaunayMesh::from_voronoi_gridfile_tables_with_refine_levels(
            &points,
            &faces,
            &counts,
            Some(&m_levels),
            Some(&w_levels),
        )
        .expect("level-aware gridfile rebuild");

        assert_eq!(rebuilt.w_faces[2].mrlw, 3);
        assert_eq!(rebuilt.m_metadata[2].mrlm, 4);
    }

    #[test]
    fn gridfile_rebuild_rejects_negative_refinement_levels() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(3, 0, 1.0, 0.25, 100)
            .expect("base Method-C mesh");
        let points = (1..=mesh.nmd)
            .map(|im| xyz_to_lonlat_degrees(mesh.m_points[im]))
            .collect::<Vec<_>>();
        let faces = (1..=mesh.nwd)
            .map(|iw| mesh.w_faces[iw].im)
            .collect::<Vec<_>>();
        let counts = (1..=mesh.nmd)
            .map(|im| mesh.m_neighbors[im].npoly)
            .collect::<Vec<_>>();
        let mut levels = vec![0; mesh.nwd];
        levels[1] = -1;

        let error = MethodCDelaunayMesh::from_voronoi_gridfile_tables_with_refine_levels(
            &points,
            &faces,
            &counts,
            Some(&levels),
            None,
        )
        .expect_err("negative level must fail");
        assert!(error.to_string().contains("negative"));

        let mut ngr = vec![1; mesh.nwd];
        ngr[0] = 0;
        ngr[1] = 0;
        let error = MethodCDelaunayMesh::from_voronoi_gridfile_tables_with_metadata(
            &points,
            &faces,
            &counts,
            MethodCGridfileMetadata {
                m_ngr: Some(&ngr),
                ..Default::default()
            },
        )
        .expect_err("active ngr zero must fail");
        assert!(error.to_string().contains("non-positive"));
    }

    #[test]
    fn gridfile_rebuild_restores_original_levels_and_ngr() {
        let mesh = MethodCDelaunayMesh::from_icosahedron(3, 0, 1.0, 0.25, 100)
            .expect("base Method-C mesh");
        let points = (1..=mesh.nmd)
            .map(|im| xyz_to_lonlat_degrees(mesh.m_points[im]))
            .collect::<Vec<_>>();
        let faces = (1..=mesh.nwd)
            .map(|iw| mesh.w_faces[iw].im)
            .collect::<Vec<_>>();
        let counts = (1..=mesh.nmd)
            .map(|im| mesh.m_neighbors[im].npoly)
            .collect::<Vec<_>>();
        let mut m_level = vec![0; mesh.nwd];
        let m_lineage = (0..mesh.nwd)
            .map(|row| if row == 0 { 1 } else { 10_000 + row as i64 })
            .collect::<Vec<_>>();
        let mut m_orig = vec![0; mesh.nwd];
        let mut m_ngr = vec![1; mesh.nwd];
        let mut w_level = vec![0; mesh.nmd];
        let w_lineage = (0..mesh.nmd)
            .map(|row| if row == 0 { 1 } else { 20_000 + row as i64 })
            .collect::<Vec<_>>();
        let mut w_orig = vec![0; mesh.nmd];
        let mut w_ngr = vec![1; mesh.nmd];
        m_level[1] = 2;
        m_orig[1] = 1;
        m_ngr[1] = 7;
        w_level[1] = 3;
        w_orig[1] = 1;
        w_ngr[1] = 8;

        let rebuilt = MethodCDelaunayMesh::from_voronoi_gridfile_tables_with_metadata(
            &points,
            &faces,
            &counts,
            MethodCGridfileMetadata {
                m_lineage: Some(&m_lineage),
                m_refine_level: Some(&m_level),
                m_refine_level_orig: Some(&m_orig),
                m_ngr: Some(&m_ngr),
                w_lineage: Some(&w_lineage),
                w_refine_level: Some(&w_level),
                w_refine_level_orig: Some(&w_orig),
                w_ngr: Some(&w_ngr),
            },
        )
        .expect("full Method-C metadata rebuild");

        assert_eq!(rebuilt.w_faces[2].mrlw, 3);
        assert_eq!(rebuilt.w_faces[2].mrlw_orig, 2);
        assert_eq!(rebuilt.w_faces[2].ngr, 7);
        assert_eq!(rebuilt.m_metadata[2].mrlm, 4);
        assert_eq!(rebuilt.m_metadata[2].mrlm_orig, 2);
        assert_eq!(rebuilt.m_metadata[2].ngr, 8);
        assert_eq!(
            rebuilt.gridfile_m_cell_lineages().unwrap(),
            m_lineage,
            "tri-cell W-face lineage must survive gridfile rebuild"
        );
        assert_eq!(
            rebuilt.gridfile_w_cell_lineages().unwrap(),
            w_lineage,
            "hex-cell M-point lineage must survive gridfile rebuild"
        );
    }
}
