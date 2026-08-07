//! Cell ancestry across refinement, spring and renumbering.
//!
//! Lineage answers "which face of the mesh before refinement did this cell come
//! from" after passes and a renumbering have moved every row. It is only worth
//! anything if it survives all three, so that is what these check.

use earthmesh_mesh::{LonLatDegrees, MethodCMesh, RefinementRegion};

const NXP: usize = 21;

fn base_mesh() -> MethodCMesh {
    MethodCMesh::from_icosahedron(NXP, 0, 1.0, 0.25, 0).expect("base mesh")
}

#[test]
fn an_unrefined_mesh_has_every_cell_as_its_own_ancestor() {
    let mesh = base_mesh();
    let m_lineage = mesh.gridfile_m_cell_lineages().expect("m lineage");
    let w_lineage = mesh.gridfile_w_cell_lineages().expect("w lineage");

    // Row 0 is the canonical placeholder; ids run from 1.
    for (offset, &ancestor) in m_lineage.iter().enumerate().skip(1) {
        let id = offset + 1;
        assert_eq!(ancestor, id as i64, "M cell {id} must descend from itself");
    }
    for (offset, &ancestor) in w_lineage.iter().enumerate().skip(1) {
        let id = offset + 1;
        assert_eq!(ancestor, id as i64, "W cell {id} must descend from itself");
    }
}

#[test]
fn refined_children_name_the_face_they_were_split_from() {
    let mesh = base_mesh();
    let before = mesh.gridfile_m_cell_lineages().expect("m lineage");
    let regions = vec![RefinementRegion::Circle {
        center: LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 1,
    }];
    let refined = mesh.spawn_nest(&regions, 1).expect("refine one level");
    let after = refined.gridfile_m_cell_lineages().expect("m lineage");

    assert!(
        after.len() > before.len(),
        "refinement must add cells: {} -> {}",
        before.len(),
        after.len()
    );

    // Every ancestor has to be a face that existed before the pass, or the
    // lineage points at something the caller cannot resolve.
    for (offset, &ancestor) in after.iter().enumerate() {
        let id = offset + 1;
        assert!(
            ancestor >= 1 && ancestor as usize <= before.len(),
            "M cell {id} names ancestor {ancestor}, outside the {} pre-refinement faces",
            before.len()
        );
    }

    // A split parent hands its lineage to four cells, so at least one ancestor
    // is named more than once -- otherwise nothing was actually subdivided.
    let mut counts = std::collections::BTreeMap::<i64, usize>::new();
    for &ancestor in &after {
        *counts.entry(ancestor).or_default() += 1;
    }
    assert!(
        counts.values().any(|count| *count >= 4),
        "a subdivided face must appear as the ancestor of four cells"
    );
}

#[test]
fn no_cell_of_either_kind_is_left_without_an_ancestor() {
    // Refinement creates M points at edge midpoints as well as splitting W
    // faces. Copying ancestry only for rows that already existed left every new
    // midpoint naming ancestor 0 -- a row that does not exist -- and the
    // earlier tests missed it because they only read the M-cell lineage, which
    // is derived from W faces. Both kinds are checked here.
    let mesh = base_mesh();
    let regions = vec![RefinementRegion::Circle {
        center: LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 1,
    }];
    let refined = mesh.spawn_nest(&regions, 1).expect("refine");

    for (label, lineage) in [
        (
            "M cell",
            refined.gridfile_m_cell_lineages().expect("m lineage"),
        ),
        (
            "W cell",
            refined.gridfile_w_cell_lineages().expect("w lineage"),
        ),
    ] {
        for (offset, &ancestor) in lineage.iter().enumerate() {
            assert!(
                ancestor >= 1,
                "{label} {} names ancestor {ancestor}, which is not a row",
                offset + 1
            );
        }
    }
}

#[test]
fn lineage_survives_a_second_pass() {
    // Two levels: the grandchildren must still name a face from the original
    // mesh, not an intermediate one that no longer exists.
    let mesh = base_mesh();
    let original_faces = mesh.gridfile_m_cell_lineages().expect("m lineage").len();
    let center = LonLatDegrees::new(114.0, 22.0);
    let regions = vec![
        RefinementRegion::Circle {
            center,
            radius_meters: 1_500_000.0,
            level: 1,
        },
        RefinementRegion::Circle {
            center,
            radius_meters: 400_000.0,
            level: 2,
        },
    ];
    let refined = mesh.spawn_nest(&regions, 2).expect("refine two levels");
    let after = refined.gridfile_m_cell_lineages().expect("m lineage");

    for (offset, &ancestor) in after.iter().enumerate() {
        let id = offset + 1;
        assert!(
            ancestor >= 1 && ancestor as usize <= original_faces,
            "M cell {id} names ancestor {ancestor}, outside the {original_faces} original faces"
        );
    }
}

#[test]
fn spring_moves_points_without_touching_ancestry() {
    let mesh = base_mesh();
    let regions = vec![RefinementRegion::Circle {
        center: LonLatDegrees::new(114.0, 22.0),
        radius_meters: 400_000.0,
        level: 1,
    }];
    let refined = mesh.spawn_nest(&regions, 1).expect("refine");
    let before = refined.gridfile_m_cell_lineages().expect("m lineage");

    let (sprung, _passes) = refined
        .spawn_nest_with_spring(&[], 0, NXP, 1)
        .expect("spring without new regions");
    let after = sprung.gridfile_m_cell_lineages().expect("m lineage");
    assert_eq!(before, after, "spring must not change ancestry");
}
