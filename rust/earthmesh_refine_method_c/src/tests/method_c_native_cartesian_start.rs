//! Where a native Cartesian-XY refinement starts its march.
//!
//! Was beside the region types in the shared crate; it is a selection test, so
//! it moved with the selection.

use crate::MethodCMesh;
use earthmesh_mesh::{active_mesh_radius, LonLatDegrees, RefinementRegion};

#[test]
fn method_c_native_cartesian_start_uses_imcent_not_global_pentagon_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let method_c_m_neighbors = mesh.method_c_m_neighbors().expect("Method-C M neighbors");
    let pentagon = mesh.impent[0];
    let non_pentagon = (2..=mesh.nmd)
        .find(|im| !mesh.impent.contains(im))
        .expect("non-pentagon M point");
    let pentagon_xy = mesh.m_points[pentagon];
    let anchor_xy = mesh.m_points[non_pentagon];
    let radius_meters = (anchor_xy.x - pentagon_xy.x).hypot(anchor_xy.y - pentagon_xy.y) * 1.01;
    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(anchor_xy.x, anchor_xy.y),
        radius_meters,
        level: 1,
    };

    assert!(region.contains_cartesian_xy(pentagon_xy));
    let start = mesh
        .method_c_refinement_start_point_with_neighbors(
            &region,
            active_mesh_radius(&mesh).expect("mesh radius"),
            &method_c_m_neighbors,
            true,
        )
        .expect("cartesian Method-C start");

    assert_eq!(
        start, non_pentagon,
        "Canonical mdomain >= 2 skips impent logic and starts from imcent"
    );
}

#[test]
fn method_c_selected_faces_do_not_pre_expand_for_future_levels_like_canonical() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let region_level_one = RefinementRegion::Circle {
        center: LonLatDegrees::new(105.0, 35.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let region_level_two = RefinementRegion::Circle {
        center: LonLatDegrees::new(105.0, 35.0),
        radius_meters: 2_500_000.0,
        level: 2,
    };

    let selected_level_one = mesh
        .selected_region_faces(&region_level_one, 1, false)
        .expect("level-one selected faces");
    let selected_level_two = mesh
        .selected_region_faces(&region_level_two, 1, false)
        .expect("level-two pass-one selected faces");

    assert_eq!(
            selected_level_one, selected_level_two,
            "Canonical spawn_nest selects each NN independently and does not pre-expand pass 1 for future nested grids"
        );
}
