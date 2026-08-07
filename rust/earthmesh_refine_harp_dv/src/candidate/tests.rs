use super::*;

use earthmesh_mesh::{lonlat_degrees_to_unit_xyz, LonLatDegrees, TriangularMesh};

fn sphere(nxp: usize) -> MeshState {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    MeshState::from_triangular_mesh(&mesh).expect("neutral state")
}

/// The ladder is in the order section 12.1 gives, and the witness leads it.
#[test]
fn the_ladder_runs_witness_then_farthest_then_off_centre_then_edge() {
    let state = sphere(6);
    let site = 40;
    let radius = magnitude(state.vertices()[site]);
    let unit = lonlat_degrees_to_unit_xyz(LonLatDegrees::new(31.0, 12.0));
    let witness = CartesianPoint::new(unit.x * radius, unit.y * radius, unit.z * radius);

    let with = candidates_for_site(&state, site, Some(witness), CandidatePolicy::default())
        .expect("ladder");
    assert_eq!(
        with.iter().map(|c| c.source).collect::<Vec<_>>(),
        vec![
            CandidateSource::Witness,
            CandidateSource::FarthestPoint,
            CandidateSource::OffCentre,
            CandidateSource::LongestEdgeMidpoint,
        ]
    );

    let without =
        candidates_for_site(&state, site, None, CandidatePolicy::default()).expect("ladder");
    assert_eq!(
        without.iter().map(|c| c.source).collect::<Vec<_>>(),
        vec![
            CandidateSource::FarthestPoint,
            CandidateSource::OffCentre,
            CandidateSource::LongestEdgeMidpoint,
        ],
        "no witness means no witness rung, not a shifted ladder"
    );
}

/// Every generated candidate is on the sphere the site lives on.
///
/// The chord midpoint of an edge is inside the sphere and the circumcentre of
/// a triangle is too. `insert_site` refuses a point off the mesh's sphere, so a
/// generator that skipped the projection would produce a ladder of nothing but
/// refusals and report every demand unresolvable for a reason unrelated to the
/// demand.
#[test]
fn every_candidate_is_on_the_sphere() {
    let state = sphere(6);
    for site in [7usize, 40, 120, 300] {
        let radius = magnitude(state.vertices()[site]);
        let ladder =
            candidates_for_site(&state, site, None, CandidatePolicy::default()).expect("ladder");
        assert!(!ladder.is_empty());
        for candidate in ladder {
            let offset = (magnitude(candidate.point) - radius).abs() / radius;
            assert!(
                offset < 1.0e-9,
                "{:?} is at {} and the site is at {radius}",
                candidate.source,
                magnitude(candidate.point)
            );
        }
    }
}

/// The off-centre rung is not the farthest-point rung under another name.
#[test]
fn the_off_centre_is_short_of_the_corner_it_aims_at() {
    let state = sphere(6);
    let site = 40;
    let centre = state.vertices()[site];
    let ladder =
        candidates_for_site(&state, site, None, CandidatePolicy::default()).expect("ladder");
    let farthest = ladder
        .iter()
        .find(|c| c.source == CandidateSource::FarthestPoint)
        .expect("rung");
    let off_centre = ladder
        .iter()
        .find(|c| c.source == CandidateSource::OffCentre)
        .expect("rung");

    let to_farthest = arc_length_unit_sphere(centre, farthest.point);
    let to_off_centre = arc_length_unit_sphere(centre, off_centre.point);
    assert!(
        to_off_centre < to_farthest,
        "{to_off_centre} < {to_farthest}"
    );
    assert!(
        to_off_centre > to_farthest * 0.5,
        "and not so short that it barely moves: {to_off_centre} vs {to_farthest}"
    );
}

/// A separation floor drops the candidates that violate it.
#[test]
fn the_separation_floor_removes_candidates_too_near_a_site() {
    let state = sphere(6);
    let site = 40;
    let generous = candidates_for_site(
        &state,
        site,
        None,
        CandidatePolicy {
            min_separation_m: 1.0,
        },
    )
    .expect("ladder");
    // Far larger than any cell on an NXP 6 sphere, so nothing can satisfy it.
    let impossible = candidates_for_site(
        &state,
        site,
        None,
        CandidatePolicy {
            min_separation_m: 1.0e9,
        },
    )
    .expect("ladder");

    assert!(!generous.is_empty());
    assert!(
        impossible.is_empty(),
        "a floor no candidate meets leaves an empty ladder rather than a violating one"
    );
}

/// The same site gives the same ladder, point for point.
#[test]
fn the_ladder_is_deterministic() {
    let state = sphere(6);
    for site in [7usize, 40, 300] {
        let first = candidates_for_site(&state, site, None, CandidatePolicy::default());
        let second = candidates_for_site(&state, site, None, CandidatePolicy::default());
        assert_eq!(first.expect("ladder"), second.expect("ladder"));
    }
}

/// A candidate's hint really is a triangle at the site it refines.
#[test]
fn the_hint_is_a_triangle_at_the_site() {
    let state = sphere(6);
    for site in [7usize, 40, 120] {
        for candidate in
            candidates_for_site(&state, site, None, CandidatePolicy::default()).expect("ladder")
        {
            assert!(
                state.triangles()[candidate.hint].contains(&site),
                "the walk would start somewhere unrelated"
            );
        }
    }
}

/// A site the mesh does not carry produces an error, not an empty ladder.
///
/// An empty ladder and an unreadable cell mean different things to the layer
/// above: one is a demand with nowhere legal to go, the other is a bug.
#[test]
fn a_site_that_is_not_there_is_an_error_not_an_empty_ladder() {
    let state = sphere(6);
    let error = candidates_for_site(
        &state,
        state.vertices().len(),
        None,
        CandidatePolicy::default(),
    )
    .expect_err("not a site of this mesh");
    assert!(matches!(error, VoronoiError::UnknownSite { .. }), "{error}");
}
