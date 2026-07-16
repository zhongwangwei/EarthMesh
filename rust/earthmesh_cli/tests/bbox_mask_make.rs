use std::fs;

#[test]
fn parse_bbox_mask_nml_matches_canonical_free_format_rules() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_bbox_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let input = root.join("bbox_mask.nml");
    fs::write(
        &input,
        "bbox_num = 2\nbbox_refine = 3\n-10.0 20.0 50.0 30.0\n100.0 120.0 10.0 -5.0\n",
    )
    .expect("write bbox nml");

    let parsed = earthmesh_cli::bbox_mask_io::parse_bbox_mask_nml(&input, 5)
        .expect("parse bbox nml")
        .expect("refine degree within max_iter_spc");

    assert_eq!(parsed.refine_degree, 3);
    assert_eq!(parsed.points.len(), 2);
    assert_eq!(
        parsed.points[0],
        earthmesh_cli::bbox_mask_io::BBoxPoint {
            west: -10.0,
            east: 20.0,
            north: 50.0,
            south: 30.0
        }
    );
    assert_eq!(
        parsed.points[1],
        earthmesh_cli::bbox_mask_io::BBoxPoint {
            west: 100.0,
            east: 120.0,
            north: 10.0,
            south: -5.0
        }
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn parse_bbox_mask_nml_accepts_directed_dateline_bbox_and_skips_too_high_refine() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_bbox_reject_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let dateline_bbox = root.join("dateline_bbox.nml");
    fs::write(
        &dateline_bbox,
        "bbox_num = 1\nbbox_refine = 1\n20.0 -10.0 40.0 20.0\n",
    )
    .expect("write directed bbox");
    let parsed = earthmesh_cli::bbox_mask_io::parse_bbox_mask_nml(&dateline_bbox, 5)
        .expect("parse directed bbox")
        .expect("refine degree within max_iter_spc");
    assert_eq!(parsed.points[0].west, 20.0);
    assert_eq!(parsed.points[0].east, -10.0);

    let too_high = root.join("too_high.nml");
    fs::write(
        &too_high,
        "bbox_num = 1\nbbox_refine = 6\n-10.0 20.0 40.0 20.0\n",
    )
    .expect("write too high refine");
    assert!(
        earthmesh_cli::bbox_mask_io::parse_bbox_mask_nml(&too_high, 5)
            .expect("too-high refine returns no parsed mask like Canonical return")
            .is_none()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn bbox_mask_output_plan_matches_canonical_numbering() {
    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();
    let file_dir = "/tmp/case/";

    let first_domain = counts
        .next_bbox_output("mask_domain", 2, file_dir)
        .expect("domain output");
    let first_refine = counts
        .next_bbox_output("mask_refine", 3, file_dir)
        .expect("refine output");
    let second_refine = counts
        .next_bbox_output("mask_refine", 3, file_dir)
        .expect("second refine output");
    let first_patch = counts
        .next_bbox_output("mask_patch", 3, file_dir)
        .expect("patch output");

    assert_eq!(
        first_domain.to_string_lossy(),
        "/tmp/case/tmpfile/mask_domain_bbox_2_01.nc4"
    );
    assert_eq!(
        first_refine.to_string_lossy(),
        "/tmp/case/tmpfile/mask_refine_bbox_3_01.nc4"
    );
    assert_eq!(
        second_refine.to_string_lossy(),
        "/tmp/case/tmpfile/mask_refine_bbox_3_02.nc4"
    );
    assert_eq!(
        first_patch.to_string_lossy(),
        "/tmp/case/tmpfile/mask_patch_bbox_3_01.nc4"
    );
    assert_eq!(counts.mask_domain_ndm, 1);
    assert_eq!(counts.mask_refine_ndm[3], 2);
    assert_eq!(counts.mask_patch_ndm[3], 1);
}

#[test]
fn copy_bbox_mask_netcdf_matches_canonical_skip_copy_and_numbering() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_bbox_copy_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tmpfile")).expect("create tmpfile");
    let source = root.join("source_bbox.nc4");
    fs::write(&source, b"pretend netcdf bytes").expect("write source nc4");
    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();

    let skipped = earthmesh_cli::mask_operation_apply::copy_bbox_mask_netcdf_with_refine(
        &source,
        "mask_refine",
        6,
        5,
        &root,
        &mut counts,
    )
    .expect("too-high refine is a no-op like Canonical");
    assert!(skipped.is_none());
    assert_eq!(counts.mask_refine_ndm[6], 0);

    let copied = earthmesh_cli::mask_operation_apply::copy_bbox_mask_netcdf_with_refine(
        &source,
        "mask_refine",
        4,
        5,
        &root,
        &mut counts,
    )
    .expect("copy valid bbox nc4")
    .expect("valid refine yields output");

    assert_eq!(copied, root.join("tmpfile/mask_refine_bbox_4_01.nc4"));
    assert_eq!(
        fs::read(&copied).expect("read copied nc4"),
        b"pretend netcdf bytes"
    );
    assert_eq!(counts.mask_refine_ndm[4], 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn bbox_netcdf_reader_and_writer_match_canonical_schema() {
    let root =
        std::env::temp_dir().join(format!("earthmesh_cli_bbox_netcdf_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");
    let source = root.join("source_bbox.nc");
    {
        let mut file = earthmesh_cli::create_netcdf_quiet(&source).expect("create source nc");
        let mut refine = file
            .add_variable::<i32>("bbox_refine", &[])
            .expect("define bbox_refine");
        refine.put_value(3_i32, ()).expect("write bbox_refine");
    }

    let refine =
        earthmesh_cli::bbox_mask_io::read_bbox_refine_netcdf(&source).expect("read bbox_refine");
    assert_eq!(refine, 3);

    let output = root.join("written_bbox.nc4");
    earthmesh_cli::bbox_mask_io::write_bbox_mask_netcdf(
        &output,
        &earthmesh_cli::bbox_mask_io::BBoxMask {
            refine_degree: 2,
            points: vec![
                earthmesh_cli::bbox_mask_io::BBoxPoint {
                    west: -1.0,
                    east: 2.0,
                    north: 30.0,
                    south: 20.0,
                },
                earthmesh_cli::bbox_mask_io::BBoxPoint {
                    west: 100.0,
                    east: 120.0,
                    north: 10.0,
                    south: -5.0,
                },
            ],
        },
    )
    .expect("write bbox netcdf");

    let file = netcdf::open(&output).expect("open written bbox");
    assert_eq!(file.dimension("bbox_num").expect("bbox_num dim").len(), 2);
    assert_eq!(file.dimension("four").expect("four dim").len(), 4);
    let points = file
        .variable("bbox_points")
        .expect("bbox_points var")
        .get_values::<f64, _>((.., ..))
        .expect("read bbox_points");
    assert_eq!(
        points,
        vec![-1.0, 2.0, 30.0, 20.0, 100.0, 120.0, 10.0, -5.0]
    );
    assert_eq!(
        file.variable("bbox_refine")
            .expect("bbox_refine var")
            .get_value::<i32, _>(())
            .expect("read written refine"),
        2
    );

    let _ = fs::remove_dir_all(&root);
}
