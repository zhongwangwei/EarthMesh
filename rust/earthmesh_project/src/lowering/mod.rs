use crate::{
    criterion_catalog, degree_to_nxp, km_to_nxp, DomainConfig, GeometryIr, HfieldRefinementRecipe,
    ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec, ThresholdField,
    ThresholdStatistic, ViolationPolicy,
};
use earthmesh_core::{
    DataLayerConfig, DataLayerRole, DataLayersNamelist, EarthmeshConfig, QualityNamelist,
    RefineConfig,
};

/// The L3 engine execution plan produced by [`ProjectConfig::lower`].
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredProject {
    pub mkgrd: EarthmeshConfig,
    pub refine: RefineConfig,
    pub data_layers: DataLayersNamelist,
    pub quality: QualityNamelist,
    /// Emitted as a standalone `&hfield` group when enabled.
    pub hfield: Option<HfieldRefinementRecipe>,
}

impl LoweredProject {
    /// Emit a runnable namelist (`&mkgrd` + `&mkrefine` + `&quality` + `&datalayers`).
    pub fn to_namelist(&self) -> String {
        // The engine validates &mkrefine whenever the block is present - even for a
        // baseline grid - and that check rejects hex without Istransition and demands
        // refine_spc/cal, both meaningless when refine is off. A baseline grid only
        // needs &mkgrd, so omit &mkrefine when refinement is disabled.
        let mkrefine = if self.mkgrd.refine {
            format!("{}\n", self.refine.to_mkrefine_namelist())
        } else {
            String::new()
        };
        let hfield = match &self.hfield {
            Some(recipe) if recipe.enabled && self.mkgrd.refine => {
                let base_line = recipe
                    .base_m
                    .filter(|base| base.is_finite() && *base > 0.0)
                    .map(|base| format!("   NL%hfield_base_m = {base}\n"))
                    .unwrap_or_default();
                let origin_lines = match (recipe.origin_lon, recipe.origin_lat) {
                    (Some(lon), Some(lat)) => format!(
                        "   NL%hfield_origin_lon = {lon}\n   NL%hfield_origin_lat = {lat}\n"
                    ),
                    _ => String::new(),
                };
                let (nlon, nlat) = hfield_raster_size(recipe, &self.mkgrd, &self.refine);
                format!(
                    "&hfield\n   NL%hfield_on = .true.\n   NL%hfield_g = {}\n   NL%hfield_max_level = {}\n   NL%hfield_nlon = {}\n   NL%hfield_nlat = {}\n{}{}/\n\n",
                    recipe.g, recipe.max_level, nlon, nlat, base_line, origin_lines
                )
            }
            _ => String::new(),
        };
        format!(
            "{}\n{}{}{}\n{}",
            self.mkgrd.to_mkgrd_namelist(),
            mkrefine,
            hfield,
            self.quality.to_quality_namelist(),
            self.data_layers.to_datalayers_namelist()
        )
    }
}

impl ProjectConfig {
    fn data_layers_namelist(&self) -> DataLayersNamelist {
        let surface_landtype_required = matches!(
            self.target.kind,
            crate::MeshDomainKind::Land
                | crate::MeshDomainKind::Ocean
                | crate::MeshDomainKind::Coupled
        );
        let landcover_criterion_enabled = self
            .effective_landcover_criterion()
            .is_some_and(|criterion| criterion.enabled);
        let categorical_refinement_enabled =
            self.refinement.threshold_enabled && landcover_criterion_enabled;
        let landtype_refinement_active = self.refinement.enabled && categorical_refinement_enabled;
        let layers = self
            .data_layers
            .iter()
            .map(|l| {
                let is_landtype = matches!(l.role, ProjectLayerRole::LandType);
                let enabled = l.enabled
                    && (!is_landtype || surface_landtype_required || landtype_refinement_active);
                let (mean_enabled, std_enabled) = match l.role {
                    ProjectLayerRole::LandType => (false, false),
                    ProjectLayerRole::Threshold(field) => (
                        self.threshold_statistic_enabled(field, ThresholdStatistic::Mean),
                        self.threshold_statistic_enabled(field, ThresholdStatistic::Std),
                    ),
                    _ => (true, true),
                };
                DataLayerConfig {
                    id: l.id.clone(),
                    role: l.role.to_core(),
                    path: l.path.clone(),
                    var: None,
                    enabled,
                    required: is_landtype && enabled,
                    mean_enabled,
                    std_enabled,
                    categorical_enabled: is_landtype && categorical_refinement_enabled,
                }
            })
            .collect();
        DataLayersNamelist { layers }
    }

    fn quality_namelist(&self) -> QualityNamelist {
        QualityNamelist {
            min_angle_warn_deg: self.quality.min_angle_deg,
            repair_batch_limit: self.quality.auto_refine_batch_cells as i32,
            on_violation: self.quality.on_violation.as_str().to_string(),
            ..QualityNamelist::default()
        }
    }

    /// Lower the project (L1) to engine config (L3). Reuses the core lowering for
    /// data layers; the mesh algorithm is untouched.
    pub fn try_lower(&self) -> Result<LoweredProject, String> {
        self.validate()?;
        let mut mkgrd = EarthmeshConfig::default();
        let mut refine = RefineConfig::default();

        mkgrd.experiment_name = self.metadata.name.clone();
        // Core defaults mirror unset Fortran namelist sentinels. A compiled
        // Project must instead be directly runnable from its working directory.
        mkgrd.base_dir = "./".to_string();
        mkgrd.mode_file = "none".to_string();
        mkgrd.mode_file_description = "none".to_string();
        mkgrd.landtype_file = "none".to_string();
        mkgrd.mask_patch_type = "none".to_string();
        mkgrd.mask_patch_fprefix = "none".to_string();
        mkgrd.mesh_type = self.target.kind.engine_str().to_string();
        // An ocean carve marks cells by their own centre sample, so narrow bays
        // and river mouths come out as orphan cells or vertex-only contacts that
        // fail the topology gates and no refinement pass can repair. Ocean
        // projects therefore keep only the largest connected water body.
        mkgrd.isolated_ocean = mkgrd.mesh_type == "oceanmesh";
        mkgrd.mode_grid = self.target.cell.engine_str().to_string();
        mkgrd.output_format = self.target.model_format.engine_str().to_string();
        if let Some(coupling) = &self.coupling {
            mkgrd.coupling_fraction_method = coupling.fraction_method.engine_str().to_string();
            mkgrd.coupling_identify_coastline = coupling.identify_coastline;
            mkgrd.coupling_identify_river_mouth = coupling.identify_river_mouth;
            mkgrd.coupling_cama_root = coupling.cama_root.clone().unwrap_or_default();
        }
        match self.target.resolution {
            ResolutionSpec::Nxp(n) => mkgrd.nxp = n,
            ResolutionSpec::ApproxKm(km) => mkgrd.nxp = km_to_nxp(km),
            ResolutionSpec::ApproxDegree(degrees) => mkgrd.nxp = degree_to_nxp(degrees),
        }

        match &self.domain {
            DomainConfig::Global => mkgrd.mask_domain_global = true,
            DomainConfig::Regional { shape, sea_ratio } => {
                mkgrd.mask_domain_global = false;
                mkgrd.mask_domain_type = match shape {
                    RegionShape::Bbox { w, e, n, s } => {
                        mkgrd.mask_domain_fprefix = bbox_geometry(*w, *e, *s, *n)?;
                        "bbox"
                    }
                    RegionShape::Circle {
                        lon,
                        lat,
                        radius_km,
                    } => {
                        mkgrd.mask_domain_fprefix = circle_geometry(*lon, *lat, *radius_km)?;
                        "circle"
                    }
                    RegionShape::Shapefile { path } => {
                        mkgrd.mask_domain_fprefix = path.clone();
                        mkgrd.mask_domain_close_boundary =
                            crate::CloseBoundaryMode::Polyline.to_engine_spec();
                        "close"
                    }
                    RegionShape::Close { path, boundary, .. } => {
                        mkgrd.mask_domain_fprefix = path.clone();
                        mkgrd.mask_domain_close_boundary = boundary.to_engine_spec();
                        "close"
                    }
                }
                .to_string();
                if let Some(ratio) = sea_ratio {
                    mkgrd.mask_sea_ratio = *ratio;
                }
            }
        }

        // Data layers drive landtype_file + refine switches (core lowering).
        let dl = self.data_layers_namelist();
        let lowering_layers = if self.refinement.threshold_enabled {
            dl.clone()
        } else {
            DataLayersNamelist {
                layers: dl
                    .layers
                    .iter()
                    .filter(|layer| !matches!(layer.role, DataLayerRole::ThresholdField(_)))
                    .cloned()
                    .collect(),
            }
        };
        lowering_layers.lower_into(&mut mkgrd, &mut refine);
        if self.refinement.threshold_enabled {
            apply_threshold_values(&mut refine, self);
        }
        // Refinement runs only when a real source supplies data. LandType mask
        // availability is independent from its explicit categorical criterion.
        if let Some(circle) = &self.refinement.specified_circle {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "circle".to_string();
            refine.mask_refine_spc_fprefix =
                circle_geometry(circle.lon, circle.lat, circle.radius_km)?;
        }
        if let Some(bbox) = &self.refinement.specified_bbox {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "bbox".to_string();
            refine.mask_refine_spc_fprefix = bbox_geometry(bbox.w, bbox.e, bbox.s, bbox.n)?;
        }
        if let Some(close) = &self.refinement.specified_close {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "close".to_string();
            refine.mask_refine_spc_fprefix = close.path.clone();
            refine.mask_refine_spc_close_boundary = close.boundary.to_engine_spec();
        }
        mkgrd.refine = self.refinement.enabled && (refine.refine_cal || refine.refine_spc);
        if mkgrd.refine {
            let max_passes = i32::from(self.refinement.max_passes);
            if refine.refine_cal {
                refine.max_iter_cal = max_passes;
            }
            if refine.refine_spc {
                refine.max_iter_spc = max_passes;
            }
        }

        // Auto spring smoothing: keep the low-level SpringGlobal/SpringRegional pair
        // mutually exclusive while deriving the Method-C-compatible choice from grid/domain.
        //
        // This applies to `tri` as well. Only hex is *required* to run with
        // `Istransition`, but leaving tri unset made it inherit the config
        // defaults, where `RefineConfig::validate` zeroes both spring types and
        // `method_c_spring_iterations` then returns 0 — a Method-C refined tri
        // mesh whose transition rows were never smoothed. Measured on a global
        // 100 km CoastalOcean project (108k cells, 2000 iterations):
        // angle_deviation_deg.max 40.87 -> 27.05 (clearing the 35 warn gate),
        // min_angle_deg 28.98 -> 38.66, aspect_ratio.max 2.04 -> 1.56.
        // Setting both fields also keeps expert overrides usable: with only one
        // of them supplied, the other kept its default of 1 and lowering failed
        // the mutual-exclusion check.
        refine.is_transition = true;
        refine.spring_global_type = if mkgrd.mask_domain_global { 1 } else { 0 };
        refine.spring_regional_type = if mkgrd.mask_domain_global { 0 } else { 1 };

        // Expert overrides win last.
        if let Some(n) = self.expert.nxp {
            mkgrd.nxp = n;
        }
        if let Some(t) = self.expert.openmp {
            mkgrd.openmp = t;
        }
        if let Some(n) = self.expert.niter {
            mkgrd.niter = n;
        }
        if let Some(n) = self.expert.niter_refine {
            refine.niter_refine = n;
            refine.niter_refine_specified = true;
        }
        if let Some(n) = self.expert.max_iter_spc {
            refine.max_iter_spc = n;
        }
        if let Some(n) = self.expert.max_iter_cal {
            refine.max_iter_cal = n;
        }
        if let Some(values) = &self.expert.halo {
            apply_i32_prefix(&mut refine.halo, values);
        }
        if let Some(values) = &self.expert.max_transition_row {
            apply_i32_prefix(&mut refine.max_transition_row, values);
        }
        if let Some(set_dis_type) = &self.expert.set_dis_type {
            refine.set_dis_type = set_dis_type.clone();
        }
        if let Some(n) = self.expert.num_rc {
            refine.num_rc = n;
        }
        if let Some(n) = self.expert.vertex_pretect_layers {
            refine.vertex_pretect_layers = n;
        }
        if self.expert.spring_global_type.is_some() || self.expert.spring_regional_type.is_some() {
            refine.is_transition = true;
            refine.spring_global_type = self
                .expert
                .spring_global_type
                .unwrap_or(refine.spring_global_type);
            refine.spring_regional_type = self
                .expert
                .spring_regional_type
                .unwrap_or(refine.spring_regional_type);
        }
        if let Some(enabled) = self.expert.isolated_ocean {
            mkgrd.isolated_ocean = enabled;
        }
        if let Some(enabled) = self.expert.weak_concav_eliminate {
            refine.weak_concav_eliminate = enabled;
        }
        if let Some(beta) = self.expert.beta {
            mkgrd.beta = beta;
        }
        if let Some(relax) = self.expert.relax {
            mkgrd.relax = relax;
        }

        let hfield = if mkgrd.refine {
            match &self.refinement.hfield {
                Some(recipe) if recipe.enabled => Some(recipe.clone()),
                Some(_) => None,
                None => Some(HfieldRefinementRecipe::default()),
            }
        } else {
            None
        };

        // Method-C local refinement advances on a stride-3 lattice. Build the
        // parent mesh on that lattice from the start so HField and later
        // quality AutoRefine passes share one physically compatible topology.
        // Rounding upward preserves or slightly improves the requested spatial
        // resolution; uniform meshes without AutoRefine remain unchanged.
        let hydro_local_refinement = self
            .hydro_execution_plan()?
            .is_some_and(|plan| plan.max_level > 0);
        if hfield.is_some()
            || hydro_local_refinement
            || self.quality.on_violation == ViolationPolicy::AutoRefine
        {
            let increment = (3 - mkgrd.nxp.rem_euclid(3)) % 3;
            mkgrd.nxp = mkgrd
                .nxp
                .checked_add(increment)
                .ok_or_else(|| "Method-C effective NXP overflows i32".to_string())?;
        }

        Ok(LoweredProject {
            mkgrd,
            refine,
            hfield,
            data_layers: lowering_layers,
            quality: self.quality_namelist(),
        })
    }

    /// Lower an already-validated project.
    pub fn lower(&self) -> LoweredProject {
        self.try_lower()
            .expect("ProjectConfig::lower requires a valid project")
    }
}

fn apply_i32_prefix(target: &mut [i32; 10], values: &[i32]) {
    for (slot, value) in target.iter_mut().skip(1).zip(values.iter().copied()) {
        *slot = value;
    }
}

fn apply_threshold_values(refine: &mut RefineConfig, project: &ProjectConfig) {
    for layer in &project.data_layers {
        if !layer.enabled || layer.path.trim().is_empty() {
            continue;
        }
        if matches!(layer.role, ProjectLayerRole::LandType) {
            let Some(criterion) = project.effective_landcover_criterion() else {
                continue;
            };
            if !criterion.enabled {
                continue;
            }
            refine.refine_num_landtypes = true;
            refine.refine_cal = true;
            refine.th_num_landtypes = criterion.value.round() as i32;
            continue;
        }
        let ProjectLayerRole::Threshold(field) = layer.role else {
            continue;
        };
        let Some(default_value) = criterion_catalog()
            .iter()
            .find(|criterion| criterion.field == field)
            .map(|criterion| layer.threshold_value.unwrap_or(criterion.gui.default))
        else {
            continue;
        };
        let mean =
            threshold_statistic_value(project, field, ThresholdStatistic::Mean, default_value);
        let std = threshold_statistic_value(project, field, ThresholdStatistic::Std, default_value);
        match field {
            ThresholdField::Lai => set_axis_values(&mut refine.th_onelayer_lnd, 0, mean, std),
            ThresholdField::Slope => set_axis_values(&mut refine.th_onelayer_lnd, 2, mean, std),
            ThresholdField::Dem => set_axis_values(&mut refine.th_onelayer_lnd, 4, mean, std),
            ThresholdField::SlopeMax => set_axis_values(&mut refine.th_onelayer_lnd, 6, mean, std),
            ThresholdField::Ks => set_layer_axis_values(&mut refine.th_twolayer_lnd, 0, mean, std),
            ThresholdField::KSolids => {
                set_layer_axis_values(&mut refine.th_twolayer_lnd, 2, mean, std)
            }
            ThresholdField::Tkdry => {
                set_layer_axis_values(&mut refine.th_twolayer_lnd, 4, mean, std)
            }
            ThresholdField::Tksatf => {
                set_layer_axis_values(&mut refine.th_twolayer_lnd, 6, mean, std)
            }
            ThresholdField::Tksatu => {
                set_layer_axis_values(&mut refine.th_twolayer_lnd, 8, mean, std)
            }
            ThresholdField::Sst => set_axis_values(&mut refine.th_onelayer_ocn, 0, mean, std),
            ThresholdField::Ssh => set_axis_values(&mut refine.th_onelayer_ocn, 2, mean, std),
            ThresholdField::Eke => set_axis_values(&mut refine.th_onelayer_ocn, 4, mean, std),
            ThresholdField::SeaSlope => set_axis_values(&mut refine.th_onelayer_ocn, 6, mean, std),
            ThresholdField::Typhoon => set_axis_values(&mut refine.th_onelayer_atmos, 0, mean, std),
        }
    }
}

fn threshold_statistic_value(
    project: &ProjectConfig,
    field: ThresholdField,
    statistic: ThresholdStatistic,
    fallback: f64,
) -> f64 {
    project
        .effective_threshold_criterion(field, statistic)
        .map(|criterion| criterion.value)
        .unwrap_or(fallback)
}

fn set_axis_values<const N: usize>(values: &mut [f64; N], start: usize, mean: f64, std: f64) {
    if let Some(slot) = values.get_mut(start) {
        *slot = mean;
    }
    if let Some(slot) = values.get_mut(start + 1) {
        *slot = std;
    }
}

fn set_layer_axis_values(values: &mut [[f64; 2]; 10], start: usize, mean: f64, std: f64) {
    if let Some(slot) = values.get_mut(start) {
        *slot = [mean; 2];
    }
    if let Some(slot) = values.get_mut(start + 1) {
        *slot = [std; 2];
    }
}

fn bbox_geometry(w: f64, e: f64, s: f64, n: f64) -> Result<String, String> {
    GeometryIr::bbox(w, e, s, n).to_inline_mask_source()
}

fn circle_geometry(lon: f64, lat: f64, radius_km: f64) -> Result<String, String> {
    GeometryIr::circle(lon, lat, radius_km).to_inline_mask_source()
}

/// Field raster size for a lowered project, explicit values winning over the
/// derived ones.
///
/// The h-field is sampled at triangle centres and edge midpoints, and Method-C
/// can only refine where a full rad3 footprint fits. That puts the usable raster
/// in a window at both ends: too coarse aliases the level map into a ragged
/// selection that `perim_fill3` rejects, too fine resolves demand narrower than
/// a footprint, which is then refined only where one happens to land.
///
/// Expressed as base cells per raster cell — `h_base / spacing` — every measured
/// point falls into place:
///
/// ```text
///    4    fails, aliased          (NXP 81 single level)
///  6.9    passes                  (the engine's fixed 720x360 at NXP 21)
///    8    passes                  (NXP 81 both levels, NXP 21 two levels)
///   12    passes                  (NXP 21 two levels)
///   16    passes at NXP 81, fails at NXP 21
///   32    fails, fragmented       (NXP 21 two levels)
/// ```
///
/// So target 8, the middle of the range that holds at both resolutions:
///
/// ```text
/// nlat = 20 * NXP        (spacing = h_base / 8)
/// nlon = 2 * nlat
/// ```
///
/// Note this does not depend on the refinement level. An earlier `h_min/4` rule
/// did, and derived a *failing* raster for NXP 21 two-level (nlat 840, ratio 16)
/// while the engine's own default worked — the level term pushed low-NXP
/// multi-level runs out the fine end of the window. The window's width is still
/// resolution-dependent (16 passes at NXP 81 but not at NXP 21), so this targets
/// the middle rather than an edge.
///
/// Raster size is measured to be free: the same project at 842x421 and
/// 3240x1620 both finish in 42 s, the gradient limiter being nowhere near the
/// bottleneck.
fn hfield_raster_size(
    recipe: &HfieldRefinementRecipe,
    mkgrd: &EarthmeshConfig,
    _refine: &RefineConfig,
) -> (usize, usize) {
    const ENGINE_DEFAULT_NLON: usize = 720;
    const ENGINE_DEFAULT_NLAT: usize = 360;
    /// Base cells per raster cell. Middle of the measured window.
    const BASE_CELLS_PER_RASTER_CELL: usize = 8;
    const MAX_DERIVED_NLAT: usize = 8192;

    let nxp = usize::try_from(mkgrd.nxp.max(1)).unwrap_or(1);
    let derived_nlat = nxp
        .saturating_mul(BASE_CELLS_PER_RASTER_CELL)
        .saturating_mul(5)
        .div_ceil(2)
        .clamp(ENGINE_DEFAULT_NLAT, MAX_DERIVED_NLAT);

    let nlat = recipe
        .nlat
        .filter(|value| *value > 0)
        .unwrap_or(derived_nlat);
    let nlon = recipe
        .nlon
        .filter(|value| *value > 0)
        .unwrap_or_else(|| (2 * nlat).max(ENGINE_DEFAULT_NLON));
    (nlon, nlat)
}
