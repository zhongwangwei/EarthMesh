use super::*;

/// Faces of `mesh` whose centres fall within any of the caps
/// `(lon, lat, radius degrees)`, closed under the concavity fill.
fn selection(mesh: &MethodCMesh, caps: &[(f64, f64, f64)]) -> Vec<bool> {
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let unit = |lon: f64, lat: f64| {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let caps = caps
        .iter()
        .map(|&(lon, lat, r)| (unit(lon, lat), r.to_radians().cos()))
        .collect::<Vec<_>>();
    let mut selected = vec![false; mesh.nwd + 1];
    for iw in 2..=mesh.nwd {
        let c = mesh.w_faces[iw]
            .im
            .iter()
            .map(|&im| mesh.m_points[im])
            .fold([0.0; 3], |a, p| [a[0] + p.x, a[1] + p.y, a[2] + p.z]);
        let n = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
        let c = [c[0] / n, c[1] / n, c[2] / n];
        selected[iw] = caps
            .iter()
            .any(|(u, cos_r)| c[0] * u[0] + c[1] * u[1] + c[2] * u[2] >= *cos_r);
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut selected, &neighbors)
        .expect("closure");
    selected
}

fn grow_both_ways(
    mesh: &MethodCMesh,
    selected: &[bool],
) -> (
    Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>,
    Option<(Vec<bool>, Vec<MethodCPerimeterPoint>)>,
) {
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let perimeters = mesh
        .method_c_perimeters_from_selected_faces(selected, &neighbors)
        .ok()
        .map(|perimeters| {
            perimeters
                .into_iter()
                .filter(|perimeter| perimeter.len() % 3 != 0)
                .flatten()
                .collect::<Vec<_>>()
        });
    let grow = || {
        mesh.try_grow_method_c_non_triplet_perimeter_once(
            selected,
            &neighbors,
            2,
            perimeters.as_deref(),
        )
        .expect("grow")
    };
    let incremental = grow();
    WHOLE_MESH_ONLY.with(|cell| cell.set(true));
    let whole = grow();
    WHOLE_MESH_ONLY.with(|cell| cell.set(false));
    (incremental, whole)
}

#[test]
fn incremental_scoring_picks_what_the_whole_mesh_picks() {
    let mesh = MethodCMesh::from_icosahedron(24, 0, 1.0, 0.25).expect("base mesh");
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut compared = 0;
    let mut grown = 0;
    for _ in 0..40 {
        // A few small, ragged caps near one another: blocks, concave corners,
        // non-triplet perimeters and the odd pinch.
        let (lon0, lat0) = (next() * 360.0 - 180.0, next() * 120.0 - 60.0);
        let caps = (0..1 + (next() * 4.0) as usize)
            .map(|_| {
                (
                    lon0 + next() * 16.0 - 8.0,
                    lat0 + next() * 16.0 - 8.0,
                    2.0 + next() * 6.0,
                )
            })
            .collect::<Vec<_>>();
        let selected = selection(&mesh, &caps);
        if !selected.iter().any(|&s| s) {
            continue;
        }
        let (incremental, whole) = grow_both_ways(&mesh, &selected);
        assert_eq!(
            incremental.as_ref().map(|(trial, _)| trial),
            whole.as_ref().map(|(trial, _)| trial),
            "caps {caps:?}"
        );
        assert_eq!(
            incremental
                .as_ref()
                .map(|(_, p)| p.iter().map(|q| q.im).collect::<Vec<_>>()),
            whole
                .as_ref()
                .map(|(_, p)| p.iter().map(|q| q.im).collect::<Vec<_>>()),
            "caps {caps:?}"
        );
        compared += 1;
        grown += usize::from(whole.is_some());
    }
    assert!(
        compared >= 30 && grown >= 10,
        "{compared} compared, {grown} grown"
    );
}

#[test]
fn the_candidates_of_a_ragged_selection_are_scored() {
    let mesh = MethodCMesh::from_icosahedron(24, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = mesh.method_c_m_neighbors().unwrap();
    let caps = [
        (82.9, -1.7, 4.0),
        (81.6, -0.7, 3.1),
        (88.2, -11.6, 5.2),
        (81.3, -4.0, 7.6),
    ];
    let selected = selection(&mesh, &caps);
    let parent = mesh.w_faces[(2..=mesh.nwd).find(|&iw| selected[iw]).unwrap()].mrlw;
    let mut incremental = IncrementalSelection::new(&mesh, &selected, &neighbors)
        .unwrap()
        .expect("a closed, walkable base");
    let mut scored = 0;
    for im in 2..=mesh.nmd {
        let nb = neighbors[im];
        let around = nb.iw[..nb.npoly].iter().filter(|&&iw| selected[iw]).count();
        if around == 0 || around == nb.npoly {
            continue;
        }
        for &iw in &nb.iw[..nb.npoly] {
            if selected[iw] || mesh.w_faces[iw].mrlw != parent {
                continue;
            }
            if let Trial::Scored(_) = incremental.trial(None, &[iw]).unwrap() {
                scored += 1;
            }
        }
    }
    assert!(scored > 40, "{scored} scored");
}

#[test]
fn incremental_boundary_fill_picks_what_the_whole_mesh_picks() {
    let mesh = MethodCMesh::from_icosahedron(24, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut filled = 0;
    for _ in 0..30 {
        let (lon0, lat0) = (next() * 360.0 - 180.0, next() * 120.0 - 60.0);
        let caps = (0..1 + (next() * 4.0) as usize)
            .map(|_| {
                (
                    lon0 + next() * 16.0 - 8.0,
                    lat0 + next() * 16.0 - 8.0,
                    2.0 + next() * 6.0,
                )
            })
            .collect::<Vec<_>>();
        let selected = selection(&mesh, &caps);
        let Ok(perimeters) = mesh.method_c_perimeters_from_selected_faces(&selected, &neighbors)
        else {
            continue;
        };
        let perimeter = perimeters.concat();
        // With a focus near the start of the perimeter and without one.
        for focus in [None, perimeter.first().map(|point| point.im)] {
            let fill = || {
                mesh.try_fill_method_c_perimeter_boundary(
                    &selected,
                    &neighbors,
                    2,
                    Some(&perimeter),
                    focus,
                )
                .expect("boundary fill")
            };
            let incremental = fill();
            WHOLE_MESH_ONLY.with(|cell| cell.set(true));
            let whole = fill();
            WHOLE_MESH_ONLY.with(|cell| cell.set(false));
            assert_eq!(
                incremental.as_ref().map(|(trial, _)| trial),
                whole.as_ref().map(|(trial, _)| trial),
                "caps {caps:?} focus {focus:?}"
            );
            filled += usize::from(whole.is_some());
        }
    }
    assert!(filled >= 10, "{filled} fills compared");
}

#[test]
fn incremental_shrink_picks_what_the_whole_mesh_picks() {
    let mesh = MethodCMesh::from_icosahedron(24, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let mut state = 0x1d8e_4e27_c47d_124f_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut shrunk = 0;
    for _ in 0..30 {
        let (lon0, lat0) = (next() * 360.0 - 180.0, next() * 120.0 - 60.0);
        let caps = (0..1 + (next() * 4.0) as usize)
            .map(|_| {
                (
                    lon0 + next() * 16.0 - 8.0,
                    lat0 + next() * 16.0 - 8.0,
                    2.0 + next() * 6.0,
                )
            })
            .collect::<Vec<_>>();
        let selected = selection(&mesh, &caps);
        let Ok(perimeters) = mesh.method_c_perimeters_from_selected_faces(&selected, &neighbors)
        else {
            continue;
        };
        let perimeter = perimeters.concat();
        let shrink = || {
            mesh.try_shrink_method_c_perimeter_once(&selected, &neighbors, 2, Some(&perimeter))
                .expect("shrink")
        };
        let incremental = shrink();
        WHOLE_MESH_ONLY.with(|cell| cell.set(true));
        let whole = shrink();
        WHOLE_MESH_ONLY.with(|cell| cell.set(false));
        assert_eq!(
            incremental.as_ref().map(|(trial, _)| trial),
            whole.as_ref().map(|(trial, _)| trial),
            "caps {caps:?}"
        );
        shrunk += usize::from(whole.is_some());
    }
    assert!(shrunk >= 10, "{shrunk} shrinks compared");
}

#[test]
fn many_blocks_short_of_a_triple_are_repaired_together() {
    let mesh = MethodCMesh::from_icosahedron(48, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    // A scatter of small separate caps: many blocks, most short of a triple.
    let caps = (0..40)
        .map(|k| {
            let k = k as f64;
            (
                -170.0 + k * 8.5,
                -50.0 + (k * 37.0) % 100.0,
                1.5 + (k * 0.7) % 2.5,
            )
        })
        .collect::<Vec<_>>();
    let mut selected = selection(&mesh, &caps);
    let before = mesh
        .method_c_perimeters_from_selected_faces(&selected, &neighbors)
        .expect("separate caps walk");
    let short = before.iter().filter(|p| !p.len().is_multiple_of(3)).count();
    assert!(
        short > super::super::method_c_perimeter_repair::BATCH_ABOVE_BLOCKS,
        "{short} short"
    );
    let perimeter = mesh
        .repair_method_c_non_triplet_perimeter(&mut selected, &neighbors, 2)
        .expect("every block made a triple");
    assert!(perimeter.len().is_multiple_of(3));
    let after = mesh
        .method_c_perimeters_from_selected_faces(&selected, &neighbors)
        .expect("repaired selection walks");
    assert!(after.iter().all(|p| p.len().is_multiple_of(3)));
}

#[test]
fn a_pinched_base_is_grown_as_the_whole_mesh_grows_it() {
    // A base whose perimeters do not walk: the grower is asked to mend a
    // pinch, with no perimeter to search along.
    let mesh = MethodCMesh::from_icosahedron(24, 0, 1.0, 0.25).expect("base mesh");
    let neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    let mut compared = 0;
    for im in (2..=mesh.nmd)
        .filter(|&im| neighbors[im].npoly == 6)
        .step_by(97)
        .take(12)
    {
        let mut selected = vec![false; mesh.nwd + 1];
        for slot in [0, 1, 3, 4] {
            selected[neighbors[im].iw[slot]] = true;
        }
        if mesh
            .method_c_perimeters_from_selected_faces(&selected, &neighbors)
            .is_ok()
        {
            continue;
        }
        let mut closed = selected.clone();
        mesh.close_method_c_concavities_for_level_with_neighbors(&mut closed, &neighbors)
            .unwrap();
        if closed != selected {
            continue;
        }
        let (incremental, whole) = grow_both_ways(&mesh, &selected);
        assert_eq!(
            incremental.as_ref().map(|(trial, _)| trial),
            whole.as_ref().map(|(trial, _)| trial),
            "pinch at {im}"
        );
        compared += 1;
    }
    assert!(compared >= 3, "{compared} pinched bases compared");
}
