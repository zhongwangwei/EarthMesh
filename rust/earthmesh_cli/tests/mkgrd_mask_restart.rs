use std::fs;

#[test]
fn mask_restart_ocean_without_patch_plans_remask_postproc_without_gridinit() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_mask_restart_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");

    let namelist = root.join("mkgrd_restart.nml");
    let base_dir = format!("{}/", root.display());
    fs::write(
        &namelist,
        format!(
            "&mkgrd\n  NL%EXPNME='case_restart'\n  NL%base_dir='{base_dir}'\n  NL%NXP=16\n  NL%mesh_type='oceanmesh'\n  NL%mode_grid='tri'\n  NL%output_format='FVCOM'\n  NL%mask_restart=.true.\n  NL%mask_patch_on=.false.\n/\n"
        ),
    )
    .expect("write namelist");

    let report =
        earthmesh_cli::plan_mkgrd_mask_restart_namelist(&namelist, &root, 7).expect("restart plan");

    assert!(report.config.mask_restart);
    assert_eq!(report.remask.mesh_type, "oceanmesh");
    assert_eq!(report.remask.file_dir, root.join("case_restart/"));
    assert_eq!(report.remask.step, 8);
    assert!(!report.remask.refine);
    assert_eq!(
        report.remask.action,
        earthmesh_cli::MaskRestartAction::RunMaskPostproc
    );
    assert!(!report.workspace_plan.remove_existing_file_dir);
    assert!(report.workspace_plan.directories_to_create.is_empty());
    assert!(report.workspace_plan.mask_operations.is_empty());

    let _ = fs::remove_dir_all(&root);
}
