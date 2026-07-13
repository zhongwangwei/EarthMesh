use std::fs;

use earthmesh_core::MaskOperation;

#[test]
fn apply_mask_operation_dispatches_bbox_sources_and_validates_refine_count() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_dispatch_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let source_dir = root.join("sources");
    let case_dir = root.join("case");
    fs::create_dir_all(&source_dir).expect("create sources");
    fs::create_dir_all(case_dir.join("tmpfile")).expect("create tmpfile");

    let nml = source_dir.join("bbox_01.nml");
    fs::write(
        &nml,
        "bbox_num = 1\nbbox_refine = 3\n100.0 120.0 30.0 20.0\n",
    )
    .expect("write bbox nml");
    let nc = source_dir.join("bbox_02.nc4");
    earthmesh_cli::bbox_mask_io::write_bbox_mask_netcdf(
        &nc,
        &earthmesh_cli::bbox_mask_io::BBoxMask {
            refine_degree: 2,
            points: vec![earthmesh_cli::bbox_mask_io::BBoxPoint {
                west: -10.0,
                east: 10.0,
                north: 5.0,
                south: -5.0,
            }],
        },
    )
    .expect("write bbox nc source");

    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();
    let report = earthmesh_cli::mask_operation_apply::apply_mask_operation(
        &MaskOperation::new(
            "mask_refine",
            "bbox",
            &source_dir.join("bbox_").to_string_lossy(),
        ),
        &case_dir,
        3,
        &mut counts,
    )
    .expect("dispatch bbox operation");

    assert_eq!(
        report.outputs,
        vec![
            case_dir.join("tmpfile/mask_refine_bbox_3_01.nc4"),
            case_dir.join("tmpfile/mask_refine_bbox_2_01.nc4"),
        ]
    );
    assert!(report.outputs.iter().all(|path| path.exists()));
    assert_eq!(counts.mask_refine_ndm[2], 1);
    assert_eq!(counts.mask_refine_ndm[3], 1);
    earthmesh_cli::mask_operation_apply::validate_mask_refine_reaches_max_iter_spc(&counts, 3)
        .expect("max_iter_spc refine exists");
    let err =
        earthmesh_cli::mask_operation_apply::validate_mask_refine_reaches_max_iter_spc(&counts, 1)
            .expect_err("missing max_iter_spc refine should fail like read_nl");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn apply_mask_operation_errors_for_missing_sources_and_unsupported_types() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_mask_dispatch_errors_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sources")).expect("create sources");
    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();

    let missing = earthmesh_cli::mask_operation_apply::apply_mask_operation(
        &MaskOperation::new(
            "mask_domain",
            "bbox",
            &root.join("sources/missing_").to_string_lossy(),
        ),
        &root,
        1,
        &mut counts,
    )
    .expect_err("empty prefix should match Canonical fexists stop");
    assert_eq!(missing.kind(), std::io::ErrorKind::NotFound);

    fs::write(
        root.join("sources/bad_01.nml"),
        "bbox_num = 0\nbbox_refine = 0\n",
    )
    .expect("write matching file");
    let unsupported = earthmesh_cli::mask_operation_apply::apply_mask_operation(
        &MaskOperation::new(
            "mask_domain",
            "unknown",
            &root.join("sources/bad_").to_string_lossy(),
        ),
        &root,
        1,
        &mut counts,
    )
    .expect_err("unsupported type_select should fail");
    assert_eq!(unsupported.kind(), std::io::ErrorKind::InvalidInput);

    let _ = fs::remove_dir_all(&root);
}
