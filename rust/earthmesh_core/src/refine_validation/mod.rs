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
            // Deliberately allow atmosmesh calculated thresholds: the Rust hfield path
            // supports atmosphere/typhoon criteria even though the old Canonical read_nl
            // guard rejected RL%refine_cal for atmosmesh.
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

        match mesh_type {
            "landmesh" if !has_land => Err(
                "Must one of TRUE in the refine_num_landtypes or refine_area_mainland or refine_onelayer_Lnd or refine_twolayer_Lnd when refine is TRUE and meshtype = landmesh"
                    .to_string(),
            ),
            "oceanmesh" if !has_ocean => Err(
                "Must one of TRUE in the refine_sea_ratio or refine_onelayer_Ocn when refine is TRUE and meshtype = oceanmesh"
                    .to_string(),
            ),
            "atmosmesh" if !has_atmos => Err(
                "Must one of TRUE in the refine_onelayer_Atmos when refine is TRUE and meshtype = atmosmesh"
                    .to_string(),
            ),
            "LOCmesh" if !(has_land || has_ocean || has_atmos) => Err(
                "Must one threshold switch be TRUE for LOCmesh among land, ocean, or atmos criteria"
                    .to_string(),
            ),
            _ => Ok(()),
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
