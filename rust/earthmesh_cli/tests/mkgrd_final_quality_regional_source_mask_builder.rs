use earthmesh_cli::{
    build_mkgrd_final_quality_regional_source_mask_io,
    enrich_mkgrd_final_quality_with_regional_source_mask_io,
    enrich_mkgrd_refine_loop_final_quality_with_regional_source_mask_io, plan_mkgrd_refine_loop_io,
    write_bbox_mask_netcdf, BBoxMask, BBoxPoint, MkgrdFinalQualityCheckIoPlan,
    MkgrdFinalQualityRegionalSpringIoPlan, MkgrdFinalQualitySpringMode,
};
use earthmesh_core::{EarthmeshConfig, RefineConfig};
use std::path::PathBuf;

fn temp_root(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("earthmesh_cli_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("tmpfile")).expect("create temp root");
    path
}

fn small_axes() -> (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>) {
    let lon_vertex = vec![
        f64::NAN,
        -180.0,
        -179.0,
        -178.0,
        -177.0,
        -176.0,
        -175.0,
        -174.0,
    ];
    let lat_vertex = vec![f64::NAN, 90.0, 89.0, 88.0, 87.0, 86.0, 85.0, 84.0];
    let lon_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| -179.5 + idx as f64))
        .collect::<Vec<_>>();
    let lat_i = std::iter::once(f64::NAN)
        .chain((0..6).map(|idx| 89.5 - idx as f64))
        .collect::<Vec<_>>();
    (lon_vertex, lat_vertex, lon_i, lat_i)
}

fn write_single_bbox_patch(root: &std::path::Path) {
    write_bbox_mask_netcdf(
        root.join("tmpfile/mask_patch_bbox_0_01.nc4"),
        &BBoxMask {
            refine_degree: 0,
            points: vec![BBoxPoint {
                west: -179.5,
                east: -176.0,
                north: 89.5,
                south: 86.0,
            }],
        },
    )
    .expect("write bbox mask patch source");
}

fn mkgrd_config(base_dir: &str, expnme: &str) -> EarthmeshConfig {
    EarthmeshConfig::from_mkgrd_namelist(&format!(
        "&mkgrd\n  NL%EXPNME='{expnme}'\n  NL%base_dir='{base_dir}'\n  NL%NXP=8\n  NL%mesh_type='landmesh'\n  NL%mode_grid='hex'\n  NL%output_format='CoLM'\n  NL%refine=.true.\n/\n"
    ))
    .expect("parse mkgrd config")
}

fn regional_final_refine_config() -> RefineConfig {
    RefineConfig::from_mkrefine_namelist(
        "&mkrefine\n  RL%Istransition=.true.\n  RL%SpringGlobal_type=0\n  RL%SpringRegional_type=2\n  RL%refine_spc=.true.\n  RL%refine_cal=.false.\n  RL%max_iter_spc=1\n  RL%max_iter_cal=0\n  RL%halo=0,3,3,3,3,3,3,3,3,3\n  RL%max_transition_row=0,1,1,1,1,1,1,1,1,1\n  RL%mask_refine_spc_type='bbox'\n  RL%mask_refine_cal_type='bbox'\n/\n",
        "landmesh",
        "hex",
    )
    .expect("parse refine config")
}

#[test]
fn final_quality_regional_source_mask_builder_merges_mask_patch_sources() {
    let root = temp_root("final_quality_regional_source_mask_builder");
    write_single_bbox_patch(&root);
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let plan = build_mkgrd_final_quality_regional_source_mask_io(
        &root,
        "bbox",
        0,
        1,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect("build final regional source mask");

    assert!(plan.source_lon_vertices[0].is_nan());
    assert!(plan.source_lat_vertices[0].is_nan());
    assert_eq!(&plan.source_lon_vertices[1..], &lon_vertex[1..]);
    assert_eq!(&plan.source_lat_vertices[1..], &lat_vertex[1..]);
    assert_eq!(plan.first_triangle_id, 2);
    assert_eq!(plan.mask_patch.len(), 7);
    assert_eq!(plan.mask_patch[1].len(), 7);
    assert!(!plan.mask_patch[1][1]);
    assert!(plan.mask_patch[2][2]);
    assert!(plan.mask_patch[3][3]);
    assert!(!plan.mask_patch[6][6]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn final_quality_regional_source_mask_enrichment_fills_regional_plan() {
    let root = temp_root("final_quality_regional_source_mask_enrichment");
    write_single_bbox_patch(&root);
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();
    let mut plan = MkgrdFinalQualityCheckIoPlan {
        step: 3,
        run_quality_check: true,
        spring_mode: MkgrdFinalQualitySpringMode::RegionalFinal,
        input_gridfile: root.join("gridfile/gridfile_NXP0008_03_hex.nc4"),
        original_gridfile: Some(root.join("gridfile/gridfile_NXP0008_03_hex_orial.nc4")),
        quality_before_spring: Some(root.join("result/quality_NXP0008_03_global_beforeSpring.nc4")),
        quality_after_spring: Some(root.join("result/quality_NXP0008_03_global.nc4")),
        output_gridfile: Some(root.join("gridfile/gridfile_NXP0008_03_hex.nc4")),
        regional_set_dis: Some(3),
        global_spring: None,
        regional_spring: Some(MkgrdFinalQualityRegionalSpringIoPlan {
            niter_refine: 0,
            radius: earthmesh_core::EARTH_RADIUS_METERS,
        }),
        regional_source_mask: None,
    };

    let injected = enrich_mkgrd_final_quality_with_regional_source_mask_io(
        &mut plan,
        &root,
        "bbox",
        0,
        1,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect("enrich final regional plan with source mask");

    assert!(injected);
    let source_mask = plan
        .regional_source_mask
        .as_ref()
        .expect("regional source mask injected");
    assert_eq!(source_mask.first_triangle_id, 2);
    assert!(source_mask.mask_patch[2][2]);
    assert!(!source_mask.mask_patch[1][1]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn refine_loop_plan_enrichment_fills_final_regional_source_mask_from_plan_file_dir() {
    let root = temp_root("refine_loop_plan_final_regional_source_mask_enrichment");
    write_single_bbox_patch(&root);
    let base_dir = format!("{}/", root.parent().expect("temp parent").display());
    let expnme = root.file_name().expect("temp dir name").to_string_lossy();
    let mkgrd = mkgrd_config(&base_dir, &expnme);
    let refine = regional_final_refine_config();
    let mut plan = plan_mkgrd_refine_loop_io(&mkgrd, &refine).expect("plan refine loop io");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let injected = enrich_mkgrd_refine_loop_final_quality_with_regional_source_mask_io(
        &mut plan,
        "bbox",
        0,
        1,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect("enrich refine-loop final regional plan");

    assert!(injected);
    let source_mask = plan
        .final_quality_check
        .regional_source_mask
        .as_ref()
        .expect("refine-loop final quality source mask");
    assert_eq!(source_mask.first_triangle_id, 2);
    assert!(source_mask.mask_patch[2][2]);
    assert!(!source_mask.mask_patch[1][1]);

    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn final_quality_regional_source_mask_builder_rejects_empty_sources() {
    let root = temp_root("final_quality_regional_source_mask_builder_empty");
    let (lon_vertex, lat_vertex, lon_i, lat_i) = small_axes();

    let err = build_mkgrd_final_quality_regional_source_mask_io(
        &root,
        "bbox",
        0,
        0,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
        1,
        6,
        6,
        2,
    )
    .expect_err("empty final regional source mask sources must be rejected");

    assert!(err.to_string().contains("at least one source"));
    let _ = std::fs::remove_dir_all(&root);
}
