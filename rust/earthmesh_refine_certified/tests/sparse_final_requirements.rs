use earthmesh_mesh::RetirementSearchOutcome;
use earthmesh_refine_certified::{
    requirement::{certify_final_cell_requirements, SourceLevelField, TargetLevelField},
    MotherGrid,
};

#[test]
fn final_cell_requirements_ignore_retired_slots_after_vertex_retirement() {
    let mut mesh = MotherGrid::generate(2).unwrap().mesh;
    let before_vertices = mesh.vertex_count();
    let before_rows = mesh.vertices().len();
    let outcome = mesh.retire_vertex_with_budget_transactionally(2, usize::MAX, |state, _| {
        state.validate().is_ok()
    });
    assert!(matches!(outcome, RetirementSearchOutcome::Committed { .. }));
    assert!(mesh.vertex_count() < before_vertices);
    assert_eq!(mesh.vertices().len(), before_rows);

    let levels = vec![1; mesh.vertex_count()];
    let source = SourceLevelField::from_active_voronoi_cells(&mesh, levels.clone()).unwrap();
    let target = TargetLevelField::from_active_voronoi_cells(&mesh, levels).unwrap();

    let cert = certify_final_cell_requirements(&mesh, &source, &mesh, &target, 1).unwrap();
    assert_eq!(cert.target_cells(), mesh.vertex_count());
    assert_eq!(cert.physical_residuals() + cert.balance_residuals(), 0);
}
