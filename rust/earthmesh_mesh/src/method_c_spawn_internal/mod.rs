use std::io;

use super::*;

impl MethodCDelaunayMesh {
    pub(crate) fn spawn_nest_internal(
        &self,
        regions: &[MethodCRefinementRegion],
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
    ) -> io::Result<(Self, usize)> {
        self.validate_topology()?;
        if regions.is_empty() || max_level == 0 {
            return Ok((self.clone(), 0));
        }
        if max_mrows == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Method-C spawn_nest max_mrows must be greater than zero",
            ));
        }
        if max_level > 5 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C refinement max_level {max_level} must be in 1..=5"),
            ));
        }
        for region in regions {
            if use_cartesian_xy {
                region.validate_cartesian_xy()?;
            } else {
                region.validate()?;
            }
        }

        let mut mesh = self.clone();
        let mut spring_passes = 0usize;
        let mut next_grid_number = self
            .w_faces
            .iter()
            .skip(2)
            .map(|face| face.ngr)
            .chain(self.m_metadata.iter().skip(2).map(|metadata| metadata.ngr))
            .max()
            .unwrap_or(1)
            .max(1)
            + 1;
        let mut previous_pass_checkpoint: Option<(
            Self,
            Vec<bool>,
            usize,
            Vec<MethodCRefinementRegion>,
            bool,
        )> = None;
        let mut pass_levels = regions
            .iter()
            .filter_map(|region| (region.level() <= max_level).then_some(region.level()))
            .collect::<Vec<_>>();
        pass_levels.sort_unstable();
        pass_levels.dedup();
        for pass in pass_levels {
            let pass_regions = regions
                .iter()
                .filter(|region| region.level() == pass)
                .cloned()
                .collect::<Vec<_>>();
            if pass_regions.is_empty() {
                continue;
            }
            let has_nested_parent = mesh.w_faces.iter().skip(2).any(|face| face.ngr > 1);
            let has_parent_level_region = regions.iter().any(|region| region.level() == pass - 1);
            if pass > 1 && pass_regions.len() > 1 && !has_nested_parent && !has_parent_level_region
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C perimeter length invalid: pass {pass} multiple child regions require explicit parent-level halo"
                    ),
                ));
            }
            if pass > 1
                && pass_regions
                    .iter()
                    .any(|region| matches!(region, MethodCRefinementRegion::Polygon { .. }))
                && !has_nested_parent
                && !has_parent_level_region
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C perimeter length invalid: pass {pass} polygon regions require explicit parent-level halo"
                    ),
                ));
            }

            let selected_faces =
                mesh.selected_regions_faces(&pass_regions, pass, use_cartesian_xy)?;
            if selected_faces.iter().skip(2).all(|selected| !*selected) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "Method-C selected no active W faces for pass {pass}; refusing to replace a local nest with global expansion"
                    ),
                ));
            }
            let grid_number = next_grid_number;
            let mesh_before_pass = mesh.clone();
            let direct_pass = mesh.spawn_nest_pass_method_c_without_mask_repair(
                &selected_faces,
                grid_number,
                max_mrows,
                !use_cartesian_xy,
            );
            let pass_requires_repair = direct_pass.is_err();
            let pass_result = match direct_pass {
                Ok(refined) => Ok(refined),
                Err(_) => mesh.spawn_nest_pass_with_max_mrows(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    !use_cartesian_xy,
                ),
            };
            match pass_result {
                Ok(refined) => mesh = refined,
                Err(error) => match mesh.spawn_nest_pass_with_mask_annealing(
                    &selected_faces,
                    grid_number,
                    max_mrows,
                    !use_cartesian_xy,
                    pass > 1,
                )? {
                    Some(refined) => mesh = refined,
                    None => {
                        if pass > 1 && spring.is_none() {
                            if let Some((
                                parent_base,
                                parent_selected,
                                parent_grid_number,
                                parent_region,
                                parent_required_repair,
                            )) = previous_pass_checkpoint.as_ref()
                            {
                                if *parent_required_repair {
                                    if let Some(refined) = parent_base
                                        .retry_child_with_eroded_parent_mask(
                                            parent_selected,
                                            *parent_grid_number,
                                            parent_region,
                                            &pass_regions,
                                            grid_number,
                                            max_mrows,
                                            !use_cartesian_xy,
                                            use_cartesian_xy,
                                        )?
                                    {
                                        mesh = refined;
                                        previous_pass_checkpoint = Some((
                                            mesh_before_pass,
                                            selected_faces.clone(),
                                            grid_number,
                                            pass_regions.clone(),
                                            pass_requires_repair,
                                        ));
                                        next_grid_number += 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        return Err(io::Error::new(
                            error.kind(),
                            format!("Method-C spawn_nest pass {pass} failed: {error}"),
                        ));
                    }
                },
            }
            previous_pass_checkpoint = Some((
                mesh_before_pass,
                selected_faces.clone(),
                grid_number,
                pass_regions,
                pass_requires_repair,
            ));
            next_grid_number += 1;

            if let Some((nxp, niter, cartesian_dist00)) = spring {
                if niter > 0 {
                    mesh = mesh.spring_nest_with_radius_projection(
                        nxp,
                        niter,
                        grid_number,
                        false,
                        !use_cartesian_xy,
                        cartesian_dist00,
                    )?;
                    spring_passes += 1;
                }
            }
        }

        Ok((mesh, spring_passes))
    }
}
