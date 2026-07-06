use crate::{fortran_quote, RefineConfig};

impl RefineConfig {
    /// Serialize the configuration back into the `&mkrefine` namelist block that
    /// `from_mkrefine_namelist` consumes. For the same `mesh_type`/`mode_grid`,
    /// `from_mkrefine_namelist(&x.to_mkrefine_namelist(), mesh_type, mode_grid)`
    /// reproduces `x`.
    ///
    /// Derived/internal fields (`refine_setting`, `max_iter`, `mask_refine_ndm`,
    /// `exit_loop_step`) are not namelist keys and are intentionally omitted; they
    /// are recomputed during re-parse. `HALO`/`max_transition_row` are Fortran
    /// 1-based arrays, so index 0 is the reserved sentinel and only indices 1..=9
    /// are emitted.
    pub fn to_mkrefine_namelist(&self) -> String {
        fn flag(value: bool) -> &'static str {
            if value {
                ".TRUE."
            } else {
                ".FALSE."
            }
        }
        fn ints_1_based(values: &[i32; 10]) -> String {
            values[1..]
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
        fn pair(values: [f64; 2]) -> String {
            format!("{}, {}", values[0], values[1])
        }
        fn q(value: &str) -> String {
            fortran_quote(value)
        }

        let mut out = String::new();
        out.push_str("&mkrefine\n");

        // common
        out.push_str(&format!(
            "  RL%weak_concav_eliminate = {}\n",
            flag(self.weak_concav_eliminate)
        ));
        out.push_str(&format!(
            "  RL%Istransition = {}\n",
            flag(self.is_transition)
        ));
        out.push_str(&format!("  RL%iterD = {}\n", flag(self.iter_d)));
        out.push_str(&format!("  RL%HALO = {}\n", ints_1_based(&self.halo)));
        out.push_str(&format!(
            "  RL%max_transition_row = {}\n",
            ints_1_based(&self.max_transition_row)
        ));

        // spring
        out.push_str(&format!(
            "  RL%SpringGlobal_type = {}\n",
            self.spring_global_type
        ));
        out.push_str(&format!(
            "  RL%SpringRegional_type = {}\n",
            self.spring_regional_type
        ));
        out.push_str(&format!("  RL%num_rc = {}\n", self.num_rc));
        out.push_str(&format!("  RL%set_dis_type = {}\n", q(&self.set_dis_type)));
        out.push_str(&format!(
            "  RL%vertex_pretect_layers = {}\n",
            self.vertex_pretect_layers
        ));
        if self.niter_refine_specified {
            out.push_str(&format!("  RL%niter_refine = {}\n", self.niter_refine));
        }

        // specified / calculated control
        out.push_str(&format!("  RL%refine_spc = {}\n", flag(self.refine_spc)));
        out.push_str(&format!("  RL%refine_cal = {}\n", flag(self.refine_cal)));
        out.push_str(&format!("  RL%max_iter_spc = {}\n", self.max_iter_spc));
        out.push_str(&format!("  RL%max_iter_cal = {}\n", self.max_iter_cal));
        out.push_str(&format!(
            "  RL%mask_refine_spc_type = {}\n",
            q(&self.mask_refine_spc_type)
        ));
        out.push_str(&format!(
            "  RL%mask_refine_spc_fprefix = {}\n",
            q(&self.mask_refine_spc_fprefix)
        ));
        out.push_str(&format!(
            "  RL%mask_refine_cal_type = {}\n",
            q(&self.mask_refine_cal_type)
        ));
        out.push_str(&format!(
            "  RL%mask_refine_cal_fprefix = {}\n",
            q(&self.mask_refine_cal_fprefix)
        ));
        out.push_str(&format!(
            "  RL%threshold_dir = {}\n",
            q(&self.threshold_dir)
        ));

        // land one-layer
        out.push_str(&format!(
            "  RL%refine_num_landtypes = {}\n",
            flag(self.refine_num_landtypes)
        ));
        out.push_str(&format!(
            "  RL%th_num_landtypes = {}\n",
            self.th_num_landtypes
        ));
        out.push_str(&format!(
            "  RL%refine_area_mainland = {}\n",
            flag(self.refine_area_mainland)
        ));
        out.push_str(&format!(
            "  RL%th_area_mainland = {}\n",
            self.th_area_mainland
        ));
        let land_one = [
            ("lai_m", 0usize),
            ("lai_s", 1),
            ("slope_m", 2),
            ("slope_s", 3),
            ("dem_m", 4),
            ("dem_s", 5),
            ("slope_max_m", 6),
            ("slope_max_s", 7),
        ];
        for (name, idx) in land_one {
            out.push_str(&format!(
                "  RL%refine_{name} = {}\n",
                flag(self.refine_onelayer_lnd[idx])
            ));
            out.push_str(&format!("  RL%th_{name} = {}\n", self.th_onelayer_lnd[idx]));
        }

        // land two-layer soil criteria (each threshold is a [f64; 2] pair)
        let two = [
            ("k_s_m", 0usize),
            ("k_s_s", 1),
            ("k_solids_m", 2),
            ("k_solids_s", 3),
            ("tkdry_m", 4),
            ("tkdry_s", 5),
            ("tksatf_m", 6),
            ("tksatf_s", 7),
            ("tksatu_m", 8),
            ("tksatu_s", 9),
        ];
        for (name, idx) in two {
            out.push_str(&format!(
                "  RL%refine_{name} = {}\n",
                flag(self.refine_twolayer_lnd[idx])
            ));
            out.push_str(&format!(
                "  RL%th_{name} = {}\n",
                pair(self.th_twolayer_lnd[idx])
            ));
        }

        // ocean criteria
        out.push_str(&format!(
            "  RL%refine_sea_ratio = {}\n",
            flag(self.refine_sea_ratio)
        ));
        out.push_str(&format!(
            "  RL%th_sea_ratio = {}\n",
            pair(self.th_sea_ratio)
        ));
        let ocn = [
            ("sst_m", 0usize),
            ("sst_s", 1),
            ("ssh_m", 2),
            ("ssh_s", 3),
            ("eke_m", 4),
            ("eke_s", 5),
            ("sea_slope_m", 6),
            ("sea_slope_s", 7),
        ];
        for (name, idx) in ocn {
            out.push_str(&format!(
                "  RL%refine_{name} = {}\n",
                flag(self.refine_onelayer_ocn[idx])
            ));
            out.push_str(&format!("  RL%th_{name} = {}\n", self.th_onelayer_ocn[idx]));
        }

        // atmosphere criteria
        out.push_str(&format!(
            "  RL%refine_typhoon_m = {}\n",
            flag(self.refine_onelayer_atmos[0])
        ));
        out.push_str(&format!(
            "  RL%th_typhoon_m = {}\n",
            self.th_onelayer_atmos[0]
        ));
        out.push_str(&format!(
            "  RL%refine_typhoon_s = {}\n",
            flag(self.refine_onelayer_atmos[1])
        ));
        out.push_str(&format!(
            "  RL%th_typhoon_s = {}\n",
            self.th_onelayer_atmos[1]
        ));

        out.push_str("/\n");
        out
    }
}
