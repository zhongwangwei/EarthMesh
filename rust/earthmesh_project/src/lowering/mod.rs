use crate::{
    criterion_catalog, degree_to_nxp, km_to_nxp, DomainConfig, HfieldRefinementRecipe,
    ProjectConfig, ProjectDataLayer, ProjectLayerRole, RegionShape, ResolutionSpec, ThresholdField,
};
use earthmesh_core::{
    DataLayerConfig, DataLayersNamelist, EarthmeshConfig, QualityNamelist, RefineConfig,
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
                format!(
                    "&hfield\n   NL%hfield_on = .true.\n   NL%hfield_g = {}\n   NL%hfield_max_level = {}\n{}/\n\n",
                    recipe.g, recipe.max_level, base_line
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
        let layers = self
            .data_layers
            .iter()
            .map(|l| DataLayerConfig {
                id: l.id.clone(),
                role: l.role.to_core(),
                path: l.path.clone(),
                var: None,
                enabled: l.enabled,
                required: matches!(l.role, ProjectLayerRole::LandType),
            })
            .collect();
        DataLayersNamelist { layers }
    }

    fn quality_namelist(&self) -> QualityNamelist {
        QualityNamelist {
            min_angle_warn_deg: self.quality.min_angle_deg,
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
        mkgrd.mesh_type = self.target.kind.engine_str().to_string();
        mkgrd.mode_grid = self.target.cell.engine_str().to_string();
        mkgrd.output_format = self.target.model_format.try_engine_str()?.to_string();
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
                    RegionShape::Bbox { .. } => "bbox",
                    RegionShape::Circle { .. } => "circle",
                    RegionShape::Shapefile { path } => {
                        mkgrd.mask_domain_fprefix = path.clone();
                        "shapefile"
                    }
                    RegionShape::Close { path, .. } => {
                        mkgrd.mask_domain_fprefix = path.clone();
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
        dl.lower_into(&mut mkgrd, &mut refine);
        apply_threshold_defaults(&mut refine, &self.data_layers);
        // Refinement actually runs only when a threshold (refine_cal) or
        // specified-mask (refine_spc) layer supplies data. Landcover/hydro layers
        // set inputs but DON'T drive refinement - turning `refine` on for them
        // sends a data-less run down the OLAM specified-refine path, which then
        // errors ("requires refine_spc/refine_cal/native..."). That is exactly why a
        // land/ocean mesh with only landcover failed to run. Gate the recipe
        // toggle on a real refinement source so such a mesh runs uniform instead.
        if self.refinement.specified_circle.is_some() {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "circle".to_string();
        }
        if self.refinement.specified_bbox.is_some() {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "bbox".to_string();
        }
        if self.refinement.specified_close.is_some() {
            refine.refine_spc = true;
            refine.mask_refine_spc_type = "close".to_string();
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
        // mutually exclusive while deriving the OLAM-compatible choice from grid/domain.
        if mkgrd.mode_grid != "tri" {
            refine.is_transition = true;
            refine.spring_global_type = if mkgrd.mask_domain_global { 1 } else { 0 };
            refine.spring_regional_type = if mkgrd.mask_domain_global { 0 } else { 1 };
        }

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

        Ok(LoweredProject {
            mkgrd,
            refine,
            hfield,
            data_layers: dl,
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
    for (slot, value) in target.iter_mut().zip(values.iter().copied()) {
        *slot = value;
    }
}

fn apply_threshold_defaults(refine: &mut RefineConfig, layers: &[ProjectDataLayer]) {
    for layer in layers {
        let ProjectLayerRole::Threshold(field) = layer.role else {
            continue;
        };
        if !layer.enabled || layer.path.trim().is_empty() {
            continue;
        }
        let Some(value) = criterion_catalog()
            .iter()
            .find(|criterion| criterion.field == field)
            .map(|criterion| criterion.gui.default)
        else {
            continue;
        };
        match field {
            ThresholdField::Lai => set_pair(&mut refine.th_onelayer_lnd, 0, value),
            ThresholdField::Slope => set_pair(&mut refine.th_onelayer_lnd, 2, value),
            ThresholdField::Ks => set_layer_pair(&mut refine.th_twolayer_lnd, 0, value),
            ThresholdField::KSolids => set_layer_pair(&mut refine.th_twolayer_lnd, 2, value),
            ThresholdField::Tkdry => set_layer_pair(&mut refine.th_twolayer_lnd, 4, value),
            ThresholdField::Tksatf => set_layer_pair(&mut refine.th_twolayer_lnd, 6, value),
            ThresholdField::Tksatu => set_layer_pair(&mut refine.th_twolayer_lnd, 8, value),
            ThresholdField::Sst => set_pair(&mut refine.th_onelayer_ocn, 0, value),
            ThresholdField::Ssh => set_pair(&mut refine.th_onelayer_ocn, 2, value),
            ThresholdField::Eke => set_pair(&mut refine.th_onelayer_ocn, 4, value),
            ThresholdField::SeaSlope => set_pair(&mut refine.th_onelayer_ocn, 6, value),
            ThresholdField::Typhoon => set_pair(&mut refine.th_onelayer_atmos, 0, value),
        }
    }
}

fn set_pair<const N: usize>(values: &mut [f64; N], start: usize, value: f64) {
    if let Some(slot) = values.get_mut(start) {
        *slot = value;
    }
    if let Some(slot) = values.get_mut(start + 1) {
        *slot = value;
    }
}

fn set_layer_pair(values: &mut [[f64; 2]; 10], start: usize, value: f64) {
    if let Some(slot) = values.get_mut(start) {
        *slot = [value; 2];
    }
    if let Some(slot) = values.get_mut(start + 1) {
        *slot = [value; 2];
    }
}
