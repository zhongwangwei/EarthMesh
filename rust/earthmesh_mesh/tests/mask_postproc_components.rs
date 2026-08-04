use earthmesh_mesh::retain_largest_edge_connected_component_one_based;

/// Builds Canonical 1-based neighbour tables from a `(cell_id, [vertex_id; N])`
/// list. Slots 0 and 1 stay placeholders, matching the carve layout.
struct Fixture {
    is_in_domain: Vec<i32>,
    center_neighbors: Vec<Vec<usize>>,
    center_neighbor_counts: Vec<usize>,
    vertex_neighbors: Vec<Vec<usize>>,
    vertex_neighbor_counts: Vec<usize>,
}

impl Fixture {
    fn new(cells: &[(usize, Vec<usize>)]) -> Self {
        let cell_capacity = cells.iter().map(|(id, _)| *id).max().unwrap_or(1) + 1;
        let vertex_capacity = cells
            .iter()
            .flat_map(|(_, vertices)| vertices.iter().copied())
            .max()
            .unwrap_or(1)
            + 1;

        let mut is_in_domain = vec![-1i32; cell_capacity];
        let mut center_neighbors = vec![Vec::new(); cell_capacity];
        let mut center_neighbor_counts = vec![0usize; cell_capacity];
        let mut vertex_neighbors = vec![Vec::new(); vertex_capacity];
        let mut vertex_neighbor_counts = vec![0usize; vertex_capacity];

        for (cell_id, vertices) in cells {
            is_in_domain[*cell_id] = 1;
            center_neighbors[*cell_id] = vertices.clone();
            center_neighbor_counts[*cell_id] = vertices.len();
            for vertex_id in vertices {
                vertex_neighbors[*vertex_id].push(*cell_id);
                vertex_neighbor_counts[*vertex_id] += 1;
            }
        }

        Self {
            is_in_domain,
            center_neighbors,
            center_neighbor_counts,
            vertex_neighbors,
            vertex_neighbor_counts,
        }
    }

    fn retain_largest(&mut self) -> earthmesh_mesh::LargestComponentRetention {
        retain_largest_edge_connected_component_one_based(
            &mut self.is_in_domain,
            &self.center_neighbors,
            &self.center_neighbor_counts,
            &self.vertex_neighbors,
            &self.vertex_neighbor_counts,
        )
        .expect("retain largest component")
    }
}

#[test]
fn orphan_cell_sharing_no_edge_is_dropped() {
    // Cells 2,3,4 form an edge-connected strip; cell 9 stands alone.
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (4, vec![12, 13, 14]),
        (9, vec![90, 91, 92]),
    ]);

    let report = fixture.retain_largest();

    assert_eq!(report.component_count, 2);
    assert_eq!(report.retained_cell_count, 3);
    assert_eq!(report.removed_cell_ids, vec![9]);
    assert_eq!(fixture.is_in_domain[9], -1);
    for cell_id in [2, 3, 4] {
        assert_eq!(fixture.is_in_domain[cell_id], 1);
    }
}

#[test]
fn vertex_only_contact_does_not_hold_a_piece_in_the_domain() {
    // Cell 9 touches the 2-3-4 strip at vertex 14 only — an hourglass pinch,
    // exactly the non-manifold vertex fan the quality gate rejects.
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (4, vec![12, 13, 14]),
        (9, vec![14, 91, 92]),
    ]);

    let report = fixture.retain_largest();

    // Cell 9 shares only vertex 14, so it is its own edge-connected component.
    assert_eq!(report.component_count, 2);
    assert_eq!(report.removed_cell_ids, vec![9]);
    assert_eq!(fixture.is_in_domain[9], -1);
}

#[test]
fn smaller_multi_cell_component_is_dropped_whole() {
    // A 2-cell bay (7,8) loses to the 3-cell main body (2,3,4).
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (4, vec![12, 13, 14]),
        (7, vec![70, 71, 72]),
        (8, vec![71, 72, 73]),
    ]);

    let report = fixture.retain_largest();

    assert_eq!(report.component_count, 2);
    assert_eq!(report.retained_cell_count, 3);
    assert_eq!(report.removed_cell_ids, vec![7, 8]);
}

#[test]
fn fully_connected_domain_is_left_untouched() {
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (4, vec![12, 13, 14]),
    ]);

    let report = fixture.retain_largest();

    assert_eq!(report.component_count, 1);
    assert_eq!(report.retained_cell_count, 3);
    assert!(report.removed_cell_ids.is_empty());
    for cell_id in [2, 3, 4] {
        assert_eq!(fixture.is_in_domain[cell_id], 1);
    }
}

#[test]
fn equal_sized_components_keep_the_smallest_cell_id() {
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (7, vec![70, 71, 72]),
        (8, vec![71, 72, 73]),
    ]);

    let report = fixture.retain_largest();

    assert_eq!(report.component_count, 2);
    assert_eq!(report.retained_cell_count, 2);
    assert_eq!(report.removed_cell_ids, vec![7, 8]);
    assert_eq!(fixture.is_in_domain[2], 1);
}

#[test]
fn empty_domain_reports_no_components() {
    let mut is_in_domain = vec![0, 0, -1, -1];
    let report = retain_largest_edge_connected_component_one_based(
        &mut is_in_domain,
        &[vec![], vec![], vec![], vec![]],
        &[0, 0, 0, 0],
        &[vec![]],
        &[0],
    )
    .expect("empty domain is not an error");

    assert_eq!(report.component_count, 0);
    assert_eq!(report.retained_cell_count, 0);
    assert!(report.removed_cell_ids.is_empty());
}

#[test]
fn pinch_vertex_loses_its_smaller_fan_even_when_both_fans_rejoin() {
    // Cells 2,3 and 6,7 pinch at vertex 12 and are also joined the long way
    // round through 4,5 — one component, but still a non-manifold vertex.
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12]),
        (3, vec![11, 12, 13]),
        (4, vec![13, 20, 21]),
        (5, vec![20, 21, 22]),
        (6, vec![12, 22, 23]),
        (7, vec![12, 23, 24]),
    ]);
    // Stitch the long way round so every cell is edge-connected.
    fixture.center_neighbors[4] = vec![13, 11, 20];
    fixture.vertex_neighbors[11].push(4);
    fixture.vertex_neighbor_counts[11] += 1;
    fixture.center_neighbors[5] = vec![20, 13, 22];
    fixture.vertex_neighbors[13].push(5);
    fixture.vertex_neighbor_counts[13] += 1;
    fixture.center_neighbors[6] = vec![12, 22, 20];
    fixture.vertex_neighbors[20].push(6);
    fixture.vertex_neighbor_counts[20] += 1;

    let report = fixture.retain_largest();

    assert!(
        report.non_manifold_removed_cell_count > 0,
        "the pinch at vertex 12 must cost at least one cell"
    );
    assert!(!report.removed_cell_ids.is_empty());
    // Whatever survives, no in-domain vertex may still carry two fans.
    for vertex_id in 0..fixture.vertex_neighbors.len() {
        let incident: Vec<usize> = fixture.vertex_neighbors[vertex_id]
            .iter()
            .copied()
            .filter(|&cell_id| fixture.is_in_domain[cell_id] == 1)
            .collect();
        if incident.len() < 2 {
            continue;
        }
        let shares_edge = |a: usize, b: usize| {
            fixture.center_neighbors[a]
                .iter()
                .filter(|v| fixture.center_neighbors[b].contains(v))
                .count()
                >= 2
        };
        assert!(
            incident.iter().skip(1).all(|&other| incident
                .iter()
                .any(|&first| first != other && shares_edge(first, other))),
            "vertex {vertex_id} still pinches"
        );
    }
}

#[test]
fn hexagonal_cells_use_the_same_two_vertex_edge_rule() {
    // Two hexes sharing vertices 12,13 are adjacent; the third shares only 20.
    let mut fixture = Fixture::new(&[
        (2, vec![10, 11, 12, 13, 14, 15]),
        (3, vec![12, 13, 16, 17, 18, 19]),
        (4, vec![20, 21, 22, 23, 24, 25]),
    ]);
    fixture.center_neighbors[4] = vec![19, 21, 22, 23, 24, 25];
    fixture.vertex_neighbors[19].push(4);
    fixture.vertex_neighbor_counts[19] += 1;

    let report = fixture.retain_largest();

    assert_eq!(report.component_count, 2);
    assert_eq!(report.retained_cell_count, 2);
    assert_eq!(report.removed_cell_ids, vec![4]);
}
