use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use earthmesh_hfield::{
    DemandComponent, DemandEpochChain, DemandHash, DemandSnapshotError, DemandSource,
    DemandSourceKind, DemandStrength, DemandSupport, DemandSupportKind, HField,
    SourceDemandSnapshot, SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};

use crate::hfield_refine::read_hfield_refine_options;
use crate::read_native_grid_mdomain;

// This artifact freezes both sides of the production contract: exact source
// pins before gradation and the regularized target consumed by Method-C.
const ARTIFACT_KIND: &str = "earthmesh_hfield_source_demand";
const ARTIFACT_SCHEMA_VERSION: u32 = 5;
const HARD_RASTER_TAG: &[u8] = b"earthmesh-hfield-hard-raster-v4\0";
const ARTIFACT_HASH_TAG: &[u8] = b"earthmesh-hfield-demand-artifact-v5\0";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedDemandLayer {
    kind: String,
    descriptor: String,
    levels: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedDemandEpoch {
    epoch_id: u32,
    parent_snapshot_hash: String,
    demand_hash: String,
    epoch_hash: String,
    intended_output_support: Vec<bool>,
    hard_layers: Vec<PersistedDemandLayer>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct PersistedHfieldDemand {
    kind: String,
    schema_version: u32,
    demand_schema_version: u32,
    output_product: String,
    base_project_hash: String,
    project_hash: String,
    snapshot_hash: String,
    chain_tip_hash: String,
    artifact_hash: String,
    gridfile_hash: String,
    nlon: usize,
    nlat: usize,
    base_m: f64,
    max_level: u8,
    g: f64,
    base_hard_levels: Vec<u8>,
    hard_levels: Vec<u8>,
    regularized_levels: Vec<u8>,
    base_intended_output_support: Vec<bool>,
    intended_output_support: Vec<bool>,
    product_support: Vec<bool>,
    hard_layers: Vec<PersistedDemandLayer>,
    epochs: Vec<PersistedDemandEpoch>,
}

pub(crate) struct PreparedHfieldDemand {
    persisted: PersistedHfieldDemand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HfieldDemandProductKind {
    Land,
    Ocean,
}

impl HfieldDemandProductKind {
    fn name(self) -> &'static str {
        match self {
            Self::Land => "land",
            Self::Ocean => "ocean",
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreparedAutoRefineDemandEpoch {
    descriptor: String,
    hard_field: HField,
}

#[derive(Debug)]
pub enum AutoRefineDemandEpochError {
    MissingImmutableAbsoluteTarget,
    RepeatedDemandEpoch {
        demand_hash: DemandHash,
        existing_epoch_id: u32,
        existing_epoch_hash: DemandHash,
    },
    Snapshot(DemandSnapshotError),
    Artifact(io::Error),
}

impl fmt::Display for AutoRefineDemandEpochError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingImmutableAbsoluteTarget => formatter.write_str(
                "AutoRefine adapter did not provide a provable immutable absolute target",
            ),
            Self::RepeatedDemandEpoch {
                demand_hash,
                existing_epoch_id,
                ..
            } => write!(
                formatter,
                "repeated AutoRefine demand epoch payload {demand_hash} already exists at epoch {existing_epoch_id}"
            ),
            Self::Snapshot(error) => error.fmt(formatter),
            Self::Artifact(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for AutoRefineDemandEpochError {}

impl From<io::Error> for AutoRefineDemandEpochError {
    fn from(error: io::Error) -> Self {
        Self::Artifact(error)
    }
}

impl From<DemandSnapshotError> for AutoRefineDemandEpochError {
    fn from(error: DemandSnapshotError) -> Self {
        match error {
            DemandSnapshotError::RepeatedDemandEpoch {
                demand_hash,
                existing_epoch_id,
                existing_epoch_hash,
            } => Self::RepeatedDemandEpoch {
                demand_hash,
                existing_epoch_id,
                existing_epoch_hash,
            },
            other => Self::Snapshot(other),
        }
    }
}

#[derive(Debug)]
pub(crate) struct LoadedHfieldDemand {
    pub snapshot_hash: DemandHash,
    pub chain_tip_hash: DemandHash,
    pub nlon: usize,
    pub nlat: usize,
    pub hard_levels: Vec<u8>,
    pub intended_output_support: Vec<bool>,
    pub base_m: f64,
    pub max_level: u8,
    pub g: f64,
}

impl PreparedHfieldDemand {
    pub(crate) fn capture(
        field: &HField,
        base_m: f64,
        max_level: u8,
        g: f64,
        namelist_contents: &str,
    ) -> io::Result<Self> {
        let layer = crate::hfield_refine::HfieldHardDemandLayer {
            kind: DemandSourceKind::Threshold,
            descriptor: "legacy-composed",
            field: field.clone(),
        };
        Self::capture_with_hard_sources(
            field,
            field,
            std::slice::from_ref(&layer),
            base_m,
            max_level,
            g,
            namelist_contents,
        )
    }

    pub(crate) fn capture_with_hard_sources(
        hard_field: &HField,
        regularized_field: &HField,
        hard_layers: &[crate::hfield_refine::HfieldHardDemandLayer],
        base_m: f64,
        max_level: u8,
        g: f64,
        namelist_contents: &str,
    ) -> io::Result<Self> {
        Self::capture_with_hard_sources_and_product_support(
            hard_field,
            regularized_field,
            hard_layers,
            base_m,
            max_level,
            g,
            namelist_contents,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn capture_with_hard_sources_and_product_support(
        hard_field: &HField,
        regularized_field: &HField,
        hard_layers: &[crate::hfield_refine::HfieldHardDemandLayer],
        base_m: f64,
        max_level: u8,
        g: f64,
        namelist_contents: &str,
        product_support_override: Option<&[bool]>,
    ) -> io::Result<Self> {
        validate_config(base_m, max_level, g)?;
        if (hard_field.nlon(), hard_field.nlat())
            != (regularized_field.nlon(), regularized_field.nlat())
        {
            return Err(invalid("hard and regularized HField dimensions must match"));
        }
        let hard_levels = field_levels(hard_field, base_m, max_level);
        let regularized_levels = field_levels(regularized_field, base_m, max_level);
        let config = earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(namelist_contents)
            .map_err(invalid)?;
        let domain = crate::read_method_c_domain_region(&config)?;
        let domain_support =
            crate::grid_quality_inputs::hfield_support_coverage::intended_domain_support_mask(
                regularized_field.nlon(),
                regularized_field.nlat(),
                &regularized_levels,
                domain.as_ref(),
            )?;
        let product_support = match product_support_override {
            Some(product_support) => product_support.to_vec(),
            None => crate::hfield_refine::intended_output_product_support_mask(
                regularized_field,
                &config,
            )?,
        };
        if product_support.len() != hard_levels.len() {
            return Err(invalid(format!(
                "HField product-support raster has {} values, expected {}",
                product_support.len(),
                hard_levels.len()
            )));
        }
        let intended_output_support = domain_support
            .into_iter()
            .zip(product_support.iter().copied())
            .map(|(domain, product)| domain && product)
            .collect::<Vec<_>>();
        let project_hash = canonical_project_hash(
            namelist_contents,
            hard_field.nlon(),
            hard_field.nlat(),
            base_m,
            max_level,
            g,
        )?;
        let hard_layers = hard_layers
            .iter()
            .filter_map(|layer| {
                let levels = field_levels(&layer.field, base_m, max_level);
                levels.iter().any(|level| *level != 0).then_some((
                    layer.kind,
                    layer.descriptor.to_string(),
                    levels,
                ))
            })
            .map(|(kind, descriptor, levels)| PersistedDemandLayer {
                kind: demand_source_kind_name(kind).to_string(),
                descriptor,
                levels,
            })
            .collect::<Vec<_>>();
        validate_hard_layers(&hard_layers, &hard_levels, max_level)?;
        let snapshot = build_snapshot(
            project_hash,
            hard_field.nlon(),
            hard_field.nlat(),
            base_m,
            &intended_output_support,
            &hard_layers,
        )?;
        let snapshot_hash = snapshot.snapshot_hash();
        Ok(Self {
            persisted: PersistedHfieldDemand {
                kind: ARTIFACT_KIND.to_string(),
                schema_version: ARTIFACT_SCHEMA_VERSION,
                demand_schema_version: SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION,
                output_product: "primary".to_string(),
                base_project_hash: project_hash.to_hex(),
                project_hash: project_hash.to_hex(),
                snapshot_hash: snapshot_hash.to_hex(),
                chain_tip_hash: snapshot_hash.to_hex(),
                artifact_hash: String::new(),
                gridfile_hash: String::new(),
                nlon: hard_field.nlon(),
                nlat: hard_field.nlat(),
                base_m,
                max_level,
                g,
                base_hard_levels: hard_levels.clone(),
                hard_levels,
                regularized_levels,
                base_intended_output_support: intended_output_support.clone(),
                intended_output_support,
                product_support,
                hard_layers,
                epochs: Vec::new(),
            },
        })
    }

    /// Write the demand snapshot to an explicit path, without a gridfile.
    ///
    /// The demand is composed before Method-C materializes anything, so it
    /// exists even when the run later fails. Anchoring the artifact to a
    /// gridfile means exactly the runs worth diagnosing leave nothing behind:
    /// a failed pass writes no gridfile, and a run with no demand writes an
    /// artifact full of zeros. `gridfile_hash` is left empty because no
    /// gridfile was produced.
    pub(crate) fn persist_to_path(&self, path: &Path) -> io::Result<PathBuf> {
        let mut persisted = self.persisted.clone();
        persisted.gridfile_hash = String::new();
        persisted.artifact_hash = artifact_hash(&persisted)?.to_hex();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_vec_pretty(&persisted)?)?;
        Ok(path.to_path_buf())
    }

    pub(crate) fn persist_for_gridfile(&self, gridfile: &Path) -> io::Result<PathBuf> {
        let mut persisted = self.persisted.clone();
        persisted.gridfile_hash = earthmesh_project::file_content_hash(gridfile)?;
        persisted.artifact_hash = artifact_hash(&persisted)?.to_hex();
        write_persisted_atomically(gridfile, &persisted)
    }

    #[cfg(test)]
    pub(crate) fn hard_center_demand_for_gridfile(
        &self,
        gridfile: &Path,
        kind: &str,
    ) -> io::Result<Vec<bool>> {
        self.hard_center_demand_for_gridfile_with_product_support(gridfile, kind, None)
    }

    pub(crate) fn hard_center_demand_for_product_gridfile(
        &self,
        gridfile: &Path,
        kind: &str,
        product_support_override: &[bool],
    ) -> io::Result<Vec<bool>> {
        self.hard_center_demand_for_gridfile_with_product_support(
            gridfile,
            kind,
            Some(product_support_override),
        )
    }

    fn hard_center_demand_for_gridfile_with_product_support(
        &self,
        gridfile: &Path,
        kind: &str,
        product_support_override: Option<&[bool]>,
    ) -> io::Result<Vec<bool>> {
        let expected = self
            .persisted
            .nlon
            .checked_mul(self.persisted.nlat)
            .ok_or_else(|| invalid("source-demand raster dimensions overflow usize"))?;
        let intended_output_support =
            if let Some(product_support_override) = product_support_override {
                if product_support_override.len() != expected {
                    return Err(invalid(format!(
                        "product-specific support has {} bins, expected {expected}",
                        product_support_override.len()
                    )));
                }
                self.persisted
                    .base_intended_output_support
                    .iter()
                    .zip(product_support_override)
                    .map(|(domain_support, product_support)| *domain_support && *product_support)
                    .collect::<Vec<_>>()
            } else {
                self.persisted.intended_output_support.clone()
            };
        let mesh = crate::grid_quality_inputs::read_gridfile_mesh_points(gridfile)?;
        let input = match kind.trim().to_ascii_lowercase().as_str() {
            "tri" => crate::grid_quality_inputs::quality_input_from_gridfile(&mesh)?,
            "hex" => crate::grid_quality_inputs::quality_input_from_gridfile_hex(&mesh)?,
            other => {
                return Err(invalid(format!(
                    "hard source-demand projection supports tri or hex view, got {other}"
                )))
            }
        };
        let (levels, _) =
            crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
                &input,
                self.persisted.nlon,
                self.persisted.nlat,
                &self.persisted.hard_levels,
                &self.persisted.hard_levels,
                &intended_output_support,
            )?;
        Ok(levels.into_iter().map(|level| level != 0).collect())
    }

    pub(crate) fn persist_for_product_gridfile(
        &self,
        gridfile: &Path,
        product_kind: HfieldDemandProductKind,
        cell_view: &str,
        product_support_override: &[bool],
    ) -> io::Result<PathBuf> {
        let mut persisted = self.persisted.clone();
        if !persisted.epochs.is_empty() || persisted.hard_levels != persisted.base_hard_levels {
            return Err(invalid(
                "product-specific source demand must be derived from an unmodified base snapshot",
            ));
        }
        if product_support_override.len() != persisted.product_support.len() {
            return Err(invalid(format!(
                "product-specific support has {} bins, expected {}",
                product_support_override.len(),
                persisted.product_support.len()
            )));
        }
        persisted.output_product = product_kind.name().to_string();
        let intended_output_support = persisted
            .base_intended_output_support
            .iter()
            .zip(product_support_override)
            .map(|(domain_support, product_support)| *domain_support && *product_support)
            .collect::<Vec<_>>();
        let mesh = crate::grid_quality_inputs::read_gridfile_mesh_points(gridfile)?;
        let input = match cell_view.trim() {
            "tri" => crate::grid_quality_inputs::quality_input_from_gridfile(&mesh)?,
            "hex" => crate::grid_quality_inputs::quality_input_from_gridfile_hex(&mesh)?,
            other => {
                return Err(invalid(format!(
                    "product-specific source demand supports tri or hex view, got {other}"
                )))
            }
        };
        let support_levels = intended_output_support
            .iter()
            .map(|supported| u8::from(*supported))
            .collect::<Vec<_>>();
        let (_, coverage) =
            crate::grid_quality_inputs::hfield_support_coverage::target_levels_with_hard_coverage(
                &input,
                persisted.nlon,
                persisted.nlat,
                &support_levels,
                &support_levels,
                &intended_output_support,
            )?;
        persisted.product_support = coverage.covered_bins;
        persisted.intended_output_support = persisted
            .base_intended_output_support
            .iter()
            .zip(&persisted.product_support)
            .map(|(domain_support, product_support)| *domain_support && *product_support)
            .collect();
        persisted.base_intended_output_support = persisted.intended_output_support.clone();
        let snapshot = build_snapshot(
            parse_hash(&persisted.base_project_hash)?,
            persisted.nlon,
            persisted.nlat,
            persisted.base_m,
            &persisted.base_intended_output_support,
            &persisted.hard_layers,
        )?;
        persisted.snapshot_hash = snapshot.snapshot_hash().to_hex();
        persisted.chain_tip_hash = persisted.snapshot_hash.clone();
        persisted.gridfile_hash = earthmesh_project::file_content_hash(gridfile)?;
        persisted.artifact_hash = artifact_hash(&persisted)?.to_hex();
        write_persisted_atomically(gridfile, &persisted)
    }
}

#[doc(hidden)]
pub fn persist_hfield_source_demand_for_gridfile(
    gridfile: impl AsRef<Path>,
    field: &HField,
    base_m: f64,
    max_level: u8,
    g: f64,
    namelist_contents: &str,
) -> io::Result<PathBuf> {
    PreparedHfieldDemand::capture(field, base_m, max_level, g, namelist_contents)?
        .persist_for_gridfile(gridfile.as_ref())
}

pub fn prepare_auto_refine_demand_epoch(
    source_namelist: impl AsRef<Path>,
    target_cells_geojson: impl AsRef<Path>,
    target_levels_json: impl AsRef<Path>,
) -> Result<PreparedAutoRefineDemandEpoch, AutoRefineDemandEpochError> {
    let contents = fs::read_to_string(source_namelist.as_ref())?;
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(&contents).map_err(invalid)?;
    let options = read_hfield_refine_options(&contents)?;
    let base_m = options
        .as_ref()
        .and_then(|options| options.base_m)
        .unwrap_or_else(|| {
            2.0 * std::f64::consts::PI * earthmesh_hfield::EARTH_RADIUS_METERS
                / (5.0 * f64::from(config.nxp))
        });
    let g = options.as_ref().map_or(0.2, |options| options.g);
    let nlon = options.as_ref().map_or(720, |options| options.nlon);
    let nlat = options.as_ref().map_or(360, |options| options.nlat);
    let domain = crate::read_method_c_domain_region(&config)?;
    let target_cells_geojson = target_cells_geojson.as_ref();
    let target_levels_json = target_levels_json.as_ref();
    if !crate::hydro_refinement_adapter::hydro_target_plan_has_positive_level(target_levels_json)? {
        return Err(AutoRefineDemandEpochError::MissingImmutableAbsoluteTarget);
    }
    let cells_hash = earthmesh_project::file_content_hash(target_cells_geojson)?;
    let levels_hash = earthmesh_project::file_content_hash(target_levels_json)?;
    let target = crate::hydro_refinement_adapter::load_hydro_target_field_in_domain(
        target_cells_geojson,
        target_levels_json,
        base_m,
        g,
        nlon,
        nlat,
        domain.as_ref(),
    )?;
    if cells_hash != earthmesh_project::file_content_hash(target_cells_geojson)?
        || levels_hash != earthmesh_project::file_content_hash(target_levels_json)?
    {
        return Err(AutoRefineDemandEpochError::Artifact(invalid(
            "AutoRefine absolute target changed while its immutable epoch was captured",
        )));
    }
    if target.summary.refined_rows == 0
        || !target
            .hard_field
            .values()
            .iter()
            .any(|value| *value < base_m)
    {
        return Err(AutoRefineDemandEpochError::MissingImmutableAbsoluteTarget);
    }
    Ok(PreparedAutoRefineDemandEpoch {
        descriptor: format!("quality-repair-absolute-target-v1:{cells_hash}:{levels_hash}"),
        hard_field: target.hard_field,
    })
}

pub fn publish_accepted_auto_refine_demand_epoch(
    baseline_gridfile: impl AsRef<Path>,
    baseline_namelist: impl AsRef<Path>,
    candidate_gridfile: impl AsRef<Path>,
    candidate_namelist: impl AsRef<Path>,
    prepared_epoch: PreparedAutoRefineDemandEpoch,
) -> Result<PathBuf, AutoRefineDemandEpochError> {
    publish_selected_candidate(
        baseline_gridfile.as_ref(),
        baseline_namelist.as_ref(),
        candidate_gridfile.as_ref(),
        candidate_namelist.as_ref(),
        Some(prepared_epoch),
    )
}

pub fn publish_accepted_gradation_retry_demand(
    baseline_gridfile: impl AsRef<Path>,
    baseline_namelist: impl AsRef<Path>,
    candidate_gridfile: impl AsRef<Path>,
    candidate_namelist: impl AsRef<Path>,
) -> Result<PathBuf, AutoRefineDemandEpochError> {
    publish_selected_candidate(
        baseline_gridfile.as_ref(),
        baseline_namelist.as_ref(),
        candidate_gridfile.as_ref(),
        candidate_namelist.as_ref(),
        None,
    )
}

fn publish_selected_candidate(
    baseline_gridfile: &Path,
    baseline_namelist: &Path,
    candidate_gridfile: &Path,
    candidate_namelist: &Path,
    prepared_epoch: Option<PreparedAutoRefineDemandEpoch>,
) -> Result<PathBuf, AutoRefineDemandEpochError> {
    let baseline_contents = fs::read_to_string(baseline_namelist)?;
    let candidate_contents = fs::read_to_string(candidate_namelist)?;
    let baseline = read_validated_persisted(baseline_gridfile, &baseline_contents)?;
    let candidate = read_validated_persisted(candidate_gridfile, &candidate_contents)?;
    if (baseline.nlon, baseline.nlat, baseline.base_m.to_bits())
        != (candidate.nlon, candidate.nlat, candidate.base_m.to_bits())
        || candidate.max_level < baseline.max_level
        || (prepared_epoch.is_none() && candidate.max_level != baseline.max_level)
    {
        return Err(AutoRefineDemandEpochError::Artifact(invalid(
            "AutoRefine candidate changed the immutable HField raster contract",
        )));
    }

    let base_snapshot = build_snapshot(
        parse_hash(&baseline.base_project_hash)?,
        baseline.nlon,
        baseline.nlat,
        baseline.base_m,
        &baseline.base_intended_output_support,
        &baseline.hard_layers,
    )?;
    let mut chain = rebuild_epoch_chain(&baseline, base_snapshot)?;
    let mut epochs = baseline.epochs.clone();
    let mut effective_hard_levels = baseline.hard_levels.clone();

    if let Some(prepared_epoch) = prepared_epoch {
        if (
            prepared_epoch.hard_field.nlon(),
            prepared_epoch.hard_field.nlat(),
        ) != (candidate.nlon, candidate.nlat)
        {
            return Err(AutoRefineDemandEpochError::Artifact(invalid(
                "AutoRefine absolute target dimensions do not match the candidate HField",
            )));
        }
        let levels = field_levels(
            &prepared_epoch.hard_field,
            candidate.base_m,
            candidate.max_level,
        );
        if !levels.iter().any(|level| *level != 0) {
            return Err(AutoRefineDemandEpochError::MissingImmutableAbsoluteTarget);
        }
        let hard_layers = vec![PersistedDemandLayer {
            kind: demand_source_kind_name(DemandSourceKind::AutoRefine).to_string(),
            descriptor: prepared_epoch.descriptor,
            levels,
        }];
        validate_hard_layers(&hard_layers, &hard_layers[0].levels, candidate.max_level)?;
        let sources = build_sources(
            candidate.nlon,
            candidate.nlat,
            candidate.base_m,
            &candidate.intended_output_support,
            &hard_layers,
        )?;
        let epoch = chain.append_epoch(sources, &demand_hash)?.clone();
        for layer in &hard_layers {
            for (effective, level) in effective_hard_levels.iter_mut().zip(&layer.levels) {
                *effective = (*effective).max(*level);
            }
        }
        epochs.push(PersistedDemandEpoch {
            epoch_id: epoch.epoch_id(),
            parent_snapshot_hash: epoch.parent_snapshot_hash().to_hex(),
            demand_hash: epoch.demand_hash().to_hex(),
            epoch_hash: epoch.epoch_hash().to_hex(),
            intended_output_support: candidate.intended_output_support.clone(),
            hard_layers,
        });
    }

    if effective_hard_levels != candidate.hard_levels {
        return Err(AutoRefineDemandEpochError::Artifact(invalid(
            "selected candidate hard demand is not exactly base demand plus accepted AutoRefine epochs",
        )));
    }

    let mut selected = candidate;
    selected.base_project_hash = baseline.base_project_hash;
    selected.snapshot_hash = baseline.snapshot_hash;
    selected.chain_tip_hash = chain.tip_hash().to_hex();
    selected.base_hard_levels = baseline.base_hard_levels;
    selected.base_intended_output_support = baseline.base_intended_output_support;
    selected.hard_layers = baseline.hard_layers;
    selected.epochs = epochs;
    selected.hard_levels = effective_hard_levels;
    selected.gridfile_hash = earthmesh_project::file_content_hash(candidate_gridfile)?;
    selected.artifact_hash = artifact_hash(&selected)?.to_hex();
    write_persisted_atomically(candidate_gridfile, &selected).map_err(Into::into)
}

pub(crate) fn load_hfield_source_demand(
    gridfile: &Path,
    namelist_contents: &str,
) -> io::Result<LoadedHfieldDemand> {
    let persisted = read_validated_persisted(gridfile, namelist_contents)?;
    Ok(LoadedHfieldDemand {
        snapshot_hash: parse_hash(&persisted.snapshot_hash)?,
        chain_tip_hash: parse_hash(&persisted.chain_tip_hash)?,
        nlon: persisted.nlon,
        nlat: persisted.nlat,
        hard_levels: persisted.hard_levels,
        intended_output_support: persisted.intended_output_support,
        base_m: persisted.base_m,
        max_level: persisted.max_level,
        g: persisted.g,
    })
}

fn read_validated_persisted(
    gridfile: &Path,
    namelist_contents: &str,
) -> io::Result<PersistedHfieldDemand> {
    let path = source_demand_artifact_path(gridfile)?;
    let bytes = fs::read(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "missing immutable source-demand snapshot {}; refusing to rebuild HField sources during quality evaluation",
                    path.display()
                ),
            )
        } else {
            error
        }
    })?;
    let persisted: PersistedHfieldDemand =
        serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))?;
    if persisted.kind != ARTIFACT_KIND
        || persisted.schema_version != ARTIFACT_SCHEMA_VERSION
        || persisted.demand_schema_version != SOURCE_DEMAND_SNAPSHOT_SCHEMA_VERSION
        || !matches!(
            persisted.output_product.as_str(),
            "primary" | "land" | "ocean"
        )
    {
        return Err(invalid(format!(
            "unsupported source-demand snapshot schema in {}",
            path.display()
        )));
    }
    validate_config(persisted.base_m, persisted.max_level, persisted.g)?;
    let expected_len = persisted
        .nlon
        .checked_mul(persisted.nlat)
        .ok_or_else(|| invalid("source-demand raster dimensions overflow usize"))?;
    if persisted.base_hard_levels.len() != expected_len
        || persisted.hard_levels.len() != expected_len
        || persisted.regularized_levels.len() != expected_len
        || persisted.base_intended_output_support.len() != expected_len
        || persisted.intended_output_support.len() != expected_len
        || persisted.product_support.len() != expected_len
        || persisted
            .base_hard_levels
            .iter()
            .any(|level| *level > persisted.max_level)
        || persisted
            .hard_levels
            .iter()
            .any(|level| *level > persisted.max_level)
        || persisted
            .regularized_levels
            .iter()
            .any(|level| *level > persisted.max_level)
    {
        return Err(invalid(format!(
            "invalid hard-demand raster in {}",
            path.display()
        )));
    }
    validate_hard_layers(
        &persisted.hard_layers,
        &persisted.base_hard_levels,
        persisted.max_level,
    )?;

    let project_hash = parse_hash(&persisted.project_hash)?;
    let expected_project_hash = canonical_project_hash(
        namelist_contents,
        persisted.nlon,
        persisted.nlat,
        persisted.base_m,
        persisted.max_level,
        persisted.g,
    )?;
    if project_hash != expected_project_hash {
        return Err(invalid(format!(
            "source-demand snapshot {} does not match the quality namelist",
            path.display()
        )));
    }
    let base_project_hash = parse_hash(&persisted.base_project_hash)?;
    let snapshot = build_snapshot(
        base_project_hash,
        persisted.nlon,
        persisted.nlat,
        persisted.base_m,
        &persisted.base_intended_output_support,
        &persisted.hard_layers,
    )?;
    let recorded_snapshot_hash = parse_hash(&persisted.snapshot_hash)?;
    if snapshot.snapshot_hash() != recorded_snapshot_hash {
        return Err(invalid(format!(
            "source-demand snapshot hash mismatch in {}",
            path.display()
        )));
    }
    let chain = rebuild_epoch_chain(&persisted, snapshot)?;
    if chain.tip_hash() != parse_hash(&persisted.chain_tip_hash)? {
        return Err(invalid(format!(
            "source-demand epoch chain tip mismatch in {}",
            path.display()
        )));
    }
    let reconstructed_hard = reconstruct_effective_hard_levels(
        &persisted.base_hard_levels,
        &persisted.epochs,
        persisted.max_level,
    )?;
    if reconstructed_hard != persisted.hard_levels {
        return Err(invalid(format!(
            "source-demand epochs do not reconstruct the effective hard raster in {}",
            path.display()
        )));
    }
    if artifact_hash(&persisted)? != parse_hash(&persisted.artifact_hash)? {
        return Err(invalid(format!(
            "source-demand artifact hash mismatch in {}",
            path.display()
        )));
    }
    if earthmesh_project::file_content_hash(gridfile)? != persisted.gridfile_hash {
        return Err(invalid(format!(
            "source-demand snapshot {} is bound to a different final gridfile",
            path.display()
        )));
    }

    Ok(persisted)
}

pub(crate) fn source_demand_artifact_path(gridfile: &Path) -> io::Result<PathBuf> {
    let mut name = gridfile
        .file_name()
        .map(OsString::from)
        .ok_or_else(|| invalid("gridfile path has no file name"))?;
    name.push(".source-demand.json");
    Ok(gridfile.with_file_name(name))
}

fn write_persisted_atomically(
    gridfile: &Path,
    persisted: &PersistedHfieldDemand,
) -> io::Result<PathBuf> {
    let path = source_demand_artifact_path(gridfile)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(persisted).map_err(io::Error::other)?;
    let mut temp_name = path
        .file_name()
        .map(OsString::from)
        .ok_or_else(|| invalid("source-demand artifact path has no file name"))?;
    temp_name.push(format!(".{}.tmp", std::process::id()));
    let temp = path.with_file_name(temp_name);
    fs::write(&temp, bytes)?;
    if let Err(error) = fs::rename(&temp, &path) {
        let _ = fs::remove_file(&temp);
        return Err(error);
    }
    Ok(path)
}

fn field_levels(field: &HField, base_m: f64, max_level: u8) -> Vec<u8> {
    let mut levels = Vec::with_capacity(field.nlon() * field.nlat());
    for j in 0..field.nlat() {
        for i in 0..field.nlon() {
            levels.push(field.topology_level_at(
                field.lon_center(i),
                field.lat_center(j),
                base_m,
                max_level,
            ));
        }
    }
    levels
}

fn build_snapshot(
    project_hash: DemandHash,
    nlon: usize,
    nlat: usize,
    base_m: f64,
    intended_output_support: &[bool],
    hard_layers: &[PersistedDemandLayer],
) -> io::Result<SourceDemandSnapshot> {
    let expected = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("source-demand raster dimensions overflow usize"))?;
    if intended_output_support.len() != expected {
        return Err(invalid(format!(
            "HField intended-support raster has {} values, expected {expected}",
            intended_output_support.len()
        )));
    }
    let sources = build_sources(nlon, nlat, base_m, intended_output_support, hard_layers)?;
    SourceDemandSnapshot::new(project_hash, sources, &demand_hash)
        .map_err(|error| invalid(error.to_string()))
}

fn build_sources(
    nlon: usize,
    nlat: usize,
    base_m: f64,
    intended_output_support: &[bool],
    hard_layers: &[PersistedDemandLayer],
) -> io::Result<Vec<DemandSource>> {
    let mut sources = Vec::with_capacity(hard_layers.len());
    for layer in hard_layers {
        let kind = parse_demand_source_kind(&layer.kind)?;
        let mut support_raster = encode_level_raster(nlon, nlat, base_m, &layer.levels, None)?;
        support_raster.extend_from_slice(layer.descriptor.as_bytes());
        let support = DemandSupport::new(DemandSupportKind::Raster, support_raster)
            .map_err(|error| invalid(error.to_string()))?;
        let domain_intersection = DemandSupport::new(
            DemandSupportKind::Raster,
            encode_level_raster(
                nlon,
                nlat,
                base_m,
                &layer.levels,
                Some(intended_output_support),
            )?,
        )
        .map_err(|error| invalid(error.to_string()))?;
        let requested_level = layer.levels.iter().copied().max().unwrap_or(0);
        let component = DemandComponent::new(
            DemandStrength::Hard,
            support,
            domain_intersection,
            requested_level,
            base_m / 2.0_f64.powi(i32::from(requested_level)),
            &demand_hash,
        )
        .map_err(|error| invalid(error.to_string()))?;
        sources.push(
            DemandSource::new(
                kind,
                layer.descriptor.as_bytes().to_vec(),
                vec![component],
                &demand_hash,
            )
            .map_err(|error| invalid(error.to_string()))?,
        );
    }
    Ok(sources)
}

fn encode_level_raster(
    nlon: usize,
    nlat: usize,
    base_m: f64,
    levels: &[u8],
    mask: Option<&[bool]>,
) -> io::Result<Vec<u8>> {
    let expected = nlon
        .checked_mul(nlat)
        .ok_or_else(|| invalid("source-demand raster dimensions overflow usize"))?;
    if levels.len() != expected || mask.is_some_and(|mask| mask.len() != expected) {
        return Err(invalid("source-demand raster shape mismatch"));
    }
    let mut raster = Vec::with_capacity(HARD_RASTER_TAG.len() + 32 + levels.len());
    raster.extend_from_slice(HARD_RASTER_TAG);
    raster.extend_from_slice(&(nlon as u64).to_le_bytes());
    raster.extend_from_slice(&(nlat as u64).to_le_bytes());
    raster.extend_from_slice(&base_m.to_bits().to_le_bytes());
    raster.extend_from_slice(&(levels.len() as u64).to_le_bytes());
    raster.extend(levels.iter().enumerate().map(|(index, level)| {
        if mask.is_none_or(|mask| mask[index]) {
            *level
        } else {
            0
        }
    }));
    Ok(raster)
}

fn demand_source_kind_name(kind: DemandSourceKind) -> &'static str {
    match kind {
        DemandSourceKind::Specified => "specified",
        DemandSourceKind::Threshold => "threshold",
        DemandSourceKind::Landcover => "landcover",
        DemandSourceKind::Hydro => "hydro",
        DemandSourceKind::AutoRefine => "auto_refine",
    }
}

fn parse_demand_source_kind(kind: &str) -> io::Result<DemandSourceKind> {
    match kind {
        "specified" => Ok(DemandSourceKind::Specified),
        "threshold" => Ok(DemandSourceKind::Threshold),
        "landcover" => Ok(DemandSourceKind::Landcover),
        "hydro" => Ok(DemandSourceKind::Hydro),
        "auto_refine" => Ok(DemandSourceKind::AutoRefine),
        other => Err(invalid(format!("unknown source-demand layer kind {other}"))),
    }
}

fn validate_hard_layers(
    layers: &[PersistedDemandLayer],
    hard_levels: &[u8],
    max_level: u8,
) -> io::Result<()> {
    let mut combined = vec![0_u8; hard_levels.len()];
    for layer in layers {
        parse_demand_source_kind(&layer.kind)?;
        if layer.descriptor.is_empty()
            || layer.levels.len() != hard_levels.len()
            || layer.levels.iter().any(|level| *level > max_level)
            || !layer.levels.iter().any(|level| *level != 0)
        {
            return Err(invalid("invalid source-demand hard layer"));
        }
        for (combined, level) in combined.iter_mut().zip(&layer.levels) {
            *combined = (*combined).max(*level);
        }
    }
    if combined != hard_levels {
        return Err(invalid(
            "source-demand hard layers do not reconstruct the hard raster",
        ));
    }
    Ok(())
}

fn rebuild_epoch_chain(
    persisted: &PersistedHfieldDemand,
    snapshot: SourceDemandSnapshot,
) -> io::Result<DemandEpochChain> {
    let expected = persisted
        .nlon
        .checked_mul(persisted.nlat)
        .ok_or_else(|| invalid("source-demand raster dimensions overflow usize"))?;
    let mut chain = DemandEpochChain::new(snapshot);
    for recorded in &persisted.epochs {
        if recorded.intended_output_support.len() != expected
            || recorded
                .hard_layers
                .iter()
                .any(|layer| layer.kind != "auto_refine")
        {
            return Err(invalid("invalid AutoRefine demand epoch"));
        }
        let mut epoch_levels = vec![0_u8; expected];
        for layer in &recorded.hard_layers {
            if layer.levels.len() != expected {
                return Err(invalid("invalid AutoRefine demand epoch raster shape"));
            }
            for (combined, level) in epoch_levels.iter_mut().zip(&layer.levels) {
                *combined = (*combined).max(*level);
            }
        }
        validate_hard_layers(&recorded.hard_layers, &epoch_levels, persisted.max_level)?;
        let sources = build_sources(
            persisted.nlon,
            persisted.nlat,
            persisted.base_m,
            &recorded.intended_output_support,
            &recorded.hard_layers,
        )?;
        let epoch = chain
            .append_epoch(sources, &demand_hash)
            .map_err(|error| invalid(error.to_string()))?;
        if epoch.epoch_id() != recorded.epoch_id
            || epoch.parent_snapshot_hash() != parse_hash(&recorded.parent_snapshot_hash)?
            || epoch.demand_hash() != parse_hash(&recorded.demand_hash)?
            || epoch.epoch_hash() != parse_hash(&recorded.epoch_hash)?
        {
            return Err(invalid("AutoRefine demand epoch chain hash mismatch"));
        }
    }
    Ok(chain)
}

fn reconstruct_effective_hard_levels(
    base_hard_levels: &[u8],
    epochs: &[PersistedDemandEpoch],
    max_level: u8,
) -> io::Result<Vec<u8>> {
    let mut effective = base_hard_levels.to_vec();
    for epoch in epochs {
        for layer in &epoch.hard_layers {
            if layer.levels.len() != effective.len()
                || layer.levels.iter().any(|level| *level > max_level)
            {
                return Err(invalid("invalid AutoRefine demand epoch hard raster"));
            }
            for (combined, level) in effective.iter_mut().zip(&layer.levels) {
                *combined = (*combined).max(*level);
            }
        }
    }
    Ok(effective)
}

fn artifact_hash(persisted: &PersistedHfieldDemand) -> io::Result<DemandHash> {
    let mut canonical = ARTIFACT_HASH_TAG.to_vec();
    push_bytes(&mut canonical, persisted.kind.as_bytes());
    canonical.extend_from_slice(&persisted.schema_version.to_le_bytes());
    canonical.extend_from_slice(&persisted.demand_schema_version.to_le_bytes());
    push_bytes(&mut canonical, persisted.output_product.as_bytes());
    push_bytes(&mut canonical, persisted.base_project_hash.as_bytes());
    push_bytes(&mut canonical, persisted.project_hash.as_bytes());
    push_bytes(&mut canonical, persisted.snapshot_hash.as_bytes());
    push_bytes(&mut canonical, persisted.chain_tip_hash.as_bytes());
    push_bytes(&mut canonical, persisted.gridfile_hash.as_bytes());
    canonical.extend_from_slice(&(persisted.nlon as u64).to_le_bytes());
    canonical.extend_from_slice(&(persisted.nlat as u64).to_le_bytes());
    canonical.extend_from_slice(&persisted.base_m.to_bits().to_le_bytes());
    canonical.push(persisted.max_level);
    canonical.extend_from_slice(&persisted.g.to_bits().to_le_bytes());
    push_bytes(&mut canonical, &persisted.base_hard_levels);
    push_bytes(&mut canonical, &persisted.hard_levels);
    push_bytes(&mut canonical, &persisted.regularized_levels);
    canonical.extend(
        persisted
            .base_intended_output_support
            .iter()
            .map(|value| u8::from(*value)),
    );
    canonical.extend(
        persisted
            .intended_output_support
            .iter()
            .map(|value| u8::from(*value)),
    );
    canonical.extend(
        persisted
            .product_support
            .iter()
            .map(|value| u8::from(*value)),
    );
    let layers = serde_json::to_vec(&persisted.hard_layers).map_err(io::Error::other)?;
    push_bytes(&mut canonical, &layers);
    let epochs = serde_json::to_vec(&persisted.epochs).map_err(io::Error::other)?;
    push_bytes(&mut canonical, &epochs);
    Ok(demand_hash(&canonical))
}

fn demand_hash(bytes: &[u8]) -> DemandHash {
    let hex = earthmesh_project::content_addressed_stage_key(
        "source-demand-sha256-v1",
        &[("canonical", bytes)],
    );
    parse_hash(&hex).expect("stage-cache keys are 64-character SHA-256 hex")
}

fn canonical_project_hash(
    namelist_contents: &str,
    nlon: usize,
    nlat: usize,
    base_m: f64,
    max_level: u8,
    g: f64,
) -> io::Result<DemandHash> {
    let config =
        earthmesh_core::EarthmeshConfig::from_mkgrd_namelist(namelist_contents).map_err(invalid)?;
    let options = read_hfield_refine_options(namelist_contents)?
        .ok_or_else(|| invalid("source-demand snapshot requires an enabled HField"))?;
    let refine = earthmesh_core::RefineConfig::from_mkrefine_namelist_with_external_field(
        namelist_contents,
        config.mesh_type.trim(),
        config.mode_grid.trim(),
        options.hydro_target_paths().is_some(),
    )
    .or_else(|_| {
        crate::read_native_grid_refine_controls(namelist_contents)
            .map_err(|error| error.to_string())
    })
    .map_err(invalid)?;
    if options.nlon != nlon
        || options.nlat != nlat
        || options.g.to_bits() != g.to_bits()
        || options
            .base_m
            .is_some_and(|configured| configured.to_bits() != base_m.to_bits())
        || options
            .max_level
            .is_some_and(|configured| configured != usize::from(max_level))
    {
        return Err(invalid(
            "source-demand snapshot controls do not match the effective HField",
        ));
    }

    let mut canonical = b"earthmesh-source-demand-project-binding-v1\0".to_vec();
    canonical.extend_from_slice(&config.nxp.to_le_bytes());
    push_bytes(
        &mut canonical,
        config.mesh_type.trim().to_ascii_lowercase().as_bytes(),
    );
    push_bytes(
        &mut canonical,
        config.mode_grid.trim().to_ascii_lowercase().as_bytes(),
    );
    push_bytes(
        &mut canonical,
        config.output_format.trim().to_ascii_lowercase().as_bytes(),
    );
    canonical.push(u8::from(config.mask_domain_global));
    encode_grid_region(
        &mut canonical,
        crate::read_method_c_domain_region(&config)?.as_ref(),
    );
    match read_native_grid_mdomain(namelist_contents)? {
        Some(mdomain) => {
            canonical.push(1);
            canonical.extend_from_slice(&(mdomain as u64).to_le_bytes());
        }
        None => canonical.push(0),
    }
    canonical.extend_from_slice(&(nlon as u64).to_le_bytes());
    canonical.extend_from_slice(&(nlat as u64).to_le_bytes());
    canonical.extend_from_slice(&base_m.to_bits().to_le_bytes());
    canonical.push(max_level);
    canonical.extend_from_slice(&g.to_bits().to_le_bytes());
    match options.geographic_origin {
        Some((lon, lat)) => {
            canonical.push(1);
            canonical.extend_from_slice(&lon.to_bits().to_le_bytes());
            canonical.extend_from_slice(&lat.to_bits().to_le_bytes());
        }
        None => canonical.push(0),
    }
    encode_optional_input_content(
        &mut canonical,
        crate::landtype_file_is_real(&config.landtype_file)
            .then_some(Path::new(config.landtype_file.trim())),
    )?;
    encode_optional_input_content(
        &mut canonical,
        options
            .target_cells_geojson
            .as_deref()
            .map(Path::new)
            .filter(|path| path.is_file()),
    )?;
    encode_optional_input_content(
        &mut canonical,
        options
            .target_levels_json
            .as_deref()
            .map(Path::new)
            .filter(|path| path.is_file()),
    )?;
    let threshold_paths =
        crate::hfield_refine::threshold_hfield_source_paths(&refine, config.mesh_type.trim());
    canonical.extend_from_slice(&(threshold_paths.len() as u64).to_le_bytes());
    for path in threshold_paths {
        encode_optional_input_content(&mut canonical, Some(&path))?;
    }
    Ok(demand_hash(&canonical))
}

fn encode_optional_input_content(output: &mut Vec<u8>, path: Option<&Path>) -> io::Result<()> {
    let Some(path) = path.filter(|path| path.is_file()) else {
        output.push(0);
        return Ok(());
    };
    output.push(1);
    push_bytes(
        output,
        earthmesh_project::file_content_hash(path)?.as_bytes(),
    );
    Ok(())
}

fn encode_grid_region(output: &mut Vec<u8>, region: Option<&crate::GridRegion>) {
    match region {
        None => output.push(0),
        Some(crate::GridRegion::Bbox {
            west,
            east,
            north,
            south,
        }) => {
            output.push(1);
            for value in [west, east, north, south] {
                output.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        Some(crate::GridRegion::Circle {
            lon,
            lat,
            radius_km,
        }) => {
            output.push(2);
            for value in [lon, lat, radius_km] {
                output.extend_from_slice(&value.to_bits().to_le_bytes());
            }
        }
        Some(crate::GridRegion::Close { points }) => {
            output.push(3);
            output.extend_from_slice(&(points.len() as u64).to_le_bytes());
            for point in points {
                output.extend_from_slice(&point.lon.to_bits().to_le_bytes());
                output.extend_from_slice(&point.lat.to_bits().to_le_bytes());
            }
        }
        Some(crate::GridRegion::Any(regions)) => {
            output.push(4);
            output.extend_from_slice(&(regions.len() as u64).to_le_bytes());
            for region in regions {
                encode_grid_region(output, Some(region));
            }
        }
    }
}

fn push_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn parse_hash(hex: &str) -> io::Result<DemandHash> {
    if hex.len() != 64 {
        return Err(invalid("source-demand hash must contain 64 hex digits"));
    }
    let mut bytes = [0_u8; 32];
    for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(pair)
            .map_err(|_| invalid("source-demand hash is not valid UTF-8"))?;
        bytes[index] = u8::from_str_radix(text, 16)
            .map_err(|_| invalid("source-demand hash contains a non-hex digit"))?;
    }
    Ok(DemandHash::from_bytes(bytes))
}

fn validate_config(base_m: f64, max_level: u8, g: f64) -> io::Result<()> {
    if !base_m.is_finite() || base_m <= 0.0 {
        return Err(invalid("source-demand base_m must be positive and finite"));
    }
    if !g.is_finite() || g <= 0.0 {
        return Err(invalid("source-demand g must be positive and finite"));
    }
    if max_level == 0 {
        return Err(invalid("source-demand max_level must be positive"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_gridfile(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "earthmesh_source_demand_{name}_{}_{}.nc4",
            std::process::id(),
            stamp
        ));
        fs::write(&path, b"final-gridfile").unwrap();
        path
    }

    fn test_namelist(g: f64) -> String {
        format!(
            "&mkgrd
 NL%NXP=6
 NL%mesh_type='atmosmesh'
 NL%mode_grid='hex'
 NL%output_format='MPAS'
 NL%mask_domain_global=.true.
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=4
 NL%hfield_nlat=2
 NL%hfield_base_m=100.0
 NL%hfield_max_level=2
 NL%hfield_g={g}
/
"
        )
    }

    fn write_test_namelist(name: &str, contents: &str) -> PathBuf {
        let path = temp_gridfile(name).with_extension("nml");
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn native_product_support_override_does_not_resample_landtype() {
        let field = HField::uniform(4, 2, 100.0).unwrap();
        let support = vec![true; 8];
        let namelist = "&mkgrd
 NL%NXP=6
 NL%mesh_type='landmesh'
 NL%mode_grid='hex'
 NL%output_format='CoLM'
 NL%landtype_file='/does/not/exist.nc'
 NL%mask_domain_global=.true.
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=4
 NL%hfield_nlat=2
 NL%hfield_base_m=100.0
 NL%hfield_max_level=1
 NL%hfield_g=0.2
/
";
        let prepared = PreparedHfieldDemand::capture_with_hard_sources_and_product_support(
            &field,
            &field,
            &[],
            100.0,
            1,
            0.2,
            namelist,
            Some(&support),
        )
        .unwrap();
        assert_eq!(prepared.persisted.product_support, support);
    }

    fn prepared_epoch_at(i: usize, j: usize) -> PreparedAutoRefineDemandEpoch {
        let mut hard_field = HField::uniform(4, 2, 100.0).unwrap();
        hard_field.set(i, j, 25.0);
        PreparedAutoRefineDemandEpoch {
            descriptor: "test-immutable-absolute-target".to_string(),
            hard_field,
        }
    }

    fn write_test_gridfile(
        path: &Path,
        m_lon: &[f64],
        m_lat: &[f64],
        w_lon: &[f64],
        w_lat: &[f64],
        m_to_w: &[i32],
        w_to_m: Option<(&[i32], &[i32], usize)>,
    ) {
        let _ = fs::remove_file(path);
        let mut file = crate::create_netcdf_quiet(path).unwrap();
        file.add_dimension("sjx_points", m_lon.len()).unwrap();
        file.add_dimension("lbx_points", w_lon.len()).unwrap();
        file.add_dimension("dimb", 3).unwrap();
        file.add_variable::<f64>("GLONM", &["sjx_points"])
            .unwrap()
            .put_values(m_lon, ..)
            .unwrap();
        file.add_variable::<f64>("GLATM", &["sjx_points"])
            .unwrap()
            .put_values(m_lat, ..)
            .unwrap();
        file.add_variable::<f64>("GLONW", &["lbx_points"])
            .unwrap()
            .put_values(w_lon, ..)
            .unwrap();
        file.add_variable::<f64>("GLATW", &["lbx_points"])
            .unwrap()
            .put_values(w_lat, ..)
            .unwrap();
        file.add_variable::<i32>("itab_m%iw", &["sjx_points", "dimb"])
            .unwrap()
            .put_values(m_to_w, (.., ..))
            .unwrap();
        if let Some((w_to_m, n_w, width)) = w_to_m {
            file.add_dimension("dimc", width).unwrap();
            file.add_variable::<i32>("itab_w%im", &["lbx_points", "dimc"])
                .unwrap()
                .put_values(w_to_m, (.., ..))
                .unwrap();
            file.add_variable::<i32>("n_ngrwm", &["lbx_points"])
                .unwrap()
                .put_values(n_w, ..)
                .unwrap();
        }
    }

    #[test]
    fn producer_and_quality_share_the_identical_hard_demand_snapshot() {
        let gridfile = temp_gridfile("roundtrip");
        let namelist = "&mkgrd
 NL%NXP=6
 NL%mesh_type='atmosmesh'
 NL%mode_grid='hex'
 NL%output_format='MPAS'
 NL%mask_domain_global=.true.
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=4
 NL%hfield_nlat=2
 NL%hfield_base_m=100.0
 NL%hfield_max_level=2
 NL%hfield_g=0.2
/
";
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        field.set(1, 0, 50.0);
        field.set(3, 1, 25.0);
        let prepared = PreparedHfieldDemand::capture(&field, 100.0, 2, 0.2, namelist).unwrap();
        prepared.persist_for_gridfile(&gridfile).unwrap();

        let loaded = load_hfield_source_demand(&gridfile, namelist).unwrap();
        assert_eq!(
            loaded.snapshot_hash.to_hex(),
            prepared.persisted.snapshot_hash
        );
        assert_eq!(loaded.nlon, field.nlon());
        assert_eq!(loaded.nlat, field.nlat());
        assert_eq!(loaded.hard_levels, prepared.persisted.hard_levels);
        assert_eq!(loaded.hard_levels[1], 1, "row-major (j=0,i=1)");
        assert_eq!(loaded.hard_levels[7], 2, "row-major (j=1,i=3)");
        assert_eq!(
            loaded.intended_output_support,
            prepared.persisted.intended_output_support
        );

        fs::write(&gridfile, b"different-final-gridfile").unwrap();
        let error = load_hfield_source_demand(&gridfile, namelist).unwrap_err();
        assert!(
            error.to_string().contains("different final gridfile"),
            "{error}"
        );

        let _ = fs::remove_file(source_demand_artifact_path(&gridfile).unwrap());
        let _ = fs::remove_file(gridfile);
    }

    #[test]
    fn regional_intended_support_excludes_apron_and_is_snapshot_hashed() {
        let gridfile = temp_gridfile("regional_support");
        let namelist = "&mkgrd
 NL%NXP=6
 NL%mesh_type='atmosmesh'
 NL%mode_grid='hex'
 NL%output_format='MPAS'
 NL%mask_domain_global=.false.
 NL%mask_domain_type='bbox'
 NL%mask_domain_fprefix='inline:bbox:w=-135,e=-90,s=-45,n=0'
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=8
 NL%hfield_nlat=4
 NL%hfield_base_m=100.0
 NL%hfield_max_level=1
 NL%hfield_g=0.2
/
";
        let mut field = HField::uniform(8, 4, 100.0).unwrap();
        field.set(1, 1, 50.0);
        field.set(7, 3, 50.0);
        let prepared = PreparedHfieldDemand::capture(&field, 100.0, 1, 0.2, namelist).unwrap();
        assert!(prepared.persisted.intended_output_support[1 * 8 + 1]);
        assert!(
            !prepared.persisted.intended_output_support[3 * 8 + 7],
            "hard gradient apron outside the output domain is not intended support"
        );
        prepared.persist_for_gridfile(&gridfile).unwrap();

        let artifact = source_demand_artifact_path(&gridfile).unwrap();
        let mut json: serde_json::Value =
            serde_json::from_slice(&fs::read(&artifact).unwrap()).unwrap();
        json["intended_output_support"][1 * 8 + 1] = serde_json::Value::Bool(false);
        fs::write(&artifact, serde_json::to_vec_pretty(&json).unwrap()).unwrap();

        let error = load_hfield_source_demand(&gridfile, namelist).unwrap_err();
        assert!(
            error.to_string().contains("artifact hash mismatch"),
            "{error}"
        );
        let _ = fs::remove_file(artifact);
    }

    #[test]
    fn semantic_binding_ignores_paths_and_order_but_rejects_changed_hfield_controls() {
        let gridfile = temp_gridfile("refuse");
        let first = "&mkgrd
 NL%base_dir='/tmp/first/'
 NL%EXPNME='case'
 NL%NXP=6
 NL%mesh_type='atmosmesh'
 NL%mode_grid='hex'
 NL%output_format='MPAS'
 NL%mask_domain_global=.true.
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=4
 NL%hfield_nlat=2
 NL%hfield_base_m=100.0
 NL%hfield_max_level=1
 NL%hfield_g=0.2
 NL%hfield_target_cells_geojson='/tmp/first/cells.json'
 NL%hfield_target_levels_json='/tmp/first/levels.json'
/
";
        let equivalent = "&hfield
 NL%hfield_target_levels_json = '/moved/levels.json'
 NL%hfield_g = 0.2
 NL%hfield_max_level = 1
 NL%hfield_nlat = 2
 NL%hfield_target_cells_geojson = '/moved/cells.json'
 NL%hfield_base_m = 100.0
 NL%hfield_nlon = 4
 NL%hfield_on = .true.
/
&mkgrd
 NL%mask_domain_global = .true.
 NL%mode_grid = 'hex'
 NL%mesh_type = 'atmosmesh'
 NL%output_format = 'MPAS'
 NL%NXP = 6
 NL%EXPNME = 'renamed'
 NL%base_dir = '/moved/'
/
";
        let missing = load_hfield_source_demand(&gridfile, first).unwrap_err();
        assert_eq!(missing.kind(), io::ErrorKind::NotFound);

        let field = HField::uniform(4, 2, 100.0).unwrap();
        PreparedHfieldDemand::capture(&field, 100.0, 1, 0.2, first)
            .unwrap()
            .persist_for_gridfile(&gridfile)
            .unwrap();
        load_hfield_source_demand(&gridfile, equivalent)
            .expect("paths, project location, whitespace, and field order are not semantic");

        let changed = equivalent.replace("NL%hfield_g = 0.2", "NL%hfield_g = 0.3");
        let mismatched = load_hfield_source_demand(&gridfile, &changed).unwrap_err();
        assert_eq!(mismatched.kind(), io::ErrorKind::InvalidData);
        assert!(mismatched.to_string().contains("controls do not match"));

        let changed_product = equivalent.replace("NL%mode_grid = 'hex'", "NL%mode_grid = 'tri'");
        let mismatched = load_hfield_source_demand(&gridfile, &changed_product).unwrap_err();
        assert!(
            mismatched
                .to_string()
                .contains("does not match the quality namelist"),
            "{mismatched}"
        );

        let _ = fs::remove_file(source_demand_artifact_path(&gridfile).unwrap());
        let _ = fs::remove_file(gridfile);
    }

    #[test]
    fn domain_and_landtype_content_are_part_of_the_semantic_binding() {
        let gridfile = temp_gridfile("semantic_inputs");
        let landtype = temp_gridfile("landtype_content");
        fs::write(&landtype, b"landtype-v1").unwrap();
        let namelist = format!(
            "&mkgrd
 NL%NXP=6
 NL%mesh_type='atmosmesh'
 NL%mode_grid='hex'
 NL%output_format='MPAS'
 NL%landtype_file='{}'
 NL%mask_domain_global=.false.
 NL%mask_domain_type='bbox'
 NL%mask_domain_fprefix='inline:bbox:w=-45,e=45,s=-45,n=45'
/
&hfield
 NL%hfield_on=.true.
 NL%hfield_nlon=8
 NL%hfield_nlat=4
 NL%hfield_base_m=100.0
 NL%hfield_max_level=1
 NL%hfield_g=0.2
/
",
            landtype.display()
        );
        let mut field = HField::uniform(8, 4, 100.0).unwrap();
        field.set(3, 1, 50.0);
        PreparedHfieldDemand::capture(&field, 100.0, 1, 0.2, &namelist)
            .unwrap()
            .persist_for_gridfile(&gridfile)
            .unwrap();

        let changed_bbox = namelist.replace("e=45", "e=90");
        let error = load_hfield_source_demand(&gridfile, &changed_bbox).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not match the quality namelist"),
            "{error}"
        );

        fs::write(&landtype, b"landtype-v2").unwrap();
        let error = load_hfield_source_demand(&gridfile, &namelist).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not match the quality namelist"),
            "{error}"
        );

        let mut first = Vec::new();
        encode_grid_region(
            &mut first,
            Some(&crate::GridRegion::Close {
                points: vec![
                    crate::LonLatPoint { lon: 0.0, lat: 0.0 },
                    crate::LonLatPoint { lon: 1.0, lat: 0.0 },
                    crate::LonLatPoint { lon: 0.0, lat: 1.0 },
                ],
            }),
        );
        let mut second = Vec::new();
        encode_grid_region(
            &mut second,
            Some(&crate::GridRegion::Close {
                points: vec![
                    crate::LonLatPoint { lon: 0.0, lat: 0.0 },
                    crate::LonLatPoint { lon: 2.0, lat: 0.0 },
                    crate::LonLatPoint { lon: 0.0, lat: 1.0 },
                ],
            }),
        );
        assert_ne!(demand_hash(&first), demand_hash(&second));

        let _ = fs::remove_file(source_demand_artifact_path(&gridfile).unwrap());
        let _ = fs::remove_file(gridfile);
        let _ = fs::remove_file(landtype);
    }

    #[test]
    fn product_gridfiles_receive_distinct_bound_source_demand_artifacts() {
        let namelist = test_namelist(0.2);
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        field.set(2, 1, 50.0);
        field.set(3, 1, 50.0);
        let prepared = PreparedHfieldDemand::capture(&field, 100.0, 2, 0.2, &namelist).unwrap();
        let land_gridfile = temp_gridfile("product_land");
        let ocean_gridfile = temp_gridfile("product_ocean");
        write_test_gridfile(
            &land_gridfile,
            &[45.0],
            &[45.0],
            &[40.0, 50.0, 45.0],
            &[40.0, 40.0, 50.0],
            &[1, 2, 3],
            None,
        );
        write_test_gridfile(
            &ocean_gridfile,
            &[135.0],
            &[45.0],
            &[130.0, 140.0, 135.0],
            &[40.0, 40.0, 50.0],
            &[1, 2, 3],
            None,
        );
        let mut land_support = vec![false; 8];
        land_support[1 * 4 + 2] = true;
        let mut ocean_support = vec![false; 8];
        ocean_support[1 * 4 + 3] = true;

        let land_artifact = prepared
            .persist_for_product_gridfile(
                &land_gridfile,
                HfieldDemandProductKind::Land,
                "tri",
                &land_support,
            )
            .unwrap();
        let ocean_artifact = prepared
            .persist_for_product_gridfile(
                &ocean_gridfile,
                HfieldDemandProductKind::Ocean,
                "tri",
                &ocean_support,
            )
            .unwrap();

        let land: PersistedHfieldDemand =
            serde_json::from_slice(&fs::read(&land_artifact).unwrap()).unwrap();
        let ocean: PersistedHfieldDemand =
            serde_json::from_slice(&fs::read(&ocean_artifact).unwrap()).unwrap();
        assert_eq!(land.output_product, "land");
        assert_eq!(ocean.output_product, "ocean");
        assert_eq!(land.product_support, land_support);
        assert_eq!(ocean.product_support, ocean_support);
        assert_eq!(land.intended_output_support, land_support);
        assert_eq!(ocean.intended_output_support, ocean_support);
        assert_ne!(land.gridfile_hash, ocean.gridfile_hash);
        assert_ne!(land.snapshot_hash, ocean.snapshot_hash);
        assert_ne!(land.artifact_hash, ocean.artifact_hash);
        assert_ne!(
            fs::read(&land_artifact).unwrap(),
            fs::read(&ocean_artifact).unwrap()
        );

        let loaded_land = load_hfield_source_demand(&land_gridfile, &namelist).unwrap();
        let loaded_ocean = load_hfield_source_demand(&ocean_gridfile, &namelist).unwrap();
        assert_eq!(loaded_land.intended_output_support, land_support);
        assert_eq!(loaded_ocean.intended_output_support, ocean_support);

        for path in [land_artifact, ocean_artifact, land_gridfile, ocean_gridfile] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn hard_center_demand_uses_positive_polygon_support_in_tri_physical_row_order() {
        let namelist = test_namelist(0.2);
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        field.set(2, 1, 50.0);
        field.set(3, 1, 50.0);
        let prepared = PreparedHfieldDemand::capture(&field, 100.0, 2, 0.2, &namelist).unwrap();
        let gridfile = temp_gridfile("hard_tri_rows");
        write_test_gridfile(
            &gridfile,
            &[5.0, 105.0],
            &[5.0, 5.0],
            &[1.0, 9.0, 5.0, 101.0, 109.0, 105.0],
            &[1.0, 1.0, 9.0, 1.0, 1.0, 9.0],
            &[1, 2, 3, 4, 5, 6],
            None,
        );

        assert_eq!(
            prepared
                .hard_center_demand_for_gridfile(&gridfile, "tri")
                .unwrap(),
            vec![true, true]
        );
        let mut product_support = vec![false; 8];
        product_support[1 * 4 + 3] = true;
        assert_eq!(
            prepared
                .hard_center_demand_for_product_gridfile(&gridfile, "tri", &product_support,)
                .unwrap(),
            vec![false, true],
            "result order must follow physical M-cell rows, not raster-bin order"
        );
        assert!(
            !source_demand_artifact_path(&gridfile).unwrap().exists(),
            "projection must not persist a sidecar before the final output is selected"
        );
        let _ = fs::remove_file(gridfile);
    }

    #[test]
    fn hard_center_demand_uses_positive_polygon_support_in_hex_physical_row_order() {
        let namelist = test_namelist(0.2);
        let mut field = HField::uniform(4, 2, 100.0).unwrap();
        field.set(2, 1, 50.0);
        field.set(3, 1, 50.0);
        let prepared = PreparedHfieldDemand::capture(&field, 100.0, 2, 0.2, &namelist).unwrap();
        let gridfile = temp_gridfile("hard_hex_rows");
        write_test_gridfile(
            &gridfile,
            &[101.0, 109.0, 105.0, 1.0, 9.0, 5.0],
            &[1.0, 1.0, 9.0, 1.0, 1.0, 9.0],
            &[105.0, 5.0],
            &[5.0, 5.0],
            &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
            Some((&[1, 2, 3, 4, 5, 6], &[3, 3], 3)),
        );

        assert_eq!(
            prepared
                .hard_center_demand_for_gridfile(&gridfile, "hex")
                .unwrap(),
            vec![true, true]
        );
        let mut product_support = vec![false; 8];
        product_support[1 * 4 + 3] = true;
        assert_eq!(
            prepared
                .hard_center_demand_for_product_gridfile(&gridfile, "hex", &product_support,)
                .unwrap(),
            vec![true, false],
            "result order must follow physical W-cell rows, not raster-bin order"
        );
        let _ = fs::remove_file(gridfile);
    }

    #[test]
    fn accepted_auto_refine_epoch_extends_chain_without_mutating_base_snapshot() {
        let candidate_namelist = test_namelist(0.2);
        let baseline_namelist =
            candidate_namelist.replace("NL%hfield_max_level=2", "NL%hfield_max_level=1");
        let baseline_namelist_path =
            write_test_namelist("accepted_epoch_baseline", &baseline_namelist);
        let candidate_namelist_path =
            write_test_namelist("accepted_epoch_candidate", &candidate_namelist);
        let baseline_gridfile = temp_gridfile("accepted_epoch_baseline");
        let candidate_gridfile = temp_gridfile("accepted_epoch_candidate");
        let mut base = HField::uniform(4, 2, 100.0).unwrap();
        base.set(0, 0, 50.0);
        PreparedHfieldDemand::capture(&base, 100.0, 1, 0.2, &baseline_namelist)
            .unwrap()
            .persist_for_gridfile(&baseline_gridfile)
            .unwrap();
        let mut candidate = base.clone();
        candidate.set(1, 0, 25.0);
        PreparedHfieldDemand::capture(&candidate, 100.0, 2, 0.2, &candidate_namelist)
            .unwrap()
            .persist_for_gridfile(&candidate_gridfile)
            .unwrap();
        let baseline_before =
            fs::read(source_demand_artifact_path(&baseline_gridfile).unwrap()).unwrap();

        publish_accepted_auto_refine_demand_epoch(
            &baseline_gridfile,
            &baseline_namelist_path,
            &candidate_gridfile,
            &candidate_namelist_path,
            prepared_epoch_at(1, 0),
        )
        .unwrap();

        assert_eq!(
            fs::read(source_demand_artifact_path(&baseline_gridfile).unwrap()).unwrap(),
            baseline_before
        );
        let baseline = read_validated_persisted(&baseline_gridfile, &baseline_namelist).unwrap();
        let selected = read_validated_persisted(&candidate_gridfile, &candidate_namelist).unwrap();
        assert_eq!(selected.snapshot_hash, baseline.snapshot_hash);
        assert_eq!(selected.base_hard_levels, baseline.base_hard_levels);
        assert_eq!(selected.epochs.len(), 1);
        assert_eq!(selected.epochs[0].epoch_id, 1);
        assert_eq!(
            selected.epochs[0].parent_snapshot_hash,
            baseline.snapshot_hash
        );
        assert_eq!(selected.epochs[0].hard_layers[0].kind, "auto_refine");
        assert_ne!(selected.chain_tip_hash, selected.snapshot_hash);
        assert_eq!(selected.hard_levels[0], 1);
        assert_eq!(selected.hard_levels[1], 2);
        load_hfield_source_demand(&candidate_gridfile, &candidate_namelist).unwrap();

        for path in [
            source_demand_artifact_path(&baseline_gridfile).unwrap(),
            source_demand_artifact_path(&candidate_gridfile).unwrap(),
            baseline_gridfile,
            candidate_gridfile,
            baseline_namelist_path,
            candidate_namelist_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn rejected_auto_refine_candidate_is_transactional_and_publishes_no_epoch() {
        let namelist = test_namelist(0.2);
        let namelist_path = write_test_namelist("rejected_epoch", &namelist);
        let baseline_gridfile = temp_gridfile("rejected_epoch_baseline");
        let candidate_gridfile = temp_gridfile("rejected_epoch_candidate");
        let mut base = HField::uniform(4, 2, 100.0).unwrap();
        base.set(0, 0, 50.0);
        for gridfile in [&baseline_gridfile, &candidate_gridfile] {
            PreparedHfieldDemand::capture(&base, 100.0, 2, 0.2, &namelist)
                .unwrap()
                .persist_for_gridfile(gridfile)
                .unwrap();
        }
        let baseline_artifact = source_demand_artifact_path(&baseline_gridfile).unwrap();
        let candidate_artifact = source_demand_artifact_path(&candidate_gridfile).unwrap();
        let baseline_before = fs::read(&baseline_artifact).unwrap();
        let candidate_before = fs::read(&candidate_artifact).unwrap();

        let error = publish_accepted_auto_refine_demand_epoch(
            &baseline_gridfile,
            &namelist_path,
            &candidate_gridfile,
            &namelist_path,
            prepared_epoch_at(1, 0),
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("not exactly base demand plus accepted AutoRefine epochs"));
        assert_eq!(fs::read(&baseline_artifact).unwrap(), baseline_before);
        assert_eq!(fs::read(&candidate_artifact).unwrap(), candidate_before);
        assert!(read_validated_persisted(&candidate_gridfile, &namelist)
            .unwrap()
            .epochs
            .is_empty());

        for path in [
            baseline_artifact,
            candidate_artifact,
            baseline_gridfile,
            candidate_gridfile,
            namelist_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn repeated_auto_refine_epoch_is_typed_and_does_not_rewrite_artifacts() {
        let namelist = test_namelist(0.2);
        let namelist_path = write_test_namelist("repeated_epoch", &namelist);
        let baseline_gridfile = temp_gridfile("repeated_epoch_baseline");
        let accepted_gridfile = temp_gridfile("repeated_epoch_accepted");
        let repeated_gridfile = temp_gridfile("repeated_epoch_candidate");
        let mut base = HField::uniform(4, 2, 100.0).unwrap();
        base.set(0, 0, 50.0);
        PreparedHfieldDemand::capture(&base, 100.0, 2, 0.2, &namelist)
            .unwrap()
            .persist_for_gridfile(&baseline_gridfile)
            .unwrap();
        let mut effective = base.clone();
        effective.set(1, 0, 25.0);
        for gridfile in [&accepted_gridfile, &repeated_gridfile] {
            PreparedHfieldDemand::capture(&effective, 100.0, 2, 0.2, &namelist)
                .unwrap()
                .persist_for_gridfile(gridfile)
                .unwrap();
        }
        publish_accepted_auto_refine_demand_epoch(
            &baseline_gridfile,
            &namelist_path,
            &accepted_gridfile,
            &namelist_path,
            prepared_epoch_at(1, 0),
        )
        .unwrap();
        let accepted_artifact = source_demand_artifact_path(&accepted_gridfile).unwrap();
        let repeated_artifact = source_demand_artifact_path(&repeated_gridfile).unwrap();
        let accepted_before = fs::read(&accepted_artifact).unwrap();
        let repeated_before = fs::read(&repeated_artifact).unwrap();

        let error = publish_accepted_auto_refine_demand_epoch(
            &accepted_gridfile,
            &namelist_path,
            &repeated_gridfile,
            &namelist_path,
            prepared_epoch_at(1, 0),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AutoRefineDemandEpochError::RepeatedDemandEpoch {
                existing_epoch_id: 1,
                ..
            }
        ));
        assert_eq!(fs::read(&accepted_artifact).unwrap(), accepted_before);
        assert_eq!(fs::read(&repeated_artifact).unwrap(), repeated_before);

        for path in [
            source_demand_artifact_path(&baseline_gridfile).unwrap(),
            accepted_artifact,
            repeated_artifact,
            baseline_gridfile,
            accepted_gridfile,
            repeated_gridfile,
            namelist_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn gradation_retry_preserves_epoch_chain_and_changes_only_candidate_controls() {
        let baseline_namelist = test_namelist(0.2);
        let candidate_namelist = test_namelist(0.1);
        let baseline_namelist_path = write_test_namelist("gradation_baseline", &baseline_namelist);
        let candidate_namelist_path =
            write_test_namelist("gradation_candidate", &candidate_namelist);
        let baseline_gridfile = temp_gridfile("gradation_baseline");
        let candidate_gridfile = temp_gridfile("gradation_candidate");
        let mut hard = HField::uniform(4, 2, 100.0).unwrap();
        hard.set(0, 0, 50.0);
        PreparedHfieldDemand::capture(&hard, 100.0, 2, 0.2, &baseline_namelist)
            .unwrap()
            .persist_for_gridfile(&baseline_gridfile)
            .unwrap();
        PreparedHfieldDemand::capture(&hard, 100.0, 2, 0.1, &candidate_namelist)
            .unwrap()
            .persist_for_gridfile(&candidate_gridfile)
            .unwrap();

        publish_accepted_gradation_retry_demand(
            &baseline_gridfile,
            &baseline_namelist_path,
            &candidate_gridfile,
            &candidate_namelist_path,
        )
        .unwrap();

        let baseline = read_validated_persisted(&baseline_gridfile, &baseline_namelist).unwrap();
        let selected = read_validated_persisted(&candidate_gridfile, &candidate_namelist).unwrap();
        assert_eq!(selected.snapshot_hash, baseline.snapshot_hash);
        assert_eq!(selected.chain_tip_hash, baseline.chain_tip_hash);
        assert_eq!(selected.epochs.len(), baseline.epochs.len());
        assert_eq!(selected.hard_levels, baseline.hard_levels);
        assert_eq!(selected.g, 0.1);
        assert_ne!(selected.project_hash, baseline.project_hash);

        for path in [
            source_demand_artifact_path(&baseline_gridfile).unwrap(),
            source_demand_artifact_path(&candidate_gridfile).unwrap(),
            baseline_gridfile,
            candidate_gridfile,
            baseline_namelist_path,
            candidate_namelist_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn zero_level_auto_refine_plan_returns_typed_missing_target() {
        let namelist = test_namelist(0.2);
        let namelist_path = write_test_namelist("zero_epoch", &namelist);
        let cells = temp_gridfile("zero_epoch_cells").with_extension("geojson");
        let levels = temp_gridfile("zero_epoch_levels").with_extension("json");
        fs::write(
            &levels,
            r#"{"kind":"earthmesh_refinement_plan","total_cells":1,"cells":[{"cell":0,"target_level":0}]}"#,
        )
        .unwrap();

        let error = prepare_auto_refine_demand_epoch(&namelist_path, &cells, &levels).unwrap_err();
        assert!(matches!(
            error,
            AutoRefineDemandEpochError::MissingImmutableAbsoluteTarget
        ));

        for path in [namelist_path, cells, levels] {
            let _ = fs::remove_file(path);
        }
    }
}
