//! CMRC's merge criteria (`&certified_merge`, guide 11.111), orchestrated.
//! The three layers meet only here: the input layer reads each criterion's
//! data in the windows of the domain's base faces
//! (`earthmesh_inputs::merge_layer_samples`), the algorithm builds the
//! lattice field from the samples (`requirement::heterogeneity`), and what
//! was read and found is kept as a record, which the certificate and the
//! preview publish as it is.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_core::{resolution, EarthmeshConfig};
use earthmesh_inputs::merge_layer_samples::{read_window_samples, LonLatWindow};
use earthmesh_refine_certified::mother_grid::lattice;
use earthmesh_refine_certified::requirement::heterogeneity::{
    Criterion, HeterogeneityField, Statistic,
};
use serde::Serialize;

use crate::certified_options::{
    read_certified_merge_options, read_certified_options, CertifiedMergeOptions,
};
use crate::read_method_c_domain_region;
use crate::GridRegion;

/// The lattice field the merge criteria ask for, and how it was made.
pub(super) struct MergeRequirement {
    pub(super) field: HeterogeneityField,
    pub(super) record: MergeRecord,
}

/// What the merge criteria read and found.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub(super) struct MergeRecord {
    source: &'static str,
    base_nxp: usize,
    base_cell_m: f64,
    levels_requested: usize,
    finest_cell_m_requested: f64,
    finest_level_required: usize,
    levels_built: usize,
    base_faces: usize,
    demanding_base_faces: usize,
    /// The criterion mesh -- every face merged where the criteria allow it,
    /// before balance and transitions -- level by level.
    criterion_mesh: Vec<MergeLevelRecord>,
    minimum_samples: u64,
    /// `[west, east, south, north]`, degrees.
    windows: Vec<[f64; 4]>,
    layers: Vec<MergeLayerRecord>,
    criteria: Vec<MergeCriterionRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct MergeLevelRecord {
    level: usize,
    cell_m: f64,
    faces: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct MergeLayerRecord {
    file: String,
    variable: String,
    samples_read: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct MergeCriterionRecord {
    layer: usize,
    statistic: &'static str,
    threshold: f64,
}

/// The merge criteria's requirement over a regional `domain`: every layer
/// read at its own resolution in the windows of the domain's base faces,
/// and the lattice field built bottom up over those faces -- no requirement
/// raster, no h-field. The field is cut to the finest level any face
/// requires (one at least, so reverse coarsening has a level to merge).
pub(super) fn merge_requirement(
    merge: &CertifiedMergeOptions,
    base_nxp: usize,
    domain: &GridRegion,
    maximum_level: usize,
) -> io::Result<MergeRequirement> {
    let invalid_data = |error: String| io::Error::new(io::ErrorKind::InvalidData, error);
    let levels = merge.levels(base_nxp)?;
    if levels > maximum_level {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "CMRC MaximumLevelReached: the merge criteria start {levels} levels below the \
                 base, &certified maximum_level is {maximum_level}"
            ),
        ));
    }
    let faces =
        earthmesh_refine_certified::construction::delivery_base_faces_by_address(domain, base_nxp)?;
    let windows = lattice::lon_lat_boxes(faces.iter().copied()).map_err(invalid_data)?;

    // Input: each distinct layer once, in every window.
    let mut layers = Vec::<(PathBuf, String)>::new();
    let criteria = merge
        .criteria
        .iter()
        .map(|criterion| {
            let key = (criterion.file.clone(), criterion.variable.clone());
            let layer = match layers.iter().position(|layer| *layer == key) {
                Some(layer) => layer,
                None => {
                    layers.push(key);
                    layers.len() - 1
                }
            };
            Criterion {
                layer,
                statistic: criterion.statistic,
                threshold: criterion.threshold,
            }
        })
        .collect::<Vec<_>>();
    let mut samples = Vec::with_capacity(layers.len());
    for (file, variable) in &layers {
        let mut read = Vec::new();
        for &[west, east, south, north] in &windows {
            let window = LonLatWindow {
                west,
                east,
                south,
                north,
            };
            read.extend(
                read_window_samples(file, variable, window).map_err(|error| {
                    io::Error::new(
                        error.kind(),
                        format!(
                            "certified_merge layer {variable} in {}: {error}",
                            file.display()
                        ),
                    )
                })?,
            );
        }
        samples.push(read);
    }
    if samples.iter().all(Vec::is_empty) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "certified_merge: no layer has a valid sample in the regional domain (check that \
             the layers cover it)",
        ));
    }

    // Algorithm: the samples, as points on the unit sphere, into the field.
    let unit = |lon: f64, lat: f64| {
        let (lon, lat) = (lon.to_radians(), lat.to_radians());
        [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
    };
    let field = HeterogeneityField::build(
        base_nxp,
        levels,
        &faces,
        layers.len(),
        samples.iter().enumerate().flat_map(|(layer, read)| {
            read.iter()
                .map(move |&(lon, lat, value)| (layer, unit(lon, lat), value))
        }),
        &criteria,
        merge.minimum_samples,
    )
    .map_err(invalid_data)?;
    let finest_required = field.finest_required();
    let field = field
        .truncated(finest_required.max(1))
        .map_err(invalid_data)?;

    // The record.
    let base_cell_m = resolution::base_cell_metres(base_nxp);
    let cell_m = |level: usize| base_cell_m / (1u64 << level) as f64;
    let record = MergeRecord {
        source: "merge_criteria",
        base_nxp,
        base_cell_m,
        levels_requested: levels,
        finest_cell_m_requested: cell_m(levels),
        finest_level_required: finest_required,
        levels_built: field.levels(),
        base_faces: field.base_faces(),
        demanding_base_faces: field.demanding_base_faces().count(),
        criterion_mesh: field
            .leaves_per_level()
            .iter()
            .enumerate()
            .map(|(level, &faces)| MergeLevelRecord {
                level,
                cell_m: cell_m(level),
                faces,
            })
            .collect(),
        minimum_samples: merge.minimum_samples,
        windows,
        layers: layers
            .iter()
            .zip(&samples)
            .map(|((file, variable), read)| MergeLayerRecord {
                file: file.display().to_string(),
                variable: variable.clone(),
                samples_read: read.len(),
            })
            .collect(),
        criteria: criteria
            .iter()
            .map(|criterion| MergeCriterionRecord {
                layer: criterion.layer,
                statistic: match criterion.statistic {
                    Statistic::StandardDeviation => "std",
                    Statistic::CoefficientOfVariation => "cv",
                    Statistic::Purity => "purity",
                },
                threshold: criterion.threshold,
            })
            .collect(),
    };
    eprintln!(
        "earthmesh_cli: cmrc_merge base_faces={} samples={:?} levels={levels} \
         finest_required={finest_required} leaves_per_level={:?}",
        faces.len(),
        samples.iter().map(Vec::len).collect::<Vec<_>>(),
        field.leaves_per_level()
    );
    Ok(MergeRequirement { field, record })
}

/// What the merge criteria alone ask of a namelist's domain (design H3,
/// step 7), before any coarsening: the record a run would publish in its
/// certificate. The layers are read as a run reads them, so thresholds can
/// be tuned against this in seconds rather than a run.
pub fn certified_merge_preview(namelist_source: &Path) -> io::Result<serde_json::Value> {
    let invalid_input = |message: &str| io::Error::new(io::ErrorKind::InvalidInput, message);
    let contents = fs::read_to_string(namelist_source)?;
    let config = EarthmeshConfig::from_mkgrd_namelist(&contents)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let merge = read_certified_merge_options(&contents)?
        .ok_or_else(|| invalid_input("the namelist has no &certified_merge"))?;
    let options = read_certified_options(&contents)?;
    let domain = (!config.mask_domain_global)
        .then(|| read_method_c_domain_region(&config))
        .transpose()?
        .flatten()
        .ok_or_else(|| invalid_input("CMRC merge criteria need a regional domain"))?;
    let base_nxp = usize::try_from(config.nxp)
        .ok()
        .filter(|&nxp| nxp > 0)
        .ok_or_else(|| invalid_input("CMRC NXP must be positive"))?;
    let requirement = merge_requirement(&merge, base_nxp, &domain, options.maximum_level)?;
    serde_json::to_value(&requirement.record).map_err(io::Error::other)
}
