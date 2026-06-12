use std::fs;

use earthmesh_core::{MaskOperation, MkgrdWorkspacePlan};

#[test]
fn apply_read_nl_workspace_plan_creates_dirs_copies_namelist_and_cleans_filelists() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_workspace_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let case_dir = root.join("case/");
    fs::create_dir_all(case_dir.join("old_subdir")).expect("create stale case dir");
    fs::write(case_dir.join("old_subdir/stale.txt"), "old").expect("write stale file");
    fs::write(root.join("mask_domain_filelist.txt"), "old list").expect("write stale filelist");
    fs::write(root.join("keep.txt"), "keep").expect("write keep file");
    let namelist = root.join("mkgrd.nml");
    fs::write(&namelist, "&mkgrd\n/\n").expect("write namelist");

    let plan = MkgrdWorkspacePlan {
        file_dir: case_dir.to_string_lossy().into_owned(),
        remove_existing_file_dir: true,
        remove_filelists: true,
        directories_to_create: [
            "contain",
            "gridfile",
            "patchtype",
            "result",
            "tmpfile",
            "threshold",
        ]
        .into_iter()
        .map(|name| case_dir.join(name).to_string_lossy().into_owned())
        .collect(),
        namelist_save_path: case_dir
            .join("result/namelist.save")
            .to_string_lossy()
            .into_owned(),
        mask_operations: vec![MaskOperation::new("mask_domain", "bbox", "/tmp/domain")],
    };

    let report = earthmesh_cli::apply_read_nl_workspace_plan(&plan, &namelist, &root)
        .expect("workspace plan should apply");

    assert_eq!(report.created_directories.len(), 6);
    assert_eq!(
        report.removed_filelists,
        vec![root.join("mask_domain_filelist.txt")]
    );
    assert_eq!(
        report.copied_namelist_to,
        Some(case_dir.join("result/namelist.save"))
    );
    assert!(!case_dir.join("old_subdir/stale.txt").exists());
    assert!(case_dir.join("contain").is_dir());
    assert!(case_dir.join("threshold").is_dir());
    assert_eq!(
        fs::read_to_string(case_dir.join("result/namelist.save")).unwrap(),
        "&mkgrd\n/\n"
    );
    assert_eq!(fs::read_to_string(root.join("keep.txt")).unwrap(), "keep");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_read_nl_workspace_plan_preserves_restart_short_circuit() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_restart_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let case_dir = root.join("restart_case/");
    fs::create_dir_all(case_dir.join("result")).expect("create case result");
    fs::write(case_dir.join("old.txt"), "old").expect("write old file");
    let namelist = root.join("mkgrd.nml");
    fs::write(&namelist, "&mkgrd\n/\n").expect("write namelist");

    let plan = MkgrdWorkspacePlan {
        file_dir: case_dir.to_string_lossy().into_owned(),
        remove_existing_file_dir: false,
        remove_filelists: false,
        directories_to_create: Vec::new(),
        namelist_save_path: case_dir
            .join("result/namelist.save")
            .to_string_lossy()
            .into_owned(),
        mask_operations: vec![MaskOperation::new("mask_patch", "bbox", "/tmp/patch")],
    };

    let report = earthmesh_cli::apply_read_nl_workspace_plan(&plan, &namelist, &root)
        .expect("restart plan should apply without deleting case dir");

    assert!(report.created_directories.is_empty());
    assert!(report.removed_filelists.is_empty());
    assert!(case_dir.join("old.txt").exists());
    assert_eq!(
        fs::read_to_string(case_dir.join("result/namelist.save")).unwrap(),
        "&mkgrd\n/\n"
    );

    let _ = fs::remove_dir_all(&root);
}
