use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::{default_hydro_close_class_refine, MeritHydroGeoJsonLayerWriteReport};

/// Rust-owned control surface for Python-compatible hydro close-refinement masks.
#[derive(Debug, Clone, PartialEq)]
pub struct HydroCloseRefinementRecipeOptions {
    pub input_geojson: PathBuf,
    pub output_prefix: PathBuf,
    pub class_refine: BTreeMap<String, usize>,
    pub buffer_deg_by_refine_degree: BTreeMap<usize, f64>,
    pub simplify_tolerance_deg: f64,
    pub example_namelist: Option<String>,
}

/// Summary from writing a hydro close-refinement recipe JSON file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroCloseRefinementRecipeWriteReport {
    pub output_json: PathBuf,
    pub max_iter_spc: usize,
    pub class_count: usize,
    pub buffer_count: usize,
}

/// One EarthMesh close-refinement mask derived from a hydro/coast GeoJSON ring.
#[derive(Debug, Clone, PartialEq)]
pub struct HydroCloseMaskSpec {
    pub river_class: String,
    pub refine_degree: usize,
    pub target_refine_degree: usize,
    pub coordinates: Vec<(f64, f64)>,
    pub source_feature_index: usize,
    pub ring_index: usize,
}

/// Native Rust options for exporting GeoJSON rings as close-mask `.nml` files.
#[derive(Debug, Clone, PartialEq)]
pub struct HydroCloseMaskNmlOptions {
    pub class_refine: BTreeMap<String, usize>,
    pub max_rings_per_class: Option<usize>,
    pub max_rings_by_class: BTreeMap<String, usize>,
    pub max_masks_per_refine_degree: Option<usize>,
    pub min_ring_separation_deg: f64,
    pub buffer_deg_by_refine_degree: BTreeMap<usize, f64>,
    pub simplify_tolerance_deg: f64,
    pub dissolve_overlapping_envelopes: bool,
    pub cumulative_refine: bool,
}

impl Default for HydroCloseMaskNmlOptions {
    fn default() -> Self {
        Self {
            class_refine: default_hydro_close_class_refine(),
            max_rings_per_class: None,
            max_rings_by_class: BTreeMap::new(),
            max_masks_per_refine_degree: Some(999),
            min_ring_separation_deg: 0.0,
            buffer_deg_by_refine_degree: BTreeMap::new(),
            simplify_tolerance_deg: 0.0,
            dissolve_overlapping_envelopes: false,
            cumulative_refine: true,
        }
    }
}

/// Summary from writing native Rust close-mask `.nml` files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroCloseMaskNmlWriteReport {
    pub output_prefix: PathBuf,
    pub files: Vec<PathBuf>,
    pub spec_count: usize,
}

/// Per-component summary from a composite close-mask recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroCompositeCloseMaskComponentSummary {
    pub name: String,
    pub input_geojson: String,
    pub files_selected: usize,
    pub class_refine: BTreeMap<String, usize>,
    pub max_rings_by_class: BTreeMap<String, usize>,
    pub max_rings_per_class: Option<usize>,
    pub dissolve_overlapping_envelopes: bool,
}

/// Summary from composing multiple hydro/coast sources into one close-mask set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HydroCompositeCloseMaskNmlWriteReport {
    pub output_prefix: PathBuf,
    pub files: Vec<PathBuf>,
    pub counts_by_component: BTreeMap<String, usize>,
    pub counts_by_class_degree: BTreeMap<String, usize>,
    pub components: Vec<HydroCompositeCloseMaskComponentSummary>,
    pub summary_json: Option<PathBuf>,
}

/// Report from [`write_merit_hydro_region_close_masks`](crate::write_merit_hydro_region_close_masks).
#[derive(Debug, Clone)]
pub struct MeritHydroRegionWorkflowReport {
    pub tile_count: usize,
    pub window_count: usize,
    pub geojson: MeritHydroGeoJsonLayerWriteReport,
    pub river_nml: HydroCloseMaskNmlWriteReport,
    pub coast_nml: HydroCloseMaskNmlWriteReport,
}
