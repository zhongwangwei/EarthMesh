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

/// Do several demand points still reach a decomposable perimeter together?
///
/// The single-seed sweep shows a bad seed recovers a remainder of zero after
/// two rings of growth. Case 9 has eight demanded faces in pass 2, so the
/// question is whether growing them at once still converges or whether their
/// perimeters merge into one component whose combined length lands elsewhere.
/// Read-only.
#[test]
fn multi_seed_adaptive_growth_remainders() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(9, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    let footprint_of = |im: usize| -> Option<Vec<usize>> {
        mesh.method_c_rad3_faces_with_neighbors(im, &m_neighbors).ok()
    };
    let perimeter_len = |selected: &[bool]| -> Option<usize> {
        mesh.method_c_perimeters_from_selected_faces(selected, &m_neighbors)
            .ok()
            .map(|perimeters| perimeters.iter().map(Vec::len).sum())
    };

    let mut bad = Vec::new();
    for im in 2..=mesh.nmd {
        if m_neighbors[im].npoly != 6 {
            continue;
        }
        let Some(footprint) = footprint_of(im) else {
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
        if perimeter_len(&selected).is_some_and(|total| total % 3 != 0) {
            bad.push(im);
        }
    }
    eprintln!("bad seeds available: {}", bad.len());

    // Adjacent seeds share footprints and merge immediately; spread ones stay
    // separate for longer. Test both so interference is visible either way.
    for (label, seeds) in [
        ("2 adjacent", bad.iter().copied().take(2).collect::<Vec<_>>()),
        ("4 adjacent", bad.iter().copied().take(4).collect::<Vec<_>>()),
        (
            "4 spread",
            bad.iter().copied().step_by(bad.len() / 4).take(4).collect(),
        ),
        (
            "8 spread",
            bad.iter().copied().step_by(bad.len() / 8).take(8).collect(),
        ),
    ] {
        let mut selected = vec![false; mesh.nwd + 1];
        for &im in &seeds {
            for iw in footprint_of(im).unwrap_or_default() {
                if iw >= 2 && iw <= mesh.nwd {
                    selected[iw] = true;
                }
            }
        }
        if mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
            .is_err()
        {
            eprintln!("  {label}: closure failed at ring 0");
            continue;
        }
        let mut trail = Vec::new();
        let mut converged = None;
        for ring in 0..5 {
            match perimeter_len(&selected) {
                Some(total) => {
                    trail.push(format!("{total}({})", total % 3));
                    if total % 3 == 0 && converged.is_none() {
                        converged = Some(ring);
                    }
                }
                None => {
                    trail.push("perim-err".to_string());
                    break;
                }
            }
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
                for iw in footprint_of(candidate).unwrap_or_default() {
                    if iw >= 2 && iw <= mesh.nwd {
                        selected[iw] = true;
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
        }
        let faces = selected.iter().filter(|&&item| item).count();
        eprintln!(
            "  {label:12}: {}  first_zero_ring={converged:?} final_faces={faces}",
            trail.join(" -> ")
        );
    }
}

/// Case 9's pass-1 demand faces, by centre. Exported from a production run via
/// `EARTHMESH_M0_FACE_DEMAND_DUMP_DIR` at NXP=243, `max_level=3`.
const CASE9_PASS1_DEMAND: &[(f64, f64)] = &[
    (29.8509, -29.1702),
    (30.1664, -23.7481),
    (138.6338, -35.1379),
    (-69.4550, -55.2375),
    (-69.4800, -54.9108),
    (-70.2543, -54.5105),
    (-70.4945, -54.7617),
    (-70.5066, -54.4312),
    (-70.2473, -54.6757),
    (-72.9556, -52.9512),
    (-72.9506, -52.7865),
    (-72.9658, -53.2806),
    (-73.1448, -51.5371),
    (-73.3724, -51.4520),
    (-73.6136, -51.6997),
    (-74.6108, -49.4285),
    (-73.6335, -46.7445),
    (-72.8166, -46.9212),
    (-72.8015, -45.9057),
    (-73.0033, -45.9888),
    (-72.6077, -46.4993),
    (-72.8089, -46.4134),
    (-70.8819, -34.0108),
    (-78.3656, -1.9707),
    (-77.5149, 0.5410),
    (-77.3673, 0.6149),
    (-77.5123, 0.3836),
    (-77.2223, 0.8463),
    (-77.5470, 2.4304),
    (36.5040, 37.3554),
    (21.0675, 39.1241),
    (21.4540, 38.3844),
    (24.5516, 41.0381),
    (20.1120, 39.8819),
    (15.9361, 39.7373),
    (10.3487, 43.4874),
    (10.6027, 46.6252),
    (104.6374, 17.3998),
    (-148.8617, 61.7742),
    (-98.6324, 18.8998),
];

/// Case 9's pass-2 demand faces, from the same run. These are the eight that
/// only appear once pass 1 has already refined; on the pass-1 product, fifteen
/// of their twenty-four candidate seeds close at 16 or 17 rather than 18.
const CASE9_PASS2_DEMAND: &[(f64, f64)] = &[
    (-69.4570, -55.2380),
    (-69.3576, -54.8680),
    (-73.4727, -51.6386),
    (-74.6162, -49.5123),
    (-73.7375, -46.7854),
    (-77.4385, 0.4988),
    (-77.1485, 0.8832),
    (21.4660, 38.4212),
];

/// Would deferring every transition band to one final materialization work?
///
/// Measured so far: on the base mesh all 120 of pass 1's candidate seeds close
/// at perimeter 18, remainder 0. After pass 1 materializes, 15 of pass 2's 24
/// candidates close at 16 or 17, and one demand face has no decomposable seed at
/// all. The 126 valence-7 points that make the pass-1 product irregular all lie
/// within 200 km of a pass-1 demand point, so pass 1's own transition band is
/// what pass 2 then trips over.
///
/// That suggests reordering: run selection for every level against the untouched
/// base, and materialize all bands at the end. Under that scheme both levels'
/// perimeters are curves on regular topology. This measures those curves —
/// per-seed footprints, and the two accumulated regions they union into — to see
/// whether they are triplet-decomposable. Read-only: nothing is materialized and
/// no production path is touched.
#[test]
fn case9_demand_projected_onto_regular_base() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
        .expect("NXP=243 base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
    eprintln!("base mesh: nmd={} nwd={}", mesh.nmd, mesh.nwd);

    // Nearest M point to a demand centre. Every m_point sits on the same sphere,
    // so maximising the dot product with the query direction is enough.
    let nearest_seed = |lon_deg: f64, lat_deg: f64| -> usize {
        let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
        let query = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let mut best = (f64::NEG_INFINITY, 0usize);
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let dot = point.x * query[0] + point.y * query[1] + point.z * query[2];
            if dot > best.0 {
                best = (dot, im);
            }
        }
        best.1
    };

    let mut regions: Vec<(&str, Vec<bool>)> = Vec::new();
    for (label, demand) in [("pass1", CASE9_PASS1_DEMAND), ("pass2", CASE9_PASS2_DEMAND)] {
        let mut histogram = std::collections::BTreeMap::new();
        let mut undecomposable = Vec::new();
        let mut region = vec![false; mesh.nwd + 1];

        for &(lon, lat) in demand {
            let seed = nearest_seed(lon, lat);
            match mesh.seed_footprint_perimeter_length(seed) {
                Ok(Some(total)) => {
                    *histogram.entry((total, total % 3)).or_insert(0usize) += 1;
                    if total % 3 != 0 {
                        undecomposable.push((lon, lat, total));
                    }
                }
                _ => undecomposable.push((lon, lat, 0)),
            }
            if let Ok(footprint) = mesh.method_c_rad3_faces_with_neighbors(seed, &m_neighbors) {
                for iw in footprint {
                    if iw >= 2 && iw <= mesh.nwd {
                        region[iw] = true;
                    }
                }
            }
        }

        eprintln!(
            "\n{label}: {} demand faces -> per-seed footprint perimeters {histogram:?}",
            demand.len()
        );
        if undecomposable.is_empty() {
            eprintln!("  every seed decomposable on the regular base");
        } else {
            for (lon, lat, total) in &undecomposable {
                eprintln!("  undecomposable seed at lon {lon:8.3} lat {lat:8.3} perimeter {total}");
            }
        }
        regions.push((label, region));
    }

    // The accumulated regions are what a deferred materialization would actually
    // have to build a band around, so their perimeters are the operative test.
    let mut closed_regions = Vec::new();
    for (label, mut region) in regions {
        let faces_before = region.iter().filter(|&&item| item).count();
        if mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
            .is_err()
        {
            eprintln!("\n{label} region: concavity closure failed");
            continue;
        }
        let faces = region.iter().filter(|&&item| item).count();
        match mesh.method_c_perimeters_from_selected_faces(&region, &m_neighbors) {
            Ok(perimeters) => {
                let lengths = perimeters
                    .iter()
                    .map(|perimeter| (perimeter.len(), perimeter.len() % 3))
                    .collect::<Vec<_>>();
                let bad = lengths.iter().filter(|&&(_, rem)| rem != 0).count();
                eprintln!(
                    "\n{label} region: {faces_before} -> {faces} faces, \
                     {} components, undecomposable {bad}",
                    perimeters.len()
                );
                eprintln!("  (length, remainder) = {lengths:?}");
            }
            Err(error) => eprintln!("\n{label} region: perimeter extraction failed: {error}"),
        }
        closed_regions.push((label, region));
    }

    // A deferred scheme also needs the finer region nested inside the coarser
    // one, or the levels cannot be materialized together.
    if let ([(_, coarse)], [(_, fine)]) = (&closed_regions[..1], &closed_regions[1..]) {
        let outside = (2..=mesh.nwd)
            .filter(|&iw| fine[iw] && !coarse[iw])
            .count();
        eprintln!(
            "\nnesting: {} pass-2 faces fall outside the pass-1 region",
            outside
        );
    }
}

/// Is a merged footprint ever triplet-decomposable?
///
/// Projecting Case 9's demand onto the untouched base showed every isolated seed
/// closing at 18, remainder 0, while all seven components formed by two or more
/// overlapping footprints closed at 22, 23, 26, 29, 32 or 37 — not one a
/// multiple of three. Two of those lengths, 22 and 40, are the exact perimeters
/// production fails on. If merging systematically lands off a multiple of three
/// then no reordering of the passes helps, because the base mesh is already as
/// regular as it can get.
///
/// This takes one regular seed and sweeps a second across the mesh, recording
/// the perimeter of whatever the pair produces. Read-only.
#[test]
fn merged_footprint_perimeter_remainders() {
    let mesh =
        MethodCDelaunayMesh::from_icosahedron(12, 0, 1.0, 0.25, 0).expect("base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    let regular = (2..=mesh.nmd)
        .filter(|&im| m_neighbors[im].npoly == 6)
        .collect::<Vec<_>>();
    // The anchor must itself close at 18, or every pairing inherits its defect
    // and the merged reading is confounded.
    let anchor = regular
        .iter()
        .copied()
        .find(|&im| {
            mesh.seed_footprint_perimeter_length(im)
                .ok()
                .flatten()
                .is_some_and(|total| total == 18)
        })
        .expect("a seed closing at 18");
    let anchor_footprint = mesh
        .method_c_rad3_faces_with_neighbors(anchor, &m_neighbors)
        .expect("anchor footprint");

    let mut by_components: std::collections::BTreeMap<usize, std::collections::BTreeMap<usize, usize>> =
        std::collections::BTreeMap::new();
    let mut merged_lengths = std::collections::BTreeMap::new();
    let mut merged_total = 0usize;
    let mut merged_decomposable = 0usize;

    for &other in &regular {
        if other == anchor {
            continue;
        }
        let Ok(other_footprint) = mesh.method_c_rad3_faces_with_neighbors(other, &m_neighbors)
        else {
            continue;
        };
        let mut selected = vec![false; mesh.nwd + 1];
        for iw in anchor_footprint.iter().chain(other_footprint.iter()) {
            if *iw >= 2 && *iw <= mesh.nwd {
                selected[*iw] = true;
            }
        }
        if mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut selected, &m_neighbors)
            .is_err()
        {
            continue;
        }
        let Ok(perimeters) = mesh.method_c_perimeters_from_selected_faces(&selected, &m_neighbors)
        else {
            continue;
        };
        let bad = perimeters.iter().filter(|p| p.len() % 3 != 0).count();
        *by_components
            .entry(perimeters.len())
            .or_default()
            .entry(bad)
            .or_insert(0) += 1;

        // A single component means the two footprints actually fused; that is
        // the case the region-level measurement flagged.
        if perimeters.len() == 1 {
            let length = perimeters[0].len();
            *merged_lengths.entry((length, length % 3)).or_insert(0usize) += 1;
            merged_total += 1;
            if length % 3 == 0 {
                merged_decomposable += 1;
            }
        }
    }

    eprintln!("anchor={anchor} regular seeds swept={}", regular.len() - 1);
    for (components, bad_counts) in &by_components {
        eprintln!("  {components} component(s): undecomposable-count histogram {bad_counts:?}");
    }
    eprintln!(
        "\nfused into one component: {merged_total}, of which decomposable {merged_decomposable}"
    );
    eprintln!("  (length, remainder) = {merged_lengths:?}");
}

/// Can a locally grown region reach a decomposable perimeter, and at what cost?
///
/// Merged footprints land on a multiple of three about half the time, and each
/// added ring changes the length, so a component that misses should have a
/// decomposable neighbour a short walk away. The concern is the fixed-buffer
/// experiment, where enlarging a region created fresh violations elsewhere: the
/// question is not whether a bad component can be fixed but whether fixing it
/// leaves the rest of the region intact.
///
/// This takes Case 9's pass-2 demand on the regular base, grows only the
/// components whose perimeter is undecomposable, and re-measures every
/// component after each ring. Read-only.
#[test]
fn case9_bad_component_local_growth() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
        .expect("NXP=243 base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    // Faces meeting at an M point are the growth stencil, matching the
    // adjacency the component walk and the perimeter builder both use.
    let mut faces_at_m = vec![Vec::new(); mesh.nmd + 1];
    for iw in 2..=mesh.nwd {
        for &im in &mesh.w_faces[iw].im {
            if im >= 2 && im <= mesh.nmd {
                faces_at_m[im].push(iw);
            }
        }
    }

    let nearest_seed = |lon_deg: f64, lat_deg: f64| -> usize {
        let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
        let query = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let mut best = (f64::NEG_INFINITY, 0usize);
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let dot = point.x * query[0] + point.y * query[1] + point.z * query[2];
            if dot > best.0 {
                best = (dot, im);
            }
        }
        best.1
    };

    let mut region = vec![false; mesh.nwd + 1];
    for &(lon, lat) in CASE9_PASS2_DEMAND {
        let seed = nearest_seed(lon, lat);
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(seed, &m_neighbors)
            .expect("footprint")
        {
            if iw >= 2 && iw <= mesh.nwd {
                region[iw] = true;
            }
        }
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
        .expect("initial closure");

    for step in 0..8 {
        let components = mesh
            .method_c_selected_face_components(&region, &m_neighbors)
            .expect("components");
        let mut lengths = Vec::new();
        let mut bad_faces = Vec::new();
        for component in &components {
            let mut mask = vec![false; mesh.nwd + 1];
            for &iw in component {
                mask[iw] = true;
            }
            let Ok(perimeters) = mesh.method_c_perimeters_from_selected_faces(&mask, &m_neighbors)
            else {
                lengths.push((0, 9));
                bad_faces.extend(component.iter().copied());
                continue;
            };
            let total = perimeters.iter().map(Vec::len).sum::<usize>();
            lengths.push((total, total % 3));
            if total % 3 != 0 {
                bad_faces.extend(component.iter().copied());
            }
        }
        let bad = lengths.iter().filter(|&&(_, rem)| rem != 0).count();
        let faces = region.iter().filter(|&&item| item).count();
        eprintln!(
            "step {step}: {faces} faces, {} components, undecomposable {bad}",
            components.len()
        );
        eprintln!("  (length, remainder) = {lengths:?}");
        if bad == 0 {
            eprintln!("  region fully decomposable after {step} growth ring(s)");
            return;
        }

        for iw in bad_faces {
            for &im in &mesh.w_faces[iw].im {
                if im >= 2 && im <= mesh.nmd {
                    for &neighbor in &faces_at_m[im] {
                        region[neighbor] = true;
                    }
                }
            }
        }
        if mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
            .is_err()
        {
            eprintln!("  closure failed after growth ring {step}");
            return;
        }
    }
    eprintln!("still undecomposable after 8 growth rings");
}

/// Does an asymmetric edit move the perimeter off its residue class?
///
/// Isotropic growth adds exactly six to a component's perimeter per ring, so it
/// preserves length modulo three: Case 9's length-22 component walks 22, 28, 34,
/// 40 and stays at remainder one forever. That rules out buffering as a repair
/// and points at asymmetric edits instead, which is also why the earlier
/// footprint-union growth did recover — it was never a uniform ring.
///
/// This adds a single face at a time to a bad component, re-closes concavities,
/// and records which perimeter lengths are reachable. Read-only.
#[test]
fn case9_bad_component_single_face_edits() {
    let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
        .expect("NXP=243 base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    let mut faces_at_m = vec![Vec::new(); mesh.nmd + 1];
    for iw in 2..=mesh.nwd {
        for &im in &mesh.w_faces[iw].im {
            if im >= 2 && im <= mesh.nmd {
                faces_at_m[im].push(iw);
            }
        }
    }
    let nearest_seed = |lon_deg: f64, lat_deg: f64| -> usize {
        let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
        let query = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let mut best = (f64::NEG_INFINITY, 0usize);
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let dot = point.x * query[0] + point.y * query[1] + point.z * query[2];
            if dot > best.0 {
                best = (dot, im);
            }
        }
        best.1
    };

    let mut region = vec![false; mesh.nwd + 1];
    for &(lon, lat) in CASE9_PASS2_DEMAND {
        let seed = nearest_seed(lon, lat);
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(seed, &m_neighbors)
            .expect("footprint")
        {
            if iw >= 2 && iw <= mesh.nwd {
                region[iw] = true;
            }
        }
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
        .expect("initial closure");

    let components = mesh
        .method_c_selected_face_components(&region, &m_neighbors)
        .expect("components");
    let perimeter_of = |mask: &[bool]| -> Option<usize> {
        mesh.method_c_perimeters_from_selected_faces(mask, &m_neighbors)
            .ok()
            .map(|perimeters| perimeters.iter().map(Vec::len).sum())
    };

    for component in &components {
        let mut base_mask = vec![false; mesh.nwd + 1];
        for &iw in component {
            base_mask[iw] = true;
        }
        let Some(base_len) = perimeter_of(&base_mask) else {
            continue;
        };
        if base_len % 3 == 0 {
            continue;
        }

        // Every face touching the component from outside is a candidate edit.
        let mut candidates = std::collections::BTreeSet::new();
        for &iw in component {
            for &im in &mesh.w_faces[iw].im {
                if im >= 2 && im <= mesh.nmd {
                    for &neighbor in &faces_at_m[im] {
                        if !base_mask[neighbor] {
                            candidates.insert(neighbor);
                        }
                    }
                }
            }
        }

        let mut reachable = std::collections::BTreeMap::new();
        let mut fixes = Vec::new();
        for &candidate in &candidates {
            let mut mask = base_mask.clone();
            mask[candidate] = true;
            if mesh
                .close_method_c_concavities_for_level_with_neighbors(&mut mask, &m_neighbors)
                .is_err()
            {
                continue;
            }
            let Some(length) = perimeter_of(&mask) else {
                continue;
            };
            *reachable.entry((length, length % 3)).or_insert(0usize) += 1;
            if length % 3 == 0 {
                fixes.push((candidate, length, mask.iter().filter(|&&x| x).count()));
            }
        }

        eprintln!(
            "component of {} faces, perimeter {base_len} (remainder {}), {} single-face candidates",
            component.len(),
            base_len % 3,
            candidates.len()
        );
        eprintln!("  reachable (length, remainder) = {reachable:?}");
        if fixes.is_empty() {
            eprintln!("  no single-face edit reaches a multiple of three");
        } else {
            eprintln!("  {} single-face edits repair it, e.g.", fixes.len());
            for (candidate, length, faces) in fixes.iter().take(3) {
                eprintln!("    add face {candidate} -> perimeter {length}, {faces} faces");
            }
        }
    }
}

/// How many faces must be added before a bad component becomes decomposable?
///
/// Uniform growth preserves the residue and a single added face only reaches
/// remainder one or two, so if the region is repairable at all it takes a small
/// asymmetric patch. The size of that patch is what decides whether repair is a
/// local edit or a redesign: a couple of faces is a search a selector can run,
/// while a large patch would drag in the surrounding geometry.
///
/// This breadth-first searches face additions from Case 9's bad components,
/// capped in width, and reports the first depth reaching a multiple of three.
/// Read-only.
#[test]
fn case9_bad_component_minimal_repair_depth() {
    const WIDTH: usize = 24;
    const MAX_DEPTH: usize = 4;

    let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
        .expect("NXP=243 base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    let mut faces_at_m = vec![Vec::new(); mesh.nmd + 1];
    for iw in 2..=mesh.nwd {
        for &im in &mesh.w_faces[iw].im {
            if im >= 2 && im <= mesh.nmd {
                faces_at_m[im].push(iw);
            }
        }
    }
    let nearest_seed = |lon_deg: f64, lat_deg: f64| -> usize {
        let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
        let query = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let mut best = (f64::NEG_INFINITY, 0usize);
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let dot = point.x * query[0] + point.y * query[1] + point.z * query[2];
            if dot > best.0 {
                best = (dot, im);
            }
        }
        best.1
    };

    let mut region = vec![false; mesh.nwd + 1];
    for &(lon, lat) in CASE9_PASS2_DEMAND {
        let seed = nearest_seed(lon, lat);
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(seed, &m_neighbors)
            .expect("footprint")
        {
            if iw >= 2 && iw <= mesh.nwd {
                region[iw] = true;
            }
        }
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
        .expect("initial closure");

    let components = mesh
        .method_c_selected_face_components(&region, &m_neighbors)
        .expect("components");
    let perimeter_of = |mask: &[bool]| -> Option<usize> {
        mesh.method_c_perimeters_from_selected_faces(mask, &m_neighbors)
            .ok()
            .map(|perimeters| perimeters.iter().map(Vec::len).sum())
    };

    for component in &components {
        let mut base_mask = vec![false; mesh.nwd + 1];
        for &iw in component {
            base_mask[iw] = true;
        }
        let Some(base_len) = perimeter_of(&base_mask) else {
            continue;
        };
        if base_len % 3 == 0 {
            continue;
        }
        eprintln!("\ncomponent of {} faces, perimeter {base_len}", component.len());

        let mut frontier = vec![(base_mask, base_len)];
        let mut repaired = None;
        for depth in 1..=MAX_DEPTH {
            let mut next: Vec<(Vec<bool>, usize)> = Vec::new();
            let mut seen_states = std::collections::BTreeSet::new();
            let mut reachable = std::collections::BTreeMap::new();

            'frontier: for (mask, _) in &frontier {
                let mut candidates = std::collections::BTreeSet::new();
                for iw in 2..=mesh.nwd {
                    if !mask[iw] {
                        continue;
                    }
                    for &im in &mesh.w_faces[iw].im {
                        if im >= 2 && im <= mesh.nmd {
                            for &neighbor in &faces_at_m[im] {
                                if !mask[neighbor] {
                                    candidates.insert(neighbor);
                                }
                            }
                        }
                    }
                }
                for &candidate in &candidates {
                    let mut grown = mask.clone();
                    grown[candidate] = true;
                    if mesh
                        .close_method_c_concavities_for_level_with_neighbors(&mut grown, &m_neighbors)
                        .is_err()
                    {
                        continue;
                    }
                    let Some(length) = perimeter_of(&grown) else {
                        continue;
                    };
                    let faces = grown.iter().filter(|&&item| item).count();
                    *reachable.entry((length, length % 3)).or_insert(0usize) += 1;
                    if length % 3 == 0 {
                        repaired = Some((depth, length, faces - component.len()));
                        break 'frontier;
                    }
                    // Distinct (size, perimeter) pairs stand in for distinct
                    // shapes; keeping one of each holds the search width down.
                    if seen_states.insert((faces, length)) && next.len() < WIDTH {
                        next.push((grown, length));
                    }
                }
            }
            if repaired.is_some() {
                break;
            }
            eprintln!("  depth {depth}: reachable {reachable:?}");
            if next.is_empty() {
                break;
            }
            frontier = next;
        }

        match repaired {
            Some((depth, length, added)) => eprintln!(
                "  repaired at depth {depth}: perimeter {length}, {added} faces added"
            ),
            None => eprintln!("  no repair within depth {MAX_DEPTH} at width {WIDTH}"),
        }
    }
}

/// Where does repair actually spend its time?
///
/// The widened search costs about twenty times the greedy walk, and the
/// suspicion is that each candidate sweeps arrays sized for the whole mesh
/// while the component being fixed is under eighty faces. Before changing any
/// helper signature it is worth knowing which sweep dominates — the mask copy,
/// the concavity closure, the perimeter walk, the parent check, or the nest_wd
/// build the support test needs. Ignored by default: this is a measurement, not
/// an assertion.
#[test]
#[ignore = "timing measurement, run explicitly"]
fn repair_candidate_cost_breakdown() {
    use std::time::Instant;

    let mesh = MethodCDelaunayMesh::from_icosahedron(243, 0, 1.0, 0.25, 0)
        .expect("NXP=243 base Method-C mesh");
    let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");

    let nearest_seed = |lon_deg: f64, lat_deg: f64| -> usize {
        let (lon, lat) = (lon_deg.to_radians(), lat_deg.to_radians());
        let query = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
        let mut best = (f64::NEG_INFINITY, 0usize);
        for im in 2..=mesh.nmd {
            let point = mesh.m_points[im];
            let dot = point.x * query[0] + point.y * query[1] + point.z * query[2];
            if dot > best.0 {
                best = (dot, im);
            }
        }
        best.1
    };

    let mut region = vec![false; mesh.nwd + 1];
    for &(lon, lat) in CASE9_PASS2_DEMAND {
        for iw in mesh
            .method_c_rad3_faces_with_neighbors(nearest_seed(lon, lat), &m_neighbors)
            .expect("footprint")
        {
            if iw >= 2 && iw <= mesh.nwd {
                region[iw] = true;
            }
        }
    }
    mesh.close_method_c_concavities_for_level_with_neighbors(&mut region, &m_neighbors)
        .expect("closure");
    let selected_count = region.iter().filter(|&&item| item).count();
    let perimeter = mesh
        .method_c_perimeters_from_selected_faces(&region, &m_neighbors)
        .expect("perimeters")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    eprintln!(
        "nwd={} selected={selected_count} perimeter={}",
        mesh.nwd,
        perimeter.len()
    );

    const REPS: usize = 20;
    let mut scratch = vec![false; mesh.nwd + 1];

    let started = Instant::now();
    for _ in 0..REPS {
        scratch.clear();
        scratch.extend_from_slice(&region);
        std::hint::black_box(&scratch);
    }
    let copy = started.elapsed();

    let started = Instant::now();
    for _ in 0..REPS {
        scratch.clear();
        scratch.extend_from_slice(&region);
        mesh.close_method_c_concavities_for_level_with_neighbors(&mut scratch, &m_neighbors)
            .expect("closure");
    }
    let copy_and_close = started.elapsed();

    let started = Instant::now();
    for _ in 0..REPS {
        std::hint::black_box(
            mesh.method_c_perimeters_from_selected_faces(&region, &m_neighbors)
                .expect("perimeters"),
        );
    }
    let perimeters = started.elapsed();

    let started = Instant::now();
    for _ in 0..REPS {
        std::hint::black_box(region.iter().filter(|&&item| item).count());
    }
    let count = started.elapsed();

    let mut probe = crate::method_c_perimeter_selection::MethodCPerimeterProbe::default();
    let started = Instant::now();
    for _ in 0..REPS {
        std::hint::black_box(
            mesh.method_c_perimeters_from_selected_faces_with_probe(
                &region,
                &m_neighbors,
                &mut probe,
            )
            .expect("perimeters"),
        );
    }
    let perimeters_probed = started.elapsed();

    let started = Instant::now();
    for _ in 0..REPS {
        std::hint::black_box(
            mesh.method_c_nest_wd_from_selected_and_perimeter(&region, &perimeter)
                .expect("nest_wd"),
        );
    }
    let nest_wd = started.elapsed();

    let each = |total: std::time::Duration| total.as_secs_f64() * 1000.0 / REPS as f64;
    let close = each(copy_and_close) - each(copy);
    eprintln!("per candidate, milliseconds:");
    eprintln!("  mask copy                {:8.3}", each(copy));
    eprintln!("  concavity closure        {close:8.3}");
    eprintln!("  perimeter walk           {:8.3}", each(perimeters));
    eprintln!("  perimeter walk (probed)  {:8.3}", each(perimeters_probed));
    eprintln!("  selected count           {:8.3}", each(count));
    eprintln!("  nest_wd build (support)  {:8.3}", each(nest_wd));
    eprintln!(
        "  total                    {:8.3}",
        each(copy) + close + each(perimeters) + each(count) + each(nest_wd)
    );
}

/// Does restricting the start scan change what the walk returns?
///
/// The candidate set is meant to be complete: a start needs `nwdiv == 2`, so it
/// touches a subdivided face and is therefore a corner of one. A spawn_nest
/// regression says otherwise, so compare the two directly over many selections.
#[test]
fn candidate_scan_matches_full_scan() {
    for nxp in [6usize, 9, 12] {
        let mesh = MethodCDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0)
            .expect("base Method-C mesh");
        let m_neighbors = mesh.method_c_m_neighbors().expect("M neighbors");
        let mut differing = 0usize;
        let mut compared = 0usize;
        for im in 2..=mesh.nmd {
            let Ok(footprint) = mesh.method_c_rad3_faces_with_neighbors(im, &m_neighbors) else {
                continue;
            };
            let mut selected = vec![false; mesh.nwd + 1];
            for iw in footprint {
                if iw >= 2 && iw <= mesh.nwd {
                    selected[iw] = true;
                }
            }
            let mut nest_wd = vec![MethodCNestWd::default(); mesh.nwd + 1];
            let mut candidates = Vec::new();
            for iw in 2..=mesh.nwd {
                if selected[iw] {
                    nest_wd[iw].iw[2] = 1;
                    candidates.extend_from_slice(&mesh.w_faces[iw].im);
                }
            }
            candidates.sort_unstable();
            candidates.dedup();
            let full = mesh.perim_maps2_method_c_over(&nest_wd, &m_neighbors, None);
            let restricted =
                mesh.perim_maps2_method_c_over(&nest_wd, &m_neighbors, Some(&candidates));
            compared += 1;
            let same = match (&full, &restricted) {
                (Ok(left), Ok(right)) => left == right,
                (Err(left), Err(right)) => left.to_string() == right.to_string(),
                _ => false,
            };
            if !same {
                if differing < 3 {
                    let describe = |result: &io::Result<Vec<Vec<MethodCPerimeterPoint>>>| match result
                    {
                        Ok(perimeters) => {
                            format!("{:?}", perimeters.iter().map(Vec::len).collect::<Vec<_>>())
                        }
                        Err(error) => format!("Err({error})"),
                    };
                    eprintln!(
                        "  nxp={nxp} seed {im}: full={} restricted={} candidates={}",
                        describe(&full),
                        describe(&restricted),
                        candidates.len()
                    );
                }
                differing += 1;
            }
        }
        eprintln!("nxp={nxp}: {differing}/{compared} 不一致");
        assert_eq!(
            differing, 0,
            "restricting the perimeter start scan to the selection's M corners changed \
             {differing} of {compared} results at nxp={nxp}"
        );
    }
}
