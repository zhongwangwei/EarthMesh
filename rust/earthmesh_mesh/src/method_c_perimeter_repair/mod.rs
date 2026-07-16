use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        method_c_repairable_payload(error).is_some()
    }

    pub(crate) fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let payload = method_c_repairable_payload(error)?;
        (payload.kind == MethodCRepairableKind::Valence)
            .then_some(payload.m_point)
            .flatten()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<MethodCPerimeterPoint>> {
        const MAX_REPAIR_PASSES: usize = 12;

        let mut last_error = None;
        for _ in 0..MAX_REPAIR_PASSES {
            let perimeter =
                match self.method_c_perimeters_from_selected_faces(selected, m_neighbors) {
                    Ok(perimeters) if Self::method_c_perimeters_are_triplets(&perimeters) => {
                        return Ok(perimeters.into_iter().flatten().collect());
                    }
                    Ok(perimeters) => Some(perimeters.into_iter().flatten().collect::<Vec<_>>()),
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
        Err(method_c_repairable_error(
            MethodCRepairableKind::NonTripletPerimeter,
            None,
            format!(
                "Method-C perimeter length invalid: perimeter lengths {:?} cannot be grouped into transition triples without crossing the parent boundary",
                perimeters.iter().map(Vec::len).collect::<Vec<_>>()
            ),
        ))
    }
}
