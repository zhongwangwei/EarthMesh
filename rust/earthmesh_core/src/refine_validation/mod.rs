use crate::RefineConfig;

impl RefineConfig {
    pub(crate) fn validate_like_read_nl(
        &mut self,
        mesh_type: &str,
        mode_grid: &str,
        external_field: bool,
    ) -> Result<(), String> {
        if !self.is_transition {
            if mode_grid != "tri" {
                return Err("not Istransition can only use in the tri".to_string());
            }
            self.spring_global_type = 0;
            self.spring_regional_type = 0;
        } else {
            if !(0..=1).contains(&self.spring_global_type) {
                return Err("SpringGlobal_type must 0,1".to_string());
            }
            if !(0..=2).contains(&self.spring_regional_type) {
                return Err("SpringRegional_type must 0,1,2".to_string());
            }
            if self.spring_global_type > 0 && self.spring_regional_type > 0 {
                return Err(
                    "only one of (SpringGlobal_type and SpringRegional_type) can larger than zero"
                        .to_string(),
                );
            }
        }

        if self.spring_global_type > 0 {
            self.vertex_pretect_layers = 0;
        }
        // Ruppert's proof reaches about 20.7 degrees and no further. Above it
        // the refinement is not guaranteed to terminate, and a run at 25 spent
        // its whole budget without converging (guide 11.29). Refused here
        // rather than discovered there.
        if !self.harp_min_angle_deg.is_finite() || self.harp_min_angle_deg < 0.0 {
            return Err("harp_min_angle_deg must be a non-negative finite angle".to_string());
        }
        if self.harp_min_angle_deg > 20.7 {
            return Err(format!(
                "harp_min_angle_deg {} exceeds the 20.7 degrees Ruppert's argument reaches; above                  it the refinement is not known to terminate. Use 0 to leave the criterion off",
                self.harp_min_angle_deg
            ));
        }
        if self.vertex_pretect_layers < 0 {
            return Err("vertex_pretect_layers must >= 0".to_string());
        }

        self.refine_setting = match (self.refine_spc, self.refine_cal) {
            (true, true) => "mixed".to_string(),
            (true, false) => "specified".to_string(),
            (false, true) => "calculate".to_string(),
            (false, false) if external_field => "external_field".to_string(),
            (false, false) => {
                return Err(
                    "Must one of TRUE in the refine_spc and refine_cal when refine is TRUE"
                        .to_string(),
                );
            }
        };

        if self.refine_setting == "calculate" || self.refine_setting == "mixed" {
            // Threshold datasets describe geography, not an output mesh type; the
            // output/domain mask decides which refined cells survive.
            self.validate_threshold_switches_for_mesh(mesh_type)?;
        }
        self.validate_enabled_threshold_values()?;

        Ok(())
    }

    fn validate_threshold_switches_for_mesh(&self, mesh_type: &str) -> Result<(), String> {
        let has_land = self.refine_num_landtypes
            || self.refine_area_mainland
            || self.refine_onelayer_lnd.iter().any(|enabled| *enabled)
            || self.refine_twolayer_lnd.iter().any(|enabled| *enabled);
        let has_ocean =
            self.refine_sea_ratio || self.refine_onelayer_ocn.iter().any(|enabled| *enabled);
        let has_atmos = self.refine_onelayer_atmos.iter().any(|enabled| *enabled);
        if has_land || has_ocean || has_atmos {
            Ok(())
        } else {
            Err(format!(
                "at least one land, ocean, or atmosphere threshold must be enabled for calculated refinement on {mesh_type}"
            ))
        }
    }

    fn validate_enabled_threshold_values(&self) -> Result<(), String> {
        for (index, enabled) in self.refine_onelayer_lnd.iter().enumerate() {
            if *enabled && self.th_onelayer_lnd[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Lnd({}) and th_onelayer_Lnd({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_twolayer_lnd.iter().enumerate() {
            if *enabled && self.th_twolayer_lnd[index].contains(&999.0) {
                return Err(format!(
                    "mismatch between refine_twolayer_Lnd({}) and th_twolayer_Lnd({}, 1:2)",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_onelayer_ocn.iter().enumerate() {
            if *enabled && self.th_onelayer_ocn[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Ocn({}) and th_onelayer_Ocn({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_onelayer_atmos.iter().enumerate() {
            if *enabled && self.th_onelayer_atmos[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Atmos({}) and th_onelayer_Atmos({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn land_threshold_is_valid_for_ocean_and_atmosphere_meshes() {
        let input = "&mkrefine\n\
 RL%Istransition = .true.\n\
 RL%SpringGlobal_type = 0\n\
 RL%SpringRegional_type = 0\n\
 RL%refine_cal = .true.\n\
 RL%max_iter_cal = 1\n\
 RL%refine_lai_m = .true.\n\
 RL%th_lai_m = 1.0\n/\n";

        for mesh_type in ["oceanmesh", "atmosmesh"] {
            let parsed = RefineConfig::from_mkrefine_namelist(input, mesh_type, "tri")
                .unwrap_or_else(|error| panic!("{mesh_type} rejected a land threshold: {error}"));
            assert!(parsed.refine_onelayer_lnd[0]);
        }
    }
}
