//! check_mpas_mesh_topology must (a) pass a self-consistent mesh and report the
//! right Euler characteristic, (b) catch a deliberately broken cross-reference.

use earthmesh_cli::{check_mpas_mesh_topology, MpasMesh};

/// Two triangular cells sharing edge e1 — a consistent open patch (disk, χ=1).
fn two_cell_open() -> MpasMesh {
    MpasMesh {
        lat_cell: vec![0.0, 11.0, 22.0],
        lon_cell: vec![0.0, 11.0, 22.0],
        x_cell: vec![0.0, 11.0, 22.0],
        y_cell: vec![0.0, 11.0, 22.0],
        z_cell: vec![0.0, 11.0, 22.0],
        lat_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        lon_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        x_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        y_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        z_vertex: vec![0.0, 101.0, 102.0, 103.0, 104.0],
        lat_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        lon_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        x_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        y_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        z_edge: vec![0.0, 201.0, 202.0, 203.0, 204.0, 205.0],
        n_edges_on_cell: vec![0, 3, 3],
        cells_on_cell: vec![vec![0, 0, 0], vec![2, 0, 0], vec![1, 0, 0]],
        vertices_on_cell: vec![vec![0, 0, 0], vec![1, 2, 3], vec![1, 2, 4]],
        edges_on_cell: vec![vec![0, 0, 0], vec![1, 2, 3], vec![1, 4, 5]],
        cells_on_vertex: vec![
            vec![0, 0, 0],
            vec![1, 2, 0],
            vec![1, 2, 0],
            vec![1, 0, 0],
            vec![2, 0, 0],
        ],
        edges_on_vertex: vec![
            vec![0, 0, 0],
            vec![1, 2, 4],
            vec![1, 3, 5],
            vec![2, 3, 0],
            vec![4, 5, 0],
        ],
        cells_on_edge: vec![[0, 0], [1, 2], [1, 0], [1, 0], [2, 0], [2, 0]],
        vertices_on_edge: vec![[0, 0], [1, 2], [1, 3], [2, 3], [1, 4], [2, 4]],
        n_edges_on_edge: vec![0, 0, 0, 0, 0, 0],
        edges_on_edge: vec![vec![], vec![], vec![], vec![], vec![], vec![]],
        area_cell: vec![0.0, 1.0, 1.0],
        area_triangle: vec![0.0, 1.0, 1.0, 1.0, 1.0],
        kite_areas_on_vertex: vec![vec![0.0; 3]; 5],
        dv_edge: vec![0.0; 6],
        dc_edge: vec![0.0; 6],
        angle_edge: vec![0.0; 6],
        weights_on_edge: vec![vec![]; 6],
        mesh_density: vec![0.0, 1.0, 1.0],
        nominal_min_dc: 0.5,
        error_segment: vec![0.0; 6],
    }
}

#[test]
fn consistent_open_patch_passes_with_disk_euler() {
    let m = two_cell_open();
    let r = check_mpas_mesh_topology(&m);
    assert!(
        r.is_consistent(),
        "unexpected violations: {:?}",
        r.violations
    );
    assert_eq!((r.n_cells, r.n_vertices, r.n_edges), (2, 4, 5));
    assert_eq!(r.euler_characteristic, 1); // disk
    assert!(!r.is_closed);
    assert_eq!(r.boundary_edges, 4); // all but the shared edge e1
}

#[test]
fn broken_cellsoncell_symmetry_is_caught() {
    let mut m = two_cell_open();
    // Cell 1 still claims cell 2 as neighbour, but cell 2 forgets cell 1.
    m.cells_on_cell[2] = vec![0, 0, 0];
    let r = check_mpas_mesh_topology(&m);
    assert!(!r.is_consistent());
    assert!(
        r.violations.iter().any(|s| s.contains("asymmetry")),
        "expected an asymmetry violation, got {:?}",
        r.violations
    );
}

#[test]
fn broken_edge_cell_reference_is_caught() {
    let mut m = two_cell_open();
    // Edge 1 claims cell 2, but cell 2 no longer lists edge 1.
    m.edges_on_cell[2] = vec![9, 4, 5]; // 9 is also out of range
    let r = check_mpas_mesh_topology(&m);
    assert!(!r.is_consistent());
}

/// When the locally-built global hex gridfile is present, the full global MPAS
/// mesh must be a closed sphere (χ=2, no boundary edges) and fully consistent.
#[test]
fn global_mpas_is_closed_sphere_when_fixture_present() {
    let gf = std::path::Path::new(
        "/tmp/earthmesh_cases/quickstart_n16/gridfile/gridfile_NXP0016_01_hex.nc4",
    );
    if !gf.exists() {
        eprintln!("skip: global hex gridfile fixture not present");
        return;
    }
    let mesh = earthmesh_cli::read_unstructured_mesh_netcdf(gf).unwrap();
    let cw = vec![100.0f64; mesh.w_points.len()];
    let g = earthmesh_cli::build_mpas_mesh_from_unstructured_fortran_indexed(&mesh, &cw, 16, 1)
        .unwrap();
    let r = check_mpas_mesh_topology(&g);
    assert!(
        r.is_consistent(),
        "global violations: {:?}",
        &r.violations[..r.violations.len().min(8)]
    );
    assert_eq!(
        r.euler_characteristic, 2,
        "global mesh should be a closed sphere"
    );
    assert_eq!(r.boundary_edges, 0);
    assert!(r.is_closed);
}
