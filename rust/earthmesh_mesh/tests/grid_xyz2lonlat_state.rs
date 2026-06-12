use earthmesh_core::GridMemory;
use earthmesh_mesh::grid_xyz2lonlat_state;

#[test]
fn grid_xyz2lonlat_state_fills_m_and_w_lonlat_arrays_like_mkgrd() {
    let mut grid = GridMemory {
        nma: 3,
        nwa: 3,
        xem: vec![0.0, 1.0, 0.0],
        yem: vec![0.0, 0.0, 1.0],
        zem: vec![0.0, 0.0, 0.0],
        xew: vec![0.0, 0.0, 0.0],
        yew: vec![0.0, 1.0, 0.0],
        zew: vec![0.0, 0.0, 1.0],
        ..GridMemory::default()
    };

    grid_xyz2lonlat_state(&mut grid).expect("fill lonlat arrays");

    assert_eq!(grid.glonm.len(), 3);
    assert_eq!(grid.glatm.len(), 3);
    assert_eq!(grid.glonw.len(), 3);
    assert_eq!(grid.glatw.len(), 3);
    assert_eq!(grid.glonm[0], 0.0);
    assert_eq!(grid.glatm[0], 0.0);
    assert!((grid.glonm[1] - 0.0).abs() < 1.0e-6);
    assert!((grid.glatm[1] - 0.0).abs() < 1.0e-6);
    assert!((grid.glonm[2] - 90.0).abs() < 1.0e-6);
    assert!((grid.glatm[2] - 0.0).abs() < 1.0e-6);
    assert!((grid.glonw[1] - 90.0).abs() < 1.0e-6);
    assert!((grid.glatw[1] - 0.0).abs() < 1.0e-6);
    assert!((grid.glonw[2] - 0.0).abs() < 1.0e-6);
    assert!((grid.glatw[2] - 90.0).abs() < 1.0e-6);
}

#[test]
fn grid_xyz2lonlat_state_rejects_short_coordinate_arrays() {
    let mut grid = GridMemory {
        nma: 2,
        nwa: 1,
        xem: vec![0.0],
        yem: vec![0.0, 0.0],
        zem: vec![0.0, 0.0],
        xew: vec![0.0],
        yew: vec![0.0],
        zew: vec![0.0],
        ..GridMemory::default()
    };

    let err = grid_xyz2lonlat_state(&mut grid).expect_err("short M x array should fail");
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
}
