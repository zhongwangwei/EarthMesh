use crate::{
    criterion_catalog, engine_mapping::DEPRECATED_OLAM_MODEL_FORMAT_ERROR, DomainConfig,
    ExpertOverrides, HydroCoastConfig, MeshDomainKind, MeshTargetConfig, ModelFormat,
    ProjectConfig, ProjectDataLayer, ProjectLayerRole, QualityConfig, RefinementRecipe,
    RegionShape, ResolutionSpec, SpecifiedBboxRefinement, SpecifiedCircleRefinement,
    SpecifiedCloseRefinement, ThresholdField,
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
        if self.schema_version.trim().is_empty() {
            return Err("project schema_version must not be empty".to_string());
        }
        if self.metadata.name.trim().is_empty() {
            return Err("project metadata.name must not be empty".to_string());
        }
        if self.metadata.name.chars().any(|c| matches!(c, '/' | '\\')) {
            return Err("project metadata.name must not contain path separators".to_string());
        }
        self.domain.validate()?;
        self.target.validate()?;
        self.validate_data_layers()?;
        self.refinement.validate()?;
        self.quality.validate()?;
        self.expert.validate()?;
        if let Some(hydro_coast) = &self.hydro_coast {
            hydro_coast.validate()?;
        }
        Ok(())
    }

    fn validated(config: Self) -> Result<Self, String> {
        config.validate()?;
        Ok(config)
    }

    fn validate_data_layers(&self) -> Result<(), String> {
        let mut ids = HashSet::new();
        for layer in &self.data_layers {
            layer.validate()?;
            if let ProjectLayerRole::Threshold(field) = layer.role {
                self.validate_threshold_layer(layer, field)?;
            }
            if !ids.insert(layer.id.as_str()) {
                return Err(format!("data layer id '{}' is duplicated", layer.id));
            }
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
        let criterion = criterion_catalog()
            .iter()
            .find(|criterion| criterion.field == field)
            .ok_or_else(|| format!("unknown threshold field '{}'", field.stem()))?;
        if !criterion.applicable.contains(&self.target.kind) {
            return Err(format!(
                "threshold layer '{}' is not applicable to {:?} targets",
                layer.id, self.target.kind
            ));
        }
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
                if w >= e {
                    return Err("bbox west must be < east".to_string());
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
            RegionShape::Close { path, format } => {
                if path.trim().is_empty() {
                    return Err("close domain path must not be empty".to_string());
                }
                let lower = path.to_ascii_lowercase();
                let ok = match format {
                    crate::CloseMaskFormat::PolygonShp => lower.ends_with(".shp"),
                    crate::CloseMaskFormat::Nml => lower.ends_with(".nml"),
                    crate::CloseMaskFormat::Netcdf => {
                        lower.ends_with(".nc") || lower.ends_with(".nc4")
                    }
                    crate::CloseMaskFormat::LonLatText => {
                        lower.ends_with(".txt") || lower.ends_with(".csv")
                    }
                };
                if !ok {
                    return Err(
                        "close domain path extension does not match selected format".to_string()
                    );
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
        if self.max_passes > 9 {
            return Err("refinement max_passes must be <= 9".to_string());
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
        RegionShape::Shapefile {
            path: self.path.clone(),
        }
        .validate()
        .map_err(|e| format!("specified refinement {e}"))
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
        if self.model_format == ModelFormat::Olam {
            return Err(DEPRECATED_OLAM_MODEL_FORMAT_ERROR.to_string());
        }
        match (self.kind, self.model_format) {
            (
                MeshDomainKind::Land | MeshDomainKind::Earth | MeshDomainKind::Coupled,
                ModelFormat::CoLM,
            )
            | (MeshDomainKind::Ocean, ModelFormat::Fvcom)
            | (MeshDomainKind::Atmosphere, ModelFormat::Mpas | ModelFormat::MpasSimple) => Ok(()),
            (MeshDomainKind::Land, _) => Err("land target model_format must be CoLM".to_string()),
            (MeshDomainKind::Earth, _) => Err("earth target model_format must be CoLM".to_string()),
            (MeshDomainKind::Coupled, _) => {
                Err("coupled target model_format must be CoLM".to_string())
            }
            (MeshDomainKind::Ocean, _) => {
                Err("ocean target model_format must be FVCOM".to_string())
            }
            (MeshDomainKind::Atmosphere, _) => {
                Err("atmosphere target model_format must be MPAS or MPAS-Simple".to_string())
            }
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
        if matches!(self.max_iter_spc, Some(n) if !(0..=9).contains(&n)) {
            return Err("expert max_iter_spc override must be between 0 and 9".to_string());
        }
        if matches!(self.max_iter_cal, Some(n) if !(0..=9).contains(&n)) {
            return Err("expert max_iter_cal override must be between 0 and 9".to_string());
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
    if values.is_empty() || values.len() > 10 {
        return Err(format!("{label} must contain 1 to 10 values"));
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
        Ok(())
    }
}
