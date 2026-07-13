use super::*;

impl MethodCDelaunayMesh {
    /// Canonical text dump of the current Method-C Delaunay M/U/W topology tables.
    ///
    /// This is intentionally exhaustive for fields owned by `earthmesh_mesh` and
    /// stable across platforms, so external Canonical parity harnesses can compare
    /// full table contents without carrying large golden fixture files.
    pub fn method_c_delaunay_topology_dump(&self) -> String {
        let mut dump = String::new();
        dump.push_str(&format!(
            "counts nmd={} nud={} nwd={}\n",
            self.nmd, self.nud, self.nwd
        ));
        for im in 2..=self.nmd {
            let neighbors = self.m_neighbors[im];
            let metadata = self.m_metadata[im];
            // Derive the M-to-M neighbor ids from the incident U edges (other
            // endpoint of each edge). Previously this column was a hardcoded
            // `[1; 7]` placeholder, which defeated the "compare full table
            // contents" purpose stated above; unused slots keep the Canonical
            // dummy `1`.
            let mut stored_m_neighbors = [1usize; 7];
            for (slot, &iu) in neighbors.iu.iter().enumerate() {
                if slot >= stored_m_neighbors.len() {
                    break;
                }
                if iu > 1 && iu <= self.nud {
                    stored_m_neighbors[slot] =
                        canonical_other_endpoint_by_first(self.u_edges[iu], im);
                }
            }
            dump.push_str(&format!(
                "M {im} npoly={} mrlm={} mrlm_orig={} ngr={}",
                neighbors.npoly, metadata.mrlm, metadata.mrlm_orig, metadata.ngr
            ));
            push_usize_fields(&mut dump, " im", &stored_m_neighbors);
            push_usize_fields(&mut dump, " iu", &neighbors.iu);
            push_usize_fields(&mut dump, " iw", &neighbors.iw);
            dump.push('\n');
        }
        for iu in 2..=self.nud {
            let edge = self.u_edges[iu];
            dump.push_str(&format!("U {iu} mrlu={}", edge.mrlu));
            push_usize_fields(&mut dump, " im", &edge.im);
            push_usize_fields(&mut dump, " iu", &edge.iu);
            push_usize_fields(&mut dump, " iw", &edge.iw);
            dump.push('\n');
        }
        for iw in 2..=self.nwd {
            let face = self.w_faces[iw];
            dump.push_str(&format!(
                "W {iw} npoly={} mrlw={} mrlw_orig={} mrow={} ngr={}",
                face.npoly, face.mrlw, face.mrlw_orig, face.mrow, face.ngr
            ));
            push_usize_fields(&mut dump, " im", &face.im);
            push_usize_fields(&mut dump, " iu", &face.iu);
            push_usize_fields(&mut dump, " iw", &face.iw);
            dump.push('\n');
        }
        dump
    }
}

fn push_usize_fields<const N: usize>(output: &mut String, label: &str, values: &[usize; N]) {
    output.push_str(label);
    for value in values {
        output.push_str(&format!(" {value}"));
    }
}
