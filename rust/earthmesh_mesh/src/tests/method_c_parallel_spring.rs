use super::*;
use rayon::ThreadPoolBuilder;

#[test]
fn method_c_parallel_springs_match_single_worker_exactly() {
    let base =
        MethodCDelaunayMesh::from_icosahedron(16, 0, 1.0, 0.25, 100).expect("base Method-C mesh");
    let one_worker = ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .expect("one-worker pool");
    let two_workers = ThreadPoolBuilder::new()
        .num_threads(2)
        .build()
        .expect("two-worker pool");
    let four_workers = ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .expect("four-worker pool");

    let global_one = one_worker
        .install(|| base.spring_global_with_controls(16, 5, 1.25, 0.035))
        .expect("single-worker global spring");
    let global_two = two_workers
        .install(|| base.spring_global_with_controls(16, 5, 1.25, 0.035))
        .expect("two-worker global spring");
    let global_four = four_workers
        .install(|| base.spring_global_with_controls(16, 5, 1.25, 0.035))
        .expect("parallel global spring");
    assert_eq!(global_two, global_one);
    assert_eq!(global_four, global_one);

    let region = RefinementRegion::Circle {
        center: LonLatDegrees::new(115.0, 25.0),
        radius_meters: 2_500_000.0,
        level: 1,
    };
    let refined = base
        .spawn_nest(std::slice::from_ref(&region), 1)
        .expect("refined Method-C mesh");
    let nest_one = one_worker
        .install(|| refined.spring_nest(16, 5, 2, false))
        .expect("single-worker nest spring");
    let nest_two = two_workers
        .install(|| refined.spring_nest(16, 5, 2, false))
        .expect("two-worker nest spring");
    let nest_four = four_workers
        .install(|| refined.spring_nest(16, 5, 2, false))
        .expect("parallel nest spring");
    assert_eq!(nest_two, nest_one);
    assert_eq!(nest_four, nest_one);
}
