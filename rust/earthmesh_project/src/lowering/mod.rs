use crate::{
    km_to_nxp, DomainConfig, ProjectConfig, ProjectLayerRole, RegionShape, ResolutionSpec,
    ViolationPolicy,
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
        format!(
            "{}\n{}{}\n{}",
            self.mkgrd.to_mkgrd_namelist(),
            mkrefine,
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
            on_violation: match self.quality.on_violation {
                ViolationPolicy::Block => "block".to_string(),
                ViolationPolicy::Warn => "warn".to_string(),
            },
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
        }

        match &self.domain {
            DomainConfig::Global => mkgrd.mask_domain_global = true,
            DomainConfig::Regional { shape, sea_ratio } => {
                mkgrd.mask_domain_global = false;
                mkgrd.mask_domain_type = match shape {
                    RegionShape::Bbox { .. } => "bbox",
                    RegionShape::Circle { .. } => "circle",
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
        // Refinement actually runs only when a threshold (refine_cal) or
        // specified-mask (refine_spc) layer supplies data. Landcover/hydro layers
        // set inputs but DON'T drive refinement - turning `refine` on for them
        // sends a data-less run down the OLAM specified-refine path, which then
        // errors ("requires refine_spc/refine_cal/native..."). That is exactly why a
        // land/ocean mesh with only landcover failed to run. Gate the recipe
        // toggle on a real refinement source so such a mesh runs uniform instead.
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

        // The engine runs hex meshes with Istransition=true, and then exactly one of
        // SpringGlobal/SpringRegional may be > 0 (core validate_like_read_nl). Tri
        // meshes keep is_transition=false (the engine then zeroes both spring types).
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

        Ok(LoweredProject {
            mkgrd,
            refine,
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
