//! How often does the production spawn path fail on data-shaped masks, and why?
//!
//! The 2026-08 lattice sweep measured raw footprint unions against the walk and
//! mod-3 gates only, before concavity closing and without emitting. This asks
//! the production question: a contiguous union of whole footprints, handed to
//! `spawn_nest_pass_with_max_mrows` (concavity closing, triplet normalisation,
//! emission, repair ladder), builds or not -- and when it does not, which gate
//! refused it and whether normalisation left a one-face spike.
//!
//! A sweep, not a regression: `cargo test -p earthmesh_refine_method_c --release
//! shape_probe -- --ignored --nocapture`.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }
}

pub(super) struct ShapeProbe {
    pub(super) mesh: MethodCMesh,
    pub(super) m_neighbors: Vec<IcosahedronMPointNeighbors>,
    /// Clear lattice seeds and their lattice neighbours among them.
    pub(super) lattice: Vec<usize>,
    pub(super) lattice_neighbors: BTreeMap<usize, Vec<usize>>,
}

impl ShapeProbe {
    pub(super) fn new(nxp: usize, cap: usize) -> Self {
        let mesh = MethodCMesh::from_icosahedron(nxp, 0, 1.0, 0.25).expect("base mesh");
        let m_neighbors = derive_icosahedron_m_neighbors_canonical_checked(
            mesh.nmd,
            &mesh.u_edges,
            &mesh.w_faces,
        )
        .expect("Method-C neighbors");
        let distance = mesh
            .method_c_defect_ring_distance(&m_neighbors)
            .expect("defect distance");
        let start = (2..=mesh.nmd)
            .find(|&im| distance[im] > 5)
            .expect("a clear M point");
        let lattice = mesh
            .method_c_lattice_seeds_with_clearance(
                start,
                cap,
                METHOD_C_LATTICE_DEFECT_CLEARANCE_RINGS,
                &m_neighbors,
            )
            .expect("lattice seeds");
        let members: BTreeSet<usize> = lattice.iter().copied().collect();
        let mut lattice_neighbors = BTreeMap::new();
        for &seed in &lattice {
            let mut jdone = vec![[false; 6]; mesh.nmd + 1];
            let thirds = mesh
                .method_c_thirdm_neighbors_canonical_with_neighbors(seed, &mut jdone, &m_neighbors)
                .unwrap_or_default();
            lattice_neighbors.insert(
                seed,
                thirds
                    .into_iter()
                    .filter(|third| members.contains(third))
                    .collect::<Vec<_>>(),
            );
        }
        Self {
            mesh,
            m_neighbors,
            lattice,
            lattice_neighbors,
        }
    }

    /// A connected set of `size` lattice seeds grown by a random walk.
    fn blob(&self, rng: &mut Lcg, size: usize) -> Vec<usize> {
        let mut chosen = BTreeSet::new();
        chosen.insert(self.lattice[rng.below(self.lattice.len())]);
        let mut guard = 0;
        while chosen.len() < size && guard < 50 * size {
            guard += 1;
            let from: Vec<usize> = chosen.iter().copied().collect();
            let seed = from[rng.below(from.len())];
            let next = &self.lattice_neighbors[&seed];
            if !next.is_empty() {
                chosen.insert(next[rng.below(next.len())]);
            }
        }
        chosen.into_iter().collect()
    }

    pub(super) fn spikes_after_normalisation(&self, selected: &[bool]) -> Option<usize> {
        let mut mask = selected.to_vec();
        self.mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut mask, &self.m_neighbors)
            .ok()?;
        let perimeter = self
            .mesh
            .repair_method_c_non_triplet_perimeter(&mut mask, &self.m_neighbors, 2)
            .ok()?;
        Some(
            perimeter
                .iter()
                .filter(|point| {
                    let n = self.m_neighbors[point.im];
                    n.iw.iter().take(n.npoly).filter(|&&iw| mask[iw]).count() == 1
                })
                .count(),
        )
    }
}

fn failure_class(error: &io::Error) -> String {
    match method_c_repairable_payload(error) {
        Some(payload) => format!("{:?}", payload.kind),
        None => {
            let text = error.to_string();
            let head: String = text.chars().take(48).collect();
            format!("other:{head}")
        }
    }
}

#[ignore]
#[test]
fn production_spawn_is_measured_on_contiguous_footprint_unions() {
    for nxp in [21usize, 40] {
        let probe = ShapeProbe::new(nxp, 800);
        println!(
            "shape-probe nxp={nxp} clear lattice seeds: {}",
            probe.lattice.len()
        );
        for size in [3usize, 6, 12] {
            let mut rng = Lcg(0x5AFE_0000 + size as u64 * 31 + nxp as u64);
            let mut direct_ok = 0usize;
            let mut full_ok = 0usize;
            let mut full_failures = BTreeMap::<String, usize>::new();
            let mut spiky_masks = 0usize;
            let mut spiky_and_direct_failed = 0usize;
            // [ok, failed] counts split by whether the normalised perimeter
            // passes through a pentagon.
            let mut pentagon_split = [[0usize; 2]; 2];
            // (misaligned corners, failed) -> count, pentagon-free masks only.
            let mut corner_split = BTreeMap::<(usize, bool), usize>::new();
            let samples = 60usize;
            for _ in 0..samples {
                let seeds = probe.blob(&mut rng, size);
                let selected = probe
                    .mesh
                    .method_c_footprint_mask(&seeds, &probe.m_neighbors)
                    .expect("footprints");
                let direct = probe
                    .mesh
                    .spawn_nest_pass_method_c_without_mask_repair(&selected, 2, 7, true);
                direct_ok += usize::from(direct.is_ok());
                let spikes = probe.spikes_after_normalisation(&selected).unwrap_or(0);
                if spikes > 0 {
                    spiky_masks += 1;
                    spiky_and_direct_failed += usize::from(direct.is_err());
                }
                // Corners (a six-valent perimeter point whose selected-face
                // count is not the straight-edge three) that do not open a
                // transition triple.
                let misaligned_corners = {
                    let mut mask = selected.clone();
                    probe
                        .mesh
                        .close_method_c_concavities_for_level_with_neighbors(
                            &mut mask,
                            &probe.m_neighbors,
                        )
                        .ok()
                        .and_then(|_| {
                            probe
                                .mesh
                                .repair_method_c_non_triplet_perimeter(
                                    &mut mask,
                                    &probe.m_neighbors,
                                    2,
                                )
                                .ok()
                        })
                        .map(|perimeter| {
                            perimeter
                                .iter()
                                .enumerate()
                                .filter(|(k, p)| {
                                    let n = probe.m_neighbors[p.im];
                                    let sel =
                                        n.iw.iter().take(n.npoly).filter(|&&iw| mask[iw]).count();
                                    n.npoly == 6 && sel != 3 && k % 3 != 0
                                })
                                .count()
                        })
                        .unwrap_or(usize::MAX)
                };
                let touches_pentagon = {
                    let mut mask = selected.clone();
                    probe
                        .mesh
                        .close_method_c_concavities_for_level_with_neighbors(
                            &mut mask,
                            &probe.m_neighbors,
                        )
                        .ok()
                        .and_then(|_| {
                            probe
                                .mesh
                                .repair_method_c_non_triplet_perimeter(
                                    &mut mask,
                                    &probe.m_neighbors,
                                    2,
                                )
                                .ok()
                        })
                        .is_some_and(|perimeter| {
                            perimeter.iter().any(|p| probe.mesh.impent.contains(&p.im))
                        })
                };
                let outcome = probe
                    .mesh
                    .spawn_nest_pass_with_max_mrows(&selected, 2, 7, true);
                pentagon_split[usize::from(touches_pentagon)][usize::from(outcome.is_err())] += 1;
                if !touches_pentagon {
                    *corner_split
                        .entry((misaligned_corners.min(9), outcome.is_err()))
                        .or_default() += 1;
                }
                match outcome {
                    Ok(_) => full_ok += 1,
                    Err(error) => {
                        *full_failures.entry(failure_class(&error)).or_default() += 1;
                        println!(
                            "shape-probe-failure nxp={nxp} size={size} class={} m_point={:?} seeds={seeds:?}",
                            failure_class(&error),
                            method_c_repairable_payload(&error).and_then(|p| p.m_point)
                        );
                    }
                }
            }
            println!(
                "shape-probe nxp={nxp} size={size}: direct_ok={direct_ok}/{samples} full_ok={full_ok}/{samples} \
                 spiky_after_norm={spiky_masks} (direct failed on {spiky_and_direct_failed}) failures={full_failures:?} \
                 [no pentagon on perimeter ok/fail, pentagon ok/fail]={pentagon_split:?} \
                 misaligned_corners(no pentagon)={corner_split:?}"
            );
        }
    }
}

/// Write a failing case's normalised mask and perimeter for plotting:
/// `EM_SHAPE_DUMP=/path EM_SHAPE_NXP=21 EM_SHAPE_SEEDS=20,614,... cargo test ... dump_shape -- --ignored`.
#[ignore]
#[test]
fn dump_shape() {
    use std::io::Write;
    let (Ok(path), Ok(nxp), Ok(seeds)) = (
        std::env::var("EM_SHAPE_DUMP"),
        std::env::var("EM_SHAPE_NXP"),
        std::env::var("EM_SHAPE_SEEDS"),
    ) else {
        return;
    };
    let probe = ShapeProbe::new(nxp.parse().unwrap(), 800);
    let seeds: Vec<usize> = seeds
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let mut mask = probe
        .mesh
        .method_c_footprint_mask(&seeds, &probe.m_neighbors)
        .unwrap();
    probe
        .mesh
        .close_method_c_concavities_for_level_with_neighbors(&mut mask, &probe.m_neighbors)
        .unwrap();
    let perimeter = probe
        .mesh
        .repair_method_c_non_triplet_perimeter(&mut mask, &probe.m_neighbors, 2)
        .unwrap();
    let ll = |im: usize| xyz_to_lonlat_degrees(probe.mesh.m_points[im]);
    let mut out = std::fs::File::create(format!("{path}_faces.csv")).unwrap();
    writeln!(out, "iw,sel,x0,y0,x1,y1,x2,y2").unwrap();
    let footprint: BTreeSet<usize> = perimeter
        .iter()
        .flat_map(|p| {
            let n = probe.m_neighbors[p.im];
            n.iw.into_iter().take(n.npoly).collect::<Vec<_>>()
        })
        .collect();
    for iw in 2..=probe.mesh.nwd {
        if !mask[iw] && !footprint.contains(&iw) {
            continue;
        }
        let mut vs: Vec<usize> = probe.mesh.w_faces[iw]
            .iu
            .iter()
            .flat_map(|&iu| probe.mesh.u_edges[iu].im)
            .collect();
        vs.sort_unstable();
        vs.dedup();
        if vs.len() != 3 {
            continue;
        }
        let p: Vec<_> = vs.iter().map(|&im| ll(im)).collect();
        writeln!(
            out,
            "{iw},{},{},{},{},{},{},{}",
            u8::from(mask[iw]),
            p[0].lon_degrees,
            p[0].lat_degrees,
            p[1].lon_degrees,
            p[1].lat_degrees,
            p[2].lon_degrees,
            p[2].lat_degrees
        )
        .unwrap();
    }
    let mut out = std::fs::File::create(format!("{path}_perim.csv")).unwrap();
    writeln!(out, "k,im,lon,lat,sel_faces").unwrap();
    for (k, point) in perimeter.iter().enumerate() {
        let n = probe.m_neighbors[point.im];
        let sel = n.iw.iter().take(n.npoly).filter(|&&iw| mask[iw]).count();
        let p = ll(point.im);
        writeln!(
            out,
            "{k},{},{},{},{sel}",
            point.im, p.lon_degrees, p.lat_degrees
        )
        .unwrap();
    }
    let seeds_out: Vec<String> = seeds
        .iter()
        .map(|&s| format!("{},{},{}", s, ll(s).lon_degrees, ll(s).lat_degrees))
        .collect();
    std::fs::write(format!("{path}_seeds.csv"), seeds_out.join("\n")).unwrap();
    println!("dumped {} perimeter points", perimeter.len());
}

/// For each recorded failure: where the failing point is, its ring distance to
/// the nearest defect, and the smallest defect distance over the perimeter.
#[ignore]
#[test]
fn failures_are_located_against_the_defects() {
    let cases: &[(usize, usize, &[usize])] = &[
        (21, 131, &[1825, 1888, 1891, 1948, 2008, 4015]),
        (21, 570, &[20, 614, 677, 740, 743, 806]),
        (
            21,
            4140,
            &[
                461, 521, 524, 581, 584, 1121, 1124, 1181, 1184, 1187, 1247, 1250,
            ],
        ),
        (
            21,
            326,
            &[105, 600, 663, 720, 723, 726, 780, 783, 786, 849, 2232, 2292],
        ),
        (
            21,
            445,
            &[
                1381, 1384, 1441, 1444, 1447, 1450, 1507, 1510, 1567, 1630, 2082, 3574,
            ],
        ),
        (
            21,
            41,
            &[28, 88, 487, 490, 550, 888, 891, 948, 951, 1011, 1791, 1794],
        ),
        (
            40,
            2083,
            &[
                1606, 1609, 1726, 3208, 3211, 3325, 3328, 4885, 4888, 5005, 6563, 6566,
            ],
        ),
    ];
    for &(nxp, failing, seeds) in cases {
        let probe = ShapeProbe::new(nxp, 800);
        let distance = probe
            .mesh
            .method_c_defect_ring_distance(&probe.m_neighbors)
            .unwrap();
        let mut mask = probe
            .mesh
            .method_c_footprint_mask(seeds, &probe.m_neighbors)
            .unwrap();
        probe
            .mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut mask, &probe.m_neighbors)
            .unwrap();
        let perimeter = probe
            .mesh
            .repair_method_c_non_triplet_perimeter(&mut mask, &probe.m_neighbors, 2)
            .unwrap();
        let on_perimeter = perimeter.iter().position(|p| p.im == failing);
        let min_perimeter_defect = perimeter.iter().map(|p| distance[p.im]).min().unwrap();
        let pentagons_on_perimeter = perimeter
            .iter()
            .filter(|p| probe.mesh.impent.contains(&p.im))
            .count();
        let ll = xyz_to_lonlat_degrees(probe.mesh.m_points[failing]);
        println!(
            "locate nxp={nxp} failing={failing} at ({:.2},{:.2}) defect_dist={} on_perimeter={on_perimeter:?} \
             min_perimeter_defect_dist={min_perimeter_defect} pentagons_on_perimeter={pentagons_on_perimeter}",
            ll.lon_degrees, ll.lat_degrees, distance[failing]
        );
    }
}

/// The unrepaired emission's own error for the two failures whose perimeter
/// touches no pentagon: the repair ladder rewrites the mask before it gives
/// up, so the full path reports a point on a mask this is not.
#[ignore]
#[test]
fn direct_failures_without_pentagons_are_located() {
    let cases: &[&[usize]] = &[
        &[105, 600, 663, 720, 723, 726, 780, 783, 786, 849, 2232, 2292],
        &[28, 88, 487, 490, 550, 888, 891, 948, 951, 1011, 1791, 1794],
    ];
    let probe = ShapeProbe::new(21, 800);
    let distance = probe
        .mesh
        .method_c_defect_ring_distance(&probe.m_neighbors)
        .unwrap();
    for seeds in cases {
        let selected = probe
            .mesh
            .method_c_footprint_mask(seeds, &probe.m_neighbors)
            .unwrap();
        let error = probe
            .mesh
            .spawn_nest_pass_method_c_without_mask_repair(&selected, 2, 7, true)
            .expect_err("recorded failure");
        let payload = method_c_repairable_payload(&error);
        let point = payload.and_then(|p| p.m_point);
        let location = point.map(|im| {
            let ll = xyz_to_lonlat_degrees(probe.mesh.m_points[im]);
            (ll.lon_degrees, ll.lat_degrees, distance[im])
        });
        println!(
            "direct kind={:?} point={point:?} at={location:?} message={}",
            payload.map(|p| p.kind),
            error.to_string().chars().take(160).collect::<String>()
        );
    }
}

/// Normalise exactly as production does, then emit once without the ladder,
/// and print the perimeter stretch around the point that fails.
#[ignore]
#[test]
fn normalised_failures_are_located_on_their_perimeter() {
    let cases: &[(usize, &[usize])] = &[
        (21, &[1825, 1888, 1891, 1948, 2008, 4015]),
        (21, &[20, 614, 677, 740, 743, 806]),
        (
            21,
            &[
                461, 521, 524, 581, 584, 1121, 1124, 1181, 1184, 1187, 1247, 1250,
            ],
        ),
        (
            21,
            &[105, 600, 663, 720, 723, 726, 780, 783, 786, 849, 2232, 2292],
        ),
        (
            21,
            &[
                1381, 1384, 1441, 1444, 1447, 1450, 1507, 1510, 1567, 1630, 2082, 3574,
            ],
        ),
        (
            21,
            &[28, 88, 487, 490, 550, 888, 891, 948, 951, 1011, 1791, 1794],
        ),
        (
            40,
            &[
                1606, 1609, 1726, 3208, 3211, 3325, 3328, 4885, 4888, 5005, 6563, 6566,
            ],
        ),
    ];
    for &(nxp, seeds) in cases {
        let probe = ShapeProbe::new(nxp, 800);
        let mut mask = probe
            .mesh
            .method_c_footprint_mask(seeds, &probe.m_neighbors)
            .unwrap();
        probe
            .mesh
            .close_method_c_concavities_for_level_with_neighbors(&mut mask, &probe.m_neighbors)
            .unwrap();
        let perimeter = probe
            .mesh
            .repair_method_c_non_triplet_perimeter(&mut mask, &probe.m_neighbors, 2)
            .unwrap();
        let turn = |im: usize| {
            let n = probe.m_neighbors[im];
            let sel = n.iw.iter().take(n.npoly).filter(|&&iw| mask[iw]).count();
            format!("{sel}/{}", n.npoly)
        };
        let result = probe
            .mesh
            .spawn_nest_pass_method_c_without_mask_repair(&mask, 2, 7, true);
        let Err(error) = result else {
            println!("normalised nxp={nxp} seeds={} -> builds", seeds.len());
            continue;
        };
        let payload = method_c_repairable_payload(&error);
        let point = payload.and_then(|p| p.m_point);
        let at = point.and_then(|im| perimeter.iter().position(|p| p.im == im));
        let window = at.map(|k| {
            (k.saturating_sub(4)..(k + 5).min(perimeter.len()))
                .map(|j| format!("{j}(pos{}):{}", j % 3, turn(perimeter[j].im)))
                .collect::<Vec<_>>()
                .join(" ")
        });
        println!(
            "normalised nxp={nxp} seeds={} kind={:?} point={point:?} perimeter_index={at:?} \
             pentagon_on_perimeter={} window=[{}]",
            seeds.len(),
            payload.map(|p| p.kind),
            perimeter.iter().any(|p| probe.mesh.impent.contains(&p.im)),
            window.unwrap_or_default()
        );
    }
}
