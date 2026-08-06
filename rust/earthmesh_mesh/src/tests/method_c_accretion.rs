//! Can a demand the kernel refuses in one call be covered by accepting whole
//! footprints one at a time?
//!
//! `spawn_nest` is all or nothing per region group: a global coastal run had 25
//! of 59 groups refused, and a refused group contributes nothing. The union
//! measurement says a nine-seed mask closes about a quarter of the time while a
//! single footprint always does, which raises the question this asks. If greedy
//! accretion covers most of a demand where one call covers none, that is a new
//! method. If it stalls after a few footprints, it is not.

use super::*;

fn probe(nxp: usize) -> (TriangularMesh, Vec<IcosahedronMPointNeighbors>) {
    let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
    let m_neighbors =
        derive_icosahedron_m_neighbors_canonical_checked(mesh.nmd, &mesh.u_edges, &mesh.w_faces)
            .expect("Method-C neighbors");
    (mesh, m_neighbors)
}

#[test]
fn greedy_footprint_accretion_is_measured_against_one_shot() {
    for nxp in [21usize, 40] {
        let (mesh, m_neighbors) = probe(nxp);
        let distance = mesh
            .method_c_defect_ring_distance(&m_neighbors)
            .expect("defect distance");

        for radius_km in [800.0f64, 2000.0] {
            let region = RefinementRegion::Circle {
                center: LonLatDegrees::new(30.0, 20.0),
                radius_meters: radius_km * 1000.0,
                level: 1,
            };
            let one_shot = mesh.spawn_nest(std::slice::from_ref(&region), 1);
            let one_shot_faces = one_shot.as_ref().map(|refined| refined.nwd).ok();

            // Seeds whose footprint meets the demand, clear of every defect.
            // Start the walk inside the demand, or the enumeration never
            // reaches it on a larger mesh.
            let start = (2..=mesh.nmd)
                .filter(|&im| distance[im] > 5)
                .min_by(|&a, &b| {
                    let key = |im: usize| {
                        let point = xyz_to_lonlat_degrees(mesh.m_points[im]);
                        (point.lon_degrees - 30.0).abs() + (point.lat_degrees - 20.0).abs()
                    };
                    key(a).partial_cmp(&key(b)).expect("finite")
                })
                .expect("a clear M point near the demand");
            let seeds: Vec<usize> = mesh
                .method_c_lattice_seeds_with_clearance(
                    start,
                    2000,
                    METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS,
                    &m_neighbors,
                )
                .expect("lattice seeds")
                .into_iter()
                .filter(|&im| {
                    region.contains_lonlat_canonical(xyz_to_lonlat_degrees(mesh.m_points[im]))
                })
                .collect();

            // Greedy: keep a footprint only if the cumulative mask still builds.
            let mut cumulative = vec![false; mesh.nwd + 1];
            let mut accepted = 0usize;
            let mut refused = 0usize;
            let mut best: Option<TriangularMesh> = None;
            for &seed in &seeds {
                let mut candidate = cumulative.clone();
                if mesh
                    .mark_fill_rad3_faces_with_neighbors(seed, &mut candidate, &m_neighbors)
                    .is_err()
                {
                    refused += 1;
                    continue;
                }
                match mesh.spawn_nest_pass_with_max_mrows(&candidate, 2, 7, true) {
                    Ok(refined) => {
                        cumulative = candidate;
                        accepted += 1;
                        best = Some(refined);
                    }
                    Err(_) => refused += 1,
                }
            }
            let covered = cumulative.iter().filter(|&&face| face).count();
            println!(
                "accretion nxp={nxp} r={radius_km}km: demand_seeds={} accepted={accepted} refused={refused} \
                 mask_faces={covered} one_shot={:?} accreted_faces={:?}",
                seeds.len(),
                one_shot_faces,
                best.as_ref().map(|refined| refined.nwd)
            );
        }
    }
}
