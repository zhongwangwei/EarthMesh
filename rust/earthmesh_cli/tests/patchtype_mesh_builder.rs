#[test]
fn patchid_mesh_from_selected_domain_matches_canonical_patchid_save_coordinate_lookup() {
    let patchtypes_select = vec![vec![2, 3, 4], vec![5, 6, 7]];
    let lon_vertex = (0..16).map(|idx| idx as f64 + 0.1).collect::<Vec<_>>();
    let lon_i = (0..16).map(|idx| idx as f64 + 0.5).collect::<Vec<_>>();
    let lat_vertex = (0..25).map(|idx| idx as f64 + 0.2).collect::<Vec<_>>();
    let lat_i = (0..25).map(|idx| idx as f64 + 0.6).collect::<Vec<_>>();

    let patch = earthmesh_cli::mask_postproc_patchtypes::patchid_mesh_from_selected_domain(
        patchtypes_select.clone(),
        10,
        20,
        &lon_vertex,
        &lat_vertex,
        &lon_i,
        &lat_i,
    )
    .expect("patchid mesh");

    assert_eq!(patch.elmindex, patchtypes_select);
    assert_eq!(patch.lon_w, vec![10.1, 11.1]);
    assert_eq!(patch.lon_e, vec![11.1, 12.1]);
    assert_eq!(patch.longitude, vec![10.5, 11.5]);
    assert_eq!(patch.lat_n, vec![20.2, 19.2, 18.2]);
    assert_eq!(patch.lat_s, vec![21.2, 20.2, 19.2]);
    assert_eq!(patch.latitude, vec![20.6, 19.6, 18.6]);
}

#[test]
fn patchid_mesh_from_selected_domain_rejects_missing_lookup_coordinates() {
    let patchtypes_select = vec![vec![2]];
    let err = earthmesh_cli::mask_postproc_patchtypes::patchid_mesh_from_selected_domain(
        patchtypes_select,
        10,
        20,
        &[0.0; 12],
        &[0.0; 21],
        &[0.0; 11],
        &[0.0; 21],
    )
    .expect_err("short lookup arrays rejected");
    assert!(err.to_string().contains("lat_vertex"));
}
