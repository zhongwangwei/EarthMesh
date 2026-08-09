use super::*;

fn point(x: f64, y: f64, z: f64) -> CartesianPoint {
    CartesianPoint::new(x, y, z)
}

/// The easy case, which the fast path has to get right without help.
#[test]
fn a_point_well_clear_of_the_plane_is_placed_without_ambiguity() {
    let a = point(1.0, 0.0, 0.0);
    let b = point(0.0, 1.0, 0.0);
    let c = point(0.0, 0.0, 1.0);

    let inside = orient3d(a, b, c, point(0.0, 0.0, 0.0)).expect("far from degenerate");
    let outside = orient3d(a, b, c, point(1.0, 1.0, 1.0)).expect("far from degenerate");
    assert_ne!(inside, Sign::Zero);
    assert_ne!(outside, Sign::Zero);
    assert_ne!(
        inside, outside,
        "the two sides of a plane have to come out different"
    );
}

/// Exactly coplanar is a fact about the input, and is reported as such.
#[test]
fn four_points_on_one_plane_are_reported_as_zero_not_guessed() {
    let sign = orient3d(
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 0.0),
        point(0.0, 1.0, 0.0),
        point(1.0, 1.0, 0.0),
    )
    .expect("exact zero is an answer, not an ambiguity");
    assert_eq!(sign, Sign::Zero);
}

/// Winding reverses when two corners swap, at every precision.
#[test]
fn swapping_two_corners_reverses_the_winding() {
    let a = point(1.0, 0.0, 0.0);
    let b = point(0.0, 1.0, 0.0);
    let c = point(0.0, 0.0, 1.0);
    let forward = orientation_on_sphere(a, b, c).expect("clear");
    let reversed = orientation_on_sphere(a, c, b).expect("clear");
    assert_ne!(forward, Sign::Zero);
    assert_ne!(forward, reversed);
}

/// The case the whole module exists for: a determinant the naive f64
/// evaluation gets wrong.
///
/// The three corners are nearly collinear and the fourth sits a hair off their
/// plane, so the six products cancel to far below what double precision can
/// carry. The naive sum comes out zero; the adaptive path still separates it.
#[test]
fn a_determinant_the_naive_sum_loses_is_still_decided() {
    let a = point(0.0, 0.0, 0.0);
    let b = point(1.0, 1.0, 1.0);
    let c = point(2.0, 2.0, 2.0 + 1e-15);
    let d = point(1.0, 2.0, 3.0);

    let naive = {
        let (ax, ay, az) = (a.x - d.x, a.y - d.y, a.z - d.z);
        let (bx, by, bz) = (b.x - d.x, b.y - d.y, b.z - d.z);
        let (cx, cy, cz) = (c.x - d.x, c.y - d.y, c.z - d.z);
        ax * (by * cz - bz * cy) - ay * (bx * cz - bz * cx) + az * (bx * cy - by * cx)
    };

    let adaptive = orient3d(a, b, c, d);
    match adaptive {
        Ok(Sign::Zero) => panic!("these four are not coplanar; the perturbation is real"),
        Ok(sign) => {
            // The adaptive path decided. If the naive sum had a sign at all it
            // must agree, and if it did not, this is exactly the rescue.
            if naive != 0.0 {
                let naive_sign = if naive > 0.0 {
                    Sign::Positive
                } else {
                    Sign::Negative
                };
                assert_eq!(sign, naive_sign, "the two paths must not disagree");
            }
        }
        Err(ambiguous) => panic!("this is decidable at extended precision: {ambiguous}"),
    }
}

/// Ambiguity is reported, never resolved by picking.
///
/// A configuration whose determinant is a true zero comes back as `Zero`; the
/// error variant exists for what neither precision can separate, and it carries
/// the numbers rather than a bare complaint.
#[test]
fn an_ambiguous_result_carries_what_it_could_not_separate() {
    let ambiguous = Ambiguous {
        residual: 1e-40,
        bound: 1e-38,
    };
    let rendered = ambiguous.to_string();
    assert!(rendered.contains("degenerate"), "{rendered}");
    assert!(rendered.contains("e-40"), "{rendered}");
}

/// Inside and outside a circumcircle, on the sphere.
#[test]
fn a_point_inside_a_spherical_circumcircle_is_told_from_one_outside() {
    // A small cap near the north pole, and two test points: one inside the cap
    // and one far away on the equator.
    let unit = |lon: f64, lat: f64| {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        point(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
    };
    let a = unit(0.0, 80.0);
    let b = unit(120.0, 80.0);
    let c = unit(240.0, 80.0);

    // Which sign means inside, not merely that the two differ. Asserting only
    // that they differ passes under either polarity, and the wrong polarity
    // makes every circumcircle appear to hold every point.
    assert_eq!(
        in_circle_on_sphere(a, b, c, unit(0.0, 90.0)).expect("clear"),
        Sign::Positive,
        "the pole is inside the cap those three corners cut"
    );
    for outside in [unit(0.0, 0.0), unit(0.0, -90.0), unit(60.0, 10.0)] {
        assert_eq!(
            in_circle_on_sphere(a, b, c, outside).expect("clear"),
            Sign::Negative,
            "a point off the cap is outside the circumcircle"
        );
    }
    let pole = in_circle_on_sphere(a, b, c, unit(0.0, 90.0)).expect("clear");

    // Reversing the triangle's winding must not change which points are inside
    // it. This is the reading the doc comment claims and the easiest one to get
    // backwards.
    let reversed_pole = in_circle_on_sphere(a, c, b, unit(0.0, 90.0)).expect("clear");
    assert_eq!(
        pole, reversed_pole,
        "inside is a property of the circle, not of how the corners were listed"
    );
}

/// A corner of the triangle sits exactly on its own circumcircle.
#[test]
fn a_corner_is_cocircular_with_its_own_triangle() {
    let a = point(1.0, 0.0, 0.0);
    let b = point(0.0, 1.0, 0.0);
    let c = point(0.0, 0.0, 1.0);
    assert_eq!(
        in_circle_on_sphere(a, b, c, a).expect("exact"),
        Sign::Zero,
        "a point on the circle is on the circle"
    );
}

/// The same question twice gives the same answer twice.
#[test]
fn the_predicates_are_deterministic() {
    let a = point(0.3, 0.7, 0.2);
    let b = point(-0.5, 0.1, 0.9);
    let c = point(0.11, -0.4, 0.6);
    let d = point(0.2, 0.2, 0.2);
    let first = orient3d(a, b, c, d);
    for _ in 0..8 {
        assert_eq!(orient3d(a, b, c, d), first);
    }
}

/// The extended-precision path is reached, and it is right when it is.
///
/// Without this the second tier is untested weight: on ordinary inputs the fast
/// filter decides everything, so the only way to exercise the rest is to feed
/// it configurations that cancel. These put the third corner almost on the line
/// through the first two, which is where the six products very nearly sum to
/// nothing.
///
/// Correctness is checked without a second implementation, by an invariant
/// the arithmetic cannot fake: exchanging two rows of a determinant negates it.
/// A sign error anywhere in the accumulation breaks it.
#[test]
fn the_extended_precision_path_is_exercised_and_stays_antisymmetric() {
    let mut state: u64 = 0x1234_5678;
    let mut next = || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (state >> 11) as f64 / (1u64 << 53) as f64
    };

    let epsilon = f64::EPSILON / 2.0;
    let (mut fast, mut refined, mut ambiguous) = (0usize, 0usize, 0usize);
    for _ in 0..20_000 {
        let a = point(next(), next(), next());
        let b = point(next(), next(), next());
        let along = next();
        let nudge = (next() - 0.5) * 1e-16;
        let c = point(
            a.x + along * (b.x - a.x) + nudge,
            a.y + along * (b.y - a.y) + nudge,
            a.z + along * (b.z - a.z) + nudge,
        );
        let d = point(next(), next(), next());

        // Whether the fast filter could have settled it, computed the same way
        // the predicate computes its bound.
        let (ax, ay, az) = (a.x - d.x, a.y - d.y, a.z - d.z);
        let (bx, by, bz) = (b.x - d.x, b.y - d.y, b.z - d.z);
        let (cx, cy, cz) = (c.x - d.x, c.y - d.y, c.z - d.z);
        let terms = [
            ax * (by * cz),
            -ax * (bz * cy),
            -ay * (bx * cz),
            ay * (bz * cx),
            az * (bx * cy),
            -az * (by * cx),
        ];
        let approximate: f64 = terms.iter().sum();
        let permanent: f64 = terms.iter().map(|term| term.abs()).sum();
        let settled_fast = approximate.abs() > (7.0 + 56.0 * epsilon) * epsilon * permanent;

        match orient3d(a, b, c, d) {
            Ok(sign) => {
                if settled_fast {
                    fast += 1;
                } else {
                    refined += 1;
                }
                let swapped = orient3d(b, a, c, d).expect("the same configuration is decidable");
                let expected = match sign {
                    Sign::Positive => Sign::Negative,
                    Sign::Negative => Sign::Positive,
                    Sign::Zero => Sign::Zero,
                };
                assert_eq!(
                    swapped, expected,
                    "exchanging two rows has to negate the determinant"
                );
            }
            Err(_) => ambiguous += 1,
        }
    }

    assert!(
        refined > fast,
        "these inputs are built to defeat the fast filter; it settled {fast} of {} \
         and the refined path only {refined}",
        fast + refined
    );
    assert_eq!(
        ambiguous, 0,
        "extended precision separates every one of these; anything else means the second tier \
         is not doing its job"
    );
}
