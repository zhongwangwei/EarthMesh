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

    let report =
        earthmesh_cli::workspace_apply::apply_read_nl_workspace_plan(&plan, &namelist, &root)
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

    let report =
        earthmesh_cli::workspace_apply::apply_read_nl_workspace_plan(&plan, &namelist, &root)
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

#[test]
fn apply_read_nl_workspace_plan_rejects_delete_outside_workdir() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_workspace_guard_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    let outside = std::env::temp_dir().join(format!(
        "earthmesh_cli_workspace_outside_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(outside.join("important")).expect("create outside dir");
    fs::write(outside.join("important/keep.txt"), "keep").expect("write outside file");
    let namelist = root.join("mkgrd.nml");
    fs::write(&namelist, "&mkgrd\n/\n").expect("write namelist");

    let plan = MkgrdWorkspacePlan {
        file_dir: outside.to_string_lossy().into_owned(),
        remove_existing_file_dir: true,
        remove_filelists: false,
        directories_to_create: Vec::new(),
        namelist_save_path: outside.join("namelist.save").to_string_lossy().into_owned(),
        mask_operations: Vec::new(),
    };

    let err = earthmesh_cli::workspace_apply::apply_read_nl_workspace_plan(&plan, &namelist, &root)
        .expect_err("outside file_dir must be rejected before deletion");

    assert!(
        err.to_string().contains("outside workdir"),
        "unexpected error: {err}"
    );
    assert_eq!(
        fs::read_to_string(outside.join("important/keep.txt")).unwrap(),
        "keep"
    );

    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn relative_workspace_paths_always_resolve_from_workdir() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_workspace_relative_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let workdir = root.join("z-workdir");
    let namelist_dir = root.join("a-namelist");
    fs::create_dir_all(&workdir).unwrap();
    fs::create_dir_all(&namelist_dir).unwrap();
    let namelist = namelist_dir.join("mkgrd.nml");
    fs::write(&namelist, "&mkgrd\n/\n").unwrap();
    let plan = MkgrdWorkspacePlan {
        file_dir: "case/".into(),
        remove_existing_file_dir: false,
        remove_filelists: false,
        directories_to_create: vec!["case/result".into()],
        namelist_save_path: "case/result/namelist.save".into(),
        mask_operations: Vec::new(),
    };

    earthmesh_cli::workspace_apply::apply_read_nl_workspace_plan(&plan, &namelist, &workdir)
        .unwrap();

    assert!(workdir.join("case/result/namelist.save").is_file());
    assert!(!namelist_dir.join("case").exists());
    let _ = fs::remove_dir_all(root);
}
