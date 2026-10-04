//! On-demand reverse coarsening (design B1) against the whole sphere: the
//! same requirement coarsened over a built region and over every face must
//! give the same faces where the region is built -- corner for corner, bit for
//! bit, in the same order -- the same report, with settled parents counted,
//! and the same remap rows for the region's certified cells.

use std::collections::{BTreeMap, BTreeSet};

use earthmesh_refine_certified::{
    coarsen::{
        assemble_region_sphere, run_elastic_component_epochs, run_region_component_epochs,
        ElasticCmrcConfig, ElasticCmrcOutcome, ElasticCmrcResult, RegionEpochs, RegionScope,
        SettledRegion,
    },
    mother_grid::region::descendant_faces,
    requirement::{graded_envelope, one_ring_adjacency},
    AngleContractId, MotherGrid, SourceLevelField, TriangleAddress, VertexAddress,
};

fn config(max_level: usize) -> ElasticCmrcConfig {
    ElasticCmrcConfig {
        angle_contract: AngleContractId::DomainQuality38To82V1,
        max_level,
        max_adjacent_level_delta: 1,
        initial_transition_rings: 1,
        maximum_transition_rings: 4,
        topology_states_per_component: 10_000,
        elastic_iterations_per_topology: 256,
        interval_boxes_per_component: 1_000_000,
        total_transition_states: 100_000,
        allow_safe_fallback: false,
        retry_at_failure: true,
    }
}

/// Requirements per active site of `grid`, from levels per vertex address.
fn by_site(grid: &MotherGrid, levels: &BTreeMap<VertexAddress, usize>) -> Vec<usize> {
    grid.mesh
        .active_vertex_slots()
        .map(|site| levels[grid.addresses[site].as_ref().unwrap()])
        .collect()
}

/// Per vertex slot (dead slots 0).
fn by_slot(grid: &MotherGrid, levels: &BTreeMap<VertexAddress, usize>) -> Vec<usize> {
    (0..grid.mesh.vertices().len())
        .map(|slot| {
            grid.addresses[slot]
                .as_ref()
                .map_or(0, |address| levels[address])
        })
        .collect()
}

fn base_ancestor(mut face: TriangleAddress, base_n: usize) -> TriangleAddress {
    while face.n > base_n {
        face = face.parent_2_to_1().unwrap();
    }
    face
}

/// Base faces sharing a vertex with `faces`, `rings` times over.
fn grown(
    base: &MotherGrid,
    faces: &BTreeSet<TriangleAddress>,
    rings: usize,
) -> BTreeSet<TriangleAddress> {
    let mut grown = faces.clone();
    for _ in 0..rings {
        let corners = base
            .mesh
            .active_triangle_slots()
            .filter(|&face| grown.contains(&base.triangle_addresses[face].unwrap()))
            .flat_map(|face| base.mesh.triangles()[face])
            .collect::<BTreeSet<_>>();
        grown.extend(
            base.mesh
                .active_triangle_slots()
                .filter(|&face| {
                    base.mesh.triangles()[face]
                        .iter()
                        .any(|v| corners.contains(v))
                })
                .map(|face| base.triangle_addresses[face].unwrap()),
        );
    }
    grown
}

/// Faces of a final mesh, each as its corners' addresses and coordinates.
type Face = [(VertexAddress, [u64; 3]); 3];

fn faces(grid: &MotherGrid, result: &ElasticCmrcResult) -> Vec<Face> {
    let mesh = result.state.mesh();
    mesh.mesh
        .active_triangle_slots()
        .map(|face| {
            mesh.mesh.triangles()[face].map(|compact| {
                let source = mesh.source_vertex_slots[compact].unwrap();
                let point = mesh.mesh.vertices()[compact];
                (
                    grid.addresses[source].clone().unwrap(),
                    [point.x.to_bits(), point.y.to_bits(), point.z.to_bits()],
                )
            })
        })
        .collect()
}

/// Runs a requirement of disks -- `(lon, lat, radius in degrees, level)` --
/// on the whole sphere and on a region `rings` base rings around it, and
/// compares them.
fn compare(base_n: usize, levels: usize, disks: &[(f64, f64, f64, usize)], rings: usize) {
    let fine_n = base_n << levels;
    let base = MotherGrid::generate(base_n).unwrap();
    let whole = MotherGrid::generate(fine_n).unwrap();

    // Projected requirement at the given whole-grid slots, then graded as the
    // pipeline grades it.
    let sites = whole.mesh.active_vertex_slots().collect::<Vec<_>>();
    let mut compact = vec![usize::MAX; whole.mesh.vertices().len()];
    for (index, &site) in sites.iter().enumerate() {
        compact[site] = index;
    }
    let mut projected = vec![0; sites.len()];
    for (index, &site) in sites.iter().enumerate() {
        let point = whole.mesh.vertices()[site];
        for &(lon, lat, radius, level) in disks {
            let (lon, lat) = (lon.to_radians(), lat.to_radians());
            let center = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
            let cosine = point.x * center[0] + point.y * center[1] + point.z * center[2];
            if cosine.clamp(-1.0, 1.0).acos() <= radius.to_radians() {
                projected[index] = projected[index].max(level);
            }
        }
    }
    let triangles = whole
        .mesh
        .active_triangle_slots()
        .map(|face| whole.mesh.triangles()[face].map(|site| compact[site]))
        .collect::<Vec<_>>();
    let adjacency = one_ring_adjacency(&[vec![[0; 3]; 2], triangles].concat(), sites.len());
    let graded = graded_envelope(&adjacency, &projected, 3);
    let address_of = |index: usize| whole.addresses[sites[index]].clone().unwrap();
    let projected_at = (0..sites.len())
        .map(|index| (address_of(index), projected[index]))
        .collect::<BTreeMap<_, _>>();
    let graded_at = (0..sites.len())
        .map(|index| (address_of(index), graded[index]))
        .collect::<BTreeMap<_, _>>();

    // R: base faces holding a graded requirement, grown; F: two rings more.
    let demand_faces = whole
        .mesh
        .active_triangle_slots()
        .filter(|&face| {
            whole.mesh.triangles()[face]
                .iter()
                .any(|&site| graded_at[whole.addresses[site].as_ref().unwrap()] > 0)
        })
        .map(|face| base_ancestor(whole.triangle_addresses[face].unwrap(), base_n))
        .collect::<BTreeSet<_>>();
    let certified = grown(&base, &demand_faces, rings);
    let built = grown(&base, &certified, 2);
    let settled = SettledRegion::new(&base, &built).unwrap();
    assert!(settled.blocks() > 0, "the test needs a settled region");
    let settled_copy = settled.clone();
    let region = MotherGrid::generate_faces(
        fine_n,
        descendant_faces(built.iter().copied(), fine_n).unwrap(),
    )
    .unwrap();
    let scope = RegionScope::new(&region, &certified, base_n).unwrap();

    let config = config(levels);
    let ElasticCmrcOutcome::Completed(whole_result) = run_elastic_component_epochs(
        whole.clone(),
        &whole.mesh,
        &SourceLevelField::from_active_voronoi_cells(&whole.mesh, by_site(&whole, &projected_at))
            .unwrap(),
        &by_slot(&whole, &graded_at),
        &config,
    ) else {
        panic!("the whole sphere must complete");
    };
    let outcome = run_region_component_epochs(
        &region,
        &region.mesh,
        &SourceLevelField::from_active_voronoi_cells(&region.mesh, by_site(&region, &projected_at))
            .unwrap(),
        &by_slot(&region, &graded_at),
        &config,
        &RegionEpochs {
            built_bases: built.clone(),
            settled,
            scope,
        },
    );
    let ElasticCmrcOutcome::Completed(region_result) = outcome else {
        panic!("the region must complete: {outcome:?}");
    };

    // The same report: components, counts, histograms.
    assert_eq!(region_result.report, whole_result.report);

    // Assembled with the settled faces, the region is the whole final mesh.
    let sphere =
        assemble_region_sphere(&region, &region_result.state, &settled_copy, base_n, 0).unwrap();
    assert!(
        sphere.mesh == whole_result.state.mesh().mesh,
        "assembled mesh differs"
    );
    assert_eq!(
        sphere.delivered_levels,
        whole_result.state.target_levels().unwrap().levels()
    );

    // The same faces where the region is built, in the same order.
    let region_faces = faces(&region, &region_result);
    let built_vertices = region
        .mesh
        .active_vertex_slots()
        .map(|site| region.addresses[site].clone().unwrap())
        .collect::<BTreeSet<_>>();
    let whole_faces = faces(&whole, &whole_result)
        .into_iter()
        .filter(|face| {
            face.iter()
                .all(|(address, _)| built_vertices.contains(address))
        })
        .collect::<Vec<_>>();
    assert_eq!(region_faces.len(), whole_faces.len());
    assert!(region_faces == whole_faces, "faces differ");

    // The region's remap rows are the whole remap's rows for those cells.
    let (Some(region_remap), Some(whole_remap)) =
        (&region_result.final_remap, &whole_result.final_remap)
    else {
        panic!("both runs commit and hand on their remap");
    };
    let site_address = |grid: &MotherGrid, result: &ElasticCmrcResult, cell: usize| {
        let mesh = result.state.mesh();
        let compact = mesh.mesh.active_vertex_slots().nth(cell).unwrap();
        grid.addresses[mesh.source_vertex_slots[compact].unwrap()]
            .clone()
            .unwrap()
    };
    let source_address = |grid: &MotherGrid, cell: usize| {
        grid.addresses[grid.mesh.active_vertex_slots().nth(cell).unwrap()]
            .clone()
            .unwrap()
    };
    let whole_rows = whole_remap
        .rows()
        .iter()
        .map(|row| {
            (
                site_address(&whole, &whole_result, row.target),
                row.sources
                    .iter()
                    .map(|&(source, weight)| (source_address(&whole, source), weight.to_bits()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert!(!region_remap.rows().is_empty());
    for row in region_remap.rows() {
        let target = site_address(&region, &region_result, row.target);
        let sources = row
            .sources
            .iter()
            .map(|&(source, weight)| (source_address(&region, source), weight.to_bits()))
            .collect::<Vec<_>>();
        assert_eq!(Some(&sources), whole_rows.get(&target), "row of {target:?}");
    }
}

#[test]
fn one_level_around_one_disk_matches_the_whole_sphere() {
    compare(8, 1, &[(102.7, 25.0, 10.0, 1)], 3);
}

/// Slow (about a minute in release): `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn one_level_on_a_finer_base_matches_the_whole_sphere() {
    compare(16, 1, &[(102.7, 25.0, 6.0, 1)], 7);
}

/// Slow (about ten minutes in release): `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn two_regions_joined_through_the_settled_region_match_the_whole_sphere() {
    compare(16, 1, &[(102.7, 25.0, 6.0, 1), (-60.0, -30.0, 6.0, 1)], 7);
}

/// Two levels: the settled region coarsens twice, through level grids built
/// over the region.
#[test]
fn two_levels_around_one_disk_match_the_whole_sphere() {
    compare(8, 2, &[(102.7, 25.0, 8.0, 2)], 5);
}
