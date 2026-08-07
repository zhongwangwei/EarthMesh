//! Regions that do not touch each other, refined in one pass.
//!
//! Selection marches a stride-3 lattice outward from one start and keeps the
//! neighbours the regions contain, so one march covers one connected piece of
//! demand. Handed several scattered regions it refined whichever piece its start
//! fell in and left the rest -- silently, since the run still succeeded and the
//! mesh was still valid.
//!
//! Each connected group is marched on its own now and the masks are combined,
//! so the emit -- which is 99.5% of the cost of a pass and renumbers the whole
//! mesh regardless of how many blocks it carries -- still runs once.

use earthmesh_mesh::{LonLatDegrees, MethodCMesh, RefinementRegion, TriangularMesh};

const NXP: usize = 81;

fn base_meters() -> f64 {
    2.0 * std::f64::consts::PI * 6_371_229.0 / (5.0 * NXP as f64)
}

fn mesh() -> MethodCMesh {
    MethodCMesh::from_icosahedron(NXP, 0, 1.0, 0.25, 0).expect("base mesh")
}

fn circle(lon: f64, lat: f64) -> RefinementRegion {
    RefinementRegion::Circle {
        center: LonLatDegrees::new(lon, lat),
        radius_meters: 0.4 * base_meters(),
        level: 1,
    }
}

fn faces(mesh: &TriangularMesh) -> usize {
    mesh.w_faces.len() - 2
}

#[test]
fn each_disjoint_region_refines_its_own_neighbourhood() {
    let base = mesh();
    let before = faces(&base);

    let one = base.spawn_nest(&[circle(20.0, 10.0)], 1).expect("one");
    let each = faces(&one) - before;
    assert!(each > 0, "a single circle must refine something");

    for (label, regions) in [
        ("two", vec![circle(20.0, 10.0), circle(-150.0, -40.0)]),
        (
            "three",
            vec![
                circle(20.0, 10.0),
                circle(-150.0, -40.0),
                circle(80.0, -20.0),
            ],
        ),
    ] {
        let count = regions.len();
        let refined = base.spawn_nest(&regions, 1).expect(label);
        assert_eq!(
            faces(&refined) - before,
            each * count,
            "{label}: every region must refine as much as one does alone"
        );
    }
}

#[test]
fn scattered_regions_cost_about_what_one_region_costs() {
    // The point of combining the masks rather than refining group by group. A
    // pass is dominated by the emit, which does not care how many blocks the
    // mask holds; refining group by group pays that emit once per group, which
    // measured out at half an hour for a real global case.
    let base = mesh();
    let start = std::time::Instant::now();
    let _ = base.spawn_nest(&[circle(20.0, 10.0)], 1).expect("one");
    let one = start.elapsed();

    let scattered: Vec<_> = (0..8)
        .map(|index| {
            let lon = -180.0 + index as f64 * 45.0;
            let lat = -35.0 + ((index * 13) % 70) as f64;
            circle(lon, lat)
        })
        .collect();
    let start = std::time::Instant::now();
    let _ = base.spawn_nest(&scattered, 1).expect("scattered");
    let many = start.elapsed();

    assert!(
        many < one * 4,
        "eight scattered regions took {many:?} against {one:?} for one -- \
         that is the shape of one emit per group"
    );
}
