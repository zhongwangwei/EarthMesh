use std::io;

use super::*;

/// Factors the whole-run retry may try per earlier level.
///
/// Each one is a complete re-run, so this is the cost of a refusal. Measured
/// over the same forty-case rows: six recovers 232 of 240, twelve recovers 236,
/// twenty four recovers 237 and takes the workspace suite to half an hour.
const RESCALE_RETRY_FACTOR_BUDGET: usize = 12;

impl TriangularMesh {
    pub(crate) fn spawn_nest_internal(
        &self,
        regions: &[RefinementRegion],
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
    ) -> io::Result<(Self, usize)> {
        self.spawn_nest_internal_rescaling(
            regions,
            max_level,
            max_mrows,
            spring,
            use_cartesian_xy,
            0,
        )
    }

    /// The pass loop, with a note of whether it is already a retry.
    ///
    /// `rescale_depth` is zero for the run the caller asked for and one for a
    /// run this function started itself after moving a level. It exists to stop
    /// the retry recursing: one level of it is what the measurement supports,
    /// and a second would multiply the cost of a refusal by twenty four again.
    #[allow(clippy::too_many_arguments)]
    fn spawn_nest_internal_rescaling(
        &self,
        regions: &[RefinementRegion],
        max_level: usize,
        max_mrows: usize,
        spring: Option<(usize, usize, Option<f64>)>,
        use_cartesian_xy: bool,
        rescale_depth: usize,
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
        // Every completed pass, not only the last. A refusal at pass n can come
        // from how pass 1's boundary landed, and with one pass remembered there
        // was nothing to try for it.
        let mut pass_checkpoints: Vec<(Self, Vec<bool>, usize, Vec<RefinementRegion>, bool)> =
            Vec::new();
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
                    .any(|region| matches!(region, RefinementRegion::Polygon { .. }))
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
                mesh.selected_regions_faces_over_groups(&pass_regions, pass, use_cartesian_xy)?;
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
                            )) = pass_checkpoints.last()
                            {
                                // A parent that built cleanly used to end the
                                // matter, on the reading that a good parent mask
                                // has nothing to answer for. It does: the child
                                // fails on how the parent's boundary landed, not
                                // on whether the parent needed mending. Measured
                                // over sixty three-level cases at NXP 21, all
                                // fourteen refusals were rescued by moving the
                                // parent, and none of those parents had needed
                                // repair. A clean parent gets the rescaling try
                                // alone, which is the cheap one; the mask
                                // erosion sequences stay for the parent that did
                                // need mending.
                                let rescued = if *parent_required_repair {
                                    parent_base.retry_child_with_eroded_parent_mask(
                                        parent_selected,
                                        *parent_grid_number,
                                        parent_region,
                                        &pass_regions,
                                        grid_number,
                                        max_mrows,
                                        !use_cartesian_xy,
                                        use_cartesian_xy,
                                    )?
                                } else {
                                    parent_base.retry_child_with_scaled_parent_region(
                                        parent_region,
                                        *parent_grid_number,
                                        &pass_regions,
                                        grid_number,
                                        max_mrows,
                                        !use_cartesian_xy,
                                        use_cartesian_xy,
                                    )?
                                };
                                {
                                    if let Some(refined) = rescued {
                                        mesh = refined;
                                        pass_checkpoints.push((
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
                            // Nothing the pass above could do. A refusal here
                            // can come from where an *earlier* level put its
                            // boundary, and moving that one means everything
                            // built on it has to be built again. Rather than
                            // replay the passes by hand, which was measured to
                            // rescue nothing because it denied the tail the
                            // cascade the driver gives it, run the whole thing
                            // again from the start with that level moved.
                            //
                            // Measured over sixty three-level cases at NXP 21:
                            // eight refusals survived the single-step retry, and
                            // moving the first level rescued all eight.
                            //
                            // Nearest level first, so the run disturbs the least
                            // of what already worked.
                            if rescale_depth == 0 {
                                for moved in (1..pass).rev() {
                                    // Bounded on purpose. Each factor is a
                                    // whole re-run with its own cascades, for
                                    // every earlier level. Unbounded at twenty
                                    // four it took the workspace suite to
                                    // thirty two minutes and rescued one case
                                    // more than twelve does. The rescues that
                                    // land, land near one.
                                    for factor in
                                        crate::method_c_spawn_retry_scaled::scaled_parent_retry_factors()
                                            .take(RESCALE_RETRY_FACTOR_BUDGET)
                                    {
                                        let mut rescaled = Vec::with_capacity(regions.len());
                                        let mut scalable = Vec::new();
                                        for region in regions {
                                            if region.level() == moved {
                                                scalable.push(region.clone());
                                            } else {
                                                rescaled.push(region.clone());
                                            }
                                        }
                                        let Some(scaled) =
                                            scale_refinement_regions_radius(&scalable, factor)
                                        else {
                                            continue;
                                        };
                                        rescaled.extend(scaled);
                                        if let Ok(result) = self.spawn_nest_internal_rescaling(
                                            &rescaled,
                                            max_level,
                                            max_mrows,
                                            spring,
                                            use_cartesian_xy,
                                            1,
                                        ) {
                                            return Ok(result);
                                        }
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
            pass_checkpoints.push((
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
