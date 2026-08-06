use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        method_c_repairable_payload(error).is_some()
    }

    pub(crate) fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let payload = method_c_repairable_payload(error)?;
        (payload.kind == RepairableKind::Valence)
            .then_some(payload.m_point)
            .flatten()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        // Twelve passes per offending block, not twelve for the whole mesh. A
        // global coastal case came out of selection with 27 refined blocks, of
        // which three had a perimeter one short of a multiple of three; the
        // budget was spent long before the search reached them.
        const MAX_REPAIR_PASSES_PER_BLOCK: usize = 12;

        let mut last_error = None;
        let block_count = self
            .method_c_perimeters_from_selected_faces(selected, m_neighbors)
            .map(|perimeters| perimeters.len())
            .unwrap_or(1)
            .max(1);
        let max_passes = MAX_REPAIR_PASSES_PER_BLOCK.saturating_mul(block_count);
        for _ in 0..max_passes {
            // Search around the blocks that are not yet a multiple of three.
            // Handing the grower every perimeter buries the ones that need work:
            // the scoring is global, so a pass keeps picking candidates near a
            // block that is already fine.
            let perimeter =
                match self.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                    Ok(perimeters) if Self::method_c_perimeters_are_triplets(&perimeters) => {
                        return Ok(perimeters.into_iter().flatten().collect());
                    }
                    Ok(perimeters) => Some(
                        perimeters
                            .into_iter()
                            .filter(|perimeter| !perimeter.len().is_multiple_of(3))
                            .flatten()
                            .collect::<Vec<_>>(),
                    ),
                    Err(error) => {
                        last_error = Some(error);
                        None
                    }
                };
            let Some((repaired, _)) = self.try_grow_method_c_non_triplet_perimeter_once(
                selected,
                m_neighbors,
                child_level,
                perimeter.as_deref(),
            )?
            else {
                break;
            };
            selected.clone_from_slice(&repaired);
            let repaired_perimeters =
                self.method_c_perimeters_from_selected_faces(selected, m_neighbors)?;
            if Self::method_c_perimeters_are_triplets(&repaired_perimeters) {
                return Ok(repaired_perimeters.into_iter().flatten().collect());
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        let perimeters = self.method_c_perimeters_from_selected_faces(selected, m_neighbors)?;
        Err(repairable_error(
            RepairableKind::NonTripletPerimeter,
            None,
            format!(
                "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples without crossing the parent boundary",
                perimeters.iter().map(Vec::len).collect::<Vec<_>>()
            ),
        ))
    }
}
