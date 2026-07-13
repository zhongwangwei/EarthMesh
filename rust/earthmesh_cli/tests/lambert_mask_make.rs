use std::fs;

#[test]
fn lambert_vertices_convert_to_mode4_mesh_with_canonical_indexing() {
    let vertices = earthmesh_cli::lambert_mode4_io::LambertVertices {
        xi_vert: 2,
        eta_vert: 2,
        lon_vert: vec![181.0, 182.0, 183.0, 184.0],
        lat_vert: vec![10.0, 11.0, 12.0, 13.0],
    };

    let mesh = earthmesh_cli::lambert_mode4_io::lambert_vertices_to_mode4_mesh(&vertices)
        .expect("convert lambert vertices");

    assert_eq!(mesh.bound_points(), 5);
    assert_eq!(mesh.mode_points(), 2);
    assert_eq!(
        mesh.lonlat_bound[0],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -999.0,
            lat: -999.0
        }
    );
    assert_eq!(
        mesh.lonlat_bound[1],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -179.0,
            lat: 10.0
        }
    );
    assert_eq!(
        mesh.lonlat_bound[4],
        earthmesh_cli::coordinate_types::LonLatPoint {
            lon: -176.0,
            lat: 13.0
        }
    );
    assert_eq!(mesh.ngr_bound[0], [1, 1, 1, 1]);
    assert_eq!(mesh.ngr_bound[1], [2, 3, 5, 4]);
    assert_eq!(mesh.n_ngr, vec![4, 4]);
}

#[test]
fn lambert_reader_writer_and_output_numbering_match_canonical_schema() {
    let root = std::env::temp_dir().join(format!("earthmesh_cli_lambert_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("tmpfile")).expect("create tmpfile");

    let source = root.join("lambert_source.nc4");
    {
        let mut file = earthmesh_cli::create_netcdf_quiet(&source).expect("create lambert source");
        file.add_dimension("xi_vert", 2).expect("xi dim");
        file.add_dimension("eta_vert", 2).expect("eta dim");
        file.add_variable::<f64>("lon_vert", &["xi_vert", "eta_vert"])
            .expect("lon var")
            .put_values(&[181.0, 182.0, 183.0, 184.0], (.., ..))
            .expect("write lon");
        file.add_variable::<f64>("lat_vert", &["xi_vert", "eta_vert"])
            .expect("lat var")
            .put_values(&[10.0, 11.0, 12.0, 13.0], (.., ..))
            .expect("write lat");
    }

    let vertices = earthmesh_cli::lambert_mode4_io::read_lambert_vertices_netcdf(&source)
        .expect("read lambert vertices");
    assert_eq!(vertices.xi_vert, 2);
    assert_eq!(vertices.eta_vert, 2);

    let mut counts = earthmesh_cli::mask_counts::MaskCountState::default();
    let output = earthmesh_cli::lambert_mode4_io::convert_lambert_mask_netcdf(
        &source,
        "mask_domain",
        &root,
        &mut counts,
    )
    .expect("convert lambert source");

    assert_eq!(output, root.join("tmpfile/mask_domain_lambert_0_01.nc4"));
    assert_eq!(counts.mask_domain_ndm, 1);
    let file = netcdf::open(&output).expect("open mode4 output");
    assert_eq!(file.dimension("bound_points").unwrap().len(), 5);
    assert_eq!(file.dimension("mode_points").unwrap().len(), 2);
    assert_eq!(file.dimension("two").unwrap().len(), 2);
    assert_eq!(file.dimension("four").unwrap().len(), 4);
    assert_eq!(
        file.variable("lonlat_bound")
            .unwrap()
            .get_values::<f64, _>((.., ..))
            .unwrap(),
        vec![-999.0, -999.0, -179.0, 10.0, -178.0, 11.0, -177.0, 12.0, -176.0, 13.0]
    );
    assert_eq!(
        file.variable("ngr_bound")
            .unwrap()
            .get_values::<i32, _>((.., ..))
            .unwrap(),
        vec![1, 1, 1, 1, 2, 3, 5, 4]
    );
    assert_eq!(
        file.variable("n_ngr")
            .unwrap()
            .get_values::<i32, _>(..)
            .unwrap(),
        vec![4, 4]
    );

    let _ = fs::remove_dir_all(&root);
}
