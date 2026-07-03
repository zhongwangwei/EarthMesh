//! M1 <-> M3 bridge: a real gradient-limited `HField` drives the real
//! grid_preprocess marking kernel, and the resulting per-round mark sets are
//! nested contiguous rings (the property the discrete transition machinery
//! relies on).

use earthmesh_hfield::{HField, HRegion};
use earthmesh_mesh::{refine_marks_from_target_levels_fortran_indexed, LonLatDegrees};

#[test]
fn hfield_levels_drive_nested_contiguous_refinement_marks() {
    let h_base = 200_000.0;
    let mut field = HField::uniform(360, 180, h_base).expect("uniform field");
    field
        .min_with_region(
            &HRegion::Circle {
                lon: 115.0,
                lat: 25.0,
                radius_m: 400_000.0,
            },
            20_000.0,
        )
        .expect("circle region");
    field.limit_gradient(0.2).expect("gradient limit");

    // Fortran-indexed synthetic transect through the region center: rows
    // 0..=num_vertex are placeholders, then one triangle center per degree of
    // latitude from 30 degrees south of the center to 29 degrees north.
    let num_vertex = 1usize;
    let mut points = vec![LonLatDegrees::new(0.0, 0.0); num_vertex + 1];
    for k in 0..60 {
        points.push(LonLatDegrees::new(115.0, 25.0 - 30.0 + k as f64));
    }
    let mrl_new = vec![1_i32; points.len()];
    let center_row = num_vertex + 1 + 30; // lat exactly 25.0

    let mut per_round: Vec<Vec<i32>> = Vec::new();
    for round in 1..=5u8 {
        let marks = refine_marks_from_target_levels_fortran_indexed(
            num_vertex,
            &points,
            &mrl_new,
            round,
            |lon, lat| field.level_at(lon, lat, h_base, 8),
        )
        .expect("marks");
        per_round.push(marks);
    }

    // Center wants ceil(log2(200k / 20k)) = 4 halvings: marked through round 4,
    // unmarked at round 5.
    assert_eq!(per_round[0][center_row], 1, "round 1 marks the center");
    assert_eq!(
        per_round[3][center_row], 1,
        "round 4 still marks the center"
    );
    assert_eq!(
        per_round[4][center_row], 0,
        "round 5 exceeds the field's demand"
    );
    // Far tail of the transect is outside every ring.
    assert_eq!(
        per_round[0][num_vertex + 1],
        0,
        "transect tail stays unmarked"
    );

    for (r, marks) in per_round.iter().enumerate() {
        // Placeholder rows never marked.
        for row in 0..=num_vertex {
            assert_eq!(marks[row], 0, "placeholder row {row} in round {}", r + 1);
        }
        // Nested subsets: each round marks a subset of the previous round.
        if r > 0 {
            for (i, (&now, &before)) in marks.iter().zip(per_round[r - 1].iter()).enumerate() {
                assert!(now <= before, "round {} grew at row {i}", r + 1);
            }
        }
        // Along the transect each round's marks form one contiguous run
        // (gradient-limited cone => interval), except rounds that mark nothing.
        let marked: Vec<usize> = (0..marks.len()).filter(|&i| marks[i] == 1).collect();
        if let (Some(&first), Some(&last)) = (marked.first(), marked.last()) {
            for i in first..=last {
                assert_eq!(marks[i], 1, "gap inside round {} ring at row {i}", r + 1);
            }
        }
    }

    // Round-1 ring must extend beyond the raw circle (the limiter grows a
    // slope-g skirt): the circle spans ~3.6 degrees of latitude radius, while
    // level >= 1 persists until h grows back to h_base, i.e. ~900 km further.
    let ring1: Vec<usize> = (0..per_round[0].len())
        .filter(|&i| per_round[0][i] == 1)
        .collect();
    let ring1_rows = ring1.len();
    assert!(
        (18..=28).contains(&ring1_rows),
        "round-1 ring spans {ring1_rows} rows; expected ~23 (2 * (400 + 900) km / 111 km)"
    );
}
