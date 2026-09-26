use std::collections::BTreeMap;

use crate::HydroCloseMaskSpec;

pub(super) fn apply_composite_refine_degree_cap(
    mut items: Vec<(String, HydroCloseMaskSpec)>,
    max_masks_per_refine_degree: Option<usize>,
) -> Vec<(String, HydroCloseMaskSpec)> {
    let Some(max_masks_per_refine_degree) = max_masks_per_refine_degree else {
        return items;
    };
    items.sort_by(|left, right| {
        left.1
            .refine_degree
            .cmp(&right.1.refine_degree)
            .then_with(|| {
                right
                    .1
                    .target_refine_degree
                    .cmp(&left.1.target_refine_degree)
            })
            .then_with(|| left.1.river_class.cmp(&right.1.river_class))
            .then_with(|| left.0.cmp(&right.0))
            .then_with(|| {
                left.1
                    .source_feature_index
                    .cmp(&right.1.source_feature_index)
            })
            .then_with(|| left.1.ring_index.cmp(&right.1.ring_index))
    });
    let mut counts = BTreeMap::<usize, usize>::new();
    let mut kept = Vec::new();
    for item in items {
        let count = counts.entry(item.1.refine_degree).or_insert(0);
        if *count >= max_masks_per_refine_degree {
            continue;
        }
        *count += 1;
        kept.push(item);
    }
    kept
}
