use super::*;

/// What perimeter does one isolated demand point produce?
///
/// Case 9 fails on a perimeter of length 40, remainder 1, arising from a single
/// demanded face once the Americas are excluded. If an isolated seed's rad3
/// footprint lands on an arbitrary remainder, then roughly two thirds of
/// isolated demand points are infeasible by construction and Case 9 is not
/// special. If it always lands on zero, the failure is position-specific and
/// needs a different explanation.
///
/// This sweeps every interior M point of a uniform base mesh, builds the
/// selection one seed would produce, and records the perimeter length modulo
/// three. Read-only: no materialization, no production path.
#[test]
fn single_seed_perimeter_remainder_distribution() {
    for nxp in [6usize, 7, 9] {
        let mesh = MethodCDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0)
            .expect("base Method-C mesh");
        let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

        let mut remainders = [0usize; 3];
        let mut lengths = Vec::new();
        let mut rejected = 0usize;

        for im in 2..=mesh.nmd {
            // Only sweep regular interior points: a pentagon or a partially
            // built ring would confound the geometry with the question.
            if m_neighbors[im].npoly != 6 {
                continue;
            }
            let Ok(footprint) = mesh.method_c_rad3_faces_with_neighbors(im, &m_neighbors) else {
                rejected += 1;
                continue;
            };
            let mut selected = vec![false; mesh.nwd + 1];
            for iw in footprint {
                if iw >= 2 && iw <= mesh.nwd {
                    selected[iw] = true;
                }
            }
            if mesh
                .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
                .is_err()
            {
                rejected += 1;
                continue;
            }
            match mesh.method_c_perimeters_from_selected_faces(&selected, &m_neighbors) {
                Ok(perimeters) => {
                    let total = perimeters.iter().map(Vec::len).sum::<usize>();
                    remainders[total % 3] += 1;
                    lengths.push(total);
                }
                Err(_) => rejected += 1,
            }
        }

        let sampled = remainders.iter().sum::<usize>();
        lengths.sort_unstable();
        let distinct = {
            let mut seen = lengths.clone();
            seen.dedup();
            seen
        };
        eprintln!(
            "nxp={nxp} sampled={sampled} rejected={rejected} \
             remainder0={} remainder1={} remainder2={} distinct_lengths={distinct:?}",
            remainders[0], remainders[1], remainders[2]
        );

        assert!(sampled > 0, "nxp={nxp} produced no measurable seeds");
    }
}

/// Does pushing the transition band inward off an irregular boundary help?
///
/// Section 36 located the failure at the coastline: the ocean mask turns a
/// regular valence-6 interior into thousands of boundary points, and rad3
/// footprints landing there produce perimeters of uncontrolled length. In the
/// regular interior the same footprint always closes at 18, which divides by
/// three. So the question is whether growing the selection inward moves the
/// perimeter into regular territory and recovers a decomposable length.
///
/// This builds a seed's footprint, then repeatedly unions in the footprints of
/// every M point already touched, and reports the perimeter remainder at each
/// radius. Read-only.
#[test]
fn inward_growth_perimeter_remainders() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(9, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    // Seeds whose own footprint is already undecomposable are the interesting
    // ones: they are this mesh's stand-in for a coastal demand point.
    let mut bad_seeds = Vec::new();
    for im in 2..=mesh.nmd {
        if m_neighbors[im].npoly != 6 {
            continue;
        }
        let Ok(footprint) = mesh.method_c_rad3_faces_with_neighbors(im, &m_neighbors) else {
            continue;
        };
        let mut selected = vec![false; mesh.nwd + 1];
        for iw in footprint {
            if iw >= 2 && iw <= mesh.nwd {
                selected[iw] = true;
            }
        }
        if mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
            .is_err()
        {
            continue;
        }
        if let Ok(perimeters) = mesh.method_c_perimeters_from_selected_faces(&selected, &m_neighbors)
        {
            let total = perimeters.iter().map(Vec::len).sum::<usize>();
            if total % 3 != 0 {
                bad_seeds.push((im, total));
            }
        }
    }
    eprintln!("undecomposable seeds at nxp=9: {}", bad_seeds.len());

    for &(im, base_len) in bad_seeds.iter().take(6) {
        let mut selected = vec![false; mesh.nwd + 1];
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(im, &m_neighbors)
            .expect("footprint")
        {
            if iw >= 2 && iw <= mesh.nwd {
                selected[iw] = true;
            }
        }
        let mut trail = vec![format!("{base_len}({})", base_len % 3)];
        for _ in 0..4 {
            let touched = (2..=mesh.nmd)
                .filter(|&candidate| {
                    let neighbors = m_neighbors[candidate];
                    neighbors
                        .iw
                        .iter()
                        .take(neighbors.npoly)
                        .any(|&iw| selected.get(iw).copied().unwrap_or(false))
                })
                .collect::<Vec<_>>();
            for candidate in touched {
                if let Ok(footprint) =
                    mesh.method_c_rad3_faces_with_neighbors(candidate, &m_neighbors)
                {
                    for iw in footprint {
                        if iw >= 2 && iw <= mesh.nwd {
                            selected[iw] = true;
                        }
                    }
                }
            }
            if mesh
                .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
                .is_err()
            {
                trail.push("closure-err".to_string());
                break;
            }
            match mesh.method_c_perimeters_from_selected_faces(&selected, &m_neighbors) {
                Ok(perimeters) => {
                    let total = perimeters.iter().map(Vec::len).sum::<usize>();
                    trail.push(format!("{total}({})", total % 3));
                }
                Err(_) => {
                    trail.push("perim-err".to_string());
                    break;
                }
            }
        }
        eprintln!("  seed {im:6}: {}", trail.join(" -> "));
    }
}
