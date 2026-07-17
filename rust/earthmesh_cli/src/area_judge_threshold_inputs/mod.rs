mod data_read;
mod groups;
mod paths;
mod read;

use earthmesh_core::RefineConfig;

pub use data_read::{data_read_onelayer_one_based, data_read_twolayer_one_based};
pub(crate) use data_read::{
    numeric_missing_values, reject_invalid_threshold_values, threshold_latitude_order,
    threshold_longitude_coordinates, LatitudeOrder,
};
pub use groups::{
    threshold_read_atmos_one_based, threshold_read_lnd_one_based, threshold_read_ocn_one_based,
};
pub use read::read_area_judge_threshold_inputs_one_based;

#[derive(Clone, Copy)]
struct AreaJudge2DThresholdName {
    file_stem: &'static str,
    var_name: &'static str,
    output_name: &'static str,
}

const fn same_name(name: &'static str) -> AreaJudge2DThresholdName {
    AreaJudge2DThresholdName {
        file_stem: name,
        var_name: name,
        output_name: name,
    }
}

const AREA_JUDGE_LAND_ONELAYER_NAMES: [AreaJudge2DThresholdName; 4] = [
    same_name("lai"),
    same_name("slope_avg"),
    AreaJudge2DThresholdName {
        file_stem: "dem",
        var_name: "topo",
        output_name: "dem",
    },
    same_name("slope_max"),
];
const AREA_JUDGE_LAND_TWOLAYER_NAMES: [&str; 5] = ["k_s", "k_solids", "tkdry", "tksatf", "tksatu"];
const AREA_JUDGE_OCEAN_ONELAYER_NAMES: [AreaJudge2DThresholdName; 4] = [
    same_name("sst"),
    same_name("ssh"),
    same_name("eke"),
    same_name("sea_slope"),
];
const AREA_JUDGE_ATMOS_ONELAYER_NAMES: [AreaJudge2DThresholdName; 1] = [same_name("typhoon")];

fn area_judge_refine_flag_pair_enabled(flags: &[bool], pair_index: usize) -> bool {
    flags.get(2 * pair_index).copied().unwrap_or(false)
        || flags.get(2 * pair_index + 1).copied().unwrap_or(false)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AreaJudgeThresholdFieldSpec {
    pub file_stem: String,
    pub var_name: String,
    pub threshold: f64,
}

pub(crate) fn enabled_mean_threshold_field_specs(
    refine: &RefineConfig,
    mesh_type: &str,
) -> Vec<AreaJudgeThresholdFieldSpec> {
    let mut specs = Vec::new();
    if matches!(
        mesh_type,
        "landmesh" | "oceanmesh" | "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh"
    ) {
        push_land_threshold_specs(refine, &mut specs);
        push_ocean_threshold_specs(refine, &mut specs);
        push_atmos_threshold_specs(refine, &mut specs);
    }
    specs
}

pub(crate) fn enabled_std_threshold_field_specs(
    refine: &RefineConfig,
    mesh_type: &str,
) -> Vec<AreaJudgeThresholdFieldSpec> {
    let mut specs = Vec::new();
    if matches!(
        mesh_type,
        "landmesh" | "oceanmesh" | "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh"
    ) {
        push_land_std_threshold_specs(refine, &mut specs);
        push_ocean_std_threshold_specs(refine, &mut specs);
        push_atmos_std_threshold_specs(refine, &mut specs);
    }
    specs
}

fn push_land_threshold_specs(refine: &RefineConfig, specs: &mut Vec<AreaJudgeThresholdFieldSpec>) {
    push_onelayer_threshold_specs(
        specs,
        &AREA_JUDGE_LAND_ONELAYER_NAMES,
        &refine.refine_onelayer_lnd,
        &refine.th_onelayer_lnd,
    );
    push_twolayer_threshold_specs(
        specs,
        &AREA_JUDGE_LAND_TWOLAYER_NAMES,
        &refine.refine_twolayer_lnd,
        &refine.th_twolayer_lnd,
    );
}

fn push_land_std_threshold_specs(
    refine: &RefineConfig,
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
) {
    push_onelayer_std_threshold_specs(
        specs,
        &AREA_JUDGE_LAND_ONELAYER_NAMES,
        &refine.refine_onelayer_lnd,
        &refine.th_onelayer_lnd,
    );
    push_twolayer_std_threshold_specs(
        specs,
        &AREA_JUDGE_LAND_TWOLAYER_NAMES,
        &refine.refine_twolayer_lnd,
        &refine.th_twolayer_lnd,
    );
}

fn push_ocean_threshold_specs(refine: &RefineConfig, specs: &mut Vec<AreaJudgeThresholdFieldSpec>) {
    push_onelayer_threshold_specs(
        specs,
        &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
        &refine.refine_onelayer_ocn,
        &refine.th_onelayer_ocn,
    );
}

fn push_ocean_std_threshold_specs(
    refine: &RefineConfig,
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
) {
    push_onelayer_std_threshold_specs(
        specs,
        &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
        &refine.refine_onelayer_ocn,
        &refine.th_onelayer_ocn,
    );
}

fn push_atmos_threshold_specs(refine: &RefineConfig, specs: &mut Vec<AreaJudgeThresholdFieldSpec>) {
    push_onelayer_threshold_specs(
        specs,
        &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
        &refine.refine_onelayer_atmos,
        &refine.th_onelayer_atmos,
    );
}

fn push_atmos_std_threshold_specs(
    refine: &RefineConfig,
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
) {
    push_onelayer_std_threshold_specs(
        specs,
        &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
        &refine.refine_onelayer_atmos,
        &refine.th_onelayer_atmos,
    );
}

fn push_onelayer_threshold_specs(
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
    names: &[AreaJudge2DThresholdName],
    flags: &[bool],
    thresholds: &[f64],
) {
    for (index, name) in names.iter().enumerate() {
        let mean_slot = 2 * index;
        if flags.get(mean_slot).copied().unwrap_or(false) {
            if let Some(&threshold) = thresholds.get(mean_slot) {
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.file_stem.to_string(),
                    var_name: name.var_name.to_string(),
                    threshold,
                });
            }
        }
    }
}

fn push_onelayer_std_threshold_specs(
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
    names: &[AreaJudge2DThresholdName],
    flags: &[bool],
    thresholds: &[f64],
) {
    for (index, name) in names.iter().enumerate() {
        let std_slot = 2 * index + 1;
        if flags.get(std_slot).copied().unwrap_or(false) {
            if let Some(&threshold) = thresholds.get(std_slot) {
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.file_stem.to_string(),
                    var_name: name.var_name.to_string(),
                    threshold,
                });
            }
        }
    }
}

fn push_twolayer_std_threshold_specs(
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
    names: &[&str],
    flags: &[bool],
    thresholds: &[[f64; 2]],
) {
    for (index, &name) in names.iter().enumerate() {
        let std_slot = 2 * index + 1;
        if flags.get(std_slot).copied().unwrap_or(false) {
            if let Some(&[layer1, layer2]) = thresholds.get(std_slot) {
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.to_string(),
                    var_name: format!("{name}_l1"),
                    threshold: layer1,
                });
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.to_string(),
                    var_name: format!("{name}_l2"),
                    threshold: layer2,
                });
            }
        }
    }
}

fn push_twolayer_threshold_specs(
    specs: &mut Vec<AreaJudgeThresholdFieldSpec>,
    names: &[&'static str],
    flags: &[bool],
    thresholds: &[[f64; 2]],
) {
    for (index, &name) in names.iter().enumerate() {
        let mean_slot = 2 * index;
        if flags.get(mean_slot).copied().unwrap_or(false) {
            if let Some(&[layer1, layer2]) = thresholds.get(mean_slot) {
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.to_string(),
                    var_name: format!("{name}_l1"),
                    threshold: layer1,
                });
                specs.push(AreaJudgeThresholdFieldSpec {
                    file_stem: name.to_string(),
                    var_name: format!("{name}_l2"),
                    threshold: layer2,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabled_specs_use_mean_threshold_slots_only() {
        let mut refine = RefineConfig::default();
        refine.refine_onelayer_lnd[0] = true;
        refine.refine_onelayer_lnd[1] = true;
        refine.th_onelayer_lnd[0] = 5.0;
        refine.th_onelayer_lnd[1] = 7.0;

        let specs = enabled_mean_threshold_field_specs(&refine, "landmesh");
        assert_eq!(
            specs,
            vec![AreaJudgeThresholdFieldSpec {
                file_stem: "lai".to_string(),
                var_name: "lai".to_string(),
                threshold: 5.0,
            }]
        );
    }

    #[test]
    fn enabled_specs_include_std_threshold_slots() {
        let mut refine = RefineConfig::default();
        refine.refine_onelayer_lnd[1] = true;
        refine.th_onelayer_lnd[1] = 7.0;

        let specs = enabled_std_threshold_field_specs(&refine, "landmesh");
        assert_eq!(
            specs,
            vec![AreaJudgeThresholdFieldSpec {
                file_stem: "lai".to_string(),
                var_name: "lai".to_string(),
                threshold: 7.0,
            }]
        );
    }

    #[test]
    fn supported_meshes_keep_every_enabled_threshold_family() {
        let mut refine = RefineConfig::default();
        refine.refine_onelayer_lnd.fill(true);
        refine.refine_twolayer_lnd.fill(true);
        refine.refine_onelayer_ocn.fill(true);
        refine.refine_onelayer_atmos.fill(true);

        let expected_mean = enabled_mean_threshold_field_specs(&refine, "earthmesh");
        let expected_std = enabled_std_threshold_field_specs(&refine, "earthmesh");
        assert_eq!(expected_mean.len(), 19);
        assert_eq!(expected_std.len(), 19);

        for mesh_type in ["landmesh", "oceanmesh", "atmos", "atmosmesh", "LOCmesh"] {
            assert_eq!(
                enabled_mean_threshold_field_specs(&refine, mesh_type),
                expected_mean,
                "mean thresholds were filtered for {mesh_type}"
            );
            assert_eq!(
                enabled_std_threshold_field_specs(&refine, mesh_type),
                expected_std,
                "standard-deviation thresholds were filtered for {mesh_type}"
            );
        }
    }
}
