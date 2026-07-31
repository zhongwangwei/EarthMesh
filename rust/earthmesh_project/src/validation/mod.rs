use crate::{
    criterion_catalog, threshold_criterion_by_id, CoupledMeshConfig, DomainConfig, ExpertOverrides,
    HfieldRefinementRecipe, HydroCoastConfig, MeshDomainKind, MeshTargetConfig, ProjectConfig,
    ProjectDataLayer, ProjectLayerRole, ProjectTargetTriple, QualityConfig, RefinementRecipe,
    RegionShape, ResolutionSpec, SpecifiedBboxRefinement, SpecifiedCircleRefinement,
    SpecifiedCloseRefinement, ThresholdCriterionConfig, ThresholdField, ThresholdStatistic,
    LANDCOVER_CRITERION_ID, METHOD_C_MAX_AUTO_REFINE_LEVEL, PROJECT_SCHEMA_VERSION,
};
use std::collections::HashSet;

impl ProjectConfig {
    pub fn from_json(s: &str) -> Result<Self, String> {
        let config: Self = serde_json::from_str(s).map_err(|e| e.to_string())?;
        Self::validated(config)
    }

    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn from_yaml(s: &str) -> Result<Self, String> {
        let config: Self = serde_yaml::from_str(s).map_err(|e| e.to_string())?;
        Self::validated(config)
    }

    pub fn to_yaml(&self) -> Result<String, String> {
        serde_yaml::to_string(self).map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported project schema_version {:?}; expected {PROJECT_SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        if self.metadata.name.trim().is_empty() {
            return Err("project metadata.name must not be empty".to_string());
        }
        if matches!(self.metadata.name.trim(), "." | "..") {
            return Err("project metadata.name must not be '.' or '..'".to_string());
        }
        if self.metadata.name.chars().any(|c| matches!(c, '/' | '\\')) {
            return Err("project metadata.name must not contain path separators".to_string());
        }
        self.domain.validate()?;
        self.target.validate()?;
        self.validate_data_layers()?;
        self.validate_landtype_requirements()?;
        self.refinement.validate()?;
        self.validate_refinement_sources()?;
        self.quality.validate()?;
        self.expert.validate()?;
        self.validate_expert_refinement_levels()?;
        if let Some(hydro_coast) = &self.hydro_coast {
            hydro_coast.validate()?;
            self.hydro_execution_plan()?;
        }
        if let Some(coupling) = &self.coupling {
            coupling.validate(self)?;
        }
        Ok(())
    }

    fn validated(config: Self) -> Result<Self, String> {
        config.validate()?;
        Ok(config)
    }

    fn validate_data_layers(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        let mut threshold_fields = HashSet::new();
        let mut enabled_landtype_seen = false;
        for layer in &self.data_layers {
            layer.validate()?;
            if layer.enabled && layer.role == ProjectLayerRole::LandType {
                if enabled_landtype_seen {
                    return Err("enabled LandType source is duplicated".to_string());
                }
                enabled_landtype_seen = true;
            }
            if let ProjectLayerRole::Threshold(field) = layer.role {
                self.validate_threshold_layer(layer, field)?;
                if layer.enabled && !threshold_fields.insert(field.stem()) {
                    return Err(format!(
                        "enabled threshold field '{}' is duplicated",
                        field.stem()
                    ));
                }
            }
            if !ids.insert(layer.id.as_str()) {
                return Err(format!("data layer id '{}' is duplicated", layer.id));
            }
        }
        let mut criterion_ids = HashSet::new();
        for criterion in &self.refinement.threshold_criteria {
            criterion.validate()?;
            if !criterion_ids.insert(criterion.id.as_str()) {
                return Err(format!(
                    "threshold criterion id '{}' is duplicated",
                    criterion.id
                ));
            }
            if criterion.id == LANDCOVER_CRITERION_ID {
                if matches!(criterion.value, Some(value) if value <= 0.0) {
                    return Err("landcover class threshold must be > 0".to_string());
                }
                if !self
                    .data_layers
                    .iter()
                    .any(|layer| layer.role == ProjectLayerRole::LandType)
                {
                    return Err(
                        "threshold criterion 'landcover' has no matching LandType data source"
                            .to_string(),
                    );
                }
                continue;
            }
            let spec = threshold_criterion_by_id(&criterion.id)
                .ok_or_else(|| format!("unknown threshold criterion '{}'", criterion.id))?;
            if !self
                .data_layers
                .iter()
                .any(|layer| layer.role == ProjectLayerRole::Threshold(spec.source_field))
            {
                return Err(format!(
                    "threshold criterion '{}' has no matching '{}' data source",
                    criterion.id,
                    spec.source_field.stem()
                ));
            }
        }
        Ok(())
    }

    fn validate_landtype_requirements(&self) -> Result<(), String> {
        let requires_surface_carve = matches!(
            self.target.kind,
            MeshDomainKind::Land | MeshDomainKind::Ocean
        );
        let requires_coupling_source = self.target.kind == MeshDomainKind::Coupled
            && (self.coupling.is_some() || self.hydro_coast.is_some());
        if !requires_surface_carve && !requires_coupling_source {
            return Ok(());
        }
        if self.data_layers.iter().any(|layer| {
            layer.role == ProjectLayerRole::LandType
                && layer.enabled
                && !layer.path.trim().is_empty()
        }) {
            Ok(())
        } else {
            Err(format!(
                "{:?} target requires an enabled landtype layer for surface carve/coupling",
                self.target.kind
            ))
        }
    }

    fn validate_refinement_sources(&self) -> Result<(), String> {
        if !self.refinement.enabled {
            return Ok(());
        }
        if self.has_specified_refinement_source() || self.has_calculated_refinement_source() {
            Ok(())
        } else {
            Err(
                "refinement is enabled but no refinement source is enabled (add a threshold or specified bbox/circle/close source, or disable refinement)"
                    .to_string(),
            )
        }
    }

    fn has_specified_refinement_source(&self) -> bool {
        self.refinement.specified_circle.is_some()
            || self.refinement.specified_bbox.is_some()
            || self.refinement.specified_close.is_some()
    }

    fn has_calculated_refinement_source(&self) -> bool {
        self.refinement.threshold_enabled
            && self.data_layers.iter().any(|layer| {
                if !layer.enabled {
                    return false;
                }
                match layer.role {
                    ProjectLayerRole::LandType => self
                        .effective_landcover_criterion()
                        .is_some_and(|criterion| criterion.enabled),
                    ProjectLayerRole::Threshold(field) => {
                        self.threshold_statistic_enabled(field, ThresholdStatistic::Mean)
                            || self.threshold_statistic_enabled(field, ThresholdStatistic::Std)
                    }
                    ProjectLayerRole::MeritHydro => {
                        self.hydro_coast.as_ref().is_some_and(|hydro| {
                            hydro.has_river_refinement()
                                || (hydro.coast_refinement_enabled
                                    && (hydro.coast_land_refinement_enabled
                                        || hydro.coast_ocean_refinement_enabled))
                        })
                    }
                    ProjectLayerRole::Cama => false,
                }
            })
    }

    pub(crate) fn threshold_statistic_enabled(
        &self,
        field: ThresholdField,
        statistic: ThresholdStatistic,
    ) -> bool {
        self.effective_threshold_criterion(field, statistic)
            .is_some_and(|criterion| criterion.enabled)
    }

    fn validate_expert_refinement_levels(&self) -> Result<(), String> {
        if !self.refinement.enabled {
            return Ok(());
        }
        if self.has_specified_refinement_source() && self.expert.max_iter_spc == Some(0) {
            return Err(
                "expert max_iter_spc override must be > 0 when specified refinement is enabled"
                    .to_string(),
            );
        }
        if self.has_calculated_refinement_source() && self.expert.max_iter_cal == Some(0) {
            return Err(
                "expert max_iter_cal override must be > 0 when calculated refinement is enabled"
                    .to_string(),
            );
        }
        Ok(())
    }

    fn validate_threshold_layer(
        &self,
        layer: &ProjectDataLayer,
        field: ThresholdField,
    ) -> Result<(), String> {
        if !layer.enabled {
            return Ok(());
        }
        criterion_catalog()
            .iter()
            .find(|criterion| criterion.field == field)
            .ok_or_else(|| format!("unknown threshold field '{}'", field.stem()))?;
        Ok(())
    }
}

impl DomainConfig {
    fn validate(&self) -> Result<(), String> {
        match self {
            DomainConfig::Global => Ok(()),
            DomainConfig::Regional { shape, sea_ratio } => {
                shape.validate()?;
                if let Some(ratio) = sea_ratio {
                    if !ratio.is_finite() {
                        return Err("domain sea_ratio must be finite".to_string());
                    }
                    if !(0.0..=1.0).contains(ratio) {
                        return Err("domain sea_ratio must be between 0 and 1".to_string());
                    }
                }
                Ok(())
            }
        }
    }
}

impl RegionShape {
    fn validate(&self) -> Result<(), String> {
        match self {
            RegionShape::Bbox { w, e, n, s } => {
                if !w.is_finite() || !e.is_finite() || !n.is_finite() || !s.is_finite() {
                    return Err("bbox coordinates must be finite".to_string());
                }
                if !(-180.0..=180.0).contains(w) || !(-180.0..=180.0).contains(e) {
                    return Err("bbox longitudes must be between -180 and 180".to_string());
                }
                if !(-90.0..=90.0).contains(s) || !(-90.0..=90.0).contains(n) {
                    return Err("bbox latitudes must be between -90 and 90".to_string());
                }
                if w == e {
                    return Err("bbox west and east must differ".to_string());
                }
                if n <= s {
                    return Err("bbox south must be < north".to_string());
                }
                Ok(())
            }
            RegionShape::Circle {
                lon,
                lat,
                radius_km,
            } => {
                if !lon.is_finite() || !lat.is_finite() || !radius_km.is_finite() {
                    return Err("circle coordinates and radius must be finite".to_string());
                }
                if !(-180.0..=180.0).contains(lon) {
                    return Err("circle longitude must be between -180 and 180".to_string());
                }
                if !(-90.0..=90.0).contains(lat) {
                    return Err("circle latitude must be between -90 and 90".to_string());
                }
                if *radius_km <= 0.0 {
                    return Err("circle radius_km must be > 0".to_string());
                }
                const MAX_MINOR_CIRCLE_RADIUS_KM: f64 =
                    std::f64::consts::FRAC_PI_2 * (earthmesh_core::EARTH_RADIUS_METERS / 1000.0);
                if *radius_km > MAX_MINOR_CIRCLE_RADIUS_KM {
                    return Err(format!(
                        "circle radius_km must be <= {MAX_MINOR_CIRCLE_RADIUS_KM:.3} for minor-hemisphere domains"
                    ));
                }
                Ok(())
            }
            RegionShape::Shapefile { path } => {
                if path.trim().is_empty() {
                    return Err("watershed shapefile path must not be empty".to_string());
                }
                let lower = path.to_ascii_lowercase();
                if !lower.ends_with(".shp") {
                    return Err("watershed domain path must end with .shp".to_string());
                }
                Ok(())
            }
            RegionShape::Close {
                path,
                format,
                boundary,
            } => {
                boundary.validate()?;
                if path.trim().is_empty() {
                    return Err("close domain path must not be empty".to_string());
                }
                let lower = path.to_ascii_lowercase();
                let ok = match format {
                    crate::CloseMaskFormat::Nml => lower.ends_with(".nml"),
                    crate::CloseMaskFormat::Netcdf => {
                        lower.ends_with(".nc") || lower.ends_with(".nc4")
                    }
                    crate::CloseMaskFormat::PolygonShp => lower.ends_with(".shp"),
                    crate::CloseMaskFormat::LonLatText => {
                        lower.ends_with(".txt") || lower.ends_with(".csv")
                    }
                };
                if !ok {
                    return Err("close domain path extension does not match its format".to_string());
                }
                Ok(())
            }
        }
    }
}

impl QualityConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.min_angle_deg.is_finite() {
            return Err("quality min_angle_deg must be finite".to_string());
        }
        if self.min_angle_deg <= 0.0 {
            return Err("quality min_angle_deg must be > 0".to_string());
        }
        if self.min_angle_deg >= 180.0 {
            return Err("quality min_angle_deg must be < 180".to_string());
        }
        if self.auto_refine_batch_cells == 0 {
            return Err("quality auto_refine_batch_cells must be > 0".to_string());
        }
        if self.auto_refine_batch_cells > i32::MAX as usize {
            return Err("quality auto_refine_batch_cells exceeds the engine limit".to_string());
        }
        Ok(())
    }
}

impl ProjectDataLayer {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("data layer id must not be empty".to_string());
        }
        if self.id.trim() != self.id {
            return Err("data layer id must not have leading or trailing whitespace".to_string());
        }
        if self.enabled && self.path.trim().is_empty() {
            return Err(format!(
                "data layer '{}' is enabled but has no path",
                self.id
            ));
        }
        if self.threshold_value.is_some()
            && !matches!(
                self.role,
                ProjectLayerRole::Threshold(_) | ProjectLayerRole::LandType
            )
        {
            return Err(format!(
                "data layer '{}' has a threshold value but is not a refinement layer",
                self.id
            ));
        }
        if matches!(self.threshold_value, Some(value) if !value.is_finite()) {
            return Err(format!(
                "data layer '{}' threshold value must be finite",
                self.id
            ));
        }
        if matches!(self.role, ProjectLayerRole::LandType)
            && matches!(self.threshold_value, Some(value) if value <= 0.0)
        {
            return Err(format!(
                "data layer '{}' landcover class threshold must be > 0",
                self.id
            ));
        }
        Ok(())
    }
}

impl ThresholdCriterionConfig {
    fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() || self.id.trim() != self.id {
            return Err(
                "threshold criterion id must be non-empty without surrounding whitespace"
                    .to_string(),
            );
        }
        if matches!(self.value, Some(value) if !value.is_finite()) {
            return Err(format!(
                "threshold criterion '{}' value must be finite",
                self.id
            ));
        }
        Ok(())
    }
}

impl RefinementRecipe {
    fn validate(&self) -> Result<(), String> {
        if let Some(circle) = &self.specified_circle {
            circle.validate()?;
        }
        if let Some(bbox) = &self.specified_bbox {
            bbox.validate()?;
        }
        if let Some(close) = &self.specified_close {
            close.validate()?;
        }
        if let Some(hfield) = &self.hfield {
            hfield.validate()?;
        }
        let shape_count = usize::from(self.specified_circle.is_some())
            + usize::from(self.specified_bbox.is_some())
            + usize::from(self.specified_close.is_some());
        if shape_count > 1 {
            return Err("only one specified refinement shape may be enabled".to_string());
        }
        if !self.enabled {
            return Ok(());
        }
        if self.max_passes == 0 {
            return Err("refinement max_passes must be > 0 when refinement is enabled".to_string());
        }
        if self.max_passes > METHOD_C_MAX_AUTO_REFINE_LEVEL {
            return Err(format!(
                "refinement max_passes must be <= {METHOD_C_MAX_AUTO_REFINE_LEVEL}"
            ));
        }
        Ok(())
    }
}

impl HfieldRefinementRecipe {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if !self.g.is_finite() || self.g <= 0.0 {
            return Err("h-field gradation g must be positive".to_string());
        }
        if self.max_level > 5 {
            return Err("h-field max_level must be in 0..=5".to_string());
        }
        if matches!(self.base_m, Some(base) if !base.is_finite() || base <= 0.0) {
            return Err("h-field base_m must be positive when set".to_string());
        }
        match (self.origin_lon, self.origin_lat) {
            (None, None) => {}
            (Some(lon), Some(lat))
                if lon.is_finite()
                    && lat.is_finite()
                    && (-180.0..=180.0).contains(&lon)
                    && (-90.0..=90.0).contains(&lat) => {}
            (Some(_), Some(_)) => {
                return Err("h-field origin must be valid WGS84 lon/lat".to_string())
            }
            _ => return Err("h-field origin_lon and origin_lat must be set together".to_string()),
        }
        Ok(())
    }
}

impl SpecifiedCircleRefinement {
    fn validate(&self) -> Result<(), String> {
        if !self.lon.is_finite() || !self.lat.is_finite() || !self.radius_km.is_finite() {
            return Err("specified refinement circle values must be finite".to_string());
        }
        if !(-180.0..=180.0).contains(&self.lon) {
            return Err("specified refinement longitude must be between -180 and 180".to_string());
        }
        if !(-90.0..=90.0).contains(&self.lat) {
            return Err("specified refinement latitude must be between -90 and 90".to_string());
        }
        if self.radius_km <= 0.0 {
            return Err("specified refinement radius_km must be > 0".to_string());
        }
        Ok(())
    }
}

impl SpecifiedBboxRefinement {
    fn validate(&self) -> Result<(), String> {
        RegionShape::Bbox {
            w: self.w,
            e: self.e,
            s: self.s,
            n: self.n,
        }
        .validate()
        .map_err(|e| format!("specified refinement {e}"))
    }
}

impl SpecifiedCloseRefinement {
    fn validate(&self) -> Result<(), String> {
        self.boundary.validate()?;
        let path = self.path.trim();
        if path.is_empty() {
            return Err("specified refinement close path must not be empty".to_string());
        }
        let lower = path.to_ascii_lowercase();
        if [".shp", ".nml", ".nc", ".nc4", ".txt", ".csv"]
            .iter()
            .any(|extension| lower.ends_with(extension))
        {
            Ok(())
        } else {
            Err(
                "specified refinement close path must end with .shp, .nml, .nc, .nc4, .txt, or .csv"
                    .to_string(),
            )
        }
    }
}

impl MeshTargetConfig {
    fn validate(&self) -> Result<(), String> {
        match self.resolution {
            ResolutionSpec::Nxp(n) if n <= 0 => {
                Err("target resolution Nxp must be > 0".to_string())
            }
            ResolutionSpec::ApproxKm(km) if !km.is_finite() => {
                Err("target resolution ApproxKm must be finite".to_string())
            }
            ResolutionSpec::ApproxKm(km) if km <= 0.0 => {
                Err("target resolution ApproxKm must be > 0".to_string())
            }
            ResolutionSpec::ApproxDegree(degrees) if !degrees.is_finite() => {
                Err("target resolution ApproxDegree must be finite".to_string())
            }
            ResolutionSpec::ApproxDegree(degrees) if degrees <= 0.0 => {
                Err("target resolution ApproxDegree must be > 0".to_string())
            }
            _ => Ok(()),
        }?;
        if let Some(reason) = ProjectTargetTriple::from(self).rejection_reason() {
            Err(reason.message().to_string())
        } else {
            Ok(())
        }
    }
}

impl ExpertOverrides {
    fn validate(&self) -> Result<(), String> {
        if matches!(self.nxp, Some(n) if n <= 0) {
            return Err("expert nxp override must be > 0".to_string());
        }
        if matches!(self.openmp, Some(n) if n <= 0) {
            return Err("expert openmp override must be > 0".to_string());
        }
        if matches!(self.niter, Some(n) if n <= 0) {
            return Err("expert niter override must be > 0".to_string());
        }
        if matches!(self.niter_refine, Some(n) if n <= 0) {
            return Err("expert niter_refine override must be > 0".to_string());
        }
        let max_refine_level = i32::from(METHOD_C_MAX_AUTO_REFINE_LEVEL);
        if matches!(self.max_iter_spc, Some(n) if !(0..=max_refine_level).contains(&n)) {
            return Err(format!(
                "expert max_iter_spc override must be between 0 and {METHOD_C_MAX_AUTO_REFINE_LEVEL}"
            ));
        }
        if matches!(self.max_iter_cal, Some(n) if !(0..=max_refine_level).contains(&n)) {
            return Err(format!(
                "expert max_iter_cal override must be between 0 and {METHOD_C_MAX_AUTO_REFINE_LEVEL}"
            ));
        }
        validate_expert_i32_list(&self.halo, "expert HALO override")?;
        validate_expert_i32_list(
            &self.max_transition_row,
            "expert max_transition_row override",
        )?;
        if let Some(set_dis_type) = &self.set_dis_type {
            match set_dis_type.as_str() {
                "linear" | "nonlinear1" | "nonlinear2" | "nonlinear3" => {}
                _ => {
                    return Err(
                        "expert set_dis_type must be linear/nonlinear1/nonlinear2/nonlinear3"
                            .to_string(),
                    );
                }
            }
        }
        if matches!(self.num_rc, Some(n) if n < 0) {
            return Err("expert num_rc override must be >= 0".to_string());
        }
        if matches!(self.vertex_pretect_layers, Some(n) if n <= 0) {
            return Err("expert vertex_pretect_layers override must be > 0".to_string());
        }
        if matches!(self.spring_global_type, Some(n) if !(0..=1).contains(&n)) {
            return Err("expert spring_global_type override must be 0 or 1".to_string());
        }
        if matches!(self.spring_regional_type, Some(n) if !(0..=2).contains(&n)) {
            return Err("expert spring_regional_type override must be 0, 1, or 2".to_string());
        }
        if self.spring_global_type.unwrap_or(0) > 0 && self.spring_regional_type.unwrap_or(0) > 0 {
            return Err(
                "only one of expert spring_global_type and spring_regional_type can be > 0"
                    .to_string(),
            );
        }
        if matches!(self.beta, Some(v) if !v.is_finite() || v <= 0.0) {
            return Err("expert beta override must be finite and > 0".to_string());
        }
        if matches!(self.relax, Some(v) if !v.is_finite() || v <= 0.0) {
            return Err("expert relax override must be finite and > 0".to_string());
        }
        Ok(())
    }
}

fn validate_expert_i32_list(values: &Option<Vec<i32>>, label: &str) -> Result<(), String> {
    let Some(values) = values else {
        return Ok(());
    };
    if values.is_empty() || values.len() > 9 {
        return Err(format!("{label} must contain 1 to 9 values"));
    }
    if values.iter().any(|value| *value < 0) {
        return Err(format!("{label} values must be >= 0"));
    }
    Ok(())
}

impl HydroCoastConfig {
    fn validate(&self) -> Result<(), String> {
        if self.merit_root.trim().is_empty() {
            return Err("hydro_coast merit_root must not be empty".to_string());
        }
        if matches!(&self.cama_root, Some(path) if path.trim().is_empty()) {
            return Err("hydro_coast cama_root must not be empty when set".to_string());
        }
        if !self.r3_width_m.is_finite() || !self.r2_width_m.is_finite() {
            return Err("hydro_coast widths must be finite".to_string());
        }
        if self.r3_width_m <= 0.0 || self.r2_width_m <= 0.0 {
            return Err("hydro_coast widths must be > 0".to_string());
        }
        if self.r3_width_m < self.r2_width_m {
            return Err("hydro_coast r3_width_m must be >= r2_width_m".to_string());
        }
        if !self.r3_upa_km2.is_finite() || !self.r2_upa_km2.is_finite() {
            return Err("hydro_coast upstream areas must be finite".to_string());
        }
        if self.r3_upa_km2 <= 0.0 || self.r2_upa_km2 <= 0.0 {
            return Err("hydro_coast upstream areas must be > 0".to_string());
        }
        if self.r3_upa_km2 < self.r2_upa_km2 {
            return Err("hydro_coast r3_upa_km2 must be >= r2_upa_km2".to_string());
        }
        if let Some(value) = self.river_width_threshold_m {
            if !value.is_finite() || value <= 0.0 {
                return Err(
                    "hydro_coast river_width_threshold_m must be finite and > 0".to_string()
                );
            }
            if value < self.r2_width_m {
                return Err(
                    "hydro_coast river_width_threshold_m must be >= the supported river width minimum"
                        .to_string(),
                );
            }
        }
        if let Some(value) = self.river_upstream_area_threshold_km2 {
            if !value.is_finite() || value <= 0.0 {
                return Err(
                    "hydro_coast river_upstream_area_threshold_km2 must be finite and > 0"
                        .to_string(),
                );
            }
            if value < self.r2_upa_km2 {
                return Err(
                    "hydro_coast river_upstream_area_threshold_km2 must be >= the supported upstream area minimum"
                        .to_string(),
                );
            }
        }
        if !self.coast_buffer_km.is_finite() || self.coast_buffer_km < 0.0 {
            return Err("hydro_coast coast_buffer_km must be finite and >= 0".to_string());
        }
        if self.coast_buffer_km > 1_000.0 {
            return Err("hydro_coast coast_buffer_km must be <= 1000".to_string());
        }
        if self.merit_stride != 1 {
            return Err(
                "hydro_coast merit_stride must be 1 for physical coast adjacency and production coupling"
                    .to_string(),
            );
        }
        Ok(())
    }
}

impl CoupledMeshConfig {
    fn validate(&self, project: &ProjectConfig) -> Result<(), String> {
        if matches!(&self.cama_root, Some(path) if path.trim().is_empty()) {
            return Err("coupling cama_root must not be empty when set".to_string());
        }
        if project.target.kind != MeshDomainKind::Coupled {
            return Err("coupling config requires the coupled target kind".to_string());
        }
        if !project.data_layers.iter().any(|layer| {
            layer.enabled
                && !layer.path.trim().is_empty()
                && layer.role == ProjectLayerRole::LandType
        }) {
            return Err(
                "coupling config requires an enabled landtype layer for LOCmesh point sampling"
                    .to_string(),
            );
        }
        if self.identify_river_mouth
            && !matches!(
                self.cama_root.as_deref(),
                Some(path) if !path.trim().is_empty()
            )
        {
            return Err(
                "coupling river-mouth identification requires coupling.cama_root".to_string(),
            );
        }
        Ok(())
    }
}
