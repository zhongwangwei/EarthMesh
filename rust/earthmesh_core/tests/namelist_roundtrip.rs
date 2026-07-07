use earthmesh_core::{
    lower_datalayers_namelist, DataLayerRole, DataLayersNamelist, EarthmeshConfig, QualityNamelist,
    RefineConfig, ThresholdVar,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

// 一个最小但通过 validate_like_read_nl 的 &mkgrd 块：
// atmosmesh → output_format 必须是 MPAS/MPAS-Simple；gridnum_perdegree 必须是 120/240。
const SAMPLE_MKGRD: &str = "\
&mkgrd
  NL%EXPNME = 'ATMOS_hex_N64_refine2_global'
  NL%base_dir = './cases/'
  NL%mesh_type = 'atmosmesh'
  NL%mode_grid = 'hex'
  NL%mode_file = 'none'
  NL%mode_file_description = 'none'
  NL%NXP = 64
  NL%refine = .TRUE.
  NL%gridnum_perdegree = 120
  NL%niter = 5000
  NL%beta = 1.0
  NL%relax = 0.035
  NL%openmp = 8
  NL%landtype_file = './input/landtype_usgs_update.nc'
  NL%mask_domain_global = .TRUE.
  NL%mask_domain_type = 'circle'
  NL%mask_domain_fprefix = 'none'
  NL%mask_restart = .FALSE.
  NL%mask_sea_ratio = 0.5
  NL%mask_patch_on = .FALSE.
  NL%mask_patch_type = 'close'
  NL%mask_patch_fprefix = 'none'
  NL%output_format = 'MPAS'
/
";

#[test]
fn mkgrd_namelist_round_trips_through_writer() {
    let original = EarthmeshConfig::from_mkgrd_namelist(SAMPLE_MKGRD).expect("sample parses");
    let rendered = original.to_mkgrd_namelist();
    let reparsed =
        EarthmeshConfig::from_mkgrd_namelist(&rendered).expect("rendered output re-parses");
    assert_eq!(original, reparsed, "parse → write → parse must be identity");
}

#[test]
fn mkgrd_writer_escapes_single_quotes_in_string_values() {
    let mut original = EarthmeshConfig::from_mkgrd_namelist(SAMPLE_MKGRD).expect("sample parses");
    original.experiment_name = "case's quoted".to_string();
    original.base_dir = "./case's/".to_string();
    original.landtype_file = "./input/o'brien.nc".to_string();

    let rendered = original.to_mkgrd_namelist();
    assert!(
        rendered.contains("case''s quoted"),
        "rendered namelist should use Fortran doubled quotes: {rendered}"
    );
    let reparsed =
        EarthmeshConfig::from_mkgrd_namelist(&rendered).expect("escaped output re-parses");

    assert_eq!(reparsed.experiment_name, original.experiment_name);
    assert_eq!(reparsed.base_dir, original.base_dir);
    assert_eq!(reparsed.landtype_file, original.landtype_file);
}

fn read_example(relative: &str) -> (PathBuf, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let contents =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    (path, contents)
}

fn assert_example_round_trips(relative: &str) {
    let (path, contents) = read_example(relative);
    let original = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    let reparsed = EarthmeshConfig::from_mkgrd_namelist(&original.to_mkgrd_namelist())
        .unwrap_or_else(|e| panic!("re-parse {}: {e}", path.display()));
    assert_eq!(original, reparsed, "round-trip mismatch for {}", relative);
}

#[test]
fn default_example_namelists_round_trip() {
    assert_example_round_trips("examples/default/atmosphere_hex_global.nml");
    assert_example_round_trips("examples/default/land_hex_global.nml");
    assert_example_round_trips("examples/default/ocean_hex_global.nml");
}

// 一个最小但通过 RefineConfig::validate_like_read_nl 的 &mkrefine 块：
// Istransition=true 允许非 tri 网格；refine_spc=true 使 refine_setting='specified'，
// 无需逐判据阈值校验。
const SAMPLE_MKREFINE: &str = "\
&mkrefine
  RL%weak_concav_eliminate = .TRUE.
  RL%Istransition = .TRUE.
  RL%HALO = 4, 4, 3
  RL%max_transition_row = 4, 4, 3
  RL%SpringGlobal_type = 1
  RL%SpringRegional_type = 0
  RL%num_rc = 1
  RL%set_dis_type = 'linear'
  RL%vertex_pretect_layers = 15
  RL%niter_refine = 5000
  RL%refine_spc = .TRUE.
  RL%refine_cal = .FALSE.
  RL%max_iter_spc = 2
  RL%mask_refine_spc_type = 'circle'
  RL%mask_refine_spc_fprefix = './input/refine_spc_circle'
/
";

#[test]
fn mkrefine_namelist_round_trips_through_writer() {
    let original = RefineConfig::from_mkrefine_namelist(SAMPLE_MKREFINE, "landmesh", "hex")
        .expect("sample parses");
    let rendered = original.to_mkrefine_namelist();
    let reparsed = RefineConfig::from_mkrefine_namelist(&rendered, "landmesh", "hex")
        .expect("rendered output re-parses");
    assert_eq!(original, reparsed, "parse → write → parse must be identity");
}

#[test]
fn mkrefine_writer_escapes_single_quotes_in_string_values() {
    let mut original = RefineConfig::from_mkrefine_namelist(SAMPLE_MKREFINE, "landmesh", "hex")
        .expect("sample parses");
    original.mask_refine_spc_fprefix = "./input/refine'spc".to_string();
    original.threshold_dir = "./threshold/o'brien".to_string();

    let rendered = original.to_mkrefine_namelist();
    assert!(
        rendered.contains("refine''spc"),
        "rendered namelist should use Fortran doubled quotes: {rendered}"
    );
    let reparsed = RefineConfig::from_mkrefine_namelist(&rendered, "landmesh", "hex")
        .expect("escaped output re-parses");

    assert_eq!(
        reparsed.mask_refine_spc_fprefix,
        original.mask_refine_spc_fprefix
    );
    assert_eq!(reparsed.threshold_dir, original.threshold_dir);
}

fn assert_example_mkrefine_round_trips(relative: &str) {
    let (path, contents) = read_example(relative);
    // mesh_type / mode_grid come from the &mkgrd block and gate &mkrefine validation.
    let base = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .unwrap_or_else(|e| panic!("parse mkgrd {}: {e}", path.display()));
    let original =
        RefineConfig::from_mkrefine_namelist(&contents, &base.mesh_type, &base.mode_grid)
            .unwrap_or_else(|e| panic!("parse mkrefine {}: {e}", path.display()));
    let reparsed = RefineConfig::from_mkrefine_namelist(
        &original.to_mkrefine_namelist(),
        &base.mesh_type,
        &base.mode_grid,
    )
    .unwrap_or_else(|e| panic!("re-parse mkrefine {}: {e}", path.display()));
    assert_eq!(
        original, reparsed,
        "mkrefine round-trip mismatch for {}",
        relative
    );
}

#[test]
fn default_example_mkrefine_round_trips() {
    assert_example_mkrefine_round_trips("examples/default/atmosphere_hex_global.nml");
    assert_example_mkrefine_round_trips("examples/default/land_hex_global.nml");
    assert_example_mkrefine_round_trips("examples/default/ocean_hex_global.nml");
}

const SAMPLE_QUALITY: &str = "\
&quality
  NL%min_angle_warn_deg = 25
  NL%min_angle_fail_deg = 5
  NL%angle_deviation_warn_deg = 30
  NL%aspect_ratio_warn = 3.5
  NL%aspect_ratio_fail = 10
  NL%cell_edge_cv_warn = 0.3
  NL%area_cv_warn = 1.2
  NL%max_adjacent_resolution_ratio_warn = 1.8
  NL%worst_cells_limit = 100
  NL%on_violation = 'block'
/
";

#[test]
fn quality_namelist_round_trips_through_writer() {
    let original = QualityNamelist::from_quality_namelist(SAMPLE_QUALITY).expect("sample parses");
    let rendered = original.to_quality_namelist();
    let reparsed =
        QualityNamelist::from_quality_namelist(&rendered).expect("rendered output re-parses");
    assert_eq!(original, reparsed, "parse → write → parse must be identity");
    assert_eq!(original.on_violation, "block");
    assert_eq!(original.min_angle_warn_deg, 25.0);
    assert_eq!(original.angle_deviation_warn_deg, 30.0);
    assert_eq!(original.cell_edge_cv_warn, 0.3);
    assert_eq!(original.worst_cells_limit, 100);
}

#[test]
fn quality_namelist_absent_block_yields_defaults() {
    // A namelist without a &quality block parses to defaults that mirror
    // earthmesh_quality::QualityThresholds::default() (back-compat for old files).
    let parsed = QualityNamelist::from_quality_namelist("&mkgrd\n/\n").expect("parses");
    assert_eq!(parsed, QualityNamelist::default());
    assert_eq!(parsed.min_angle_warn_deg, 20.0);
    assert_eq!(parsed.min_angle_fail_deg, 5.0);
    assert_eq!(parsed.angle_deviation_warn_deg, 35.0);
    assert_eq!(parsed.aspect_ratio_warn, 4.0);
    assert_eq!(parsed.cell_edge_cv_warn, 0.35);
    assert_eq!(parsed.worst_cells_limit, 50);
    assert_eq!(parsed.on_violation, "warn");
}

const SAMPLE_DATALAYERS: &str = "\
&datalayers
  NL%layer = 'landcover|landtype|./input/landtype.nc|landtype|T|T'
  NL%layer = 'lai|threshold:lai|./threshold/lai.nc||T|F'
  NL%layer = 'ks|threshold:k_s|./threshold/k_s.nc||T|F'
/
";

#[test]
fn datalayers_namelist_round_trips_through_writer() {
    let original = DataLayersNamelist::from_datalayers_namelist(SAMPLE_DATALAYERS);
    assert_eq!(original.layers.len(), 3);
    let rendered = original.to_datalayers_namelist();
    let reparsed = DataLayersNamelist::from_datalayers_namelist(&rendered);
    assert_eq!(original, reparsed, "parse → write → parse must be identity");
    assert_eq!(original.layers[0].role, DataLayerRole::LandType);
    assert_eq!(
        original.layers[1].role,
        DataLayerRole::ThresholdField(ThresholdVar::Lai)
    );
    assert_eq!(
        original.layers[2].role,
        DataLayerRole::ThresholdField(ThresholdVar::Ks)
    );
    assert_eq!(original.layers[0].var.as_deref(), Some("landtype"));
    assert_eq!(original.layers[1].var, None);
    assert!(original.layers[0].required);
    assert!(!original.layers[1].required);
}

#[test]
fn datalayers_absent_block_is_empty() {
    let parsed = DataLayersNamelist::from_datalayers_namelist("&mkgrd\n/\n");
    assert!(parsed.layers.is_empty());
}

#[test]
fn threshold_var_stems_match_engine_contract() {
    // Stems must equal the engine's AREA_JUDGE_*_NAMES (cli/lib.rs:20724-26).
    assert_eq!(ThresholdVar::Slope.file_stem(), "slope_avg");
    assert_eq!(ThresholdVar::Dem.file_stem(), "dem");
    assert_eq!(ThresholdVar::SlopeMax.file_stem(), "slope_max");
    assert_eq!(ThresholdVar::Ks.file_stem(), "k_s");
    assert_eq!(ThresholdVar::SeaSlope.file_stem(), "sea_slope");
    assert!(ThresholdVar::Ks.is_two_layer());
    assert!(!ThresholdVar::Dem.is_two_layer());
    assert!(!ThresholdVar::Lai.is_two_layer());
    assert_eq!(ThresholdVar::from_stem("dem"), Some(ThresholdVar::Dem));
    assert_eq!(
        ThresholdVar::from_stem("slope_max"),
        Some(ThresholdVar::SlopeMax)
    );
    assert_eq!(
        ThresholdVar::from_stem("typhoon"),
        Some(ThresholdVar::Typhoon)
    );
    assert_eq!(ThresholdVar::from_stem("nope"), None);
}

#[test]
fn datalayers_lower_sets_landtype_and_refine_switches() {
    let dl = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers\n\
         NL%layer = 'lc|landtype|./in/landtype.nc|landtype|T|T'\n\
         NL%layer = 'lai|threshold:lai|./th/lai.nc||T|F'\n\
         NL%layer = 'dem|threshold:dem|./th/dem.nc||T|F'\n\
         NL%layer = 'ss|threshold:sea_slope|./th/sea_slope.nc||T|F'\n\
         /\n",
    );
    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    let report = dl.lower_into(&mut mkgrd, &mut refine);

    assert_eq!(mkgrd.landtype_file, "./in/landtype.nc");
    assert!(report.landtype_set);
    assert!(
        refine.refine_cal,
        "any enabled threshold enables calc refine"
    );
    // LAI → refine_onelayer_lnd[0] & [1]; slope (idx 2/3) untouched.
    assert!(refine.refine_onelayer_lnd[0] && refine.refine_onelayer_lnd[1]);
    let fresh = RefineConfig::default();
    assert_eq!(
        refine.refine_onelayer_lnd[2], fresh.refine_onelayer_lnd[2],
        "slope switch must be untouched"
    );
    // dem → refine_onelayer_lnd[4] & [5].
    assert!(refine.refine_onelayer_lnd[4] && refine.refine_onelayer_lnd[5]);
    // sea_slope → refine_onelayer_ocn[6] & [7].
    assert!(refine.refine_onelayer_ocn[6] && refine.refine_onelayer_ocn[7]);
    assert!(report.warnings.is_empty(), "matching stems => no warnings");
}

#[test]
fn datalayers_lower_warns_on_stem_mismatch_but_still_sets_switch() {
    let dl = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers\n  NL%layer = 'x|threshold:lai|./th/WRONG.nc||T|F'\n/\n",
    );
    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    let report = dl.lower_into(&mut mkgrd, &mut refine);

    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].contains("lai"));
    assert!(refine.refine_onelayer_lnd[0], "switch still set (lenient)");
}

#[test]
fn lower_datalayers_namelist_applies_and_reemits() {
    let nml = format!(
        "{SAMPLE_MKGRD}\
&datalayers
  NL%layer = 'lc|landtype|./in/landtype.nc|landtype|T|T'
  NL%layer = 'lai|threshold:lai|./th/lai.nc||T|F'
/
"
    );
    let out = lower_datalayers_namelist(&nml, Some("/fallback/th/")).expect("lower ok");
    assert!(out.namelist.contains("&mkgrd"));
    assert!(out.namelist.contains("&mkrefine"));
    assert!(
        out.namelist.contains("landtype_file = './in/landtype.nc'"),
        "LandType layer drives landtype_file"
    );
    assert!(!out.threshold_dir.trim().is_empty());
    assert_eq!(
        out.threshold_files,
        vec![("lai".to_string(), "./th/lai.nc".to_string())]
    );
}
