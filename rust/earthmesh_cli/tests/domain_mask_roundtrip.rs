//! Confirms the boundary-mask writers the GUI uses to author regional domains
//! produce files the engine readers accept (write → read identity on geometry).

#[test]
fn bbox_mask_round_trips() {
    let path = std::env::temp_dir().join("test_em_bbox_mask.nc");
    let mask = earthmesh_cli::BBoxMask {
        refine_degree: 0,
        points: vec![earthmesh_cli::BBoxPoint {
            west: 100.0,
            east: 120.0,
            north: 40.0,
            south: 20.0,
        }],
    };
    earthmesh_cli::write_bbox_mask_netcdf(&path, &mask).expect("write bbox mask");
    let read = earthmesh_cli::read_bbox_mask_netcdf(&path).expect("read bbox mask");
    assert_eq!(read.points.len(), 1);
    assert_eq!(read.points[0].west, 100.0);
    assert_eq!(read.points[0].east, 120.0);
    assert_eq!(read.points[0].north, 40.0);
    assert_eq!(read.points[0].south, 20.0);
}

#[test]
fn circle_mask_round_trips() {
    let path = std::env::temp_dir().join("test_em_circle_mask.nc");
    let mask = earthmesh_cli::CircleMask {
        refine_degree: 0,
        points: vec![earthmesh_cli::LonLatPoint {
            lon: 115.0,
            lat: 25.0,
        }],
        radius_km: vec![500.0],
    };
    earthmesh_cli::write_circle_mask_netcdf(&path, &mask).expect("write circle mask");
    let read = earthmesh_cli::read_circle_mask_netcdf(&path).expect("read circle mask");
    assert_eq!(read.points.len(), 1);
    assert_eq!(read.points[0].lon, 115.0);
    assert_eq!(read.points[0].lat, 25.0);
    assert_eq!(read.radius_km, vec![500.0]);
}
