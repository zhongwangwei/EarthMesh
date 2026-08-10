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
fn mkgrd_namelist_rejects_unknown_fields() {
    let input = SAMPLE_MKGRD.replace("NL%relax = 0.035", "NL%relxa = 0.035");
    let error = EarthmeshConfig::from_mkgrd_namelist(&input).unwrap_err();
    assert!(error.contains("unknown &mkgrd field 'relxa'"), "{error}");
}

#[test]
fn mkgrd_namelist_accepts_native_method_c_extension_fields() {
    let input = SAMPLE_MKGRD.replace(
        "/\n",
        "  NL%mdomain = 5\n\
         NL%deltax = 1000.0\n\
         NL%ngrids = 2\n\
         NL%ngrdll(2) = 1\n\
         NL%grdrad(2,1) = 2500000.0\n\
         NL%grdlat(2,1) = 25.0\n\
         NL%grdlon(2,1) = 115.0\n\
         NL%gridplot_base = 2\n\
         NL%nsfcgrids = 1\n\
         NL%nsfcgrdll(1) = 1\n\
         NL%sfcgrdrad(1,1) = 500000.0\n\
         NL%sfcgrdlat(1,1) = 25.0\n\
         NL%sfcgrdlon(1,1) = 115.0\n\
         NL%sfcgridplot_base = 1\n\
         NL%sfcgrid_res_factor = 2\n/\n",
    );
    EarthmeshConfig::from_mkgrd_namelist(&input).expect("native Method-C fields must parse");
}

#[test]
fn mkgrd_close_boundary_spec_round_trips_when_non_default() {
    let mut original = EarthmeshConfig::from_mkgrd_namelist(SAMPLE_MKGRD).expect("sample parses");
    original.mask_domain_close_boundary =
        "spherical_chaikin:iterations=2,max_segment_angle_deg=0.25".to_string();

    let rendered = original.to_mkgrd_namelist();
    assert!(rendered.contains("mask_domain_close_boundary"));
    let reparsed =
        EarthmeshConfig::from_mkgrd_namelist(&rendered).expect("rendered output re-parses");
    assert_eq!(
        reparsed.mask_domain_close_boundary,
        original.mask_domain_close_boundary
    );
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
        "rendered namelist should use Canonical doubled quotes: {rendered}"
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
fn mkrefine_namelist_rejects_unknown_fields() {
    let input = SAMPLE_MKREFINE.replace("RL%niter_refine", "RL%niter_refin");
    let error = RefineConfig::from_mkrefine_namelist(&input, "landmesh", "hex").unwrap_err();
    assert!(
        error.contains("unknown &mkrefine field 'niter_refin'"),
        "{error}"
    );
}

#[test]
fn mkrefine_close_boundary_spec_round_trips_when_non_default() {
    let mut original = RefineConfig::from_mkrefine_namelist(SAMPLE_MKREFINE, "landmesh", "hex")
        .expect("sample parses");
    original.mask_refine_spc_close_boundary =
        "enclosing_cap:margin_km=20,max_radius_deg=80,max_segment_angle_deg=0.25".to_string();

    let rendered = original.to_mkrefine_namelist();
    assert!(rendered.contains("mask_refine_spc_close_boundary"));
    let reparsed = RefineConfig::from_mkrefine_namelist(&rendered, "landmesh", "hex")
        .expect("rendered output re-parses");
    assert_eq!(
        reparsed.mask_refine_spc_close_boundary,
        original.mask_refine_spc_close_boundary
    );
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
        "rendered namelist should use Canonical doubled quotes: {rendered}"
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
  NL%lepp_post_quality = .true.
  NL%lepp_post_quality_max_insertions = 12
  NL%lepp_post_quality_max_edge_km = 75
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
    assert_eq!(original.repair_batch_limit, 1);
    assert!(original.lepp_post_quality);
    assert_eq!(original.lepp_post_quality_max_insertions, 12);
    assert_eq!(original.lepp_post_quality_max_edge_km, 75.0);
}

#[test]
fn quality_namelist_rejects_unknown_fields() {
    let input = SAMPLE_QUALITY.replace("NL%area_cv_warn", "NL%area_cv_wran");
    let error = QualityNamelist::from_quality_namelist(&input).unwrap_err();
    assert!(
        error.contains("unknown &quality field 'area_cv_wran'"),
        "{error}"
    );
}

#[test]
fn quality_namelist_absent_block_yields_defaults() {
    // A namelist without a &quality block parses to defaults that mirror
    // earthmesh_quality::QualityThresholds::default() (back-compat for old files).
    let parsed = QualityNamelist::from_quality_namelist("&mkgrd\n/\n").expect("parses");
    assert_eq!(parsed, QualityNamelist::default());
    assert_eq!(parsed.min_angle_warn_deg, 25.0);
    assert_eq!(parsed.min_angle_fail_deg, 5.0);
    assert_eq!(parsed.angle_deviation_warn_deg, 35.0);
    assert_eq!(parsed.aspect_ratio_warn, 4.0);
    assert_eq!(parsed.cell_edge_cv_warn, 0.35);
    assert_eq!(parsed.worst_cells_limit, 50);
    assert_eq!(parsed.repair_batch_limit, 1);
    assert!(!parsed.lepp_post_quality);
    assert_eq!(parsed.lepp_post_quality_max_insertions, 50);
    assert_eq!(parsed.lepp_post_quality_max_edge_km, 0.0);
    assert_eq!(parsed.on_violation, "warn");
}

#[test]
fn quality_namelist_rejects_semantically_invalid_gates() {
    for (input, expected) in [
        (
            "&quality NL%on_violation='blok' /",
            "on_violation must be warn, block, or auto_refine",
        ),
        (
            "&quality NL%min_angle_fail_deg=NaN /",
            "min_angle_fail_deg must be finite",
        ),
        (
            "&quality NL%worst_cells_limit=-1 /",
            "worst_cells_limit must be non-negative",
        ),
        (
            "&quality NL%min_angle_fail_deg=30, NL%min_angle_warn_deg=20 /",
            "min_angle_fail_deg must not exceed min_angle_warn_deg",
        ),
        (
            "&quality NL%aspect_ratio_warn=5, NL%aspect_ratio_fail=4 /",
            "aspect_ratio_warn must not exceed aspect_ratio_fail",
        ),
        (
            "&quality NL%lepp_post_quality_max_insertions=0 /",
            "lepp_post_quality_max_insertions must be positive",
        ),
        (
            "&quality NL%lepp_post_quality_max_edge_km=-1 /",
            "lepp_post_quality_max_edge_km must be finite and non-negative",
        ),
    ] {
        let error = QualityNamelist::from_quality_namelist(input)
            .expect_err("invalid quality gates must not fail open");
        assert!(error.contains(expected), "{error}");
    }
}

const SAMPLE_DATALAYERS: &str = "\
&datalayers
  NL%layer = 'landcover|landtype|./input/landtype.nc|landtype|T|T'
  NL%layer = 'lai|threshold:lai|./threshold/lai.nc||T|F'
  NL%layer = 'ks|threshold:k_s|./threshold/k_s.nc||T|F'
/
";

#[test]
fn core_namelist_parsers_accept_inline_groups_and_assignments() {
    let mkgrd = EarthmeshConfig::from_mkgrd_namelist(
        "&mkgrd NL%expnme='inline,case', NL%nxp=4, NL%mesh_type='landmesh', NL%mode_grid='tri', NL%output_format='CoLM' /",
    )
    .expect("inline mkgrd");
    assert_eq!(mkgrd.experiment_name, "inline,case");
    assert_eq!(mkgrd.nxp, 4);

    let refine = RefineConfig::from_mkrefine_namelist(
        "&mkrefine RL%Istransition=.true., RL%HALO=4,4,3, RL%SpringGlobal_type=1, RL%SpringRegional_type=0, RL%refine_spc=.true., RL%max_iter_spc=2 /",
        "landmesh",
        "hex",
    )
    .expect("inline mkrefine");
    assert_eq!(&refine.halo[1..4], &[4, 4, 3]);
    assert_eq!(refine.refine_setting, "specified");

    let quality = QualityNamelist::from_quality_namelist(
        "&quality NL%min_angle_warn_deg=24, NL%worst_cells_limit=12, NL%on_violation='block' /",
    )
    .expect("inline quality");
    assert_eq!(quality.min_angle_warn_deg, 24.0);
    assert_eq!(quality.worst_cells_limit, 12);
    assert_eq!(quality.on_violation, "block");

    let layers = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers NL%layer='lc|landtype|./land.nc|landtype|T|T', NL%layer='lai|threshold:lai|./lai.nc||T|F' /",
    );
    assert_eq!(layers.layers.len(), 2);
    assert_eq!(
        layers.layers[1].role,
        DataLayerRole::ThresholdField(ThresholdVar::Lai)
    );
}

#[test]
fn core_namelist_parser_preserves_multiline_array_continuations() {
    let refine = RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n RL%Istransition=.true.\n RL%HALO=4,4,\n 3\n RL%SpringGlobal_type=1\n RL%SpringRegional_type=0\n RL%refine_spc=.true.\n RL%max_iter_spc=2\n/\n",
        "landmesh",
        "hex",
    )
    .expect("multiline mkrefine");
    assert_eq!(&refine.halo[1..4], &[4, 4, 3]);
}

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
    assert!(!original.layers[0].categorical_enabled);
    assert!(!original.layers[1].required);
    assert!(original.layers[1].mean_enabled);
    assert!(original.layers[1].std_enabled);
}

#[test]
fn landtype_mask_and_categorical_refinement_are_independent() {
    let mask_only = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers NL%layer='landcover|landtype|./land.nc|landtype|T|T|F|F|F' /",
    );
    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    mask_only.lower_into(&mut mkgrd, &mut refine);
    assert_eq!(mkgrd.landtype_file, "./land.nc");
    assert!(!refine.refine_num_landtypes);
    assert!(!refine.refine_cal);

    let explicit_landcover = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers NL%layer='landcover|landtype|./land.nc|landtype|T|T|F|F|T' /",
    );
    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    explicit_landcover.lower_into(&mut mkgrd, &mut refine);
    assert_eq!(mkgrd.landtype_file, "./land.nc");
    assert!(refine.refine_num_landtypes);
    assert!(refine.refine_cal);

    let lai_only = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers\n\
         NL%layer='landcover|landtype|./land.nc|landtype|T|T|F|F|F'\n\
         NL%layer='lai|threshold:lai|./lai.nc||T|F|T|F|F'\n/",
    );
    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    lai_only.lower_into(&mut mkgrd, &mut refine);
    assert!(refine.refine_cal);
    assert!(refine.refine_onelayer_lnd[0]);
    assert!(!refine.refine_onelayer_lnd[1]);
    assert!(!refine.refine_num_landtypes);
}

#[test]
fn datalayers_statistic_axes_preserve_legacy_defaults_and_explicit_switches() {
    let legacy = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers NL%layer='lai|threshold:lai|./lai.nc||T|F' /",
    );
    assert!(legacy.layers[0].mean_enabled);
    assert!(legacy.layers[0].std_enabled);

    let explicit = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers NL%layer='lai|threshold:lai|./lai.nc||T|F|T|F' /",
    );
    assert!(explicit.layers[0].mean_enabled);
    assert!(!explicit.layers[0].std_enabled);

    let rendered = explicit.to_datalayers_namelist();
    let reparsed = DataLayersNamelist::from_datalayers_namelist(&rendered);
    assert_eq!(explicit, reparsed, "explicit mean/std axes must round-trip");

    let mut mkgrd = EarthmeshConfig::default();
    let mut refine = RefineConfig::default();
    explicit.lower_into(&mut mkgrd, &mut refine);
    assert!(refine.refine_onelayer_lnd[0]);
    assert!(!refine.refine_onelayer_lnd[1]);
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
fn datalayers_parser_rejects_removed_specified_mask_role() {
    let dl = DataLayersNamelist::from_datalayers_namelist(
        "&datalayers\n  NL%layer = 'mask|specified_mask|./masks/refine.nc4||T|F'\n/\n",
    );
    assert!(dl.layers.is_empty());
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
    assert_eq!(out.threshold_dir, "/fallback/th/");
    assert_eq!(
        out.threshold_files,
        vec![("lai".to_string(), "./th/lai.nc".to_string())]
    );
}

#[test]
fn lower_datalayers_namelist_rejects_duplicate_threshold_outputs() {
    let nml = format!(
        "{SAMPLE_MKGRD}\
&datalayers
  NL%layer = 'lai-a|threshold:lai|./th/lai-a.nc||T|F'
  NL%layer = 'lai-b|threshold:lai|./th/lai-b.nc||T|F'
/
"
    );

    let error = lower_datalayers_namelist(&nml, Some("/fallback/th/")).unwrap_err();
    assert!(error.contains("enabled threshold field 'lai' is duplicated"));
}

#[test]
fn lower_datalayers_namelist_preserves_hfield_and_quality_groups() {
    let nml = format!(
        "{SAMPLE_MKGRD}{SAMPLE_MKREFINE}\
&hfield
  NL%hfield_on = .true.
  NL%hfield_g = 0.2
  NL%hfield_max_level = 3
/
{SAMPLE_QUALITY}\
&datalayers
  NL%layer = 'lai|threshold:lai|./th/lai.nc||T|F'
/
"
    );

    let out = lower_datalayers_namelist(&nml, Some("/fallback/th/"))
        .expect("datalayer lowering should succeed");

    assert!(out.namelist.contains("&hfield"), "{}", out.namelist);
    assert!(
        out.namelist.contains("NL%hfield_g = 0.2"),
        "{}",
        out.namelist
    );
    assert!(out.namelist.contains("&quality"), "{}", out.namelist);
    assert!(
        out.namelist.contains("NL%min_angle_warn_deg = 25"),
        "{}",
        out.namelist
    );
    assert!(
        !out.namelist.contains("&datalayers"),
        "the execution namelist must contain only engine-consumed groups: {}",
        out.namelist
    );
}

#[test]
fn lower_datalayers_namelist_preserves_native_method_c_assignments() {
    let native = "  NL%mdomain = 5\n\
                  NL%deltax = 1000.0\n\
                  NL%ngrids = 2\n\
                  NL%ngrdll(2) = 1\n\
                  NL%grdrad(2,1) = 2500000.0\n\
                  NL%grdlat(2,1) = 25.0\n\
                  NL%grdlon(2,1) = 115.0\n\
                  NL%gridplot_base = 2\n\
                  NL%nsfcgrids = 1\n\
                  NL%nsfcgrdll(1) = 1\n\
                  NL%sfcgrdrad(1,1) = 500000.0\n\
                  NL%sfcgrdlat(1,1) = 25.0\n\
                  NL%sfcgrdlon(1,1) = 115.0\n\
                  NL%sfcgridplot_base = 1\n\
                  NL%sfcgrid_res_factor = 2\n";
    let nml = format!(
        "{}&datalayers\n  NL%layer = 'lc|landtype|./in/landtype.nc|landtype|T|T'\n/\n",
        SAMPLE_MKGRD.replace("/\n", &format!("{native}/\n"))
    );

    let out = lower_datalayers_namelist(&nml, Some("/fallback/th/"))
        .expect("datalayer lowering should preserve native Method-C controls");

    for field in [
        "mdomain",
        "deltax",
        "ngrids",
        "ngrdll(2)",
        "grdrad(2,1)",
        "grdlat(2,1)",
        "grdlon(2,1)",
        "gridplot_base",
        "nsfcgrids",
        "nsfcgrdll(1)",
        "sfcgrdrad(1,1)",
        "sfcgrdlat(1,1)",
        "sfcgrdlon(1,1)",
        "sfcgridplot_base",
        "sfcgrid_res_factor",
    ] {
        assert!(
            out.namelist.contains(field),
            "missing {field}: {}",
            out.namelist
        );
    }
}

#[test]
fn lower_datalayers_inline_preserves_native_method_c_fields_without_duplicate_groups() {
    let nml = "&mkgrd NL%expnme='inline', NL%nxp=4, NL%mesh_type='landmesh', NL%mode_grid='tri', NL%output_format='CoLM', NL%mdomain=5, NL%ngrids=2, NL%ngrdll(2)=1, NL%grdrad(2,1)=2500000.0 /\n&datalayers NL%layer='lc|landtype|./in/landtype.nc|landtype|T|T' /";

    let out = lower_datalayers_namelist(nml, Some("/fallback/th/"))
        .expect("inline datalayer lowering should preserve native controls");

    assert_eq!(
        out.namelist.matches("&mkgrd").count(),
        1,
        "{}",
        out.namelist
    );
    assert_eq!(
        out.namelist.matches("&mkrefine").count(),
        1,
        "{}",
        out.namelist
    );
    for field in ["mdomain", "ngrids", "ngrdll(2)", "grdrad(2,1)"] {
        assert!(
            out.namelist.contains(field),
            "missing {field}: {}",
            out.namelist
        );
    }
}

#[test]
fn lower_datalayers_inline_mkrefine_keeps_explicit_threshold_dir() {
    let nml = "&mkgrd NL%expnme='inline', NL%nxp=4, NL%mesh_type='landmesh', NL%mode_grid='tri', NL%output_format='CoLM', NL%refine=.false. /\n&mkrefine RL%threshold_dir='/explicit,threshold', RL%refine_spc=.true. /\n&datalayers NL%layer='lc|landtype|./in/landtype.nc|landtype|T|T' /";

    let out = lower_datalayers_namelist(nml, Some("/fallback/th/"))
        .expect("inline threshold_dir should be recognized");

    assert_eq!(out.threshold_dir, "/explicit,threshold");
    assert!(
        out.namelist
            .contains("threshold_dir = '/explicit,threshold'"),
        "{}",
        out.namelist
    );
}

#[test]
fn lower_datalayers_inline_removes_data_group_and_preserves_other_inline_groups_once() {
    let nml = "&mkgrd NL%expnme='inline', NL%nxp=4, NL%mesh_type='landmesh', NL%mode_grid='tri', NL%output_format='CoLM' /\n&quality NL%min_angle_warn_deg=24, NL%worst_cells_limit=12 /\n&datalayers NL%layer='lai|threshold:lai|./th/lai.nc||T|F' /";

    let out = lower_datalayers_namelist(nml, Some("/fallback/th/"))
        .expect("inline non-execution groups should be filtered correctly");

    assert!(!out.namelist.contains("&datalayers"), "{}", out.namelist);
    assert_eq!(
        out.namelist.matches("&quality").count(),
        1,
        "{}",
        out.namelist
    );
    assert!(
        out.namelist.contains("NL%min_angle_warn_deg=24"),
        "{}",
        out.namelist
    );
}
