use crate::geojson_feature_nodes;
use crate::read_text_maybe_gzip;
use crate::HydroCloseMaskNmlOptions;
use crate::HydroCloseMaskSpec;
use crate::JsonNode;
use crate::JsonParser;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use crate::hydro_close_envelope_merge::dissolve_overlapping_envelope_candidates;
use crate::hydro_close_geometry::{
    buffer_close_mask_line_for_refine_degree, buffer_close_mask_ring_for_refine_degree,
    geojson_close_mask_lines, geojson_close_mask_rings, is_close_mask_ring_too_close, ring_area,
    simplify_closed_ring,
};

/// Read hydro/coast GeoJSON polygon rings into close-mask specs without writing files.
pub fn read_hydro_close_mask_specs(
    input_geojson: impl AsRef<Path>,
    options: HydroCloseMaskNmlOptions,
) -> io::Result<Vec<HydroCloseMaskSpec>> {
    let text = read_text_maybe_gzip(input_geojson.as_ref())?;
    geojson_text_to_hydro_close_mask_specs(&text, &options)
}

fn geojson_text_to_hydro_close_mask_specs(
    text: &str,
    options: &HydroCloseMaskNmlOptions,
) -> io::Result<Vec<HydroCloseMaskSpec>> {
    if !options.simplify_tolerance_deg.is_finite() || options.simplify_tolerance_deg < 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "simplify_tolerance_deg must be a finite non-negative number",
        ));
    }
    for (degree, buffer) in &options.buffer_deg_by_refine_degree {
        if *degree == 0 || !buffer.is_finite() || *buffer < 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "buffer_deg_by_refine_degree requires positive degrees and finite non-negative buffers",
            ));
        }
    }
    let root = JsonParser::new(text).parse()?;
    let features = geojson_feature_nodes(&root);
    if features.is_empty() {
        return Ok(Vec::new());
    }

    let mut candidates = Vec::<(f64, HydroCloseMaskSpec)>::new();
    for (feature_index, feature) in features.into_iter().enumerate() {
        let Some(feature_object) = feature.as_object() else {
            continue;
        };
        let river_class = feature_object
            .get("properties")
            .and_then(JsonNode::as_object)
            .and_then(|properties| {
                properties
                    .get("river_class")
                    .and_then(JsonNode::as_str)
                    .or_else(|| properties.get("hydro_class").and_then(JsonNode::as_str))
                    .or_else(|| properties.get("mask_class").and_then(JsonNode::as_str))
            })
            .unwrap_or("")
            .to_string();
        let Some(&target_refine_degree) = options.class_refine.get(&river_class) else {
            continue;
        };
        let Some(geometry) = feature_object.get("geometry").and_then(JsonNode::as_object) else {
            continue;
        };
        let mut geometry_index = 0_usize;
        for coordinates in geojson_close_mask_rings(geometry)? {
            let coordinates = simplify_closed_ring(coordinates, options.simplify_tolerance_deg);
            if coordinates.len() < 3 {
                continue;
            }
            if options.cumulative_refine {
                for refine_degree in 1..=target_refine_degree {
                    let spec = HydroCloseMaskSpec {
                        river_class: river_class.clone(),
                        refine_degree,
                        target_refine_degree,
                        coordinates: buffer_close_mask_ring_for_refine_degree(
                            &coordinates,
                            refine_degree,
                            &options.buffer_deg_by_refine_degree,
                        ),
                        source_feature_index: feature_index,
                        ring_index: geometry_index,
                    };
                    candidates.push((ring_area(&spec.coordinates), spec));
                }
            } else {
                let spec = HydroCloseMaskSpec {
                    river_class: river_class.clone(),
                    refine_degree: target_refine_degree,
                    target_refine_degree,
                    coordinates: buffer_close_mask_ring_for_refine_degree(
                        &coordinates,
                        target_refine_degree,
                        &options.buffer_deg_by_refine_degree,
                    ),
                    source_feature_index: feature_index,
                    ring_index: geometry_index,
                };
                candidates.push((ring_area(&spec.coordinates), spec));
            }
            geometry_index += 1;
        }
        for line in geojson_close_mask_lines(geometry) {
            if options.cumulative_refine {
                for refine_degree in 1..=target_refine_degree {
                    let Some(coordinates) = buffer_close_mask_line_for_refine_degree(
                        &line,
                        refine_degree,
                        &options.buffer_deg_by_refine_degree,
                    ) else {
                        continue;
                    };
                    let coordinates =
                        simplify_closed_ring(coordinates, options.simplify_tolerance_deg);
                    if coordinates.len() < 3 {
                        continue;
                    }
                    let spec = HydroCloseMaskSpec {
                        river_class: river_class.clone(),
                        refine_degree,
                        target_refine_degree,
                        coordinates,
                        source_feature_index: feature_index,
                        ring_index: geometry_index,
                    };
                    candidates.push((ring_area(&spec.coordinates), spec));
                }
            } else {
                let Some(coordinates) = buffer_close_mask_line_for_refine_degree(
                    &line,
                    target_refine_degree,
                    &options.buffer_deg_by_refine_degree,
                ) else {
                    continue;
                };
                let coordinates = simplify_closed_ring(coordinates, options.simplify_tolerance_deg);
                if coordinates.len() < 3 {
                    continue;
                }
                let spec = HydroCloseMaskSpec {
                    river_class: river_class.clone(),
                    refine_degree: target_refine_degree,
                    target_refine_degree,
                    coordinates,
                    source_feature_index: feature_index,
                    ring_index: geometry_index,
                };
                candidates.push((ring_area(&spec.coordinates), spec));
            }
            geometry_index += 1;
        }
    }

    if options.dissolve_overlapping_envelopes {
        candidates = dissolve_overlapping_envelope_candidates(candidates);
    }

    candidates.sort_by(|left, right| {
        left.1
            .refine_degree
            .cmp(&right.1.refine_degree)
            .then_with(|| {
                right
                    .1
                    .target_refine_degree
                    .cmp(&left.1.target_refine_degree)
            })
            .then_with(|| {
                right
                    .0
                    .partial_cmp(&left.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| left.1.river_class.cmp(&right.1.river_class))
            .then_with(|| {
                left.1
                    .source_feature_index
                    .cmp(&right.1.source_feature_index)
            })
            .then_with(|| left.1.ring_index.cmp(&right.1.ring_index))
    });

    let mut emitted_rings_by_class = BTreeMap::<String, BTreeSet<(usize, usize)>>::new();
    let mut emitted_by_refine_degree = BTreeMap::<usize, usize>::new();
    let mut emitted_specs_by_refine_degree = BTreeMap::<usize, Vec<HydroCloseMaskSpec>>::new();
    let mut specs = Vec::new();
    for (_, spec) in candidates {
        let ring_key = (spec.source_feature_index, spec.ring_index);
        let class_rings = emitted_rings_by_class
            .entry(spec.river_class.clone())
            .or_default();
        let class_cap = options
            .max_rings_by_class
            .get(&spec.river_class)
            .copied()
            .or(options.max_rings_per_class);
        if let Some(class_cap) = class_cap {
            if !class_rings.contains(&ring_key) && class_rings.len() >= class_cap {
                continue;
            }
        }
        if let Some(max_masks) = options.max_masks_per_refine_degree {
            if emitted_by_refine_degree
                .get(&spec.refine_degree)
                .copied()
                .unwrap_or(0)
                >= max_masks
            {
                continue;
            }
        }
        if options.min_ring_separation_deg > 0.0
            && is_close_mask_ring_too_close(
                &spec,
                emitted_specs_by_refine_degree
                    .get(&spec.refine_degree)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]),
                options.min_ring_separation_deg,
            )
        {
            continue;
        }
        class_rings.insert(ring_key);
        *emitted_by_refine_degree
            .entry(spec.refine_degree)
            .or_insert(0) += 1;
        emitted_specs_by_refine_degree
            .entry(spec.refine_degree)
            .or_default()
            .push(spec.clone());
        specs.push(spec);
    }
    specs.sort_by(|left, right| {
        left.river_class
            .cmp(&right.river_class)
            .then_with(|| left.refine_degree.cmp(&right.refine_degree))
            .then_with(|| left.source_feature_index.cmp(&right.source_feature_index))
            .then_with(|| left.ring_index.cmp(&right.ring_index))
    });
    Ok(specs)
}
