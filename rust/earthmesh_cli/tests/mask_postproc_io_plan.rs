#[test]
fn ocean_tri_patch_plan_matches_compatibility_mask_postproc_paths_and_boundary_outputs() {
    let root = std::env::temp_dir().join("earthmesh_mask_postproc_io_ocean");
    let plan = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        64,
        "tri",
        "oceanmesh",
        true,
    )
    .expect("ocean plan");

    assert_eq!(plan.mesh_type, "oceanmesh");
    assert_eq!(plan.mode_grid, "tri");
    assert_eq!(
        plan.source_gridfile,
        root.join("result/gridfile_NXP0064_tri.nc4")
    );
    assert_eq!(
        plan.contain_domain,
        root.join("contain/contain_oceanmesh_domain_NXP0064_tri.nc4")
    );
    assert_eq!(
        plan.result_gridfile,
        root.join("result/gridfile_NXP0064_tri_oceanmesh_patch.nc4")
    );
    assert_eq!(plan.patchtype_output, None);
    assert_eq!(plan.obc_output, Some(root.join("result/obc_patch.nc4")));
    assert_eq!(plan.obcv2_output, Some(root.join("result/obcv2_patch.nc4")));
}

#[test]
fn land_and_earth_plans_include_patchtype_but_no_boundary_outputs() {
    let root = std::env::temp_dir().join("earthmesh_mask_postproc_io_land");
    let land = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root, 7, "hex", "landmesh", false,
    )
    .expect("land plan");
    assert_eq!(
        land.source_gridfile,
        root.join("result/gridfile_NXP0007_hex.nc4")
    );
    assert_eq!(
        land.contain_domain,
        root.join("contain/contain_landmesh_domain_NXP0007_hex.nc4")
    );
    assert_eq!(
        land.result_gridfile,
        root.join("result/gridfile_NXP0007_hex_landmesh.nc4")
    );
    assert_eq!(
        land.patchtype_output,
        Some(root.join("patchtype/patchtype_NXP0007_hex.nc4"))
    );
    assert_eq!(land.obc_output, None);
    assert_eq!(land.obcv2_output, None);

    let earth = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        7,
        "tri",
        "earthmesh",
        false,
    )
    .expect("earth plan");
    assert_eq!(
        earth.patchtype_output,
        Some(root.join("patchtype/patchtype_NXP0007_tri.nc4"))
    );
    assert_eq!(earth.obc_output, None);
    assert_eq!(earth.obcv2_output, None);
}

#[test]
fn mask_postproc_io_plan_rejects_atmos_and_unsupported_grid_shapes() {
    let root = std::env::temp_dir().join("earthmesh_mask_postproc_io_invalid");
    let atmos = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        1,
        "tri",
        "atmosmesh",
        false,
    )
    .expect_err("atmos is separate MPAS branch");
    assert!(atmos.to_string().contains("domain mask_postproc"));

    let square = earthmesh_cli::mask_postproc_domain::plan_mask_postproc_domain_io(
        &root,
        1,
        "square",
        "oceanmesh",
        false,
    )
    .expect_err("unsupported grid shape");
    assert!(square.to_string().contains("tri or hex"));
}
