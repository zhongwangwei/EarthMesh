use std::fs;

#[test]
fn parse_circle_and_close_nml_match_canonical_free_format_rules() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_circle_close_parse_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let circle_input = root.join("circle.nml");
    fs::write(
        &circle_input,
        "circle_num = 2\ncircle_refine = 4\n113.2 22.4 25.0\n114.0 23.0 10.0\n",
    )
    .expect("write circle nml");
    let circle = earthmesh_cli::circle_close_mask_io::parse_circle_mask_nml(&circle_input, 5)
        .expect("parse circle")
        .expect("within max_iter_spc");
    assert_eq!(circle.refine_degree, 4);
    assert_eq!(circle.points.len(), 2);
    assert_eq!(circle.points[0].lon, 113.2);
    assert_eq!(circle.points[0].lat, 22.4);
    assert_eq!(circle.radius_km, vec![25.0, 10.0]);

    let close_input = root.join("close.nml");
    fs::write(
        &close_input,
        "close_num = 3\nclose_refine = 2\n100.0 20.0\n101.0 21.0\n102.0 20.5\n",
    )
    .expect("write close nml");
    let close = earthmesh_cli::circle_close_mask_io::parse_close_mask_nml(&close_input, 5)
        .expect("parse close")
        .expect("within max_iter_spc");
    assert_eq!(close.refine_degree, 2);
    assert_eq!(close.points.len(), 3);
    assert_eq!(close.points[1].lon, 101.0);
    assert_eq!(close.points[1].lat, 21.0);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn circle_and_close_netcdf_reader_writer_match_canonical_schema() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_circle_close_netcdf_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create root");

    let circle_output = root.join("circle.nc4");
    earthmesh_cli::circle_close_mask_io::write_circle_mask_netcdf(
        &circle_output,
        &earthmesh_cli::circle_close_mask_io::CircleMask {
            refine_degree: 3,
            points: vec![
                earthmesh_cli::coordinate_types::LonLatPoint {
                    lon: 113.2,
                    lat: 22.4,
                },
                earthmesh_cli::coordinate_types::LonLatPoint {
                    lon: 114.0,
                    lat: 23.0,
                },
            ],
            radius_km: vec![25.0, 10.0],
        },
    )
    .expect("write circle nc");
    assert_eq!(
        earthmesh_cli::circle_close_mask_io::read_circle_refine_netcdf(&circle_output)
            .expect("read circle_refine"),
        3
    );
    let circle_file = netcdf::open(&circle_output).expect("open circle");
    assert_eq!(circle_file.dimension("circle_num").unwrap().len(), 2);
    assert_eq!(circle_file.dimension("two").unwrap().len(), 2);
    assert_eq!(
        circle_file
            .variable("circle_points")
            .unwrap()
            .get_values::<f64, _>((.., ..))
            .unwrap(),
        vec![113.2, 22.4, 114.0, 23.0]
    );
    assert_eq!(
        circle_file
            .variable("circle_radius")
            .unwrap()
            .get_values::<f64, _>(..)
            .unwrap(),
        vec![25.0, 10.0]
    );

    let close_output = root.join("close.nc4");
    earthmesh_cli::circle_close_mask_io::write_close_mask_netcdf(
        &close_output,
        &earthmesh_cli::circle_close_mask_io::CloseMask {
            refine_degree: 2,
            points: vec![
                earthmesh_cli::coordinate_types::LonLatPoint {
                    lon: 100.0,
                    lat: 20.0,
                },
                earthmesh_cli::coordinate_types::LonLatPoint {
                    lon: 101.0,
                    lat: 21.0,
                },
            ],
        },
    )
    .expect("write close nc");
    assert_eq!(
        earthmesh_cli::circle_close_mask_io::read_close_refine_netcdf(&close_output)
            .expect("read close_refine"),
        2
    );
    let close_file = netcdf::open(&close_output).expect("open close");
    assert_eq!(close_file.dimension("close_num").unwrap().len(), 2);
    assert_eq!(close_file.dimension("two").unwrap().len(), 2);
    assert_eq!(
        close_file
            .variable("close_points")
            .unwrap()
            .get_values::<f64, _>((.., ..))
            .unwrap(),
        vec![100.0, 20.0, 101.0, 21.0]
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn circle_and_close_output_numbering_preserves_widths() {
    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();
    let root = "/tmp/case/";

    let circle = counts
        .next_circle_output("mask_refine", 4, root)
        .expect("circle output");
    let close = counts
        .next_close_output("mask_refine", 4, root)
        .expect("close output");

    assert_eq!(
        circle.to_string_lossy(),
        "/tmp/case/tmpfile/mask_refine_circle_4_01.nc4"
    );
    assert_eq!(
        close.to_string_lossy(),
        "/tmp/case/tmpfile/mask_refine_close_4_002.nc4"
    );
    assert_eq!(counts.mask_refine_ndm[4], 2);
}

#[test]
fn copy_circle_and_close_netcdf_match_canonical_skip_copy_and_numbering() {
    let root = std::env::temp_dir().join(format!(
        "earthmesh_cli_circle_close_copy_{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tmpfile")).expect("create tmpfile");
    let circle_source = root.join("circle_source.nc4");
    let close_source = root.join("close_source.nc4");
    fs::write(&circle_source, b"circle bytes").expect("write circle source");
    fs::write(&close_source, b"close bytes").expect("write close source");
    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();

    assert!(
        earthmesh_cli::mask_operation_apply::copy_circle_mask_netcdf_with_refine(
            &circle_source,
            "mask_patch",
            7,
            5,
            &root,
            &mut counts,
        )
        .expect("skip high circle refine")
        .is_none()
    );
    assert_eq!(counts.mask_patch_ndm[7], 0);

    let circle = earthmesh_cli::mask_operation_apply::copy_circle_mask_netcdf_with_refine(
        &circle_source,
        "mask_patch",
        4,
        5,
        &root,
        &mut counts,
    )
    .expect("copy circle")
    .expect("circle output");
    let close = earthmesh_cli::mask_operation_apply::copy_close_mask_netcdf_with_refine(
        &close_source,
        "mask_patch",
        4,
        5,
        &root,
        &mut counts,
    )
    .expect("copy close")
    .expect("close output");

    assert_eq!(circle, root.join("tmpfile/mask_patch_circle_4_01.nc4"));
    assert_eq!(close, root.join("tmpfile/mask_patch_close_4_002.nc4"));
    assert_eq!(fs::read(circle).unwrap(), b"circle bytes");
    assert_eq!(fs::read(close).unwrap(), b"close bytes");
    assert_eq!(counts.mask_patch_ndm[4], 2);

    let _ = fs::remove_dir_all(&root);
}
