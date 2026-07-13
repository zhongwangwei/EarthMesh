use super::*;

impl MethodCDelaunayMesh {
    /// Port of Method-C `expand_global2`: insert one M point on every active
    /// Delaunay edge and subdivide every triangular W face into four children.
    ///
    /// The Canonical routine preserves/copies many atmosphere-loop fields while
    /// rebuilding the same triangular topology. This Rust path keeps the mesh
    /// fields currently owned by `earthmesh_mesh`, then performs a full M/U/W
    /// neighbor rebuild rather than depending on local edge-number patches.
    pub fn expand_global2(&self) -> io::Result<Self> {
        self.validate_topology()?;

        let radius = active_mesh_radius(self)?;
        let mut m_points = self.m_points.clone();
        let mut midpoint_by_edge = BTreeMap::new();

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            let midpoint = CartesianPoint::new(
                0.5 * (self.m_points[im1].x + self.m_points[im2].x),
                0.5 * (self.m_points[im1].y + self.m_points[im2].y),
                0.5 * (self.m_points[im1].z + self.m_points[im2].z),
            );
            let midpoint = normalize_cartesian_to_radius(midpoint, radius)?;
            let midpoint_id = m_points.len();
            m_points.push(midpoint);
            midpoint_by_edge.insert(method_c_edge_key(im1, im2), midpoint_id);
        }

        let mut child_faces = Vec::with_capacity((self.nwd - 1) * 4);
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            let [a, b, c] = face.im;
            let ab = lookup_method_c_midpoint(&midpoint_by_edge, a, b, iw)?;
            let bc = lookup_method_c_midpoint(&midpoint_by_edge, b, c, iw)?;
            let ca = lookup_method_c_midpoint(&midpoint_by_edge, c, a, iw)?;
            let metadata = (face.mrlw, face.mrlw_orig, face.ngr);

            child_faces.push(MethodCTriangleSeed::new([a, ab, ca], metadata).with_mrow(face.mrow));
            child_faces.push(MethodCTriangleSeed::new([b, bc, ab], metadata).with_mrow(face.mrow));
            child_faces.push(MethodCTriangleSeed::new([c, ca, bc], metadata).with_mrow(face.mrow));
            child_faces.push(MethodCTriangleSeed::new([ab, bc, ca], metadata).with_mrow(face.mrow));
        }

        method_c_mesh_from_triangle_seeds(m_points.len() - 1, self.impent, m_points, &child_faces)
    }

    /// Apply Method-C global expansion factors in the same 3-first, then 2-second
    /// order used by Method-C `expand_delaunay_mesh`.
    pub fn expand_by_factor(&self, factor: usize) -> io::Result<Self> {
        if factor == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C expansion factor must be positive",
            ));
        }

        let mut reduced = factor;
        while reduced.is_multiple_of(3) {
            reduced /= 3;
        }
        while reduced.is_multiple_of(2) {
            reduced /= 2;
        }
        if reduced != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C expansion factor {factor} must contain only factors of 2 and 3"),
            ));
        }

        let mut expanded = self.clone();
        let mut remaining = factor;
        while remaining.is_multiple_of(3) {
            expanded = expanded.expand_global3()?;
            remaining /= 3;
        }
        while remaining > 1 {
            expanded = expanded.expand_global2()?;
            remaining /= 2;
        }
        Ok(expanded)
    }

    /// Port of Method-C `expand_global3`: insert two M points on every active
    /// Delaunay edge, one M point inside every active W face, and subdivide
    /// each triangular face into nine children.
    pub fn expand_global3(&self) -> io::Result<Self> {
        self.validate_topology()?;

        let radius = active_mesh_radius(self)?;
        let mut m_points = self.m_points.clone();
        let mut thirds_by_edge = BTreeMap::new();

        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            let [im1, im2] = edge.im;
            let point1 = self.m_points[im1];
            let point2 = self.m_points[im2];
            let first_from_im1 = normalized_weighted_point(point1, 2.0, point2, 1.0, radius)?;
            let second_from_im1 = normalized_weighted_point(point1, 1.0, point2, 2.0, radius)?;
            let first_id = m_points.len();
            m_points.push(first_from_im1);
            let second_id = m_points.len();
            m_points.push(second_from_im1);
            let ids_from_low_to_high = if im1 <= im2 {
                [first_id, second_id]
            } else {
                [second_id, first_id]
            };
            thirds_by_edge.insert(method_c_edge_key(im1, im2), ids_from_low_to_high);
        }

        let mut child_faces = Vec::with_capacity((self.nwd - 1) * 9);
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            let [a, b, c] = face.im;
            let [ab1, ab2] = lookup_method_c_thirds(&thirds_by_edge, a, b, iw)?;
            let [bc1, bc2] = lookup_method_c_thirds(&thirds_by_edge, b, c, iw)?;
            let [ac1, ac2] = lookup_method_c_thirds(&thirds_by_edge, a, c, iw)?;
            let center = normalized_face_center(
                self.m_points[a],
                self.m_points[b],
                self.m_points[c],
                radius,
            )?;
            let center_id = m_points.len();
            m_points.push(center);
            let metadata = (face.mrlw, face.mrlw_orig, face.ngr);

            child_faces
                .push(MethodCTriangleSeed::new([a, ab1, ac1], metadata).with_mrow(face.mrow));
            child_faces.push(
                MethodCTriangleSeed::new([ab1, ab2, center_id], metadata).with_mrow(face.mrow),
            );
            child_faces.push(
                MethodCTriangleSeed::new([ac1, center_id, ac2], metadata).with_mrow(face.mrow),
            );
            child_faces
                .push(MethodCTriangleSeed::new([ab2, b, bc1], metadata).with_mrow(face.mrow));
            child_faces.push(
                MethodCTriangleSeed::new([center_id, bc1, bc2], metadata).with_mrow(face.mrow),
            );
            child_faces
                .push(MethodCTriangleSeed::new([ac2, bc2, c], metadata).with_mrow(face.mrow));
            child_faces.push(
                MethodCTriangleSeed::new([center_id, ac1, ab1], metadata).with_mrow(face.mrow),
            );
            child_faces.push(
                MethodCTriangleSeed::new([bc1, center_id, ab2], metadata).with_mrow(face.mrow),
            );
            child_faces.push(
                MethodCTriangleSeed::new([bc2, ac2, center_id], metadata).with_mrow(face.mrow),
            );
        }

        method_c_mesh_from_triangle_seeds(m_points.len() - 1, self.impent, m_points, &child_faces)
    }
}
