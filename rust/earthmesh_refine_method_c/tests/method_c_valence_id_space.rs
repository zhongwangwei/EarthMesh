//! What the valence error's M point actually indexes, pinned.
//!
//! The repair ladder in `method_c_spawn_pass` reads this id and hands it to
//! parent-indexed repairs. It is not a parent id. Guide 11.5 has the
//! measurements; this file is the executable half, so a reader who assumes the
//! id spaces agree finds out here rather than in a mesh.

use earthmesh_refine_method_c::{LonLatDegrees, MethodCMesh, RefinementRegion};

/// The id in the valence error is outside the parent mesh's id space.
///
/// One circle at NXP 6 is enough. Guide 11.5 used to record this defect as
/// needing a 7,022-circle coastal band on a spring-relaxed base mesh; it does
/// not, and this runs in under two seconds.
///
/// The assertion is `im > nmd`, which is a one-way test: it can only fire when
/// the id is definitely not a parent id, and it is silent on the far more
/// common case where a child id happens to land inside the parent's range.
/// That case is the one the ladder acts on, 379 times over this crate's lib
/// suite, and no assertion can catch it from out here -- which is why the guide
/// carries the counts and this only carries the proof.
#[test]
fn valence_error_names_a_point_in_the_emitted_mesh_not_the_parent() {
    let mesh = MethodCMesh::from_icosahedron(6, 0, 1.0, 0.25).expect("base Method-C mesh");
    let regions = [RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 900_000.0,
        level: 1,
    }];

    let error = mesh
        .spawn_nest(&regions, 5)
        .expect_err("one circle at NXP 6 overflows a Method-C ring");
    let message = error.to_string();
    assert!(
        message.contains("exceeds 7-edge Method-C ring"),
        "expected the valence error, got {error}"
    );

    let reported: usize = message
        .split("M point ")
        .nth(1)
        .and_then(|tail| {
            tail.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .ok()
        })
        .unwrap_or_else(|| panic!("no M point in {error}"));

    assert!(
        reported > mesh.nmd,
        "the valence error reported M point {reported}, which is inside the parent mesh's \
         {} points -- if emission has stopped renumbering, the ladder's `im <= self.nmd` \
         guard means what it looks like it means and guide 11.5 needs rewriting",
        mesh.nmd
    );
}
