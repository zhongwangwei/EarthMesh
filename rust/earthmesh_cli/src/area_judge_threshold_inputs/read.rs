use std::io;

use earthmesh_mesh::AreaJudgeSourceBounds;

use super::{
    groups::{
        read_area_judge_threshold_2d_group_one_based,
        read_area_judge_threshold_2layer_group_one_based,
    },
    AREA_JUDGE_ATMOS_ONELAYER_NAMES, AREA_JUDGE_LAND_ONELAYER_NAMES,
    AREA_JUDGE_LAND_TWOLAYER_NAMES, AREA_JUDGE_OCEAN_ONELAYER_NAMES,
};
use crate::{
    grid_covers_area_judge_bounds_one_based, AreaJudgeThresholdInputsReport,
    AreaJudgeThresholdReadConfig,
};

fn crop_area_judge_landtypes_one_based(
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<Vec<Vec<i32>>> {
    grid_covers_area_judge_bounds_one_based("landtypes_global", landtypes_global, bounds)?;
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let mut landtypes = vec![vec![0; nlats_select + 1]; nlons_select + 1];
    for lon_offset in 0..nlons_select {
        for lat_offset in 0..nlats_select {
            landtypes[lon_offset + 1][lat_offset + 1] = landtypes_global
                [bounds.minlon_source + lon_offset][bounds.maxlat_source + lat_offset];
        }
    }
    Ok(landtypes)
}

/// Read and crop threshold inputs after calculated `Area_judge` refine bounds are known.
pub fn read_area_judge_threshold_inputs_one_based(
    config: AreaJudgeThresholdReadConfig<'_>,
    landtypes_global: &[Vec<i32>],
    bounds: AreaJudgeSourceBounds,
) -> io::Result<AreaJudgeThresholdInputsReport> {
    if bounds.maxlon_source < bounds.minlon_source || bounds.minlat_source < bounds.maxlat_source {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Area_judge threshold bounds lon {}..{} lat {}..{}",
                bounds.minlon_source,
                bounds.maxlon_source,
                bounds.maxlat_source,
                bounds.minlat_source
            ),
        ));
    }
    let nlons_select = bounds.maxlon_source - bounds.minlon_source + 1;
    let nlats_select = bounds.minlat_source - bounds.maxlat_source + 1;
    let landtypes = crop_area_judge_landtypes_one_based(landtypes_global, bounds)?;

    if !matches!(
        config.mesh_type,
        "landmesh" | "oceanmesh" | "atmos" | "atmosmesh" | "LOCmesh" | "earthmesh"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unsupported Area_judge threshold mesh_type {}",
                config.mesh_type
            ),
        ));
    }
    let land_onelayer = read_area_judge_threshold_2d_group_one_based(
        config.threshold_dir,
        &AREA_JUDGE_LAND_ONELAYER_NAMES,
        config.refine_onelayer_lnd,
        bounds,
    )?;
    let land_twolayer = read_area_judge_threshold_2layer_group_one_based(
        config.threshold_dir,
        &AREA_JUDGE_LAND_TWOLAYER_NAMES,
        config.refine_twolayer_lnd,
        bounds,
    )?;
    let ocean_onelayer = read_area_judge_threshold_2d_group_one_based(
        config.threshold_dir,
        &AREA_JUDGE_OCEAN_ONELAYER_NAMES,
        config.refine_onelayer_ocn,
        bounds,
    )?;
    let atmos_onelayer = read_area_judge_threshold_2d_group_one_based(
        config.threshold_dir,
        &AREA_JUDGE_ATMOS_ONELAYER_NAMES,
        config.refine_onelayer_atmos,
        bounds,
    )?;

    Ok(AreaJudgeThresholdInputsReport {
        bounds,
        nlons_select,
        nlats_select,
        landtypes,
        land_onelayer,
        land_twolayer,
        ocean_onelayer,
        atmos_onelayer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn ocean_and_atmosphere_read_enabled_land_thresholds() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_cross_domain_threshold_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        write_lai(&root.join("lai.nc"));

        let bounds = AreaJudgeSourceBounds {
            minlon_source: 1,
            maxlon_source: 1,
            maxlat_source: 1,
            minlat_source: 1,
        };
        let landtypes = vec![vec![0, 0], vec![0, 1]];
        let mut land_flags = [false; 8];
        land_flags[0] = true;
        let no_land_two = [false; 10];
        let no_ocean = [false; 8];
        let no_atmos = [false; 2];

        for mesh_type in ["oceanmesh", "atmosmesh"] {
            let report = read_area_judge_threshold_inputs_one_based(
                AreaJudgeThresholdReadConfig {
                    threshold_dir: &root,
                    mesh_type,
                    refine_onelayer_lnd: &land_flags,
                    refine_twolayer_lnd: &no_land_two,
                    refine_onelayer_ocn: &no_ocean,
                    refine_onelayer_atmos: &no_atmos,
                },
                &landtypes,
                bounds,
            )
            .unwrap_or_else(|error| panic!("{mesh_type} did not read LAI: {error}"));
            assert_eq!(report.land_onelayer[0].as_ref().unwrap().values[1][1], 2.0);
        }
        let _ = std::fs::remove_dir_all(root);
    }

    fn write_lai(path: &Path) {
        let mut file = crate::create_netcdf_quiet(path).unwrap();
        file.add_dimension("longitude", 1).unwrap();
        file.add_dimension("latitude", 1).unwrap();
        file.add_variable::<f64>("lai", &["longitude", "latitude"])
            .unwrap()
            .put_value(2.0, (0, 0))
            .unwrap();
    }
}
