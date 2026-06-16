use earthmesh_core::{EarthmeshConfig, RefineConfig};
use std::path::Path;

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

fn assert_example_round_trips(relative: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
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

fn assert_example_mkrefine_round_trips(relative: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let contents = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // mesh_type / mode_grid come from the &mkgrd block and gate &mkrefine validation.
    let base = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .unwrap_or_else(|e| panic!("parse mkgrd {}: {e}", path.display()));
    let original = RefineConfig::from_mkrefine_namelist(&contents, &base.mesh_type, &base.mode_grid)
        .unwrap_or_else(|e| panic!("parse mkrefine {}: {e}", path.display()));
    let reparsed = RefineConfig::from_mkrefine_namelist(
        &original.to_mkrefine_namelist(),
        &base.mesh_type,
        &base.mode_grid,
    )
    .unwrap_or_else(|e| panic!("re-parse mkrefine {}: {e}", path.display()));
    assert_eq!(original, reparsed, "mkrefine round-trip mismatch for {}", relative);
}

#[test]
fn default_example_mkrefine_round_trips() {
    assert_example_mkrefine_round_trips("examples/default/atmosphere_hex_global.nml");
    assert_example_mkrefine_round_trips("examples/default/land_hex_global.nml");
    assert_example_mkrefine_round_trips("examples/default/ocean_hex_global.nml");
}
