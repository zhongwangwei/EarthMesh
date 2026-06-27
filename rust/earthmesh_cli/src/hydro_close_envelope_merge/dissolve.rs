use crate::hydro_close_buffer::ring_area;
use crate::hydro_close_geometry_utils::*;
use crate::hydro_close_proximity::bbox_distance_deg;
use crate::HydroCloseMaskSpec;

use super::{
    polygon::{
        merge_contained_polygon_close_masks, merge_convex_overlapping_close_masks,
        ring_has_all_bbox_corners_and_interior_vertices,
    },
    rectilinear::{merge_rectilinear_close_masks, split_rectilinear_hole_union_cells},
    shared_edge::merge_shared_edge_polygon_close_masks,
};

pub(crate) fn dissolve_overlapping_envelope_candidates(
    candidates: Vec<(f64, HydroCloseMaskSpec)>,
) -> Vec<(f64, HydroCloseMaskSpec)> {
    let mut dissolved = Vec::<HydroCloseMaskSpec>::new();
    for (_, spec) in candidates {
        insert_dissolved_close_mask_spec(&mut dissolved, spec);
    }
    dissolved
        .into_iter()
        .map(|spec| (ring_area(&spec.coordinates), spec))
        .collect()
}

fn insert_dissolved_close_mask_spec(
    dissolved: &mut Vec<HydroCloseMaskSpec>,
    mut spec: HydroCloseMaskSpec,
) {
    let mut index = 0_usize;
    while index < dissolved.len() {
        if close_mask_specs_can_dissolve(&dissolved[index], &spec) {
            let other = dissolved.remove(index);
            match merge_close_mask_envelopes(other, spec) {
                CloseMaskMergeResult::Single(merged) => {
                    spec = merged;
                    index = 0;
                }
                CloseMaskMergeResult::Multiple(mut merged) => {
                    dissolved.append(&mut merged);
                    return;
                }
            }
        } else {
            index += 1;
        }
    }
    dissolved.push(spec);
}

fn close_mask_specs_can_dissolve(left: &HydroCloseMaskSpec, right: &HydroCloseMaskSpec) -> bool {
    if left.river_class != right.river_class
        || left.refine_degree != right.refine_degree
        || left.target_refine_degree != right.target_refine_degree
    {
        return false;
    }
    let left_bbox = ring_bbox(&left.coordinates);
    let right_bbox = ring_bbox(&right.coordinates);
    if bbox_distance_deg(left_bbox, right_bbox) != 0.0 {
        return false;
    }
    merge_rectilinear_close_masks(&left.coordinates, &right.coordinates).is_some()
        || merge_contained_polygon_close_masks(&left.coordinates, &right.coordinates).is_some()
        || merge_shared_edge_polygon_close_masks(&left.coordinates, &right.coordinates).is_some()
        || merge_convex_overlapping_close_masks(&left.coordinates, &right.coordinates).is_some()
}

enum CloseMaskMergeResult {
    Single(HydroCloseMaskSpec),
    Multiple(Vec<HydroCloseMaskSpec>),
}

fn merge_close_mask_envelopes(
    left: HydroCloseMaskSpec,
    right: HydroCloseMaskSpec,
) -> CloseMaskMergeResult {
    if let Some(coordinates) =
        split_rectilinear_hole_union_cells(&left.coordinates, &right.coordinates)
    {
        let source_feature_index = left.source_feature_index.min(right.source_feature_index);
        let ring_index = left.ring_index.min(right.ring_index);
        return CloseMaskMergeResult::Multiple(
            coordinates
                .into_iter()
                .enumerate()
                .map(|(offset, coordinates)| HydroCloseMaskSpec {
                    river_class: left.river_class.clone(),
                    refine_degree: left.refine_degree,
                    target_refine_degree: left.target_refine_degree,
                    coordinates,
                    source_feature_index,
                    ring_index: ring_index + offset,
                })
                .collect(),
        );
    }
    if let Some(coordinates) = merge_rectilinear_close_masks(&left.coordinates, &right.coordinates)
    {
        return CloseMaskMergeResult::Single(HydroCloseMaskSpec {
            river_class: left.river_class,
            refine_degree: left.refine_degree,
            target_refine_degree: left.target_refine_degree,
            coordinates,
            source_feature_index: left.source_feature_index.min(right.source_feature_index),
            ring_index: left.ring_index.min(right.ring_index),
        });
    }
    if let Some(coordinates) =
        merge_contained_polygon_close_masks(&left.coordinates, &right.coordinates)
    {
        return CloseMaskMergeResult::Single(HydroCloseMaskSpec {
            river_class: left.river_class,
            refine_degree: left.refine_degree,
            target_refine_degree: left.target_refine_degree,
            coordinates,
            source_feature_index: left.source_feature_index.min(right.source_feature_index),
            ring_index: left.ring_index.min(right.ring_index),
        });
    }
    if let Some(coordinates) =
        merge_shared_edge_polygon_close_masks(&left.coordinates, &right.coordinates)
    {
        if (!is_rectilinear_ring(&left.coordinates) || !is_rectilinear_ring(&right.coordinates))
            && ring_has_all_bbox_corners_and_interior_vertices(&coordinates)
        {
            return CloseMaskMergeResult::Multiple(vec![left, right]);
        }
        return CloseMaskMergeResult::Single(HydroCloseMaskSpec {
            river_class: left.river_class,
            refine_degree: left.refine_degree,
            target_refine_degree: left.target_refine_degree,
            coordinates,
            source_feature_index: left.source_feature_index.min(right.source_feature_index),
            ring_index: left.ring_index.min(right.ring_index),
        });
    }
    if let Some(coordinates) =
        merge_convex_overlapping_close_masks(&left.coordinates, &right.coordinates)
    {
        return CloseMaskMergeResult::Single(HydroCloseMaskSpec {
            river_class: left.river_class,
            refine_degree: left.refine_degree,
            target_refine_degree: left.target_refine_degree,
            coordinates,
            source_feature_index: left.source_feature_index.min(right.source_feature_index),
            ring_index: left.ring_index.min(right.ring_index),
        });
    }
    if rectilinear_ring_cells(&left.coordinates).is_some()
        && rectilinear_ring_cells(&right.coordinates).is_some()
    {
        return CloseMaskMergeResult::Multiple(vec![left, right]);
    }
    if !is_rectilinear_ring(&left.coordinates) || !is_rectilinear_ring(&right.coordinates) {
        return CloseMaskMergeResult::Multiple(vec![left, right]);
    }
    let (left_min_lon, left_min_lat, left_max_lon, left_max_lat) = ring_bbox(&left.coordinates);
    let (right_min_lon, right_min_lat, right_max_lon, right_max_lat) =
        ring_bbox(&right.coordinates);
    let min_lon = left_min_lon.min(right_min_lon);
    let min_lat = left_min_lat.min(right_min_lat);
    let max_lon = left_max_lon.max(right_max_lon);
    let max_lat = left_max_lat.max(right_max_lat);
    CloseMaskMergeResult::Single(HydroCloseMaskSpec {
        river_class: left.river_class,
        refine_degree: left.refine_degree,
        target_refine_degree: left.target_refine_degree,
        coordinates: vec![
            (min_lon, min_lat),
            (max_lon, min_lat),
            (max_lon, max_lat),
            (min_lon, max_lat),
        ],
        source_feature_index: left.source_feature_index.min(right.source_feature_index),
        ring_index: left.ring_index.min(right.ring_index),
    })
}
