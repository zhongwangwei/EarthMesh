use earthmesh_cli::{apply_ngr_renew_fortran_indexed, LonLatPoint};

fn point(lon: f64, lat: f64) -> LonLatPoint {
    LonLatPoint { lon, lat }
}

fn assert_point(actual: LonLatPoint, lon: f64, lat: f64) {
    assert!(
        (actual.lon - lon).abs() < 1.0e-12,
        "lon {:?} != {lon}",
        actual
    );
    assert!(
        (actual.lat - lat).abs() < 1.0e-12,
        "lat {:?} != {lat}",
        actual
    );
}

#[test]
fn ngr_renew_adapter_allocates_final_fortran_rows_and_remaps_boundaries() {
    let iter = 2;
    let num_vertex = 1;
    let num_mp = vec![0, 3, 6];
    let num_wp = vec![0, 4, 7];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    mp_new[1] = point(-1.0, -1.0);
    mp_new[2] = point(1.0, 1.0);
    mp_new[3] = point(2.0, 2.0);
    mp_new[4] = point(4.0, 4.0);
    mp_new[5] = point(5.0, 5.0);
    mp_new[6] = point(6.0, 6.0);
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    for id in 1..=4 {
        wp_new[id] = point(id as f64, 0.0);
    }
    wp_new[5] = point(10.0, 1.0);
    wp_new[6] = point(10.0, 1.0);
    wp_new[7] = point(11.0, 2.0);
    let ngrmw_new = vec![
        vec![0, 0, 0, 0, 0, 0, 0],
        vec![0, 1, 2, 3, 2, 1, 4],
        vec![0, 2, 3, 4, 5, 1, 6],
        vec![0, 3, 4, 2, 6, 1, 7],
    ];
    let mut mp_f = Vec::new();
    let mut wp_f = Vec::new();
    let mut ngrmw_f = Vec::new();
    let mut ngrwm_f = Vec::new();
    let mut n_ngrwm_f = Vec::new();
    let mut bdy_refine = vec![5, 6, 7];
    let mut bdy_refine_tran = vec![6, 7];

    let report = apply_ngr_renew_fortran_indexed(
        iter,
        num_vertex,
        &num_mp,
        &num_wp,
        &mp_new,
        &wp_new,
        &ngrmw_new,
        &mut mp_f,
        &mut wp_f,
        &mut ngrmw_f,
        &mut ngrwm_f,
        &mut n_ngrwm_f,
        &mut bdy_refine,
        &mut bdy_refine_tran,
    )
    .expect("apply NGR_RENEW through CLI adapter");

    assert_eq!(report.num_sjx, 5);
    assert_eq!(report.num_dbx, 6);
    assert_eq!(report.vertex_mapping[5], 5);
    assert_eq!(report.vertex_mapping[6], 5);
    assert_eq!(report.vertex_mapping[7], 6);
    assert_eq!(report.adjacency_capacity, 7);
    assert_eq!(report.boundary_refine, vec![5, 5, 6]);
    assert_eq!(report.boundary_refine_transition, vec![5, 6]);

    assert_eq!(mp_f.len(), report.num_sjx + 1);
    assert_eq!(wp_f.len(), report.num_dbx + 1);
    assert_point(mp_f[5], 6.0, 6.0);
    assert_point(wp_f[5], 10.0, 1.0);
    assert_point(wp_f[6], 11.0, 2.0);

    assert_eq!(ngrmw_f.len(), 4);
    assert_eq!([ngrmw_f[1][4], ngrmw_f[2][4], ngrmw_f[3][4]], [2, 5, 5]);
    assert_eq!([ngrmw_f[1][5], ngrmw_f[2][5], ngrmw_f[3][5]], [4, 5, 6]);
    assert_eq!(n_ngrwm_f[5], 3);
    assert_eq!(ngrmw_f[1][1], 1);
    assert_eq!(ngrmw_f[2][1], 2);
    assert_eq!(ngrmw_f[3][1], 3);

    assert_eq!(ngrwm_f.len(), 8);
    assert_eq!(ngrwm_f[1][5], 4);
    assert_eq!(ngrwm_f[2][5], 4);
    assert_eq!(ngrwm_f[3][5], 5);
    assert_eq!(ngrwm_f[4][5], 1);
    assert_eq!(bdy_refine, vec![5, 5, 6]);
    assert_eq!(bdy_refine_tran, vec![5, 6]);
}

#[test]
fn ngr_renew_adapter_sorts_final_cell_adjacency_like_get_sort_new() {
    let iter = 2;
    let num_vertex = 1;
    let num_mp = vec![0, 1, 5];
    let num_wp = vec![0, 6, 6];
    let mut mp_new = vec![point(0.0, 0.0); num_mp[iter] + 1];
    mp_new[1] = point(-1.0, -1.0);
    mp_new[2] = point(2.0, 2.0);
    mp_new[3] = point(0.0, 0.0);
    mp_new[4] = point(1.0, 0.0);
    mp_new[5] = point(0.0, 1.0);
    let mut wp_new = vec![point(0.0, 0.0); num_wp[iter] + 1];
    for id in 1..=6 {
        wp_new[id] = point(id as f64, 0.0);
    }
    let ngrmw_new = vec![
        vec![0, 0, 0, 0, 0, 0],
        vec![0, 1, 6, 2, 4, 5],
        vec![0, 2, 3, 3, 3, 3],
        vec![0, 3, 1, 4, 5, 6],
    ];
    let mut mp_f = Vec::new();
    let mut wp_f = Vec::new();
    let mut ngrmw_f = Vec::new();
    let mut ngrwm_f = Vec::new();
    let mut n_ngrwm_f = Vec::new();
    let mut bdy_refine = Vec::new();
    let mut bdy_refine_tran = Vec::new();

    let report = apply_ngr_renew_fortran_indexed(
        iter,
        num_vertex,
        &num_mp,
        &num_wp,
        &mp_new,
        &wp_new,
        &ngrmw_new,
        &mut mp_f,
        &mut wp_f,
        &mut ngrmw_f,
        &mut ngrwm_f,
        &mut n_ngrwm_f,
        &mut bdy_refine,
        &mut bdy_refine_tran,
    )
    .expect("apply sorted NGR_RENEW through CLI adapter");

    assert_eq!(report.num_dbx, 6);
    assert_eq!(n_ngrwm_f[3], 4);
    assert_eq!(
        [ngrwm_f[1][3], ngrwm_f[2][3], ngrwm_f[3][3], ngrwm_f[4][3]],
        [3, 4, 5, 2]
    );
}
