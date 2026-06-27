use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::{geojson_feature_nodes, JsonNode, JsonParser};

/// Flat v3 source categories that can feed data_preprocess-style source state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V3DataSourceKind {
    Landtype,
    Threshold,
    Hydro,
    Coast,
}

/// Normalized v3 data-source descriptor used before concrete readers are bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3DataSourceDescriptor {
    pub kind: V3DataSourceKind,
    pub path: PathBuf,
    pub semantic_layers: Vec<String>,
}

/// Summary from reading a v3 hydro/coast GeoJSON layer produced by MERIT/CaMa tools.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3GeoJsonSourceSummary {
    pub source: V3DataSourceDescriptor,
    pub feature_count: usize,
    pub classes: Vec<String>,
}

/// Paired hydro/coast v3 source evidence carried as Rust-owned state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V3HydroCoastSourceBundle {
    pub sources: Vec<V3DataSourceDescriptor>,
    pub hydro: V3GeoJsonSourceSummary,
    pub coast: V3GeoJsonSourceSummary,
    pub total_feature_count: usize,
}

/// Build a normalized v3 data-source descriptor.
///
/// This keeps the migrated Fortran `landtype_file` and `threshold_dir` inputs
/// on the same flat source boundary as newer hydro/coast products, so later
/// MERIT/CaMa readers can plug in without reintroducing module-global handoff.
pub fn build_v3_data_source_descriptor(
    kind: V3DataSourceKind,
    path: impl AsRef<Path>,
) -> io::Result<V3DataSourceDescriptor> {
    let semantic_layers = match kind {
        V3DataSourceKind::Landtype => ["landtype"].as_slice(),
        V3DataSourceKind::Threshold => ["threshold_fields"].as_slice(),
        V3DataSourceKind::Hydro => ["river_r2", "river_r3", "estuary"].as_slice(),
        V3DataSourceKind::Coast => ["coast_land", "coast_ocean", "shoreline"].as_slice(),
    }
    .iter()
    .map(|layer| (*layer).to_string())
    .collect();
    Ok(V3DataSourceDescriptor {
        kind,
        path: path.as_ref().to_path_buf(),
        semantic_layers,
    })
}

/// Read a standardized v3 hydro/coast GeoJSON source and summarize its classes.
///
/// The existing Python MERIT/CaMa utilities emit bounded GeoJSON feature
/// collections with `hydro_class`, `mask_class`, or `coast_class` properties.
/// This reader gives the Rust migration a concrete source boundary without
/// adding a JSON dependency or reintroducing Fortran module globals.
pub fn read_v3_geojson_source_summary(
    kind: V3DataSourceKind,
    path: impl AsRef<Path>,
) -> io::Result<V3GeoJsonSourceSummary> {
    let keys: &[&str] = match kind {
        V3DataSourceKind::Hydro => &["hydro_class", "river_class", "mask_class"],
        V3DataSourceKind::Coast => &["mask_class", "coast_class"],
        V3DataSourceKind::Landtype | V3DataSourceKind::Threshold => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "v3 GeoJSON source summaries are only defined for hydro/coast sources",
            ));
        }
    };
    let path = path.as_ref();
    let text = fs::read_to_string(path)?;
    let root = JsonParser::new(&text).parse()?;
    let mut classes = BTreeSet::new();
    let features = geojson_feature_nodes(&root);
    for feature in &features {
        let Some(properties) = feature
            .as_object()
            .and_then(|object| object.get("properties"))
            .and_then(JsonNode::as_object)
        else {
            continue;
        };
        for key in keys {
            if let Some(value) = properties.get(*key).and_then(JsonNode::as_str) {
                classes.insert(value.to_string());
            }
        }
    }
    Ok(V3GeoJsonSourceSummary {
        source: build_v3_data_source_descriptor(kind, path)?,
        feature_count: features.len(),
        classes: classes.into_iter().collect(),
    })
}

/// Read paired standardized v3 hydro/coast GeoJSON layers as one source bundle.
pub fn read_v3_hydro_coast_source_bundle(
    hydro_path: impl AsRef<Path>,
    coast_path: impl AsRef<Path>,
) -> io::Result<V3HydroCoastSourceBundle> {
    let hydro = read_v3_geojson_source_summary(V3DataSourceKind::Hydro, hydro_path)?;
    let coast = read_v3_geojson_source_summary(V3DataSourceKind::Coast, coast_path)?;
    let total_feature_count = hydro.feature_count + coast.feature_count;
    let sources = vec![hydro.source.clone(), coast.source.clone()];
    Ok(V3HydroCoastSourceBundle {
        sources,
        hydro,
        coast,
        total_feature_count,
    })
}
