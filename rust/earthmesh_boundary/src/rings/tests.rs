use super::*;

/// Two separate rings come back separately, each in traversal order.
#[test]
fn disjoint_rings_are_walked_apart() {
    let edges = vec![
        (0, 1),
        (1, 2),
        (2, 0),
        (10, 11),
        (11, 12),
        (12, 13),
        (13, 10),
    ];
    let rings = closed_rings(&edges).expect("two rings");
    assert_eq!(rings.len(), 2, "{rings:?}");
    assert_eq!(rings[0], vec![0, 1, 2]);
    assert_eq!(rings[1].len(), 4);
    // Traversal order, not input order: consecutive entries share an edge.
    for ring in &rings {
        for step in 0..ring.len() {
            let pair = (ring[step], ring[(step + 1) % ring.len()]);
            assert!(
                edges.contains(&pair) || edges.contains(&(pair.1, pair.0)),
                "{pair:?} is not an edge of {edges:?}"
            );
        }
    }
}

/// A curve that stops in mid-air is refused, naming the vertex.
#[test]
fn an_open_curve_is_refused_rather_than_closed_for_the_caller() {
    let error = closed_rings(&[(0, 1), (1, 2)]).expect_err("open");
    assert!(
        matches!(
            error,
            RingError::NotTwoNeighbours {
                vertex: 0,
                neighbours: 1
            }
        ) || matches!(
            error,
            RingError::NotTwoNeighbours {
                vertex: 2,
                neighbours: 1
            }
        ),
        "{error}"
    );
}

/// A junction is refused too: the walk would have to choose, and either choice
/// invents a boundary the caller did not give.
#[test]
fn a_junction_is_refused() {
    let error =
        closed_rings(&[(0, 1), (1, 2), (2, 0), (1, 5), (5, 6), (6, 1)]).expect_err("junction");
    assert!(
        matches!(
            error,
            RingError::NotTwoNeighbours {
                vertex: 1,
                neighbours: 4
            }
        ),
        "{error}"
    );
}

/// The same edges in a different order give the same rings.
#[test]
fn the_walk_is_deterministic() {
    let forward = vec![(0, 1), (1, 2), (2, 3), (3, 0)];
    let shuffled = vec![(3, 0), (1, 2), (0, 1), (2, 3)];
    assert_eq!(
        closed_rings(&forward).expect("ring"),
        closed_rings(&shuffled).expect("ring")
    );
}

/// No edges is no rings, not an error: a pass that marked nothing has no
/// frontier, and that is an ordinary outcome.
#[test]
fn no_edges_is_no_rings() {
    assert_eq!(closed_rings(&[]).expect("none"), Vec::<Vec<usize>>::new());
}
