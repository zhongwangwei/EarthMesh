//! `quality_input_from_gridfile` must derive cell adjacency from shared edges, so the
//! topology validator / neighbor-reciprocity / transition metrics are not no-ops on a
//! real gridfile. Pure (no NetCDF): we hand-build a tiny `GridfileMeshPoints`.

use earthmesh_cli::{quality_input_from_gridfile, GridfileMeshPoints};

#[test]
fn cell_neighbors_are_derived_from_shared_edges() {
    // 7 W vertices; triangles A{0,1,2} and B{0,1,3} share edge (0,1); C{4,5,6} is
    // isolated. m_to_w is 1-based.
    let mesh = GridfileMeshPoints {
        m_lon: vec![0.0; 3],
        m_lat: vec![0.0; 3],
        w_lon: vec![0.0, 1.0, 0.5, 0.5, 10.0, 11.0, 10.5],
        w_lat: vec![0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 1.0],
        m_to_w: vec![1, 2, 3, 1, 2, 4, 5, 6, 7],
        m_refine_level: vec![],
        w_to_m: vec![],
        w_to_m_width: 0,
        n_w: vec![],
        w_refine_level: vec![],
    };

    let input = quality_input_from_gridfile(&mesh);
    assert_eq!(input.cells.len(), 3, "three valid triangles");

    // A and B are reciprocal neighbors; C is isolated.
    assert_eq!(input.cells[0].neighbors, vec![1], "cell A neighbor = B");
    assert_eq!(input.cells[1].neighbors, vec![0], "cell B neighbor = A");
    assert!(input.cells[2].neighbors.is_empty(), "cell C is isolated");

    // The quality report's topology validator now has real adjacency to work with:
    // the isolated cell is flagged, the connected pair is not.
    let report =
        earthmesh_quality::compute(&input, &earthmesh_quality::QualityThresholds::default());
    assert_eq!(
        report.topology.orphan_cell_count, 1,
        "only the isolated cell is orphan"
    );
}
