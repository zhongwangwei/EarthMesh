//! E0-1: how does multilevel Method-C actually fail?
//!
//! A global coastal run had 25 of 59 region groups refused, which is the number
//! that started all of this, but nobody had classified the refusals. This runs
//! random concentric multilevel cases and sorts every failure by the gate that
//! produced it. The table is the point; it is the first count of what actually
//! stops a second and third level.

use super::*;

/// Deterministic sampling, so a row in the table can be reproduced.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    fn unit(&mut self) -> f64 {
        (self.next() % 1_000_000) as f64 / 1_000_000.0
    }
}

/// Which gate a refusal came from, by the message the kernel wrote.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Gate {
    /// Selection walk left the parent generation.
    G1Selection,
    /// mrow clearance to the parent's fine edge.
    G2Mrow,
    /// A transition face out of generation.
    G3Transition,
    /// A perimeter ring that is not a multiple of three.
    G4Triplets,
    /// The walk itself: no convex start, a revisit, or a ring past seven.
    G5Walk,
    /// Something the five gates do not name.
    Unclassified,
}

fn classify(message: &str) -> Gate {
    if message.contains("cannot be grouped into transition triples") {
        Gate::G4Triplets
    } else if message.contains("revisited M point")
        || message.contains("nwdiv == 2")
        || message.contains("7-edge")
    {
        Gate::G5Walk
    } else if message.contains("in Method-C mrow") {
        Gate::G2Mrow
    } else if message.contains("in Method-C transition") {
        Gate::G3Transition
    } else if message.contains("crosses the parent boundary") {
        Gate::G1Selection
    } else {
        Gate::Unclassified
    }
}

#[test]
fn multilevel_failures_are_counted_by_the_gate_that_produced_them() {
    let mut rows: Vec<(
        usize,
        usize,
        usize,
        usize,
        std::collections::BTreeMap<Gate, usize>,
    )> = Vec::new();
    for nxp in [21usize, 40] {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
        for levels in [2usize, 3, 4] {
            let mut rng = Lcg(0xE0_0001_u64 + (nxp * 100 + levels) as u64);
            let (mut ok, mut err) = (0usize, 0usize);
            let mut gates: std::collections::BTreeMap<Gate, usize> = Default::default();
            let cases = 40;
            for _ in 0..cases {
                let lon = rng.unit() * 360.0 - 180.0;
                let lat = rng.unit() * 140.0 - 70.0;
                // Concentric, halving outward-in, the shape the radius ladder
                // produces and the only one multilevel is known to serve.
                let outer = 800_000.0 + rng.unit() * 2_200_000.0;
                let regions: Vec<RefinementRegion> = (1..=levels)
                    .map(|level| RefinementRegion::Circle {
                        center: LonLatDegrees::new(lon, lat),
                        radius_meters: outer / 2f64.powi(level as i32 - 1),
                        level,
                    })
                    .collect();
                match mesh.spawn_nest(&regions, levels) {
                    Ok(_) => ok += 1,
                    Err(error) => {
                        err += 1;
                        *gates.entry(classify(&error.to_string())).or_insert(0) += 1;
                    }
                }
            }
            rows.push((nxp, levels, ok, err, gates));
        }
    }

    println!("E0-1 multilevel baseline (40 concentric cases per row)");
    println!("nxp levels  ok  err  gates");
    for (nxp, levels, ok, err, gates) in &rows {
        let detail: Vec<String> = gates
            .iter()
            .map(|(gate, count)| format!("{gate:?}={count}"))
            .collect();
        println!("{nxp:<4}{levels:<7}{ok:<4}{err:<5}{}", detail.join(" "));
    }
    let unclassified: usize = rows
        .iter()
        .filter_map(|(_, _, _, _, gates)| gates.get(&Gate::Unclassified))
        .sum();
    println!("E0-1 unclassified refusals: {unclassified}");
}

/// E0-1b: G3 is the gate that fails, and it is monotone in clearance. How many
/// rows does it want?
///
/// The design derives the inter-level clearance from G2 as three rows. The
/// baseline says the gate that actually refuses concentric multilevel is G3,
/// not G2, so the number G2 gives is the answer to a different question. This
/// sweeps the clearance and counts each gate, which is what a ladder should be
/// built from.
#[test]
fn the_inter_level_clearance_is_swept_against_the_gate_that_fails() {
    println!("E0-1b clearance sweep (40 concentric cases per row, levels = 3)");
    println!("nxp halo_rows  ok  err  gates");
    for nxp in [21usize, 40] {
        let mesh = TriangularMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 0).expect("base mesh");
        // The cell the base generation carries, which every level halves.
        let base_cell = 2.0 * std::f64::consts::PI * 6_371_000.0 / (5.0 * nxp as f64);
        for halo_rows in [0usize, 1, 2, 3, 4, 6] {
            let mut rng = Lcg(0xE0_1B00_u64 + (nxp * 100 + halo_rows) as u64);
            let (mut ok, mut err) = (0usize, 0usize);
            let mut gates: std::collections::BTreeMap<Gate, usize> = Default::default();
            for _ in 0..40 {
                let lon = rng.unit() * 360.0 - 180.0;
                let lat = rng.unit() * 140.0 - 70.0;
                let outer = 800_000.0 + rng.unit() * 2_200_000.0;
                let mut radius = outer;
                let mut regions = Vec::new();
                for level in 1..=3usize {
                    regions.push(RefinementRegion::Circle {
                        center: LonLatDegrees::new(lon, lat),
                        radius_meters: radius.max(base_cell),
                        level,
                    });
                    // Each level sits inside the one above by this many rows of
                    // the generation it is entering.
                    let cell = base_cell / 2f64.powi(level as i32 - 1);
                    radius = radius / 2.0 - halo_rows as f64 * cell;
                }
                match mesh.spawn_nest(&regions, 3) {
                    Ok(_) => ok += 1,
                    Err(error) => {
                        err += 1;
                        *gates.entry(classify(&error.to_string())).or_insert(0) += 1;
                    }
                }
            }
            let detail: Vec<String> = gates
                .iter()
                .map(|(gate, count)| format!("{gate:?}={count}"))
                .collect();
            println!("{nxp:<4}{halo_rows:<11}{ok:<4}{err:<5}{}", detail.join(" "));
        }
    }
}
