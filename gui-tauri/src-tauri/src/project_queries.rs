//! Read-only project catalog and summary command handlers.

use crate::dto::{
    CriterionInfo, LayerSummary, ProjectCapabilities, ProjectSummary, TargetCompatibilityInfo,
    TargetPresetInfo, ThresholdCriterionSummary,
};
use earthmesh_project::{
    criterion_catalog, default_mask_sea_ratio, threshold_criterion_catalog, CloseMaskFormat,
    DomainConfig, HfieldRefinementRecipe, HydroCoastConfig, MeshDomainKind, MeshIntentPreset,
    ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec,
    DEFAULT_ATMOSPHERE_REFINE_SPRING_ITERATIONS, DEFAULT_LANDCOVER_CLASS_THRESHOLD,
    DEFAULT_MIN_ANGLE_DEG, DEFAULT_SURFACE_REFINE_SPRING_ITERATIONS, INTENT_PRESETS,
    KM_PER_DEGREE_EQUATOR, LANDCOVER_CRITERION_ID, METHOD_C_MAX_AUTO_REFINE_LEVEL,
    METHOD_C_MIN_BASE_NXP, METHOD_C_SPRING_NXP1_KM,
};

/// List every registered refinement criterion (self-describing GUI specs).
#[tauri::command]
pub(crate) fn list_criteria() -> Vec<CriterionInfo> {
    let mut criteria = vec![CriterionInfo {
        id: LANDCOVER_CRITERION_ID.to_string(),
        source_stem: "landcover".to_string(),
        statistic: "categorical".to_string(),
        physical_process: "land-cover heterogeneity".to_string(),
        label: "Landcover classes".to_string(),
        help: "Refine where a cell contains many land-cover classes".to_string(),
        unit: "classes".to_string(),
        range_min: 1.0,
        range_max: 32.0,
        default_value: DEFAULT_LANDCOVER_CLASS_THRESHOLD,
    }];
    criteria.extend(threshold_criterion_catalog().into_iter().map(|criterion| {
        let source = criterion_catalog()
            .iter()
            .find(|source| source.field == criterion.source_field)
            .expect("threshold criterion source must be cataloged");
        CriterionInfo {
            id: criterion.id,
            source_stem: criterion.source_field.stem().to_string(),
            statistic: criterion.statistic.suffix().to_string(),
            physical_process: source.physical_process.to_string(),
            label: criterion.label,
            help: criterion.gui.help.to_string(),
            unit: criterion.gui.unit.to_string(),
            range_min: criterion.gui.range.0,
            range_max: criterion.gui.range.1,
            default_value: criterion.gui.default,
        }
    }));
    criteria
}

/// Return defaults and limits that affect GUI runtime decisions.
#[tauri::command]
pub(crate) fn project_capabilities() -> Result<ProjectCapabilities, String> {
    let baseline = ProjectConfig::scaffold(
        "earthmesh-capabilities",
        MeshIntentPreset::Custom,
        DomainConfig::Global,
        ResolutionSpec::Nxp(80),
    )
    .try_lower()?;
    Ok(ProjectCapabilities {
        intent_ids: INTENT_PRESETS
            .iter()
            .map(|intent| intent.id().to_string())
            .collect(),
        target_presets: INTENT_PRESETS
            .iter()
            .map(|intent| {
                let defaults = intent.defaults();
                TargetPresetInfo {
                    intent: intent.id().to_string(),
                    kind: target_kind_id(defaults.kind).to_string(),
                    cell: defaults.cell.engine_str().to_string(),
                    model_format: defaults.model_format.engine_str().to_string(),
                }
            })
            .collect(),
        // Derived from the capability registry rather than restated here: a
        // second hand-written list is a second thing to forget when a model or
        // a writer is added.
        target_compatibility: model_specialized_cells(),
        default_sea_ratio: default_mask_sea_ratio(),
        default_min_angle_deg: DEFAULT_MIN_ANGLE_DEG,
        method_c_min_base_nxp: METHOD_C_MIN_BASE_NXP,
        method_c_max_refinement_level: METHOD_C_MAX_AUTO_REFINE_LEVEL,
        default_openmp: baseline.mkgrd.openmp,
        default_niter: baseline.mkgrd.niter,
        default_surface_refine_spring_iterations: DEFAULT_SURFACE_REFINE_SPRING_ITERATIONS,
        default_atmosphere_refine_spring_iterations: DEFAULT_ATMOSPHERE_REFINE_SPRING_ITERATIONS,
        default_beta: baseline.mkgrd.beta,
        default_relax: baseline.mkgrd.relax,
        default_hfield_g: HfieldRefinementRecipe::default().g,
        method_c_defaults: Default::default(),
        harp_dv_defaults: Default::default(),
        method_c_spring_nxp1_km: METHOD_C_SPRING_NXP1_KM,
        km_per_degree_equator: KM_PER_DEGREE_EQUATOR,
    })
}

/// For each output model, the cell shapes that have a specialized writer.
fn model_specialized_cells() -> Vec<TargetCompatibilityInfo> {
    use earthmesh_project::{
        MeshCellKind, MeshDomainKind, ModelFormat, ProjectOutputDelivery, ProjectTargetTriple,
    };
    const MODELS: [ModelFormat; 6] = [
        ModelFormat::CoLM,
        ModelFormat::Fvcom,
        ModelFormat::Icon,
        ModelFormat::Mpas,
        ModelFormat::MpasOcean,
        ModelFormat::MpasSimple,
    ];
    MODELS
        .into_iter()
        .map(|model_format| TargetCompatibilityInfo {
            model_format: model_format.engine_str().to_string(),
            specialized_cells: [MeshCellKind::Tri, MeshCellKind::Hex]
                .into_iter()
                .filter(|cell| {
                    ProjectTargetTriple {
                        // Delivery depends on cell shape and model only; the
                        // kind is here because the triple carries one.
                        kind: MeshDomainKind::Land,
                        cell: *cell,
                        model_format,
                    }
                    .output_delivery()
                        == ProjectOutputDelivery::Full
                })
                .map(|cell| cell.engine_str().to_string())
                .collect(),
        })
        .collect()
}

/// Summarize a project YAML for the UI (name, intent, resolution, data layers).
#[tauri::command]
pub(crate) fn project_summary(yaml: String) -> Result<ProjectSummary, String> {
    let cfg = ProjectConfig::from_yaml(&yaml)?;
    let (nxp, approx_km, approx_degree) = match cfg.target.resolution {
        ResolutionSpec::Nxp(n) => (Some(n), None, None),
        ResolutionSpec::ApproxKm(k) => (None, Some(k), None),
        ResolutionSpec::ApproxDegree(d) => (None, None, Some(d)),
    };
    let (domain, domain_shape, bbox, watershed_path, close_format, sea_ratio) = match &cfg.domain {
        DomainConfig::Global => ("global", "global", None, None, None, None),
        DomainConfig::Regional {
            shape: RegionShape::Bbox { w, e, n, s },
            sea_ratio,
        } => (
            "regional",
            "bbox",
            Some([*w, *e, *s, *n]),
            None,
            None,
            *sea_ratio,
        ),
        DomainConfig::Regional {
            shape: RegionShape::Circle { .. },
            ..
        } => ("regional", "circle", None, None, None, None),
        DomainConfig::Regional {
            shape: RegionShape::Shapefile { path },
            sea_ratio,
        } => (
            "regional",
            "shapefile",
            None,
            Some(path.clone()),
            None,
            *sea_ratio,
        ),
        DomainConfig::Regional {
            shape: RegionShape::Close { path, format, .. },
            sea_ratio,
        } => (
            "regional",
            "close",
            None,
            Some(path.clone()),
            Some(close_format_id(*format).to_string()),
            *sea_ratio,
        ),
    };
    let cell = cfg.target.cell.engine_str().to_string();
    let quality_mode = if cell == "tri" {
        "tri-strict"
    } else {
        "hex-cgrid"
    }
    .to_string();
    let on_violation = cfg.quality.on_violation.as_str().to_string();
    // The panel has one set of lon/lat/radius controls, so it shows the head of
    // a chain. `specified_refine_circle_count` is what tells the UI a chain is
    // there at all; without it a 19-circle coastline would look like one circle.
    let specified_circles = cfg
        .refinement
        .specified_circle
        .as_ref()
        .map(|circles| circles.as_slice())
        .unwrap_or_default();
    let specified_circle = specified_circles.first();
    let specified_bbox = cfg.refinement.specified_bbox.as_ref();
    let specified_close = cfg.refinement.specified_close.as_ref();
    let domain_close_boundary = match &cfg.domain {
        DomainConfig::Regional {
            shape: RegionShape::Close { boundary, .. },
            ..
        } => Some(boundary.clone()),
        _ => None,
    };
    // The h-field is opt-in now: absent means the run refines by point+radius,
    // not that the h-field is on with defaults. Showing the defaults as if they
    // were live is how a panel tells the user something the run will not do.
    let target_triple = earthmesh_project::ProjectTargetTriple::from(&cfg.target);
    let hfield_requested = cfg
        .refinement
        .hfield
        .as_ref()
        .is_some_and(|recipe| recipe.enabled);
    let hfield_effective = cfg.refinement.hfield.clone().unwrap_or_default();
    let adaptive_effective = cfg.refinement.adaptive.clone().unwrap_or_default();
    let adaptive_active = !hfield_requested && adaptive_effective.enabled;
    let hydro = cfg.hydro_coast.as_ref();
    let mut threshold_criteria = Vec::new();
    if let Some(effective) = cfg.effective_landcover_criterion() {
        threshold_criteria.push(ThresholdCriterionSummary {
            id: effective.id.to_string(),
            source_id: effective.source_layer_id,
            statistic: "categorical".to_string(),
            source_enabled: effective.source_enabled,
            enabled: effective.enabled,
            value: effective.value,
        });
    }
    threshold_criteria.extend(
        threshold_criterion_catalog()
            .into_iter()
            .filter_map(|criterion| {
                let effective =
                    cfg.effective_threshold_criterion(criterion.source_field, criterion.statistic)?;
                Some(ThresholdCriterionSummary {
                    id: effective.id,
                    source_id: effective.source_layer_id,
                    statistic: criterion.statistic.suffix().to_string(),
                    source_enabled: effective.source_enabled,
                    enabled: effective.enabled,
                    value: effective.value,
                })
            }),
    );
    let layers = cfg
        .data_layers
        .iter()
        .map(|l| LayerSummary {
            id: l.id.clone(),
            role_kind: l.role.role_kind().to_string(),
            source_field: match l.role {
                ProjectLayerRole::Threshold(field) => Some(field.stem().to_string()),
                ProjectLayerRole::LandType => Some("landcover".to_string()),
                _ => None,
            },
            role: l.role.label(),
            path: l.path.clone(),
            enabled: l.enabled,
            threshold_value: l.threshold_value,
            wants_folder: l.role.wants_folder(),
        })
        .collect();
    Ok(ProjectSummary {
        name: cfg.metadata.name.clone(),
        authors: cfg.metadata.authors.clone(),
        description: cfg.metadata.description.clone(),
        intent: cfg.target.intent.id().to_string(),
        target_kind: target_kind_id(cfg.target.kind).to_string(),
        cell,
        quality_mode,
        model_format: cfg.target.model_format.engine_str().to_string(),
        delivery_status: match target_triple.output_delivery() {
            earthmesh_project::ProjectOutputDelivery::Full => "full",
            earthmesh_project::ProjectOutputDelivery::GridOnly => "grid_only",
        }
        .to_string(),
        delivery_skipped_reason: target_triple
            .skipped_adapter_reason()
            .map(|reason| reason.to_string()),
        domain: domain.to_string(),
        domain_shape: domain_shape.to_string(),
        nxp,
        approx_km,
        approx_degree,
        effective_nxp: cfg.try_lower()?.mkgrd.nxp,
        bbox,
        watershed_path,
        close_format,
        domain_close_boundary,
        sea_ratio,
        min_angle_deg: cfg.quality.min_angle_deg,
        auto_refine_batch_cells: cfg.quality.auto_refine_batch_cells,
        on_violation,
        refine_enabled: cfg.refinement.enabled,
        threshold_refine_enabled: cfg.refinement.threshold_enabled,
        threshold_criteria,
        refinement_backend: refinement_backend_id(cfg.refinement.backend).to_string(),
        refinement_algorithm: refinement_algorithm_id(&cfg).to_string(),
        method_c_lepp_max_cycles: cfg.refinement.method_c.max_cycles,
        method_c_lepp_target_size_tolerance: cfg.refinement.method_c.target_size_tolerance,
        method_c_lepp_maximum_neighbor_size_ratio: cfg
            .refinement
            .method_c
            .maximum_neighbor_size_ratio,
        method_c_lepp_maximum_vertices: cfg.refinement.method_c.maximum_vertices,
        method_c_lepp_maximum_insertions_per_cycle: cfg
            .refinement
            .method_c
            .maximum_insertions_per_cycle,
        method_c_lepp_maximum_path_length: cfg.refinement.method_c.maximum_path_length,
        method_c_lepp_stop_at_source_resolution: cfg.refinement.method_c.stop_at_source_resolution,
        method_c_lepp_minimum_triangle_angle_deg: cfg
            .refinement
            .method_c
            .minimum_triangle_angle_deg,
        harp_dv_max_cycles: cfg.refinement.harp_dv.max_cycles,
        harp_dv_minimum_cell_width_m: cfg.refinement.harp_dv.minimum_cell_width_m,
        harp_dv_maximum_cells: cfg.refinement.harp_dv.maximum_cells,
        harp_dv_maximum_patch_cells: cfg.refinement.harp_dv.maximum_patch_cells,
        harp_dv_maximum_neighbor_scale_ratio: cfg.refinement.harp_dv.maximum_neighbor_scale_ratio,
        harp_dv_minimum_candidate_separation_m: cfg
            .refinement
            .harp_dv
            .minimum_candidate_separation_m,
        harp_dv_maximum_vertex_degree: cfg.refinement.harp_dv.maximum_vertex_degree,
        harp_dv_minimum_triangle_angle_deg: cfg.refinement.harp_dv.minimum_triangle_angle_deg,
        harp_dv_criterion_minimum_angle_deg: cfg.refinement.harp_dv.criterion_minimum_angle_deg,
        hydro_river_refine_enabled: hydro.is_some_and(|value| value.river_refinement_enabled),
        hydro_river_width_refine_enabled: hydro
            .is_some_and(HydroCoastConfig::river_width_refinement_active),
        hydro_river_upstream_area_refine_enabled: hydro
            .is_some_and(HydroCoastConfig::river_upstream_area_refinement_active),
        hydro_river_width_threshold_m: hydro
            .map(HydroCoastConfig::effective_river_width_threshold_m),
        hydro_river_upstream_area_threshold_km2: hydro
            .map(HydroCoastConfig::effective_river_upstream_area_threshold_km2),
        hydro_coast_refine_enabled: hydro.is_some_and(|value| value.coast_refinement_enabled),
        hydro_coast_buffer_km: hydro.map(|value| value.coast_buffer_km),
        hydro_coast_land_refine_enabled: hydro
            .is_some_and(|value| value.coast_land_refinement_enabled),
        hydro_coast_ocean_refine_enabled: hydro
            .is_some_and(|value| value.coast_ocean_refinement_enabled),
        hydro_r2_width_m: hydro.map(|value| value.r2_width_m),
        hydro_r2_upa_km2: hydro.map(|value| value.r2_upa_km2),
        hydro_r3_width_m: hydro.map(|value| value.r3_width_m),
        hydro_r3_upa_km2: hydro.map(|value| value.r3_upa_km2),
        max_passes: cfg.refinement.max_passes,
        specified_refine_enabled: !specified_circles.is_empty()
            || specified_bbox.is_some()
            || specified_close.is_some(),
        specified_refine_kind: if specified_close.is_some() {
            "close"
        } else if specified_bbox.is_some() {
            "bbox"
        } else {
            "radius"
        }
        .to_string(),
        specified_refine_lon: specified_circle.map(|c| c.lon),
        specified_refine_lat: specified_circle.map(|c| c.lat),
        specified_refine_radius_km: specified_circle.map(|c| c.radius_km),
        specified_refine_circle_count: specified_circles.len(),
        specified_refine_bbox: specified_bbox.map(|b| [b.w, b.e, b.s, b.n]),
        specified_refine_path: specified_close.map(|c| c.path.clone()),
        specified_refine_close_boundary: specified_close.map(|c| c.boundary.clone()),
        hfield_enabled: hfield_requested,
        adaptive_enabled: adaptive_active,
        adaptive_max_level: adaptive_effective.max_level,
        adaptive_coastline: adaptive_effective.coastline,
        hfield_g: Some(hfield_effective.g),
        hfield_max_level: Some(hfield_effective.max_level),
        hfield_base_m: hfield_effective.base_m,
        expert_nxp: cfg.expert.nxp,
        expert_openmp: cfg.expert.openmp,
        expert_niter: cfg.expert.niter,
        expert_niter_refine: cfg.expert.niter_refine,
        expert_max_iter_spc: cfg.expert.max_iter_spc,
        expert_max_iter_cal: cfg.expert.max_iter_cal,
        expert_halo: cfg.expert.halo.clone(),
        expert_max_transition_row: cfg.expert.max_transition_row.clone(),
        expert_set_dis_type: cfg.expert.set_dis_type.clone(),
        expert_num_rc: cfg.expert.num_rc,
        expert_vertex_pretect_layers: cfg.expert.vertex_pretect_layers,
        expert_spring_global_type: cfg.expert.spring_global_type,
        expert_spring_regional_type: cfg.expert.spring_regional_type,
        expert_beta: cfg.expert.beta,
        expert_relax: cfg.expert.relax,
        expert_weak_concav_eliminate: cfg.expert.weak_concav_eliminate,
        expert_isolated_ocean: cfg.expert.isolated_ocean,
        layers,
    })
}

fn refinement_backend_id(backend: earthmesh_project::RefinementBackend) -> &'static str {
    match backend {
        earthmesh_project::RefinementBackend::MethodC => "method_c",
        earthmesh_project::RefinementBackend::RedGreen => "red_green",
        earthmesh_project::RefinementBackend::HarpDv => "harp_dv",
    }
}

fn refinement_algorithm_id(cfg: &ProjectConfig) -> &'static str {
    match cfg.refinement.backend {
        earthmesh_project::RefinementBackend::MethodC
            if cfg.refinement.method_c.algorithm
                == earthmesh_project::MethodCAlgorithm::LeppDelaunay =>
        {
            "lepp_delaunay"
        }
        earthmesh_project::RefinementBackend::MethodC => "method_c",
        earthmesh_project::RefinementBackend::RedGreen => "red_green",
        earthmesh_project::RefinementBackend::HarpDv => "harp_dv",
    }
}

fn target_kind_id(kind: MeshDomainKind) -> &'static str {
    match kind {
        MeshDomainKind::Land => "land",
        MeshDomainKind::Ocean => "ocean",
        MeshDomainKind::Atmosphere => "atmosphere",
        MeshDomainKind::Coupled => "coupled",
        MeshDomainKind::Earth => "earth",
    }
}

fn close_format_id(format: CloseMaskFormat) -> &'static str {
    match format {
        CloseMaskFormat::PolygonShp => "polygon_shp",
        CloseMaskFormat::Nml => "nml",
        CloseMaskFormat::Netcdf => "netcdf",
        CloseMaskFormat::LonLatText => "lonlat_text",
    }
}
