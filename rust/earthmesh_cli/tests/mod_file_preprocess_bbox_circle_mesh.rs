use std::fs;

use earthmesh_cli::{
    bbox_mask_io::BBoxMesh, bbox_mask_io::BBoxPoint, circle_close_mask_io::CircleMesh,
    coordinate_types::LonLatPoint,
};

#[test]
fn bbox_mesh_read_write_preserves_canonical_schema_without_refine_field() {
    let root = temp_root("earthmesh_cli_bbox_mesh_roundtrip");
    let output = root.join("bbox_mesh.nc4");
    let mesh = BBoxMesh {
        points: vec![
            BBoxPoint {
                west: 110.0,
                east: 111.0,
                north: 23.0,
                south: 22.0,
            },
            BBoxPoint {
                west: 170.0,
                east: -170.0,
                north: 2.0,
                south: -3.0,
            },
        ],
    };

    earthmesh_cli::bbox_mask_io::write_bbox_mesh_netcdf(&output, &mesh).expect("write bbox mesh");
    let file = netcdf::open(&output).expect("open bbox mesh");
    assert_eq!(file.dimension("bbox_num").expect("bbox_num").len(), 2);
    assert_eq!(file.dimension("four").expect("four").len(), 4);
    assert!(file.variable("bbox_refine").is_none());

    let roundtrip =
        earthmesh_cli::bbox_mask_io::read_bbox_mesh_netcdf(&output).expect("read bbox mesh");
    assert_eq!(roundtrip, mesh);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn circle_mesh_read_write_preserves_canonical_schema_without_refine_field() {
    let root = temp_root("earthmesh_cli_circle_mesh_roundtrip");
    let output = root.join("circle_mesh.nc4");
    let mesh = CircleMesh {
        points: vec![
            LonLatPoint {
                lon: 113.0,
                lat: 22.0,
            },
            LonLatPoint {
                lon: -10.5,
                lat: 4.25,
            },
        ],
        radius_km: vec![15.0, 20.5],
    };

    earthmesh_cli::circle_close_mask_io::write_circle_mesh_netcdf(&output, &mesh)
        .expect("write circle mesh");
    let file = netcdf::open(&output).expect("open circle mesh");
    assert_eq!(file.dimension("circle_num").expect("circle_num").len(), 2);
    assert_eq!(file.dimension("two").expect("two").len(), 2);
    assert!(file.variable("circle_refine").is_none());

    let roundtrip = earthmesh_cli::circle_close_mask_io::read_circle_mesh_netcdf(&output)
        .expect("read circle mesh");
    assert_eq!(roundtrip, mesh);

    let _ = fs::remove_dir_all(&root);
}

fn temp_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("{label}_{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create temp root");
    root
}
