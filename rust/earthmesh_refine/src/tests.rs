use super::*;
use earthmesh_mesh::LonLatDegrees;

fn evidence(id: &str, violation: f64, scale: Option<f64>, hard: bool) -> DemandEvidence {
    DemandEvidence {
        criterion_id: id.to_string(),
        semantics: CriterionSemantics::TargetScale,
        measured_value: 1.0,
        threshold: 0.0,
        normalized_violation: violation,
        requested_scale_m: scale,
        witness: Some(LonLatDegrees::new(violation, 0.0)),
        confidence: 1.0,
        source_resolution_m: None,
        hard_requirement: hard,
        satisfiable: true,
        stop_reason: None,
    }
}

/// The backend name survives the trip through a namelist and back.
#[test]
fn every_backend_round_trips_through_its_namelist_name() {
    for backend in [
        RefinementBackend::MethodC,
        RefinementBackend::RedGreen,
        RefinementBackend::HarpDv,
        RefinementBackend::Certified,
    ] {
        assert_eq!(
            RefinementBackend::from_engine_str(backend.engine_str()),
            Some(backend)
        );
    }
    assert_eq!(RefinementBackend::from_engine_str("jigsaw"), None);
    assert_eq!(RefinementBackend::default(), RefinementBackend::MethodC);
}

/// Method-C does not read a criterion itself, and the type says so.
#[test]
fn only_the_backends_that_read_criteria_say_they_do() {
    assert!(!RefinementBackend::MethodC.serves_criteria_directly());
    assert!(RefinementBackend::RedGreen.serves_criteria_directly());
    assert!(RefinementBackend::HarpDv.serves_criteria_directly());
    assert!(RefinementBackend::Certified.serves_criteria_directly());
}

/// Which semantics can be driven down by refining, and which cannot.
///
/// The one that cannot is the one a naive loop refines for ever on.
#[test]
fn a_target_scale_criterion_does_not_get_better_by_refining() {
    assert!(!CriterionSemantics::TargetScale.value_falls_with_refinement());
    assert!(CriterionSemantics::ErrorTolerance.value_falls_with_refinement());
    assert!(CriterionSemantics::FeatureCoverage.value_falls_with_refinement());
}

/// A cell asked for by several criteria takes the finest of their scales.
#[test]
fn a_demand_takes_the_finest_scale_any_criterion_asked_for() {
    let demand = RefinementDemand::from_evidence(
        7,
        vec![
            evidence("coast", 0.4, Some(5_000.0), false),
            evidence("terrain", 0.9, Some(1_000.0), false),
            evidence("landcover", 0.2, None, false),
        ],
        RefinementCause::UserSpecified,
    );
    assert_eq!(
        demand.requested_scale_m,
        Some(1_000.0),
        "satisfying the loosest request still fails the others"
    );
    assert_eq!(
        demand.preferred_witness,
        Some(LonLatDegrees::new(0.9, 0.0)),
        "the witness comes from the strongest violation"
    );
    assert!(demand.demands_work());
}

/// One hard requirement makes the whole demand hard.
#[test]
fn a_demand_is_hard_if_any_of_its_evidence_is() {
    let demand = RefinementDemand::from_evidence(
        1,
        vec![
            evidence("soft", 0.9, None, false),
            evidence("named", 0.1, None, true),
        ],
        RefinementCause::UserSpecified,
    );
    assert!(demand.hard);
}

/// Hard first, then priority, then cell id -- and the last term is what makes
/// the order the same on every machine.
#[test]
fn demands_order_the_same_way_every_time() {
    let mut demands = vec![
        RefinementDemand::from_evidence(
            9,
            vec![evidence("a", 0.5, None, false)],
            RefinementCause::UserSpecified,
        ),
        RefinementDemand::from_evidence(
            3,
            vec![evidence("b", 0.5, None, false)],
            RefinementCause::UserSpecified,
        ),
        RefinementDemand::from_evidence(
            5,
            vec![evidence("c", 0.1, None, true)],
            RefinementCause::UserSpecified,
        ),
    ];
    order_demands(&mut demands);
    assert_eq!(
        demands.iter().map(|demand| demand.cell).collect::<Vec<_>>(),
        vec![5, 3, 9],
        "hard first, then priority, then the id as the tie-break"
    );
}

/// Balance is counted apart from the data, so a run cannot report its own
/// bookkeeping as a finding.
#[test]
fn balance_and_quality_causes_are_not_physical() {
    assert!(RefinementCause::UserSpecified.is_physical());
    assert!(RefinementCause::PhysicalCriterion {
        criterion_id: "coast".to_string()
    }
    .is_physical());
    assert!(!RefinementCause::ScaleBalance { ratio_before: 2.0 }.is_physical());
    assert!(!RefinementCause::QualityRepair.is_physical());
}

/// The h-field is reachable from here, which is the point of the module.
#[test]
fn the_h_field_is_reachable_through_the_refinement_layer() {
    let _ = crate::hfield::EARTH_RADIUS_METERS;
}
