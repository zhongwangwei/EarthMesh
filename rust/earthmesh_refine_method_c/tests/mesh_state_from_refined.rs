//! `MeshState` over a mesh Method-C refined.
//!
//! Lives here rather than beside `MeshState` because the shared crate cannot
//! build a refined mesh -- the refining is what moved out.

use earthmesh_refine_method_c::MethodCMesh;

/// A refined mesh converts too, which is what a backend lifted onto this type
/// would need.
#[test]
fn a_refined_method_c_mesh_also_converts() {
    let mesh = MethodCMesh::from_icosahedron(9, 0, 1.0, 0.25, 0).expect("base mesh");
    let refined = mesh
        .spawn_nest(
            &[earthmesh_mesh::RefinementRegion::Circle {
                center: earthmesh_mesh::LonLatDegrees::new(0.0, 0.0),
                radius_meters: 2_000_000.0,
                level: 1,
            }],
            1,
        )
        .expect("one level");
    let state = earthmesh_mesh::MeshState::from_triangular_mesh(&refined).expect("convert refined");
    assert_eq!(state.open_edge_count(), 0);
    state
        .validate()
        .expect("still a valid state after refining");
    assert!(
        state.triangle_count()
            > earthmesh_mesh::MeshState::from_triangular_mesh(&mesh)
                .unwrap()
                .triangle_count()
    );
}
