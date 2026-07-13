#[test]
fn orial_vertices_protect_preserves_refinement_markers_as_canonical_noop() {
    let mut ref_sjx = vec![0, 1, 0, 1];
    earthmesh_mesh::refine_orial_vertices_protect_one_based(&mut ref_sjx);
    assert_eq!(ref_sjx, vec![0, 1, 0, 1]);
}
