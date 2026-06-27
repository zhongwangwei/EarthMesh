use std::io;

use super::*;

impl OlamDelaunayMesh {
    pub(crate) fn is_repairable_method_c_transition_error(error: &io::Error) -> bool {
        let message = error.to_string();
        message.contains("transition patch")
            || message.contains("exceeds 7-edge OLAM ring")
            || message.contains("cannot be grouped into transition triples")
    }

    pub(crate) fn method_c_valence_error_m_point(error: &io::Error) -> Option<usize> {
        let message = error.to_string();
        let start = message.find("M point ")? + "M point ".len();
        let rest = &message[start..];
        let digit_count = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if digit_count == 0 || !rest[digit_count..].starts_with(" exceeds 7-edge") {
            return None;
        }
        rest[..digit_count].parse().ok()
    }

    pub(crate) fn repair_method_c_non_triplet_perimeter(
        &self,
        selected: &mut [bool],
        m_neighbors: &[IcosahedronMPointNeighbors],
        child_level: usize,
    ) -> io::Result<Vec<OlamMethodCPerimeterPoint>> {
        const MAX_REPAIR_PASSES: usize = 12;

        let mut last_error = None;
        for _ in 0..MAX_REPAIR_PASSES {
            let perimeter = match self.method_c_perimeter_from_selected_faces(selected, m_neighbors)
            {
                Ok(perimeter) if perimeter.len() % 3 == 0 => return Ok(perimeter),
                Ok(perimeter) => Some(perimeter),
                Err(error) => {
                    last_error = Some(error);
                    None
                }
            };
            let Some((repaired, repaired_perimeter)) = self
                .try_grow_method_c_non_triplet_perimeter_once(
                    selected,
                    m_neighbors,
                    child_level,
                    perimeter.as_deref(),
                )?
            else {
                break;
            };
            selected.clone_from_slice(&repaired);
            if repaired_perimeter.len() % 3 == 0 {
                return Ok(repaired_perimeter);
            }
        }

        if let Some(error) = last_error {
            return Err(error);
        }

        let perimeter = self.method_c_perimeter_from_selected_faces(selected, m_neighbors)?;
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Method-C perimeter length invalid: perimeter length {} cannot be grouped into transition triples without crossing the parent boundary",
                perimeter.len()
            ),
        ))
    }
}
