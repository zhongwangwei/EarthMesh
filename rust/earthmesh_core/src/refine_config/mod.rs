use crate::{
    namelist_assignments, parse_canonical_bool, parse_canonical_string, parse_f64, parse_f64_array,
    parse_i32, parse_i32_canonical_1_based_array,
};

/// Typed equivalent of the operational `refine_vars` module state.
#[derive(Debug, Clone, PartialEq)]
pub struct RefineConfig {
    pub refine_setting: String,
    pub mask_refine_spc_type: String,
    pub mask_refine_spc_fprefix: String,
    /// Optional preprocessing for specified close masks. `polyline` is the
    /// compatibility default and is omitted from serialized namelists.
    pub mask_refine_spc_close_boundary: String,
    pub mask_refine_cal_type: String,
    pub mask_refine_cal_fprefix: String,
    pub threshold_dir: String,
    pub set_dis_type: String,
    pub mask_refine_ndm: [i32; 10],
    pub max_iter_spc: i32,
    pub max_iter_cal: i32,
    pub halo: [i32; 10],
    pub max_transition_row: [i32; 10],
    pub spring_global_type: i32,
    pub spring_regional_type: i32,
    pub num_rc: i32,
    pub vertex_pretect_layers: i32,
    pub niter_refine: i32,
    pub niter_refine_specified: bool,
    pub th_num_landtypes: i32,
    pub th_area_mainland: f64,
    pub th_sea_ratio: [f64; 2],
    pub th_onelayer_lnd: [f64; 8],
    pub th_onelayer_ocn: [f64; 8],
    pub th_onelayer_atmos: [f64; 2],
    pub th_twolayer_lnd: [[f64; 2]; 10],
    pub weak_concav_eliminate: bool,
    pub is_transition: bool,
    pub iter_d: bool,
    pub refine_spc: bool,
    pub refine_cal: bool,
    pub refine_num_landtypes: bool,
    pub refine_area_mainland: bool,
    pub refine_sea_ratio: bool,
    pub refine_onelayer_lnd: [bool; 8],
    pub refine_onelayer_ocn: [bool; 8],
    pub refine_onelayer_atmos: [bool; 2],
    pub refine_twolayer_lnd: [bool; 10],
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            refine_setting: "/tmp".to_string(),
            mask_refine_spc_type: "/tmp".to_string(),
            mask_refine_spc_fprefix: "/tmp".to_string(),
            mask_refine_spc_close_boundary: "polyline".to_string(),
            mask_refine_cal_type: "/tmp".to_string(),
            mask_refine_cal_fprefix: "/tmp".to_string(),
            threshold_dir: "/tmp".to_string(),
            set_dis_type: "/tmp".to_string(),
            mask_refine_ndm: [0; 10],
            max_iter_spc: 0,
            max_iter_cal: 0,
            halo: [0; 10],
            max_transition_row: [0; 10],
            spring_global_type: 1,
            spring_regional_type: 1,
            num_rc: 0,
            vertex_pretect_layers: 1,
            niter_refine: 200,
            niter_refine_specified: false,
            th_num_landtypes: 12,
            th_area_mainland: 0.6,
            th_sea_ratio: [0.5; 2],
            th_onelayer_lnd: [999.0; 8],
            th_onelayer_ocn: [999.0; 8],
            th_onelayer_atmos: [999.0; 2],
            th_twolayer_lnd: [[999.0; 2]; 10],
            weak_concav_eliminate: true,
            is_transition: false,
            iter_d: false,
            refine_spc: false,
            refine_cal: false,
            refine_num_landtypes: false,
            refine_area_mainland: false,
            refine_sea_ratio: false,
            refine_onelayer_lnd: [false; 8],
            refine_onelayer_ocn: [false; 8],
            refine_onelayer_atmos: [false; 2],
            refine_twolayer_lnd: [false; 10],
        }
    }
}

impl RefineConfig {
    /// Parse the `/mkrefine/ RL` namelist and apply the non-I/O validation and
    /// derived mode changes performed by `mkgrd.F90:read_nl`.
    pub fn from_mkrefine_namelist(
        input: &str,
        mesh_type: &str,
        mode_grid: &str,
    ) -> Result<Self, String> {
        Self::from_mkrefine_namelist_with_external_field(input, mesh_type, mode_grid, false)
    }

    /// Parse `&mkrefine` while allowing an independently validated external
    /// cell-width field to be the sole refinement source. The caller must only
    /// pass `external_field=true` after validating that field's own inputs.
    pub fn from_mkrefine_namelist_with_external_field(
        input: &str,
        mesh_type: &str,
        mode_grid: &str,
        external_field: bool,
    ) -> Result<Self, String> {
        let mut config = Self::default();
        for assignment in namelist_assignments(input, "mkrefine")? {
            let field = assignment.field.as_str();
            let value = assignment.value.as_str();

            match field.to_ascii_lowercase().as_str() {
                "weak_concav_eliminate" => {
                    config.weak_concav_eliminate = parse_canonical_bool(field, value)?
                }
                "istransition" => config.is_transition = parse_canonical_bool(field, value)?,
                "iterd" => config.iter_d = parse_canonical_bool(field, value)?,
                "halo" => config.halo = parse_i32_canonical_1_based_array(field, value)?,
                "max_transition_row" => {
                    config.max_transition_row = parse_i32_canonical_1_based_array(field, value)?
                }
                "springglobal_type" => config.spring_global_type = parse_i32(field, value)?,
                "springregional_type" => config.spring_regional_type = parse_i32(field, value)?,
                "num_rc" => config.num_rc = parse_i32(field, value)?,
                "set_dis_type" => config.set_dis_type = parse_canonical_string(value),
                "vertex_pretect_layers" => config.vertex_pretect_layers = parse_i32(field, value)?,
                "niter_refine" => {
                    config.niter_refine = parse_i32(field, value)?;
                    config.niter_refine_specified = true;
                }
                "refine_spc" => config.refine_spc = parse_canonical_bool(field, value)?,
                "refine_cal" => config.refine_cal = parse_canonical_bool(field, value)?,
                "max_iter_spc" => config.max_iter_spc = parse_i32(field, value)?,
                "max_iter_cal" => config.max_iter_cal = parse_i32(field, value)?,
                "mask_refine_spc_type" => {
                    config.mask_refine_spc_type = parse_canonical_string(value)
                }
                "mask_refine_spc_fprefix" => {
                    config.mask_refine_spc_fprefix = parse_canonical_string(value)
                }
                "mask_refine_spc_close_boundary" => {
                    config.mask_refine_spc_close_boundary = parse_canonical_string(value)
                }
                "mask_refine_cal_type" => {
                    config.mask_refine_cal_type = parse_canonical_string(value)
                }
                "mask_refine_cal_fprefix" => {
                    config.mask_refine_cal_fprefix = parse_canonical_string(value)
                }
                "threshold_dir" => config.threshold_dir = parse_canonical_string(value),
                "refine_num_landtypes" => {
                    config.refine_num_landtypes = parse_canonical_bool(field, value)?
                }
                "refine_area_mainland" => {
                    config.refine_area_mainland = parse_canonical_bool(field, value)?
                }
                "refine_sea_ratio" => config.refine_sea_ratio = parse_canonical_bool(field, value)?,
                "refine_lai_m" => {
                    config.refine_onelayer_lnd[0] = parse_canonical_bool(field, value)?
                }
                "refine_lai_s" => {
                    config.refine_onelayer_lnd[1] = parse_canonical_bool(field, value)?
                }
                "refine_slope_m" => {
                    config.refine_onelayer_lnd[2] = parse_canonical_bool(field, value)?
                }
                "refine_slope_s" => {
                    config.refine_onelayer_lnd[3] = parse_canonical_bool(field, value)?
                }
                "refine_dem_m" => {
                    config.refine_onelayer_lnd[4] = parse_canonical_bool(field, value)?
                }
                "refine_dem_s" => {
                    config.refine_onelayer_lnd[5] = parse_canonical_bool(field, value)?
                }
                "refine_slope_max_m" => {
                    config.refine_onelayer_lnd[6] = parse_canonical_bool(field, value)?
                }
                "refine_slope_max_s" => {
                    config.refine_onelayer_lnd[7] = parse_canonical_bool(field, value)?
                }
                "refine_k_s_m" => {
                    config.refine_twolayer_lnd[0] = parse_canonical_bool(field, value)?
                }
                "refine_k_s_s" => {
                    config.refine_twolayer_lnd[1] = parse_canonical_bool(field, value)?
                }
                "refine_k_solids_m" => {
                    config.refine_twolayer_lnd[2] = parse_canonical_bool(field, value)?
                }
                "refine_k_solids_s" => {
                    config.refine_twolayer_lnd[3] = parse_canonical_bool(field, value)?
                }
                "refine_tkdry_m" => {
                    config.refine_twolayer_lnd[4] = parse_canonical_bool(field, value)?
                }
                "refine_tkdry_s" => {
                    config.refine_twolayer_lnd[5] = parse_canonical_bool(field, value)?
                }
                "refine_tksatf_m" => {
                    config.refine_twolayer_lnd[6] = parse_canonical_bool(field, value)?
                }
                "refine_tksatf_s" => {
                    config.refine_twolayer_lnd[7] = parse_canonical_bool(field, value)?
                }
                "refine_tksatu_m" => {
                    config.refine_twolayer_lnd[8] = parse_canonical_bool(field, value)?
                }
                "refine_tksatu_s" => {
                    config.refine_twolayer_lnd[9] = parse_canonical_bool(field, value)?
                }
                "refine_sst_m" => {
                    config.refine_onelayer_ocn[0] = parse_canonical_bool(field, value)?
                }
                "refine_sst_s" => {
                    config.refine_onelayer_ocn[1] = parse_canonical_bool(field, value)?
                }
                "refine_ssh_m" => {
                    config.refine_onelayer_ocn[2] = parse_canonical_bool(field, value)?
                }
                "refine_ssh_s" => {
                    config.refine_onelayer_ocn[3] = parse_canonical_bool(field, value)?
                }
                "refine_eke_m" => {
                    config.refine_onelayer_ocn[4] = parse_canonical_bool(field, value)?
                }
                "refine_eke_s" => {
                    config.refine_onelayer_ocn[5] = parse_canonical_bool(field, value)?
                }
                "refine_sea_slope_m" => {
                    config.refine_onelayer_ocn[6] = parse_canonical_bool(field, value)?
                }
                "refine_sea_slope_s" => {
                    config.refine_onelayer_ocn[7] = parse_canonical_bool(field, value)?
                }
                "refine_typhoon_m" => {
                    config.refine_onelayer_atmos[0] = parse_canonical_bool(field, value)?
                }
                "refine_typhoon_s" => {
                    config.refine_onelayer_atmos[1] = parse_canonical_bool(field, value)?
                }
                "th_num_landtypes" => config.th_num_landtypes = parse_i32(field, value)?,
                "th_area_mainland" => config.th_area_mainland = parse_f64(field, value)?,
                "th_lai_m" => config.th_onelayer_lnd[0] = parse_f64(field, value)?,
                "th_lai_s" => config.th_onelayer_lnd[1] = parse_f64(field, value)?,
                "th_slope_m" => config.th_onelayer_lnd[2] = parse_f64(field, value)?,
                "th_slope_s" => config.th_onelayer_lnd[3] = parse_f64(field, value)?,
                "th_dem_m" => config.th_onelayer_lnd[4] = parse_f64(field, value)?,
                "th_dem_s" => config.th_onelayer_lnd[5] = parse_f64(field, value)?,
                "th_slope_max_m" => config.th_onelayer_lnd[6] = parse_f64(field, value)?,
                "th_slope_max_s" => config.th_onelayer_lnd[7] = parse_f64(field, value)?,
                "th_k_s_m" => {
                    config.th_twolayer_lnd[0] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[0])?
                }
                "th_k_s_s" => {
                    config.th_twolayer_lnd[1] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[1])?
                }
                "th_k_solids_m" => {
                    config.th_twolayer_lnd[2] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[2])?
                }
                "th_k_solids_s" => {
                    config.th_twolayer_lnd[3] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[3])?
                }
                "th_tkdry_m" => {
                    config.th_twolayer_lnd[4] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[4])?
                }
                "th_tkdry_s" => {
                    config.th_twolayer_lnd[5] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[5])?
                }
                "th_tksatf_m" => {
                    config.th_twolayer_lnd[6] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[6])?
                }
                "th_tksatf_s" => {
                    config.th_twolayer_lnd[7] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[7])?
                }
                "th_tksatu_m" => {
                    config.th_twolayer_lnd[8] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[8])?
                }
                "th_tksatu_s" => {
                    config.th_twolayer_lnd[9] =
                        parse_f64_array(field, value, config.th_twolayer_lnd[9])?
                }
                "th_sea_ratio" => {
                    config.th_sea_ratio = parse_f64_array(field, value, config.th_sea_ratio)?
                }
                "th_sst_m" => config.th_onelayer_ocn[0] = parse_f64(field, value)?,
                "th_sst_s" => config.th_onelayer_ocn[1] = parse_f64(field, value)?,
                "th_ssh_m" => config.th_onelayer_ocn[2] = parse_f64(field, value)?,
                "th_ssh_s" => config.th_onelayer_ocn[3] = parse_f64(field, value)?,
                "th_eke_m" => config.th_onelayer_ocn[4] = parse_f64(field, value)?,
                "th_eke_s" => config.th_onelayer_ocn[5] = parse_f64(field, value)?,
                "th_sea_slope_m" => config.th_onelayer_ocn[6] = parse_f64(field, value)?,
                "th_sea_slope_s" => config.th_onelayer_ocn[7] = parse_f64(field, value)?,
                "th_typhoon_m" => config.th_onelayer_atmos[0] = parse_f64(field, value)?,
                "th_typhoon_s" => config.th_onelayer_atmos[1] = parse_f64(field, value)?,
                _ => return Err(format!("unknown &mkrefine field '{field}'")),
            }
        }

        config.validate_like_read_nl(mesh_type, mode_grid, external_field)?;
        Ok(config)
    }
}
