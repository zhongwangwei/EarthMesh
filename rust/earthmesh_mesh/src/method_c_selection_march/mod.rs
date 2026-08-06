use super::*;

fn euclidean_distance(a: CartesianPoint, b: CartesianPoint) -> f64 {
    magnitude(vector_between(a, b))
}

impl MethodCDelaunayMesh {
    #[cfg(test)]
    pub(crate) fn method_c_march_from_nearby_pentagon_to_region_with_neighbors(
        &self,
        pentagon_id: usize,
        region: &RefinementRegion,
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        self.method_c_march_from_nearby_pentagon_to_regions_with_neighbors(
            pentagon_id,
            std::slice::from_ref(region),
            radius,
            m_neighbors,
            use_cartesian_xy,
        )
    }

    pub(crate) fn method_c_march_from_nearby_pentagon_to_regions_with_neighbors(
        &self,
        pentagon_id: usize,
        regions: &[RefinementRegion],
        radius: f64,
        m_neighbors: &[IcosahedronMPointNeighbors],
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        require_method_c_id(
            "Method-C refinement nearby pentagon M point",
            pentagon_id,
            self.nmd,
        )?;
        let Some(nearest_inside) =
            self.nearest_inside_m_point_to_regions(pentagon_id, regions, radius, use_cartesian_xy)?
        else {
            return Ok(None);
        };

        let mut current = pentagon_id;
        let mut visited = BTreeSet::new();
        let mut jdone = vec![[false; 6]; self.nmd + 1];
        for _ in 0..self.nmd {
            if !visited.insert(current) {
                return Ok(None);
            }

            let mut best_neighbor = 0usize;
            let mut best_distance = f64::INFINITY;
            jdone[current] = [false; 6];
            for neighbor in self.method_c_thirdm_neighbors_canonical_with_neighbors(
                current,
                &mut jdone,
                m_neighbors,
            )? {
                let point = self.m_points[neighbor];
                if refine_regions_contain_method_c(regions, point, radius, use_cartesian_xy) {
                    return Ok(Some(neighbor));
                }
                let distance = euclidean_distance(point, self.m_points[nearest_inside]);
                if distance < best_distance {
                    best_distance = distance;
                    best_neighbor = neighbor;
                }
            }
            if best_neighbor <= 1 {
                return Ok(None);
            }
            current = best_neighbor;
        }

        Ok(None)
    }

    #[cfg(test)]
    pub(crate) fn nearest_inside_m_point_to(
        &self,
        source_im: usize,
        region: &RefinementRegion,
        radius: f64,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        self.nearest_inside_m_point_to_regions(
            source_im,
            std::slice::from_ref(region),
            radius,
            use_cartesian_xy,
        )
    }

    pub(crate) fn nearest_inside_m_point_to_regions(
        &self,
        source_im: usize,
        regions: &[RefinementRegion],
        radius: f64,
        use_cartesian_xy: bool,
    ) -> io::Result<Option<usize>> {
        require_method_c_id("Method-C refinement source M point", source_im, self.nmd)?;
        let mut nearest_inside = None;
        let mut nearest_distance = f64::INFINITY;
        for im in 2..=self.nmd {
            if !refine_regions_contain_method_c(
                regions,
                self.m_points[im],
                radius,
                use_cartesian_xy,
            ) {
                continue;
            }
            let distance = euclidean_distance(self.m_points[im], self.m_points[source_im]);
            if distance < nearest_distance {
                nearest_distance = distance;
                nearest_inside = Some(im);
            }
        }
        Ok(nearest_inside)
    }
}
