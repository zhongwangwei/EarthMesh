use crate::{
    criterion_catalog, degree_to_nxp, km_to_nxp, AdaptiveRefinementRecipe,
    CertifiedRefinementRecipe, DomainConfig, GeometryIr, HfieldRefinementRecipe, MethodCAlgorithm,
    MethodCRefinementRecipe, ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec,
    ThresholdField, ThresholdStatistic, ViolationPolicy,
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
    /// Which refinement algorithm the run asked for.
    pub backend: crate::RefinementBackend,
    /// Method-C's internal algorithm and bounded LEPP settings.
    pub method_c: MethodCRefinementRecipe,
    /// CMRC delivery mode and strict resource/search bounds.
    pub certified: CertifiedRefinementRecipe,
    /// Emitted as a standalone `&adaptive` group when enabled.
    pub adaptive: Option<AdaptiveRefinementRecipe>,
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
        let adaptive = match &self.adaptive {
            Some(recipe) if recipe.enabled && self.mkgrd.refine => {
                let base_line = recipe
                    .base_m
                    .filter(|base| base.is_finite() && *base > 0.0)
                    .map(|base| format!("   NL%adaptive_base_m = {base}\n"))
                    .unwrap_or_default();
                format!(
                    "&adaptive\n   NL%adaptive_on = .true.\n   NL%adaptive_max_level = {}\n   NL%adaptive_coastline = {}\n{}/\n\n",
                    recipe.max_level,
                    if recipe.coastline { ".true." } else { ".false." },
                    base_line
                )
            }
            _ => String::new(),
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
        let method_c = if self.mkgrd.refine
            && self.backend == crate::RefinementBackend::MethodC
            && self.method_c.algorithm == MethodCAlgorithm::LeppDelaunay
        {
            format!(
                "&method_c\n   NL%algorithm = 'lepp_delaunay'\n   NL%max_cycles = {}\n   NL%target_size_tolerance = {}\n   NL%maximum_neighbor_size_ratio = {}\n   NL%maximum_vertices = {}\n   NL%maximum_insertions_per_cycle = {}\n   NL%maximum_path_length = {}\n   NL%stop_at_source_resolution = {}\n   NL%minimum_triangle_angle_deg = {}\n/\n\n",
                self.method_c.max_cycles,
                self.method_c.target_size_tolerance,
                self.method_c.maximum_neighbor_size_ratio,
                self.method_c.maximum_vertices,
                self.method_c.maximum_insertions_per_cycle,
                self.method_c.maximum_path_length,
                if self.method_c.stop_at_source_resolution {
                    ".true."
                } else {
                    ".false."
                },
                self.method_c.minimum_triangle_angle_deg,
            )
        } else {
            String::new()
        };
        let certified = if self.mkgrd.refine && self.backend == crate::RefinementBackend::Certified
        {
            format!(
                    "&certified\n   NL%mode = '{}'\n   NL%delivery = '{}'\n   NL%angle_contract = '{}'\n   NL%maximum_level = {}\n   NL%maximum_cells = {}\n   NL%gradation_rings_per_level = {}\n   NL%search_budget = {}\n/\n\n",
                    match self.certified.mode {
                        crate::CertifiedMode::SafeMotherOnly => "safe_mother_only",
                        crate::CertifiedMode::ReverseCoarsening => "reverse_coarsening",
                        crate::CertifiedMode::StretchedMother => "stretched_mother",
                        crate::CertifiedMode::EquidistributedMother => "equidistributed_mother",
                    },
                    match self.certified.delivery {
                        crate::CertifiedDeliveryMode::Tri => "tri",
                        crate::CertifiedDeliveryMode::Hex => "hex",
                        crate::CertifiedDeliveryMode::Coupled => "coupled",
                    },
                    match self.certified.angle_contract {
                        crate::CertifiedAngleContract::LegacyStrict40To80 => "legacy_strict_40_to_80",
                        crate::CertifiedAngleContract::DomainQuality38To82V1 => "domain_quality_38_to_82_v1",
                    },
                    self.certified.maximum_level,
                    self.certified.maximum_cells,
                    self.certified.gradation_rings_per_level,
                    self.certified.search_budget,
                )
        } else {
            String::new()
        };
        format!(
            "{}\n{}{}{}{}{}{}\n{}",
            self.mkgrd.to_mkgrd_namelist(),
            mkrefine,
            method_c,
            certified,
            adaptive,
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
        let sea_ratio_criterion_enabled = self
            .effective_sea_ratio_criterion()
            .is_some_and(|criterion| criterion.enabled);
        let landtype_refinement_active = self.refinement.enabled
            && self.refinement.threshold_enabled
            && (landcover_criterion_enabled || sea_ratio_criterion_enabled);
        let categorical_refinement_enabled = self.refinement.enabled
            && self.refinement.threshold_enabled
            && landcover_criterion_enabled;
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
        let lepp = self.quality.lepp_post_quality.as_ref();
        QualityNamelist {
            min_angle_warn_deg: self.quality.min_angle_deg,
            min_angle_fail_deg: self
                .quality
                .min_angle_deg
                .min(QualityNamelist::default().min_angle_fail_deg),
            repair_batch_limit: self.quality.auto_refine_batch_cells as i32,
            on_violation: self.quality.on_violation.as_str().to_string(),
            lepp_post_quality: lepp.is_some(),
            lepp_post_quality_max_insertions: lepp
                .map(|config| config.maximum_insertions as i32)
                .unwrap_or_else(|| QualityNamelist::default().lepp_post_quality_max_insertions),
            lepp_post_quality_max_edge_km: lepp
                .and_then(|config| config.maximum_edge_km)
                .unwrap_or(0.0),
            ..QualityNamelist::default()
        }
    }

    /// Lower the project (L1) to engine config (L3). Reuses the core lowering for
    /// data layers; the mesh algorithm is untouched.
    pub fn try_lower(&self) -> Result<LoweredProject, String> {
        self.validate()?;
        if self.quality.quality_policy == crate::QualityPolicy::DomainExport {
            return Err(
                "quality_policy=domain_export is schema/preflight-only until DQX execution is implemented"
                    .to_string(),
            );
        }
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
        if self.refinement.enabled && self.refinement.threshold_enabled {
            apply_threshold_values(&mut refine, self);
            if let Some(shape) = &self.refinement.threshold_region {
                // File imports and primitives are staged as evaluation-only
                // masks by the CLI; these are never specified/hard demands.
                let (kind, source) = match shape {
                    RegionShape::Bbox { w, e, s, n } => ("bbox", bbox_geometry(*w, *e, *s, *n)?),
                    RegionShape::Circle {
                        lon,
                        lat,
                        radius_km,
                    } => ("circle", circle_geometry(*lon, *lat, *radius_km)?),
                    RegionShape::Shapefile { path } | RegionShape::Close { path, .. } => {
                        ("close", path.clone())
                    }
                };
                refine.mask_refine_cal_type = kind.into();
                refine.mask_refine_cal_fprefix = source;
            }
        }
        // Refinement runs only when a real source supplies data. LandType mask
        // availability is independent from its explicit categorical criterion.
        if let Some(circles) = &self.refinement.specified_circle {
            let circles = circles.as_slice();
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "circle".to_string();
            // Keep the single-circle form byte-identical so existing projects
            // lower exactly as before; only a chain takes the new syntax.
            refine.mask_refine_spc_fprefix = match circles {
                [circle] => circle_geometry(circle.lon, circle.lat, circle.radius_km)?,
                many => GeometryIr::circles_inline_mask_source(
                    many.iter().map(|c| (c.lon, c.lat, c.radius_km)),
                )?,
            };
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
        if matches!(self.refinement.backend, crate::RefinementBackend::Certified) {
            // These backends certify their own transactional geometry; the
            // fixed-topology generic spring would invalidate that certificate.
            refine.spring_global_type = 0;
            refine.spring_regional_type = 0;
        }
        if let Some(enabled) = self.expert.isolated_ocean {
            mkgrd.isolated_ocean = enabled;
        }
        if let Some(enabled) = self.expert.hex_cell_evening {
            mkgrd.hex_cell_evening = enabled;
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

        // A run refines one way or the other. Point+radius is the default
        // because it is the only route that can re-ask a criterion after the
        // cells it judges exist; the h-field stays reachable by asking for it.
        // The backend is settled here rather than deeper down, because this is
        // the last place that still knows what the project asked for -- past it
        // the run is a namelist, and "red-green" would have to be inferred from
        // the absence of something.
        let backend = self.refinement.backend;
        // Carried into the namelist rather than settled and dropped here. The
        // runner only ever sees a namelist, so a choice that stops at lowering
        // is a choice nothing downstream can act on -- which is how this one
        // came to be unreachable from a run.
        mkgrd.refine_backend = match backend {
            crate::RefinementBackend::MethodC => "method_c",
            crate::RefinementBackend::RedGreen => "red_green",
            crate::RefinementBackend::Certified => "certified",
            crate::RefinementBackend::Stretch => "stretch",
            crate::RefinementBackend::IconNest => "icon_nest",
        }
        .to_string();
        let explicit_hfield = matches!(&self.refinement.hfield, Some(recipe) if recipe.enabled);
        // Canonical Method-C cannot build a region whose shape came from the
        // data -- its seeds step three cells at a time and its perimeters come
        // in multiples of three -- so the point+radius route refuses such
        // demand, once the data has been read. Its h-field route takes the same
        // data as a cell-width field and nests from quantized target levels,
        // which it can build. A project that asks for threshold refinement on
        // canonical Method-C and names neither route gets that one.
        let criteria_on_canonical_method_c = backend == crate::RefinementBackend::MethodC
            && self.refinement.method_c.algorithm == MethodCAlgorithm::Canonical
            && self.refinement.threshold_enabled
            && self.refinement.adaptive.is_none()
            && self.refinement.hfield.is_none()
            && self
                .data_layers
                .iter()
                .any(|layer| self.layer_has_threshold_criterion(layer));
        // With a demand and no h-field asked for, the run takes the
        // point+radius route below, which re-judges its criteria on the cells
        // it makes; a mother would have to move it to the h-field instead.
        let hfield_route = explicit_hfield || criteria_on_canonical_method_c || !mkgrd.refine;
        let regional_mother_levels = self.regional_mother_levels(&mkgrd, &refine, hfield_route)?;
        if regional_mother_levels > 0 {
            // The mother is built this many halvings coarser and only the
            // domain is refined back down; `nxp` names the mother, and the
            // requested resolution rounds up to one it can be halved to.
            let stride = 3_i32 << regional_mother_levels;
            let requested = mkgrd
                .nxp
                .checked_add((stride - mkgrd.nxp.rem_euclid(stride)) % stride)
                .ok_or_else(|| "regional mother NXP overflows i32".to_string())?;
            mkgrd.refine = true;
            mkgrd.regional_mother_levels = regional_mother_levels;
            mkgrd.nxp = requested >> regional_mother_levels;
        }
        let hfield_requested =
            explicit_hfield || criteria_on_canonical_method_c || regional_mother_levels > 0;
        // Not gated on the backend: the criteria half of the point+radius route
        // is raster work that produces an ordinary circle list, and both
        // backends consume it. Only turning those circles into mesh is
        // per-backend -- and that is the half suspended on Method-C, which is
        // why red-green is the one that can actually serve a coastline.
        let adaptive = if mkgrd.refine
            && !hfield_requested
            && backend != crate::RefinementBackend::Certified
        {
            match &self.refinement.adaptive {
                Some(recipe) if recipe.enabled => Some(recipe.clone()),
                Some(_) => None,
                None => Some(AdaptiveRefinementRecipe::default()),
            }
            .map(|mut recipe| {
                // Chasing the coast reads the land-type raster like any
                // threshold row, so it follows the threshold switch. A project
                // that names only a region asked for that region: on Method-C,
                // which refuses data-shaped demand, a default-on coastline
                // failed every land and ocean run with a specified circle.
                recipe.coastline &= self.refinement.threshold_enabled;
                recipe
            })
        } else {
            None
        };
        // Only ever chosen by asking for it. Turning the adaptive route off
        // disables that route; it does not silently swap in another backend.
        let hfield = if mkgrd.refine && hfield_requested {
            self.refinement
                .hfield
                .clone()
                .or_else(|| Some(HfieldRefinementRecipe::default()))
        } else {
            None
        };

        // Canonical Method-C plus hydro/quality Method-C adapters advance on a
        // stride-3 lattice. Build their parent mesh on it from the start.
        // Rounding upward preserves or slightly improves the requested spatial
        // resolution; uniform meshes without AutoRefine remain unchanged.
        let hydro_local_refinement = self
            .hydro_execution_plan()?
            .is_some_and(|plan| plan.max_level > 0);
        let canonical_method_c = backend == crate::RefinementBackend::MethodC
            && self.refinement.method_c.algorithm == MethodCAlgorithm::Canonical;
        if (canonical_method_c && (hfield.is_some() || adaptive.is_some()))
            || hydro_local_refinement
            || (matches!(
                backend,
                crate::RefinementBackend::MethodC | crate::RefinementBackend::RedGreen
            ) && self.quality.on_violation == ViolationPolicy::AutoRefine)
        {
            let increment = (3 - mkgrd.nxp.rem_euclid(3)) % 3;
            mkgrd.nxp = mkgrd
                .nxp
                .checked_add(increment)
                .ok_or_else(|| "stride-compatible effective NXP overflows i32".to_string())?;
        }

        Ok(LoweredProject {
            mkgrd,
            refine,
            backend,
            method_c: self.refinement.method_c.clone(),
            certified: self.refinement.certified.clone(),
            adaptive,
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
            if let Some(criterion) = project.effective_landcover_criterion() {
                if criterion.enabled {
                    refine.refine_num_landtypes = true;
                    refine.refine_cal = true;
                    refine.th_num_landtypes = criterion.value.round() as i32;
                }
            }
            if let Some(criterion) = project.effective_sea_ratio_criterion() {
                if criterion.enabled {
                    refine.refine_sea_ratio = true;
                    refine.refine_cal = true;
                    refine.th_sea_ratio = [criterion.value, 1.0 - criterion.value];
                }
            }
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
/// The coarsest a regional run's automatic mother gets: about 170 km cells.
const REGIONAL_MOTHER_MIN_NXP: i32 = 48;
/// Automatic mothers stop at 3 halvings (1/64 of the triangles), leaving
/// levels for the run's own refinement under the Method-C cap of 5.
const REGIONAL_MOTHER_AUTO_LEVELS: u8 = 3;

impl ProjectConfig {
    /// How many halvings coarser than the requested resolution a regional
    /// run's global mother is built, the domain then refined back down.
    ///
    /// Building the mother at the requested resolution and carving the domain
    /// out afterwards costs the whole globe at that resolution: a 4 km Heihe
    /// run spent 12 minutes and 39 GB on 80 million global triangles for a
    /// basin of a few thousand cells. Only the h-field route can refine a
    /// domain to a level (canonical Method-C and red-green); the other routes,
    /// an explicit NXP, and hydro refinement keep the mother at the requested
    /// resolution. Chosen automatically only for a run already on the h-field
    /// or with no demand besides the domain: regions or criteria that would
    /// take the point+radius route (which re-judges criteria on the cells it
    /// makes) keep it, unless `expert.regional_mother_levels` asks -- moving
    /// the Heihe red-green run to the h-field changed its mesh by 42%. The
    /// run's own levels come first: mother plus refinement levels stay within 5.
    fn regional_mother_levels(
        &self,
        mkgrd: &EarthmeshConfig,
        refine: &RefineConfig,
        hfield_route: bool,
    ) -> Result<u8, String> {
        let max_levels = crate::METHOD_C_MAX_AUTO_REFINE_LEVEL;
        let route = if mkgrd.mask_domain_global {
            Err("a global domain has nothing to refine a mother down to")
        } else if !match self.refinement.backend {
            crate::RefinementBackend::MethodC => {
                self.refinement.method_c.algorithm == MethodCAlgorithm::Canonical
            }
            crate::RefinementBackend::RedGreen => true,
            _ => false,
        } {
            Err("only canonical Method-C and red-green refine a domain on the h-field")
        } else if matches!(&self.refinement.adaptive, Some(recipe) if recipe.enabled) {
            Err("the point+radius route does not refine a domain to a level")
        } else if !hfield_route && self.expert.regional_mother_levels.is_none() {
            // Chosen automatically only where it changes nothing but the cost.
            Err("this run's demand takes the point+radius route")
        } else if matches!(&self.refinement.hfield, Some(recipe) if !recipe.enabled || recipe.base_m.is_some())
        {
            Err("the h-field is turned off or has its own base size")
        } else if self.quality.lepp_post_quality.is_some() {
            Err("LEPP post-quality refines the mother as built")
        } else if self
            .hydro_execution_plan()?
            .is_some_and(|plan| plan.max_level > 0)
        {
            Err("hydro refinement counts levels from the requested resolution")
        } else {
            Ok(())
        };
        let own_levels = if mkgrd.refine {
            let spc = if refine.refine_spc {
                refine.max_iter_spc
            } else {
                0
            };
            let cal = if refine.refine_cal {
                refine.max_iter_cal
            } else {
                0
            };
            let field = self
                .refinement
                .hfield
                .as_ref()
                .map_or(0, |recipe| i32::from(recipe.max_level));
            u8::try_from(spc.max(cal).max(field).max(1)).unwrap_or(max_levels)
        } else {
            0
        };
        match self.expert.regional_mother_levels {
            Some(0) => Ok(0),
            Some(levels) => {
                route.map_err(|why| format!("expert regional_mother_levels: {why}"))?;
                if self.expert.nxp.is_some() {
                    return Err(
                        "expert regional_mother_levels and expert nxp both set the mother; set one"
                            .to_string(),
                    );
                }
                if levels + own_levels > max_levels {
                    return Err(format!(
                        "expert regional_mother_levels {levels} plus {own_levels} refinement level(s) exceeds {max_levels}"
                    ));
                }
                Ok(levels)
            }
            None => {
                if route.is_err() || self.expert.nxp.is_some() {
                    return Ok(0);
                }
                let mut levels = 0;
                while levels < REGIONAL_MOTHER_AUTO_LEVELS
                    && levels + own_levels < max_levels
                    && mkgrd.nxp >= REGIONAL_MOTHER_MIN_NXP << (levels + 1)
                {
                    levels += 1;
                }
                Ok(levels)
            }
        }
    }
}

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

    // The mother's NXP, regional mother or not: sized to the requested
    // resolution instead, the Heihe and Sichuan runs made the same meshes
    // (within 2% of cells) at five times the memory.
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
