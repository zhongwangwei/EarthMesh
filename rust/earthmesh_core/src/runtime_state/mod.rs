use crate::{
    DelaunayMemory, EarthmeshConfig, GridMemory, IjTabs, MeshMemoryShape, RefineConfig,
    EARTH_RADIUS_METERS,
};

/// Derived Earth radius values initialized by `mkgrd.F90:init_consts`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthRadii {
    pub radius_meters: f64,
    pub double_radius_meters: f64,
    pub radius_over_sqrt_five_meters: f64,
    pub inverse_radius_meters: f64,
    pub double_radius_squared_meters: f64,
}

impl EarthRadii {
    /// Build the same secondary radius values that Fortran initializes from `erad`.
    pub fn from_radius_meters(radius_meters: f64) -> Self {
        let double_radius_meters = radius_meters * 2.0;
        Self {
            radius_meters,
            double_radius_meters,
            radius_over_sqrt_five_meters: radius_meters / 5.0_f64.sqrt(),
            inverse_radius_meters: 1.0 / radius_meters,
            double_radius_squared_meters: double_radius_meters * double_radius_meters,
        }
    }
}

impl Default for EarthRadii {
    fn default() -> Self {
        Self::from_radius_meters(EARTH_RADIUS_METERS)
    }
}

/// Rust-owned replacement for `MOD_data_preprocess` source-grid globals kept in
/// `consts_coms`.
///
/// Fortran derives `nlons_source` and `nlats_source` from
/// `gridnum_perdegree`, then records `maxlc` after reading the landtype source.
/// Keeping these values on the explicit runtime state removes another implicit
/// handoff from the old module-global bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceGridState {
    pub nlons_source: usize,
    pub nlats_source: usize,
    pub maxlc: usize,
}

/// Rust-owned replacement for `consts_coms` mask counter globals.
///
/// The Fortran driver mutates `mask_domain_ndm`, `mask_refine_ndm(0:9)`, and
/// `mask_patch_ndm(0:9)` while applying `Mask_make`. Keeping the final counters
/// on runtime state makes downstream Area_judge/refine handoffs explicit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MaskCounterState {
    pub mask_domain_ndm: usize,
    pub mask_refine_ndm: [usize; 10],
    pub mask_patch_ndm: [usize; 10],
}

/// Rust-owned scalar defaults that used to live as `consts_coms` module
/// globals or top-level `mkgrd` initialization assignments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuntimeScalarState {
    pub rinit: f32,
    pub rinit8: f64,
    pub iunit: i32,
    pub io6: i32,
    pub num_center: usize,
}

impl Default for RuntimeScalarState {
    fn default() -> Self {
        Self {
            rinit: 0.0,
            rinit8: 0.0,
            iunit: 10,
            io6: 6,
            num_center: 1,
        }
    }
}

#[deprecated(note = "use RuntimeScalarState")]
pub type LegacyScalarState = RuntimeScalarState;

/// Rust-owned replacement for the production `consts_coms` + `mem_*` global
/// bundle used by the legacy Fortran driver.
///
/// The individual memory structs preserve the old allocation/default rules;
/// this container makes the runtime dependency explicit so downstream mkgrd and
/// refine code can receive state by value/reference instead of reading module
/// globals.
#[derive(Debug, Clone, PartialEq)]
pub struct EarthmeshRuntimeState {
    pub config: EarthmeshConfig,
    pub refine: Option<RefineConfig>,
    pub radii: EarthRadii,
    pub grid: GridMemory,
    pub ijtabs: IjTabs,
    pub delaunay: DelaunayMemory,
    pub source_grid: SourceGridState,
    pub mask_counts: MaskCounterState,
    pub scalars: RuntimeScalarState,
    pub pentagon_indices: [usize; 12],
    pub step: usize,
    pub num_vertex: usize,
    pub num_mp_step: [usize; 10],
    pub num_wp_step: [usize; 10],
}

impl EarthmeshRuntimeState {
    /// Initialize the non-allocating runtime state that Fortran gets from
    /// `consts_coms` defaults plus `mkgrd:init_consts`.
    pub fn new(config: EarthmeshConfig) -> Self {
        Self {
            config,
            refine: None,
            radii: EarthRadii::default(),
            grid: GridMemory::default(),
            ijtabs: IjTabs::default(),
            delaunay: DelaunayMemory::default(),
            source_grid: SourceGridState::default(),
            mask_counts: MaskCounterState::default(),
            scalars: RuntimeScalarState::default(),
            pentagon_indices: [0; 12],
            step: 1,
            num_vertex: 0,
            num_mp_step: [1; 10],
            num_wp_step: [1; 10],
        }
    }

    /// Attach the typed `refine_vars` replacement parsed from `/mkrefine/`.
    pub fn with_refine_config(mut self, refine: RefineConfig) -> Self {
        self.refine = Some(refine);
        self
    }

    /// Return the configured `nxp` as a Rust index/count, rejecting the legacy
    /// uninitialized or invalid non-positive values at the state boundary.
    pub fn try_nxp(&self) -> Result<usize, String> {
        if self.config.nxp <= 0 {
            return Err(format!(
                "EarthmeshRuntimeState requires positive NXP, got {}",
                self.config.nxp
            ));
        }
        usize::try_from(self.config.nxp).map_err(|_| {
            format!(
                "EarthmeshRuntimeState NXP {} cannot be represented as usize",
                self.config.nxp
            )
        })
    }

    /// Convenience accessor for already-validated runtime states.
    pub fn nxp(&self) -> usize {
        self.try_nxp()
            .expect("EarthmeshRuntimeState requires positive NXP")
    }

    /// Update the current `mkgrd` loop step in the Rust-owned runtime state.
    pub fn with_step(mut self, step: usize) -> Self {
        self.step = step;
        self
    }

    /// Record real mesh point counts for a Fortran-style 1-based `mkgrd` step.
    ///
    /// Fortran stores `num_mp_step(step)` and `num_wp_step(step)` after reading
    /// or generating a grid.  The Rust array uses `step - 1` as the storage
    /// slot while keeping `self.step` in the same 1-based convention as the
    /// migrated orchestration.
    pub fn record_mesh_counts_for_step(
        &mut self,
        step: usize,
        num_mp: usize,
        num_wp: usize,
    ) -> Result<(), String> {
        let Some(slot) = step.checked_sub(1) else {
            return Err("mkgrd step must be 1-based when recording mesh counts".to_string());
        };
        if slot >= self.num_mp_step.len() || slot >= self.num_wp_step.len() {
            return Err(format!(
                "mkgrd step {step} exceeds runtime mesh-count storage length {}",
                self.num_mp_step.len()
            ));
        }
        self.step = step;
        self.num_mp_step[slot] = num_mp;
        self.num_wp_step[slot] = num_wp;
        Ok(())
    }

    /// Record the legacy `num_vertex` boundary reported by `Get_Contain`.
    ///
    /// Fortran kept this value in `consts_coms` as an implicit module-global
    /// handoff between containment and postprocess code. Rust keeps it on the
    /// explicit runtime state and rejects the uninitialized zero sentinel when a
    /// production handoff attempts to record it.
    pub fn record_num_vertex(&mut self, num_vertex: usize) -> Result<(), String> {
        if num_vertex == 0 {
            return Err("num_vertex must be positive when recording runtime state".to_string());
        }
        self.num_vertex = num_vertex;
        Ok(())
    }

    /// Record the `MOD_GetContain` refine-area `num_center` handoff.
    ///
    /// Fortran derives `num_center = num_wp_step(step-1)` before computing
    /// refine-area containment. Rust keeps `step` 1-based like the driver while
    /// reading the previous step from the zero-based storage slot.
    pub fn record_num_center_from_previous_step(&mut self, step: usize) -> Result<(), String> {
        if step <= 1 {
            return Err(
                "Get_Contain refine num_center handoff requires step greater than 1".to_string(),
            );
        }
        if step > self.num_wp_step.len() {
            return Err(format!(
                "mkgrd step {step} exceeds runtime mesh-count storage length {}",
                self.num_wp_step.len()
            ));
        }
        let previous_slot = step - 2;
        if previous_slot >= self.num_wp_step.len() {
            return Err(format!(
                "mkgrd previous step for num_center handoff exceeds runtime mesh-count storage length {}",
                self.num_wp_step.len()
            ));
        }
        self.step = step;
        self.scalars.num_center = self.num_wp_step[previous_slot];
        Ok(())
    }

    /// Record the `icosahedron` `impent(12)` scratch handoff explicitly.
    ///
    /// The legacy Fortran icosahedron initializer stores the 12 pentagonal
    /// M-point indices in `consts_coms:impent`; keeping them here avoids another
    /// hidden module-global dependency when spring/grid kernels need the same
    /// pentagon markers.
    pub fn record_pentagon_indices_from_icosahedron(
        &mut self,
        indices: [usize; 12],
    ) -> Result<(), String> {
        if indices.contains(&0) {
            return Err(
                "icosahedron impent pentagon indices must be positive when recorded".to_string(),
            );
        }
        self.pentagon_indices = indices;
        Ok(())
    }

    /// Record source-grid dimensions and maximum land class from the
    /// `MOD_data_preprocess` stage.
    ///
    /// Fortran stores `nlons_source = gridnum_perdegree * 360`,
    /// `nlats_source = gridnum_perdegree * 180`, and `maxlc =
    /// maxval(landtypes_global)` in `consts_coms`. Rust derives the dimensions
    /// from the typed config and keeps the resulting handoff explicit.
    pub fn record_data_preprocess_source_grid(&mut self, maxlc: usize) -> Result<(), String> {
        if self.config.gridnum_perdegree <= 0 {
            return Err(format!(
                "gridnum_perdegree must be positive when recording source-grid state, got {}",
                self.config.gridnum_perdegree
            ));
        }
        let gridnum = usize::try_from(self.config.gridnum_perdegree).map_err(|_| {
            format!(
                "gridnum_perdegree {} cannot be represented as usize",
                self.config.gridnum_perdegree
            )
        })?;
        let nlons_source = gridnum
            .checked_mul(360)
            .ok_or_else(|| "nlons_source overflow while recording source-grid state".to_string())?;
        let nlats_source = gridnum
            .checked_mul(180)
            .ok_or_else(|| "nlats_source overflow while recording source-grid state".to_string())?;
        self.source_grid = SourceGridState {
            nlons_source,
            nlats_source,
            maxlc,
        };
        Ok(())
    }

    /// Allocate all migrated `mem_grid`, `mem_ijtabs`, and `mem_delaunay`
    /// buffers from one explicit shape.
    pub fn allocate_mesh_memories(&mut self, shape: MeshMemoryShape) {
        self.grid.nma = shape.nma;
        self.grid.nua = shape.nua;
        self.grid.nva = shape.nva;
        self.grid.nwa = shape.nwa;
        self.grid.mma = shape.mma;
        self.grid.mua = shape.mua;
        self.grid.mva = shape.mva;
        self.grid.mwa = shape.mwa;
        self.grid.allocate_xyzem(shape.mma);
        self.grid.allocate_xyzew(shape.mwa);
        self.grid
            .allocate_grid_lonlatmw(shape.mma, shape.mva, shape.mwa);
        self.ijtabs = IjTabs::allocate(shape.mma, shape.mva, shape.mwa);
        self.delaunay
            .allocate_itabsd(shape.mma, shape.mua, shape.mwa);
    }

    #[deprecated(note = "use allocate_mesh_memories")]
    pub fn allocate_legacy_memories(&mut self, shape: MeshMemoryShape) {
        self.allocate_mesh_memories(shape);
    }
}
