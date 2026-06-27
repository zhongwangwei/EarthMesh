mod data_read;
mod groups;
mod paths;
mod read;

pub use data_read::{data_read_onelayer_fortran_indexed, data_read_twolayer_fortran_indexed};
pub use groups::{
    threshold_read_atmos_fortran_indexed, threshold_read_lnd_fortran_indexed,
    threshold_read_ocn_fortran_indexed,
};
pub use read::read_area_judge_threshold_inputs_fortran_indexed;

const AREA_JUDGE_LAND_ONELAYER_NAMES: [&str; 2] = ["lai", "slope_avg"];
const AREA_JUDGE_LAND_TWOLAYER_NAMES: [&str; 5] = ["k_s", "k_solids", "tkdry", "tksatf", "tksatu"];
const AREA_JUDGE_OCEAN_ONELAYER_NAMES: [&str; 4] = ["sst", "ssh", "eke", "sea_slope"];
const AREA_JUDGE_ATMOS_ONELAYER_NAMES: [&str; 1] = ["typhoon"];

fn area_judge_refine_flag_pair_enabled(flags: &[bool], pair_index: usize) -> bool {
    flags.get(2 * pair_index).copied().unwrap_or(false)
        || flags.get(2 * pair_index + 1).copied().unwrap_or(false)
}
