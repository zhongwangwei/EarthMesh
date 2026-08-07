use std::io;

use super::*;

impl MethodCMesh {
    pub(crate) fn spawn_nest_pass_with_mask_annealing(
        &self,
        selected_faces: &[bool],
        child_level: usize,
        max_mrows: usize,
        project_to_radius: bool,
        strict: bool,
    ) -> io::Result<Option<Self>> {
        let mut selected = selected_faces.to_vec();
        for _ in 0..32 {
            let eroded = if strict {
                self.erode_method_c_selected_m_boundary(&selected)?
            } else {
                self.erode_method_c_selected_boundary(&selected)?
            };
            let Some(eroded) = eroded else {
                return Ok(None);
            };
            selected = eroded;
            if selected.iter().skip(2).all(|selected| !*selected) {
                return Ok(None);
            }
            let attempt = if strict {
                self.spawn_nest_pass_method_c_without_mask_repair(
                    &selected,
                    child_level,
                    max_mrows,
                    project_to_radius,
                )
            } else {
                self.spawn_nest_pass_with_max_mrows(
                    &selected,
                    child_level,
                    max_mrows,
                    project_to_radius,
                )
            };
            if let Ok(refined) = attempt {
                return Ok(Some(refined));
            }
        }
        Ok(None)
    }

    pub(crate) fn erode_method_c_selected_boundary(
        &self,
        selected: &[bool],
    ) -> io::Result<Option<Vec<bool>>> {
        require_method_c_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let mut eroded = selected.to_vec();
        let mut removed = false;
        for iw in 2..=self.nwd {
            if !selected[iw] {
                continue;
            }
            let face = self.w_faces[iw];
            for &neighbor in face.iw.iter().take(3) {
                if neighbor <= 1 || neighbor > self.nwd || !selected[neighbor] {
                    eroded[iw] = false;
                    removed = true;
                    break;
                }
            }
        }
        if removed {
            Ok(Some(eroded))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn erode_method_c_selected_m_boundary(
        &self,
        selected: &[bool],
    ) -> io::Result<Option<Vec<bool>>> {
        require_method_c_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let m_neighbors = self.method_c_m_neighbors()?;
        let mut eroded = selected.to_vec();
        let mut removed = false;
        for im in 2..=self.nmd {
            let neighbors = m_neighbors[im];
            let mut selected_count = 0usize;
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                require_method_c_id("Method-C M-boundary erosion W face", iw, self.nwd)?;
                selected_count += usize::from(selected[iw]);
            }
            if selected_count == 0 || selected_count == neighbors.npoly {
                continue;
            }
            for &iw in neighbors.iw.iter().take(neighbors.npoly) {
                if selected[iw] {
                    eroded[iw] = false;
                    removed = true;
                }
            }
        }
        if removed {
            Ok(Some(eroded))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn grow_method_c_selected_boundary(
        &self,
        selected: &[bool],
    ) -> io::Result<Option<Vec<bool>>> {
        require_method_c_len("Method-C selected faces", selected.len(), self.nwd + 1)?;
        let parent_mrlw = selected
            .iter()
            .enumerate()
            .skip(2)
            .find_map(|(iw, is_selected)| is_selected.then_some(self.w_faces[iw].mrlw));
        let Some(parent_mrlw) = parent_mrlw else {
            return Ok(None);
        };
        let mut grown = selected.to_vec();
        let mut added = false;
        for iw in 2..=self.nwd {
            if !selected[iw] {
                continue;
            }
            let face = self.w_faces[iw];
            for &neighbor in face.iw.iter().take(3) {
                if neighbor <= 1 || neighbor > self.nwd {
                    continue;
                }
                if !selected[neighbor] && self.w_faces[neighbor].mrlw == parent_mrlw {
                    grown[neighbor] = true;
                    added = true;
                }
            }
        }
        if added {
            Ok(Some(grown))
        } else {
            Ok(None)
        }
    }

    #[cfg(test)]
    pub(crate) fn close_method_c_selected_face_concavities(
        &self,
        selected_faces: &mut [bool],
    ) -> io::Result<()> {
        self.close_method_c_concavities(selected_faces)
    }
}
