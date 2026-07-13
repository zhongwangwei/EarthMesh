use std::fs;

use earthmesh_core::{MaskOperation, MkgrdWorkspacePlan};

fn write_bbox_source(path: &std::path::Path, refine_degree: usize) {
    earthmesh_cli::bbox_mask_io::write_bbox_mask_netcdf(
        path,
        &earthmesh_cli::bbox_mask_io::BBoxMask {
            refine_degree,
            points: vec![earthmesh_cli::bbox_mask_io::BBoxPoint {
                west: 100.0,
                east: 120.0,
                north: 30.0,
                south: 20.0,
            }],
        },
    )
    .expect("write bbox source");
}

#[test]
fn apply_workspace_and_mask_operations_runs_masks_after_workspace_setup() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_workspace_masks_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let sources = root.join("sources");
    let case_dir = root.join("case");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_source(&sources.join("domain_01.nc4"), 0);
    write_bbox_source(&sources.join("refine_01.nc4"), 2);
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
        mask_operations: vec![
            MaskOperation::new(
                "mask_domain",
                "bbox",
                &sources.join("domain_").to_string_lossy(),
            ),
            MaskOperation::new(
                "mask_refine",
                "bbox",
                &sources.join("refine_").to_string_lossy(),
            ),
        ],
    };

    let report = earthmesh_cli::workspace_mask_apply::apply_workspace_and_mask_operations(
        &plan, &namelist, &root, 2, true,
    )
    .expect("workspace plus masks");

    assert_eq!(report.workspace.created_directories.len(), 6);
    assert_eq!(report.mask_reports.len(), 2);
    assert_eq!(report.mask_counts.mask_domain_ndm, 1);
    assert_eq!(report.mask_counts.mask_refine_ndm[2], 1);
    assert!(case_dir.join("result/namelist.save").exists());
    assert!(case_dir.join("tmpfile/mask_domain_bbox_0_01.nc4").exists());
    assert!(case_dir.join("tmpfile/mask_refine_bbox_2_01.nc4").exists());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_workspace_and_mask_operations_validates_requested_refine_count() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_workspace_masks_validate_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let sources = root.join("sources");
    let case_dir = root.join("case");
    fs::create_dir_all(&sources).expect("create sources");
    write_bbox_source(&sources.join("refine_01.nc4"), 1);
    let namelist = root.join("mkgrd.nml");
    fs::write(&namelist, "&mkgrd\n/\n").expect("write namelist");
    let plan = MkgrdWorkspacePlan {
        file_dir: case_dir.to_string_lossy().into_owned(),
        remove_existing_file_dir: false,
        remove_filelists: false,
        directories_to_create: vec![case_dir.join("tmpfile").to_string_lossy().into_owned()],
        namelist_save_path: case_dir
            .join("result/namelist.save")
            .to_string_lossy()
            .into_owned(),
        mask_operations: vec![MaskOperation::new(
            "mask_refine",
            "bbox",
            &sources.join("refine_").to_string_lossy(),
        )],
    };

    let err = earthmesh_cli::workspace_mask_apply::apply_workspace_and_mask_operations(
        &plan, &namelist, &root, 2, true,
    )
    .expect_err("missing max_iter_spc mask_refine should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

    let _ = fs::remove_dir_all(&root);
}
