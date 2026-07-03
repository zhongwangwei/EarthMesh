use std::io;

use crate::LonLatDegrees;

/// H-field driven refinement marking for the grid_preprocess refine loop.
///
/// Replaces only the mark-generation step of the discrete pipeline: instead of
/// per-criterion threshold comparisons or per-region containment tests
/// producing `ref_sjx`, the caller supplies a quantized target-level closure
/// (typically `|lon, lat| hfield.level_at(lon, lat, h_base, max_level)` from
/// `earthmesh_hfield`, with the field already composed via `min` and
/// gradient-limited). Everything downstream (iterB/C/D transition logic, LOP,
/// subdivision, renewal) is unchanged.
///
/// Semantics: `refine_round` is the 1-based refinement round of the caller's
/// loop. A triangle is marked (`ref_sjx = 1`) when it is still unrefined
/// (`mrl_new == 1`) and the sampled target level at its center is at least
/// `refine_round` — i.e. round 1 marks everything the field wants refined at
/// all, round 2 marks what wants a second halving, and so on. With a
/// gradient-limited field the marked sets of successive rounds are properly
/// nested rings with bounded shrink per round, so the discrete engine's
/// transition machinery always sees legal inputs.
///
/// Rows `0..=num_vertex` are placeholders/vertex rows and are never marked,
/// matching the sibling `refine_iter*` kernels.
pub fn refine_marks_from_target_levels_fortran_indexed<F: Fn(f64, f64) -> u8>(
    num_vertex: usize,
    triangle_points: &[LonLatDegrees],
    mrl_new: &[i32],
    refine_round: u8,
    target_level: F,
) -> io::Result<Vec<i32>> {
    if refine_round == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refine_round is 1-based and must be positive",
        ));
    }
    if triangle_points.len() != mrl_new.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "triangle points {} must match mrl_new length {}",
                triangle_points.len(),
                mrl_new.len()
            ),
        ));
    }
    let sjx_points = mrl_new.len().saturating_sub(1);
    if num_vertex > sjx_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("num_vertex {num_vertex} exceeds sjx_points {sjx_points}"),
        ));
    }

    let mut ref_sjx = vec![0_i32; mrl_new.len()];
    for triangle in (num_vertex + 1)..=sjx_points {
        if mrl_new[triangle] != 1 {
            continue;
        }
        let point = triangle_points[triangle];
        if target_level(point.lon_degrees, point.lat_degrees) >= refine_round {
            ref_sjx[triangle] = 1;
        }
    }
    Ok(ref_sjx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantized_level(h_base: f64, h: f64, max_level: u8) -> u8 {
        if !(h > 0.0) || h >= h_base {
            return 0;
        }
        let raw = ((h_base / h).log2() - 1e-9).ceil();
        if raw <= 0.0 {
            0
        } else if raw >= max_level as f64 {
            max_level
        } else {
            raw as u8
        }
    }

    #[test]
    fn marks_only_unrefined_triangles_at_or_above_round() {
        // Fortran layout: rows 0..=num_vertex are placeholders, triangles after.
        let num_vertex = 2usize;
        let points = vec![
            LonLatDegrees::new(0.0, 0.0),   // row 0 placeholder
            LonLatDegrees::new(0.0, 0.0),   // row 1 placeholder
            LonLatDegrees::new(0.0, 0.0),   // row 2 vertex row
            LonLatDegrees::new(10.0, 0.0),  // 3: h=20k under 100k base -> level 3
            LonLatDegrees::new(50.0, 0.0),  // 4: h=60k -> level 1
            LonLatDegrees::new(120.0, 0.0), // 5: h=150k >= base -> level 0
            LonLatDegrees::new(10.0, 0.0),  // 6: level 3 demand but already refined
        ];
        let mrl_new = vec![1, 1, 1, 1, 1, 1, 4];
        // Piecewise target sizes keyed by longitude.
        let h_base = 100_000.0;
        let field = |lon: f64, _lat: f64| {
            let h = if lon < 30.0 {
                20_000.0
            } else if lon < 100.0 {
                60_000.0
            } else {
                150_000.0
            };
            quantized_level(h_base, h, 8)
        };

        let round1 = refine_marks_from_target_levels_fortran_indexed(
            num_vertex, &points, &mrl_new, 1, field,
        )
        .expect("round 1 marks");
        assert_eq!(round1, vec![0, 0, 0, 1, 1, 0, 0]);

        let round2 = refine_marks_from_target_levels_fortran_indexed(
            num_vertex, &points, &mrl_new, 2, field,
        )
        .expect("round 2 marks");
        assert_eq!(round2, vec![0, 0, 0, 1, 0, 0, 0]);

        let round3 = refine_marks_from_target_levels_fortran_indexed(
            num_vertex, &points, &mrl_new, 3, field,
        )
        .expect("round 3 marks");
        assert_eq!(round3, vec![0, 0, 0, 1, 0, 0, 0]);

        let round4 = refine_marks_from_target_levels_fortran_indexed(
            num_vertex, &points, &mrl_new, 4, field,
        )
        .expect("round 4 marks");
        assert_eq!(round4, vec![0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn round_marks_are_nested_subsets() {
        let num_vertex = 0usize;
        let points: Vec<LonLatDegrees> = (0..64)
            .map(|i| LonLatDegrees::new(-179.0 + 5.5 * i as f64, (i % 7) as f64))
            .collect();
        let mrl_new = vec![1_i32; points.len()];
        let h_base = 200_000.0;
        let field = |lon: f64, lat: f64| {
            // Smooth synthetic target: finer toward (0, 0).
            let d = (lon * lon + lat * lat).sqrt();
            let h = 10_000.0 + 1_500.0 * d;
            quantized_level(h_base, h, 8)
        };
        let mut previous: Option<Vec<i32>> = None;
        for round in 1..=6u8 {
            let marks = refine_marks_from_target_levels_fortran_indexed(
                num_vertex, &points, &mrl_new, round, field,
            )
            .expect("marks");
            if let Some(prev) = &previous {
                for (i, (&now, &before)) in marks.iter().zip(prev.iter()).enumerate() {
                    assert!(
                        now <= before,
                        "round marks must shrink monotonically (triangle {i})"
                    );
                }
            }
            previous = Some(marks);
        }
    }

    #[test]
    fn rejects_zero_round_and_mismatched_lengths() {
        let points = vec![LonLatDegrees::new(0.0, 0.0); 4];
        let mrl_new = vec![1_i32; 4];
        assert!(
            refine_marks_from_target_levels_fortran_indexed(0, &points, &mrl_new, 0, |_, _| 1)
                .is_err()
        );
        let short = vec![1_i32; 3];
        assert!(
            refine_marks_from_target_levels_fortran_indexed(0, &points, &short, 1, |_, _| 1)
                .is_err()
        );
        assert!(
            refine_marks_from_target_levels_fortran_indexed(9, &points, &mrl_new, 1, |_, _| 1)
                .is_err()
        );
    }
}
