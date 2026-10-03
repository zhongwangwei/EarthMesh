//! The merge-if-homogeneous requirement driving reverse coarsening (design
//! H2, `docs/certified_mesh/heterogeneity_merge.md`): samples of a synthetic
//! DEM -- a gentle slope with a rough patch -- give a lattice requirement
//! field; its demanding faces seed the extent, its faces give the region
//! mother's sites their requirement, and the region epochs coarsen to it.
//! The slope merges to the base; the patch keeps the finest level.
//!
//! A compact patch, not a thin feature: a long narrow band of the finest
//! level (a 50 m step along a line) has a long transition boundary, and its
//! topology search takes many minutes (guide 11.110).

use std::collections::BTreeSet;

use earthmesh_refine_certified::{
    coarsen::{
        run_region_component_epochs, ElasticCmrcConfig, ElasticCmrcOutcome, RegionEpochs,
        RegionScope, SettledRegion,
    },
    mother_grid::{lattice, region::descendant_faces},
    on_demand::{materialization_extent_from_seeds, ExtentMargins},
    requirement::{
        graded_envelope,
        heterogeneity::{Criterion, HeterogeneityField, Statistic},
        one_ring_adjacency,
    },
    AngleContractId, MotherGrid, SourceLevelField, TriangleAddress,
};

fn unit(lon: f64, lat: f64) -> [f64; 3] {
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn lon_lat(point: [f64; 3]) -> (f64, f64) {
    (
        point[1].atan2(point[0]).to_degrees(),
        point[2].clamp(-1.0, 1.0).asin().to_degrees(),
    )
}

/// A gentle slope (10 m per degree of longitude) with a rough patch -- 30 m
/// of pseudo-random relief -- within 0.8 degrees of Kunming (about one and a
/// half base edges at n = 120).
fn height(point: [f64; 3]) -> f64 {
    let (lon, lat) = lon_lat(point);
    let slope = 1000.0 + 10.0 * (lon - 102.7);
    let (dlon, dlat) = ((lon - 102.7) * 25.0f64.to_radians().cos(), lat - 25.0);
    if dlon * dlon + dlat * dlat > 0.8 * 0.8 {
        return slope;
    }
    let hash = (point[0] * 1.0e9).to_bits() ^ (point[1] * 1.0e9).to_bits().rotate_left(21);
    slope + 30.0 * ((hash % 1000) as f64 / 1000.0)
}

fn inside(face: TriangleAddress, weights: [f64; 3]) -> [f64; 3] {
    let corners = lattice::face_corner_points(face).unwrap();
    let mut point = [0.0; 3];
    for (corner, weight) in corners.iter().zip(weights) {
        for axis in 0..3 {
            point[axis] += corner[axis] * weight;
        }
    }
    let length = point.iter().map(|value| value * value).sum::<f64>().sqrt();
    point.map(|value| value / length)
}

#[test]
fn a_heterogeneity_field_drives_the_region_epochs() {
    let (base_n, levels) = (120, 1);
    let fine_n = base_n << levels;
    let centre = lattice::locate(base_n, unit(102.7, 25.0)).unwrap();
    let mut covered = BTreeSet::from([centre]);
    for _ in 0..3 {
        for face in covered.clone() {
            covered.extend(lattice::faces_around(face).unwrap());
        }
    }
    // Three samples in every finest face of the covered base faces.
    let samples = descendant_faces(covered.iter().copied(), fine_n)
        .unwrap()
        .into_iter()
        .flat_map(|face| {
            [[1.0, 1.0, 1.0], [4.0, 1.0, 1.0], [1.0, 1.0, 4.0]]
                .map(|weights| {
                    let point = inside(face, weights);
                    (point, vec![height(point)])
                })
                .into_iter()
        })
        .collect::<Vec<_>>();
    let field = HeterogeneityField::build(
        base_n,
        levels,
        &covered,
        1,
        samples,
        &[Criterion {
            layer: 0,
            statistic: Statistic::StandardDeviation,
            threshold: 5.0,
        }],
        2,
    )
    .unwrap();
    let leaves = field.leaves_per_level();
    assert!(
        leaves[0] > 0 && leaves[levels] > 0,
        "leaves per level {leaves:?}"
    );
    let demanding = field.demanding_base_faces().count();
    assert!(
        demanding > 0 && demanding < covered.len(),
        "{demanding} of {}",
        covered.len()
    );

    // The extent from the demanding faces, the region mother over it.
    let gradation = 3;
    let seeds = field.seed_faces(field.reach_rings(gradation)).unwrap();
    let extent = materialization_extent_from_seeds(
        base_n,
        levels,
        ExtentMargins {
            gradation_rings_per_level: gradation,
            parent_rings_per_level: 7,
        },
        &seeds,
        &BTreeSet::new(),
    )
    .unwrap();
    let built = extent.built_faces().collect::<BTreeSet<_>>();
    let settled = SettledRegion::by_address(base_n, &built).unwrap();
    let region = MotherGrid::generate_faces(
        fine_n,
        descendant_faces(built.iter().copied(), fine_n).unwrap(),
    )
    .unwrap();

    // Each site's requirement from the field, graded as the pipeline grades.
    let projected = field.required_by_site(&region).unwrap();
    let sites = region.mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut compact = vec![usize::MAX; region.mesh.vertices().len()];
    for (row, &site) in sites.iter().enumerate() {
        compact[site] = row;
    }
    let triangles = region
        .mesh
        .active_triangle_slots()
        .map(|face| region.mesh.triangles()[face].map(|site| compact[site]))
        .collect::<Vec<_>>();
    let adjacency = one_ring_adjacency(&[vec![[0; 3]; 2], triangles].concat(), sites.len());
    let graded = graded_envelope(&adjacency, &projected, gradation);
    let mut graded_by_slot = vec![0; region.mesh.vertices().len()];
    for (&site, &level) in sites.iter().zip(&graded) {
        graded_by_slot[site] = level;
    }
    let outcome = run_region_component_epochs(
        region.clone(),
        &region.mesh,
        &SourceLevelField::from_active_voronoi_cells(&region.mesh, projected.clone()).unwrap(),
        &graded_by_slot,
        &ElasticCmrcConfig {
            angle_contract: AngleContractId::DomainQuality38To82V1,
            max_level: levels,
            max_adjacent_level_delta: 1,
            initial_transition_rings: 1,
            maximum_transition_rings: 4,
            topology_states_per_component: 10_000,
            elastic_iterations_per_topology: 256,
            interval_boxes_per_component: 1_000_000,
            total_transition_states: 100_000,
            allow_safe_fallback: false,
        },
        &RegionEpochs {
            built_bases: built,
            settled,
            scope: RegionScope::new(&region, &extent.region, base_n).unwrap(),
        },
    );
    let ElasticCmrcOutcome::Completed(result) = outcome else {
        panic!("the region must complete: {outcome:?}");
    };
    // The patch keeps the finest level, the slope merges to the base: every
    // final cell is certified against its sites' requirement inside the
    // epochs, and both ends of the range are delivered.
    let delivered = result.state.target_levels().unwrap().levels().to_vec();
    assert!(delivered.contains(&levels) && delivered.contains(&0));
    assert!(result.report.components_committed > 0);
}
