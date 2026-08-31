//! Rotation-aware width evidence for shared parent-layer trace vertices.

use super::{
    stratified_annulus::{band_face_labels, directed_traces, vertex_rotation},
    BandFaceLabel, CoupledAnnulus, DirectedTrace, HierarchyComponent, StratifiedAnnulus,
    StratifiedAnnulusError,
};
use crate::mother_grid::{MotherGrid, VertexAddress};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JunctionWidthClass {
    TruePinch,
    OneFaceWedge { faces: usize },
    MultiFaceWedge { faces: usize },
    AnchorJunction { faces: usize },
}

impl JunctionWidthClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TruePinch => "TruePinch",
            Self::OneFaceWedge { .. } => "OneFaceWedge",
            Self::MultiFaceWedge { .. } => "MultiFaceWedge",
            Self::AnchorJunction { .. } => "AnchorJunction",
        }
    }

    pub fn face_count(&self) -> usize {
        match self {
            Self::TruePinch => 0,
            Self::OneFaceWedge { faces }
            | Self::MultiFaceWedge { faces }
            | Self::AnchorJunction { faces } => *faces,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationAwareWidthEvidence {
    pub shared_vertex: usize,
    pub left_trace: usize,
    pub right_trace: usize,
    pub rotation_faces: Vec<usize>,
    pub between_trace_face_wedges: Vec<Vec<usize>>,
    pub width_class: JunctionWidthClass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationAwareWidthReport {
    pub adjacent_trace_shared_occurrence_count: usize,
    pub unique_shared_vertex_count: usize,
    pub true_pinch_count: usize,
    pub one_face_wedge_count: usize,
    pub multi_face_wedge_count: usize,
    pub anchor_junction_count: usize,
    pub evidence: Vec<RotationAwareWidthEvidence>,
}

pub fn audit_rotation_aware_width(
    source: &MotherGrid,
    stratified: &StratifiedAnnulus,
) -> RotationAwareWidthReport {
    audit(source, &stratified.traces, &stratified.band_face_labels)
}

pub fn audit_coupled_rotation_aware_width(
    source: &MotherGrid,
    component: &HierarchyComponent,
    coupled: &CoupledAnnulus,
) -> Result<RotationAwareWidthReport, StratifiedAnnulusError> {
    let traces = directed_traces(coupled);
    let labels = band_face_labels(source, component, coupled, &traces)?;
    Ok(audit(source, &traces, &labels))
}

fn audit(
    source: &MotherGrid,
    traces: &[DirectedTrace],
    labels: &[BandFaceLabel],
) -> RotationAwareWidthReport {
    let evidence = traces
        .windows(2)
        .enumerate()
        .flat_map(|(band_id, pair)| {
            let left = trace_vertices(&pair[0]);
            let right = trace_vertices(&pair[1]);
            left.intersection(&right)
                .copied()
                .map(|shared_vertex| {
                    let rotation = vertex_rotation(source, shared_vertex)
                        .expect("trace vertex must retain its source rotation");
                    let band_faces = labels
                        .iter()
                        .filter_map(|label| {
                            (label.band_id == band_id
                                && source.mesh.triangles()[label.face_slot]
                                    .contains(&shared_vertex))
                            .then_some(label.face_slot)
                        })
                        .collect::<BTreeSet<_>>();
                    let between_trace_face_wedges =
                        cyclic_wedges(&rotation.incident_faces, &band_faces);
                    let faces = between_trace_face_wedges
                        .iter()
                        .map(Vec::len)
                        .min()
                        .unwrap_or(0);
                    let anchor = matches!(
                        source.addresses[shared_vertex],
                        Some(VertexAddress::IcosahedronVertex(_))
                    );
                    RotationAwareWidthEvidence {
                        shared_vertex,
                        left_trace: band_id,
                        right_trace: band_id + 1,
                        rotation_faces: rotation.incident_faces,
                        between_trace_face_wedges,
                        width_class: classify_width(anchor, faces),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let unique_shared_vertex_count = evidence
        .iter()
        .map(|item| item.shared_vertex)
        .collect::<BTreeSet<_>>()
        .len();
    RotationAwareWidthReport {
        adjacent_trace_shared_occurrence_count: evidence.len(),
        unique_shared_vertex_count,
        true_pinch_count: evidence
            .iter()
            .filter(|item| item.width_class == JunctionWidthClass::TruePinch)
            .count(),
        one_face_wedge_count: evidence
            .iter()
            .filter(|item| matches!(item.width_class, JunctionWidthClass::OneFaceWedge { .. }))
            .count(),
        multi_face_wedge_count: evidence
            .iter()
            .filter(|item| matches!(item.width_class, JunctionWidthClass::MultiFaceWedge { .. }))
            .count(),
        anchor_junction_count: evidence
            .iter()
            .filter(|item| matches!(item.width_class, JunctionWidthClass::AnchorJunction { .. }))
            .count(),
        evidence,
    }
}

pub fn rotation_aware_width_report_json(report: &RotationAwareWidthReport) -> String {
    let evidence = report
        .evidence
        .iter()
        .map(|item| {
            let rotation = usize_json(&item.rotation_faces);
            let wedges = format!(
                "[{}]",
                item.between_trace_face_wedges
                    .iter()
                    .map(|wedge| usize_json(wedge))
                    .collect::<Vec<_>>()
                    .join(",")
            );
            format!(
                "{{\"shared_vertex\":{},\"left_trace\":{},\"right_trace\":{},\"rotation_faces\":{},\"between_trace_face_wedges\":{},\"width_class\":\"{}\",\"wedge_faces\":{}}}",
                item.shared_vertex,
                item.left_trace,
                item.right_trace,
                rotation,
                wedges,
                item.width_class.as_str(),
                item.width_class.face_count(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"adjacent_trace_shared_occurrence_count\":{},\"unique_shared_vertex_count\":{},\"true_pinch_count\":{},\"one_face_wedge_count\":{},\"multi_face_wedge_count\":{},\"anchor_junction_count\":{},\"evidence\":[{}]}}",
        report.adjacent_trace_shared_occurrence_count,
        report.unique_shared_vertex_count,
        report.true_pinch_count,
        report.one_face_wedge_count,
        report.multi_face_wedge_count,
        report.anchor_junction_count,
        evidence,
    )
}

fn trace_vertices(trace: &DirectedTrace) -> BTreeSet<usize> {
    trace
        .occurrences
        .iter()
        .map(|occurrence| occurrence.source_slot)
        .collect()
}

fn cyclic_wedges(rotation: &[usize], members: &BTreeSet<usize>) -> Vec<Vec<usize>> {
    if rotation.is_empty() || members.is_empty() {
        return Vec::new();
    }
    if rotation.iter().all(|face| members.contains(face)) {
        return vec![rotation.to_vec()];
    }
    let starts = (0..rotation.len())
        .filter(|&index| {
            members.contains(&rotation[index])
                && !members.contains(&rotation[(index + rotation.len() - 1) % rotation.len()])
        })
        .collect::<Vec<_>>();
    starts
        .into_iter()
        .map(|start| {
            (0..rotation.len())
                .map(|offset| rotation[(start + offset) % rotation.len()])
                .take_while(|face| members.contains(face))
                .collect()
        })
        .collect()
}

fn classify_width(anchor: bool, faces: usize) -> JunctionWidthClass {
    if anchor {
        JunctionWidthClass::AnchorJunction { faces }
    } else {
        match faces {
            0 => JunctionWidthClass::TruePinch,
            1 => JunctionWidthClass::OneFaceWedge { faces },
            _ => JunctionWidthClass::MultiFaceWedge { faces },
        }
    }
}

fn usize_json(values: &[usize]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wedge_classification_distinguishes_true_width() {
        assert_eq!(classify_width(false, 0), JunctionWidthClass::TruePinch);
        assert_eq!(
            classify_width(false, 1),
            JunctionWidthClass::OneFaceWedge { faces: 1 }
        );
        assert_eq!(
            classify_width(false, 2),
            JunctionWidthClass::MultiFaceWedge { faces: 2 }
        );
        assert_eq!(
            classify_width(true, 2),
            JunctionWidthClass::AnchorJunction { faces: 2 }
        );
    }
}
